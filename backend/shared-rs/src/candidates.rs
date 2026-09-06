//! `OpportunityCandidate` + `RouteMetadata` — enriched opportunity contracts.
//!
//! `OpportunityCandidate` carries the full topology information needed by
//! sim-core's encoder to construct a `RoundTripContext` for REVM simulation.
//! Unlike the minimal `Opportunity` (which only carries token_in/out and
//! dex_a/b), it includes multi-hop route details.
//!
//! `RouteMetadata` is the persistent JSONB subset stored in
//! `opportunities.route_metadata` (G-SIM-1 PR-B2b Fase 2 A1) so sim-ctl can
//! reconstruct an `OpportunityCandidate` without a second divergent encoder.
//!
//! These are the shared contracts between:
//! - searcher-rs (producer)
//! - api-server (enricher/proxy)
//! - sim-ctl (consumer → sim-core encoder)
//! - frontend (selector UI)

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Complete route topology for simulation.
///
/// Carries the multi-hop metadata that `build_round_trip_context_from_candidate`
/// requires to encode a proper `RoundTripContext` (not just a 2-hop stub).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityCandidate {
    /// Corresponds to `Opportunity.id` — links back to the DB row.
    pub opportunity_id: Uuid,

    /// Chain ID for EVM dispatch.
    pub chain_id: u64,

    /// Full token path including intermediates (e.g. [WETH, USDC, DAI]).
    /// Length = hops + 1. First = token_in, last = token_out.
    pub token_addresses: Vec<String>,

    /// Pool address per hop (e.g. [0xUniswapPool, 0xSushiPool]).
    /// Length = hops. Order aligns with the segment between token_addresses[i] → [i+1].
    pub pool_addresses: Vec<String>,

    /// DEX router labels per hop (uniswap_v2_router, sushiswap, curveswap, etc).
    /// Length = hops. Used by the encoder to select the correct router contract.
    pub dex_adapters: Vec<String>,

    /// Input amount as f64 (USD-normalized or token-normalized).
    /// The encoder converts this to wei using `decimals[token_in]`.
    pub amount_in: f64,

    /// Expected output amount (f64). Used for profitability check before simulation.
    pub expected_amount_out: f64,

    /// Gross profit estimate (USD) before gas/fees/slippage.
    pub gross_profit: f64,

    /// Token decimals map: address → decimals (18 for WETH, 6 for USDC, etc).
    /// Required for f64→wei conversion. Every address in `token_addresses`
    /// must have an entry here.
    pub decimals: DecimalsMap,

    /// Optional: block number for deterministic forking.
    /// If None, sim-ctl uses the latest block.
    pub block_number: Option<u64>,

    /// Route fingerprint for deduplication (SHA-256 of route topology).
    pub route_fingerprint: String,
}

/// Decimals map: token address → decimals (uint8).
///
/// Stored as a map rather than an array to handle arbitrary-length routes
/// and make lookups O(1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecimalsMap {
    /// Inner map: lowercased 0x-prefixed address → decimals (0-255).
    pub map: std::collections::HashMap<String, u8>,
}

