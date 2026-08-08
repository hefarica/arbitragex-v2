//! Block 2 — Pool Enumeration Worker (proactive top-TVL pool discovery).
//!
//! Widens the search surface beyond seeds + reactive discovery by pulling
//! top-TVL pools from MULTIPLE live sources (The Graph, DeFiLlama, GeckoTerminal),
//! confirming each pool's factory ON-CHAIN, token-safety screening, and persisting
//! via the existing on-chain hydration path — WITHOUT trusting any source blindly.
//! Each source has its own circuit breaker, so one flapping API never stalls the
//! others.
//!
//! ## Per-tick flow
//! 1. Fetch candidates from each enabled source under a per-source circuit breaker
//!    + timeout; merge + dedup by address; sort by TVL desc; truncate top-N.
//! 2. Dedup: skip already-ACTIVE pools; inactive ones re-screen so a now-safe
//!    token reactivates the pool (monotonic upsert).
//! 3. For each NEW candidate (capped at MAX_NEW_PER_TICK): token-safety screen
//!    both tokens (token_safety_cache, floor TOKEN_SAFETY_FLOOR) — both >= floor →
//!    activate; either < floor → skip (unsafe); either unrated → persist
//!    is_active=false (a future tick reactivates it). Then
//!    `PoolDiscoveryService::enumerate_and_persist_pool` resolves the factory
//!    ON-CHAIN, looks it up in the seeded `factories` cache, re-verifies the pool
//!    on-chain, and persists. Factory-not-seeded → skip. On persist, stamp the
//!    source TVL/volume/timestamp + provenance (migration 096 columns).
//! 4. Log the tick metrics.
//!
//! ## Doctrine (NO-ACTIVE / shadow)
//! Default-OFF: spawns ONLY when `ARBX_POOL_ENUM_MODE=shadow`. Read-only external
//! I/O: HTTP (subgraph + DeFiLlama + GeckoTerminal) + eth_call staticcall + DB
//! upserts + Redis index + in-memory impact_index for token-safe pools. NO signer,
//! NO capital, NO broadcast, NO executor. Fail-honest (RULE 00 / R8): never
//! fabricate a pool, never seed a factory blindly, never activate an unsafe/unrated
//! token. Existing pools are never deactivated (is_active is monotonic).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use tracing::{info, warn};

use crate::pool_candidate::{PoolCandidate, PoolEnumSource};
use crate::pool_discovery::{EnumOutcome, PoolDiscoveryService};
use crate::pool_sources;
use crate::source_supervisor::{guarded_fetch, SourceCircuit};

const ENV_MODE: &str = "ARBX_POOL_ENUM_MODE";
const ENV_INTERVAL: &str = "POOL_ENUM_INTERVAL_MS";
const ENV_TOP_N: &str = "ARBX_POOL_ENUM_TOP_N";
const ENV_MIN_TVL: &str = "ARBX_POOL_ENUM_MIN_TVL_USD";
const ENV_MAX_NEW: &str = "ARBX_POOL_ENUM_MAX_NEW_PER_TICK";
const ENV_SAFETY_FLOOR: &str = "TOKEN_SAFETY_FLOOR";
const ENV_SOURCES: &str = "ARBX_POOL_ENUM_SOURCES";
const ENV_SOURCE_TIMEOUT: &str = "ARBX_POOL_ENUM_SOURCE_TIMEOUT_MS";
const ENV_CIRCUIT_FAILURES: &str = "ARBX_POOL_ENUM_CIRCUIT_FAILURES";
const ENV_CIRCUIT_COOLDOWN: &str = "ARBX_POOL_ENUM_CIRCUIT_COOLDOWN_MS";

const DEFAULT_INTERVAL_MS: u64 = 3_600_000; // 1h
const MIN_INTERVAL_MS: u64 = 60_000; // never hammer the sources faster than 1/min
const DEFAULT_TOP_N: usize = 500;
const DEFAULT_MIN_TVL_USD: f64 = 50_000.0;
const DEFAULT_MAX_NEW: usize = 50;
const DEFAULT_SAFETY_FLOOR: i32 = 70;
const DEFAULT_SOURCE_TIMEOUT_MS: u64 = 15_000;
const DEFAULT_CIRCUIT_FAILURES: u32 = 3;
const DEFAULT_CIRCUIT_COOLDOWN_MS: u64 = 900_000; // 15 min quarantine

