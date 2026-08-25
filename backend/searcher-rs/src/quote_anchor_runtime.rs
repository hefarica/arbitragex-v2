//! EMIT-02 Layer-2 (ARBX-FE-EMIT-02, FE-MASTER P4): runtime quote-anchor
//! selection over LIVE graph data — the writer behind `GET /api/quote/anchor`.
//!
//! The workbook (05_QUOTE_BASE) defines the SCORE FORM and its fixtures but
//! prescribes NO runtime derivation for the five axes — this module is the
//! repo's documented derivation, every axis computed from the live scan with
//! zero hardcoded token lists (05 r16: a dynamic score outranks any
//! stablecoin list) and zero fabricated values (RULE 00 / R8):
//!
//! | Axis | Runtime derivation (all inputs real) |
//! |---|---|
//! | `liquidity` | Σ over the token's pools of its own side reserve valued at the live price (`arbx:token_prices:<chain>`, G-PRICE-1). The side reserve is reconstructed EXACTLY from `RouteEdge` magnitudes: `hint = r_a + r_b` and `log_weight = −ln((1−fee)·rate)` ⇒ `r_a = hint/(1+rate)` (exact when `fee_bps` is `Some`; the `None` path drops the `(1−fee)` correction — bounded by one fee tier, documented approximation). |
//! | `venue_coverage` | distinct `(protocol, fee_bps)` combinations among the token's pools — V2/V3/fee-tier is the honest venue granularity the pool index carries today (brand-level RouterKind is not per-pool; same boundary convention as the `venues` wire mapping). |
//! | `stability` | `100·e^(−mean_dispersion)` where dispersion is the max pairwise `|Δln net_rate|` across PARALLEL pools of the same pair — observed cross-venue pricing coherence. A pair with fewer than 2 rate-computable pools contributes 0 (no evidence of dispersion); orientation is normalized via reciprocal (net-rate fee asymmetry is second-order for dispersion). |
//! | `cross_dex` | `100 · (pairs with ≥2 distinct pools / total pairs)` — share of the token's pairs with venue redundancy. |
//! | `prior` | `100 · degree/max_degree` — structural degree centrality ("Peso prior estructural", 01_CONFIG), distinct counterparties over the scan. |
//!
//! `liquidity`, `venue_coverage` and `prior` are normalized to the scan's
//! observed maximum (relative-but-real: monotone in the underlying quantity,
//! deterministic, no absolute anchors invented); `stability` and `cross_dex`
//! are absolute 0–100 by construction.
//!
//! Candidacy (R8): a token is scoreable only when it has a symbol, a live
//! price and ≥1 pool with a price-valued liquidity contribution. Tokens
//! without a price are EXCLUDED from candidacy (the axis is not computable —
//! never zero-filled); they still count as counterparties in other tokens'
//! stats. An empty candidate set publishes NOTHING — the Redis key expires
//! and the endpoint serves its honest 503.
//!
//! Versioning (09 r25 + EMIT-04 convention): `arbx:quote:version:<chain>` is
//! a monotonic counter INCR'd ONLY when the selected anchor symbol actually
//! changes (a same-anchor republish keeps the version — minimal churn).
//! `graph_version` rides the block the graph was built over (monotone proxy —
//! `GraphBuildOutcome` carries no separate topology version).
//!
//! The wire payloads are built by `quote_anchor_signal` (EXACT key contract,
//! `.strict()` Zod on the other side) — this module never formats wire keys
//! itself beyond composing those builders.

use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use tracing::{debug, warn};

use crate::quote_anchor_signal::{anchor_view_to_wire, token_row_to_wire, TokenRef};
use crate::quote_score::{quote_score, QuoteComponents, QuoteWeights};

/// Redis key holding the latest quote-anchor snapshot for one chain. Read by
/// `GET /api/quote/anchor` + `POST /api/admin/quote/preview` (api-server).
pub const QUOTE_ANCHOR_KEY_PREFIX: &str = "arbx:quote:anchor:";

/// Monotonic per-chain anchor-version counter (INCR on actual anchor change).
pub const QUOTE_VERSION_KEY_PREFIX: &str = "arbx:quote:version:";

/// Snapshot TTL: tick cadence (~30s) + slack — a reader between two ticks
/// always finds a fresh-enough snapshot; a dead searcher lets it EXPIRE into
/// the endpoint's honest 503 (never a stale-forever anchor).
pub const QUOTE_ANCHOR_TTL_SECS: u64 = 35;

pub fn quote_anchor_key(chain_id: u64) -> String {
    format!("{QUOTE_ANCHOR_KEY_PREFIX}{chain_id}")
}

pub fn quote_version_key(chain_id: u64) -> String {
    format!("{QUOTE_VERSION_KEY_PREFIX}{chain_id}")
}

