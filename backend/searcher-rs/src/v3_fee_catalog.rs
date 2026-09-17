//! V3 fee catalog — FEE-TIER-AWARE-QUOTING (WO-06).
//!
//! QuoterV2.`quoteExactInputSingle` does NOT receive a pool address: it
//! derives the pool via `factory.getPool(tokenIn, tokenOut, fee)`, so the fee
//! tier (raw pips, uint24) is part of the call identity. Quoting with a tier
//! that has no real pool makes the quoter revert — which surfaced downstream
//! as `v3_quote_unavailable` (a transport-looking label hiding a tier gap).
//! The prior projector default (`fee_bps.unwrap_or(500)`) was exactly that:
//! a blind 0.05% guess.
//!
//! This catalog is the authoritative fee source for every V3 quote:
//!   * `load_from_redis` mirrors `arbx:pool_index_v3:<chain>:<symA>:<symB>`
//!     (the same wire contract the scanner already consumes; populated by
//!     pool_sync_worker from PG — one source of truth, no new PG reader).
//!   * `record_observed` passively reconciles after every successful RPC
//!     quote (covers Redis index lag / desync).
//!   * `resolve` decides Catalog / Mismatch / NotCatalogued; the projector
//!     turns NotCatalogued into a zero-RPC honest rejection instead of a
//!     blind quote (R8).

use std::collections::{BTreeSet, HashMap};
use std::sync::RwLock;

use ethers::types::Address;
use tracing::warn;

use crate::reserves::V3PoolInfo;

/// Outcome of resolving a quote's fee tier against the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeResolution {
    /// Pool catalogued at this fee (raw pips) — quote with it.
    Catalog(u32),
    /// Caller offered a fee that differs from the catalog: the CATALOG wins
    /// (counted in `arbx_v3_fee_resolution_total{resolution="mismatch"}`).
    Mismatch { offered: u32, catalog: u32 },
    /// Pool address unknown to the catalog — must NOT be quoted blind.
    NotCatalogued,
}

/// In-memory mirror of the Redis V3 pool index. Thread-safe (`RwLock`);
/// loaded at boot + refreshed on a timer by the scanner, and passively
/// reconciled by `record_observed` after successful RPC quotes.
#[derive(Default)]
pub struct V3FeeCatalog {
    /// pool address → fee tier (raw pips: 100, 500, 3000, 10000).
    by_pool: RwLock<HashMap<Address, u32>>,
    /// Unordered token pair → known fee tiers. The Redis index is
    /// symbol-keyed, so the address-keyed pair side is seeded only by
    /// `record_observed` (post-RPC observations) — a pair with no observed
    /// tiers AND an uncatalogued pool is honestly rejected without RPC.
    by_pair: RwLock<HashMap<(Address, Address), BTreeSet<u32>>>,
}

/// Increment `arbx_v3_fee_resolution_total{resolution}` (R8). `pub(crate)`
/// so the projector can emit at the decision point (kept next to the enum so
/// the label strings never drift from the metric contract).
pub(crate) fn fee_resolution_metric(resolution: &str) {
    crate::metrics::V3_FEE_RESOLUTION_TOTAL
        .with_label_values(&[resolution])
        .inc();
}