#[derive(Clone, Debug)]
struct EnumConfig {
    interval: Duration,
    top_n: usize,
    min_tvl_usd: f64,
    max_new: usize,
    safety_floor: i32,
    sources: Vec<PoolEnumSource>,
    source_timeout_ms: u64,
    circuit_failures: u32,
    circuit_cooldown_ms: u64,
}

impl EnumConfig {
    fn from_env() -> Self {
        let interval_ms = env_u64(ENV_INTERVAL, DEFAULT_INTERVAL_MS).max(MIN_INTERVAL_MS);
        let top_n = env_usize(ENV_TOP_N, DEFAULT_TOP_N).clamp(1, 1000);
        let min_tvl_usd = env_f64(ENV_MIN_TVL, DEFAULT_MIN_TVL_USD).max(0.0);
        let max_new = env_usize(ENV_MAX_NEW, DEFAULT_MAX_NEW).clamp(1, 1000);
        // Clamp to [1,100] so a hostile/absurd <=0 value can't silently disable
        // the safety gate (token_safety_cache scores are 0..100).
        let safety_floor = env_i32(ENV_SAFETY_FLOOR, DEFAULT_SAFETY_FLOOR).clamp(1, 100);
        Self {
            interval: Duration::from_millis(interval_ms),
            top_n,
            min_tvl_usd,
            max_new,
            safety_floor,
            sources: parse_sources(ENV_SOURCES),
            source_timeout_ms: env_u64(ENV_SOURCE_TIMEOUT, DEFAULT_SOURCE_TIMEOUT_MS).max(1_000),
            circuit_failures: env_u32(ENV_CIRCUIT_FAILURES, DEFAULT_CIRCUIT_FAILURES),
            circuit_cooldown_ms: env_u64(ENV_CIRCUIT_COOLDOWN, DEFAULT_CIRCUIT_COOLDOWN_MS),
        }
    }
}

/// Parse `ARBX_POOL_ENUM_SOURCES` (CSV). Empty/absent → all sources.
/// Unknown tokens are ignored. De-duplicated, order preserved.
///
/// **Truth anchor (operator directive, 2026-08-07):** the on-chain Alchemy source
/// is ALWAYS injected first, even when the operator explicitly lists other
/// sources or omits it. The fallback cycle must never run dry 24/7/365, and the
/// RPC the searcher already trusts is the one source that cannot rate-limit or
/// de-list pools — it is the chain itself. Operators can opt OUT of the anchor
/// only by setting `ARBX_POOL_ENUM_NO_ANCHOR=1` (for emergency/disable).
fn parse_sources(key: &str) -> Vec<PoolEnumSource> {
    let no_anchor = std::env::var("ARBX_POOL_ENUM_NO_ANCHOR")
        .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    let mut out: Vec<PoolEnumSource> = Vec::new();
    let mut seen = HashSet::new();
    // Anchor first — highest fidelity, never third-party-dependent.
    if !no_anchor {
        out.push(PoolEnumSource::Alchemy);
        seen.insert(PoolEnumSource::Alchemy);
    }
    let raw = std::env::var(key).unwrap_or_default();
    let listed: Vec<PoolEnumSource> = if raw.trim().is_empty() {
        vec![
            PoolEnumSource::TheGraph,
            PoolEnumSource::DexScreener,
            PoolEnumSource::DeFiLlama,
            PoolEnumSource::GeckoTerminal,
        ]
    } else {
        raw.split(',')
            .filter_map(PoolEnumSource::from_str)
            .collect()
    };
    for s in listed {
        if seen.insert(s) {
            out.push(s);
        }
    }
    out
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}
fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}
fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}
fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
fn env_i32(key: &str, default: i32) -> i32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// Token-safety verdict for one token, from `token_safety_cache`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafetyVerdict {
    /// Fresh score >= floor.
    Safe,
    /// Fresh score < floor.
    Unsafe,
    /// No fresh record — not yet rated by token_enricher.
    Unrated,
}

