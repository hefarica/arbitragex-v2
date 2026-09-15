#![allow(dead_code)] // The complete searcher is independently compiled in Rust CI.
#[path = "../../backend/searcher-rs/src/reserves/v3_pool_info.rs"]
mod v3_pool_info;
mod reserves {
    pub(crate) use crate::v3_pool_info::V3PoolInfo;
}
#[path = "../../backend/searcher-rs/src/pool_discovery/v3_fee.rs"]
mod v3_fee;

#[cfg(test)]
mod redis_retry;
