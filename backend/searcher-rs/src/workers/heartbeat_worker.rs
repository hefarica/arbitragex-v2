//! Heartbeat Worker — periodic structured summary of pipeline state.
//!
//! Emits one `scanner.heartbeat` event every `period` seconds with:
//!   - Redis stream length (total + delta vs last tick) for `arbx:opps:detected`
//!   - PG opportunities inserted in the last `period` seconds
//!   - PG opportunities with `expected_profit_usd > 0` in the last `period`
//!
//! Why this exists: individual events (`candidate_enriched`, `gate_rejected`)
//! are sparse — sometimes a few per hour. Operators reading docker logs
//! between events cannot distinguish "idle" from "stuck". The heartbeat
//! gives a steady minute-by-minute pulse with deltas so the operator (and
//! external watchers) can spot pipeline degradation without grepping.
//!
//! Cost: 1 Redis XLEN + 1 PG count query per period (default 60s).

use crate::counters::chain_counters;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::Serialize;
use sqlx::{postgres::PgPool, Row};
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;
use tracing::{info, warn};

/// Snapshot persisted to Redis after every heartbeat tick. Operator can
/// query the latest snapshot via GET /api/v1/scanner/heartbeat (no need
/// to grep logs). Schema mirrors the structured log fields one-to-one
/// for documentation parity.
///
/// TTL on the Redis key is 3× heartbeat period — if the searcher dies
/// the snapshot expires automatically and the API returns 404, surfacing
/// the outage instead of stale data (R8 fail-honest).
///
/// **Forward-compat (cs-validator MAJOR fix 2026-05-06):** every field is
/// `#[serde(default)]` so a typed Rust client deserialising during a rolling
/// deploy (where api-server runs the new binary while searcher-rs still emits
/// the old schema, or vice versa) sees missing fields as zero/empty rather
/// than failing the whole snapshot. Zero behavioural cost on the writer side.
#[derive(Debug, Clone, Serialize, Default)]
pub struct HeartbeatSnapshot {
    #[serde(default)]
    pub period_secs: u64,
    #[serde(default)]
    pub emitted_at_unix: u64,
    #[serde(default)]
    pub redis_stream_total: i64,
    #[serde(default)]
    pub redis_stream_delta: i64,
    #[serde(default)]
    pub pg_period_inserted: i64,
    #[serde(default)]
    pub pg_period_profit_pos: i64,
    #[serde(default)]
    pub pending_received: u64,
    #[serde(default)]
    pub decoded_ok: u64,
    /// N-01: V2 path decode failures (route_decoder errors). Visible so the
    /// decode bottleneck is never a silent drown.
    #[serde(default)]
    pub decoded_err: u64,
    #[serde(default)]
    pub enriched_v2: u64,
    #[serde(default)]
    pub enriched_v3: u64,
    #[serde(default)]
    pub gate_token_not_allowed: u64,
    #[serde(default)]
    pub gate_strategy_disabled: u64,
    #[serde(default)]
    pub gate_no_config: u64,
    #[serde(default)]
    pub gate_unknown_token_price: u64,
    #[serde(default)]
    pub gate_anomalous_math: u64,
    #[serde(default)]
    pub gate_other_rejected: u64,
    #[serde(default)]
    pub passed_all_gates: u64,
    #[serde(default)]
    pub db_persisted: u64,
    #[serde(default)]
    pub db_errors: u64,
    /// Price worker — tokens whose live USD price came from Alchemy this period.
    /// Operator interpretation: high value = Alchemy healthy + key valid.
    #[serde(default)]
    pub price_alchemy_hits: u64,
    /// Price worker — tokens whose price came from Coingecko fallback.
    /// Spike here vs alchemy_hits = Alchemy degraded; check key + RPC env.
    #[serde(default)]
    pub price_coingecko_hits: u64,
    /// Price worker — tokens not priced by EITHER source this period.
    /// Sustained non-zero = upstream outage OR token outside both providers'
    /// universe (operator should remove from allowlist or set explicit price).
    #[serde(default)]
    pub price_cache_misses: u64,
    /// Price worker — HTTP / Redis / parse errors per period. Steady non-zero
    /// = config or upstream API regression.
    #[serde(default)]
    pub price_worker_errors: u64,
    /// TriangularWorker — cycles scanned this period. Steady non-zero proves
    /// the worker is alive and exercising MVP_CYCLES every tick. A zero value
    /// across consecutive heartbeats (with the worker enabled in main.rs) is
    /// a regression signal — likely Redis / pool index gap.
    #[serde(default)]
    pub triangular_cycles_scanned: u64,
    /// TriangularWorker — opportunities emitted this period after spot-check,
    /// golden-section and dedup all pass; spine gates evaluated separately downstream.
    /// Compare against `passed_all_gates` to learn what fraction of triangular
    /// candidates survive the canonical risk policy.
    #[serde(default)]
    pub triangular_opps_emitted: u64,
    /// FlashloanArbWorker — pairs scanned this period. Steady non-zero proves
    /// the worker is alive. A zero value across consecutive heartbeats (with
    /// the worker enabled) is a regression signal.
    #[serde(default)]
    pub flashloan_arb_pairs_scanned: u64,
    /// FlashloanArbWorker — opportunities emitted this period after spot-diff,
    /// golden-section, dedup, min-profit, and worker-level sanity bound all
    /// pass. Spine evaluator runs downstream with the canonical risk gates.
    #[serde(default)]
    pub flashloan_arb_opps_emitted: u64,
    /// FlashloanArbWorker — combos rejected because profit_usd > 10% of borrow_usd.
    /// Anti-Incidente #9 self-defense; non-zero means orientation/decimal bug
    /// somewhere in the pool reserves cache. Inspect the
    /// `flashloan_arb_worker.sanity_reject` warn log for the diagnostic dump.
    #[serde(default)]
    pub flashloan_arb_sanity_reject: u64,
    /// TriangularWorker — V3-bearing cycles scanned this period (long-tail cycles
    /// X-WETH-USDC-X for X in {PEPE, SHIB, MKR, COMP}, both directions). Steady
    /// non-zero proves the V3 fan-out path is alive. Compare against
    /// `triangular_v3_quote_failures` to learn the V3-quote success rate.
    #[serde(default)]
    pub triangular_v3_cycles_scanned: u64,
    /// TriangularWorker — V3 quote calls (per-pool) that came back unsuccessful
    /// or zero this period. R8 fail-honest: every failure is counted, never
    /// silently masked with a fabricated quote.
    #[serde(default)]
    pub triangular_v3_quote_failures: u64,
    /// TriangularWorker — V3 cycles rejected by the worker-level sanity bound
    /// (expected_profit_usd > 5× cap_usd). Mirrors the V2 sanity guard. Non-zero
    /// = orientation / pool-index / fee-tier / decimal bug; inspect the
    /// `triangular_worker.sanity_reject_v3` warn log for the diagnostic dump.
    #[serde(default)]
    pub triangular_v3_sanity_reject: u64,
    /// LiquidationWorker — Aave V3 positions scanned this period. Steady
    /// non-zero proves the worker is alive and reading on-chain. Zero across
    /// several heartbeats with the worker enabled is a regression signal
    /// (watchlist empty / provider absent / multicall failing).
    #[serde(default)]
    pub liquidation_positions_scanned: u64,
    /// LiquidationWorker — opportunities emitted this period after HF, dedup,
    /// profit estimation, sanity bound and min-profit gates all pass. Spine
    /// evaluator runs downstream with the canonical risk gates.
    #[serde(default)]
    pub liquidation_opps_emitted: u64,
    /// LiquidationWorker — positions rejected because gross_profit_usd >
    /// 20% of debt_to_repay_usd. Anti-Incidente #9 self-defense; non-zero
    /// means bonus-source / decimal-scaling bug. Inspect the
    /// `liquidation_worker.sanity_reject` warn log for the diagnostic dump.
    #[serde(default)]
    pub liquidation_sanity_reject: u64,
    /// Phase A.1/A.2 simulator gate — candidates rejected because no real REVM
    /// dispatch is available (Phase A.3 encoder pending). Until the encoder
    /// ships, this counter is expected to equal `passed_all_gates_pre_simulator`
    /// for the period — every candidate that passes earlier gates fails closed
    /// at the simulator. Drops once Phase A.3 produces real `SimulatorV2`
    /// verdicts (success / revert / error).
    #[serde(default)]
    pub simulator_fail_closed_rejected: u64,
}

