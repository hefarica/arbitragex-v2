//! rpc_failover — multi-vendor RPC pool with health, drift, circuit breaker.
//!
//! Implements `arbx-rpc-failover-discipline`:
//!   - ≥2 providers per chain recommended; pool of 1 is allowed but warns
//!     (operational debt flag for the operator to fix during onboarding fase 5).
//!   - Active health checks every 15s via `eth_blockNumber`.
//!   - Drift detection: a provider >2 blocks behind max for >60s → Degraded.
//!   - Circuit breaker per provider: 5 errors / 60s → Open (effective cooldown: 30s base,
//!     120s floor when rate-limit-opened, doubling per reopen, cap 600s — ARBX-R-0003)
//!     → Half-open → probe (real eth_call load-proof for rate-limit openings,
//!     blockNumber otherwise) → success → Healthy.
//!   - Selection: best-of-Healthy by EWMA latency (recently-rate-limited entries rank
//!     behind clean ones for 60s — sticky penalty, ARBX-R-0003), fallback to Degraded
//!     if none Healthy, never Open.
//!   - No-hardcode: empty env → empty pool. Service must stay idle (no default URLs).
//!   - Honest: PoolError::AllUnhealthy when no provider is selectable.
//!
//! Env format per chain (production):
//!   RPC_HTTP_1 = "alchemy=https://eth-mainnet.g.alchemy.com/v2/KEY,infura=https://mainnet.infura.io/v3/KEY"
//!   RPC_WS_1   = "alchemy=wss://eth-mainnet.g.alchemy.com/v2/KEY,infura=wss://mainnet.infura.io/ws/v3/KEY"
//!
//! Env format per chain (single-vendor onboarding fase, allowed but warned):
//!   RPC_HTTP_1 = "https://eth-mainnet.g.alchemy.com/v2/KEY"
//!     → parsed as one entry named "primary" with `single_vendor=true` warning.

use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Alloy 1.0 HTTP provider type used throughout this module.
///
/// In alloy 1.0, `RootProvider<N>` is parameterized by the NETWORK (not the
/// transport). `ProviderBuilder::new().connect_http(url)` returns
/// `RootProvider<Ethereum>`. `disable_recommended_fillers()` removes the gas/
/// nonce fillers — this provider is used exclusively for read-only view calls
/// (eth_blockNumber, eth_chainId, eth_call) in the MEV searcher.
pub type AlloyHttpProvider = RootProvider<Ethereum>;

// ---------- tunables (constants — not productive data, doctrine OK) ----------

pub const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(15);
pub const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
pub const DRIFT_THRESHOLD_BLOCKS: u64 = 2;
pub const DRIFT_PERSIST: Duration = Duration::from_secs(60);
pub const CB_ERROR_LIMIT: usize = 5;
pub const CB_WINDOW: Duration = Duration::from_secs(60);
pub const CB_OPEN_DURATION: Duration = Duration::from_secs(30);
pub const EWMA_ALPHA_BPS: u64 = 3000; // 0.30 weight on new sample (in basis points / 10000)

/// Boot-time eth_chainId probe attempts per provider before dropping it.
/// MC-RPC-1: a provider that was merely slow at boot used to be excluded
/// PERMANENTLY (10 `boot_check_failed` drops in 48h on prod silently shrank
/// the failover pool). Retrying keeps transient outages from evicting
/// healthy providers; a wrong chain id remains a permanent, no-retry drop.
pub const BOOT_PROBE_ATTEMPTS: u32 = 3;
pub const BOOT_PROBE_RETRY_DELAY: Duration = Duration::from_millis(750);

// ---------- ARBX-R-0003: rate-limit (429) policy ----------
//
// The 429-storm incident (64 rate-limit errors / 10 min on the quoter path)
// had a structural cause beyond any single dead provider: the standard 30s
// breaker cooldown lets a CHEAP `eth_blockNumber` health probe close the
// circuit while the provider still 429s the `eth_call` load the quoter
// actually issues — health-pass ≠ load-ok. The circuit then reopens under
// the same load, in an open/close/reopen hammer loop. Three mitigations:
// a rate-limit cooldown floor + exponential reopen backoff, a sticky pick
// penalty, and a load-proof half-open probe (real eth_call).

/// Cooldown floor while the breaker opened on a rate-limit-class failure.
pub const RATE_LIMIT_MIN_COOLDOWN: Duration = Duration::from_secs(120);
/// Backoff before `with_retry`'s second attempt when the first failed with
/// a rate-limit-class error — respect the provider's backpressure instead
/// of hitting the backup instantly from the same hot loop.
pub const RATE_LIMIT_RETRY_BACKOFF: Duration = Duration::from_millis(500);
/// A Healthy provider that rate-limited within this window is picked LAST
/// among Healthy entries (sticky penalty — the selection weight of R-0003).
pub const RATE_LIMIT_STICKY: Duration = Duration::from_secs(60);
/// Cap of the exponential reopen backoff (`base × 2^(open_count−1)`).
pub const REOPEN_BACKOFF_CAP: Duration = Duration::from_secs(600);

/// Universal Multicall3 CREATE2 address — identical on every EVM chain where
/// it is deployed. A protocol constant (same standing as the public chain
/// IDs in the env generator), NOT operator config; the full ABI the repo
/// already uses lives at `searcher-rs/src/amm_math.rs` (`aggregate3Call`).
pub const MULTICALL3_ADDRESS: alloy::primitives::Address =
    alloy::primitives::address!("0xcA11bde05977b3631167028862bE2a173976CA11");

/// `aggregate3([])` — the FULL calldata encoding of an empty calls array:
/// selector + head offset (0x20) + array length (0), 68 bytes. Selector-only
/// would REVERT in the ABI decoder (out-of-bounds head read), turning every
/// load probe into a failure and pinning rate-limited breakers open forever —
/// the searcher-rs pin test exists to keep this byte-exact against the real
/// `aggregate3Call` encoding. Cheap, but it travels the provider's full
/// eth_call surface — exactly what a 429 rate-limits — which an
/// `eth_blockNumber` probe never touches.
pub const LOAD_PROBE_CALLDATA: &[u8] = &[
    // aggregate3(Call3[]) selector
    0x82, 0xad, 0x56, 0xcb, // head: offset of the dynamic `calls` argument (32)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32,
    // tail: Call3[] length = 0 (empty)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// ---------- errors ----------

#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("no RPC providers configured for chain_id={0}")]
    Empty(u64),
    #[error("all providers unhealthy for chain_id={0}")]
    AllUnhealthy(u64),
    #[error("invalid url for provider {name}: {detail}")]
    InvalidUrl { name: String, detail: String },
    #[error("chain id mismatch for {name}: expected {expected} got {observed}")]
    ChainIdMismatch {
        name: String,
        expected: u64,
        observed: u64,
    },
    #[error("provider error: {0}")]
    Provider(String),
}

