//! V3 quote provider — wires the on-chain Uniswap V3 QuoterV2 into `StateProjector`
//! so V3 pools can be priced (eliminates the `no_price_oracle` rejection for V3).
//!
//! NO-ACTIVE: this is a **read-only** path — `quoteExactInputSingle` is a
//! `staticcall` batched through Multicall3 (`eth_call`), plus pure result
//! decoding. No signer, no private key, no broadcast, no capital. It only feeds
//! *detection* (a real `amount_out` so the Risk Gate can value a V3 candidate);
//! the capital barrier downstream is untouched.
//!
//! Reuses: `amm_math::v3_quote_exact_in_multicall` (the same kernel
//! `triangular_worker` uses), `HttpRpcPool::with_retry` (circuit-breaker +
//! failover + EWMA), and the `shared_rs::chains` quoter/multicall catalog.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use ethers::types::{Address, U256};

use crate::amm_math::{v3_quote_exact_in_multicall, V3QuoteRequest};
use crate::state_projector::V3QuoteProvider;
use shared_rs::chains::{multicall3_for_chain, quoter_v2_for_chain};
use shared_rs::rpc_failover::HttpRpcPool;

/// Quotes V3 pools via the on-chain QuoterV2 (read-only) through the
/// failover-backed RPC pool.
pub struct MulticallV3QuoteProvider {
    pool: Arc<HttpRpcPool>,
    quoter_addr: Address,
    multicall_addr: Address,
}

impl MulticallV3QuoteProvider {
    /// Build for a chain, or `None` when the quoter/multicall cannot be resolved
    /// (fail-honest — `StateProjector` then keeps V3 projection `None`).
    pub fn from_pool_and_chain(pool: Arc<HttpRpcPool>, chain_id: u64) -> Option<Self> {
        let (quoter_addr, multicall_addr) = resolve_quoter_multicall(chain_id)?;
        Some(Self {
            pool,
            quoter_addr,
            multicall_addr,
        })
    }
}

/// Resolve `(quoter, multicall)` for a chain. Order (no-hardcode, fail-honest):
///   1. env override `V3_QUOTER_<chain>` / `MULTICALL3_<chain>`,
///   2. `shared_rs::chains` catalog,
///   3. `None`.
pub(crate) fn resolve_quoter_multicall(chain_id: u64) -> Option<(Address, Address)> {
    let quoter = resolve_addr_env(&format!("V3_QUOTER_{chain_id}"))
        .or_else(|| quoter_v2_for_chain(chain_id).map(Address::from))?;
    let multicall = resolve_addr_env(&format!("MULTICALL3_{chain_id}"))
        .or_else(|| multicall3_for_chain(chain_id).map(Address::from))?;
    Some((quoter, multicall))
}

/// Parse an address from an env var. Unset/empty/invalid/zero → `None`.
fn resolve_addr_env(key: &str) -> Option<Address> {
    let raw = std::env::var(key).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let addr = trimmed.parse::<Address>().ok()?;
    if addr == Address::zero() {
        None
    } else {
        Some(addr)
    }
}

impl V3QuoteProvider for MulticallV3QuoteProvider {
    fn quote_exact_input_single(
        &self,
        pool: Address,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        fee_bps: u32,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<U256>> + Send + '_>> {
        let rpc_pool = self.pool.clone();
        let quoter = self.quoter_addr;
        let multicall = self.multicall_addr;
        Box::pin(async move {
            let reqs = vec![V3QuoteRequest {
                pool_addr: pool,
                token_in,
                token_out,
                amount_in,
                fee_bps,
            }];
            // with_retry engages circuit-breaker + failover; the closure may run
            // more than once, so clone the (single-element) request set per call.
            let results = rpc_pool
                .with_retry(|provider| {
                    let reqs = reqs.clone();
                    async move { v3_quote_exact_in_multicall(provider, quoter, multicall, reqs).await }
                })
                .await
                .map_err(|e| anyhow::anyhow!("v3 quote rpc failover exhausted: {e}"))?;

            match results.into_iter().next() {
                Some(r) if r.success => Ok(r.amount_out),
                Some(_) => Err(anyhow::anyhow!(
                    "v3 quote failed (insufficient liquidity / wrong fee tier / pool revert)"
                )),
                None => Err(anyhow::anyhow!("v3 quote returned an empty result set")),
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_resolves_from_catalog() {
        let (quoter, multicall) = resolve_quoter_multicall(1).expect("chain 1 must resolve");
        // QuoterV2 + canonical Multicall3 (non-zero, catalogued).
        assert_ne!(quoter, Address::zero());
        assert_ne!(multicall, Address::zero());
        assert_eq!(
            quoter,
            "0x61fFE014bA17989E743c5F6cB21bF9697530B21e"
                .parse::<Address>()
                .unwrap()
        );
        assert_eq!(
            multicall,
            "0xcA11bde05977b3631167028862bE2a173976CA11"
                .parse::<Address>()
                .unwrap()
        );
    }

    #[test]
    fn uncatalogued_chain_is_none() {
        // A chain with no catalog entry and no env override → fail-honest None.
        assert!(resolve_quoter_multicall(987_654).is_none());
    }

    #[test]
    fn resolve_addr_env_rejects_unset_empty_zero_junk() {
        // Unset.
        assert!(resolve_addr_env("ARBX_V3_TEST_UNSET_KEY_XYZ").is_none());
        // Empty.
        std::env::set_var("ARBX_V3_TEST_EMPTY_KEY_XYZ", "   ");
        assert!(resolve_addr_env("ARBX_V3_TEST_EMPTY_KEY_XYZ").is_none());
        std::env::remove_var("ARBX_V3_TEST_EMPTY_KEY_XYZ");
        // Zero address.
        std::env::set_var(
            "ARBX_V3_TEST_ZERO_KEY_XYZ",
            "0x0000000000000000000000000000000000000000",
        );
        assert!(resolve_addr_env("ARBX_V3_TEST_ZERO_KEY_XYZ").is_none());
        std::env::remove_var("ARBX_V3_TEST_ZERO_KEY_XYZ");
        // Junk.
        std::env::set_var("ARBX_V3_TEST_JUNK_KEY_XYZ", "not-an-address");
        assert!(resolve_addr_env("ARBX_V3_TEST_JUNK_KEY_XYZ").is_none());
        std::env::remove_var("ARBX_V3_TEST_JUNK_KEY_XYZ");
        // Valid override.
        std::env::set_var(
            "ARBX_V3_TEST_OK_KEY_XYZ",
            "0x1111111111111111111111111111111111111111",
        );
        assert_eq!(
            resolve_addr_env("ARBX_V3_TEST_OK_KEY_XYZ"),
            Some(
                "0x1111111111111111111111111111111111111111"
                    .parse::<Address>()
                    .unwrap()
            )
        );
        std::env::remove_var("ARBX_V3_TEST_OK_KEY_XYZ");
    }

    #[test]
    fn env_override_takes_precedence_over_catalog() {
        // Use an uncatalogued chain so only the env override can satisfy it.
        std::env::set_var("V3_QUOTER_424242", "0x2222222222222222222222222222222222222222");
        std::env::set_var("MULTICALL3_424242", "0x3333333333333333333333333333333333333333");
        let (q, m) = resolve_quoter_multicall(424_242).expect("env override must resolve");
        assert_eq!(q, "0x2222222222222222222222222222222222222222".parse::<Address>().unwrap());
        assert_eq!(m, "0x3333333333333333333333333333333333333333".parse::<Address>().unwrap());
        std::env::remove_var("V3_QUOTER_424242");
        std::env::remove_var("MULTICALL3_424242");
    }
}