impl V3FeeCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Canonical unordered pair key.
    fn pair_key(token0: Address, token1: Address) -> (Address, Address) {
        if token0 <= token1 {
            (token0, token1)
        } else {
            (token1, token0)
        }
    }

    fn set_pool_gauge(&self) {
        let len = self.by_pool.read().unwrap_or_else(|e| e.into_inner()).len();
        crate::metrics::V3_FEE_CATALOG_POOLS.set(len as i64);
    }

    /// Record a pool + its pair tier. Called after every successful RPC quote
    /// (passive reconciliation — the quoter answering at this tier proves the
    /// tier is real even if the Redis index lags) and directly by tests.
    pub fn record_observed(&self, pool: Address, token0: Address, token1: Address, fee_pips: u32) {
        {
            let mut by_pool = self.by_pool.write().unwrap_or_else(|e| e.into_inner());
            by_pool.insert(pool, fee_pips);
        }
        {
            let mut by_pair = self.by_pair.write().unwrap_or_else(|e| e.into_inner());
            by_pair
                .entry(Self::pair_key(token0, token1))
                .or_default()
                .insert(fee_pips);
        }
        self.set_pool_gauge();
    }

    /// Catalog fee for a pool address (raw pips).
    pub fn fee_for_pool(&self, pool: Address) -> Option<u32> {
        self.by_pool
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&pool)
            .copied()
    }

    /// Known fee tiers for an unordered token pair.
    pub fn tiers_for_pair(&self, token0: Address, token1: Address) -> BTreeSet<u32> {
        self.by_pair
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&Self::pair_key(token0, token1))
            .cloned()
            .unwrap_or_default()
    }

    /// Resolve the fee to quote `pool` with, given the caller's offered fee
    /// (`PoolRef::fee_bps`, also raw pips for V3 legs). The catalog always
    /// wins; a pool missing from the catalog is `NotCatalogued` (never a
    /// blind default — that is what this module exists to fix).
    pub fn resolve(&self, pool: Address, offered: Option<u32>) -> FeeResolution {
        match self.fee_for_pool(pool) {
            Some(catalog) => match offered {
                Some(offered) if offered != catalog => FeeResolution::Mismatch { offered, catalog },
                _ => FeeResolution::Catalog(catalog),
            },
            None => FeeResolution::NotCatalogued,
        }
    }

    /// Number of pools currently catalogued.
    pub fn pool_count(&self) -> usize {
        self.by_pool.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Merge the Redis `arbx:pool_index_v3:<chain>:*` index into `by_pool`
    /// (SCAN + per-key GET — same access pattern as price_worker's pool-index
    /// scan). Entries never disappear on refresh: a pool dropped by Redis
    /// while still observable would otherwise flap between catalogued and
    /// rejected. Returns the post-merge pool count. A Redis error returns
    /// `Err` — callers keep the prior snapshot (R8, non-fatal by design).
    pub async fn load_from_redis(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        chain_id: u64,
    ) -> anyhow::Result<usize> {
        let pattern = format!("arbx:pool_index_v3:{}:*", chain_id);
        let mut keys = Vec::new();
        {
            let mut iter: redis::AsyncIter<String> = redis::cmd("SCAN")
                .cursor_arg(0)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(500)
                .clone()
                .iter_async(redis)
                .await
                .map_err(|e| anyhow::anyhow!("v3 fee catalog SCAN failed: {e}"))?;
            while let Some(k) = iter.next_item().await {
                keys.push(k);
            }
        }
        // `iter` (and its ConnectionManager borrow) dropped before the GETs.

        let mut malformed = 0usize;
        for key in keys {
            let raw: Option<String> = redis::AsyncCommands::get(redis, &key)
                .await
                .map_err(|e| anyhow::anyhow!("v3 fee catalog GET {key} failed: {e}"))?;
            let Some(json) = raw else { continue };
            let pools: Vec<V3PoolInfo> = match serde_json::from_str(&json) {
                Ok(p) => p,
                Err(_) => {
                    malformed += 1;
                    continue;
                }
            };
            let mut by_pool = self.by_pool.write().unwrap_or_else(|e| e.into_inner());
            for info in pools {
                match info.pool_addr.parse::<Address>() {
                    Ok(addr) => {
                        by_pool.insert(addr, info.fee_bps);
                    }
                    Err(_) => malformed += 1,
                }
            }
        }
        if malformed > 0 {
            warn!(
                event = "v3_fee_catalog.entries_malformed",
                chain_id,
                malformed,
                "skipped malformed pool_index_v3 entries (R8: no fabricated tiers)"
            );
        }
        self.set_pool_gauge();
        Ok(self.pool_count())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::amm_math::{encode_quote_calldata, V3QuoteRequest};
    use ethers::types::{Address, U256};

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    // ── Catalog resolution semantics ─────────────────────────────────────────

    #[test]
    fn resolve_catalog_mismatch_not_catalogued() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(1), addr(0xA), addr(0xB), 3000);
        assert_eq!(c.resolve(addr(1), None), FeeResolution::Catalog(3000));
        assert_eq!(c.resolve(addr(1), Some(3000)), FeeResolution::Catalog(3000));
        assert_eq!(
            c.resolve(addr(1), Some(100)),
            FeeResolution::Mismatch {
                offered: 100,
                catalog: 3000
            }
        );
        assert_eq!(c.resolve(addr(2), Some(500)), FeeResolution::NotCatalogued);
        assert_eq!(c.fee_for_pool(addr(1)), Some(3000));
        assert_eq!(c.fee_for_pool(addr(2)), None);
        assert_eq!(c.pool_count(), 1);
    }

    #[test]
    fn tiers_for_pair_is_order_insensitive() {
        let c = V3FeeCatalog::new();
        // Register the pair in BOTH argument orders — the key is canonical.
        c.record_observed(addr(1), addr(0xB), addr(0xA), 500);
        c.record_observed(addr(2), addr(0xA), addr(0xB), 3000);
        let tiers = c.tiers_for_pair(addr(0xA), addr(0xB));
        assert_eq!(tiers, BTreeSet::from([500, 3000]));
        assert_eq!(c.tiers_for_pair(addr(0xB), addr(0xA)), tiers);
        assert!(
            c.tiers_for_pair(addr(0xA), addr(0xC)).is_empty(),
            "unknown pair must have no tiers (R8)"
        );
    }

    // ── T1: PIN ENCODING — fee word is raw pips, ABI-padded to 32 bytes ──────
    //
    // No-regression anchor for the bps-caused-reverts precedent: the calldata
    // is `selector || tokenIn || tokenOut || amountIn || fee || sqrtLimit`
    // (the single struct argument encodes inline — 5 words, no offset word),
    // so the fee word sits at bytes [100..132].

    #[test]
    fn t1_pin_encoding_fee_words_are_raw_pips() {
        let req = |fee: u32| V3QuoteRequest {
            pool_addr: addr(1),
            token_in: addr(2),
            token_out: addr(3),
            amount_in: U256::from(1_000u64),
            fee_bps: fee,
        };
        let cd3000 = encode_quote_calldata(&req(3000)).unwrap();
        assert_eq!(cd3000.len(), 164, "selector + 5 inline words");
        let mut expect_3000 = [0u8; 32];
        expect_3000[30] = 0x0b;
        expect_3000[31] = 0xb8; // 3000 = 0x0bb8
        assert_eq!(
            &cd3000[100..132],
            &expect_3000,
            "fee=3000 must encode as 0x0bb8"
        );
        let cd5 = encode_quote_calldata(&req(5)).unwrap();
        let mut expect_5 = [0u8; 32];
        expect_5[31] = 0x05; // 5 = 0x05
        assert_eq!(&cd5[100..132], &expect_5, "fee=5 must encode as 0x05");
    }

    // ── T7: ANCHORED VECTOR CAST — WETH/USDC (500 + 3000 tiers) ──────────────
    //
    // Anchor derivation (2026-09-17):
    //   * Selector 0xc6a5026a verified EXTERNALLY against the 4byte /
    //     openchain signature registries for
    //     quoteExactInputSingle((address,address,uint256,uint24,uint160)).
    //   * Words are canonical ABI for the inline struct argument:
    //     WETH  = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2,
    //     USDC  = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48,
    //     amountIn = 1e18 = 0x0de0b6b3a7640000, sqrtPriceLimitX96 = 0.
    //   * 500 and 3000 are the two real WETH/USDC mainnet pool tiers; the
    //     pool address is NOT part of the calldata (the quoter derives it
    //     from the factory — the root cause this module fixes).

    #[test]
    fn t7_anchored_quoterv2_calldata_weth_usdc() {
        let weth: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            .parse()
            .unwrap();
        let usdc: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            .parse()
            .unwrap();
        for (fee, fee_word) in [
            (
                500u32,
                "00000000000000000000000000000000000000000000000000000000000001f4",
            ),
            (
                3000u32,
                "0000000000000000000000000000000000000000000000000000000000000bb8",
            ),
        ] {
            let req = V3QuoteRequest {
                pool_addr: Address::zero(),
                token_in: weth,
                token_out: usdc,
                amount_in: U256::from(1_000_000_000_000_000_000u64),
                fee_bps: fee,
            };
            let cd = encode_quote_calldata(&req).unwrap();
            let expected = format!(
                "c6a5026a\
                 000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                 000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48\
                 0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                 {fee_word}\
                 0000000000000000000000000000000000000000000000000000000000000000"
            );
            assert_eq!(
                hex::encode(cd.as_ref()),
                expected,
                "anchored calldata mismatch for fee={fee}"
            );
        }
    }
}