// ---------- state ----------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderState {
    Healthy = 0,
    Degraded = 1,
    Open = 2,
}

impl ProviderState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderState::Healthy => "healthy",
            ProviderState::Degraded => "degraded",
            ProviderState::Open => "open",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => ProviderState::Healthy,
            1 => ProviderState::Degraded,
            _ => ProviderState::Open,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CircuitState {
    pub failures_window: Vec<Instant>, // sliding `CB_WINDOW`
    pub opened_at: Option<Instant>,
    pub half_open_pending: bool,
    pub drift_first_seen: Option<Instant>,
    /// ARBX-R-0003: total openings over this process lifetime — the exponent
    /// of the reopen backoff (`open_count − 1`). Deliberately NOT reset on a
    /// successful close: a chronically rate-limited free provider does not
    /// re-earn short cooldowns, and a healthy provider never opens, so it
    /// pays nothing.
    pub open_count: u32,
    /// ARBX-R-0003: the OPENING failure was rate-limit class → extended
    /// cooldown floor + the load-proof half-open probe.
    pub opened_by_rate_limit: bool,
    /// ARBX-R-0003: last rate-limit-class failure seen on this entry — drives
    /// the `pick()` sticky penalty while recent.
    pub last_rate_limit_at: Option<Instant>,
}

// ---------- HTTP entry ----------

#[derive(Debug)]
pub struct HttpEntry {
    pub name: String,
    pub url: String,
    pub provider: Arc<AlloyHttpProvider>,
    pub state: AtomicU8,
    pub last_block: AtomicU64,
    pub latency_ms_ewma: AtomicU64,
    pub circuit: RwLock<CircuitState>,
}

impl HttpEntry {
    pub fn snapshot_state(&self) -> ProviderState {
        ProviderState::from_u8(self.state.load(Ordering::Relaxed))
    }

    pub fn snapshot_latency_ms(&self) -> u64 {
        self.latency_ms_ewma.load(Ordering::Relaxed)
    }

    pub fn snapshot_block(&self) -> u64 {
        self.last_block.load(Ordering::Relaxed)
    }

    pub fn set_state(&self, s: ProviderState) {
        self.state.store(s as u8, Ordering::Relaxed);
        crate::metrics::RPC_PROVIDER_STATE
            .with_label_values(&[self.name.as_str(), "http"])
            .set(s as u8 as i64);
    }
}

// ---------- HTTP pool ----------

pub struct HttpRpcPool {
    pub chain_id: u64,
    pub entries: Vec<Arc<HttpEntry>>,
}