impl DecimalsMap {
    /// Create a new empty map.
    pub fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
        }
    }

    /// Insert a token's decimals. Address is lowercased automatically.
    pub fn insert(&mut self, address: String, decimals: u8) {
        self.map.insert(address.to_lowercase(), decimals);
    }

    /// Get decimals for an address. Returns None if not found.
    pub fn get(&self, address: &str) -> Option<u8> {
        self.map.get(&address.to_lowercase()).copied()
    }

    /// Validate that every token in `addresses` has a decimals entry.
    /// Returns Ok(()) if all present, Err(missing_addresses) otherwise.
    pub fn validate_complete(&self, addresses: &[String]) -> Result<(), Vec<String>> {
        let missing: Vec<String> = addresses
            .iter()
            .filter(|addr| !self.map.contains_key(&addr.to_lowercase()))
            .cloned()
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

impl Default for DecimalsMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistent route topology stored as JSONB in `opportunities.route_metadata`.
///
/// This is the **A1 enrichment path** data: the complete multi-hop route
/// topology that searcher-rs captures at detection time and persists alongside
/// the minimal `Opportunity`. sim-ctl (or api-server) reads this column to
/// reconstruct a full `OpportunityCandidate` without a second divergent encoder.
///
/// Design: `RouteMetadata` is the persistent subset of `OpportunityCandidate` —
/// everything except the transient fields (opportunity_id, chain_id, amount_in,
/// expected_amount_out, gross_profit, block_number, route_fingerprint) which
/// already live on the `Opportunity` row itself. This avoids duplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMetadata {
    /// Pool address per hop. Length = hops.
    /// Order aligns with the segment between token_addresses[i] → [i+1].
    pub pool_addresses: Vec<String>,

    /// Full token path including intermediates. Length = hops + 1.
    /// First = token_in, last = token_out.
    pub token_addresses: Vec<String>,

    /// DEX router labels per hop (uniswap_v2_router, sushiswap, etc).
    /// Length = hops.
    pub dex_adapters: Vec<String>,

    /// Token decimals: lowercased address → decimals (uint8).
    /// Every token in `token_addresses` should have an entry.
    pub decimals: DecimalsMap,

    /// Per-leg INPUT amounts as exact wei decimal strings (HOPS-LEDGER-04).
    /// Length = hops, parallel to `dex_adapters`. Present ONLY when the sizing
    /// kernel computed the full chain (`OptimizeOutcome::Sized`) — None on
    /// pre-reprice rows, unprofitable rejects, and all legacy rows (R8: absent
    /// = not computed, never a repeated intent amount dressed as a ledger).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leg_amounts_in: Option<Vec<String>>,

    /// Per-leg OUTPUT amounts as exact wei decimal strings (HOPS-LEDGER-04).
    /// `leg_amounts_out[i]` is what hop i yields in `token_addresses[i+1]`.
    /// Present iff `leg_amounts_in` is present (all-or-nothing — a half ledger
    /// would fabricate the missing links; the kernels guarantee
    /// out[i] == in[i+1] on the sized chain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leg_amounts_out: Option<Vec<String>>,

    /// Swap orientation per leg (HOPS-LEDGER-04): true = token0 → token1 where
    /// token0 is the lower address (the Uniswap V2/V3 ascending-sort
    /// convention — a deployment fact, derivable without pool state). Length =
    /// hops; present iff the amount arrays are present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leg_zero_for_one: Option<Vec<bool>>,
}

impl RouteMetadata {
    /// Create an empty RouteMetadata (for legacy rows / detection failures).
    pub fn empty() -> Self {
        Self {
            pool_addresses: Vec::new(),
            token_addresses: Vec::new(),
            dex_adapters: Vec::new(),
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        }
    }

    /// Returns true if the route metadata is populated (non-empty topology).
    /// Used to decide whether the A1 enrichment path can serve a given opportunity.
    pub fn is_populated(&self) -> bool {
        !self.pool_addresses.is_empty()
            && !self.token_addresses.is_empty()
            && !self.dex_adapters.is_empty()
    }