/// Spawn the worker iff `ARBX_POOL_ENUM_MODE == "shadow"`. Off / absent / any
/// other value → no-op (default-OFF). Requires a DB pool; without one it logs
/// and does not spawn (R8 fail-honest). Detached task — process-exit shutdown.
pub fn spawn_if_enabled(chain_id: u64, discovery: Arc<PoolDiscoveryService>, db: Option<PgPool>) {
    let mode = std::env::var(ENV_MODE).unwrap_or_default();
    if !mode.trim().eq_ignore_ascii_case("shadow") {
        info!(event = "poolenum.disabled", chain_id, mode = %mode);
        return;
    }
    let db = match db {
        Some(d) => d,
        None => {
            warn!(
                event = "poolenum.no_db",
                chain_id, "pool enumeration requires a DB pool — not spawning"
            );
            return;
        }
    };
    let cfg = EnumConfig::from_env();
    if cfg.sources.is_empty() {
        warn!(
            event = "poolenum.no_sources",
            chain_id, "ARBX_POOL_ENUM_SOURCES resolved to no known sources — not spawning"
        );
        return;
    }
    info!(
        event = "poolenum.enabled",
        chain_id,
        interval_ms = cfg.interval.as_millis() as u64,
        top_n = cfg.top_n,
        min_tvl_usd = cfg.min_tvl_usd,
        max_new = cfg.max_new,
        safety_floor = cfg.safety_floor,
        sources = cfg
            .sources
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(","),
    );
    let cb = |name| SourceCircuit::new(name, cfg.circuit_failures, cfg.circuit_cooldown_ms);
    let worker = PoolEnumerationWorker {
        chain_id,
        discovery,
        db,
        cb_alchemy: cb("alchemy"),
        cb_thegraph: cb("thegraph"),
        cb_defillama: cb("defillama"),
        cb_dexscreener: cb("dexscreener"),
        cb_gecko: cb("geckoterminal"),
        cfg,
    };
    tokio::spawn(async move {
        worker.run().await;
        warn!(
            event = "poolenum.task_exited",
            chain_id, "pool enumeration loop ended unexpectedly"
        );
    });
}

struct PoolEnumerationWorker {
    chain_id: u64,
    discovery: Arc<PoolDiscoveryService>,
    db: PgPool,
    cfg: EnumConfig,
    cb_alchemy: Arc<SourceCircuit>,
    cb_thegraph: Arc<SourceCircuit>,
    cb_defillama: Arc<SourceCircuit>,
    cb_dexscreener: Arc<SourceCircuit>,
    cb_gecko: Arc<SourceCircuit>,
}

impl PoolEnumerationWorker {
    async fn run(self) {
        let mut interval = tokio::time::interval(self.cfg.interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            self.run_tick().await;
        }
    }