impl HttpRpcPool {
    /// Build a pool from `RPC_HTTP_<chain_id>` env. Returns `Ok(None)` if the env
    /// var is missing or empty (caller decides whether that is fatal — we never
    /// fabricate a default URL).
    pub async fn from_env(chain_id: u64) -> Result<Option<Self>, PoolError> {
        let key = format!("RPC_HTTP_{chain_id}");
        let raw = std::env::var(&key).unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(None);
        }
        Self::from_csv(chain_id, &raw).await.map(Some)
    }

    /// Parse the CSV form (with or without `name=` prefixes) into a live pool.
    ///
    /// Each entry's `chain_id` is validated against the on-chain `eth_chainid` to
    /// catch the classic "wrong RPC URL" misconfiguration. Entries that fail the
    /// chain-id check are omitted; if all fail we return `Empty`.
    pub async fn from_csv(chain_id: u64, csv: &str) -> Result<Self, PoolError> {
        let raw_entries = parse_csv(csv)?;
        if raw_entries.is_empty() {
            return Err(PoolError::Empty(chain_id));
        }

        let mut alive: Vec<Arc<HttpEntry>> = Vec::with_capacity(raw_entries.len());
        for (name, url) in raw_entries {
            let parsed_url = match url.parse::<reqwest::Url>() {
                Ok(u) => u,
                Err(e) => {
                    warn!(
                        event = "rpc_pool.invalid_url",
                        chain_id, name, error = %e,
                        "dropping provider — invalid URL"
                    );
                    continue;
                }
            };
            // Client-level HTTP request timeout. A hung eth_call (observed: a Multicall3
            // aggregate3 to a throttled public RPC never returned) cannot be preempted by an
            // OUTER tokio::time::timeout when the future wedges in poll() without yielding —
            // reqwest enforces THIS timeout inside the transport and returns a timeout error
            // the failover/circuit layer handles cleanly. Env-tunable; default 10s.
            let http_timeout_ms: u64 = std::env::var("RPC_HTTP_REQUEST_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&n| n > 0)
                .unwrap_or(10_000);
            // `with_reqwest` hands the closure alloy's OWN reqwest::ClientBuilder, so we set the
            // timeout without any reqwest-version-mismatch on the Client type.
            let provider = Arc::new(
                ProviderBuilder::new()
                    .disable_recommended_fillers()
                    .with_reqwest(parsed_url, |b| {
                        b.timeout(std::time::Duration::from_millis(http_timeout_ms))
                            .build()
                            .expect("build reqwest client with request timeout")
                    }),
            );
            // Validate chain id once; on mismatch we drop the entry rather than
            // poisoning the pool (operator catches it via the warn log). A wrong
            // chain is a permanent config error — no retry. A transient network
            // error, however, gets BOOT_PROBE_ATTEMPTS tries before the entry
            // is dropped (MC-RPC-1: boot drops used to be permanent).
            let mut admitted = false;
            for attempt in 1..=BOOT_PROBE_ATTEMPTS {
                match provider.get_chain_id().await {
                    Ok(observed) => {
                        if observed != chain_id {
                            warn!(
                                event = "rpc_pool.chain_mismatch",
                                chain_id_expected = chain_id,
                                chain_id_observed = observed,
                                name = name.as_str(),
                                "dropping provider — wrong chain"
                            );
                        } else {
                            admitted = true;
                        }
                        break;
                    }
                    Err(_) if attempt < BOOT_PROBE_ATTEMPTS => {
                        // The error payload can embed the provider URL (API
                        // key) — deliberately NOT logged; retry silently.
                        debug!(
                            event = "rpc_pool.boot_probe_retry",
                            chain_id,
                            name = name.as_str(),
                            attempt,
                            "boot eth_chainid failed — retrying"
                        );
                        tokio::time::sleep(BOOT_PROBE_RETRY_DELAY).await;
                    }
                    Err(e) => {
                        warn!(
                            event = "rpc_pool.boot_check_failed",
                            chain_id,
                            name = name.as_str(),
                            attempts = BOOT_PROBE_ATTEMPTS,
                            error = %e,
                            "dropping provider — boot eth_chainid failed after retries"
                        );
                    }
                }
            }
            if !admitted {
                continue;
            }
            alive.push(Arc::new(HttpEntry {
                name,
                url,
                provider,
                state: AtomicU8::new(ProviderState::Healthy as u8),
                last_block: AtomicU64::new(0),
                latency_ms_ewma: AtomicU64::new(0),
                circuit: RwLock::new(CircuitState::default()),
            }));
        }

        if alive.is_empty() {
            return Err(PoolError::Empty(chain_id));
        }
        if alive.len() == 1 {
            warn!(
                event = "rpc_pool.single_vendor",
                chain_id,
                name = %alive[0].name,
                "only ONE RPC provider configured for this chain — failover is not possible. \
                 Add a second vendor in env RPC_HTTP_{chain_id} (CSV: name=url,name=url)."
            );
        } else {
            info!(
                event = "rpc_pool.ready",
                chain_id,
                count = alive.len(),
                providers = ?alive.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
                "http rpc pool initialized"
            );
        }

        // Initialize gauges so dashboards show all entries from the first scrape.
        for e in &alive {
            crate::metrics::RPC_PROVIDER_STATE
                .with_label_values(&[e.name.as_str(), "http"])
                .set(ProviderState::Healthy as u8 as i64);
            crate::metrics::RPC_PROVIDER_BLOCK_HEIGHT
                .with_label_values(&[e.name.as_str(), "http"])
                .set(0);
        }

        Ok(Self {
            chain_id,
            entries: alive,
        })
    }

    /// Pick the best provider for a read operation, or return AllUnhealthy.
    /// Selection rule: lowest EWMA latency among Healthy — EXCEPT that a
    /// Healthy entry still inside the rate-limit sticky window (R-0003)
    /// sorts behind every clean Healthy entry (the selection weight: a
    /// recently-429ing provider is only chosen when it is the only Healthy
    /// option); if none Healthy, lowest EWMA latency among Degraded; never
    /// Open.
    pub fn pick(&self) -> Result<Arc<HttpEntry>, PoolError> {
        let now = Instant::now();
        let mut best_clean: Option<&Arc<HttpEntry>> = None;
        let mut best_sticky: Option<&Arc<HttpEntry>> = None;
        let mut best_degraded: Option<&Arc<HttpEntry>> = None;

        for e in &self.entries {
            match e.snapshot_state() {
                ProviderState::Open => continue,
                ProviderState::Healthy => {
                    let lat = e.snapshot_latency_ms();
                    let better = |b: Option<&Arc<HttpEntry>>| {
                        b.map(|b| lat < b.snapshot_latency_ms() || b.snapshot_latency_ms() == 0)
                            .unwrap_or(true)
                    };
                    if is_rate_limit_sticky(e, now) {
                        if better(best_sticky) {
                            best_sticky = Some(e);
                        }
                    } else if better(best_clean) {
                        best_clean = Some(e);
                    }
                }
                ProviderState::Degraded => {
                    let lat = e.snapshot_latency_ms();
                    if best_degraded
                        .map(|b| lat < b.snapshot_latency_ms() || b.snapshot_latency_ms() == 0)
                        .unwrap_or(true)
                    {
                        best_degraded = Some(e);
                    }
                }
            }
        }

        match best_clean.or(best_sticky).or(best_degraded) {
            Some(e) => Ok(Arc::clone(e)),
            None => Err(PoolError::AllUnhealthy(self.chain_id)),
        }
    }

    /// Execute `op` against the best provider, with one retry on a different
    /// provider if the first fails. Reports outcomes back to the pool so the
    /// circuit breaker stays in sync. The closure receives `Arc<AlloyHttpProvider>`
    /// (alloy 1.0 `RootProvider<Http<reqwest::Client>>`).
    pub async fn with_retry<F, Fut, R>(&self, op: F) -> Result<R, PoolError>
    where
        F: Fn(Arc<AlloyHttpProvider>) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<R>>,
    {
        // Try 1: best provider.
        let first = self.pick()?;
        let started = Instant::now();
        match op(first.provider.clone()).await {
            Ok(v) => {
                self.report_success(&first, started.elapsed()).await;
                return Ok(v);
            }
            Err(e) => {
                let msg = format!("{e}");
                self.report_failure(&first, &msg).await;
                // R-0003: a rate-limited provider is screaming backpressure —
                // back off briefly before the backup attempt instead of
                // firing it from the same hot instant.
                let backoff = retry_backoff_for(&msg);
                if !backoff.is_zero() {
                    tokio::time::sleep(backoff).await;
                }
            }
        }

        // Try 2: any other Healthy/Degraded provider that isn't `first`.
        let backup = self.entries.iter().find(|e| {
            !Arc::ptr_eq(e, &first) && !matches!(e.snapshot_state(), ProviderState::Open)
        });
        if let Some(bk) = backup {
            crate::metrics::RPC_POOL_FAILOVERS_TOTAL
                .with_label_values(&[&self.chain_id.to_string()])
                .inc();
            let started = Instant::now();
            match op(bk.provider.clone()).await {
                Ok(v) => {
                    self.report_success(bk, started.elapsed()).await;
                    return Ok(v);
                }
                Err(e) => {
                    self.report_failure(bk, &format!("{e}")).await;
                }
            }
        }

        Err(PoolError::AllUnhealthy(self.chain_id))
    }

    /// Mark a successful call: bump EWMA latency, possibly close half-open.
    pub async fn report_success(&self, entry: &Arc<HttpEntry>, latency: Duration) {
        let lat_ms = latency.as_millis().min(u64::MAX as u128) as u64;
        let prev = entry.latency_ms_ewma.load(Ordering::Relaxed);
        let next = if prev == 0 {
            lat_ms
        } else {
            // EWMA: new = alpha*sample + (1-alpha)*prev (in basis points)
            let alpha = EWMA_ALPHA_BPS;
            let beta = 10_000u64.saturating_sub(alpha);
            (alpha.saturating_mul(lat_ms) + beta.saturating_mul(prev)) / 10_000
        };
        entry.latency_ms_ewma.store(next, Ordering::Relaxed);
        crate::metrics::RPC_PROVIDER_LATENCY_MS
            .with_label_values(&[entry.name.as_str(), "http"])
            .observe(lat_ms as f64);

        let mut cb = entry.circuit.write().await;
        if entry.snapshot_state() == ProviderState::Open && cb.half_open_pending {
            cb.opened_at = None;
            cb.half_open_pending = false;
            cb.failures_window.clear();
            entry.set_state(ProviderState::Healthy);
            info!(
                event = "rpc_pool.circuit_closed",
                provider = %entry.name,
                "half-open success → circuit closed"
            );
        }
    }

    /// Mark a failed call: append failure to window, possibly trip the breaker.
    pub async fn report_failure(&self, entry: &Arc<HttpEntry>, cause: &str) {
        let cause_class = classify_cause(cause);
        crate::metrics::RPC_PROVIDER_ERRORS_TOTAL
            .with_label_values(&[entry.name.as_str(), "http", cause_class])
            .inc();
        emit_rotation_needed_if_credential_error(entry, cause);
        let mut cb = entry.circuit.write().await;
        let now = Instant::now();
        cb.failures_window
            .retain(|t| now.duration_since(*t) < CB_WINDOW);
        cb.failures_window.push(now);
        // R-0003: a rate-limit-class failure leaves the sticky mark even
        // before/without a trip — pick() deprioritizes the provider for the
        // sticky window so the hot path stops hammering it.
        if cause_class == "rate_limit" {
            cb.last_rate_limit_at = Some(now);
        }

        if cb.failures_window.len() >= CB_ERROR_LIMIT && cb.opened_at.is_none() {
            cb.opened_at = Some(now);
            cb.half_open_pending = false;
            cb.open_count = cb.open_count.saturating_add(1);
            cb.opened_by_rate_limit = cause_class == "rate_limit";
            entry.set_state(ProviderState::Open);
            warn!(
                event = "rpc_pool.circuit_opened",
                provider = %entry.name,
                failures = cb.failures_window.len(),
                window_secs = CB_WINDOW.as_secs(),
                rate_limit = cb.opened_by_rate_limit,
                "circuit breaker tripped — provider muted for {}s",
                effective_cooldown(&cb).as_secs()
            );
        } else {
            debug!(
                event = "rpc_pool.failure",
                provider = %entry.name,
                failures = cb.failures_window.len(),
                cause
            );
        }
    }

    /// Spawn a background task that pings each provider every
    /// `HEALTH_CHECK_INTERVAL`, applies drift detection, and rotates the
    /// circuit breaker (Open → Half-open after cooldown).
    pub fn spawn_health_loop(&self) -> tokio::task::JoinHandle<()> {
        let entries = self.entries.clone();
        let chain_id = self.chain_id;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(HEALTH_CHECK_INTERVAL);
            loop {
                ticker.tick().await;
                let mut max_block: u64 = 0;
                for e in &entries {
                    // Cool-down rotation: Open → Half-open after the EFFECTIVE
                    // cooldown (R-0003: rate-limit openings floor at 120s and
                    // reopens back off exponentially — the fixed 30s fed the
                    // open/close/reopen hammer loop).
                    {
                        let mut cb = e.circuit.write().await;
                        if let Some(opened_at) = cb.opened_at {
                            if opened_at.elapsed() >= effective_cooldown(&cb)
                                && !cb.half_open_pending
                            {
                                cb.half_open_pending = true;
                                e.set_state(ProviderState::Degraded);
                                info!(
                                    event = "rpc_pool.circuit_half_open",
                                    provider = %e.name,
                                    "cooldown elapsed → half-open (one probe will decide)"
                                );
                            }
                        }
                    }

                    // Probe. R-0003: a rate-limit-opened breaker reopens on a
                    // LOAD proof — a real eth_call (Multicall3 aggregate3([]))
                    // — because the 429-storm incident showed a cheap
                    // blockNumber probe passing while eth_calls kept failing.
                    // Every other state probes with blockNumber as before.
                    let load_probe = {
                        let cb = e.circuit.read().await;
                        half_open_probe_is_load(&cb)
                    };
                    let started = Instant::now();
                    let probed = if load_probe {
                        let tx = load_probe_transaction();
                        tokio::time::timeout(HEALTH_CHECK_TIMEOUT, e.provider.call(tx))
                            .await
                            .map(|r| r.map(|_| None))
                    } else {
                        tokio::time::timeout(HEALTH_CHECK_TIMEOUT, e.provider.get_block_number())
                            .await
                            .map(|r| r.map(Some))
                    };
                    match probed {
                        Ok(Ok(bn)) => {
                            if let Some(bn) = bn {
                                let bn: u64 = bn;
                                e.last_block.store(bn, Ordering::Relaxed);
                                crate::metrics::RPC_PROVIDER_BLOCK_HEIGHT
                                    .with_label_values(&[e.name.as_str(), "http"])
                                    .set(bn as i64);
                                if bn > max_block {
                                    max_block = bn;
                                }
                            }
                            // For success, run through pool's report_success-like
                            // path inline (we don't have &self here; emulate).
                            let lat_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                            let prev = e.latency_ms_ewma.load(Ordering::Relaxed);
                            let next = if prev == 0 {
                                lat_ms
                            } else {
                                let alpha = EWMA_ALPHA_BPS;
                                let beta = 10_000u64.saturating_sub(alpha);
                                (alpha.saturating_mul(lat_ms) + beta.saturating_mul(prev)) / 10_000
                            };
                            e.latency_ms_ewma.store(next, Ordering::Relaxed);
                            crate::metrics::RPC_PROVIDER_LATENCY_MS
                                .with_label_values(&[e.name.as_str(), "http"])
                                .observe(lat_ms as f64);

                            // Half-open success → close circuit. A load-probe
                            // success closes it too — that WAS the load proof.
                            let mut cb = e.circuit.write().await;
                            if cb.half_open_pending {
                                cb.opened_at = None;
                                cb.half_open_pending = false;
                                cb.failures_window.clear();
                                e.set_state(ProviderState::Healthy);
                                info!(
                                    event = "rpc_pool.circuit_closed",
                                    provider = %e.name,
                                    load_prove = load_probe,
                                    "half-open probe success → healthy"
                                );
                            } else if e.snapshot_state() == ProviderState::Degraded {
                                // Recovery from drift-only degradation handled below.
                            }
                        }
                        Ok(Err(err)) => {
                            let cause = format!("{err}");
                            let cause_class = classify_cause(&cause);
                            emit_rotation_needed_if_credential_error(e, &cause);
                            crate::metrics::RPC_PROVIDER_ERRORS_TOTAL
                                .with_label_values(&[e.name.as_str(), "http", cause_class])
                                .inc();
                            let mut cb = e.circuit.write().await;
                            let now = Instant::now();
                            // R-0003: rate-limit-class health failures carry the
                            // sticky penalty too — the probe path sees the same
                            // 429 the request path does.
                            if cause_class == "rate_limit" {
                                cb.last_rate_limit_at = Some(now);
                            }
                            cb.failures_window
                                .retain(|t| now.duration_since(*t) < CB_WINDOW);
                            cb.failures_window.push(now);
                            if cb.half_open_pending {
                                // R-0003: a FAILED half-open probe re-opens the
                                // breaker (classic half-open semantics) so a
                                // provider still rate-limiting is not load-probed
                                // every 15s; `open_count` stepping up doubles the
                                // next cooldown.
                                cb.half_open_pending = false;
                                cb.opened_at = Some(now);
                                cb.open_count = cb.open_count.saturating_add(1);
                                cb.opened_by_rate_limit =
                                    cb.opened_by_rate_limit || cause_class == "rate_limit";
                                e.set_state(ProviderState::Open);
                                warn!(
                                    event = "rpc_pool.circuit_reopened",
                                    provider = %e.name,
                                    cooldown_secs = effective_cooldown(&cb).as_secs(),
                                    "half-open probe failed → re-open with next backoff step"
                                );
                            } else if cb.failures_window.len() >= CB_ERROR_LIMIT
                                && cb.opened_at.is_none()
                            {
                                cb.opened_at = Some(now);
                                cb.half_open_pending = false;
                                cb.open_count = cb.open_count.saturating_add(1);
                                cb.opened_by_rate_limit = cause_class == "rate_limit";
                                e.set_state(ProviderState::Open);
                                warn!(
                                    event = "rpc_pool.circuit_opened",
                                    provider = %e.name,
                                    failures = cb.failures_window.len(),
                                    cause = "health_check_failed",
                                    rate_limit = cb.opened_by_rate_limit,
                                    cooldown_secs = effective_cooldown(&cb).as_secs(),
                                    "circuit breaker tripped from health checks"
                                );
                            }
                        }
                        Err(_) => {
                            crate::metrics::RPC_PROVIDER_ERRORS_TOTAL
                                .with_label_values(&[e.name.as_str(), "http", "timeout"])
                                .inc();
                        }
                    }
                }

                // Drift detection: compare each provider against max_block.
                if max_block > 0 {
                    for e in &entries {
                        let bn = e.last_block.load(Ordering::Relaxed);
                        if max_block.saturating_sub(bn) >= DRIFT_THRESHOLD_BLOCKS {
                            let mut cb = e.circuit.write().await;
                            let now = Instant::now();
                            let first_seen = *cb.drift_first_seen.get_or_insert(now);
                            if now.duration_since(first_seen) >= DRIFT_PERSIST
                                && e.snapshot_state() == ProviderState::Healthy
                            {
                                e.set_state(ProviderState::Degraded);
                                crate::metrics::RPC_POOL_DRIFT_DETECTED_TOTAL
                                    .with_label_values(&[&e.name, &chain_id.to_string()])
                                    .inc();
                                warn!(
                                    event = "rpc_pool.drift_detected",
                                    provider = %e.name,
                                    chain_id,
                                    behind_blocks = max_block - bn,
                                    "marking provider Degraded — fell behind tip for >{}s",
                                    DRIFT_PERSIST.as_secs()
                                );
                            }
                        } else {
                            // Caught up — reset drift timer + consider recovery to Healthy.
                            let mut cb = e.circuit.write().await;
                            cb.drift_first_seen = None;
                            if e.snapshot_state() == ProviderState::Degraded
                                && cb.opened_at.is_none()
                                && !cb.half_open_pending
                            {
                                e.set_state(ProviderState::Healthy);
                                info!(
                                    event = "rpc_pool.recovered",
                                    provider = %e.name,
                                    "drift cleared and circuit closed → healthy"
                                );
                            }
                        }
                    }
                }
            }
        })
    }

    /// Snapshot for /health responses or admin pages.
    pub fn snapshot(&self) -> RpcPoolSnapshot {
        RpcPoolSnapshot {
            chain_id: self.chain_id,
            providers: self
                .entries
                .iter()
                .map(|e| RpcProviderSnapshot {
                    name: e.name.clone(),
                    state: e.snapshot_state().as_str().to_string(),
                    last_block: e.snapshot_block(),
                    latency_ms_ewma: e.snapshot_latency_ms(),
                })
                .collect(),
        }
    }
}

