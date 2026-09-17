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
//!
//! V3-QUOTE-CACHE-20260916 (B2+B3 of
//! audits/real-cycles-audit-20260916/ROOT-CAUSE-v3-quote.md): detection probes
//! use a FIXED probe amount (`probe_amount`, `dex_engine.rs`), so the same
//! (pool, direction, amount, fee) key is re-quoted hundreds of times per tick.
//! Against the public RPC failover pool that opened circuit breakers in cascade
//! (429/403) and rejected ~71% of candidates as `v3_quote_unavailable` — a
//! transport failure, not math. This provider now answers repeated keys from a
//! short-TTL cache with per-key single-flight, collapsing RPC volume by >10x.
//! Failures are negative-cached under a shorter TTL to absorb retry storms.
//! Every outcome is counted in `arbx_v3_quote_total{outcome}` (R8: the
//! `debug!`-only failure visibility cost the original diagnosis). B1 (batching
//! N quotes into one multicall) remains open — callers quote one pool at a
//! time; the cache removes the equivalent duplicate volume.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use ethers::types::{Address, U256};

use crate::amm_math::{v3_quote_exact_in_multicall, V3QuoteRequest};
use crate::state_projector::V3QuoteProvider;
use shared_rs::chains::{multicall3_for_chain, quoter_v2_for_chain};
use shared_rs::rpc_failover::HttpRpcPool;

/// TTL for a successful quote. On-chain state moves every block; Ethereum's
/// 12s blocks → 8s keeps a cached quote at most one block stale.
const QUOTE_TTL: Duration = Duration::from_secs(8);

/// TTL for a negative cache entry. Shorter than QUOTE_TTL: a failed provider
/// call is often transient (429 / half-open breaker), so retrying sooner is
/// cheap once the cache has absorbed the storm.
const QUOTE_NEG_TTL: Duration = Duration::from_secs(2);

/// Rough guard so the cache cannot grow unbounded in a long-lived process
/// (each entry is ~100 bytes; 4096 keys is far above the per-tick working set
/// of unique probed pools).
const QUOTE_CACHE_MAX: usize = 4096;

/// Cache key: fully determines the QuoterV2 response (pool state, direction,
/// size, tier) at a given block.
type QuoteKey = (Address, Address, Address, U256, u32);

type QuoteResult = Result<U256, String>;

struct CacheEntry {
    result: QuoteResult,
    stored_at: Instant,
    neg: bool,
}

/// TTL cache for V3 quotes — pure in-memory state, unit-tested in isolation
/// below (no RPC needed). Guarded externally by the provider.
#[derive(Default)]
struct TtlQuoteCache {
    map: HashMap<QuoteKey, CacheEntry>,
}

impl TtlQuoteCache {
    fn get_fresh(&self, key: &QuoteKey) -> Option<&QuoteResult> {
        let e = self.map.get(key)?;
        let ttl = if e.neg { QUOTE_NEG_TTL } else { QUOTE_TTL };
        (Instant::now().duration_since(e.stored_at) < ttl).then_some(&e.result)
    }

    fn put(&mut self, key: QuoteKey, result: QuoteResult) {
        let neg = result.is_err();
        self.map.insert(
            key,
            CacheEntry {
                result,
                stored_at: Instant::now(),
                neg,
            },
        );
    }

    /// Evict expired entries; if still above the cap, drop arbitrary ones.
    fn evict(&mut self) {
        if self.map.len() <= QUOTE_CACHE_MAX {
            return;
        }
        self.map.retain(|_, e| {
            let ttl = if e.neg { QUOTE_NEG_TTL } else { QUOTE_TTL };
            Instant::now().duration_since(e.stored_at) < ttl
        });
        while self.map.len() > QUOTE_CACHE_MAX {
            let k = match self.map.keys().next() {
                Some(k) => *k,
                None => break,
            };
            self.map.remove(&k);
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

/// Quotes V3 pools via the on-chain QuoterV2 (read-only) through the
/// failover-backed RPC pool, with a per-key TTL cache + single-flight.
pub struct MulticallV3QuoteProvider {
    pool: Arc<HttpRpcPool>,
    quoter_addr: Address,
    multicall_addr: Address,
    /// Completed quotes (TTL'd, success + negative).
    cache: RwLock<TtlQuoteCache>,
    /// Per-key single-flight slots. A caller for an in-flight key awaits that
    /// key's mutex, then re-checks the cache — it never issues a duplicate RPC.
    /// Different keys proceed in parallel (no global serialization).
    inflight: std::sync::Mutex<HashMap<QuoteKey, Arc<tokio::sync::Mutex<()>>>>,
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
            cache: RwLock::new(TtlQuoteCache::default()),
            inflight: std::sync::Mutex::new(HashMap::new()),
        })
    }

    fn cache_get(&self, key: &QuoteKey) -> Option<QuoteResult> {
        self.cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get_fresh(key)
            .cloned()
    }

    fn cache_put(&self, key: QuoteKey, result: QuoteResult) {
        let mut c = self.cache.write().unwrap_or_else(|e| e.into_inner());
        c.put(key, result);
        c.evict();
    }

    /// Acquire (or create) the single-flight slot for `key`.
    fn inflight_slot(&self, key: &QuoteKey) -> Arc<tokio::sync::Mutex<()>> {
        let mut inf = self.inflight.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(
            inf.entry(*key)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        )
    }

