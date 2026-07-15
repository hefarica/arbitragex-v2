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
}

impl RouteMetadata {
    /// Create an empty RouteMetadata (for legacy rows / detection failures).
    pub fn empty() -> Self {
        Self {
            pool_addresses: Vec::new(),
            token_addresses: Vec::new(),
            dex_adapters: Vec::new(),
            decimals: DecimalsMap::new(),
        }
    }

    /// Returns true if the route metadata is populated (non-empty topology).
    /// Used to decide whether the A1 enrichment path can serve a given opportunity.
    pub fn is_populated(&self) -> bool {
        !self.pool_addresses.is_empty()
            && !self.token_addresses.is_empty()
            && !self.dex_adapters.is_empty()
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
        };

        let json = serde_json::to_string(&rm).expect("serialize");
        let deserialized: RouteMetadata = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.pool_addresses, rm.pool_addresses);
        assert_eq!(deserialized.token_addresses, rm.token_addresses);
        assert_eq!(deserialized.dex_adapters, rm.dex_adapters);
        assert!(deserialized.is_populated());
    }
}