// ---------- WS list (light pool — caller drives reconnect) ----------

#[derive(Debug, Clone)]
pub struct WsEndpoint {
    pub name: String,
    pub url: String,
}

pub struct WsRpcPool {
    pub chain_id: u64,
    pub endpoints: Vec<WsEndpoint>,
}

impl WsRpcPool {
    /// Read `RPC_WS_<chain_id>` env. Returns `Ok(None)` if absent/empty.
    pub fn from_env(chain_id: u64) -> Result<Option<Self>, PoolError> {
        let key = format!("RPC_WS_{chain_id}");
        let raw = std::env::var(&key).unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(None);
        }
        let parsed = parse_csv(&raw)?;
        if parsed.is_empty() {
            return Err(PoolError::Empty(chain_id));
        }
        let endpoints: Vec<WsEndpoint> = parsed
            .into_iter()
            .map(|(name, url)| WsEndpoint { name, url })
            .collect();
        if endpoints.len() == 1 {
            warn!(
                event = "rpc_pool.single_vendor_ws",
                chain_id,
                name = %endpoints[0].name,
                "only ONE WS RPC provider for this chain — failover impossible. \
                 Add a second vendor in env RPC_WS_{chain_id}."
            );
        } else {
            info!(
                event = "rpc_pool.ws_ready",
                chain_id,
                count = endpoints.len(),
                "ws rpc pool initialized"
            );
        }
        for e in &endpoints {
            crate::metrics::RPC_PROVIDER_STATE
                .with_label_values(&[e.name.as_str(), "ws"])
                .set(ProviderState::Healthy as u8 as i64);
        }
        Ok(Some(Self {
            chain_id,
            endpoints,
        }))
    }
}

