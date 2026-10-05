//! GAS-PRICE-ADAPTER-01 — USD price of the chain's **gas-fee token**, for recon.
//!
//! ## Why this module exists
//!
//! `drift_tracker::compute_gas_cost_usd` converts the simulated re-execution's
//! `gas_used_total × gas_price_wei` into a native-coin amount and needs ONE
//! more input to finish the arithmetic: the native coin's USD price. Until this
//! adapter existed the function carried an explicit `None` placeholder
//! (`drift_tracker.rs`, "MVP uses a nominal placeholder of None … until a
//! reliable ETH-USD feed is wired into recon"), so `actual_gas_cost_usd` could
//! never be anything but SQL NULL — the A.6 gas-coverage breaker denominated its
//! numerator on a column that had no working producer (GAS-COVERAGE-01
//! defect (a), reported by the Data surface).
//!
//! This module is the **adapter**, not a new provider: it consumes the price
//! that the stack already produces and publishes.
//!
//! ## Producer contract (verified against the tree, not assumed)
//!
//! | Element | Contract | Evidence |
//! |---|---|---|
//! | Redis key | `arbx:token_prices:<chain_id>` (HASH) | `shared-rs/src/price_oracle.rs::redis_token_prices_key` — the key string is **derived from that helper**, never re-typed here |
//! | Field | token symbol, UPPERCASE, as declared by the chain's token universe | `searcher-rs/src/workers/price_worker.rs` (`meta.symbol.to_ascii_uppercase()`, then `TokenRef{symbol: upper}`) |
//! | Value | USD per **whole** token, decimal string | same worker (Alchemy Token Prices / CoinGecko return per-token USD) |
//! | Writer cadence | one write per `PRICE_WORKER_INTERVAL_SECS` (default 30s); hash TTL ≈ 2× the period | `price_worker.rs` (`DEFAULT_PERIOD_SECS`, `CACHE_TTL_MULTIPLIER`) |
//! | Other writers | `token-enricher` DexScreener + GeckoTerminal tiers, **last-writer-wins, no arbitration** on the same key | `shared-rs/src/price_oracle.rs` doc of `PRICE_MAX_TICK_RATIO` |
//! | Sanitisation | non-finite / non-positive values are dropped before the snapshot is built | `RedisCachedPriceOracle::from_snapshot` |
//!
//! ## The identity problem this adapter solves (and does not paper over)
//!
//! The hash is keyed **by symbol**, and symbols are metadata, not identity
//! (`shared-rs/src/token_identity.rs`: "Symbols are METADATA ONLY … a symbol
//! string that happens to match the operator's allowlist is NOT a token").
//! Every allowlist fixture in the tree lists the wrapped-native symbol —
//! `["WETH","USDC",…]` (`shared-rs/src/trading_config.rs`) — and mainnet has no
//! ERC-20 whose `symbol()` is `ETH`, because native ETH is not a token. A reader
//! that hardcodes the field `"ETH"` would therefore miss the live price and
//! report the gas cost as uncomputable forever, for a reason nobody can see.
//!
//! Resolution: a **declared, per-chain candidate list of same-asset aliases**,
//! with the winning field recorded in the returned [`GasPriceQuote`]. Two
//! invariants make the fallthrough safe:
//!
//! 1. **One asset per chain.** Every entry in a chain's list denotes the gas
//!    asset of that chain (the native coin and its wrapped ERC-20 are the same
//!    asset by construction of the wrapper; Polygon's `MATIC→POL` rename is the
//!    same token — `shared-rs/src/chains.rs` documents the former WMATIC
//!    contract self-identifying as `WPOL`). So a corrupt first field may not
//!    block a valid second: this is **not** a silent fallback to a *different*
//!    asset, the list is assets-closed.
//! 2. **No cross-chain borrowing.** The candidate list is selected by
//!    `chain_id` BEFORE any field is read, and an unmapped chain refuses with
//!    [`GasPriceMiss::UnsupportedChain`] even when a plausible `WETH` field is
//!    present. A mainnet ether price must never price Polygon/BSC/Avalanche gas.
//!
//! ## Failure = named absence (R8), never a fabricated price
//!
//! Every miss is a total variant of [`GasPriceMiss`] with a stable
//! `reason()` string for telemetry. There is **no default price, no nominal
//! placeholder, and no fallback provider** here: absence stays absent and the
//! caller writes SQL NULL, which the coverage ledger (migration 126) classifies
//! from evidence (`actual_gas_cost_usd IS NULL` ⇒ not `measured`).

