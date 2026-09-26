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
//! `debug!`-only failure visibility cost the original diagnosis). B1 closed
//! 2026-09-19 (V3-QUOTE-BATCH-20260919): `quote_batch` prefetches a tick's
//! V3 pools in `ARBX_V3_QUOTE_BATCH_SIZE`-wide aggregate3 multicalls before
//! the per-pair probing loop, so the unary lookups answer from this cache.

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
const DEFAULT_QUOTE_TTL_MS: u64 = 8_000;

/// TTL for a negative cache entry. Shorter than the positive TTL: a failed
/// provider call is often transient (429 / half-open breaker), so retrying
/// sooner is cheap once the cache has absorbed the storm.
const DEFAULT_QUOTE_NEG_TTL_MS: u64 = 2_000;

/// Default (and max sane) number of quote sub-calls per aggregate3 multicall.
/// 100 × ~150k gas per QuoterV2 call ≈ 15M gas, under the 25M hard cap
/// asserted by the batch gas test (amm_math::v3_tests).
const DEFAULT_BATCH_SIZE: usize = 100;
const MAX_BATCH_SIZE: usize = 1_000;

/// V3-QUOTE-BATCH-20260919 (B1): env knobs so the operator can tune cache
/// TTLs and batch width without a rebuild. Parsed once (OnceLock); unset,
/// empty or non-numeric values fall back to the defaults above (fail-honest —
/// a malformed knob never changes behavior silently to garbage).
fn env_ms(raw: Option<String>, default_ms: u64) -> Duration {
    raw.and_then(|v| v.trim().parse::<u64>().ok().map(Duration::from_millis))
        .unwrap_or(Duration::from_millis(default_ms))
}

fn quote_ttl() -> Duration {
    static V: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        env_ms(
            std::env::var("ARBX_V3_QUOTE_TTL_MS").ok(),
            DEFAULT_QUOTE_TTL_MS,
        )
    })
}

fn quote_neg_ttl() -> Duration {
    static V: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        env_ms(
            std::env::var("ARBX_V3_QUOTE_NEG_TTL_MS").ok(),
            DEFAULT_QUOTE_NEG_TTL_MS,
        )
    })
}

const DEFAULT_QUOTE_NEG_TRANSPORT_TTL_MS: u64 = 30_000;

/// Pure parse (unit-testable): `ARBX_V3_QUOTE_NEG_TRANSPORT_TTL_MS` → Duration.
fn parse_neg_transport_ttl_ms(raw: Option<String>) -> Duration {
    raw.and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|ms| *ms > 0)
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(DEFAULT_QUOTE_NEG_TRANSPORT_TTL_MS))
}

/// R5 (V3-QUOTE-TRANSPORT-TUNE 2026-09-25): negative TTL for TRANSPORT failures
/// (failover exhausted / all providers unhealthy), distinct from the short 2s
/// negative TTL used for per-pool reverts (tier/liquidity — cheap, worth
/// re-probing). A provider-wide outage re-probed every 2s is the 36.6
/// rejection/s loop that floods `v3_quote_unavailable`; the 30s default cuts
/// that churn 15× and lets the breaker floor (120s) do the pacing. Fail-honest
/// parse: malformed/zero → default.
fn quote_neg_transport_ttl() -> Duration {
    static V: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        parse_neg_transport_ttl_ms(std::env::var("ARBX_V3_QUOTE_NEG_TRANSPORT_TTL_MS").ok())
    })
}

const DEFAULT_QUOTE_BATCH_BACKOFF_MS: u64 = 30_000;

/// Pure parse (unit-testable): `ARBX_V3_QUOTE_BATCH_BACKOFF_MS` → Duration.
fn parse_batch_backoff_ms(raw: Option<String>) -> Duration {
    raw.and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|ms| *ms > 0)
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(DEFAULT_QUOTE_BATCH_BACKOFF_MS))
}

/// R5 v2 (peer-review P0, 2026-09-25): pause BATCH PREFETCH for this long after
/// a batch transport failure. The unary quote path is NEVER paused and never
/// reads this state. Fail-honest parse: malformed/zero → default.
fn batch_backoff() -> Duration {
    static V: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *V.get_or_init(|| parse_batch_backoff_ms(std::env::var("ARBX_V3_QUOTE_BATCH_BACKOFF_MS").ok()))
}

/// Pure gate (unit-testable).
fn batch_backoff_active_at(now: Instant, until: Option<Instant>) -> bool {
    until.is_some_and(|u| now < u)
}