// ---------- snapshot for /health JSON ----------

#[derive(Debug, Clone, serde::Serialize)]
pub struct RpcPoolSnapshot {
    pub chain_id: u64,
    pub providers: Vec<RpcProviderSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RpcProviderSnapshot {
    pub name: String,
    pub state: String,
    pub last_block: u64,
    pub latency_ms_ewma: u64,
}

// ---------- helpers ----------

/// Parse a CSV of providers. Each token is either `name=url` or a bare URL
/// (which is implicitly named `primary`, `secondary`, ...). Whitespace around
/// commas/equals is trimmed. Empty tokens are skipped silently.
///
/// MC-RPC-1: malformed tokens (empty side of `name=`, unsupported scheme) are
/// SKIPPED with a warn instead of failing the whole pool — one typo in a
/// 10-provider CSV used to kill failover entirely (from_csv Err → supervisor
/// fell through to env → pool None). Duplicate names get a `-2`, `-3`, …
/// suffix so Prometheus labels and logs stay unambiguous. The returned vec is
/// empty when nothing parsed; callers turn that into `PoolError::Empty`.
/// Warns never include the URL (API keys live there — redacted-logger doctrine).
pub fn parse_csv(raw: &str) -> Result<Vec<(String, String)>, PoolError> {
    let mut out: Vec<(String, String)> = Vec::new();
    let fallback_names = ["primary", "secondary", "tertiary", "quaternary"];
    let mut bare_idx = 0usize;
    for tok in raw.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        if let Some((n, u)) = tok.split_once('=') {
            let name = n.trim().to_string();
            let url = u.trim().to_string();
            if name.is_empty() || url.is_empty() {
                warn!(
                    event = "rpc_pool.csv_token_skipped",
                    name = "(unnamed)",
                    reason = "empty side of name=url token"
                );
                continue;
            }
            if !is_supported_scheme(&url) {
                warn!(
                    event = "rpc_pool.csv_token_skipped",
                    name = name.as_str(),
                    reason = "unsupported scheme"
                );
                continue;
            }
            push_uniquely_named(&mut out, name, url);
        } else {
            // bare URL
            if !is_supported_scheme(tok) {
                warn!(
                    event = "rpc_pool.csv_token_skipped",
                    name = "(bare)",
                    reason = "unsupported scheme"
                );
                continue;
            }
            let name = fallback_names
                .get(bare_idx)
                .copied()
                .unwrap_or("extra")
                .to_string();
            bare_idx += 1;
            push_uniquely_named(&mut out, name, tok.to_string());
        }
    }
    Ok(out)
}