use redis::aio::ConnectionManager;
use shared_rs::price_oracle::RedisCachedPriceOracle;
use std::collections::HashMap;

/// Versioned identity of this adapter's contract. Bump on ANY change to the
/// candidate tables, the key derivation or the miss vocabulary: consumers
/// (drift-tracker telemetry, coverage audit) cite this string.
pub const ADAPTER_VERSION: &str = "gas-price-adapter/1";

/// Gas-token symbol candidates for `chain_id`, in precedence order, or `None`
/// when the chain has no declared gas token.
///
/// The lists mirror the wrapped-native contracts declared in
/// `shared-rs/src/chains.rs` (`WETH_MAINNET`, `WETH_ARBITRUM`, `WETH_OPTIMISM`,
/// `WETH_BASE`, `WPOL_POLYGON`, `WAVAX_AVALANCHE`, `WBNB_BNB`, and the Sepolia
/// set). The native symbol is kept as a lower-priority alias because a chain's
/// universe may publish the native name instead of the wrapped one.
pub fn gas_token_symbol_candidates(chain_id: u64) -> Option<&'static [&'static str]> {
    match chain_id {
        // Ethereum mainnet + Sepolia. The wrapped contract is the priced
        // ERC-20; "ETH" is the alias a native-named universe row would use.
        1 | 11155111 => Some(&["WETH", "ETH"]),
        // Arbitrum One / Arb Sepolia.
        42161 | 421614 => Some(&["WETH", "ETH"]),
        // OP Mainnet / Base / OP Sepolia (same OP-Stack WETH predeploy).
        10 | 8453 | 11155420 => Some(&["WETH", "ETH"]),
        // Polygon. Gas is POL; the wrapped contract reports WPOL after the
        // MATIC→POL rename, and pre-rename universe rows still carry WMATIC.
        // All three denote the same asset — no ether alias belongs here.
        137 => Some(&["WPOL", "WMATIC", "POL"]),
        43114 => Some(&["WAVAX", "AVAX"]),
        56 => Some(&["WBNB", "BNB"]),
        // Unmapped chain: refuse. Guessing the gas asset would be exactly the
        // fabricated-input failure R8 forbids.
        _ => None,
    }
}

/// Chain id for the `arbx:token_prices:<chain>` key, from the DB's `i32`
/// `paper_trade_runs.chain_id`. A non-positive value maps to `0`, which is
/// unmapped ⇒ [`GasPriceMiss::UnsupportedChain`] (absence, never chain 1).
pub fn chain_key_id(chain_id: i32) -> u64 {
    if chain_id > 0 {
        chain_id as u64
    } else {
        0
    }
}

/// A resolved gas-token USD price with its provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct GasPriceQuote {
    /// Chain the price is valid for (the list was selected by this).
    pub chain_id: u64,
    /// USD per whole native token.
    pub usd_per_unit: f64,
    /// The hash field that answered — provenance, not decoration: it is the
    /// only way to tell a `WETH`-sourced price from an `ETH`-sourced one in
    /// telemetry after the fact.
    pub source_field: String,
    /// Adapter contract that produced this quote.
    pub adapter_version: &'static str,
}