    /// Attach the sizing kernel's per-leg ledger (HOPS-LEDGER-04).
    ///
    /// `amounts_in`/`amounts_out` are the EXACT wei strings computed at the
    /// final sized amount. All-or-nothing: both arrays must be present and
    /// aligned with `dex_adapters` (len == hops), otherwise nothing is
    /// attached and `false` is returned (a partial ledger would fabricate the
    /// missing links). `leg_zero_for_one` is derived from ascending token
    /// order per leg — the Uniswap V2/V3 token0/token1 convention, a
    /// deployment fact, not pool state.
    pub fn attach_leg_ledger(
        &mut self,
        amounts_in: &[String],
        amounts_out: &[String],
    ) -> bool {
        let hops = self.dex_adapters.len();
        if amounts_in.len() != hops || amounts_out.len() != hops {
            return false;
        }
        // A malformed token path cannot yield honest swap orientations —
        // refuse rather than fabricate `false` directions (R8).
        if self.token_addresses.len() != hops + 1 {
            return false;
        }
        let zero_for_one = (0..hops)
            .map(|i| {
                // token_in < token_out (ascending) ⇒ input IS token0 ⇒ 0→1.
                self.token_addresses
                    .get(i)
                    .map(|t| t.as_str())
                    .zip(self.token_addresses.get(i + 1).map(|t| t.as_str()))
                    // Defensive lowercase compare: a checksummed (mixed-case)
                    // address would otherwise sort above lowercase hex.
                    .map(|(tin, tout)| tin.to_ascii_lowercase() < tout.to_ascii_lowercase())
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        self.leg_amounts_in = Some(amounts_in.to_vec());
        self.leg_amounts_out = Some(amounts_out.to_vec());
        self.leg_zero_for_one = Some(zero_for_one);
        true
    }

    /// Validate route topology consistency:
    /// - token_addresses length == dex_adapters length + 1
    /// - pool_addresses length == dex_adapters length
    /// - every token_address has a decimals entry
    ///
    /// Returns Ok(()) if consistent, Err(reason) otherwise.
    pub fn validate(&self) -> Result<(), String> {
        let hops = self.dex_adapters.len();
        if self.token_addresses.len() != hops + 1 {
            return Err(format!(
                "token_addresses length {} != hops+1 {} (dex_adapters len={})",
                self.token_addresses.len(),
                hops + 1,
                hops
            ));
        }
        if self.pool_addresses.len() != hops {
            return Err(format!(
                "pool_addresses length {} != hops {} (dex_adapters len={})",
                self.pool_addresses.len(),
                hops,
                hops
            ));
        }
        if let Err(missing) = self.decimals.validate_complete(&self.token_addresses) {
            return Err(format!("missing decimals for tokens: {:?}", missing));
        }
        Ok(())
    }
}

impl Default for RouteMetadata {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimals_map_basic() {
        let mut map = DecimalsMap::new();
        map.insert("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".into(), 18);
        map.insert("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into(), 6);

        // Case-insensitive lookup
        assert_eq!(
            map.get("0xc02aaA39b223fE8D0A0e5C4F27eAD9083C756Cc2"),
            Some(18)
        );
        assert_eq!(
            map.get("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            Some(6)
        );
        assert_eq!(map.get("0xunknown"), None);
    }

    #[test]
    fn test_validate_complete_pass() {
        let mut map = DecimalsMap::new();
        map.insert("0xtoken1".into(), 18);
        map.insert("0xtoken2".into(), 6);

        let addresses = vec!["0xtoken1".into(), "0xtoken2".into()];
        assert!(map.validate_complete(&addresses).is_ok());
    }

    #[test]
    fn test_validate_complete_fail() {
        let mut map = DecimalsMap::new();
        map.insert("0xtoken1".into(), 18);
        // Missing token2

        let addresses = vec!["0xtoken1".into(), "0xtoken2".into()];
        let missing = map.validate_complete(&addresses).unwrap_err();
        assert_eq!(missing, vec!["0xtoken2"]);
    }

    #[test]
    fn test_route_metadata_empty() {
        let rm = RouteMetadata::empty();
        assert!(!rm.is_populated());
        assert!(rm.pool_addresses.is_empty());
        assert!(rm.token_addresses.is_empty());
        assert!(rm.dex_adapters.is_empty());
    }

    #[test]
    fn test_route_metadata_is_populated() {
        let rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into()],
            token_addresses: vec!["0xtokenIn".into(), "0xtokenOut".into()],
            dex_adapters: vec!["uniswap_v2_router".into()],
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        assert!(rm.is_populated());
    }

    #[test]
    fn test_route_metadata_validate_ok() {
        let mut decimals = DecimalsMap::new();
        decimals.insert("0xtokenIn".into(), 18);
        decimals.insert("0xtokenMid".into(), 6);
        decimals.insert("0xtokenOut".into(), 18);

        let rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into(), "0xpool2".into()],
            token_addresses: vec!["0xtokenIn".into(), "0xtokenMid".into(), "0xtokenOut".into()],
            dex_adapters: vec!["uniswap_v2_router".into(), "sushiswap".into()],
            decimals,
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        assert!(
            rm.validate().is_ok(),
            "2-hop route with complete decimals should validate"
        );
    }

    #[test]
    fn test_route_metadata_validate_length_mismatch() {
        let rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into()],
            token_addresses: vec!["0xtokenIn".into(), "0xtokenOut".into()],
            dex_adapters: vec!["router1".into(), "router2".into()],
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        let err = rm.validate().unwrap_err();
        assert!(
            err.contains("length"),
            "expected length mismatch in: {}",
            err
        );
    }

    #[test]
    fn test_route_metadata_validate_missing_decimals() {
        let rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into()],
            token_addresses: vec!["0xtokenIn".into(), "0xtokenOut".into()],
            dex_adapters: vec!["uniswap_v2_router".into()],
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        let err = rm.validate().unwrap_err();
        assert!(
            err.contains("missing decimals"),
            "expected missing decimals in: {}",
            err
        );
    }