fn is_supported_scheme(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("ws://")
        || url.starts_with("wss://")
}

/// Append `(name, url)`, suffixing `-2`, `-3`, … on duplicate names so
/// provider labels (Prometheus metrics, health logs) stay unambiguous when
/// the operator reuses a placeholder name (e.g. `otro=` twice in one CSV).
fn push_uniquely_named(out: &mut Vec<(String, String)>, name: String, url: String) {
    if !out.iter().any(|(n, _)| *n == name) {
        out.push((name, url));
        return;
    }
    let mut i = 2usize;
    while out.iter().any(|(n, _)| *n == format!("{name}-{i}")) {
        i += 1;
    }
    out.push((format!("{name}-{i}"), url));
}

/// ARBX-R-0003: cooldown before the breaker may half-open. Base `CB_OPEN_DURATION`
/// (30s); a rate-limit opening floors it at `RATE_LIMIT_MIN_COOLDOWN` (120s);
/// each reopen doubles it (`open_count − 1` exponent), capped at
/// `REOPEN_BACKOFF_CAP`. Exponential backoff against the open/close/reopen
/// hammer loop of the 429-storm incident.
pub fn effective_cooldown(cb: &CircuitState) -> Duration {
    let base = if cb.opened_by_rate_limit {
        RATE_LIMIT_MIN_COOLDOWN
    } else {
        CB_OPEN_DURATION
    };
    let exp = cb.open_count.saturating_sub(1).min(3);
    let ms = base.as_millis() as u64;
    let scaled = ms.saturating_mul(1u64 << exp);
    Duration::from_millis(scaled.min(REOPEN_BACKOFF_CAP.as_millis() as u64))
}

/// ARBX-R-0003: `with_retry`'s inter-attempt backoff — a rate-limit-class
/// first failure backs off before trying the backup; every other class
/// retries immediately (the historical behavior).
pub fn retry_backoff_for(cause: &str) -> Duration {
    if classify_cause(cause) == "rate_limit" {
        RATE_LIMIT_RETRY_BACKOFF
    } else {
        Duration::ZERO
    }
}

/// ARBX-R-0003: does this (Healthy) entry still carry the recent
/// rate-limit sticky penalty in `pick()`? `try_read` — `pick` is sync and a
/// contended lock simply means "no penalty this pick" (the historical
/// latency-only order), never a block.
pub fn is_rate_limit_sticky(entry: &HttpEntry, now: Instant) -> bool {
    entry
        .circuit
        .try_read()
        .map(|cb| {
            cb.last_rate_limit_at
                .is_some_and(|t| now.duration_since(t) < RATE_LIMIT_STICKY)
        })
        .unwrap_or(false)
}

/// ARBX-R-0003: is this entry's pending half-open probe a LOAD probe? True
/// only when the breaker opened on rate-limit — a cheap blockNumber probe
/// proved nothing about eth_call capacity in the 429-storm incident
/// (health-pass ≠ load-ok), so the reopening decision goes through a real
/// eth_call instead. Callers hold the read lock already; the flag is plain
/// data, so this stays a free function over `&CircuitState`.
pub fn half_open_probe_is_load(cb: &CircuitState) -> bool {
    cb.half_open_pending && cb.opened_by_rate_limit
}

/// ARBX-R-0003: the load-probe transaction — a read-only `eth_call` of
/// `Multicall3.aggregate3([])` (`0x82ad56cb` + no args), the cheapest call
/// that still exercises the provider's eth_call quota the same way the quoter
/// path does. Multicall3 lives at the same CREATE2 address on every chain the
/// pool serves (universal protocol constant, not config), so the probe needs
/// no per-chain setup.
fn load_probe_transaction() -> alloy::rpc::types::TransactionRequest {
    alloy::rpc::types::TransactionRequest::default()
        .to(MULTICALL3_ADDRESS)
        .input(alloy::primitives::Bytes::from(LOAD_PROBE_CALLDATA).into())
}

fn classify_cause(msg: &str) -> &'static str {
    let m = msg.to_ascii_lowercase();
    if m.contains("timeout") || m.contains("timed out") {
        "timeout"
    } else if m.contains("rate") || m.contains("429") || m.contains("too many requests") {
        // R-0003: the incident messages were `HTTP status client error
        // (429 Too Many Requests)` — which contains NEITHER "rate" nor any
        // other bucket marker, so real 429s used to classify as "other" and
        // every rate-limit policy silently no-opped. Match the status code
        // and the reason phrase explicitly.
        "rate_limit"
    } else if m.contains("connection") || m.contains("connect") {
        "connection"
    } else if m.contains("decode") || m.contains("parse") {
        "decode"
    } else {
        "other"
    }
}