pub fn heartbeat_redis_key(chain_id: u64) -> String {
    format!("arbx:heartbeat:scanner:{chain_id}:latest")
}

pub struct HeartbeatWorker {
    pub period: Duration,
    pub chain_id: u64,
}

impl HeartbeatWorker {
    pub fn new(period_secs: u64, chain_id: u64) -> Self {
        Self {
            period: Duration::from_secs(period_secs.max(1)),
            chain_id,
        }
    }

    pub async fn run(&self, mut redis: ConnectionManager, db: Option<PgPool>) {
        let mut ticker = interval(self.period);
        // Skip the first immediate tick — gives downstream services time to
        // ingest and produces a meaningful delta on the first emit.
        ticker.tick().await;

        let mut last_redis_total: i64 = read_redis_xlen(&mut redis).await.unwrap_or_else(|e| {
            warn!(event = "heartbeat.redis_init_failed", error = %e);
            0
        });

        loop {
            ticker.tick().await;

            let now_redis_total = match read_redis_xlen(&mut redis).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(event = "heartbeat.redis_failed", error = %e);
                    last_redis_total
                }
            };
            let redis_delta = now_redis_total.saturating_sub(last_redis_total);
            last_redis_total = now_redis_total;

            // PG counts are best-effort — when DB is absent or unhealthy we emit
            // -1 sentinels so the heartbeat never blocks pipeline observability
            // on persistence health.
            let (pg_inserted, pg_profit_pos): (i64, i64) = match db.as_ref() {
                Some(pool) => match read_pg_counts(pool, self.period.as_secs()).await {
                    Ok(pair) => pair,
                    Err(e) => {
                        warn!(event = "heartbeat.pg_failed", error = %e);
                        (-1, -1)
                    }
                },
                None => (-1, -1),
            };

