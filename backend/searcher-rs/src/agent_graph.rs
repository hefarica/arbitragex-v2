//! Bounded cycle discovery and exact V2 integer quotes.
//! No fixed prices, fees, decimals, weights, profitability or gas estimates.
use crate::rhai_agent_bridge::{canonical_hash, Usd};
use bigdecimal::BigDecimal;
use ethers::types::{U256, U512};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub edge_id: String,
    pub pool_id: String,
    pub chain_id: u64,
    pub token_in: String,
    pub token_out: String,
    pub protocol: String,
    pub snapshot_id: String,
    pub block_hash: String,
    pub reserve_in_raw: Option<String>,
    pub reserve_out_raw: Option<String>,
    pub fee_units: Option<u32>,
    pub fee_denominator: Option<u32>,
    pub token_in_decimals: u8,
    pub token_out_decimals: u8,
    pub adapter_version: String,
    /// V3 within-tick spot quote inputs, from the SAME cached slot0 the
    /// `get_v3_slot0` host binding reads (sqrt_price_x96 decimal string,
    /// liquidity raw). `None` = no cached slot0 → the edge stays honest and
    /// `quote_path` demands a producer exact quote.
    #[serde(default)]
    pub sqrt_price_x96_raw: Option<String>,
    #[serde(default)]
    pub liquidity: Option<u128>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchLimits {
    pub max_hops: usize,
    pub max_expansions: usize,
    pub max_paths: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReport {
    pub paths: Vec<Vec<String>>,
    pub expansions: usize,
    pub truncated: bool,
    pub stopping_reason: Option<String>,
}
/// Enumerate directed SIMPLE cycles, including reverse edges when supplied.
/// Do not use ordinary Dijkstra: edge economics depend on size and pool state.
pub fn enumerate_cycles(
    edges: &[Edge],
    start: &str,
    limits: &SearchLimits,
) -> Result<SearchReport, String> {
    if !(2..=7).contains(&limits.max_hops) || limits.max_paths == 0 || limits.max_expansions == 0 {
        return Err("invalid_search_limits".into());
    }
    if start.is_empty() {
        return Err("missing_start_token".into());
    }
    let mut adjacency: BTreeMap<&str, Vec<&Edge>> = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for e in edges {
        if e.edge_id.is_empty()
            || e.pool_id.is_empty()
            || e.token_in == e.token_out
            || !ids.insert(&e.edge_id)
        {
            return Err("invalid_or_duplicate_graph_edge".into());
        }
        adjacency.entry(&e.token_in).or_default().push(e);
    }
    for list in adjacency.values_mut() {
        list.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
    }
    struct Walk<'a> {
        adj: &'a BTreeMap<&'a str, Vec<&'a Edge>>,
        start: &'a str,
        limits: &'a SearchLimits,
        report: SearchReport,
    }
    impl<'a> Walk<'a> {
        fn visit(
            &mut self,
            token: &str,
            path: &mut Vec<String>,
            tokens: &mut BTreeSet<String>,
            pools: &mut BTreeSet<(u64, String)>,
        ) {
            if self.report.truncated || path.len() >= self.limits.max_hops {
                return;
            }
            let Some(next) = self.adj.get(token).cloned() else {
                return;
            };
            for edge in next {
                if self.report.expansions >= self.limits.max_expansions {
                    self.report.truncated = true;
                    self.report.stopping_reason = Some("expansion_budget_exhausted".into());
                    return;
                }
                self.report.expansions += 1;
                let pool = (edge.chain_id, edge.pool_id.clone());
                if pools.contains(&pool) {
                    continue;
                }
                let closes = edge.token_out == self.start;
                if closes && path.len() + 1 < 2 {
                    continue;
                }
                if !closes && tokens.contains(&edge.token_out) {
                    continue;
                }
                path.push(edge.edge_id.clone());
                pools.insert(pool.clone());
                if closes {
                    if self.report.paths.len() >= self.limits.max_paths {
                        self.report.truncated = true;
                        self.report.stopping_reason = Some("path_budget_exhausted".into());
                    } else {
                        self.report.paths.push(path.clone());
                    }
                } else {
                    tokens.insert(edge.token_out.clone());
                    self.visit(&edge.token_out, path, tokens, pools);
                    tokens.remove(&edge.token_out);
                }
                pools.remove(&pool);
                path.pop();
                if self.report.truncated {
                    return;
                }
            }
        }
    }
    let mut walker = Walk {
        adj: &adjacency,
        start,
        limits,
        report: SearchReport {
            paths: Vec::new(),
            expansions: 0,
            truncated: false,
            stopping_reason: None,
        },
    };
    let mut tokens = BTreeSet::new();
    tokens.insert(start.to_owned());
    walker.visit(start, &mut Vec::new(), &mut tokens, &mut BTreeSet::new());
    Ok(walker.report)
}
fn u256(s: &str) -> Result<U256, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s != "0" && s.starts_with('0')) {
        return Err("invalid_base_unit_integer".into());
    }
    U256::from_dec_str(s).map_err(|_| "u256_overflow".into())
}
fn as512(v: U256) -> U512 {
    let mut b = [0u8; 32];
    v.to_big_endian(&mut b);
    U512::from_big_endian(&b)
}
/// xOut = floor(Rout * xIn * (den-fee) / (Rin*den + xIn*(den-fee))).
/// V2 reserves are uint112. Products fit U512 even for uint256 input amounts.
pub fn cpmm_exact_in(
    amount: &str,
    reserve_in: &str,
    reserve_out: &str,
    fee: u32,
    den: u32,
) -> Result<String, String> {
    if den == 0 || fee >= den {
        return Err("invalid_fee_fraction".into());
    }
    let x = u256(amount)?;
    let ri = u256(reserve_in)?;
    let ro = u256(reserve_out)?;
    let max_res = (U256::one() << 112) - U256::one();
    if ri.is_zero() || ro.is_zero() || ri > max_res || ro > max_res {
        return Err("invalid_v2_reserve_uint112".into());
    }
    let dx = as512(x) * U512::from(den - fee);
    let num = as512(ro) * dx;
    let div = as512(ri) * U512::from(den) + dx;
    if div.is_zero() {
        return Err("zero_denominator".into());
    }
    let out = num / div;
    // Output cannot exceed uint112 reserve_out; no narrowing truncation.
    if out > as512(ro) {
        return Err("quote_out_exceeds_reserve".into());
    }
    Ok(out.to_string())
}
/// Exact rational display metrics. V2 fees are embedded in the pool curve,
/// not separate token transfers. Rational raw units avoid inventing rounding.
fn cpmm_metrics(
    amount: &str,
    ri: &str,
    ro: &str,
    fee: u32,
    den: u32,
    out: &str,
) -> Result<Value, String> {
    let x = as512(u256(amount)?);
    let a = as512(u256(ri)?);
    let b = as512(u256(ro)?);
    let y = as512(u256(out)?);
    let fee_num = x * U512::from(fee);
    let ref_num = x * U512::from(den - fee) * b;
    let ref_den = a * U512::from(den);
    let impact = if ref_num.is_zero() {
        json!({"status":"NOT_APPLICABLE","reason":"zero_input_has_no_relative_impact"})
    } else {
        json!({"status":"COMPUTED","numerator":(ref_num-y*ref_den).to_string(),"denominator":ref_num.to_string(),"basis":"fee_adjusted_marginal_output_including_integer_rounding"})
    };
    Ok(
        json!({"lp_fee_input_raw":{"status":"COMPUTED","numerator":fee_num.to_string(),"denominator":den.to_string(),"treatment":"embedded"},"price_impact_fraction":impact}),
    )
}