/// PIEZA B (svc_cred FASE 4): a credential-class failure (401/403/429/quota)
/// is NOT transient — the circuit breaker below mutes the provider, but the
/// only real fix is rotating its API key (titular→fallback, `cred_rotation`).
/// Emit the signal so credential consumers pick it up. Gated on not-Open so a
/// hard-down provider does not flood the log (R9); the cause string itself is
/// never logged (it can embed the provider URL — redacted-logger doctrine).
fn emit_rotation_needed_if_credential_error(entry: &HttpEntry, cause: &str) {
    if entry.snapshot_state() == ProviderState::Open {
        return; // breaker already muted this provider — the signal was sent.
    }
    if let Some(reason) = crate::cred_rotation::credential_error_reason(cause) {
        warn!(
            event = "credential.rotation_needed",
            provider = %entry.name,
            reason,
            "credential-class failure — provider API key must rotate (titular→fallback, svc_cred FASE 4)"
        );
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_named_csv() {
        let v = parse_csv("alchemy=https://a/v2/k,infura=https://i/v3/k").unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].0, "alchemy");
        assert_eq!(v[1].0, "infura");
        assert!(v[0].1.starts_with("https://a"));
    }

    #[test]
    fn parse_bare_url_falls_back_to_primary() {
        let v = parse_csv("https://only-one/v2/k").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "primary");
    }

    #[test]
    fn parse_mixed_named_and_bare() {
        let v = parse_csv("https://a/v2/k, infura=https://i/v3/k").unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].0, "primary");
        assert_eq!(v[1].0, "infura");
    }

    #[test]
    fn parse_empty_tokens_skipped() {
        let v = parse_csv(",, alchemy=https://a/v2/k ,, ").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "alchemy");
    }

    #[test]
    fn parse_skips_unsupported_scheme_and_keeps_rest() {
        let v = parse_csv("alchemy=ftp://nope,infura=https://i/v3/k").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "infura");
    }

    #[test]
    fn parse_skips_empty_side_tokens() {
        let v = parse_csv("=https://nope,alchemy=,drpc=https://d").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "drpc");
    }

    #[test]
    fn parse_all_malformed_yields_empty_vec() {
        // Callers map an empty vec to PoolError::Empty — the pool dies only
        // when NOTHING parses, not when one token is a typo (MC-RPC-1).
        assert!(parse_csv("alchemy=, = ,ftp://x").unwrap().is_empty());
    }

    #[test]
    fn parse_uniquifies_duplicate_names() {
        let v = parse_csv("otro=https://a/v2/k,otro=https://b/eth,drpc=https://d").unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].0, "otro");
        assert_eq!(v[1].0, "otro-2");
        assert_eq!(v[2].0, "drpc");
        assert!(v[1].1.starts_with("https://b"));
    }

    #[test]
    fn parse_csv_supports_ws_scheme() {
        let v = parse_csv("alchemy=wss://a/v2/k,infura=ws://i/v3/k").unwrap();
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn classify_cause_buckets() {
        assert_eq!(classify_cause("connection refused"), "connection");
        assert_eq!(classify_cause("operation timeout"), "timeout");
        assert_eq!(classify_cause("429 rate limit"), "rate_limit");
        assert_eq!(classify_cause("decode error: unexpected"), "decode");
        assert_eq!(classify_cause("something weird"), "other");
    }

    #[test]
    fn provider_state_round_trip() {
        for s in [
            ProviderState::Healthy,
            ProviderState::Degraded,
            ProviderState::Open,
        ] {
            assert_eq!(ProviderState::from_u8(s as u8), s);
        }
    }

    fn dummy_entry(name: &str) -> Arc<HttpEntry> {
        // Provider that won't actually be used in these tests — we only inspect
        // the metadata. We pass a syntactically-valid URL; alloy ProviderBuilder
        // accepts it without a live connection (no boot check here in tests).
        let url: reqwest::Url = "http://127.0.0.1:1".parse().unwrap();
        let provider = Arc::new(
            alloy::providers::ProviderBuilder::new()
                .disable_recommended_fillers()
                .connect_http(url),
        );
        Arc::new(HttpEntry {
            name: name.to_string(),
            url: "http://127.0.0.1:1".into(),
            provider,
            state: AtomicU8::new(ProviderState::Healthy as u8),
            last_block: AtomicU64::new(0),
            latency_ms_ewma: AtomicU64::new(0),
            circuit: RwLock::new(CircuitState::default()),
        })
    }

    #[tokio::test]
    async fn pick_prefers_lowest_latency_healthy() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a"), dummy_entry("b")],
        };
        pool.entries[0]
            .latency_ms_ewma
            .store(200, Ordering::Relaxed);
        pool.entries[1].latency_ms_ewma.store(50, Ordering::Relaxed);
        let picked = pool.pick().unwrap();
        assert_eq!(picked.name, "b");
    }

    #[tokio::test]
    async fn pick_falls_back_to_degraded_when_no_healthy() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a"), dummy_entry("b")],
        };
        pool.entries[0].set_state(ProviderState::Open);
        pool.entries[1].set_state(ProviderState::Degraded);
        let picked = pool.pick().unwrap();
        assert_eq!(picked.name, "b");
    }

    #[tokio::test]
    async fn pick_returns_all_unhealthy_when_only_open() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a"), dummy_entry("b")],
        };
        pool.entries[0].set_state(ProviderState::Open);
        pool.entries[1].set_state(ProviderState::Open);
        let err = pool.pick().unwrap_err();
        assert!(matches!(err, PoolError::AllUnhealthy(1)));
    }

    #[tokio::test]
    async fn report_failure_trips_breaker_at_limit() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a")],
        };
        let e = pool.entries[0].clone();
        for _ in 0..CB_ERROR_LIMIT {
            pool.report_failure(&e, "boom").await;
        }
        assert_eq!(e.snapshot_state(), ProviderState::Open);
        let cb = e.circuit.read().await;
        assert!(cb.opened_at.is_some());
    }

    #[tokio::test]
    async fn report_failure_credential_error_still_trips_breaker() {
        // PIEZA B smoke: a 401 goes through the new credential-classification
        // path (emitting `credential.rotation_needed`) without disturbing the
        // breaker's failure accounting.
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a")],
        };
        let e = pool.entries[0].clone();
        for _ in 0..CB_ERROR_LIMIT {
            pool.report_failure(&e, "HTTP status client error (401 Unauthorized)")
                .await;
        }
        assert_eq!(e.snapshot_state(), ProviderState::Open);
    }

    #[tokio::test]
    async fn report_success_after_half_open_closes_circuit() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a")],
        };
        let e = pool.entries[0].clone();
        // simulate Open + half-open pending
        e.set_state(ProviderState::Open);
        {
            let mut cb = e.circuit.write().await;
            cb.opened_at = Some(Instant::now());
            cb.half_open_pending = true;
        }
        pool.report_success(&e, Duration::from_millis(10)).await;
        assert_eq!(e.snapshot_state(), ProviderState::Healthy);
    }

    #[tokio::test]
    async fn ewma_first_sample_takes_full_value() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a")],
        };
        let e = pool.entries[0].clone();
        pool.report_success(&e, Duration::from_millis(123)).await;
        assert_eq!(e.snapshot_latency_ms(), 123);
    }

    // ---------- ARBX-R-0003: 429-storm mitigation ----------

    /// `Instant` that is `secs` in the past, for expiry tests.
    fn instant_ago(secs: u64) -> Instant {
        Instant::now()
            .checked_sub(Duration::from_secs(secs))
            .expect("clock rewind within uptime")
    }

    #[test]
    fn effective_cooldown_floors_and_backs_off_rate_limit_openings() {
        // Plain opening: historical 30s.
        let mut cb = CircuitState {
            open_count: 1,
            ..Default::default()
        };
        assert_eq!(effective_cooldown(&cb), CB_OPEN_DURATION);

        // Rate-limit opening: floored at 120s even on the first trip.
        cb.opened_by_rate_limit = true;
        assert_eq!(effective_cooldown(&cb), RATE_LIMIT_MIN_COOLDOWN);

        // Each reopen doubles: 240s, 480s, then capped at 600s (960 → cap).
        cb.open_count = 2;
        assert_eq!(effective_cooldown(&cb), Duration::from_secs(240));
        cb.open_count = 3;
        assert_eq!(effective_cooldown(&cb), Duration::from_secs(480));
        cb.open_count = 4;
        assert_eq!(effective_cooldown(&cb), REOPEN_BACKOFF_CAP);
        cb.open_count = 99;
        assert_eq!(effective_cooldown(&cb), REOPEN_BACKOFF_CAP);

        // Non-rate-limit reopens also back off, just from the 30s base.
        cb.opened_by_rate_limit = false;
        cb.open_count = 2;
        assert_eq!(effective_cooldown(&cb), Duration::from_secs(60));
    }

    #[test]
    fn retry_backoff_only_for_rate_limit_class() {
        assert_eq!(
            retry_backoff_for("HTTP status client error (429 Too Many Requests)"),
            RATE_LIMIT_RETRY_BACKOFF
        );
        assert_eq!(retry_backoff_for("request timed out"), Duration::ZERO);
        assert_eq!(retry_backoff_for("connection reset"), Duration::ZERO);
        assert_eq!(retry_backoff_for("boom"), Duration::ZERO);
    }

    #[test]
    fn half_open_probe_is_load_only_for_rate_limit_half_open() {
        let mut cb = CircuitState::default();
        assert!(!half_open_probe_is_load(&cb));
        cb.half_open_pending = true;
        assert!(
            !half_open_probe_is_load(&cb),
            "non-RL half-open probes blockNumber"
        );
        cb.half_open_pending = false;
        cb.opened_by_rate_limit = true;
        assert!(
            !half_open_probe_is_load(&cb),
            "RL but not half-open has no probe yet"
        );
        cb.half_open_pending = true;
        assert!(half_open_probe_is_load(&cb));
    }

    #[tokio::test]
    async fn pick_demotes_recently_rate_limited_behind_clean() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("sticky-fast"), dummy_entry("clean-slow")],
        };
        // The sticky one is FASTER — the penalty must still lose to cleanliness.
        pool.entries[0].latency_ms_ewma.store(10, Ordering::Relaxed);
        pool.entries[1]
            .latency_ms_ewma
            .store(500, Ordering::Relaxed);
        pool.entries[0].circuit.write().await.last_rate_limit_at = Some(Instant::now());
        let picked = pool.pick().unwrap();
        assert_eq!(picked.name, "clean-slow");
    }

    #[tokio::test]
    async fn pick_all_sticky_falls_back_to_latency_order() {
        // No thundering-herd cliff: when EVERY healthy entry is sticky, the
        // tier demotes to none and selection degrades to the historical
        // best-latency order — sticky only reorders, never excludes.
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a"), dummy_entry("b")],
        };
        pool.entries[0]
            .latency_ms_ewma
            .store(200, Ordering::Relaxed);
        pool.entries[1].latency_ms_ewma.store(50, Ordering::Relaxed);
        for e in &pool.entries {
            e.circuit.write().await.last_rate_limit_at = Some(Instant::now());
        }
        let picked = pool.pick().unwrap();
        assert_eq!(picked.name, "b");
    }

    #[tokio::test]
    async fn sticky_penalty_expires_after_window() {
        let e = dummy_entry("a");
        e.circuit.write().await.last_rate_limit_at =
            Some(instant_ago(RATE_LIMIT_STICKY.as_secs() + 1));
        assert!(!is_rate_limit_sticky(&e, Instant::now()));
        e.circuit.write().await.last_rate_limit_at = Some(Instant::now());
        assert!(is_rate_limit_sticky(&e, Instant::now()));
    }

    #[tokio::test]
    async fn report_failure_rate_limit_sets_sticky_and_trip_flags() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a")],
        };
        let e = pool.entries[0].clone();
        for _ in 0..CB_ERROR_LIMIT {
            pool.report_failure(&e, "HTTP status client error (429 Too Many Requests)")
                .await;
        }
        assert_eq!(e.snapshot_state(), ProviderState::Open);
        let cb = e.circuit.read().await;
        assert!(cb.opened_by_rate_limit, "trip flags the rate-limit opening");
        assert_eq!(cb.open_count, 1, "first trip = first cooldown step");
        assert!(
            cb.last_rate_limit_at.is_some(),
            "sticky mark set on the request path"
        );
        // The very same cooldown that will gate the half-open rotation.
        assert_eq!(effective_cooldown(&cb), RATE_LIMIT_MIN_COOLDOWN);
        drop(cb);
    }

    #[tokio::test]
    async fn report_failure_non_rate_limit_keeps_base_cooldown() {
        let pool = HttpRpcPool {
            chain_id: 1,
            entries: vec![dummy_entry("a")],
        };
        let e = pool.entries[0].clone();
        for _ in 0..CB_ERROR_LIMIT {
            pool.report_failure(&e, "connection reset by peer").await;
        }
        let cb = e.circuit.read().await;
        assert!(!cb.opened_by_rate_limit);
        assert_eq!(effective_cooldown(&cb), CB_OPEN_DURATION);
    }
}