    #[test]
    fn test_route_metadata_serde_roundtrip() {
        let mut decimals = DecimalsMap::new();
        decimals.insert("0xtokenIn".into(), 18);
        decimals.insert("0xtokenOut".into(), 6);

        let rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into()],
            token_addresses: vec!["0xtokenIn".into(), "0xtokenOut".into()],
            dex_adapters: vec!["uniswap_v2_router".into()],
            decimals,
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };

        let json = serde_json::to_string(&rm).expect("serialize");
        let deserialized: RouteMetadata = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.pool_addresses, rm.pool_addresses);
        assert_eq!(deserialized.token_addresses, rm.token_addresses);
        assert_eq!(deserialized.dex_adapters, rm.dex_adapters);
        assert!(deserialized.is_populated());
    }

    // HOPS-LEDGER-04 — wire-compat contract for the optional per-leg arrays.
    #[test]
    fn test_route_metadata_none_serializes_without_ledger_keys() {
        // None rows must be BYTE-compatible with pre-HOPS-LEDGER-04 rows so
        // old consumers see identical JSON (skip_serializing_if).
        let rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into()],
            token_addresses: vec!["0xtokenIn".into(), "0xtokenOut".into()],
            dex_adapters: vec!["uniswap_v2_router".into()],
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        let json = serde_json::to_string(&rm).expect("serialize");
        assert!(!json.contains("leg_amounts_in"));
        assert!(!json.contains("leg_amounts_out"));
        assert!(!json.contains("leg_zero_for_one"));
    }

    #[test]
    fn test_route_metadata_old_json_without_ledger_deserializes() {
        // Rows persisted BEFORE this change carry no ledger keys — the struct
        // must deserialize them untouched (serde default).
        let old_json = r#"{
            "pool_addresses": ["0xpool1"],
            "token_addresses": ["0xtokenIn", "0xtokenOut"],
            "dex_adapters": ["uniswap_v2_router"],
            "decimals": {"map": {}}
        }"#;
        let rm: RouteMetadata = serde_json::from_str(old_json).expect("old json deserializes");
        assert!(rm.leg_amounts_in.is_none());
        assert!(rm.leg_amounts_out.is_none());
        assert!(rm.leg_zero_for_one.is_none());
        assert!(rm.is_populated());
    }

    #[test]
    fn test_attach_leg_ledger_roundtrip_and_zero_for_one() {
        let mut rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into(), "0xpool2".into()],
            // Ascending order matters: 0xB < 0xC ⇒ leg0 0→1 true; leg1 C→B ⇒ false.
            token_addresses: vec!["0xB".into(), "0xC".into(), "0xB".into()],
            dex_adapters: vec!["uni".into(), "sushi".into()],
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        assert!(rm.attach_leg_ledger(
            &["1000".to_string(), "995".to_string()],
            &["995".to_string(), "1010".to_string()]
        ));
        assert_eq!(rm.leg_amounts_in.as_deref().unwrap(), &["1000", "995"]);
        assert_eq!(rm.leg_amounts_out.as_deref().unwrap(), &["995", "1010"]);
        assert_eq!(rm.leg_zero_for_one.as_deref().unwrap(), &[true, false]);

        // Round-trip preserves the arrays as exact strings.
        let json = serde_json::to_string(&rm).expect("serialize");
        let back: RouteMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.leg_amounts_in, rm.leg_amounts_in);
        assert_eq!(back.leg_amounts_out, rm.leg_amounts_out);
        assert_eq!(back.leg_zero_for_one, rm.leg_zero_for_one);
    }

    #[test]
    fn test_attach_leg_ledger_all_or_nothing() {
        let mut rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into(), "0xpool2".into()],
            token_addresses: vec!["0xA".into(), "0xB".into(), "0xA".into()],
            dex_adapters: vec!["uni".into(), "sushi".into()],
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        // Length mismatch (1 vs 2 hops) ⇒ nothing attached, no partial ledger.
        assert!(!rm.attach_leg_ledger(&["1000".to_string()], &["995".to_string()]));
        assert!(rm.leg_amounts_in.is_none());
        assert!(rm.leg_amounts_out.is_none());
        assert!(rm.leg_zero_for_one.is_none());
    }

    // HOPS-LEDGER-04 review minor-1: a malformed token path (tokens ≠ hops+1)
    // must REFUSE the attach — deriving orientations from a broken path would
    // fabricate `false` directions on the legs lacking a pair (R8).
    #[test]
    fn test_attach_leg_ledger_refuses_malformed_token_path() {
        let mut rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into(), "0xpool2".into()],
            // 3 tokens for 2 hops is coherent, but use 2 (malformed): leg 1
            // has no out-token pair.
            token_addresses: vec!["0xA".into(), "0xB".into()],
            dex_adapters: vec!["uni".into(), "sushi".into()],
            decimals: DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        // Amounts ARE correctly aligned (2 vs 2 hops) — the token path is not.
        assert!(!rm.attach_leg_ledger(
            &["1000".to_string(), "995".to_string()],
            &["995".to_string(), "1010".to_string()]
        ));
        assert!(rm.leg_amounts_in.is_none());
        assert!(rm.leg_amounts_out.is_none());
        assert!(rm.leg_zero_for_one.is_none());
    }
}
