//! Redis Streams consumer for sim-ctl.
//!
//! Reads `arbx:opps:validated` published by selector-api (S3). For each opp:
//!   1. simulate — SIMWIRE-02: when the full B2c env is present at boot
//!      (`SIM_BACKEND=revm` + `REVM_RPC_URL` + `ARBITRAGE_EXECUTOR` +
//!      `REDIS_URL`), the route-aware REAL pipeline runs: canonical
//!      `route_metadata` from PG → decimals resolution → the SAME encoder
//!      the searcher uses → `execute_multistep_revm` (paper_mode=true,
//!      observer-only). The legacy `SimulatorBackend` (SIMWIRE-01 wiring)
//!      remains for the anvil default and HTTP compat — it is NEVER Canal
//!      B's source, because RevmBackend's calldata is empty by construction
//!      (`route_encoding_not_available`).
//!   2. persist simulation + update opp status (typed capability gaps keep
//!      the opp non-rejected — see persistence::is_sim_capability_gap)
//!   3. if passed → XADD arbx:opps:simulated (downstream for S5)
//!   4. XACK only after persist.
//!
//! SIMWIRE-02 P2 — PEL recovery: transient infra errors (PG fetch, gas read,
//! REVM state fetch) leave the entry UNACKED in the group's Pending Entries
//! List. `recover_stale_pending` runs XPENDING (gauges) + XAUTOCLAIM
//! (redelivery) on a fixed cadence, so a crashed/errored consumer's entries
//! are reclaimed and reprocessed instead of living in the PEL forever.

use crate::persistence::insert_simulation;
use crate::route_lookup;
use crate::sim_runner::{run_real_simulation, RealSimEnvConfig};
use crate::simulator_backend::SimulatorBackend;
use anyhow::{Context, Result};
use chrono::Utc;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::contracts::{Opportunity, SimulationResult, SimulatorKind};
use shared_rs::killswitch::KillSwitchClient;
use shared_rs::metrics::{
    SIMULATIONS_TOTAL, SIM_STREAM_CLAIMED_COUNT, SIM_STREAM_CLAIM_FAILURES, SIM_STREAM_GHOST_ACKED,
    SIM_STREAM_OLDEST_PENDING_MS, SIM_STREAM_PENDING_COUNT,
};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

const STREAM_IN: &str = "arbx:opps:validated";
const STREAM_OUT: &str = "arbx:opps:simulated";
const GROUP: &str = "sim-ctl-g0";
const STREAM_MAXLEN: usize = 10_000;

/// PEL observation + recovery cadence (SIMWIRE-02 P2).
const RECOVERY_INTERVAL: Duration = Duration::from_secs(60);

/// Entries idle at least this long are considered crashed/abandoned and are
/// claimed for redelivery. Deliberately longer than a full in-flight batch
/// (COUNT 8 x worst-case one-shot REVM sim) so live work is never stolen
/// from a healthy-but-slow iteration of THIS consumer.
const CLAIM_MIN_IDLE_MS: u64 = 120_000;

/// Route-aware REAL-sim context (SIMWIRE-02 Canal B source).
///
/// Built at boot only when the FULL B2c env is present. `Some` in the
/// Consumer switches `process_message` off the legacy `SimulatorBackend`
/// onto the route-aware pipeline (`route_lookup::fetch_candidate_inputs` →
/// `sim_runner::run_real_simulation`).
pub struct B2cCtx {
    pub simulator: Arc<simulator_v2::SimulatorV2>,
    pub env: RealSimEnvConfig,
    /// Live gas_price_wei read handle (same key scheme as RevmBackend).
    pub gas_redis: Arc<tokio::sync::Mutex<ConnectionManager>>,
}

pub struct Consumer {
    pub redis: ConnectionManager,
    pub pool: PgPool,
    pub backend: Arc<dyn SimulatorBackend>,
    /// SIMWIRE-02: `None` on the legacy (anvil) path, `Some` on B2c.
    pub b2c: Option<B2cCtx>,
    pub killswitch: KillSwitchClient,
    pub consumer_name: String,
}