            // Drain in-memory scanner counters via atomic swap → 0
            // so each heartbeat reports the delta for the just-elapsed period.
            // Lock-free; safe across all increment sites in scanner.rs.
            //
            // B0.1 (2026-05-13): per-chain isolation. Each HeartbeatWorker is
            // already constructed with `chain_id`; we now drain ONLY the
            // counters for this chain. Other chains' counters are read by
            // their own HeartbeatWorker instances. Crosstalk eliminated.
            let c = chain_counters(self.chain_id);
            let pending = c.pending_received.swap(0, Ordering::Relaxed);
            let decoded = c.decoded_ok.swap(0, Ordering::Relaxed);
            let decoded_err = c.decoded_err.swap(0, Ordering::Relaxed);
            let enriched_v2 = c.enriched_v2.swap(0, Ordering::Relaxed);
            let enriched_v3 = c.enriched_v3.swap(0, Ordering::Relaxed);
            let gate_token_na = c.gate_token_not_allowed.swap(0, Ordering::Relaxed);
            let gate_strat_dis = c.gate_strategy_disabled.swap(0, Ordering::Relaxed);
            let gate_no_cfg = c.gate_no_config.swap(0, Ordering::Relaxed);
            let gate_unk_price = c.gate_unknown_token_price.swap(0, Ordering::Relaxed);
            let gate_anom = c.gate_anomalous_math.swap(0, Ordering::Relaxed);
            let gate_other = c.gate_other_rejected.swap(0, Ordering::Relaxed);
            let passed = c.passed_all_gates.swap(0, Ordering::Relaxed);
            let db_ok = c.db_persisted.swap(0, Ordering::Relaxed);
            let db_err = c.db_errors.swap(0, Ordering::Relaxed);
            let price_alchemy = c.price_alchemy_hits.swap(0, Ordering::Relaxed);
            let price_coingecko = c.price_coingecko_hits.swap(0, Ordering::Relaxed);
            let price_misses = c.price_cache_misses.swap(0, Ordering::Relaxed);
            let price_errors = c.price_worker_errors.swap(0, Ordering::Relaxed);
            let tri_scanned = c.triangular_cycles_scanned.swap(0, Ordering::Relaxed);
            let tri_emitted = c.triangular_opps_emitted.swap(0, Ordering::Relaxed);
            let fl_scanned = c.flashloan_arb_pairs_scanned.swap(0, Ordering::Relaxed);
            let fl_emitted = c.flashloan_arb_opps_emitted.swap(0, Ordering::Relaxed);
            let fl_sanity = c.flashloan_arb_sanity_reject.swap(0, Ordering::Relaxed);
            let tri_v3_scanned = c.triangular_v3_cycles_scanned.swap(0, Ordering::Relaxed);
            let tri_v3_quote_fail = c.triangular_v3_quote_failures.swap(0, Ordering::Relaxed);
            let tri_v3_sanity = c.triangular_v3_sanity_reject.swap(0, Ordering::Relaxed);
            let liq_scanned = c.liquidation_positions_scanned.swap(0, Ordering::Relaxed);
            let liq_emitted = c.liquidation_opps_emitted.swap(0, Ordering::Relaxed);
            let liq_sanity = c.liquidation_sanity_reject.swap(0, Ordering::Relaxed);
            let sim_fail_closed = c.simulator_fail_closed_rejected.swap(0, Ordering::Relaxed);

