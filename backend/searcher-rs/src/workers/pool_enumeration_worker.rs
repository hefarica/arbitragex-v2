//! Block 2 — Pool Enumeration Worker (proactive top-TVL pool discovery).
//!
//! Today the search surface depends on seed pools + reactive discovery. This
//! worker periodically widens it by pulling top-TVL pools from The Graph
//! (`math_engine::subgraph_client::fetch_top_pools_by_tvl`) and persisting the
//! ones that pass an on-chain factory check AND a token-safety screen — WITHOUT
//! trusting the subgraph blindly.
//!
//! ## Per-tick flow
//! 1. `fetch_top_pools_by_tvl` (V3 primary; V2 best-effort, same endpoint) →
//!    merge → sort by TVL desc → truncate top-N.
//! 2. Dedup: skip already-ACTIVE pools; inactive ones re-screen so a now-safe
//!    token reactivates the pool (monotonic upsert).
//! 3. For each NEW candidate (capped at MAX_NEW_PER_TICK): token-safety screen
//!    both tokens (token_safety_cache, floor TOKEN_SAFETY_FLOOR) — both ≥ floor →
//!    activate; either < floor → skip (unsafe); either unrated → persist
//!    is_active=false (a future tick reactivates it). Then
//!    `PoolDiscoveryService::enumerate_and_persist_pool` resolves the factory
//!    ON-CHAIN, looks it up in the seeded `factories` cache, re-verifies the pool
//!    on-chain, and persists. Factory-not-seeded → skip (observation).
//! 4. Log the tick: fetched / already_known / factory_missing / safety_blocked /
//!    hydration_failed / activated / persisted_inactive / elapsed_ms.
//!
//! ## Doctrine (NO-ACTIVE / shadow)
//! Default-OFF: spawns ONLY when `ARBX_POOL_ENUM_MODE=shadow`. Read-only external
//! I/O: HTTP GraphQL + eth_call staticcall + DB upserts + Redis index + in-memory
//! impact_index for token-safe pools. NO signer, NO capital, NO broadcast, NO
//! executor. Fail-honest (RULE 00 / R8): never fabricate a pool, never seed a
//! factory blindly, never activate an unsafe/unrated token. Does not touch
//! PoolSyncWorker, the scanner's downstream logic, or existing pools (is_active is
//! monotonic — never deactivated by this path).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use tracing::{info, warn};

use crate::pool_discovery::{EnumOutcome, PoolDiscoveryService};
use math_engine::subgraph_client::{self, RankedPool, SubgraphPoolKind};

const ENV_MODE: &str = "ARBX_POOL_ENUM_MODE";
const ENV_INTERVAL: &str = "POOL_ENUM_INTERVAL_MS";
const ENV_TOP_N: &str = "ARBX_POOL_ENUM_TOP_N";
const ENV_MIN_TVL: &str = "ARBX_POOL_ENUM_MIN_TVL_USD";
const ENV_MAX_NEW: &str = "ARBX_POOL_ENUM_MAX_NEW_PER_TICK";
const ENV_SAFETY_FLOOR: &str = "TOKEN_SAFETY_FLOOR";

const DEFAULT_INTERVAL_MS: u64 = 3_600_000; // 1h
const MIN_INTERVAL_MS: u64 = 60_000; // never hammer the subgraph faster than 1/min
const DEFAULT_TOP_N: usize = 500;
const DEFAULT_MIN_TVL_USD: f64 = 50_000.0;
const DEFAULT_MAX_NEW: usize = 50;
const DEFAULT_SAFETY_FLOOR: i32 = 70;

#[derive(Clone, Debug)]
struct EnumConfig {
    interval: Duration,
    top_n: usize,
    min_tvl_usd: f64,
    max_new: usize,
    safety_floor: i32,
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
        }
    }
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
    /// Fresh score ≥ floor.
    Safe,
    /// Fresh score < floor.
    Unsafe,
    /// No fresh record — not yet rated by token_enricher.
    Unrated,
}

