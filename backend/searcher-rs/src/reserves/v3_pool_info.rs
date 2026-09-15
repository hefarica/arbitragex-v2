//! Canonical pool_index_v3 wire contract shared by every producer/reader.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct V3PoolInfo {
    /// Pool address, lowercase hex with 0x prefix.
    pub pool_addr: String,
    /// Legacy field name: V3 fee tier in raw pips (1e-6): 100 (0.01%), 500 (0.05%), 3000 (0.30%), 10000 (1.00%).
    pub fee_bps: u32,
}