/// ONE pool's contribution, extracted by the worker glue from the graph
/// (deduped by pool address — each pool yields two directed `RouteEdge`s,
/// the glue keeps one canonical direction; fields are address-KEYED strings,
/// lowercase, so the pure core stays ethers-free and probe-testable).
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteAnchorEdgeStat {
    pub pool: String,
    /// Directed orientation of `log_weight`: `rate = r_b / r_a`.
    pub token_a: String,
    pub token_b: String,
    pub protocol: String,
    pub fee_bps: Option<u32>,
    /// `r_a + r_b` in display units (decimals-normalized). `None` = not
    /// computable from fresh on-chain data.
    pub liquidity_hint: Option<f64>,
    /// `−ln((1−fee)·rate)` for the `a→b` direction. `None` = no rate.
    pub log_weight: Option<f64>,
}

/// Per-token aggregation before axis normalization.
#[derive(Debug, Default)]
struct TokenAcc {
    pools: std::collections::HashSet<String>,
    venues: std::collections::HashSet<(String, Option<u32>)>,
    counterparties: std::collections::HashSet<String>,
    /// counterparty → distinct pool count (cross_dex numerator/denominator).
    pair_pools: std::collections::HashMap<String, usize>,
    /// per multi-pool pair: max pairwise |Δln net_rate| (stability input).
    pair_dispersion: std::collections::HashMap<String, f64>,
    liquidity_valued: f64,
    priced_pools: usize,
}

/// The pure selection result: scored rows (best-first), the winner, and the
/// sidecar counts the preview math needs. `None` when NO token is scoreable.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteAnchorSelection {
    pub rows: Vec<(TokenRef, QuoteComponents, f64)>,
    pub anchor: usize,
    /// symbol → distinct counterparty count (affected_pairs of a preview).
    pub pairs_by_symbol: std::collections::BTreeMap<String, u64>,
    /// symbol → distinct pool count (affected_edges of a preview).
    pub pools_by_symbol: std::collections::BTreeMap<String, u64>,
}

impl QuoteAnchorSelection {
    /// The anchor row (best-first head).
    pub fn anchor_row(&self) -> &(TokenRef, QuoteComponents, f64) {
        &self.rows[self.anchor]
    }
}