/// Why no USD price could be produced. Total: every path returns a variant, so
/// "no price" is always reportable with its exact cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GasPriceMiss {
    /// The chain has no declared gas token (or a non-positive chain id).
    UnsupportedChain { chain_id: u64 },
    /// `REDIS_URL` was not configured / the recon process has no Redis handle.
    NoConnection,
    /// Redis reachable but the chain's price hash yielded zero usable fields
    /// (absent hash, or every field non-finite/non-positive).
    EmptySnapshot { chain_id: u64 },
    /// The hash has other tokens but none of the chain's gas aliases.
    FieldAbsent {
        chain_id: u64,
        tried: Vec<&'static str>,
    },
    /// At least one candidate field was present but not a usable price.
    InvalidValue { field: String, value: String },
}

impl GasPriceMiss {
    /// Stable telemetry reason (snake_case, greppable).
    pub fn reason(&self) -> &'static str {
        match self {
            GasPriceMiss::UnsupportedChain { .. } => "gas_price_unsupported_chain",
            GasPriceMiss::NoConnection => "gas_price_no_redis",
            GasPriceMiss::EmptySnapshot { .. } => "gas_price_empty_snapshot",
            GasPriceMiss::FieldAbsent { .. } => "gas_price_field_absent",
            GasPriceMiss::InvalidValue { .. } => "gas_price_invalid_value",
        }
    }

    /// Human-readable detail for the log line (bounded size, no secrets).
    pub fn detail(&self) -> String {
        match self {
            GasPriceMiss::UnsupportedChain { chain_id } => {
                format!("chain_id={chain_id} has no declared gas token")
            }
            GasPriceMiss::NoConnection => "no Redis connection handle".to_string(),
            GasPriceMiss::EmptySnapshot { chain_id } => {
                format!("arbx:token_prices:{chain_id} yielded no usable field")
            }
            GasPriceMiss::FieldAbsent { chain_id, tried } => {
                format!(
                    "none of [{}] present in arbx:token_prices:{chain_id}",
                    tried.join(",")
                )
            }
            GasPriceMiss::InvalidValue { field, value } => {
                format!("field {field} present but not a usable price (value={value})")
            }
        }
    }
}

/// Pure resolution over ONE already-fetched snapshot. `snapshot` keys are
/// uppercase symbols; values are USD per whole token (the caller is expected to
/// have gone through `RedisCachedPriceOracle`, which drops junk — this function
/// re-validates anyway so an injected/test map cannot smuggle a bad price in).
pub fn resolve_from_snapshot(
    chain_id: u64,
    snapshot: &HashMap<String, f64>,
) -> Result<GasPriceQuote, GasPriceMiss> {
    let Some(candidates) = gas_token_symbol_candidates(chain_id) else {
        return Err(GasPriceMiss::UnsupportedChain { chain_id });
    };
    let mut invalid: Option<(String, f64)> = None;
    for field in candidates {
        let Some(raw) = snapshot.get(*field) else {
            continue;
        };
        if raw.is_finite() && *raw > 0.0 {
            return Ok(GasPriceQuote {
                chain_id,
                usd_per_unit: *raw,
                source_field: (*field).to_string(),
                adapter_version: ADAPTER_VERSION,
            });
        }
        // Same-asset alias with a corrupt value: remember it, keep looking
        // (invariant 1 — a bad WETH field must not hide a valid ETH one).
        if invalid.is_none() {
            invalid = Some(((*field).to_string(), *raw));
        }
    }
    match invalid {
        Some((field, value)) => Err(GasPriceMiss::InvalidValue {
            field,
            value: format!("{value}"),
        }),
        None => Err(GasPriceMiss::FieldAbsent {
            chain_id,
            tried: candidates.to_vec(),
        }),
    }
}