/// V3 within-tick EXACT-input spot quote (integer math, Q96, Uniswap V3
/// whitepaper single-tick formulas), fee applied on the input:
///   zero_for_one (token0→token1): sqrtP' = L·sqrtP·Q96 / (L·Q96 + dx'·sqrtP);
///                                 out1 = L·(sqrtP − sqrtP') / Q96
///   one_for_zero (token1→token0): sqrtP' = sqrtP + dx'·Q96 / L;
///                                 out0 = L·Q96·(sqrtP' − sqrtP) / (sqrtP'·sqrtP)
/// with dx' = dx·(den−fee)/den. CONSERVATIVE tick-cross guard (R8): the
/// closed form is only valid while the swap stays inside the current tick;
/// a sqrt-price move > 0.5% (≈ 1% price impact) voids the assumption and the
/// call is rejected with an explicit reason — a real QuoterV2 (tick-crossing)
/// quote is required instead. Never a disguised approximation.
fn v3_spot_within_tick(
    sqrt_price_x96_raw: &str,
    liquidity: u128,
    amount_in_raw: &str,
    fee: u32,
    denominator: u32,
    zero_for_one: bool,
) -> Result<(String, String), String> {
    // V3 fee is denominated in millionths of the pool (pips).
    if denominator != 1_000_000 {
        return Err("invalid_v3_fee_denominator".into());
    }
    if fee >= denominator {
        return Err("invalid_v3_fee_fraction".into());
    }
    let sp = U512::from(u256(sqrt_price_x96_raw)?);
    let l = U512::from(liquidity);
    let dx = U512::from(u256(amount_in_raw)?);
    // Q96 = 2^96.
    let q96 = U512::from(u256("79228162514264337593543950336")?);
    if sp.is_zero() || l.is_zero() {
        return Err("degenerate_v3_pool_state".into());
    }
    let dx_adj = dx * U512::from(denominator - fee) / U512::from(denominator);
    let (sp_next, out) = if zero_for_one {
        let num = l * sp * q96;
        let den = l * q96 + dx_adj * sp;
        let sp_next = num / den;
        let out = l * (sp - sp_next) / q96;
        (sp_next, out)
    } else {
        let sp_next = sp + dx_adj * q96 / l;
        let out = l * q96 * (sp_next - sp) / (sp_next * sp);
        (sp_next, out)
    };
    // Tick-cross guard: |sqrtP' − sqrtP| > sqrtP/200 ⇒ > ~1% price impact.
    let moved = if sp_next > sp {
        sp_next - sp
    } else {
        sp - sp_next
    };
    if moved * U512::from(200u32) > sp {
        return Err("v3_tick_cross_requires_quoter".into());
    }
    let to_u256 = |v: U512| -> Result<String, String> {
        let v256: U256 = v.try_into().map_err(|_| "v3_result_overflow")?;
        Ok(v256.to_string())
    };
    Ok((to_u256(out)?, to_u256(sp_next)?))
}