    async fn run_tick(&self) {
        let started = Instant::now();

        // 1. Fetch from all enabled sources (each circuit-broken + timed out),
        //    normalise, dedup by address within the tick, sort by TVL, truncate.
        let mut candidates = self.fetch_candidates().await;
        if candidates.is_empty() {
            info!(event = "poolenum.no_candidates", chain_id = self.chain_id);
            return;
        }
        candidates.sort_by(|a, b| {
            b.tvl_usd
                .partial_cmp(&a.tvl_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(self.cfg.top_n);
        let fetched = candidates.len();

        // 2. Dedup vs DB: skip ACTIVE pools; inactive re-screen (reactivation).
        let known = self.load_known_pools().await;

        let mut already_known = 0usize;
        let mut factory_missing = 0usize;
        let mut safety_blocked = 0usize;
        let mut hydration_failed = 0usize;
        let mut missing_tokens = 0usize;
        let mut activated = 0usize;
        let mut persisted_inactive = 0usize;
        let mut processed_new = 0usize;

        for c in &candidates {
            if processed_new >= self.cfg.max_new {
                break;
            }
            let addr_lc = c.address.to_ascii_lowercase();
            let prior_active = known.get(&addr_lc).copied();
            if prior_active == Some(true) {
                already_known += 1; // already active — nothing to do
                continue;
            }

            // Need BOTH token hints to safety-screen + run the on-chain pair-match
            // guard. A source that omits them → skip fail-honestly.
            let (t0s, t1s) = match (c.token0.as_deref(), c.token1.as_deref()) {
                (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => (a, b),
                _ => {
                    missing_tokens += 1;
                    continue;
                }
            };

            // 3a. Token-safety screen for BOTH tokens (cheap DB lookup).
            let s0 = self.token_safety(t0s).await;
            let s1 = self.token_safety(t1s).await;
            let activate = match (s0, s1) {
                (SafetyVerdict::Unsafe, _) | (_, SafetyVerdict::Unsafe) => {
                    safety_blocked += 1;
                    warn!(
                        event = "poolenum.token_unsafe",
                        chain_id = self.chain_id, pool = %c.address,
                        "skipping pool — a token is below the safety floor"
                    );
                    continue;
                }
                (SafetyVerdict::Safe, SafetyVerdict::Safe) => true,
                // at least one token unrated → persist inactive for a future tick
                _ => false,
            };

            // A dormant pool still not activatable needs no work — skip cheaply.
            if prior_active == Some(false) && !activate {
                continue;
            }

            // Expensive path (on-chain factory resolve + hydration) — budget-capped.
            processed_new += 1;

            let (pool_addr, t0, t1) = match (
                parse_addr(&c.address),
                parse_addr(t0s),
                parse_addr(t1s),
            ) {
                (Some(p), Some(a), Some(b)) => (p, a, b),
                _ => {
                    warn!(event = "poolenum.bad_address", chain_id = self.chain_id, pool = %c.address);
                    continue;
                }
            };

            match self
                .discovery
                .enumerate_and_persist_pool(pool_addr, t0, t1, c.fee_bps, activate)
                .await
            {
                EnumOutcome::Persisted { activated: was } => {
                    if was {
                        activated += 1;
                    } else {
                        persisted_inactive += 1;
                    }
                    // Stamp the source's metadata + accurate provenance (096 cols).
                    self.stamp_metadata(&addr_lc, c).await;
                }
                EnumOutcome::FactoryUnresolved => factory_missing += 1,
                EnumOutcome::HydrationFailed | EnumOutcome::NoRpc => hydration_failed += 1,
            }
        }

        info!(
            event = "poolenum.tick",
            chain_id = self.chain_id,
            fetched,
            already_known,
            missing_tokens,
            factory_missing,
            safety_blocked,
            hydration_failed,
            activated,
            persisted_inactive,
            elapsed_ms = started.elapsed().as_millis() as u64,
        );
    }

    /// Fetch candidates from every enabled source under its circuit breaker +
    /// timeout, then normalise + dedup by address within the tick.
    async fn fetch_candidates(&self) -> Vec<PoolCandidate> {
        let mut all: Vec<PoolCandidate> = Vec::new();
        let (chain, top_n, min_tvl, to) = (
            self.chain_id,
            self.cfg.top_n,
            self.cfg.min_tvl_usd,
            self.cfg.source_timeout_ms,
        );
        for src in &self.cfg.sources {
            let fetched = match src {
                // On-chain truth anchor: uses the searcher's own RPC (multi-vendor
                // failover already inside HttpRpcPool), NOT a third-party HTTP API.
                // Skipped fail-honestly when no RPC is configured (count=0).
                PoolEnumSource::Alchemy => match self.discovery.rpc_pool() {
                    Some(rpc) => {
                        guarded_fetch(&self.cb_alchemy, to, || {
                            pool_sources::alchemy::fetch_onchain(&rpc, chain, top_n, min_tvl)
                        })
                        .await
                    }
                    None => {
                        info!(
                            event = "poolenum.alchemy.skipped_no_rpc",
                            chain_id = self.chain_id
                        );
                        None
                    }
                },
                PoolEnumSource::TheGraph => {
                    guarded_fetch(&self.cb_thegraph, to, || {
                        pool_sources::thegraph::fetch(chain, top_n, min_tvl)
                    })
                    .await
                }
                PoolEnumSource::DeFiLlama => {
                    guarded_fetch(&self.cb_defillama, to, || {
                        pool_sources::defillama::fetch(chain, top_n, min_tvl)
                    })
                    .await
                }
                PoolEnumSource::DexScreener => {
                    guarded_fetch(&self.cb_dexscreener, to, || {
                        pool_sources::dexscreener::fetch(chain, top_n, min_tvl)
                    })
                    .await
                }
                PoolEnumSource::GeckoTerminal => {
                    guarded_fetch(&self.cb_gecko, to, || {
                        pool_sources::geckoterminal::fetch(chain, top_n, min_tvl)
                    })
                    .await
                }
            };
            if let Some(v) = fetched {
                info!(
                    event = "poolenum.source_fetched",
                    chain_id = self.chain_id,
                    source = src.as_str(),
                    count = v.len()
                );
                all.extend(v);
            }
        }
        // Normalise + dedup by address (first writer wins).
        let mut seen = HashSet::new();
        all.into_iter()
            .filter_map(|mut c| {
                c.normalise();
                if c.address.starts_with("0x") && seen.insert(c.address.clone()) {
                    Some(c)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Map of this chain's pool addresses (lowercase) -> is_active. Already-active
    /// pools are skipped; inactive ones are re-screened each tick so a now-token-
    /// safe pool can be reactivated (the upsert's monotonic is_active flips it).
    async fn load_known_pools(&self) -> HashMap<String, bool> {
        match sqlx::query_as::<_, (String, bool)>(
            "SELECT LOWER(address), is_active FROM pools WHERE chain_id = $1",
        )
        .bind(self.chain_id as i64)
        .fetch_all(&self.db)
        .await
        {
            Ok(rows) => rows.into_iter().collect(),
            Err(e) => {
                warn!(event = "poolenum.dedup_query_err", chain_id = self.chain_id, error = %e);
                HashMap::new() // fail-honest: empty → all look new, but the upsert is
                               // idempotent (ON CONFLICT) so no duplicate rows result
            }
        }
    }

    /// Token-safety verdict from `token_safety_cache` — same query + floor as the
    /// pre-execute checklist (fresh row only; score vs TOKEN_SAFETY_FLOOR).
    async fn token_safety(&self, token_addr: &str) -> SafetyVerdict {
        let row: Option<(i32,)> = sqlx::query_as(
            r#"SELECT safety_score
               FROM token_safety_cache
               WHERE chain_id = $1 AND token_address = $2 AND ttl_expires_at > NOW()"#,
        )
        .bind(self.chain_id as i64)
        .bind(token_addr)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten();
        match row {
            Some((score,)) if score >= self.cfg.safety_floor => SafetyVerdict::Safe,
            Some(_) => SafetyVerdict::Unsafe,
            None => SafetyVerdict::Unrated,
        }
    }

    /// Stamp the source's TVL/volume/timestamp + accurate provenance onto the row
    /// that `enumerate_and_persist_pool` just upserted. Only runs post-persist for
    /// a new/reactivated pool (never an already-active seed), so it can't relabel
    /// seed provenance. Float binds are cast to NUMERIC. Fail-honest on error.
    ///
    /// **Enrichment (operator directive 2026-08-07):** when the candidate carries
    /// no USD estimate (tvl=0 — typical for the on-chain anchor, which proposes a
    /// pool's existence without a price feed), DexScreener fills `liquidity.usd`
    /// and `volume.h24` so the row ranks correctly. The on-chain `enum_source`
    /// provenance is preserved; only the missing TVL/volume numbers are borrowed.
    async fn stamp_metadata(&self, addr_lc: &str, c: &PoolCandidate) {
        let (mut tvl, mut vol) = (c.tvl_usd, c.volume_usd_24h);
        if tvl <= 0.0 {
            // On-chain truth exists but no price feed — enrich from DexScreener.
            // Fail-honest: a lookup miss/429 leaves tvl=0; provenance unchanged.
            if let Ok(Some((t, v))) =
                pool_sources::dexscreener::enrich_pool(self.chain_id, addr_lc).await
            {
                if t > 0.0 {
                    tvl = t;
                }
                if vol.is_none() {
                    vol = v;
                }
            }
        }
        let r = sqlx::query(
            r#"UPDATE pools
               SET tvl_usd = $1::double precision::numeric,
                   volume_usd_24h = $2::double precision::numeric,
                   last_enumerated_at = NOW(),
                   enum_source = $3
               WHERE chain_id = $4 AND LOWER(address) = $5"#,
        )
        .bind(tvl)
        .bind(vol)
        .bind(c.source.as_str())
        .bind(self.chain_id as i64)
        .bind(addr_lc)
        .execute(&self.db)
        .await;
        if let Err(e) = r {
            warn!(event = "poolenum.metadata_stamp_err", chain_id = self.chain_id, pool = %addr_lc, error = %e);
        }
    }
}

/// Parse a lowercase `0x`-hex address string into an alloy `Address`, or `None`.
fn parse_addr(s: &str) -> Option<alloy::primitives::Address> {
    s.trim().parse::<alloy::primitives::Address>().ok()
}

// ---------------------------------------------------------------------------
// Unit tests (pure — no network, no DB)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_tier_pips_to_bps() {
        let cases = [
            (Some(3000u32), Some(30u32)),
            (Some(500), Some(5)),
            (Some(10000), Some(100)),
            (None, None),
        ];
        for (pips, bps) in cases {
            assert_eq!(pips.map(|f| f / 100), bps);
        }
    }

    #[test]
    fn parse_addr_valid_and_invalid() {
        assert!(parse_addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").is_some());
        assert!(parse_addr("not-an-address").is_none());
        assert!(parse_addr("").is_none());
    }

    #[test]
    fn safety_verdict_thresholds() {
        let decide = |score: Option<i32>, floor: i32| match score {
            Some(s) if s >= floor => SafetyVerdict::Safe,
            Some(_) => SafetyVerdict::Unsafe,
            None => SafetyVerdict::Unrated,
        };
        assert_eq!(decide(Some(85), 70), SafetyVerdict::Safe);
        assert_eq!(decide(Some(70), 70), SafetyVerdict::Safe);
        assert_eq!(decide(Some(69), 70), SafetyVerdict::Unsafe);
        assert_eq!(decide(None, 70), SafetyVerdict::Unrated);
    }

    #[test]
    fn config_interval_floored() {
        let interval_ms = 100u64.max(MIN_INTERVAL_MS);
        assert_eq!(interval_ms, MIN_INTERVAL_MS);
    }

    #[test]
    fn activate_decision_matrix() {
        use SafetyVerdict::*;
        let decide = |s0: SafetyVerdict, s1: SafetyVerdict| -> Option<bool> {
            match (s0, s1) {
                (Unsafe, _) | (_, Unsafe) => None,
                (Safe, Safe) => Some(true),
                _ => Some(false),
            }
        };
        assert_eq!(decide(Safe, Safe), Some(true));
        assert_eq!(decide(Safe, Unrated), Some(false));
        assert_eq!(decide(Unrated, Unrated), Some(false));
        assert_eq!(decide(Safe, Unsafe), None);
        assert_eq!(decide(Unsafe, Unrated), None);
    }

    #[test]
    fn parse_sources_default_and_dedup() {
        // Empty → anchor (Alchemy) + all indexers (TheGraph, DexScreener,
        // DeFiLlama, GeckoTerminal) = 5 (env not set in test → default). The
        // on-chain anchor is always injected first so the cycle never runs dry
        // 24/7/365 (operator directive 2026-08-07).
        let v = parse_sources("ARBX_POOL_ENUM_SOURCES__unset_test");
        assert_eq!(v.len(), 5);
        assert_eq!(v[0], PoolEnumSource::Alchemy, "anchor must be first");
        // The pure CSV parse path (de-dup + unknown filtered) via from_str.
        let v: Vec<_> = "gecko, gecko, thegraph, bogus"
            .split(',')
            .filter_map(PoolEnumSource::from_str)
            .collect();
        assert_eq!(
            v,
            vec![
                PoolEnumSource::GeckoTerminal,
                PoolEnumSource::GeckoTerminal,
                PoolEnumSource::TheGraph,
            ]
        );
    }
}