/// Async resolution through the canonical price tower. Read-only, one
/// `HGETALL` per call; no cache is kept here (the hash's own TTL is the cache —
/// a second cache would have to re-prove the same staleness budget).
pub async fn resolve(
    redis: &mut Option<ConnectionManager>,
    chain_id: u64,
) -> Result<GasPriceQuote, GasPriceMiss> {
    if gas_token_symbol_candidates(chain_id).is_none() {
        return Err(GasPriceMiss::UnsupportedChain { chain_id });
    }
    let Some(cm) = redis.as_mut() else {
        return Err(GasPriceMiss::NoConnection);
    };
    let snapshot = RedisCachedPriceOracle::snapshot_from_redis(cm, chain_id)
        .await
        .into_snapshot();
    if snapshot.is_empty() {
        return Err(GasPriceMiss::EmptySnapshot { chain_id });
    }
    resolve_from_snapshot(chain_id, &snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    // ── The live shape of the hash: the universe field is the WRAPPED symbol ──

    #[test]
    fn weth_field_resolves_and_is_recorded_as_provenance() {
        // The operator allowlist in this tree lists "WETH" (trading_config.rs
        // fixtures), so this is the field mainnet actually publishes.
        let q = resolve_from_snapshot(1, &snap(&[("WETH", 2619.59), ("USDC", 0.9998)])).unwrap();
        assert_eq!(q.source_field, "WETH");
        assert_eq!(q.usd_per_unit, 2619.59);
        assert_eq!(q.chain_id, 1);
        assert_eq!(q.adapter_version, ADAPTER_VERSION);
    }

    #[test]
    fn native_alias_is_used_when_the_universe_publishes_it() {
        let q = resolve_from_snapshot(1, &snap(&[("ETH", 2500.0)])).unwrap();
        assert_eq!(q.source_field, "ETH");
        assert_eq!(q.usd_per_unit, 2500.0);
    }

    #[test]
    fn precedence_is_deterministic_not_last_writer() {
        let both = snap(&[("WETH", 2600.0), ("ETH", 2400.0)]);
        let a = resolve_from_snapshot(1, &both).unwrap();
        let b = resolve_from_snapshot(1, &both).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.source_field, "WETH");
        assert_eq!(a.usd_per_unit, 2600.0);
    }

    // ── Adversarial: cross-chain / cross-asset contamination ─────────────────

    #[test]
    fn polygon_gas_is_not_priced_by_a_mainnet_ether_field() {
        // A WETH field present on a Polygon snapshot must NOT price POL gas.
        let q = resolve_from_snapshot(137, &snap(&[("WETH", 2619.59), ("WPOL", 0.42)])).unwrap();
        assert_eq!(q.source_field, "WPOL");
        assert_eq!(q.usd_per_unit, 0.42);
        // …and with ONLY the ether field present, Polygon is a NAMED miss.
        let miss = resolve_from_snapshot(137, &snap(&[("WETH", 2619.59)])).unwrap_err();
        assert_eq!(miss.reason(), "gas_price_field_absent");
        assert!(matches!(
            miss,
            GasPriceMiss::FieldAbsent { chain_id: 137, .. }
        ));
    }

    #[test]
    fn pre_rename_polygon_universe_rows_still_resolve() {
        let q = resolve_from_snapshot(137, &snap(&[("WMATIC", 0.41)])).unwrap();
        assert_eq!(q.source_field, "WMATIC");
        assert_eq!(q.usd_per_unit, 0.41);
    }

    #[test]
    fn unmapped_chain_refuses_even_with_a_plausible_field() {
        // Unknown chain + a perfectly plausible WETH price ⇒ STILL refused.
        let miss = resolve_from_snapshot(999, &snap(&[("WETH", 2619.59)])).unwrap_err();
        assert_eq!(miss.reason(), "gas_price_unsupported_chain");
        // Chain 0 (the non-positive DB chain_id mapping) is unmapped too.
        assert!(gas_token_symbol_candidates(0).is_none());
        assert_eq!(chain_key_id(-1), 0);
        assert_eq!(chain_key_id(1), 1);
    }

    #[test]
    fn avalanche_and_bnb_use_their_own_gas_assets() {
        assert_eq!(
            resolve_from_snapshot(43114, &snap(&[("WAVAX", 30.0)]))
                .unwrap()
                .source_field,
            "WAVAX"
        );
        assert_eq!(
            resolve_from_snapshot(56, &snap(&[("WBNB", 600.0)]))
                .unwrap()
                .source_field,
            "WBNB"
        );
    }

    // ── Adversarial: absent / corrupt / duplicated fields ────────────────────

    #[test]
    fn absent_field_is_a_named_miss_never_a_default_price() {
        let miss =
            resolve_from_snapshot(1, &snap(&[("USDC", 0.9998), ("PEPE", 0.00001)])).unwrap_err();
        assert_eq!(miss.reason(), "gas_price_field_absent");
        assert!(matches!(
            miss,
            GasPriceMiss::FieldAbsent { chain_id: 1, ref tried } if tried == &vec!["WETH", "ETH"]
        ));
    }

    #[test]
    fn empty_snapshot_is_a_named_miss() {
        let miss = resolve_from_snapshot(1, &HashMap::new()).unwrap_err();
        assert_eq!(miss.reason(), "gas_price_field_absent");
    }

    #[test]
    fn corrupt_first_alias_does_not_hide_a_valid_second() {
        // WETH=0 (a scaling/poisoning bug) must not make the row permanently
        // unpriced when the same asset is present under its native alias.
        let q = resolve_from_snapshot(1, &snap(&[("WETH", 0.0), ("ETH", 2619.59)])).unwrap();
        assert_eq!(q.source_field, "ETH");
        assert_eq!(q.usd_per_unit, 2619.59);
    }

    #[test]
    fn all_candidates_corrupt_is_an_invalid_value_miss() {
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            let miss = resolve_from_snapshot(1, &snap(&[("WETH", bad)])).unwrap_err();
            assert_eq!(miss.reason(), "gas_price_invalid_value", "bad={bad}");
            assert!(
                matches!(miss, GasPriceMiss::InvalidValue { ref field, .. } if field == "WETH")
            );
        }
    }

    #[test]
    fn every_miss_reason_is_stable_and_namespaced() {
        let all = [
            GasPriceMiss::UnsupportedChain { chain_id: 7 },
            GasPriceMiss::NoConnection,
            GasPriceMiss::EmptySnapshot { chain_id: 1 },
            GasPriceMiss::FieldAbsent {
                chain_id: 1,
                tried: vec!["WETH", "ETH"],
            },
            GasPriceMiss::InvalidValue {
                field: "WETH".into(),
                value: "0".into(),
            },
        ];
        let reasons: Vec<&str> = all.iter().map(|m| m.reason()).collect();
        assert_eq!(
            reasons,
            vec![
                "gas_price_unsupported_chain",
                "gas_price_no_redis",
                "gas_price_empty_snapshot",
                "gas_price_field_absent",
                "gas_price_invalid_value",
            ]
        );
        for m in &all {
            assert!(m.reason().starts_with("gas_price_"));
            assert!(!m.detail().is_empty());
        }
    }

    #[test]
    fn candidate_tables_cover_the_chains_the_repo_names() {
        // The set mirrors shared-rs/src/chains.rs wrapped-native constants (and
        // the Sepolia family). A chain added there without a gas token here
        // fails closed into UnsupportedChain — visible, never silent.
        for c in [1u64, 11155111, 42161, 421614, 10, 8453, 11155420] {
            assert_eq!(
                gas_token_symbol_candidates(c).unwrap()[0],
                "WETH",
                "chain {c}"
            );
        }
        assert_eq!(
            gas_token_symbol_candidates(137).unwrap(),
            &["WPOL", "WMATIC", "POL"]
        );
        assert_eq!(gas_token_symbol_candidates(43114).unwrap()[0], "WAVAX");
        assert_eq!(gas_token_symbol_candidates(56).unwrap()[0], "WBNB");
    }
}