            info!(
                event = "scanner.heartbeat",
                period_secs = self.period.as_secs(),
                redis_stream_total = now_redis_total,
                redis_stream_delta = redis_delta,
                pg_period_inserted = pg_inserted,
                pg_period_profit_pos = pg_profit_pos,
                // In-memory pipeline counters (delta this period).
                pending_received = pending,
                decoded_ok = decoded,
                decoded_err = decoded_err,
                enriched_v2 = enriched_v2,
                enriched_v3 = enriched_v3,
                gate_token_not_allowed = gate_token_na,
                gate_strategy_disabled = gate_strat_dis,
                gate_no_config = gate_no_cfg,
                gate_unknown_token_price = gate_unk_price,
                gate_anomalous_math = gate_anom,
                gate_other_rejected = gate_other,
                passed_all_gates = passed,
                db_persisted = db_ok,
                db_errors = db_err,
                price_alchemy_hits = price_alchemy,
                price_coingecko_hits = price_coingecko,
                price_cache_misses = price_misses,
                price_worker_errors = price_errors,
                triangular_cycles_scanned = tri_scanned,
                triangular_opps_emitted = tri_emitted,
                flashloan_arb_pairs_scanned = fl_scanned,
                flashloan_arb_opps_emitted = fl_emitted,
                flashloan_arb_sanity_reject = fl_sanity,
                triangular_v3_cycles_scanned = tri_v3_scanned,
                triangular_v3_quote_failures = tri_v3_quote_fail,
                triangular_v3_sanity_reject = tri_v3_sanity,
                liquidation_positions_scanned = liq_scanned,
                liquidation_opps_emitted = liq_emitted,
                liquidation_sanity_reject = liq_sanity,
                simulator_fail_closed_rejected = sim_fail_closed,
                "scanner pipeline heartbeat"
            );

            // Persist snapshot to Redis for API consumption (api-server reads
            // this key on GET /api/v1/scanner/heartbeat). TTL = 3× period so
            // that if the searcher dies, the API returns 404 (R8 fail-honest)
            // rather than serving stale data.
            let snapshot = HeartbeatSnapshot {
                period_secs: self.period.as_secs(),
                emitted_at_unix: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                redis_stream_total: now_redis_total,
                redis_stream_delta: redis_delta,
                pg_period_inserted: pg_inserted,
                pg_period_profit_pos: pg_profit_pos,
                pending_received: pending,
                decoded_ok: decoded,
                decoded_err,
                enriched_v2,
                enriched_v3,
                gate_token_not_allowed: gate_token_na,
                gate_strategy_disabled: gate_strat_dis,
                gate_no_config: gate_no_cfg,
                gate_unknown_token_price: gate_unk_price,
                gate_anomalous_math: gate_anom,
                gate_other_rejected: gate_other,
                passed_all_gates: passed,
                db_persisted: db_ok,
                db_errors: db_err,
                price_alchemy_hits: price_alchemy,
                price_coingecko_hits: price_coingecko,
                price_cache_misses: price_misses,
                price_worker_errors: price_errors,
                triangular_cycles_scanned: tri_scanned,
                triangular_opps_emitted: tri_emitted,
                flashloan_arb_pairs_scanned: fl_scanned,
                flashloan_arb_opps_emitted: fl_emitted,
                flashloan_arb_sanity_reject: fl_sanity,
                triangular_v3_cycles_scanned: tri_v3_scanned,
                triangular_v3_quote_failures: tri_v3_quote_fail,
                triangular_v3_sanity_reject: tri_v3_sanity,
                liquidation_positions_scanned: liq_scanned,
                liquidation_opps_emitted: liq_emitted,
                liquidation_sanity_reject: liq_sanity,
                simulator_fail_closed_rejected: sim_fail_closed,
            };
            if let Ok(json) = serde_json::to_string(&snapshot) {
                let key = heartbeat_redis_key(self.chain_id);
                let ttl = self.period.as_secs().saturating_mul(3);
                let res: redis::RedisResult<()> = redis.set_ex(&key, json, ttl).await;
                if let Err(e) = res {
                    warn!(event = "heartbeat.redis_persist_failed", error = %e);
                }
            }
        }
    }
}

async fn read_redis_xlen(redis: &mut ConnectionManager) -> redis::RedisResult<i64> {
    redis::cmd("XLEN")
        .arg("arbx:opps:detected")
        .query_async(redis)
        .await
}

async fn read_pg_counts(pool: &PgPool, period_secs: u64) -> Result<(i64, i64), sqlx::Error> {
    // period_secs is u64 from internal config — not user input, safe to inline.
    let sql = format!(
        "SELECT \
             COUNT(*)::int8 AS total, \
             COUNT(*) FILTER (WHERE expected_profit_usd > 0)::int8 AS profit_pos \
         FROM opportunities \
         WHERE detected_at > NOW() - INTERVAL '{} seconds'",
        period_secs
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total: i64 = row.try_get("total").unwrap_or(0);
    let profit_pos: i64 = row.try_get("profit_pos").unwrap_or(0);
    Ok((total, profit_pos))
}