fn quote_batch_size() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("ARBX_V3_QUOTE_BATCH_SIZE")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|n| n.clamp(1, MAX_BATCH_SIZE))
            .unwrap_or(DEFAULT_BATCH_SIZE)
    })
}

/// Pure chunk planner (unit-tested in isolation): how many aggregate3 calls a
/// set of `len` pending quotes needs at chunk width `max`, and how large the
/// final partial chunk is.
fn chunk_plan(len: usize, max: usize) -> (usize, usize) {
    if len == 0 || max == 0 {
        return (0, 0);
    }
    let chunks = len.div_ceil(max);
    (chunks, len - (chunks - 1) * max)
}

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
    /// R5: transport-failure negative (failover exhausted) — paced by
    /// `quote_neg_transport_ttl()` instead of the short pool-revert TTL.
    neg_transport: bool,
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
        let ttl = if e.neg_transport {
            quote_neg_transport_ttl()
        } else if e.neg {
            quote_neg_ttl()
        } else {
            quote_ttl()
        };
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
                neg_transport: false,
            },
        );
    }

    /// R5: store a TRANSPORT-failure negative with the longer pacing TTL.
    fn put_neg_transport(&mut self, key: QuoteKey, result: QuoteResult) {
        self.map.insert(
            key,
            CacheEntry {
                result,
                stored_at: Instant::now(),
                neg: true,
                neg_transport: true,
            },
        );
    }

    /// Evict expired entries; if still above the cap, drop arbitrary ones.
    fn evict(&mut self) {
        if self.map.len() <= QUOTE_CACHE_MAX {
            return;
        }
        self.map.retain(|_, e| {
            let ttl = if e.neg_transport {
                quote_neg_transport_ttl()
            } else if e.neg {
                quote_neg_ttl()
            } else {
                quote_ttl()
            };
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
    /// R5 v2 (peer-review P0): BATCH-scoped backoff. A batch transport failure
    /// pauses PREFETCH for `batch_backoff()` WITHOUT writing per-key cache
    /// entries — the authoritative unary path keeps quoting. Per-key transport
    /// negatives live only on the unary path itself.
    batch_backoff_until: std::sync::Mutex<Option<Instant>>,
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
            batch_backoff_until: std::sync::Mutex::new(None),
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

    /// R5 v2: store a TRANSPORT-failure negative with the longer pacing TTL
    /// (`quote_neg_transport_ttl`), bypassing the short pool-revert pacing.
    /// UNARY path only: the authoritative path pacing its own retry. The batch
    /// prefetch NEVER writes here (its failures set batch backoff instead).
    fn cache_put_neg_transport(&self, key: QuoteKey, result: QuoteResult) {
        let mut c = self.cache.write().unwrap_or_else(|e| e.into_inner());
        c.put_neg_transport(key, result);
        c.evict();
    }

    /// R5 v2 (peer-review P0): batch-scoped backoff control.
    fn set_batch_backoff(&self) {
        let mut g = self
            .batch_backoff_until
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *g = Some(Instant::now() + batch_backoff());
    }

    fn clear_batch_backoff(&self) {
        let mut g = self
            .batch_backoff_until
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *g = None;
    }

    fn batch_backoff_active(&self) -> bool {
        let g = self
            .batch_backoff_until
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        batch_backoff_active_at(Instant::now(), *g)
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

    /// B1 (V3-QUOTE-BATCH-20260919): prefetch a set of quotes in batched
    /// aggregate3 multicalls, filling the TTL cache so the per-pool
    /// `quote_exact_input_single` lookups that follow are cache hits.
    ///
    /// One `eth_call` carries up to `ARBX_V3_QUOTE_BATCH_SIZE` (default 100)
    /// quote sub-calls — ~100x fewer RPC round-trips than the unary path on
    /// the exact keys detection re-probes every tick.
    ///
    /// Semantics (best-effort, unary path stays authoritative):
    ///   - Duplicate keys are collapsed to one request.
    ///   - Keys with a fresh cache entry (positive or negative) are skipped
    ///     (`batch_cached_skip`).
    ///   - Keys already single-flighted by a concurrent unary caller are
    ///     skipped (`batch_inflight_skip`) — the batch takes the slot only
    ///     when free, so it never duplicates an in-flight unary quote.
    ///   - Per-element outcomes reuse the unary labels (`rpc`, `rpc_ok`,
    ///     `rpc_tier_revert`, `rpc_error`) so the
    ///     `arbx_v3_quote_total{outcome}` funnel stays comparable across
    ///     paths; chunk-level transport health lands in `batch_call`,
    ///     `batch_call_ok`, `batch_call_error`.
    ///   - Transport failure of a chunk pauses PREFETCH (`batch_backoff()`,
    ///     R5 v2) and writes NOTHING to the per-key cache — a failed batch
    ///     must never block a potentially profitable unary quote
    ///     (peer-review P0).
    async fn quote_batch_impl(
        &self,
        reqs: Vec<V3QuoteRequest>,
    ) -> Vec<(V3QuoteRequest, QuoteResult)> {
        let mut out: Vec<(V3QuoteRequest, QuoteResult)> = Vec::new();
        if reqs.is_empty() {
            return out;
        }
        if self.batch_backoff_active() {
            // R5 v2: prefetch paused after a batch transport failure; the
            // authoritative unary path is unaffected.
            quote_outcome_metric("batch_backoff_skip");
            return out;
        }

        // 1. Dedup by key, skip fresh cache entries, take free single-flight
        //    slots. Guards are held for the chunk's RPC so a concurrent unary
        //    caller for the same key waits and then reads the warm cache.
        let mut pending: Vec<(QuoteKey, V3QuoteRequest, tokio::sync::OwnedMutexGuard<()>)> =
            Vec::with_capacity(reqs.len());
        {
            let mut seen = std::collections::HashSet::<QuoteKey>::with_capacity(reqs.len());
            for req in reqs {
                let key: QuoteKey = (
                    req.pool_addr,
                    req.token_in,
                    req.token_out,
                    req.amount_in,
                    req.fee_bps,
                );
                if !seen.insert(key) {
                    continue;
                }
                if self.cache_get(&key).is_some() {
                    quote_outcome_metric("batch_cached_skip");
                    continue;
                }
                let slot = self.inflight_slot(&key);
                match Arc::clone(&slot).try_lock_owned() {
                    Ok(guard) => pending.push((key, req, guard)),
                    Err(_) => {
                        // A unary caller is mid-flight for this key — its
                        // result will land in the cache; skip (no duplicate).
                        quote_outcome_metric("batch_inflight_skip");
                    }
                }
            }
        }
        if pending.is_empty() {
            return out;
        }

        // 2. Dispatch one aggregate3 per chunk of `quote_batch_size()` keys.
        let size = quote_batch_size();
        let (n_chunks, last_chunk_len) = chunk_plan(pending.len(), size);
        tracing::debug!(
            event = "v3_quote.batch_dispatch",
            pending = pending.len(),
            chunks = n_chunks,
            last_chunk_len,
            batch_size = size,
        );
        let rpc_pool = self.pool.clone();
        let quoter = self.quoter_addr;
        let multicall = self.multicall_addr;
        for chunk in pending.chunks(size) {
            quote_outcome_metric("batch_call");
            let chunk_reqs: Vec<V3QuoteRequest> = chunk.iter().map(|(_, r, _)| r.clone()).collect();
            let rpc_result =
                rpc_pool
                    .with_retry(|provider| {
                        // with_retry may re-run the closure after failover, so the
                        // request set is rebuilt per attempt (same as the unary path).
                        let reqs = chunk_reqs.clone();
                        async move {
                            v3_quote_exact_in_multicall(provider, quoter, multicall, reqs).await
                        }
                    })
                    .await;

            match rpc_result {
                Ok(results) => {
                    quote_outcome_metric("batch_call_ok");
                    // R5 v2: batch recovered → prefetch resumes immediately.
                    self.clear_batch_backoff();
                    for (i, (key, req, _guard)) in chunk.iter().enumerate() {
                        quote_outcome_metric("rpc");
                        let res: QuoteResult = match results.get(i) {
                            Some(r) if r.success => {
                                quote_outcome_metric("rpc_ok");
                                Ok(r.amount_out)
                            }
                            // Per-pool revert inside a successful multicall:
                            // same classification as the unary path (WO-06).
                            Some(_) => {
                                quote_outcome_metric("rpc_tier_revert");
                                Err("v3 quote failed (insufficient liquidity / wrong fee tier / pool revert)".to_string())
                            }
                            None => {
                                quote_outcome_metric("rpc_error");
                                Err("v3 quote batch returned a short result set".to_string())
                            }
                        };
                        out.push((req.clone(), res.clone()));
                        self.cache_put(*key, res);
                        self.inflight_clear(key);
                    }
                }
                Err(e) => {
                    quote_outcome_metric("batch_call_error");
                    // R5 v2 (peer-review P0): the batch is BEST-EFFORT prefetch.
                    // Its transport failure must NOT poison the per-key quote
                    // cache — the unary path is authoritative and must keep
                    // trying every key. Storm control lives in the
                    // BATCH-scoped backoff (prefetch paused), never in per-key
                    // negatives that would block profitable unary quotes for
                    // the whole chunk (up to batch_size keys) during the TTL.
                    self.set_batch_backoff();
                    for (key, req, _guard) in chunk {
                        quote_outcome_metric("rpc");
                        quote_outcome_metric("rpc_error");
                        let res = Err(format!("v3 quote batch rpc failover exhausted: {e}"));
                        out.push((req.clone(), res.clone()));
                        self.inflight_clear(key);
                    }
                }
            }
        }
        out
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
            let mut transport_fail = false;
            let (outcome, rpc_result) = rpc_pool
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
                .map_err(|e| {
                    // R5: failover exhausted = TRANSPORT failure → long pacing TTL.
                    transport_fail = true;
                    anyhow::anyhow!("v3 quote rpc failover exhausted: {e}")
                })
                .map(|results| match results.into_iter().next() {
                    Some(r) if r.success => ("rpc_ok", Ok(r.amount_out)),
                    // Per-pool revert: the quoter call itself reverted at the
                    // requested tier (insufficient liquidity / wrong tier /
                    // pool revert). WO-06: split from transport failures — with
                    // catalog-resolved tiers this label is the tier-mismatch
                    // canary that used to hide inside `rpc_error`.
                    Some(_) => (
                        "rpc_tier_revert",
                        Err(anyhow::anyhow!(
                            "v3 quote failed (insufficient liquidity / wrong fee tier / pool revert)"
                        )),
                    ),
                    None => (
                        "rpc_error",
                        Err(anyhow::anyhow!("v3 quote returned an empty result set")),
                    ),
                })
                .unwrap_or_else(|e| ("rpc_error", Err(e)));

            // 5. Store (success + negative, distinct TTLs), release the slot,
            //    answer. The cache stores String errors (Clone); the returned
            //    anyhow::Error is rebuilt from the stored String on hits.
            let cached = rpc_result.as_ref().map_err(|e| e.to_string()).cloned();
            if transport_fail {
                // R5: transport-failure negative → long pacing TTL.
                self.cache_put_neg_transport(key, cached);
            } else {
                self.cache_put(key, cached);
            }
            self.inflight_clear(&key);
            quote_outcome_metric(outcome);
            rpc_result
        })
    }

    /// B1 batching: prefetch a set of quotes in batched multicalls. See
    /// `MulticallV3QuoteProvider::quote_batch_impl`.
    fn quote_batch(
        &self,
        reqs: Vec<V3QuoteRequest>,
    ) -> Pin<Box<dyn Future<Output = crate::state_projector::V3BatchQuoteResults> + Send + '_>>
    {
        Box::pin(self.quote_batch_impl(reqs))
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
    fn parse_neg_transport_ttl_fail_honest() {
        assert_eq!(
            parse_neg_transport_ttl_ms(None),
            Duration::from_millis(DEFAULT_QUOTE_NEG_TRANSPORT_TTL_MS)
        );
        assert_eq!(
            parse_neg_transport_ttl_ms(Some("junk".into())),
            Duration::from_millis(DEFAULT_QUOTE_NEG_TRANSPORT_TTL_MS)
        );
        assert_eq!(
            parse_neg_transport_ttl_ms(Some("0".into())),
            Duration::from_millis(DEFAULT_QUOTE_NEG_TRANSPORT_TTL_MS)
        );
        assert_eq!(
            parse_neg_transport_ttl_ms(Some("120000".into())),
            Duration::from_millis(120_000)
        );
    }

    /// R5: a transport negative stored 3s ago is still fresh (30s TTL), while a
    /// pool-revert negative stored 3s ago is stale (2s TTL). Deterministic: no
    /// env override for `ARBX_V3_QUOTE_NEG_TRANSPORT_TTL_MS` in CI → defaults.
    #[test]
    fn transport_negative_outlives_pool_revert_negative() {
        let mut cache = TtlQuoteCache::default();
        let key: QuoteKey = (
            Address::from_low_u64_be(1),
            Address::from_low_u64_be(2),
            Address::from_low_u64_be(3),
            U256::one(),
            500,
        );
        let stale_ago = Instant::now() - std::time::Duration::from_secs(3);
        cache.map.insert(
            key,
            CacheEntry {
                result: Err("tier revert".to_string()),
                stored_at: stale_ago,
                neg: true,
                neg_transport: false,
            },
        );
        assert!(
            cache.get_fresh(&key).is_none(),
            "2s pool-revert negative must expire at 3s"
        );
        cache.map.insert(
            key,
            CacheEntry {
                result: Err("failover exhausted".to_string()),
                stored_at: stale_ago,
                neg: true,
                neg_transport: true,
            },
        );
        assert!(
            cache.get_fresh(&key).is_some(),
            "30s transport negative must survive 3s"
        );
        let mut cache2 = TtlQuoteCache::default();
        cache2.put_neg_transport(key, Err("x".to_string()));
        let e = cache2.map.get(&key).unwrap();
        assert!(e.neg && e.neg_transport);
    }

    #[test]
    fn parse_batch_backoff_fail_honest() {
        assert_eq!(
            parse_batch_backoff_ms(None),
            Duration::from_millis(DEFAULT_QUOTE_BATCH_BACKOFF_MS)
        );
        assert_eq!(
            parse_batch_backoff_ms(Some("junk".into())),
            Duration::from_millis(DEFAULT_QUOTE_BATCH_BACKOFF_MS)
        );
        assert_eq!(
            parse_batch_backoff_ms(Some("0".into())),
            Duration::from_millis(DEFAULT_QUOTE_BATCH_BACKOFF_MS)
        );
        assert_eq!(
            parse_batch_backoff_ms(Some("60000".into())),
            Duration::from_millis(60_000)
        );
    }

    #[test]
    fn batch_backoff_gate_boundaries() {
        let t0 = Instant::now();
        assert!(!batch_backoff_active_at(t0, None));
        assert!(batch_backoff_active_at(
            t0,
            Some(t0 + Duration::from_secs(10))
        ));
        assert!(!batch_backoff_active_at(
            t0 + Duration::from_secs(10),
            Some(t0 + Duration::from_secs(10))
        ));
        assert!(!batch_backoff_active_at(
            t0 + Duration::from_secs(11),
            Some(t0 + Duration::from_secs(10))
        ));
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
        // Aged beyond the positive TTL → expired (None), even though still stored.
        age_entry(&mut c, &k, quote_ttl() + Duration::from_millis(1));
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
        // Aged past the negative TTL but BELOW the positive TTL → already
        // expired: negatives must not live as long as successes.
        age_entry(&mut c, &k, quote_neg_ttl() + Duration::from_millis(1));
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

    // ── V3-QUOTE-BATCH-20260919 (B1) ────────────────────────────────────────

    #[test]
    fn env_ms_parses_and_falls_back() {
        // Valid value wins.
        assert_eq!(
            env_ms(Some("1500".to_string()), 8_000),
            Duration::from_millis(1_500)
        );
        // Default on unset / junk / whitespace-padded junk (fail-honest).
        assert_eq!(env_ms(None, 8_000), Duration::from_millis(8_000));
        assert_eq!(
            env_ms(Some("junk".to_string()), 2_000),
            Duration::from_millis(2_000)
        );
        assert_eq!(
            env_ms(Some("  ".to_string()), 2_000),
            Duration::from_millis(2_000)
        );
        assert_eq!(
            env_ms(Some(" 250 ".to_string()), 2_000),
            Duration::from_millis(250)
        );
    }

    #[test]
    fn batch_size_env_clamps_to_sane_range() {
        let parse = |raw: Option<String>| {
            raw.and_then(|v| v.trim().parse::<usize>().ok())
                .map(|n| n.clamp(1, MAX_BATCH_SIZE))
                .unwrap_or(DEFAULT_BATCH_SIZE)
        };
        assert_eq!(parse(None), DEFAULT_BATCH_SIZE);
        assert_eq!(parse(Some("50".to_string())), 50);
        // Degenerate values clamp instead of producing an empty or runaway chunk.
        assert_eq!(parse(Some("0".to_string())), 1);
        assert_eq!(parse(Some("999999".to_string())), MAX_BATCH_SIZE);
        assert_eq!(parse(Some("junk".to_string())), DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn chunk_plan_boundaries() {
        assert_eq!(chunk_plan(0, 100), (0, 0));
        assert_eq!(chunk_plan(1, 100), (1, 1));
        assert_eq!(chunk_plan(100, 100), (1, 100));
        assert_eq!(chunk_plan(101, 100), (2, 1));
        assert_eq!(chunk_plan(250, 100), (3, 50));
        // A degenerate width of 0 never loops forever.
        assert_eq!(chunk_plan(10, 0), (0, 0));
    }
}