/// Compute the axes, score every candidate and rank. `prices` maps the
/// UPPERCASED symbol → live quote-unit price; `symbol_of` maps a lowercase
/// address → display symbol. Deterministic: rows sort by score desc, then
/// symbol asc, then address asc (stable across re-sortings).
pub fn select_quote_anchor(
    chain_id: u64,
    edges: &[QuoteAnchorEdgeStat],
    symbol_of: &std::collections::HashMap<String, String>,
    prices: &std::collections::HashMap<String, f64>,
    weights: &QuoteWeights,
) -> Option<QuoteAnchorSelection> {
    let _ = chain_id; // carried by the snapshot, not the math
    let mut acc: std::collections::HashMap<String, TokenAcc> = std::collections::HashMap::new();
    // Per pair (sorted token key) → per pool → ln(net_rate) oriented lo→hi
    // (stability dispersion input; ONE entry per pool).
    let mut pair_rates: std::collections::HashMap<
        (String, String),
        std::collections::HashMap<String, f64>,
    > = std::collections::HashMap::new();

    for e in edges {
        if e.token_a.is_empty() || e.token_b.is_empty() || e.pool.is_empty() {
            continue; // degenerate row — skip, never fabricate
        }
        let ln_net_rate = e.log_weight.filter(|w| w.is_finite()).map(|w| -w);

        // Stability input: ONE insert per pool, orientation normalized to the
        // SORTED pair key (lo→hi) — pools of the same pair may carry opposite
        // edge directions in the glue, and dispersion is only meaningful over
        // a consistent orientation. Per-side accumulation happens below.
        if let Some(lnr) = ln_net_rate {
            let mut key = [e.token_a.clone(), e.token_b.clone()];
            key.sort();
            let oriented = if e.token_a == key[0] { lnr } else { -lnr };
            pair_rates
                .entry((key[0].clone(), key[1].clone()))
                .or_default()
                .insert(e.pool.clone(), oriented);
        }

        for (side, other, flip) in [
            (e.token_a.clone(), e.token_b.clone(), false),
            (e.token_b.clone(), e.token_a.clone(), true),
        ] {
            let a = acc.entry(side.clone()).or_default();
            a.pools.insert(e.pool.clone());
            a.venues.insert((e.protocol.clone(), e.fee_bps));
            a.counterparties.insert(other.clone());
            *a.pair_pools.entry(other.clone()).or_insert(0) += 1;

            // Liquidity: this side's reserve, valued at the live price.
            if let (Some(hint), Some(lw)) = (e.liquidity_hint, e.log_weight) {
                if hint.is_finite() && hint > 0.0 && lw.is_finite() {
                    let fee = e.fee_bps.unwrap_or(0) as f64 / 10_000.0;
                    let rate = (-lw).exp() / (1.0 - fee);
                    if rate.is_finite() && rate > 0.0 {
                        let r_a = hint / (1.0 + rate); // a-side of THIS edge's orientation
                        let r_side = if flip { hint - r_a } else { r_a };
                        let sym = symbol_of.get(&side).map(|s| s.trim().to_ascii_uppercase());
                        if let Some(sym) = sym {
                            if let Some(p) = prices.get(&sym) {
                                if p.is_finite() && *p > 0.0 {
                                    a.liquidity_valued += r_side * p;
                                    a.priced_pools += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Fold per-pair dispersion into each endpoint's accumulator.
    for ((t0, t1), per_pool) in &pair_rates {
        let lns: Vec<f64> = per_pool.values().copied().collect();
        if lns.len() < 2 {
            continue; // no parallel-rate evidence — contributes 0 (documented)
        }
        let mut spread = 0.0f64;
        for i in 0..lns.len() {
            for j in (i + 1)..lns.len() {
                spread = spread.max((lns[i] - lns[j]).abs());
            }
        }
        for t in [t0, t1] {
            if let Some(a) = acc.get_mut(t) {
                a.pair_dispersion.insert(other_key(t0, t1), spread);
            }
        }
    }

    // Axis maxima over the scan (relative normalization anchors).
    let max_liq = acc
        .values()
        .map(|a| a.liquidity_valued)
        .fold(0.0f64, f64::max);
    let max_venues = acc.values().map(|a| a.venues.len()).max().unwrap_or(0);
    let max_degree = acc
        .values()
        .map(|a| a.counterparties.len())
        .max()
        .unwrap_or(0);

    let mut rows: Vec<(TokenRef, QuoteComponents, f64)> = Vec::new();
    let mut pairs_by_symbol = std::collections::BTreeMap::new();
    let mut pools_by_symbol = std::collections::BTreeMap::new();

    for (addr, a) in &acc {
        // Candidacy (R8): symbol + live price + ≥1 priced liquidity pool.
        let Some(sym_raw) = symbol_of.get(addr) else {
            continue;
        };
        let sym = sym_raw.trim().to_ascii_uppercase();
        if sym.is_empty() {
            continue;
        }
        if !prices.contains_key(&sym) || a.priced_pools == 0 {
            continue; // axis not computable — excluded, never zero-filled
        }
        let components = QuoteComponents {
            prior: norm(a.counterparties.len() as f64, max_degree as f64),
            liquidity: norm(a.liquidity_valued, max_liq),
            venue_coverage: norm(a.venues.len() as f64, max_venues as f64),
            stability: if a.pair_dispersion.is_empty() {
                100.0 // no multi-pool pair observed — no dispersion evidence
            } else {
                let mean = a.pair_dispersion.values().sum::<f64>() / a.pair_dispersion.len() as f64;
                100.0 * (-mean).exp()
            },
            cross_dex: {
                let total = a.pair_pools.len() as f64;
                let multi = a.pair_pools.values().filter(|&&n| n >= 2).count() as f64;
                norm(multi, total)
            },
        };
        let score = quote_score(&components, weights);
        if !score.is_finite() {
            continue; // a NaN must never be selected by accident of ordering
        }
        let tref = TokenRef {
            symbol: sym_raw.trim().to_string(),
            address: addr.clone(),
        };
        pairs_by_symbol.insert(tref.symbol.clone(), a.pair_pools.len() as u64);
        pools_by_symbol.insert(tref.symbol.clone(), a.pools.len() as u64);
        rows.push((tref, components, score));
    }

    if rows.is_empty() {
        return None; // honest: nothing scoreable — publish nothing
    }

    // Deterministic best-first: score desc, then symbol asc, then address asc.
    rows.sort_by(|x, y| {
        y.2.partial_cmp(&x.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| x.0.symbol.cmp(&y.0.symbol))
            .then_with(|| x.0.address.cmp(&y.0.address))
    });

    Some(QuoteAnchorSelection {
        rows,
        anchor: 0,
        pairs_by_symbol,
        pools_by_symbol,
    })
}

fn other_key(t0: &str, t1: &str) -> String {
    format!("{t0}|{t1}")
}

/// Relative normalization to the scan maximum: `0` when the max is `0` (a
/// single-token universe — the axis degenerates honestly), else `100·v/max`.
fn norm(v: f64, max: f64) -> f64 {
    if !(max.is_finite() && max > 0.0) {
        0.0
    } else {
        ((v / max) * 100.0).clamp(0.0, 100.0)
    }
}

/// Build the Redis snapshot payload around the selection (wire keys come from
/// `quote_anchor_signal` — EXACT contract). `quote_version`/`graph_version`
/// are supplied by the writer after the version compare.
pub fn quote_anchor_snapshot_to_wire(
    chain_id: u64,
    sel: &QuoteAnchorSelection,
    weights: &QuoteWeights,
    quote_version: u64,
    graph_version: u64,
) -> Value {
    let (tref, comps, score) = sel.anchor_row();
    let mut view = anchor_view_to_wire(
        chain_id,
        &tref.symbol,
        *score,
        comps,
        quote_version,
        graph_version,
        weights,
    );
    let obj = view.as_object_mut().expect("anchor view is an object");
    obj.insert(
        "tokens".into(),
        Value::Array(
            sel.rows
                .iter()
                .map(|(t, c, s)| token_row_to_wire(t, c, *s))
                .collect(),
        ),
    );
    obj.insert("pairs_by_symbol".into(), json!(sel.pairs_by_symbol));
    obj.insert("pools_by_symbol".into(), json!(sel.pools_by_symbol));
    view
}

/// Pure bump decision: the counter moves ONLY on an actual anchor-symbol
/// change (same-anchor republish keeps the version — minimal churn, 09 r25).
pub fn anchor_symbol_from_snapshot(snapshot: &Value) -> Option<&str> {
    snapshot.get("quote_symbol").and_then(|v| v.as_str())
}

/// ARBX-0023 (05_QUOTE_BASE r12/r13): re-denominate the stream price map
/// into the chosen anchor's quote units — `P_Q(x) = p(x)/p(anchor)`, with
/// the anchor itself at EXACTLY 1.0 (one of itself, by numéraire
/// definition). This is the integration leg from the dynamic selection
/// (`select_quote_anchor`, r16: score over any fixed list) to the
/// evaluation path: the F_e state is filled in the anchor's units, so the
/// published anchor and the F_e ratios describe the same numéraire.
///
/// `None` when the anchor has no usable price in the stream (finite > 0):
/// the caller keeps the stream's raw units (fail-open, counted) — never a
/// fabricated 1.0 (R8). Direction-neutral by construction (r15): F_e
/// consumes only `P_Q(dst)/P_Q(src)` ratios, which a common scaling factor
/// cancels — pinned by test in this module, so choosing the QUOTE can never
/// change a trading direction's verdict.
pub fn redenominate_to_anchor(
    prices: &std::collections::HashMap<String, f64>,
    anchor_symbol: &str,
) -> Option<std::collections::HashMap<String, f64>> {
    let key = anchor_symbol.to_ascii_uppercase();
    let p_anchor = *prices.get(&key)?;
    if !p_anchor.is_finite() || p_anchor <= 0.0 {
        return None;
    }
    let mut out = std::collections::HashMap::with_capacity(prices.len());
    for (sym, p) in prices {
        out.insert(sym.clone(), p / p_anchor);
    }
    // x/x is 1.0 in IEEE-754 for finite positive x, but the numéraire
    // invariant is the contract — set it explicitly, casing-proof.
    out.insert(key, 1.0);
    Some(out)
}

/// Writer side (EMIT-02). Orchestrates: old-symbol read → INCR-if-changed →
/// snapshot SET with TTL. Best-effort like the other signal writers: one warn
/// line on failure, never fatal (the tick keeps running).
pub async fn write_quote_anchor_snapshot(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sel: &QuoteAnchorSelection,
    weights: &QuoteWeights,
    graph_version: u64,
) {
    let key = quote_anchor_key(chain_id);
    let old_symbol: Option<String> = match redis::cmd("GET")
        .arg(&key)
        .query_async::<_, Option<String>>(redis)
        .await
    {
        Ok(raw) => raw
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| anchor_symbol_from_snapshot(&v).map(|s| s.to_string())),
        Err(e) => {
            debug!(event = "quote_anchor.old_read_failed", chain_id, err = %e);
            None
        }
    };

    let new_symbol = sel.anchor_row().0.symbol.clone();
    let changed = old_symbol.as_deref() != Some(new_symbol.as_str());
    let quote_version: u64 = if changed {
        match redis::cmd("INCR")
            .arg(quote_version_key(chain_id))
            .query_async::<_, i64>(redis)
            .await
        {
            Ok(n) if n >= 0 => n as u64,
            Ok(_) | Err(_) => {
                warn!(event = "quote_anchor.version_incr_failed", chain_id);
                return; // without a coherent version the snapshot stays unpublished
            }
        }
    } else {
        // Same anchor: reuse the standing counter (read; absent = 0 honest).
        match redis::cmd("GET")
            .arg(quote_version_key(chain_id))
            .query_async::<_, Option<String>>(redis)
            .await
        {
            Ok(Some(s)) => s.parse::<u64>().unwrap_or(0),
            _ => 0,
        }
    };

    let payload =
        quote_anchor_snapshot_to_wire(chain_id, sel, weights, quote_version, graph_version);
    let json = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            warn!(event = "quote_anchor.snapshot_serialize_failed", chain_id, error = %e);
            return;
        }
    };
    let res: redis::RedisResult<()> = redis::cmd("SET")
        .arg(&key)
        .arg(&json)
        .arg("EX")
        .arg(QUOTE_ANCHOR_TTL_SECS)
        .query_async(redis)
        .await;
    if let Err(e) = res {
        warn!(event = "quote_anchor.snapshot_set_failed", chain_id, error = %e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(
        pool: &str,
        a: &str,
        b: &str,
        protocol: &str,
        fee: Option<u32>,
        hint: Option<f64>,
        lw: Option<f64>,
    ) -> QuoteAnchorEdgeStat {
        QuoteAnchorEdgeStat {
            pool: pool.into(),
            token_a: a.into(),
            token_b: b.into(),
            protocol: protocol.into(),
            fee_bps: fee,
            liquidity_hint: hint,
            log_weight: lw,
        }
    }

    fn env(
        edges: &[QuoteAnchorEdgeStat],
        symbols: &[(&str, &str)],
        prices: &[(&str, f64)],
    ) -> Option<QuoteAnchorSelection> {
        let sym: std::collections::HashMap<String, String> = symbols
            .iter()
            .map(|(a, s)| (a.to_string(), s.to_string()))
            .collect();
        let px: std::collections::HashMap<String, f64> =
            prices.iter().map(|(s, p)| (s.to_string(), *p)).collect();
        select_quote_anchor(1, edges, &sym, &px, &QuoteWeights::default())
    }

    /// Liquidity reconstruction is EXACT when the fee is known: reserves
    /// (1_000_000 USDC, 500 WETH) with rate 3000 USDC/WETH and fee 30bps.
    #[test]
    fn liquidity_reconstruction_exact_with_fee() {
        let r_usdc = 1_000_000.0f64;
        let r_weth = 500.0f64;
        let rate = r_usdc / r_weth; // r_b/r_a with a=WETH, b=USDC
        let fee = 0.0030;
        let lw = -((1.0 - fee) * rate).ln();
        let hint = r_weth + r_usdc;
        let sel = env(
            &[stat(
                "0xp1",
                "0xweth",
                "0xusdc",
                "v2",
                Some(30),
                Some(hint),
                Some(lw),
            )],
            &[("0xweth", "WETH"), ("0xusdc", "USDC")],
            &[("WETH", 3000.0), ("USDC", 1.0)],
        )
        .unwrap();
        // liquidity axis: single pair — both tokens normalize to 100 (each is
        // the max). The EXACTNESS lives in the valued sum; recompute it here
        // from the same inputs the core used.
        let usdc_side = r_usdc * 1.0;
        let weth_side = r_weth * 3000.0;
        // rows sorted score desc; assert both present with liquidity 100.0
        // (relative max with 2 tokens = the larger side; both share axis 100
        // only if equal — here they differ, so verify ordering instead).
        assert_eq!(sel.rows.len(), 2);
        let liq: Vec<f64> = sel.rows.iter().map(|r| r.1.liquidity).collect();
        let expect_max = 100.0;
        assert!(liq
            .iter()
            .all(|l| (*l - 0.0..=expect_max).contains(&0.0) || *l <= 100.0));
        // The proportion: weth_side(1.5e6) > usdc_side(1e6) ⇒ WETH axis 100,
        // USDC axis 100·(1e6/1.5e6) = 66.666…
        let by_sym = |s: &str| sel.rows.iter().find(|r| r.0.symbol == s).unwrap();
        assert!((by_sym("WETH").1.liquidity - 100.0).abs() < 1e-9);
        assert!((by_sym("USDC").1.liquidity - (100.0 * usdc_side / weth_side)).abs() < 1e-6);
        // Sanity of the reconstruction itself: r_weth recovered from hint+lw.
        let rate_rec = (-lw).exp() / (1.0 - fee);
        let r_a_rec = hint / (1.0 + rate_rec);
        assert!((r_a_rec - r_weth).abs() < 1e-6);
        assert!((hint - r_a_rec - r_usdc).abs() < 1e-6);
    }

    /// Venue axis counts distinct (protocol, fee_bps) combos; cross_dex is
    /// the share of pairs with ≥2 pools; stability penalizes rate dispersion.
    #[test]
    fn venues_crossdex_stability_axes() {
        // Token T on: v2 pool + v3-500 pool with the SAME pair (multi-pool),
        // plus a single-pool pair with C.
        let lw1 = -2.0f64; // net rates e^-2 and e^-2.2 across the two pools
        let lw2 = -2.2f64;
        let sel = env(
            &[
                stat(
                    "0xp1",
                    "0xt",
                    "0xc1",
                    "v2",
                    Some(30),
                    Some(2000.0),
                    Some(lw1),
                ),
                stat(
                    "0xp2",
                    "0xt",
                    "0xc1",
                    "v3",
                    Some(5),
                    Some(2000.0),
                    Some(lw2),
                ),
                stat(
                    "0xp3",
                    "0xt",
                    "0xc2",
                    "v2",
                    Some(30),
                    Some(500.0),
                    Some(-1.0),
                ),
            ],
            &[("0xt", "T"), ("0xc1", "C1"), ("0xc2", "C2")],
            &[("T", 1.0), ("C1", 1.0), ("C2", 1.0)],
        )
        .unwrap();
        let t = sel.rows.iter().find(|r| r.0.symbol == "T").unwrap();
        // venues: {(v2,30),(v3,5)} over max venues (2, T itself) → 100
        assert!((t.1.venue_coverage - 100.0).abs() < 1e-9);
        // cross_dex: 1 multi-pool pair of 2 total → 50
        assert!((t.1.cross_dex - 50.0).abs() < 1e-9);
        // stability: single multi-pool pair with dispersion |−2 −(−2.2)| = 0.2
        assert!((t.1.stability - 100.0 * (-0.2f64).exp()).abs() < 1e-9);
        // sidecars
        assert_eq!(sel.pairs_by_symbol.get("T"), Some(&2));
        assert_eq!(sel.pools_by_symbol.get("T"), Some(&3));
    }

    /// No multi-pool pair anywhere → stability is the documented vacuous 100.
    #[test]
    fn stability_vacuous_100_single_pool_pairs() {
        let sel = env(
            &[stat(
                "0xp1",
                "0xt",
                "0xc",
                "v2",
                Some(30),
                Some(100.0),
                Some(-1.0),
            )],
            &[("0xt", "T"), ("0xc", "C")],
            &[("T", 1.0), ("C", 1.0)],
        )
        .unwrap();
        for r in &sel.rows {
            assert!((r.1.stability - 100.0).abs() < 1e-9);
        }
    }

    /// Prior axis = relative degree; single counterparty universe → both 100.
    /// A token WITHOUT a live price is EXCLUDED from candidacy (never
    /// zero-filled) but still counts as a counterparty.
    #[test]
    fn unpriced_token_excluded_but_counts_as_counterparty() {
        let sel = env(
            &[stat(
                "0xp1",
                "0xt",
                "0xu",
                "v2",
                Some(30),
                Some(100.0),
                Some(-1.0),
            )],
            &[("0xt", "T"), ("0xu", "UNPRICED")],
            &[("T", 1.0)], // no price for UNPRICED
        )
        .unwrap();
        assert_eq!(sel.rows.len(), 1);
        assert_eq!(sel.rows[0].0.symbol, "T");
        // T's degree counts UNPRICED → prior = 100 (the max).
        assert!((sel.rows[0].1.prior - 100.0).abs() < 1e-9);
    }

    /// Deterministic ordering: score desc, ties broken by symbol asc then
    /// address asc — stable across input shuffles.
    #[test]
    fn ordering_deterministic_score_desc_symbol_tiebreak() {
        let e1 = stat(
            "0xp1",
            "0xa",
            "0xb",
            "v2",
            Some(30),
            Some(100.0),
            Some(-1.0),
        );
        let e2 = stat(
            "0xp2",
            "0xa",
            "0xc",
            "v2",
            Some(30),
            Some(100.0),
            Some(-1.0),
        );
        let e3 = stat(
            "0xp3",
            "0xd",
            "0xa",
            "v2",
            Some(30),
            Some(100.0),
            Some(-1.0),
        );
        let syms = [
            ("0xa", "AAA"),
            ("0xb", "BBB"),
            ("0xc", "CCC"),
            ("0xd", "DDD"),
        ];
        let px = [("AAA", 1.0), ("BBB", 1.0), ("CCC", 1.0), ("DDD", 1.0)];
        let s1 = env(&[e1.clone(), e2.clone(), e3.clone()], &syms, &px).unwrap();
        let s2 = env(&[e3, e2, e1], &syms, &px).unwrap();
        let symbols = |s: &QuoteAnchorSelection| {
            s.rows
                .iter()
                .map(|r| r.0.symbol.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(symbols(&s1), symbols(&s2));
        // All four tokens are symmetric (same degree/liquidity shape) → all
        // scores equal → pure symbol-asc ordering, AAA first (the anchor).
        assert_eq!(s1.anchor_row().0.symbol, "AAA");
        assert_eq!(symbols(&s1), vec!["AAA", "BBB", "CCC", "DDD"]);
    }

    /// Empty / fully-unpriced input → `None` (publish nothing — honest).
    #[test]
    fn empty_or_unscoreable_yields_none() {
        assert!(env(&[], &[("0xa", "A")], &[("A", 1.0)]).is_none());
        assert!(env(
            &[stat(
                "0xp",
                "0xa",
                "0xb",
                "v2",
                Some(30),
                Some(1.0),
                Some(-1.0)
            )],
            &[("0xa", "A"), ("0xb", "B")],
            &[] // no prices at all
        )
        .is_none());
        // Degenerate rows are skipped: only-degenerate input = empty input.
        assert!(env(
            &[stat("", "", "", "v2", None, None, None)],
            &[("0xa", "A")],
            &[("A", 1.0)]
        )
        .is_none());
    }

    /// Snapshot wire: the anchor view's 7 exact keys + `tokens` rows (4 keys
    /// each) + the two sidecars — nothing else.
    #[test]
    fn snapshot_wire_keys_exact() {
        let sel = env(
            &[stat(
                "0xp1",
                "0xweth",
                "0xusdc",
                "v2",
                Some(30),
                Some(2_000_000.0),
                Some(-0.5),
            )],
            &[("0xweth", "WETH"), ("0xusdc", "USDC")],
            &[("WETH", 3000.0), ("USDC", 1.0)],
        )
        .unwrap();
        let v = quote_anchor_snapshot_to_wire(1, &sel, &QuoteWeights::default(), 7, 12345);
        let obj = v.as_object().unwrap();
        for key in [
            "chain_id",
            "quote_symbol",
            "quote_score",
            "quote_version",
            "graph_version",
            "components",
            "weights",
            "tokens",
            "pairs_by_symbol",
            "pools_by_symbol",
        ] {
            assert!(obj.contains_key(key), "missing key {key}");
        }
        assert_eq!(obj.len(), 10);
        assert_eq!(v["quote_version"], json!(7));
        assert_eq!(v["graph_version"], json!(12345));
        assert_eq!(v["tokens"].as_array().unwrap().len(), 2);
        let row = &v["tokens"][0];
        assert_eq!(row.as_object().unwrap().len(), 4);
        for key in ["symbol", "address", "components", "score"] {
            assert!(row.as_object().unwrap().contains_key(key));
        }
    }

    /// The pure bump decision reads the OLD snapshot's anchor symbol.
    #[test]
    fn anchor_symbol_extraction_from_snapshot() {
        let v = json!({ "quote_symbol": "USDC", "other": 1 });
        assert_eq!(anchor_symbol_from_snapshot(&v), Some("USDC"));
        assert_eq!(anchor_symbol_from_snapshot(&json!({})), None);
    }

    /// Orientation regression: two pools of the SAME pair whose glue edges
    /// carry OPPOSITE directions but AGREEING rates (p1: X→Y at rate r,
    /// p2: Y→X at rate 1/r) must yield dispersion 0 (stability 100) — a
    /// per-side or un-normalized insert would compare r against 1/r and
    /// fabricate dispersion out of consistent prices.
    #[test]
    fn opposite_direction_pools_with_agreeing_rates_have_zero_dispersion() {
        let ln_r = 1.5f64; // ln(rate X→Y) = 1.5
                           // p1 edge X→Y: lw = −ln(r) = −1.5; p2 edge Y→X: lw = −ln(1/r) = +1.5
        let sel = env(
            &[
                stat(
                    "0xp1",
                    "0xx",
                    "0xy",
                    "v2",
                    Some(30),
                    Some(1000.0),
                    Some(-ln_r),
                ),
                stat(
                    "0xp2",
                    "0xy",
                    "0xx",
                    "v2",
                    Some(30),
                    Some(1000.0),
                    Some(ln_r),
                ),
            ],
            &[("0xx", "XX"), ("0xy", "YY")],
            &[("XX", 1.0), ("YY", 1.0)],
        )
        .unwrap();
        for r in &sel.rows {
            assert!(
                (r.1.stability - 100.0).abs() < 1e-9,
                "agreeing pools fabricated dispersion: {} -> {}",
                r.0.symbol,
                r.1.stability
            );
        }
        // Sanity: genuinely disagreeing parallel pools DO register dispersion.
        let sel2 = env(
            &[
                stat(
                    "0xp1",
                    "0xx",
                    "0xy",
                    "v2",
                    Some(30),
                    Some(1000.0),
                    Some(-ln_r),
                ),
                stat(
                    "0xp2",
                    "0xy",
                    "0xx",
                    "v2",
                    Some(30),
                    Some(1000.0),
                    Some(ln_r + 0.4),
                ),
            ],
            &[("0xx", "XX"), ("0xy", "YY")],
            &[("XX", 1.0), ("YY", 1.0)],
        )
        .unwrap();
        // p2's Y→X net rate differs by e^{0.4} from p1's reciprocal ⇒ the
        // lo→hi orientations differ by 0.4 ⇒ stability = 100·e^{−0.4}.
        for r in &sel2.rows {
            assert!((r.1.stability - 100.0 * (-0.4f64).exp()).abs() < 1e-9);
        }
    }

    /// r13 pin (ARBX-0023): the anchor lands at EXACTLY 1.0 and every other
    /// price scales by 1/p(anchor) — P_Q(x) = valor de x en quote anchor.
    #[test]
    fn redenomination_makes_anchor_exactly_one_and_scales_the_rest() {
        let stream: std::collections::HashMap<String, f64> = [
            ("USDC".to_string(), 1.0),
            ("WETH".to_string(), 3000.0),
            ("WBTC".to_string(), 60000.0),
        ]
        .into_iter()
        .collect();
        let q = redenominate_to_anchor(&stream, "weth").unwrap(); // casing tolerated
        assert_eq!(q["WETH"], 1.0);
        assert!((q["USDC"] - 1.0 / 3000.0).abs() < 1e-15);
        assert!((q["WBTC"] - 20.0).abs() < 1e-12);
        assert_eq!(q.len(), 3);
    }

    /// R8 pin (ARBX-0023): an absent / non-positive anchor price yields
    /// `None` — the caller keeps the stream units; a 1.0 is never fabricated
    /// for a token the stream cannot price.
    #[test]
    fn redenomination_is_none_when_anchor_unpriced() {
        let stream: std::collections::HashMap<String, f64> =
            [("WETH".to_string(), 3000.0)].into_iter().collect();
        assert!(redenominate_to_anchor(&stream, "LINK").is_none());
        assert!(redenominate_to_anchor(&std::collections::HashMap::new(), "WETH").is_none());
        // Defense-in-depth: a poisoned row (the worker filters these upstream)
        // still refuses here rather than zero-divide or fabricate.
        let poisoned: std::collections::HashMap<String, f64> =
            [("WETH".to_string(), 0.0)].into_iter().collect();
        assert!(redenominate_to_anchor(&poisoned, "WETH").is_none());
    }

    /// r15 pin (ARBX-0023): choosing the QUOTE re-denominates every price by
    /// a common factor, and F_e consumes only `P_Q(dst)/P_Q(src)` ratios —
    /// so the cycle prefilter's verdict is IDENTICAL in stream units and
    /// anchor units, for a cycle AND its reverse direction. If a
    /// re-denomination ever moved a verdict, choosing the quote would be
    /// restricting trading directions — the exact thing 05_QUOTE_BASE
    /// forbids ("nunca borrar la dirección inversa por elegir QUOTE").
    #[test]
    fn cycle_alpha_is_redenomination_invariant_r15() {
        use crate::fe_normalization::QuoteState;
        use std::collections::HashMap;

        // ids: 0=A, 1=B (the anchor), 2=C.
        let stream: HashMap<String, f64> = [
            ("A".to_string(), 1.0),
            ("B".to_string(), 3000.0),
            ("C".to_string(), 60000.0),
        ]
        .into_iter()
        .collect();
        let anchored = redenominate_to_anchor(&stream, "B").unwrap();
        assert_eq!(anchored["B"], 1.0);

        let mut qs_stream = QuoteState::new(1, 3);
        qs_stream.set_price(0, stream["A"]).unwrap();
        qs_stream.set_price(1, stream["B"]).unwrap();
        qs_stream.set_price(2, stream["C"]).unwrap();

        let mut qs_anchor = QuoteState::new(1, 3);
        qs_anchor.set_price(0, anchored["A"]).unwrap();
        qs_anchor.set_price(1, anchored["B"]).unwrap();
        qs_anchor.set_price(2, anchored["C"]).unwrap();

        // Executable rates per DIRECTED edge (asymmetric, like real venues).
        let rates: HashMap<(usize, usize), f64> = [
            ((0, 1), 2998.5),
            ((1, 0), 1.0 / 3001.0),
            ((1, 2), 19.98),
            ((2, 1), 1.0 / 20.02),
            ((2, 0), 1.0 / 59990.0),
            ((0, 2), 59880.0),
        ]
        .into_iter()
        .collect();
        let rate_of = |a: usize, b: usize| rates.get(&(a, b)).copied();

        for cycle in [vec![0, 1, 2, 0], vec![0, 2, 1, 0]] {
            let a_stream = qs_stream.cycle_ln_alpha(&cycle, &rate_of).unwrap();
            let a_anchor = qs_anchor.cycle_ln_alpha(&cycle, &rate_of).unwrap();
            let (Some(s), Some(t)) = (a_stream, a_anchor) else {
                panic!("fixture must price every edge of {cycle:?}");
            };
            assert!(
                (s - t).abs() < 1e-9,
                "re-denomination moved the verdict of {cycle:?}: {s} vs {t}"
            );
        }
    }
}
