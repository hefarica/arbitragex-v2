//! Live pool-candidate sources for the enumeration worker.
//!
//! Each `fetch(chain_id, top_n, min_tvl_usd)` returns TVL-ranked
//! [`crate::pool_candidate::PoolCandidate`]s. None of them persist or trust the
//! data blindly — they only produce candidates; the worker re-verifies every
//! pool on-chain (factory + token0/1) before persisting. All read-only HTTP.

pub mod alchemy;
pub mod defillama;
pub mod dexscreener;
pub mod geckoterminal;
pub mod thegraph;