    /// Drop the single-flight slot for `key`. Waiters already holding the Arc
    /// are unaffected — they re-check the cache after acquiring.
    fn inflight_clear(&self, key: &QuoteKey) {
        let mut inf = self.inflight.lock().unwrap_or_else(|e| e.into_inner());
        inf.remove(key);
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

fn quote_outcome_metric(outcome: &str) {
    crate::metrics::V3_QUOTE_TOTAL
        .with_label_values(&[outcome])
        .inc();
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
            let key: QuoteKey = (pool, token_in, token_out, amount_in, fee_bps);

            // 1. Fresh cache hit (success or negative) → answer without RPC.
            if let Some(res) = self.cache_get(&key) {
                quote_outcome_metric(if res.is_ok() {
                    "cache_hit"
                } else {
                    "cache_neg_hit"
                });
                return res.map_err(anyhow::Error::msg);
            }

            // 2. Single-flight per key: concurrent same-key callers await this
            //    mutex; different keys are unaffected.
            let slot = self.inflight_slot(&key);
            let _guard = slot.lock().await;

            // 3. Re-check after acquiring: the leader may have filled the cache
            //    while we waited.
            if let Some(res) = self.cache_get(&key) {
                quote_outcome_metric(if res.is_ok() {
                    "cache_hit"
                } else {
                    "cache_neg_hit"
                });
                return res.map_err(anyhow::Error::msg);
            }

            // 4. One in-flight RPC for this key.
            quote_outcome_metric("rpc");
            let rpc_result =
                rpc_pool
                    .with_retry(|provider| {
                        // with_retry engages circuit-breaker + failover; the closure
                        // may run more than once, so build the (single-element)
                        // request set per attempt.
                        let reqs = vec![V3QuoteRequest {
                            pool_addr: pool,
                            token_in,
                            token_out,
                            amount_in,
                            fee_bps,
                        }];
                        async move {
                            v3_quote_exact_in_multicall(provider, quoter, multicall, reqs).await
                        }
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("v3 quote rpc failover exhausted: {e}"))
                    .and_then(|results| match results.into_iter().next() {
                        Some(r) if r.success => Ok(r.amount_out),
                        Some(_) => Err(anyhow::anyhow!(
                        "v3 quote failed (insufficient liquidity / wrong fee tier / pool revert)"
                    )),
                        None => Err(anyhow::anyhow!("v3 quote returned an empty result set")),
                    });

            // 5. Store (success + negative, distinct TTLs), release the slot,
            //    answer. The cache stores String errors (Clone); the returned
            //    anyhow::Error is rebuilt from the stored String on hits.
            self.cache_put(key, rpc_result.as_ref().map_err(|e| e.to_string()).cloned());
            self.inflight_clear(&key);
            quote_outcome_metric(if rpc_result.is_ok() {
                "rpc_ok"
            } else {
                "rpc_error"
            });
            rpc_result
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
        std::env::set_var(
            "V3_QUOTER_424242",
            "0x2222222222222222222222222222222222222222",
        );
        std::env::set_var(
            "MULTICALL3_424242",
            "0x3333333333333333333333333333333333333333",
        );
        let (q, m) = resolve_quoter_multicall(424_242).expect("env override must resolve");
        assert_eq!(
            q,
            "0x2222222222222222222222222222222222222222"
                .parse::<Address>()
                .unwrap()
        );
        assert_eq!(
            m,
            "0x3333333333333333333333333333333333333333"
                .parse::<Address>()
                .unwrap()
        );
        std::env::remove_var("V3_QUOTER_424242");
        std::env::remove_var("MULTICALL3_424242");
    }

    // ── TtlQuoteCache (V3-QUOTE-CACHE-20260916) ─────────────────────────────

    fn test_key(n: u64) -> QuoteKey {
        let addr = Address::from_low_u64_be(n);
        (addr, addr, addr, U256::from(1_000_000u64), 500)
    }

    /// Force an entry to look `aged` old by rewinding its `stored_at`.
    fn age_entry(cache: &mut TtlQuoteCache, key: &QuoteKey, aged: Duration) {
        let e = cache.map.get_mut(key).expect("key present");
        e.stored_at = Instant::now().checked_sub(aged).expect("rewind");
    }

    #[test]
    fn ttl_cache_serves_fresh_and_expires() {
        let mut c = TtlQuoteCache::default();
        let k = test_key(1);
        c.put(k, Ok(U256::from(42u64)));
        // Fresh hit.
        assert_eq!(c.get_fresh(&k), Some(&Ok(U256::from(42u64))));
        // Aged beyond QUOTE_TTL → expired (None), even though still stored.
        age_entry(&mut c, &k, QUOTE_TTL + Duration::from_millis(1));
        assert_eq!(c.get_fresh(&k), None);
        assert_eq!(c.len(), 1, "expired entry is evicted lazily, not dropped");
    }

    #[test]
    fn ttl_cache_negative_entry_has_shorter_ttl() {
        let mut c = TtlQuoteCache::default();
        let k = test_key(2);
        c.put(k, Err("rpc failover exhausted".to_string()));
        // Fresh negative hit (absorbs the retry storm).
        assert_eq!(
            c.get_fresh(&k),
            Some(&Err("rpc failover exhausted".to_string()))
        );
        // Aged past QUOTE_NEG_TTL but BELOW QUOTE_TTL → already expired:
        // negatives must not live as long as successes.
        age_entry(&mut c, &k, QUOTE_NEG_TTL + Duration::from_millis(1));
        assert_eq!(c.get_fresh(&k), None);
    }

    #[test]
    fn ttl_cache_eviction_caps_size() {
        let mut c = TtlQuoteCache::default();
        for i in 0..(QUOTE_CACHE_MAX as u64 + 50) {
            c.put(test_key(i), Ok(U256::from(i)));
        }
        assert!(c.len() > QUOTE_CACHE_MAX);
        c.evict();
        assert!(c.len() <= QUOTE_CACHE_MAX, "cap enforced: {}", c.len());
        // Surviving entries must still be answerable.
        let any_key = test_key(0);
        let _ = c.get_fresh(&any_key);
    }
}
