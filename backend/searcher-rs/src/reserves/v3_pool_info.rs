//! Canonical pool_index_v3 wire contract shared by every producer/reader.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct V3PoolInfo {
    /// Pool address, lowercase hex with 0x prefix.
    ///
    /// CATALOG-BACKFILL-02 (2026-09-25): WO-06 renamed this field from
    /// `address` to `pool_addr`, but the live Redis index still holds BOTH
    /// wire formats (measured on the VPS: `pool_index_v3:1:zec:weth` =
    /// `[{"address":"0x…","fee_bps":30}]` while `usdc:ustc` uses the new
    /// key). Serde rejects the WHOLE per-pair array on one legacy entry,
    /// so the pair resolves `NotCatalogued` and every V3 leg quotes
    /// `v3_quote_unavailable` (71% of live cards). The alias accepts both
    /// spellings on READ; serialization still emits the canonical
    /// `pool_addr`, so the wire contract does not regress.
    #[serde(alias = "address")]
    pub pool_addr: String,
    /// Legacy field name: V3 fee tier in raw pips (1e-6): 100 (0.01%), 500 (0.05%), 3000 (0.30%), 10000 (1.00%).
    pub fee_bps: u32,
}