impl Consumer {
    pub async fn run(mut self) -> Result<()> {
        self.ensure_group().await.ok();
        info!(event = "sim_consumer.started", stream = STREAM_IN, group = GROUP, consumer = %self.consumer_name,
              b2c = self.b2c.is_some());
        // A5-STALL (2026-08-29): the kill-switch halt was 100% silent — 4 days
        // of zero simulation consumption with zero logs. Transition + 10-min
        // summary logs only (R9: no per-loop flooding).
        let mut halt_started_at: Option<std::time::Instant> = None;
        let mut halt_last_logged = std::time::Instant::now();
        // SIMWIRE-02 P2: claim the crash backlog promptly at startup, then
        // observe/recover on the fixed cadence. `None` = never ran yet.
        let mut last_recovery: Option<std::time::Instant> = None;
        loop {
            if self.killswitch.is_enabled().await {
                match halt_started_at {
                    None => {
                        halt_started_at = Some(std::time::Instant::now());
                        halt_last_logged = std::time::Instant::now();
                        warn!(
                            event = "sim_consumer.halted_kill_switch",
                            detail = "kill-switch enabled (explicit arm or fail-closed default after Redis key loss) — simulation consumption paused"
                        );
                    }
                    Some(start) if halt_last_logged.elapsed() >= Duration::from_secs(600) => {
                        halt_last_logged = std::time::Instant::now();
                        warn!(
                            event = "sim_consumer.still_halted_kill_switch",
                            halted_for_s = start.elapsed().as_secs()
                        );
                    }
                    _ => {}
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            if let Some(start) = halt_started_at.take() {
                warn!(
                    event = "sim_consumer.resumed_after_kill_switch",
                    halted_for_s = start.elapsed().as_secs()
                );
            }
            if last_recovery.is_none_or(|t| t.elapsed() >= RECOVERY_INTERVAL) {
                self.recover_stale_pending().await;
                last_recovery = Some(std::time::Instant::now());
            }
            if let Err(e) = self.read_batch().await {
                error!(event = "sim_consumer.read_batch_err", error = %e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    async fn ensure_group(&mut self) -> Result<()> {
        let res: redis::RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(STREAM_IN)
            .arg(GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut self.redis)
            .await;
        match res {
            Ok(_) => {
                info!(event = "sim_consumer.group_created");
                Ok(())
            }
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn read_batch(&mut self) -> Result<()> {
        let res: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(8)
            .arg("BLOCK")
            .arg(2000)
            .arg("STREAMS")
            .arg(STREAM_IN)
            .arg(">")
            .query_async(&mut self.redis)
            .await?;

        let Some(reply) = res else {
            return Ok(());
        };
        for stream_entry in reply {
            if let redis::Value::Bulk(v) = stream_entry {
                // [stream_name, [[id, [k, v, k, v, ...]], ...]]
                if v.len() != 2 {
                    continue;
                }
                for (id, fields) in parse_entry_array(&v[1]) {
                    self.process_message(id, fields).await.ok();
                }
            }
        }
        Ok(())
    }

    /// Observe the group PEL and set the health gauges (SIMWIRE-02 P2).
    ///
    /// * `pending_count` — EXACT total from the XPENDING summary form.
    /// * `oldest_pending_ms` — TRUE age of the oldest entry from its stream
    ///   ID timestamp: the PEL idle-ms resets on every XAUTOCLAIM, so
    ///   idle-since-last-delivery would hide an entry cycling claims forever
    ///   on persistent infra failure.
    ///
    /// Non-fatal: an XPENDING failure counts as a claim failure and returns
    /// — the consumer keeps processing `>` entries regardless.
    async fn observe_pel(&mut self) -> Result<(), String> {
        // Summary form: [total-count, min-id, max-id, consumer-stats[]] —
        // EXACT total; the range form's COUNT cap would understate a backlog.
        let summary: Option<Vec<redis::Value>> = redis::cmd("XPENDING")
            .arg(STREAM_IN)
            .arg(GROUP)
            .query_async(&mut self.redis)
            .await
            .map_err(|e| e.to_string())?;
        let count: i64 = match summary.as_ref().and_then(|v| v.first()) {
            Some(redis::Value::Int(n)) => *n,
            _ => 0,
        };
        // Range form (bounded sample): entry IDs carry first-delivery time.
        let range: Option<Vec<redis::Value>> = redis::cmd("XPENDING")
            .arg(STREAM_IN)
            .arg(GROUP)
            .arg("-")
            .arg("+")
            .arg(64u64)
            .query_async(&mut self.redis)
            .await
            .map_err(|e| e.to_string())?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let mut oldest_ms: i64 = 0;
        if let Some(entries) = range {
            for entry in &entries {
                // [id, consumer-name, idle-ms, delivery-count]
                if let redis::Value::Bulk(f) = entry {
                    if let Some(redis::Value::Data(raw)) = f.first() {
                        let id = String::from_utf8_lossy(raw).to_string();
                        if let Some(age) = stream_id_age_ms(&id, now_ms) {
                            if age > oldest_ms {
                                oldest_ms = age;
                            }
                        }
                    }
                }
            }
        }
        SIM_STREAM_PENDING_COUNT.set(count);
        SIM_STREAM_OLDEST_PENDING_MS.set(oldest_ms);
        if count > 0 {
            info!(
                event = "sim_consumer.pel_observed",
                pending = count,
                oldest_pending_ms = oldest_ms
            );
        }
        Ok(())
    }

    /// SIMWIRE-02 P2: reclaim entries abandoned in the PEL (crashed consumer,
    /// transient-infra no-ack) via XAUTOCLAIM and reprocess them through the
    /// normal `process_message` path. Entries whose stream record was trimmed
    /// away (MAXLEN) are ACKed so the PEL cannot accumulate ghosts.
    async fn recover_stale_pending(&mut self) {
        if let Err(e) = self.observe_pel().await {
            SIM_STREAM_CLAIM_FAILURES.inc();
            warn!(event = "sim_consumer.pel_observe_err", error = %e);
            return;
        }
        let mut cursor = "0-0".to_string();
        loop {
            let reply: redis::RedisResult<Option<Vec<redis::Value>>> = redis::cmd("XAUTOCLAIM")
                .arg(STREAM_IN)
                .arg(GROUP)
                .arg(&self.consumer_name)
                .arg(CLAIM_MIN_IDLE_MS)
                .arg(&cursor)
                .arg("COUNT")
                .arg(8)
                .query_async(&mut self.redis)
                .await;
            let reply = match reply {
                Ok(Some(v)) => v,
                Ok(None) => return,
                Err(e) => {
                    SIM_STREAM_CLAIM_FAILURES.inc();
                    warn!(event = "sim_consumer.xautoclaim_err", error = %e);
                    return;
                }
            };
            // Redis 7: [next-cursor, entries, deleted-ids]; Redis 6.2: [next-cursor, entries]
            if reply.is_empty() {
                return;
            }
            let next_cursor = match &reply[0] {
                redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(),
                _ => return,
            };
            let entries = reply.get(1).map(parse_entry_array).unwrap_or_default();
            if let Some(deleted) = reply.get(2) {
                for id in parse_id_list(deleted) {
                    // Trimmed entries: Redis 7 removes them from the PEL
                    // itself; the explicit ACK is a defensive no-op then.
                    let _: redis::RedisResult<()> = self
                        .redis
                        .xack::<_, _, &str, ()>(STREAM_IN, GROUP, &[id.as_str()])
                        .await;
                }
            }
            for (id, fields) in entries {
                SIM_STREAM_CLAIMED_COUNT.inc();
                // SIMWIRE-02b: a stale entry whose opportunities row is gone
                // (retention purge) can NEVER satisfy the simulations→
                // opportunities FK — every reprocess fails at insert and
                // re-queues forever. ACK it (terminal dead-letter) with a
                // loud, honest warning instead of spinning the loop.
                if let Some(reason) = self.ghost_reason(&fields).await {
                    SIM_STREAM_GHOST_ACKED.inc();
                    warn!(event = "sim_consumer.ghost_entry_acked", id = %id, reason,
                          "stale PEL entry dropped: its payload can never complete");
                    let _: redis::RedisResult<()> = self
                        .redis
                        .xack::<_, _, &str, ()>(STREAM_IN, GROUP, &[id.as_str()])
                        .await;
                    continue;
                }
                info!(event = "sim_consumer.pel_claimed", id = %id, "stale PEL entry reclaimed for reprocessing");
                if let Err(e) = self.process_message(id.clone(), fields).await {
                    error!(event = "sim_consumer.recovered_process_err", id = %id, error = %e);
                }
            }
            if next_cursor == "0-0" || next_cursor == cursor {
                return;
            }
            cursor = next_cursor;
        }
    }

    /// SIMWIRE-02b ghost check (recovery path ONLY — a fresh `>` entry
    /// cannot be a ghost: its opportunities row was just inserted upstream).
    /// Returns `Some(reason)` when this entry can never complete:
    /// - `malformed_payload`: the `json` field carries no usable id.
    /// - `opportunity_row_missing`: the opportunities row was purged while
    ///   the entry sat in the PEL; the simulations FK fails every persist.
    ///
    /// A DB error here degrades to `None` (not a ghost) so transient outages
    /// keep the honest retry path — only a confirmed missing row dead-letters.
    async fn ghost_reason(&self, kv: &[redis::Value]) -> Option<&'static str> {
        let payload = match extract_field(kv, "json") {
            Some(p) => p,
            None => return Some("malformed_payload"),
        };
        let parsed_id: Option<uuid::Uuid> = serde_json::from_str::<serde_json::Value>(&payload)
            .ok()
            .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(str::to_owned))
            .and_then(|s| uuid::Uuid::parse_str(&s).ok());
        let Some(opp_id) = parsed_id else {
            return Some("malformed_payload");
        };
        let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM opportunities WHERE id = $1")
            .bind(opp_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        if exists.is_some() {
            None
        } else {
            Some("opportunity_row_missing")
        }
    }

    async fn process_message(&mut self, id: String, kv: Vec<redis::Value>) -> Result<()> {
        let json = extract_field(&kv, "json");
        let Some(json) = json else {
            warn!(event = "sim_consumer.invalid_msg_no_json", id=%id);
            let _: () = self
                .redis
                .xack::<_, _, &str, ()>(STREAM_IN, GROUP, &[id.as_str()])
                .await?;
            return Ok(());
        };
        let opportunity: Opportunity = match serde_json::from_str(&json) {
            Ok(o) => o,
            Err(e) => {
                warn!(event = "sim_consumer.invalid_msg_parse", id=%id, error=%e);
                let _: () = self
                    .redis
                    .xack::<_, _, &str, ()>(STREAM_IN, GROUP, &[id.as_str()])
                    .await?;
                return Ok(());
            }
        };

        // SIMWIRE-02: Canal B's source is the route-aware B2c REAL pipeline
        // when available; the legacy `SimulatorBackend` otherwise (anvil
        // default). Both branches converge on the same persist/XADD/ACK tail.
        let sim = match &self.b2c {
            Some(b2c) => match self.simulate_b2c(b2c, &opportunity, &id).await {
                Ok(s) => s,
                Err(e) => {
                    // Transient infra (PG fetch / gas read / REVM state
                    // fetch). Record nothing, ACK nothing: the entry stays
                    // in the group PEL and `recover_stale_pending`
                    // (XAUTOCLAIM) redelivers it once infra heals. This is
                    // the recovery mechanism SIMWIRE-01's "retry" comment
                    // promised but did not implement.
                    warn!(event = "sim_consumer.b2c_transient_err", id = %id, error = %e,
                          "entry left unacked for PEL recovery");
                    return Ok(());
                }
            },
            None => {
                // SIMWIRE-01: dispatch through the boot-selected backend so
                // the legacy path honors `SIM_BACKEND`.
                let sim = match self.backend.simulate(&opportunity).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!(event = "sim_consumer.backend_infra_err", id = %id, backend = %self.backend.name(), error = %e);
                        return Ok(());
                    }
                };
                // G-SIM-1 layer-3 flow (legacy path): count EVERY
                // consumer-path simulation in the shared Prometheus counter.
                // The B2c branch counts itself (run_real_simulation counts
                // its outcomes; counted_gap counts typed gaps) — counting
                // here too would double-count.
                count_simulation(&sim);
                sim
            }
        };

        // Persist; if it fails, do NOT ack — retry on next iteration.
        if let Err(e) = insert_simulation(&self.pool, &sim).await {
            error!(event = "sim_consumer.persist_err", id=%id, error=%e);
            return Ok(());
        }

        if sim.passed {
            let payload = serde_json::to_string(&opportunity).unwrap_or_default();
            let _: redis::RedisResult<String> = redis::cmd("XADD")
                .arg(STREAM_OUT)
                .arg("MAXLEN")
                .arg("~")
                .arg(STREAM_MAXLEN)
                .arg("*")
                .arg("json")
                .arg(payload)
                .query_async(&mut self.redis)
                .await;
        }

        let _: () = self
            .redis
            .xack::<_, _, &str, ()>(STREAM_IN, GROUP, &[id.as_str()])
            .await
            .context("xack")?;
        Ok(())
    }

    /// Route-aware B2c REAL simulation for one stream opportunity
    /// (SIMWIRE-02 P1).
    ///
    /// * `Ok(SimulationResult)` — terminal: a market/economic verdict, or a
    ///   TYPED capability gap (`route_metadata_not_available`,
    ///   `candidate_incomplete:*`, `b2c_encode_failed:*`) which persistence
    ///   keeps non-rejecting. Persisted + ACKed.
    /// * `Err(String)` — transient infra: PG fetch failure, gas read
    ///   failure, REVM state-fetch failure (`multistep_lazy_db_failed`,
    ///   `b2c_spawn_blocking_join`). NOTHING persisted, NO ack → PEL →
    ///   XAUTOCLAIM redelivery. A dead RPC must retry, never reject.
    async fn simulate_b2c(
        &self,
        b2c: &B2cCtx,
        opp: &Opportunity,
        entry_id: &str,
    ) -> Result<SimulationResult, String> {
        // 1) Canonical inputs — same source as the A3 HTTP path.
        let inputs = match route_lookup::fetch_candidate_inputs(&self.pool, opp.id).await {
            Ok(Some(i)) => i,
            Ok(None) => {
                // Structural: the producer writes route_metadata at insert
                // time; absence does not self-heal (S4-02 STRUCTURAL family).
                info!(event = "sim_consumer.b2c_gap", id = %entry_id, reason = "route_metadata_not_available");
                return Ok(counted_gap(opp.id, "route_metadata_not_available"));
            }
            Err(e) => return Err(format!("pg_fetch_failed: {e}")),
        };

        // 2) Same completeness gates as the A3 handler — typed
        //    `candidate_incomplete:*` gaps, never silent fabrications.
        let token_addresses = &inputs.route_metadata.token_addresses;
        if token_addresses.is_empty() {
            return Ok(counted_gap(
                opp.id,
                "candidate_incomplete:token_addresses_empty",
            ));
        }
        if let Err(missing) = inputs.resolved_decimals.validate_complete(token_addresses) {
            return Ok(counted_gap(
                opp.id,
                &format!("candidate_incomplete:missing_decimals_{missing:?}"),
            ));
        }
        if inputs.chain_id <= 0 {
            return Ok(counted_gap(
                opp.id,
                &format!("candidate_incomplete:invalid_chain_{}", inputs.chain_id),
            ));
        }
        let amount_in_wei: u128 = match inputs.amount_in_wei.trim().parse() {
            Ok(w) if w > 0 => w,
            _ => {
                return Ok(counted_gap(
                    opp.id,
                    "candidate_incomplete:amount_in_wei_unparseable",
                ));
            }
        };
        // validate_complete already guaranteed this entry; the match keeps
        // the trading path defensive (no unwrap).
        let decimals_in = match inputs.resolved_decimals.get(&token_addresses[0]) {
            Some(d) => d,
            None => {
                return Ok(counted_gap(
                    opp.id,
                    &format!(
                        "candidate_incomplete:missing_decimals_token_in_{}",
                        token_addresses[0]
                    ),
                ));
            }
        };
        let amount_in = amount_in_wei as f64 / 10f64.powi(i32::from(decimals_in));
        if !amount_in.is_finite() || amount_in <= 0.0 {
            return Ok(counted_gap(
                opp.id,
                &format!("candidate_incomplete:amount_in_non_positive_{amount_in}"),
            ));
        }

        // 3) Candidate — same construction as A3 (honest 0.0 for the fields
        //    the encoder does not consume; R8: never fabricated).
        let candidate = shared_rs::candidates::OpportunityCandidate {
            opportunity_id: opp.id,
            chain_id: inputs.chain_id as u64,
            token_addresses: token_addresses.clone(),
            pool_addresses: inputs.route_metadata.pool_addresses.clone(),
            dex_adapters: inputs.route_metadata.dex_adapters.clone(),
            amount_in,
            expected_amount_out: 0.0,
            gross_profit: 0.0,
            decimals: inputs.resolved_decimals.clone(),
            block_number: inputs.block_number.filter(|b| *b >= 0).map(|b| b as u64),
            route_fingerprint: format!("{}_{}_{}", inputs.dex_a, inputs.token_in, inputs.token_out),
        };

        // 4) Live gas price — transient: a missing/zero gas oracle must
        //    retry via the PEL, never become a rejection.
        let gas_price_wei = crate::read_gas_price(&b2c.gas_redis, candidate.chain_id).await?;

        // 5) REAL multi-step REVM simulation (same encoder as the searcher).
        let outcome =
            run_real_simulation(candidate, b2c.simulator.clone(), &b2c.env, gas_price_wei).await;

        // 6) REVM state-fetch infra failures are TRANSIENT — a dead/slow RPC
        //    must not drain the stream into rejections.
        if !outcome.passed {
            if let Some(fr) = outcome.fail_reason.as_deref() {
                if fr.starts_with("multistep_lazy_db_failed")
                    || fr.starts_with("b2c_spawn_blocking_join")
                {
                    return Err(format!("revm_state_infra: {fr}"));
                }
            }
        }

        // 7) Translate → SimulationResult. PRICES-FREE by design: net-USD is
        //    computed downstream from prices; `simulated_profit_usd` stays
        //    None (R8: None = not computed, never fabricated).
        Ok(SimulationResult {
            opportunity_id: opp.id,
            passed: outcome.passed,
            gas_estimate_wei: Some(outcome.gas_used_total.to_string()),
            gas_price_wei: Some(outcome.gas_price_wei.to_string()),
            slippage_pct: None,
            revert_risk_pct: None,
            simulated_profit_usd: None,
            simulator: SimulatorKind::Revm,
            fail_reason: outcome.fail_reason,
            simulated_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        })
    }
}

/// Count a simulation in the shared Prometheus counter (declared semantics:
/// "Simulations by simulator and pass/fail"). Used by the legacy consumer
/// branch and the typed-gap returns of the B2c branch — `run_real_simulation`
/// counts its own outcomes, so the B2c success/failure tail does not call this.
fn count_simulation(sim: &SimulationResult) {
    let sim_kind = match &sim.simulator {
        SimulatorKind::Anvil => "anvil",
        SimulatorKind::Tenderly => "tenderly",
        SimulatorKind::Hardhat => "hardhat",
        SimulatorKind::Revm => "revm",
        SimulatorKind::NotImplemented => "not_implemented",
    };
    SIMULATIONS_TOTAL
        .with_label_values(&[sim_kind, if sim.passed { "true" } else { "false" }])
        .inc();
}

/// Typed-gap SimulationResult: `passed=false` with a fail_reason that
/// `is_sim_capability_gap` classifies as absence-of-capability — the
/// opportunity stays non-rejected (status detected/validated,
/// rejection_reason NULL) while the simulations row records the skip
/// honestly. Counted in SIMULATIONS_TOTAL because the attempt really ran.
fn counted_gap(opportunity_id: Uuid, reason: &str) -> SimulationResult {
    let r = SimulationResult {
        opportunity_id,
        passed: false,
        gas_estimate_wei: None,
        gas_price_wei: None,
        slippage_pct: None,
        revert_risk_pct: None,
        simulated_profit_usd: None,
        simulator: SimulatorKind::Revm,
        fail_reason: Some(reason.to_string()),
        simulated_at: Utc::now(),
        trace_id: Uuid::new_v4(),
    };
    count_simulation(&r);
    r
}

/// TRUE age in ms of a stream entry from its ID timestamp (`<ms>-<seq>`),
/// or None when the ID is not in that form. Age-from-ID survives XAUTOCLAIM
/// redeliveries (which reset the PEL idle counter), so an entry cycling
/// claims forever on persistent infra failure still shows its real age.
fn stream_id_age_ms(id: &str, now_ms: i64) -> Option<i64> {
    let ms: i64 = id.split('-').next()?.parse().ok()?;
    if ms <= 0 {
        return None;
    }
    Some((now_ms - ms).max(0))
}

/// Parse a Redis array of stream entries (`[[id, [k, v, k, v, ...]], ...]`)
/// as returned by XREADGROUP's per-stream entry list AND XAUTOCLAIM's second
/// reply element. Shared so the live path and PEL recovery decode identically.
fn parse_entry_array(entries: &redis::Value) -> Vec<(String, Vec<redis::Value>)> {
    let mut out = Vec::new();
    if let redis::Value::Bulk(list) = entries {
        for e in list {
            if let redis::Value::Bulk(parts) = e {
                if parts.len() != 2 {
                    continue;
                }
                let id = match &parts[0] {
                    redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(),
                    _ => continue,
                };
                let fields = match &parts[1] {
                    redis::Value::Bulk(f) => f.clone(),
                    _ => continue,
                };
                out.push((id, fields));
            }
        }
    }
    out
}

/// Parse a Redis array of entry IDs (XAUTOCLAIM's third reply element).
fn parse_id_list(v: &redis::Value) -> Vec<String> {
    match v {
        redis::Value::Bulk(list) => list
            .iter()
            .filter_map(|e| match e {
                redis::Value::Data(s) => Some(String::from_utf8_lossy(s).to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_field(kv: &[redis::Value], name: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < kv.len() {
        let k = match &kv[i] {
            redis::Value::Data(s) => std::str::from_utf8(s).ok()?.to_string(),
            _ => return None,
        };
        let v = match &kv[i + 1] {
            redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(),
            _ => return None,
        };
        if k == name {
            return Some(v);
        }
        i += 2;
    }
    None
}

#[cfg(test)]
mod simwire02_parse_tests {
    use super::{extract_field, parse_entry_array, parse_id_list};

    fn data(s: &str) -> redis::Value {
        redis::Value::Data(s.as_bytes().to_vec())
    }

    #[test]
    fn parses_xreadgroup_style_entry_list() {
        let entries = redis::Value::Bulk(vec![redis::Value::Bulk(vec![
            data("1-0"),
            redis::Value::Bulk(vec![data("json"), data("{\"id\":\"a\"}")]),
        ])]);
        let out = parse_entry_array(&entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "1-0");
        assert_eq!(
            extract_field(&out[0].1, "json").as_deref(),
            Some("{\"id\":\"a\"}")
        );
    }

    #[test]
    fn parses_xautoclaim_reply_shape() {
        // [cursor, entries, deleted]
        let cursor = data("0-0");
        let entries = redis::Value::Bulk(vec![
            redis::Value::Bulk(vec![
                data("5-1"),
                redis::Value::Bulk(vec![data("json"), data("{}")]),
            ]),
            redis::Value::Bulk(vec![
                data("5-2"),
                redis::Value::Bulk(vec![data("json"), data("{}")]),
            ]),
        ]);
        let deleted_val = redis::Value::Bulk(vec![data("9-9")]);
        let out = parse_entry_array(&entries);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].0, "5-2");
        let deleted = parse_id_list(&deleted_val);
        assert_eq!(deleted, vec!["9-9".to_string()]);
        assert_eq!(cursor, data("0-0"));
    }

    #[test]
    fn stream_id_age_survives_claims_and_rejects_garbage() {
        use super::stream_id_age_ms;
        let now = 1_700_000_000_000i64;
        assert_eq!(stream_id_age_ms("1699999999995-0", now), Some(5));
        // Future timestamps clamp to 0, never negative:
        assert_eq!(stream_id_age_ms("1700000000100-3", now), Some(0));
        assert_eq!(stream_id_age_ms("json", now), None);
        assert_eq!(stream_id_age_ms("", now), None);
        assert_eq!(stream_id_age_ms("abc-0", now), None);
        assert_eq!(stream_id_age_ms("0-0", now), None);
    }

    #[test]
    fn skips_malformed_entries_without_panicking() {
        let entries = redis::Value::Bulk(vec![
            redis::Value::Bulk(vec![]),                               // no parts
            redis::Value::Bulk(vec![data("7-0")]),                    // missing fields
            redis::Value::Bulk(vec![redis::Value::Int(7), data("")]), // id not Data
            redis::Value::Nil,
        ]);
        assert!(parse_entry_array(&entries).is_empty());
    }
}