/// Exact terminating decimal token valuation (USD), without binary floats.
pub fn token_value_usd(raw: &str, decimals: u8, price: &str) -> Result<String, String> {
    let amount = u256(raw)?;
    let px = Usd::parse(price)?;
    if !px.is_positive() {
        return Err("nonpositive_canonical_price".into());
    }
    let scaled = BigDecimal::from_str(&format!("{amount}e-{decimals}"))
        .map_err(|_| "invalid_token_scale")?;
    Ok((scaled * px.0).normalized().to_plain_string())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExactHopQuote {
    pub edge_id: String,
    pub snapshot_id: String,
    pub block_hash: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in_raw: String,
    pub amount_out_raw: String,
    pub quote_id: String,
    pub precision: String,
    pub fees_and_impact_embedded: bool,
    pub adapter_version: String,
    /// Exact protocol-owned fee/impact metrics, with COMPUTED or explicit N/A reason.
    pub metrics: Value,
}
/// A cache entry is supplied by the real protocol adapter/Quoter at the fixed
/// block. It is keyed by exact amount and direction, never just by token pair.
pub fn quote_request_key(edge: &Edge, amount: &str) -> String {
    canonical_hash(
        &json!({"edge_id":edge.edge_id,"snapshot_id":edge.snapshot_id,"block_hash":edge.block_hash,"token_in":edge.token_in,"token_out":edge.token_out,"amount_in_raw":amount,"adapter_version":edge.adapter_version}),
    )
}
pub fn quote_path(
    edges: &[Edge],
    amount: &str,
    exact: &BTreeMap<String, ExactHopQuote>,
) -> Result<Vec<Value>, String> {
    if edges.is_empty() {
        return Err("empty_path".into());
    }
    let mut current = u256(amount)?.to_string();
    let mut ledger = Vec::new();
    let snap = &edges[0].snapshot_id;
    let block = &edges[0].block_hash;
    let mut pools = BTreeSet::new();
    for (i, e) in edges.iter().enumerate() {
        if &e.snapshot_id != snap || &e.block_hash != block || e.chain_id != edges[0].chain_id {
            return Err("mixed_block_or_domain_in_atomic_route".into());
        }
        if !pools.insert((e.chain_id, e.pool_id.clone())) {
            return Err("repeated_pool_requires_stateful_adapter".into());
        }
        if i > 0 && edges[i - 1].token_out != e.token_in {
            return Err("broken_token_path".into());
        }
        let key = quote_request_key(e, &current);
        let (out, qid, method, metrics) = if e.protocol == "cpmm_v2" {
            let ri = e.reserve_in_raw.as_deref().ok_or("missing_reserve_in")?;
            let ro = e.reserve_out_raw.as_deref().ok_or("missing_reserve_out")?;
            let fee = e.fee_units.ok_or("missing_fee_units")?;
            let den = e.fee_denominator.ok_or("missing_fee_denominator")?;
            let out = cpmm_exact_in(&current, ri, ro, fee, den)?;
            let qid = canonical_hash(
                &json!({"request":key,"reserve_in":ri,"reserve_out":ro,"fee":fee,"denominator":den,"out":out}),
            );
            let metrics = cpmm_metrics(&current, ri, ro, fee, den, &out)?;
            (out, qid, "cpmm_exact_integer".to_owned(), metrics)
        } else if e.protocol == "uniswap_v3" && e.sqrt_price_x96_raw.is_some() {
            // V3 within-tick: slot0 cacheado presente → spot quote exacto
            // dentro del tick (guard conservador de cruce — R8).
            let sp = e.sqrt_price_x96_raw.as_deref().ok_or("missing_v3_slot0")?;
            let liq = e.liquidity.ok_or("missing_v3_slot0")?;
            let fee = e.fee_units.ok_or("missing_fee_units")?;
            let den = e.fee_denominator.ok_or("missing_fee_denominator")?;
            let zero_for_one = e.token_in <= e.token_out;
            let (out, sp_next) = v3_spot_within_tick(sp, liq, &current, fee, den, zero_for_one)?;
            let qid = canonical_hash(
                &json!({"request":key,"sqrt_price_x96":sp,"liquidity":liq.to_string(),"fee":fee,"denominator":den,"zero_for_one":zero_for_one,"out":out}), // gitleaks:allow — constantes matematicas de la quote V3 (sqrt/liquidity), no secretos
            );
            let metrics = json!({"status":"COMPUTED","model":"v3_within_tick_single_tick",
                "single_tick_assumption":true,"sqrt_price_x96_next":sp_next,
                "direction":if zero_for_one {"zero_for_one"} else {"one_for_zero"}});
            (out, qid, "v3_spot_within_tick".to_owned(), metrics)
        } else {
            let q = exact
                .get(&key)
                .ok_or("exact_protocol_quote_required_no_cpmm_fallback")?;
            if q.edge_id != e.edge_id
                || q.snapshot_id != e.snapshot_id
                || q.block_hash != e.block_hash
                || q.token_in != e.token_in
                || q.token_out != e.token_out
                || q.amount_in_raw != current
                || q.adapter_version != e.adapter_version
            {
                return Err("quote_context_mismatch".into());
            }
            if q.precision != "protocol_exact_integer"
                || !q.fees_and_impact_embedded
                || q.quote_id.is_empty()
            {
                return Err("quote_is_bound_or_missing_provenance".into());
            }
            let out = u256(&q.amount_out_raw)?.to_string();
            (
                out,
                q.quote_id.clone(),
                q.precision.clone(),
                q.metrics.clone(),
            )
        };
        ledger.push(json!({"index":i,"edge_id":e.edge_id,"pool":e.pool_id,"chain_id":e.chain_id,"protocol":e.protocol,
            "token_in":e.token_in,"token_out":e.token_out,"amount_in_raw":current,"amount_out_raw":out,
            "token_in_decimals":e.token_in_decimals,"token_out_decimals":e.token_out_decimals,
            "snapshot_id":e.snapshot_id,"block_hash":e.block_hash,"quote_id":qid,"quote_method":method,
            "fees_and_impact_embedded":true,"adapter_version":e.adapter_version,"metrics":metrics}));
        current = out;
    }
    Ok(ledger)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_parallel_counterexample() {
        let a = cpmm_exact_in("100000000", "1000000000000", "1000000000000", 30, 10000).unwrap();
        let end = cpmm_exact_in(&a, "1001000000000", "1000000000000", 30, 10000).unwrap();
        assert_eq!(a, "99690060");
        assert_eq!(end, "99281840");
    }
    #[test]
    fn quote_path_computes_cpmm_locally_with_fee() {
        // Edge con la forma EXACTA que produce cartridge_boot (Fase 3c):
        // protocolo cpmm_v2 + fee del intent (30/10000) + reservas reales.
        // quote_path debe computar el quote LOCAL sin productor externo:
        // out = floor(R_out * dx * (den-fee) / (R_in*den + dx*(den-fee))).
        let edge = Edge {
            edge_id: "0xpool".into(),
            pool_id: "0xpool".into(),
            chain_id: 1,
            token_in: "0xa".into(),
            token_out: "0xb".into(),
            protocol: "cpmm_v2".into(),
            snapshot_id: "intent-snap".into(),
            block_hash: "blk-1".into(),
            reserve_in_raw: Some("1000000000000".into()),
            reserve_out_raw: Some("1000000000000".into()),
            fee_units: Some(30),
            fee_denominator: Some(10_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: None,
            liquidity: None,
        };
        let legs = quote_path(std::slice::from_ref(&edge), "100000000", &BTreeMap::new()).unwrap();
        assert_eq!(legs[0]["amount_out_raw"], "99690060");
        assert_eq!(legs[0]["quote_method"], "cpmm_exact_integer");
        assert_eq!(legs[0]["edge_id"], "0xpool");
    }
    #[test]
    fn quote_path_requires_exact_quotes_for_non_cpmm() {
        // V3 (y cualquier protocolo no-cpmm) queda en exact-only por
        // doctrina: sin fallback constant-product, con o sin fee.
        let mut edge = Edge {
            edge_id: "0xpool".into(),
            pool_id: "0xpool".into(),
            chain_id: 1,
            token_in: "0xa".into(),
            token_out: "0xb".into(),
            protocol: "uniswap_v3".into(),
            snapshot_id: "intent-snap".into(),
            block_hash: "blk-1".into(),
            reserve_in_raw: Some("1000000000000".into()),
            reserve_out_raw: Some("1000000000000".into()),
            fee_units: None,
            fee_denominator: None,
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: None,
            liquidity: None,
        };
        let err =
            quote_path(std::slice::from_ref(&edge), "100000000", &BTreeMap::new()).unwrap_err();
        assert!(err.contains("exact_protocol_quote_required_no_cpmm_fallback"));
        edge.fee_units = Some(500);
        edge.fee_denominator = Some(1_000_000);
        let err =
            quote_path(std::slice::from_ref(&edge), "100000000", &BTreeMap::new()).unwrap_err();
        assert!(err.contains("exact_protocol_quote_required_no_cpmm_fallback"));
    }
    #[test]
    fn quote_path_fails_honest_without_fee() {
        // cpmm_v2 sin fee determinable: falla con razón explícita (R8).
        let edge = Edge {
            edge_id: "0xpool".into(),
            pool_id: "0xpool".into(),
            chain_id: 1,
            token_in: "0xa".into(),
            token_out: "0xb".into(),
            protocol: "cpmm_v2".into(),
            snapshot_id: "intent-snap".into(),
            block_hash: "blk-1".into(),
            reserve_in_raw: Some("1000000000000".into()),
            reserve_out_raw: Some("1000000000000".into()),
            fee_units: None,
            fee_denominator: None,
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: None,
            liquidity: None,
        };
        let err =
            quote_path(std::slice::from_ref(&edge), "100000000", &BTreeMap::new()).unwrap_err();
        assert!(err.contains("missing_fee_units"));
    }
    #[test]
    fn quote_path_v3_within_tick_hand_verifiable() {
        // sqrtP = Q96 (precio 1:1), L = 1e18, dx = 1e15 (0.1% de L — dentro
        // del tick), fee 3000/1e6.
        // out = L·dx'/ (L + dx') con dx' = dx·997/1000 = 9.97e14
        //     = 1e18·9.97e14/1.000997e18 ≈ 996007192000000 (>0, < dx, ≤ dx').
        let edge = Edge {
            edge_id: "0xpool".into(),
            pool_id: "0xpool".into(),
            chain_id: 1,
            token_in: "0xaaaa".into(),
            token_out: "0xbbbb".into(),
            protocol: "uniswap_v3".into(),
            snapshot_id: "intent-snap".into(),
            block_hash: "blk-1".into(),
            reserve_in_raw: None,
            reserve_out_raw: None,
            fee_units: Some(3_000),
            fee_denominator: Some(1_000_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: Some("79228162514264337593543950336".into()),
            liquidity: Some(1_000_000_000_000_000_000),
        };
        let legs = quote_path(
            std::slice::from_ref(&edge),
            "1000000000000000",
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(legs[0]["quote_method"], "v3_spot_within_tick");
        let out: u128 = legs[0]["amount_out_raw"].as_str().unwrap().parse().unwrap();
        assert!(out > 0, "out must be positive");
        assert!(out < 1_000_000_000_000_000, "out < in (fee, P=1)");
        assert!(out <= 997_000_000_000_000, "out <= dx·(1−fee)");
        // Valor hand-derived (±1e9 por la precisión del cálculo a mano; los
        // floors enteros del algoritmo son sub-unitarios en escala Q96).
        assert!(
            (996_006_000_000_000..=996_008_000_000_000).contains(&out),
            "out {out} vs hand-derived ~996007192000000"
        );
    }
    #[test]
    fn quote_path_v3_within_tick_production_slot0() {
        // sqrt_price_x96/liquidity REALES de producción
        // (log v3_source_priced: sqrt=84480035704410099036497320888,
        //  liquidity=26328339605015878). Monto pequeño dentro del tick.
        let edge = Edge {
            edge_id: "0xv3pool".into(),
            pool_id: "0xv3pool".into(),
            chain_id: 1,
            token_in: "0xaaaa".into(),
            token_out: "0xbbbb".into(),
            protocol: "uniswap_v3".into(),
            snapshot_id: "intent-snap".into(),
            block_hash: "blk-1".into(),
            reserve_in_raw: None,
            reserve_out_raw: None,
            fee_units: Some(3_000),
            fee_denominator: Some(1_000_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: Some("84480035704410099036497320888".into()),
            liquidity: Some(26_328_339_605_015_878),
        };
        let legs =
            quote_path(std::slice::from_ref(&edge), "10000000000", &BTreeMap::new()).unwrap();
        assert_eq!(legs[0]["quote_method"], "v3_spot_within_tick");
        let out: u128 = legs[0]["amount_out_raw"].as_str().unwrap().parse().unwrap();
        assert!(out > 0, "within-tick out must be positive");
        // Cota matemática del precio spot (zero_for_one: precio medio ≤ spot):
        // out · Q96² · den ≤ sqrtP² · dx · (den − fee).
        let sp = U512::from_dec_str("84480035704410099036497320888").unwrap();
        let q96 = U512::from_dec_str("79228162514264337593543950336").unwrap();
        let out512 = U512::from(out);
        assert!(
            out512 * q96 * q96 * U512::from(1_000_000u32)
                <= sp * sp * U512::from(10_000_000_000u64) * U512::from(997_000u32),
            "out bounded by the spot price"
        );
        // Determinista.
        let legs2 =
            quote_path(std::slice::from_ref(&edge), "10000000000", &BTreeMap::new()).unwrap();
        assert_eq!(legs[0]["amount_out_raw"], legs2[0]["amount_out_raw"]);
    }
    #[test]
    fn quote_path_v3_huge_amount_hits_tick_cross_guard() {
        // Monto enorme con la liquidity real de producción: el movimiento de
        // sqrtP supera el guard conservador → razón explícita, jamás una
        // aproximación within-tick disfrazada de exacta (R8).
        let edge = Edge {
            edge_id: "0xv3pool".into(),
            pool_id: "0xv3pool".into(),
            chain_id: 1,
            token_in: "0xaaaa".into(),
            token_out: "0xbbbb".into(),
            protocol: "uniswap_v3".into(),
            snapshot_id: "intent-snap".into(),
            block_hash: "blk-1".into(),
            reserve_in_raw: None,
            reserve_out_raw: None,
            fee_units: Some(3_000),
            fee_denominator: Some(1_000_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: Some("84480035704410099036497320888".into()),
            liquidity: Some(26_328_339_605_015_878),
        };
        let err = quote_path(
            std::slice::from_ref(&edge),
            "1000000000000000000",
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(err.contains("v3_tick_cross_requires_quoter"));
    }
    #[test]
    fn exact_large_token_valuation() {
        assert_eq!(
            token_value_usd("1000000000000000001", 18, "2000.00000001").unwrap(),
            "2000.00000001000000200000000001"
        );
    }
    #[test]
    fn no_unknown_fees() {
        assert!(cpmm_exact_in("10", "1000", "1000", 10000, 10000).is_err());
    }
    #[test]
    fn zero_output_is_computed() {
        assert_eq!(cpmm_exact_in("1", "1000000", "1", 30, 10000).unwrap(), "0");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteProgress {
    pub legs: Vec<Value>,
    pub complete: bool,
    pub failed_hop: Option<usize>,
    pub reason: Option<String>,
}
/// Diagnostic variant: preserve every completed hop on a later quote failure.
/// A prefix is NEVER valued as if it were the final asset of the full route.
pub fn quote_path_progress(
    edges: &[Edge],
    amount: &str,
    exact: &BTreeMap<String, ExactHopQuote>,
) -> QuoteProgress {
    let mut legs = Vec::new();
    let mut current = amount.to_owned();
    let mut pools = BTreeSet::new();
    if edges.is_empty() {
        return QuoteProgress {
            legs,
            complete: false,
            failed_hop: None,
            reason: Some("empty_path".into()),
        };
    }
    for (i, e) in edges.iter().enumerate() {
        let error = if e.snapshot_id != edges[0].snapshot_id
            || e.block_hash != edges[0].block_hash
            || e.chain_id != edges[0].chain_id
        {
            Some("mixed_block_or_domain_in_atomic_route")
        } else if i > 0 && edges[i - 1].token_out != e.token_in {
            Some("broken_token_path")
        } else if !pools.insert((e.chain_id, e.pool_id.clone())) {
            Some("repeated_pool_requires_stateful_adapter")
        } else {
            None
        };
        if let Some(error) = error {
            return QuoteProgress {
                legs,
                complete: false,
                failed_hop: Some(i),
                reason: Some(error.into()),
            };
        }
        match quote_path(std::slice::from_ref(e), &current, exact) {
            Ok(mut one) => {
                let mut leg = one.remove(0);
                leg["index"] = json!(i);
                current = leg["amount_out_raw"].as_str().unwrap_or("").to_owned();
                legs.push(leg);
            }
            Err(error) => {
                return QuoteProgress {
                    legs,
                    complete: false,
                    failed_hop: Some(i),
                    reason: Some(error),
                }
            }
        }
    }
    QuoteProgress {
        legs,
        complete: true,
        failed_hop: None,
        reason: None,
    }
}