/// Spawn the worker iff `ARBX_POOL_ENUM_MODE == "shadow"`. Off / absent / any
/// other value → no-op (default-OFF). Requires a DB pool; without one it logs
/// and does not spawn (R8 fail-honest). Detached task — process-exit shutdown,
/// matching the other workers' idiom.
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
    info!(
        event = "poolenum.enabled",
        chain_id,
        interval_ms = cfg.interval.as_millis() as u64,
        top_n = cfg.top_n,
        min_tvl_usd = cfg.min_tvl_usd,
        max_new = cfg.max_new,
        safety_floor = cfg.safety_floor,
    );
    tokio::spawn(async move {
        let worker = PoolEnumerationWorker {
            chain_id,
            discovery,
            db,
            cfg,
        };
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

        // 1. Fetch candidates: V3 primary, V2 best-effort (same endpoint).
        let mut candidates: Vec<RankedPool> = Vec::new();
        match subgraph_client::fetch_top_pools_by_tvl(
            self.chain_id,
            SubgraphPoolKind::V3,
            self.cfg.top_n,
            self.cfg.min_tvl_usd,
        )
        .await
        {
            Ok(Some(v)) => candidates.extend(v),
            Ok(None) => {
                info!(
                    event = "poolenum.subgraph_unconfigured",
                    chain_id = self.chain_id
                );
                return; // no ARBX_SUBGRAPH_URL_<chain> → nothing to do
            }
            Err(e) => {
                warn!(event = "poolenum.subgraph_v3_err", chain_id = self.chain_id, error = %e)
            }
        }
        if let Ok(Some(v)) = subgraph_client::fetch_top_pools_by_tvl(
            self.chain_id,
            SubgraphPoolKind::V2,
            self.cfg.top_n,
            self.cfg.min_tvl_usd,
        )
        .await
        {
            candidates.extend(v);
        }
        if candidates.is_empty() {
            info!(event = "poolenum.no_candidates", chain_id = self.chain_id);
            return;
        }
        // Merge-sort by TVL desc, truncate top-N.
        candidates.sort_by(|a, b| {
            b.tvl_usd
                .partial_cmp(&a.tvl_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(self.cfg.top_n);
        let fetched = candidates.len();

        // 2. Dedup: skip ALREADY-ACTIVE pools; inactive ones are re-screened so a
        // now-token-safe pool can be reactivated (monotonic upsert). Map is
        // address(lc) -> is_active.
        let known = self.load_known_pools().await;

        let mut already_known = 0usize;
        let mut factory_missing = 0usize;
        let mut safety_blocked = 0usize;
        let mut hydration_failed = 0usize;
        let mut activated = 0usize;
        let mut persisted_inactive = 0usize;
        let mut processed_new = 0usize;

        for rp in &candidates {
            if processed_new >= self.cfg.max_new {
                break;
            }
            let addr_lc = rp.address.to_lowercase();
            let prior_active = known.get(&addr_lc).copied();
            if prior_active == Some(true) {
                already_known += 1; // already active — nothing to do
                continue;
            }

            // 3a. Token-safety screen for BOTH tokens (cheap DB lookup).
            let s0 = self.token_safety(&rp.token0).await;
            let s1 = self.token_safety(&rp.token1).await;
            let activate = match (s0, s1) {
                (SafetyVerdict::Unsafe, _) | (_, SafetyVerdict::Unsafe) => {
                    safety_blocked += 1;
                    warn!(
                        event = "poolenum.token_unsafe",
                        chain_id = self.chain_id, pool = %rp.address,
                        "skipping pool — a token is below the safety floor"
                    );
                    continue;
                }
                (SafetyVerdict::Safe, SafetyVerdict::Safe) => true,
                // at least one token unrated → persist inactive for a future tick
                _ => false,
            };

            // A dormant (is_active=false) pool that is STILL not activatable needs
            // no work (already persisted inactive); skip without consuming the
            // expensive-op budget. A now-safe one falls through and is reactivated.
            if prior_active == Some(false) && !activate {
                continue;
            }

            // Expensive path (on-chain factory resolve + hydration) — budget-capped.
            processed_new += 1;

            // Parse addresses (subgraph gives lowercase 0x hex).
            let (pool_addr, t0, t1) = match (
                parse_addr(&rp.address),
                parse_addr(&rp.token0),
                parse_addr(&rp.token1),
            ) {
                (Some(p), Some(a), Some(b)) => (p, a, b),
                _ => {
                    warn!(event = "poolenum.bad_address", chain_id = self.chain_id, pool = %rp.address);
                    continue;
                }
            };
            // V3 feeTier is in pips (3000 = 0.30%); bps = pips/100. V2 → None.
            let fee_bps = rp.fee_tier.map(|f| f / 100);

            match self
                .discovery
                .enumerate_and_persist_pool(pool_addr, t0, t1, fee_bps, activate)
                .await
            {
                EnumOutcome::Persisted { activated: true } => activated += 1,
                EnumOutcome::Persisted { activated: false } => persisted_inactive += 1,
                EnumOutcome::FactoryUnresolved => factory_missing += 1,
                EnumOutcome::HydrationFailed | EnumOutcome::NoRpc => hydration_failed += 1,
            }
        }

        info!(
            event = "poolenum.tick",
            chain_id = self.chain_id,
            fetched,
            already_known,
            factory_missing,
            safety_blocked,
            hydration_failed,
            activated,
            persisted_inactive,
            elapsed_ms = started.elapsed().as_millis() as u64,
        );
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
        // V3 feeTier (pips) -> bps: 3000=30, 500=5, 10000=100.
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
        // Verdict logic mirrored as a pure check (floor = 70).
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
        // A tiny interval must be floored so we never hammer the subgraph.
        let interval_ms = 100u64.max(MIN_INTERVAL_MS);
        assert_eq!(interval_ms, MIN_INTERVAL_MS);
    }

    #[test]
    fn activate_decision_matrix() {
        use SafetyVerdict::*;
        let decide = |s0: SafetyVerdict, s1: SafetyVerdict| -> Option<bool> {
            match (s0, s1) {
                (Unsafe, _) | (_, Unsafe) => None, // skip
                (Safe, Safe) => Some(true),        // activate
                _ => Some(false),                  // persist inactive
            }
        };
        assert_eq!(decide(Safe, Safe), Some(true));
        assert_eq!(decide(Safe, Unrated), Some(false));
        assert_eq!(decide(Unrated, Unrated), Some(false));
        assert_eq!(decide(Safe, Unsafe), None);
        assert_eq!(decide(Unsafe, Unrated), None);
    }
}
