//! Redis Streams consumer for sim-ctl.
//!
//! Reads `arbx:opps:validated` published by selector-api (S3). For each opp:
//!   0. ADMISSION — SIMCTL-VERDICT-01: the STRICT FRONTIER. The stream is a
//!      DECISION LOG, not an accept queue: the producer stamps `verdict` /
//!      `verdict_reason` on every payload, and `Opportunity` declares neither,
//!      so serde used to drop them silently. Only `verdict == "accept"` reaches
//!      step 1. Every other message is diagnosed, persisted with its typed
//!      reason and ACKed WITHOUT consuming an in-flight/rate permit, a fork
//!      snapshot or an `eth_call`. See `decision_frontier`.
//!   1. simulate — SIMWIRE-02: when the full B2c env is present at boot
//!      (`SIM_BACKEND=revm` + `REVM_RPC_URL` + `ARBITRAGE_EXECUTOR` +
//!      `REDIS_URL`), the route-aware REAL pipeline runs: validated-plan
//!      carrier from Redis FIRST (Issue #567 — the producer's plan is the
//!      route of record; full 2-5 hop cycles, never reconstructed from
//!      token_in/token_out) → canonical `route_metadata` from PG → decimals
//!      resolution → the SAME encoder
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

use crate::canonical_plan_consumer;
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
// DL-02 / SIMCTL-VERDICT-01: the strict frontier lives in the LIB target so its
// tests run under CI's blocking `cargo test --workspace --locked --lib` gate; a
// bin-only `#[cfg(test)]` module would never execute there (see lib.rs).
use sim_ctl::decision_frontier::{self, Admission};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use std::time::{Duration, Instant};
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

// ─────────────────────────────────────────────────────────────────────────────
// SIMCTL-BOUND-01 — bounding policy for the validated stream.
//
// WHY THIS EXISTS (measured, 2026-10-07): `simulations` grew 40.958 rows in
// 10 min (~68 rows/s) and the chain is 1:1 with `arbx:opps:validated` — every
// validated message produced exactly one row. That is cheap TODAY only because
// 100% of those rows die BEFORE touching the fork
// (`strategy_cyclic_route_not_simulatable_in_s4` returns without acquiring a
// snapshot or issuing an `eth_call`). The moment that route executes for real
// (SIM4-CYCLIC-01/02), the same rate becomes snapshot acquisition + `eth_call`
// + `estimate_gas` + decode per message, on a host shared with nginx (the only
// public surface) and the searcher. This makes the cost a POLICY instead of an
// accident of throughput.
//
// WHAT IT IS NOT (U6): this does NOT improve any business metric. It bounds
// cost. It must never be reported as a profitability win.
//
// TWO STRATEGIES ARE REFUTED BY MEASUREMENT. Neither may be "simplified" into:
//
//   1. TOP-N ORDERING (U1). Ordering needs the whole universe in hand, and
//      `arbx:opps:validated` retains MAXLEN `STREAM_MAXLEN` = 10_000 entries
//      at ~68 rows/s — a ~2.4 min window that never holds the universe. There
//      is no orderable set, so admissibility is an O(1) INLINE predicate.
//
//   2. `f64::max(key, 0.0)` AS SANITISATION (U2). `f64::max(NaN, 0.0)` returns
//      `0.0` — a zero indistinguishable from a MEASURED zero, which is exactly
//      the R8 failure mode ("None = not computed; Some(0.0) = computed and
//      exactly zero"). And `NaN.partial_cmp(&x)` is `None`, so any
//      `partial_cmp(..).unwrap()` over the RAW key panics and takes the whole
//      consumer loop with it. The key is sanitised with `is_finite()`. Both
//      refuted alternatives are pinned as TESTS at the end of this file.
// ─────────────────────────────────────────────────────────────────────────────

/// Conservative default admission rate, in simulations per second.
///
/// DERIVATION (U9 — deliberately NOT derived from the ~68 rows/s):
/// that rate is the rate of rows that die BEFORE the fork. Treating it as REVM
/// load assumes (a) the call site resolves a route for every one of them and
/// (b) all of them reach the fork. NEITHER is measured, and upstream suggests
/// part of that mass keeps dying in cheaper stages. So this number does not
/// come from it. It comes from a COST-POLICY premise with its arithmetic:
/// the B2c pipeline's measured per-simulation latency is ~2.084 s, so a single
/// sequential consumer cannot exceed 1/2.084 ≈ 0.48 sims/s. A cap of 1.0/s sits
/// ABOVE that physical ceiling — it therefore does NOT throttle the
/// single-consumer deployment — while bounding a runaway at 86.400 sims/day
/// instead of the 5.9 M/day an unbounded 68/s would allow. The DEFINITIVE
/// dimension is fixed post-deploy by measuring the real fork-arrival ratio
/// (see `docs/sre/SIMCTL-BOUND-01.md`); this constant is the conservative
/// placeholder until that measurement exists, never a claim about REVM load.
pub const DEFAULT_MAX_SIMS_PER_SEC: f64 = 1.0;

/// Conservative default in-flight ceiling. `1` equals what this consumer
/// already does (the read loop is sequential), so it is a CEILING for any
/// future concurrent dispatch rather than a throttle: with one permit the
/// permit is never the binding constraint, the rate is. It is NOT a claim that
/// REVM concurrency is otherwise unbounded — `fork_manager`'s snapshot pool
/// bounds the ANVIL path on its own; this is the OUTER policy ceiling.
pub const DEFAULT_MAX_IN_FLIGHT: usize = 1;

/// Env knobs (U5). A value that is present but unusable WARNS and falls back to
/// the default: a typo must never silently change the effective policy.
pub const ENV_MAX_SIMS_PER_SEC: &str = "SIMCTL_MAX_SIMS_PER_SEC";
pub const ENV_MAX_IN_FLIGHT: &str = "SIMCTL_MAX_IN_FLIGHT";

/// Report cadence for the bound's own counters (R9). This loop runs per
/// message; per-message logging would flood the 50 MB log window and destroy
/// the observability of everything else.
const BOUND_LOG_INTERVAL: Duration = Duration::from_secs(600);

/// Sanitise a key before ANY comparison (U2).
///
/// `NaN`/`±inf` become `f64::NEG_INFINITY` — the worst key — so a non-finite
/// input can never be mistaken for a healthy value nor turned into a
/// fabricated `0.0`. Finite values pass through untouched.
#[inline]
pub fn sanitize_key(x: f64) -> f64 {
    if x.is_finite() {
        x
    } else {
        f64::NEG_INFINITY
    }
}

/// The admission key of one message: `amount_in_wei` (a decimal string) read as
/// an `f64` and SANITISED. Unparseable input yields `NEG_INFINITY`, never `0.0`.
#[inline]
pub fn key_of_amount_in_wei(raw: &str) -> f64 {
    match raw.trim().parse::<u128>() {
        Ok(v) => sanitize_key(v as f64),
        Err(_) => f64::NEG_INFINITY,
    }
}

/// SIMCTL-THROUGHPUT-01 (2026-10-08): cuántas entradas pedir en UN pase de
/// lectura (`XREADGROUP COUNT`).
///
/// El `COUNT` debe ser la cota superior de lo que este pase puede ADMITIR, no un
/// número fijo. Un `COUNT` mayor que el presupuesto de admisión no adelanta
/// trabajo: convierte las entradas sobrantes en postergaciones que vuelven al
/// PEL y se redeliveran por `XAUTOCLAIM` — `defer -> 120 s -> defer`.
///
/// Nunca devuelve 0: con el semáforo agotado se pide UNA entrada, para que el
/// bucle siga avanzando en vez de quedarse bloqueado hasta el próximo tick.
#[inline]
pub fn read_batch_size(available_permits: usize) -> usize {
    if available_permits == 0 {
        1
    } else {
        available_permits
    }
}

/// Structural admissibility of ONE already-parsed stream message — O(1), NO
/// I/O, NO ordering, NO global view (U1). `Some(reason)` means this entry can
/// never be simulated, so it is terminated cheaply instead of paying for a PG
/// round-trip and a fork.
///
/// The reasons stay inside the EXISTING `candidate_incomplete:*` family, so
/// `persistence::is_sim_capability_gap` keeps classifying them as capability
/// gaps: the opportunity is NOT rejected (the row explains why nothing ran;
/// that is not a market verdict).
///
/// NOT ECONOMIC (U4). Only `amount_in_wei` is read. `expected_profit_usd`,
/// `net_expected_profit_usd` and `roi_pct` are deliberately NOT consulted: an
/// economic threshold here would pin `passed=true` at zero forever (measured:
/// gross max = 0 over 10.000 entries, 0 with gross > 0) and would blind us
/// exactly when simulation becomes able to execute. The single boundary is
/// EXACTLY zero, which is a structural identity — no amount, no swap — not a
/// profitability threshold.
pub fn inadmissible_reason(opp: &Opportunity) -> Option<String> {
    let raw = opp.amount_in_wei.trim();
    if key_of_amount_in_wei(raw) > 0.0 {
        return None;
    }
    // Two structural reasons, reported distinctly so the persisted row says
    // which one happened (an operator seeing a spike of either needs to know).
    if raw.parse::<u128>().is_ok() {
        Some("candidate_incomplete:amount_in_wei_zero".to_string())
    } else {
        Some("candidate_incomplete:amount_in_wei_unparseable".to_string())
    }
}

/// Parse a rate knob. `Ok(None)` = absent (default applies); `Err(msg)` = present
/// but unusable, which the caller must WARN about rather than swallow.
fn read_rate_env(name: &str) -> Result<Option<f64>, String> {
    let Ok(raw) = std::env::var(name) else {
        return Ok(None);
    };
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    match t.parse::<f64>() {
        // `Duration::from_secs_f64` PANICS on a non-finite or negative value, so
        // the guard here is also what keeps a bad env var from aborting the
        // process at startup instead of merely bounding it.
        Ok(v) if v.is_finite() && v > 0.0 => Ok(Some(v)),
        _ => Err(format!(
            "{name}={raw:?} unusable (expected a finite value > 0); conservative default in force"
        )),
    }
}

/// Parse an in-flight knob. Absent = default; `< 1` or unparseable = unusable.
fn read_in_flight_env(name: &str) -> Result<Option<usize>, String> {
    let Ok(raw) = std::env::var(name) else {
        return Ok(None);
    };
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    match t.parse::<usize>() {
        Ok(v) if v >= 1 => Ok(Some(v)),
        _ => Err(format!(
            "{name}={raw:?} unusable (expected an integer >= 1); conservative default in force"
        )),
    }
}

/// The resolved bound. Built ONCE at startup and logged from its EFFECTIVE
/// values (U5) — never assumed, never documented from memory.
///
/// MODE-INVARIANT by construction: this is COST policy, so PAPER == TESTNET ==
/// LIVE. The doctrine's hot-path mode-invariance (§34.1) covers the maths; a
/// cost ceiling that silently differed per mode would be a different system
/// wearing the same name.
#[derive(Debug, Clone, Copy)]
pub struct BoundPolicy {
    pub max_sims_per_sec: f64,
    pub max_in_flight: usize,
    /// `true` when the value came from the env; `false` = default in force.
    pub rate_from_env: bool,
    pub in_flight_from_env: bool,
}

impl BoundPolicy {
    pub fn from_env() -> Self {
        let (max_sims_per_sec, rate_from_env) = match read_rate_env(ENV_MAX_SIMS_PER_SEC) {
            Ok(Some(v)) => (v, true),
            Ok(None) => (DEFAULT_MAX_SIMS_PER_SEC, false),
            Err(msg) => {
                warn!(event = "sim_ctl.bound_env_unusable", detail = %msg);
                (DEFAULT_MAX_SIMS_PER_SEC, false)
            }
        };
        let (max_in_flight, in_flight_from_env) = match read_in_flight_env(ENV_MAX_IN_FLIGHT) {
            Ok(Some(v)) => (v, true),
            Ok(None) => (DEFAULT_MAX_IN_FLIGHT, false),
            Err(msg) => {
                warn!(event = "sim_ctl.bound_env_unusable", detail = %msg);
                (DEFAULT_MAX_IN_FLIGHT, false)
            }
        };
        Self {
            max_sims_per_sec,
            max_in_flight,
            rate_from_env,
            in_flight_from_env,
        }
    }
}

/// O(1) pacing state for the bound. `now` is always a PARAMETER so the decision
/// is deterministic and testable without a clock or a tokio runtime.
pub struct StreamBound {
    pub policy: BoundPolicy,
    /// Minimum spacing between two admissions (1 / rate).
    spacing: Duration,
    /// Next instant at which an admission is allowed.
    next_allowed: Instant,
    pub admitted: u64,
    pub deferred: u64,
    pub terminated_inadmissible: u64,
    /// DL-02 / SIMCTL-VERDICT-01: messages the producer's OWN verdict kept out
    /// of the simulation path (`producer_verdict:*`). Terminal and recorded,
    /// never a deferral — and reached without consuming a permit.
    pub terminated_decision_log: u64,
    last_report: Instant,
}

impl StreamBound {
    pub fn new(policy: BoundPolicy, now: Instant) -> Self {
        // A hand-built policy (tests, future callers) must not be able to panic
        // the process through `Duration::from_secs_f64`: an unusable rate falls
        // back to the conservative default here too.
        let rate = if policy.max_sims_per_sec.is_finite() && policy.max_sims_per_sec > 0.0 {
            policy.max_sims_per_sec
        } else {
            DEFAULT_MAX_SIMS_PER_SEC
        };
        let in_flight = if policy.max_in_flight >= 1 {
            policy.max_in_flight
        } else {
            DEFAULT_MAX_IN_FLIGHT
        };
        Self {
            policy: BoundPolicy {
                max_sims_per_sec: rate,
                max_in_flight: in_flight,
                ..policy
            },
            spacing: Duration::from_secs_f64(1.0 / rate),
            next_allowed: now,
            admitted: 0,
            deferred: 0,
            terminated_inadmissible: 0,
            terminated_decision_log: 0,
            last_report: now,
        }
    }

    /// O(1) admission decision. Refuses (and consumes nothing) when the pacer
    /// has not reached the next slot. Never sorts, never buffers, never reads
    /// ahead (U1).
    pub fn try_admit(&mut self, now: Instant) -> bool {
        if now < self.next_allowed {
            return false;
        }
        self.admitted += 1;
        self.next_allowed = now + self.spacing;
        true
    }

    /// Record a DEFERRAL. First occurrence logs immediately (so an operator
    /// sees the bound bite without waiting for the cadence), then at most once
    /// per `BOUND_LOG_INTERVAL` (R9).
    pub fn note_deferred(&mut self, now: Instant, entry_id: &str, detail: &str) {
        self.deferred += 1;
        if self.deferred == 1
            || now.saturating_duration_since(self.last_report) >= BOUND_LOG_INTERVAL
        {
            self.last_report = now;
            warn!(
                event = "sim_ctl.bound_deferred",
                id = %entry_id,
                detail = %detail,
                deferred_total = self.deferred,
                admitted_total = self.admitted,
                max_sims_per_sec = self.policy.max_sims_per_sec,
                max_in_flight = self.policy.max_in_flight,
                "policy bound reached — entry left UNACKED in the PEL for redelivery, NOT discarded"
            );
        }
    }

    /// Record a terminal structural rejection. Persisted and ACKed by the
    /// caller with its typed reason; NOT counted as a simulation attempt.
    pub fn note_inadmissible(&mut self, now: Instant, entry_id: &str, reason: &str) {
        self.terminated_inadmissible += 1;
        if self.terminated_inadmissible == 1
            || now.saturating_duration_since(self.last_report) >= BOUND_LOG_INTERVAL
        {
            self.last_report = now;
            warn!(
                event = "sim_ctl.bound_terminated_inadmissible",
                id = %entry_id,
                reason = %reason,
                terminated_total = self.terminated_inadmissible,
                "structurally inadmissible entry terminated before any I/O or fork"
            );
        }
    }

    /// DL-02 / SIMCTL-VERDICT-01: record a message the PRODUCER's own verdict
    /// kept out of the simulation path.
    ///
    /// Terminal and RECORDED, not deferred and not dropped: `finish` persists
    /// the typed reason on the `simulations` row, so the entry is explained
    /// instead of vanishing, and no rate/in-flight permit was consumed to reach
    /// this point. `family` is the BOUNDED metric label (the raw producer
    /// reason stays in `reason`).
    ///
    /// Logged on the first occurrence and then at most once per
    /// `BOUND_LOG_INTERVAL` (R9): this loop runs per message and a reject-only
    /// stream is 10.000 entries — per-entry logging here is exactly the flood
    /// that destroys the observability of everything else.
    pub fn note_decision_log(&mut self, now: Instant, entry_id: &str, reason: &str, family: &str) {
        self.terminated_decision_log += 1;
        if self.terminated_decision_log == 1
            || now.saturating_duration_since(self.last_report) >= BOUND_LOG_INTERVAL
        {
            self.last_report = now;
            warn!(
                event = "sim_ctl.bound_terminated_decision_log",
                id = %entry_id,
                reason = %reason,
                reason_family = %family,
                decision_log_total = self.terminated_decision_log,
                "producer verdict kept this entry out of the simulation path — recorded with \
                 its typed reason, no fork RPC consumed"
            );
        }
    }

    /// Periodic summary of the bound's counters (R9 cadence).
    pub fn maybe_report(&mut self, now: Instant) {
        if now.saturating_duration_since(self.last_report) >= BOUND_LOG_INTERVAL {
            self.last_report = now;
            info!(
                event = "sim_ctl.bound_report",
                admitted = self.admitted,
                deferred = self.deferred,
                terminated_inadmissible = self.terminated_inadmissible,
                terminated_decision_log = self.terminated_decision_log,
                max_sims_per_sec = self.policy.max_sims_per_sec,
                max_in_flight = self.policy.max_in_flight
            );
        }
    }
}

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
        // SIMCTL-BOUND-01 (U5): resolve the bound ONCE and RECORD the EFFECTIVE
        // value at startup. Logged from the RESOLVED struct, not transcribed
        // from documentation — whoever reads this line sees the policy that is
        // actually in force, including whether each knob came from the env or
        // from the conservative default.
        let policy = BoundPolicy::from_env();
        info!(
            event = "sim_ctl.bound_policy_effective",
            max_sims_per_sec = policy.max_sims_per_sec,
            max_in_flight = policy.max_in_flight,
            rate_from_env = policy.rate_from_env,
            in_flight_from_env = policy.in_flight_from_env,
            env_rate = ENV_MAX_SIMS_PER_SEC,
            env_in_flight = ENV_MAX_IN_FLIGHT,
            mode_invariant = true,
            "cost ceiling only — no business metric is affected (SIMCTL-BOUND-01 U6)"
        );
        let mut bound = StreamBound::new(policy, Instant::now());
        // Outer in-flight ceiling. With `max_in_flight = 1` it never binds in
        // the sequential loop; it is the POLICY ceiling for any future
        // concurrent dispatch, not a claim that REVM concurrency was otherwise
        // unbounded (`fork_manager` bounds the anvil snapshot pool itself).
        let in_flight = Arc::new(tokio::sync::Semaphore::new(policy.max_in_flight));
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
                self.recover_stale_pending(&mut bound, &in_flight).await;
                last_recovery = Some(std::time::Instant::now());
            }
            bound.maybe_report(Instant::now());
            if let Err(e) = self.read_batch(&mut bound, &in_flight).await {
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

    async fn read_batch(
        &mut self,
        bound: &mut StreamBound,
        in_flight: &Arc<tokio::sync::Semaphore>,
    ) -> Result<()> {
        // ── SIMCTL-THROUGHPUT-01 (2026-10-08): el COUNT se ALINEA con el
        // presupuesto de admisión.
        //
        // Antes era un `8` FIJO contra `max_in_flight = 1` — el default
        // (`DEFAULT_MAX_IN_FLIGHT`, :119) y el valor REALMENTE en vigor, porque
        // el contenedor no define `SIMCTL_MAX_IN_FLIGHT`. Con esa combinación,
        // de cada lote de 8 entradas sólo UNA podía tomar permiso; las otras 7
        // caían en `note_deferred`. Y las que caen son justamente las
        // ADMISIBLES: el permiso se pide sólo si `inadmissible.is_none()`
        // (`process_message`), así que las inadmisibles nunca lo piden y se
        // procesan siempre.
        //
        // MEDIDO en el arranque 02:51:25Z (`sim_ctl.bound_report`):
        //   admitted = 243 · deferred = 103604 · terminated_inadmissible = 220
        // => 426 postergaciones por cada admisión. Eso NO es "ir detrás": es
        // churn de PEL (defer -> `CLAIM_MIN_IDLE_MS` 120 s -> XAUTOCLAIM ->
        // defer otra vez), visible como `arbx_sim_stream_claimed_count 67514`,
        // `pending 11004`, `oldest_pending 431335 ms`.
        //
        // Leer más de lo que se puede admitir no adelanta trabajo: lo CONVIERTE
        // en churn. Pedir exactamente lo que cabe lo elimina sin tocar el techo:
        // NO se sube `max_in_flight` ni `max_sims_per_sec`, NO se relaja ningún
        // umbral y NO se filtra por rentabilidad. El límite es el mismo; lo que
        // cambia es que deja de gastarse en entradas que no se van a poder
        // procesar en este pase.
        //
        // `available_permits()` al momento de leer es la cota superior de lo que
        // este pase puede admitir. `max(1)` mantiene el caso degenerado
        // (semáforo agotado) en UNA entrada y no en cero: leer 0 dejaría al
        // consumidor sin avanzar hasta el próximo tick, que es peor que el
        // defecto que se corrige.
        let batch = read_batch_size(in_flight.available_permits());
        let res: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(batch)
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
                    self.process_message(id, fields, bound, in_flight)
                        .await
                        .ok();
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
    ///
    /// SIMCTL-BOUND-01: the bound is THREADED THROUGH here on purpose. A
    /// redelivered entry runs the same simulation, so an unbounded recovery
    /// path would be a hole big enough to swallow the whole policy: a standing
    /// PEL backlog would re-simulate at full speed while the `>` path was
    /// politely paced. Both entry points to `process_message` are bounded.
    async fn recover_stale_pending(
        &mut self,
        bound: &mut StreamBound,
        in_flight: &Arc<tokio::sync::Semaphore>,
    ) {
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
                // SIMWIRE-02c P1: the ghost metric + log fire ONLY after
                // Redis CONFIRMS the XACK — a failed ACK leaves the entry
                // in the PEL uncounted (the count means "dead-lettered",
                // not "attempted").
                if let Some(reason) = self.ghost_reason(&fields).await {
                    let ack: redis::RedisResult<()> =
                        self.redis.xack(STREAM_IN, GROUP, &[id.as_str()]).await;
                    match ack {
                        Ok(()) => {
                            SIM_STREAM_GHOST_ACKED.inc();
                            warn!(event = "sim_consumer.ghost_entry_acked", id = %id, reason,
                                  "stale PEL entry dead-lettered: its payload can never complete");
                        }
                        Err(e) => {
                            error!(event = "sim_consumer.ghost_ack_failed", id = %id, reason,
                                   error = %e,
                                   "XACK failed — entry stays in the PEL; not counted as ghost_acked");
                        }
                    }
                    continue;
                }
                info!(event = "sim_consumer.pel_claimed", id = %id, "stale PEL entry reclaimed for reprocessing");
                if let Err(e) = self
                    .process_message(id.clone(), fields, bound, in_flight)
                    .await
                {
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
        let lookup: Result<Option<i32>, sqlx::Error> =
            sqlx::query_scalar("SELECT 1 FROM opportunities WHERE id = $1")
                .bind(opp_id)
                .fetch_optional(&self.pool)
                .await;
        // SIMWIRE-02c P1-1: a DB error during the check is NOT a ghost —
        // dead-lettering on an outage would ACK entries whose opportunity
        // rows are alive. Loud warn + honest PEL retry instead.
        if let Err(e) = &lookup {
            warn!(event = "sim_consumer.ghost_check_db_err", id = %opp_id, error = %e,
                  "ghost check query failed — treating as NOT ghost; entry stays in the PEL");
        }
        match ghost_verdict(lookup) {
            GhostVerdict::NotGhost => None,
            GhostVerdict::RowMissing => Some("opportunity_row_missing"),
        }
    }

    async fn process_message(
        &mut self,
        id: String,
        kv: Vec<redis::Value>,
        bound: &mut StreamBound,
        in_flight: &Arc<tokio::sync::Semaphore>,
    ) -> Result<()> {
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

        // ── SIMCTL-VERDICT-01 (DL-02) gate 0: the STRICT FRONTIER ────────────
        // `arbx:opps:validated` is a DECISION LOG, not an accept queue: the
        // producer stamps `verdict` / `verdict_reason` on EVERY payload it
        // publishes (`selector-api` `publishValidated`, decision vocabulary in
        // `policy/engine.ts`). `Opportunity` does not declare either key, so
        // serde dropped both SILENTLY — the `Ok(..)` of the parse above was the
        // MEASURE of that drop, never a signal that the message was
        // admissible. Measured on the channel: 10.000 of 10.000 entries
        // carried `"verdict":"reject"`.
        //
        // The frontier reads both keys from the RAW JSON (never from
        // `Opportunity`, which stays byte-identical: PR #869 measured that
        // adding the fields breaks 15+ `Opportunity {` literals OUTSIDE this
        // crate, and that 15 is a lower bound) and splits the stream into two
        // sets. It runs BEFORE gate 1, gate 2 and every simulator on purpose: a
        // producer-rejected entry must not consume a rate permit, an in-flight
        // permit, a fork snapshot or an `eth_call`. The fork is the scarce
        // resource — diagnosing a reject must not spend it.
        //
        // The diagnosed set is NOT discarded: it reaches the SAME
        // persist -> XACK tail as every other terminal outcome, carrying its
        // typed reason on the `simulations` row, and every message is counted
        // in `arbx_sim_validated_frontier_total{outcome,reason}`. That counter
        // is what makes `passed = true = 0` interpretable instead of a blind
        // skip: "10.000 rejected, 0 eligible" is not "the market gave nothing".
        let admission = decision_frontier::admission(&decision_frontier::read_verdict(&json));
        // ONE call site for the telemetry of BOTH sets, before the branch, so
        // the two counts cannot drift apart.
        admission.record();
        if let Admission::DecisionLog { reason, family } = admission {
            bound.note_decision_log(Instant::now(), &id, &reason, family.label());
            let skip = recorded_skip(opportunity.id, &reason);
            return self.finish(&id, &opportunity, skip).await;
        }

        // ── SIMCTL-BOUND-01 gate 1: O(1) INLINE structural admissibility ──────
        // Evaluated on the message we ALREADY parsed: no I/O, no ordering, no
        // reading ahead. An entry that can never be simulated must not pay for
        // a PG round-trip, a fork snapshot or an `eth_call`.
        let now = Instant::now();
        let inadmissible = inadmissible_reason(&opportunity);

        // ── SIMCTL-BOUND-01 gate 2: policy rate + concurrency ceiling ────────
        // A bound is a DEFERRAL, never a verdict. This path persists nothing,
        // publishes nothing and — critically — does NOT XACK: the entry REMAINS
        // in the group PEL and `recover_stale_pending` (XAUTOCLAIM) redelivers
        // it when budget returns. An ACK here would silently DISCARD a real
        // opportunity; persisting a "throttled" verdict would claim an attempt
        // that never ran AND consume the opportunity forever. Neither is
        // acceptable, so the deferral reuses the recovery path that already
        // exists for transient infra errors. Cost: the redelivery floor is
        // `CLAIM_MIN_IDLE_MS` (120 s) and a standing backlog is visible in
        // `SIM_STREAM_PENDING_COUNT` / `SIM_STREAM_OLDEST_PENDING_MS`.
        let _permit = if inadmissible.is_none() {
            let permit = in_flight.clone().try_acquire_owned().ok();
            if permit.is_none() {
                bound.note_deferred(now, &id, "concurrency ceiling (max_in_flight) reached");
                return Ok(());
            }
            if !bound.try_admit(now) {
                bound.note_deferred(now, &id, "rate budget exhausted (max_sims_per_sec)");
                return Ok(());
            }
            permit
        } else {
            None
        };
        // `_permit` lives until the end of this function: the in-flight ceiling
        // is held across the simulate AND the persist below.

        // SIMWIRE-02: Canal B's source is the route-aware B2c REAL pipeline
        // when available; the legacy `SimulatorBackend` otherwise (anvil
        // default). Both branches converge on the same persist/XADD/ACK tail.
        if let Some(reason) = inadmissible {
            // Terminal and RECORDED. The row carries the typed reason, so the
            // entry is explained in `simulations` instead of vanishing.
            //
            // Deliberately NOT counted in SIMULATIONS_TOTAL: that metric counts
            // simulation ATTEMPTS, and there was none here. Inflating it would
            // corrupt the very measurement this bound exists to protect.
            bound.note_inadmissible(now, &id, reason.as_str());
            let gap = counted_gap(opportunity.id, &reason);
            return self.finish(&id, &opportunity, gap).await;
        }

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
                //
                // SIM4-CYCLIC-02 (cierre de N1): la ruta de recorrido se lee del
                // `route_metadata` que el searcher YA persiste, con el lector que
                // YA existe (`route_lookup::fetch_candidate_inputs`, el mismo que
                // el camino B2c usa más abajo), y se le pasa al backend. Sin esto
                // una ruta CERRADA (token_in == token_out) no tiene forma de
                // llegar a `build_probe_with_path` y sigue cayendo en la negativa.
                //
                // Fail-honest y best-effort a la vez: si la lectura falla o no hay
                // fila se pasa una ruta VACÍA — NUNCA una sonda fabricada. Con
                // ruta vacía, una oportunidad NO cíclica se comporta EXACTAMENTE
                // como antes, y una CERRADA falla con su motivo TIPADO
                // (`cyclic_route_missing_route_metadata:<kind>`), jamás con un
                // `passed` silencioso.
                let route_path: Vec<ethers::types::Address> =
                    match route_lookup::fetch_candidate_inputs(&self.pool, opportunity.id).await {
                        Ok(Some(inputs)) => inputs
                            .route_metadata
                            .token_addresses
                            .iter()
                            .filter_map(|s| s.parse::<ethers::types::Address>().ok())
                            .collect(),
                        Ok(None) => Vec::new(),
                        Err(e) => {
                            warn!(event = "sim_consumer.route_metadata_read_err", id = %id, error = %e);
                            Vec::new()
                        }
                    };
                let sim = match self
                    .backend
                    .simulate_with_route(&opportunity, &route_path)
                    .await
                {
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

        self.finish(&id, &opportunity, sim).await
    }

    /// The shared persist → publish → ACK tail.
    ///
    /// SIMCTL-BOUND-01 extracted this from `process_message` for one reason:
    /// the terminal inadmissible path must reach the SAME tail instead of
    /// duplicating it. One place where a verdict is persisted and
    /// acknowledged, one place where a persist failure means "no ACK, retry on
    /// the next iteration". Behaviour is unchanged for every pre-existing
    /// caller — the body below is the original, moved verbatim.
    async fn finish(
        &mut self,
        id: &str,
        opportunity: &Opportunity,
        sim: SimulationResult,
    ) -> Result<()> {
        // Persist; if it fails, do NOT ack — retry on next iteration.
        // SIMWIRE-02c P1-5: `insert_simulation` returns Ok(false) when this
        // (opportunity_id, simulator='revm') verdict was already persisted
        // by a PRIOR delivery whose final XACK failed (XAUTOCLAIM
        // redelivery). The verdict row is authoritative; skip the
        // downstream XADD too — republishing would double-count the
        // opportunity downstream (double paper-trade). The XACK below still
        // runs: this delivery's job is done, the entry must leave the PEL.
        let inserted_fresh = match insert_simulation(&self.pool, &sim).await {
            Ok(fresh) => fresh,
            Err(e) => {
                error!(event = "sim_consumer.persist_err", id=%id, error=%e);
                return Ok(());
            }
        };
        if !inserted_fresh {
            info!(event = "sim_consumer.redelivery_dedup", id=%id,
                  "revm verdict already persisted by a prior delivery — skipping duplicate publish");
        }

        if sim.passed && inserted_fresh {
            let payload = serde_json::to_string(opportunity).unwrap_or_default();
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
            .xack::<_, _, &str, ()>(STREAM_IN, GROUP, &[id])
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
        // 0) Canonical carrier FIRST (Issue #567): the producer's validated
        //    plan is the route of record. Recover it by opportunity id and
        //    re-simulate the FULL cycle — never reconstruct a cyclic route
        //    from token_in/token_out (the single-swap probe CANNOT represent
        //    token_in == token_out; that is the 712/1000 `not_implemented/
        //    strategy_cyclic_route_not_simulatable_in_s4` class). Absent or
        //    expired carrier → fall through to the reconstruction path below,
        //    unchanged (strictly additive).
        {
            let mut carrier_redis = self.redis.clone();
            match canonical_plan_consumer::fetch(&mut carrier_redis, opp.id).await {
                Ok(Some(plan)) => {
                    info!(event = "sim_consumer.canonical_plan_hit", id = %entry_id);
                    return canonical_plan_consumer::resimulate(b2c, opp, plan).await;
                }
                Ok(None) => {
                    // Normal outside the 300s TTL window or when the producer
                    // never reached SIM_SUCCESS — reconstruct below.
                }
                Err(err @ canonical_plan_consumer::FetchError::Redis(_)) => {
                    // Transient infra — PEL retry, never a market verdict.
                    return Err(err.as_fail_reason());
                }
                Err(err @ canonical_plan_consumer::FetchError::Parse(_)) => {
                    // Terminal, honest: typed gap keeps the opportunity
                    // non-rejected while the simulations row records why.
                    return Ok(counted_gap(opp.id, &err.as_fail_reason()));
                }
            }
        }

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
            // SIMWIRE-02c P1-7: missing decimals are HEALABLE — the
            // token-enricher backfills tokens.decimals asynchronously, so a
            // later redelivery CAN complete this candidate. Transient Err
            // (PEL retry), never a terminal persisted gap.
            return Err(format!("candidate_decimals_pending:{missing:?}"));
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
        // the trading path defensive (no unwrap). Same P1-7 semantics as
        // validate_complete above: healable by the enricher → PEL retry.
        let decimals_in = match inputs.resolved_decimals.get(&token_addresses[0]) {
            Some(d) => d,
            None => {
                return Err(format!(
                    "candidate_decimals_pending:token_in_{}",
                    token_addresses[0]
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

        // 4) FlashLoanExecutor readiness (SIMWIRE-02c P1-3, fail-closed):
        //    `execute_multistep_revm` resolves FLASHLOAN_EXECUTOR_<chain_id>
        //    at step 0; with the env var absent EVERY simulation would fail
        //    with `multistep_flashloan_executor_unresolved` — an operator
        //    env config gap, not a market verdict. Refuse as TRANSIENT
        //    (PEL): the operator setting the env var heals this entry via
        //    XAUTOCLAIM redelivery. Boot (`flashloan_executor_boot_ready`)
        //    warns about the same gap before the consumer even starts.
        if let Err(e) = shared_rs::chains::resolve_flashloan_executor_address(candidate.chain_id) {
            return Err(format!("flashloan_executor_unresolved: {e}"));
        }

        // 5) Live gas price — transient: a missing/zero gas oracle must
        //    retry via the PEL, never become a rejection.
        let gas_price_wei = crate::read_gas_price(&b2c.gas_redis, candidate.chain_id).await?;

        // 6) Block-pinned replay (SIMWIRE-02c P1-4): pin the simulator to
        //    the candidate's detection block — same determinism as the A3
        //    HTTP path — so the LazyDb inside execute_multistep_revm reads
        //    detection-time state, not `latest`.
        let pinned_sim = simulator_for_candidate(&b2c.simulator, candidate.block_number);

        // 7) REAL multi-step REVM simulation (same encoder as the searcher).
        let outcome = run_real_simulation(candidate, pinned_sim, &b2c.env, gas_price_wei).await;

        // 8) REVM state-fetch infra failures are TRANSIENT — a dead/slow RPC
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

        // 9) Translate → SimulationResult. PRICES-FREE by design: net-USD is
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

/// DL-02 / SIMCTL-VERDICT-01: a typed SKIP that is recorded but is NOT a
/// simulation attempt.
///
/// The producer's own verdict kept this message out of the simulation path, so
/// no attempt ran and `SIMULATIONS_TOTAL` must not be inflated with it — that
/// counter is the admissible side of the very measurement this change exists to
/// make interpretable ("10.000 rejects are not 10.000 simulations"). The row
/// still lands in `simulations` so the entry is EXPLAINED instead of vanishing,
/// and `persistence::is_sim_capability_gap` classifies the `producer_verdict:`
/// family as non-rejecting, so `opportunities.status` / `rejection_reason`
/// stay owned by the producer that already wrote them.
pub(crate) fn recorded_skip(opportunity_id: Uuid, reason: &str) -> SimulationResult {
    SimulationResult {
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
    }
}

/// Typed-gap SimulationResult: `passed=false` with a fail_reason that
/// `is_sim_capability_gap` classifies as absence-of-capability — the
/// opportunity stays non-rejected (status detected/validated,
/// rejection_reason NULL) while the simulations row records the skip
/// honestly. Counted in SIMULATIONS_TOTAL because the attempt really ran.
pub(crate) fn counted_gap(opportunity_id: Uuid, reason: &str) -> SimulationResult {
    let r = recorded_skip(opportunity_id, reason);
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

/// SIMWIRE-02c P1-1: pure ghost classification, testable without a DB.
///
/// The `.ok().flatten()` conflation this replaces treated a PG ERROR the
/// same as a confirmed-missing row — an outage dead-lettered entries whose
/// opportunities rows were alive (XACK + drop on an infrastructure fault).
/// Fail-honest split (R8):
/// - `Ok(Some(_))` → row confirmed alive → NOT a ghost.
/// - `Ok(None)` → row CONFIRMED missing (retention purge) → ghost: the simulations FK can never satisfy, PEL retry is futile, dead-letter is correct.
/// - `Err(_)` → unknown → NOT a ghost: the entry keeps its honest PEL-retry path; a transient outage must never silently drop live work.
#[derive(Debug, PartialEq, Eq)]
pub enum GhostVerdict {
    NotGhost,
    RowMissing,
}

pub fn ghost_verdict(lookup: Result<Option<i32>, sqlx::Error>) -> GhostVerdict {
    match lookup {
        Ok(Some(_)) | Err(_) => GhostVerdict::NotGhost,
        Ok(None) => GhostVerdict::RowMissing,
    }
}

/// SIMWIRE-02c P1-4: deterministic replay pinning. The consumer's shared
/// simulator carries whatever pin boot gave it (typically none — `latest`);
/// the candidate carries the block the searcher actually detected on. Mirror
/// the A3 HTTP path (main.rs): when the candidate names a block, build a
/// cheap dedicated simulator pinned to THAT block, so the LazyDb built inside
/// `execute_multistep_revm` resolves state at detection time instead of
/// `latest`. `SimulatorV2::new` is cheap (LazyDb is created per execute
/// call); the shared simulator is reused untouched when no pin applies.
pub fn simulator_for_candidate(
    shared: &Arc<simulator_v2::SimulatorV2>,
    block: Option<u64>,
) -> Arc<simulator_v2::SimulatorV2> {
    match block {
        Some(b) => Arc::new(simulator_v2::SimulatorV2::new(shared.rpc_url.clone()).with_block(b)),
        None => Arc::clone(shared),
    }
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

#[cfg(test)]
mod simwire02c_tests {
    use super::{ghost_verdict, simulator_for_candidate, GhostVerdict};
    use std::sync::Arc;

    /// P1-1 matrix, first row: a DB ERROR must never classify as ghost —
    /// the row may exist, and an outage dead-lettering live entries is the
    /// exact `.ok().flatten()` defect this patch closes.
    #[test]
    fn db_down_is_not_a_ghost() {
        let db_err = sqlx::Error::Io(std::io::Error::other("connection refused"));
        assert_eq!(ghost_verdict(Err(db_err)), GhostVerdict::NotGhost);
    }

    /// Only a CONFIRMED missing row (Ok(None)) is a ghost: the simulations
    /// FK can never satisfy, so PEL retry is futile and dead-letter is right.
    #[test]
    fn confirmed_missing_row_is_the_only_ghost() {
        assert_eq!(ghost_verdict(Ok(None)), GhostVerdict::RowMissing);
    }

    #[test]
    fn existing_row_is_not_a_ghost() {
        assert_eq!(ghost_verdict(Ok(Some(1))), GhostVerdict::NotGhost);
    }

    /// P1-4: when the candidate names a detection block, the replay
    /// simulator must be pinned to THAT block — not the shared boot pin
    /// (typically none → `latest`).
    #[test]
    fn replay_is_pinned_to_the_candidate_block() {
        let shared = Arc::new(simulator_v2::SimulatorV2::new("http://127.0.0.1:8545"));
        assert_eq!(shared.pinned_block(), None, "precondition: shared unpinned");
        let pinned = simulator_for_candidate(&shared, Some(21_000_042));
        assert_eq!(pinned.pinned_block(), Some(21_000_042));
        // The shared simulator is never mutated by the pin decision:
        assert_eq!(shared.pinned_block(), None);
    }

    /// P1-4 fallback: no candidate block → reuse the shared simulator as-is
    /// (same allocation, no throwaway clone with different semantics).
    #[test]
    fn unpinned_candidate_reuses_shared_simulator() {
        let shared = Arc::new(simulator_v2::SimulatorV2::new("http://127.0.0.1:8545"));
        let out = simulator_for_candidate(&shared, None);
        assert!(Arc::ptr_eq(&out, &shared));
    }
}

/// SIMCTL-BOUND-01 — the bound's own tests.
///
/// These pin the BEHAVIOUR and the two REFUTED alternatives, so a future
/// "simplification" back to `f64::max` or to a raw `partial_cmp().unwrap()`
/// fails loudly HERE instead of panicking in production.
#[cfg(test)]
mod simctl_bound_01_tests {
    use super::{
        inadmissible_reason, key_of_amount_in_wei, read_batch_size, read_in_flight_env,
        read_rate_env, sanitize_key, BoundPolicy, StreamBound, DEFAULT_MAX_IN_FLIGHT,
        DEFAULT_MAX_SIMS_PER_SEC,
    };
    use shared_rs::contracts::{Opportunity, StrategyKind};
    use std::time::{Duration, Instant};

    /// The message exactly as it arrives on `arbx:opps:validated`.
    fn msg(amount_in_wei: &str) -> Opportunity {
        Opportunity {
            id: uuid::Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::dex_arb(),
            dex_a: "uniswap_v3".into(),
            dex_b: None,
            pair_symbol: "WETH/USDC".into(),
            token_in: "0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2".into(),
            token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into(),
            amount_in_wei: amount_in_wei.into(),
            expected_profit_usd: Some(10.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: chrono::Utc::now(),
            trace_id: uuid::Uuid::new_v4(),
            economics: None,
        }
    }

    fn policy(rate: f64, in_flight: usize) -> BoundPolicy {
        BoundPolicy {
            max_sims_per_sec: rate,
            max_in_flight: in_flight,
            rate_from_env: true,
            in_flight_from_env: true,
        }
    }

    // ── U2 — the sanitisation, and the two REFUTED substitutes ────────────

    #[test]
    fn sanitize_key_maps_non_finite_to_the_worst_key_and_never_to_a_zero() {
        assert_eq!(sanitize_key(f64::NAN), f64::NEG_INFINITY);
        assert_eq!(sanitize_key(f64::INFINITY), f64::NEG_INFINITY);
        assert_eq!(sanitize_key(f64::NEG_INFINITY), f64::NEG_INFINITY);
        // Finite values pass through UNTOUCHED — including a real zero, which
        // must stay a real zero (it is an observation, not a sanitisation).
        assert_eq!(sanitize_key(0.0), 0.0);
        assert_eq!(sanitize_key(-0.1), -0.1);
        assert_eq!(sanitize_key(1.5), 1.5);
        // The sanitised key orders TOTAL: the comparison returns Some, which is
        // what makes an `unwrap()` over it safe. The RAW key does not (next).
        assert!(sanitize_key(f64::NAN).partial_cmp(&-0.1).is_some());
    }

    #[test]
    fn refuted_max_substitute_would_fabricate_a_measured_zero() {
        // WHY `f64::max(key, 0.0)` is forbidden as a sanitiser: it returns a
        // zero indistinguishable from a MEASURED zero — the exact R8 failure
        // mode ("Some(0.0) = computed and exactly zero").
        assert_eq!(f64::max(f64::NAN, 0.0), 0.0);
        assert_ne!(sanitize_key(f64::NAN), f64::max(f64::NAN, 0.0));
        // And the disagreement is not cosmetic: max() would ADMIT the NaN.
        assert!(f64::max(f64::NAN, 0.0) >= 0.0, "max() would admit the NaN");
        assert!(
            !(sanitize_key(f64::NAN) > 0.0),
            "the sanitised key rejects it"
        );
    }

    #[test]
    fn refuted_unwrap_on_the_raw_key_is_none_and_would_panic() {
        // `NaN.partial_cmp(-0.1)` is None — so `.unwrap()` PANICS, and inside
        // the consumer loop that kills consumption for the whole group.
        assert!(f64::NAN.partial_cmp(&-0.1).is_none());
        assert!(sanitize_key(f64::NAN).partial_cmp(&-0.1).is_some());
        // The shape that must never appear (a sort over raw keys) is total only
        // after sanitising — pinned here so the trap stays visible:
        assert!(f64::NAN.partial_cmp(&f64::NAN).is_none());
    }

    // ── U3 / U4 — admissibility: structural only, never economic ──────────

    #[test]
    fn zero_and_unparseable_amounts_are_inadmissible_and_stay_capability_gaps() {
        for raw in ["0", "0 ", "", "   ", "abc", "-1", "1e18", "0x10"] {
            let reason = inadmissible_reason(&msg(raw))
                .unwrap_or_else(|| panic!("{raw:?} must be inadmissible"));
            assert!(
                reason.starts_with("candidate_incomplete"),
                "{raw:?} -> {reason} must stay inside the candidate_incomplete family, \
                 so persistence keeps it a NON-rejecting capability gap"
            );
        }
        assert_eq!(
            inadmissible_reason(&msg("0")).as_deref(),
            Some("candidate_incomplete:amount_in_wei_zero")
        );
        assert_eq!(
            inadmissible_reason(&msg("nope")).as_deref(),
            Some("candidate_incomplete:amount_in_wei_unparseable")
        );
    }

    #[test]
    fn a_real_amount_is_admissible() {
        assert_eq!(inadmissible_reason(&msg("1000000000000000000")), None);
        assert_eq!(inadmissible_reason(&msg("1")), None);
        // Surrounding whitespace is the wire's business, not a defect:
        assert_eq!(inadmissible_reason(&msg("  1000000000000000000  ")), None);
        // u128::MAX stays finite as f64, so it is admissible.
        assert_eq!(inadmissible_reason(&msg(&u128::MAX.to_string())), None);
    }

    #[test]
    fn predicate_ignores_profitability_entirely() {
        // U4: a huge profit does NOT buy an inadmissible entry past the gate...
        let mut rich_but_empty = msg("0");
        rich_but_empty.expected_profit_usd = Some(999_999.0);
        rich_but_empty.net_expected_profit_usd = Some(888_888.0);
        rich_but_empty.roi_pct = Some(1_000.0);
        assert!(
            inadmissible_reason(&rich_but_empty).is_some(),
            "an economic field must never buy an entry past the structural gate"
        );
        // ...and a zero/negative profit does NOT make an admissible one
        // inadmissible: an economic threshold here would pin `passed=true` at
        // zero forever and blind us exactly when simulation can execute.
        let mut poor_but_real = msg("1000000000000000000");
        poor_but_real.expected_profit_usd = Some(0.0);
        poor_but_real.net_expected_profit_usd = Some(-5.0);
        poor_but_real.roi_pct = Some(-100.0);
        assert_eq!(inadmissible_reason(&poor_but_real), None);
    }

    #[test]
    fn verdict_depends_only_on_the_amount_key_not_on_route_or_strategy() {
        // O(1) means: no global view, no ordering, no dependence on anything but
        // the message's own amount. Two messages identical except for route and
        // strategy must get the SAME verdict.
        let a = msg("1000000000000000000");
        let mut b = a.clone();
        b.strategy_kind = StrategyKind::triangular();
        b.dex_a = "curve".into();
        b.pair_symbol = "USDC/DAI".into();
        b.token_out = b.token_in.clone(); // a CLOSED route — still irrelevant here
        assert_eq!(inadmissible_reason(&a), inadmissible_reason(&b));
    }

    #[test]
    fn key_of_amount_in_wei_sanitises_the_unparseable_to_the_worst_key() {
        assert_eq!(key_of_amount_in_wei("1"), 1.0);
        assert_eq!(key_of_amount_in_wei("0"), 0.0);
        assert_eq!(key_of_amount_in_wei("not-a-number"), f64::NEG_INFINITY);
        assert_eq!(key_of_amount_in_wei(""), f64::NEG_INFINITY);
        // The sanitised key is NEVER NaN — that is the whole point. It may be
        // NEG_INFINITY, which is an ordering VALUE (unlike NaN), and that is
        // exactly what makes every comparison over it TOTAL. (Asserting
        // `is_finite()` here would be wrong: NEG_INFINITY is not finite.)
        assert!(!key_of_amount_in_wei("not-a-number").is_nan());
        assert!(key_of_amount_in_wei("not-a-number") < 0.0);
    }

    // ── U1 — O(1) pacing, deterministic, no ordering ──────────────────────

    #[test]
    fn budget_admits_at_the_policy_rate_and_defers_everything_else() {
        let t0 = Instant::now();
        let mut b = StreamBound::new(policy(1.0, 1), t0);
        assert!(b.try_admit(t0), "the first entry passes immediately");
        assert_eq!(b.admitted, 1);
        // Anything within the 1 s spacing is deferred — not dropped, and not
        // counted as admitted.
        assert!(!b.try_admit(t0 + Duration::from_millis(1)));
        assert!(!b.try_admit(t0 + Duration::from_millis(999)));
        assert_eq!(b.admitted, 1);
        assert!(b.try_admit(t0 + Duration::from_secs(1)));
        assert!(!b.try_admit(t0 + Duration::from_secs(1) + Duration::from_millis(1)));
        assert!(b.try_admit(t0 + Duration::from_secs(2)));
        assert_eq!(b.admitted, 3);
    }

    #[test]
    fn deferral_never_promotes_itself_and_the_counters_stay_separate() {
        let t0 = Instant::now();
        let mut b = StreamBound::new(policy(1.0, 1), t0);
        assert!(b.try_admit(t0));
        for i in 0..5 {
            assert!(!b.try_admit(t0 + Duration::from_millis(i)));
            b.note_deferred(t0 + Duration::from_millis(i), "1-0", "test");
        }
        assert_eq!(b.admitted, 1, "a deferral must never consume a slot");
        assert_eq!(b.deferred, 5);
        b.note_inadmissible(t0, "1-1", "candidate_incomplete:amount_in_wei_zero");
        assert_eq!(b.terminated_inadmissible, 1);
        assert_eq!(b.deferred, 5, "terminations are not deferrals");
    }

    #[test]
    fn an_unusable_policy_rate_cannot_panic_the_process() {
        let t0 = Instant::now();
        // `Duration::from_secs_f64` PANICS on NaN / inf / <= 0. The constructor
        // must absorb that: a panicking consumer is worse than a bound that fell
        // back to its conservative default.
        for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let b = StreamBound::new(policy(bad, 1), t0);
            assert_eq!(b.policy.max_sims_per_sec, DEFAULT_MAX_SIMS_PER_SEC);
        }
        // Same for a nonsensical in-flight ceiling:
        let b = StreamBound::new(policy(1.0, 0), t0);
        assert_eq!(b.policy.max_in_flight, DEFAULT_MAX_IN_FLIGHT);
    }

    #[test]
    fn default_rate_sits_above_the_sequential_ceiling_and_is_not_the_row_rate() {
        // U9: the default is NOT derived from the ~68 rows/s — that rate counts
        // rows that die BEFORE the fork. It must be strictly ABOVE what a single
        // sequential consumer can physically reach at the measured ~2.084 s per
        // simulation (1/2.084 ≈ 0.48/s), so it does not throttle real work...
        const MEASURED_PIPELINE_LATENCY_S: f64 = 2.084;
        assert!(
            DEFAULT_MAX_SIMS_PER_SEC > 1.0 / MEASURED_PIPELINE_LATENCY_S,
            "the cap must not bind the sequential ceiling"
        );
        // ...and it must be nowhere near the unmeasured row rate:
        assert!(DEFAULT_MAX_SIMS_PER_SEC < 68.0 / 10.0);
        assert_eq!(DEFAULT_MAX_IN_FLIGHT, 1, "conservative = today's reality");
    }

    // ── U5 — env knobs: unusable values are ERRORS, never silent overrides ─

    #[test]
    fn unusable_env_values_are_errors_and_absent_ones_are_not() {
        // Unique names: cargo runs these in parallel threads, and touching a
        // name another test also uses would race it.
        let absent = "SIMCTL_T97_RATE_ABSENT";
        std::env::remove_var(absent);
        assert_eq!(read_rate_env(absent), Ok(None), "absent -> default applies");

        let blank = "SIMCTL_T97_RATE_BLANK";
        std::env::set_var(blank, "   ");
        assert_eq!(read_rate_env(blank), Ok(None));

        let bad = "SIMCTL_T97_RATE_BAD";
        for v in ["0", "-1", "abc", "NaN", "inf", "-inf", "1e999"] {
            std::env::set_var(bad, v);
            let got = read_rate_env(bad);
            assert!(
                got.is_err(),
                "{bad}={v:?} must be an ERROR (default in force + a WARN), \
                 not a silent override; got {got:?}"
            );
        }
        let good = "SIMCTL_T97_RATE_GOOD";
        std::env::set_var(good, "2.5");
        assert_eq!(read_rate_env(good), Ok(Some(2.5)));
        std::env::remove_var(good);

        let inflight = "SIMCTL_T97_INFLIGHT";
        for v in ["0", "-3", "x", "1.5"] {
            std::env::set_var(inflight, v);
            assert!(
                read_in_flight_env(inflight).is_err(),
                "{v:?} must be rejected"
            );
        }
        std::env::set_var(inflight, "4");
        assert_eq!(read_in_flight_env(inflight), Ok(Some(4)));
        std::env::remove_var(inflight);
    }

    /// SIMCTL-THROUGHPUT-01: el `COUNT` de lectura debe ser la cota de lo que el
    /// pase puede ADMITIR, no un número fijo.
    ///
    /// El defecto medido: `COUNT 8` contra `max_in_flight = 1` — el default
    /// (`DEFAULT_MAX_IN_FLIGHT`) y el valor realmente en vigor, porque el
    /// contenedor no define `SIMCTL_MAX_IN_FLIGHT`. De cada lote de 8, siete
    /// entradas caían en `note_deferred`, y son justamente las ADMISIBLES: el
    /// permiso se pide sólo cuando `inadmissible.is_none()`, así que las
    /// inadmisibles nunca lo piden y se procesan siempre.
    ///
    /// Medido en producción (`sim_ctl.bound_report`, arranque 02:51:25Z):
    /// `admitted 243` contra `deferred 103604` = 426:1, con
    /// `arbx_sim_stream_claimed_count 67514` de churn de PEL.
    #[test]
    fn read_batch_size_follows_the_admission_budget_not_a_fixed_eight() {
        // El caso DESPLEGADO. Pedir 8 aquí era exactamente el defecto.
        assert_eq!(
            read_batch_size(DEFAULT_MAX_IN_FLIGHT),
            1,
            "con el default desplegado (max_in_flight=1) el pase debe pedir 1"
        );
        assert_ne!(
            read_batch_size(DEFAULT_MAX_IN_FLIGHT),
            8,
            "el 8 fijo es el defecto que esta tarea corrige"
        );

        // CONTROL: no está hardcodeado a 1 — sigue al presupuesto.
        assert_eq!(read_batch_size(4), 4);
        assert_eq!(read_batch_size(16), 16);

        // Y NUNCA 0: con el semáforo agotado se sigue pidiendo 1, para que el
        // bucle avance en vez de quedarse quieto hasta el próximo tick.
        assert_eq!(read_batch_size(0), 1);
    }

    /// CONTROL del test de arriba. Sin esto, aquel pasaría igual si la función
    /// devolviera una constante. La propiedad que hace legítimo alinear el
    /// `COUNT` con el presupuesto es la MONOTONÍA no decreciente: alinear nunca
    /// puede REDUCIR el trabajo hecho por pase, sólo evitar el sobrante.
    #[test]
    fn read_batch_size_is_monotone_in_the_budget() {
        let mut prev = read_batch_size(0);
        for n in 1..=64usize {
            let cur = read_batch_size(n);
            assert!(
                cur >= prev,
                "no decreciente: {prev} -> {cur} con presupuesto {n}"
            );
            prev = cur;
        }
        // El techo degenerado: nunca menos de 1 y nunca más que el presupuesto.
        for n in 0..=64usize {
            let b = read_batch_size(n);
            assert!(b >= 1, "nunca 0 con presupuesto {n}");
            assert!(b <= n.max(1), "nunca más que el presupuesto ({n})");
        }
    }
}

/// DL-02 / SIMCTL-VERDICT-01: the two SimulationResult builders must stay
/// distinguishable, because one of them is what makes `passed = true = 0`
/// readable. `recorded_skip` explains a message WITHOUT inflating the attempt
/// counter; `counted_gap` still counts, because a structural gate really ran.
///
/// Named for the COUNTER, not for the frontier's placement: the placement
/// invariant lives in `decision_frontier` (lib target), where it can run under
/// CI's blocking `cargo test --workspace --locked --lib` gate.
#[cfg(test)]
mod dl02_skip_counter_tests {
    use super::{counted_gap, recorded_skip};
    use shared_rs::metrics::SIMULATIONS_TOTAL;

    fn attempts() -> u64 {
        SIMULATIONS_TOTAL
            .with_label_values(&["revm", "false"])
            .get()
    }

    #[test]
    fn a_recorded_skip_is_not_a_simulation_attempt_but_a_gap_still_is() {
        // Both halves live in ONE test on purpose: `SIMULATIONS_TOTAL` is
        // process-global and the harness runs tests in parallel, so splitting
        // the "does not count" assertion from the "does count" control would
        // let the control's increment land inside the other's window. No other
        // test in this crate writes the `["revm","false"]` series.
        let id = uuid::Uuid::new_v4();
        let before = attempts();
        let skip = recorded_skip(id, "producer_verdict:reject:producer_rejected");

        assert!(!skip.passed, "a skip is never a pass");
        assert_eq!(skip.opportunity_id, id);
        assert_eq!(
            skip.fail_reason.as_deref(),
            Some("producer_verdict:reject:producer_rejected"),
            "the typed reason is what explains the entry on the simulations row"
        );
        assert_eq!(
            attempts(),
            before,
            "a producer-rejected message is not a simulation attempt — counting it would \
             re-corrupt the very measurement this change protects"
        );

        // CONTROL: the same builder path DOES move the counter when a structural
        // gate ran, so the equality above is not a vacuous pass over a metric
        // that never moves.
        let gap = counted_gap(
            uuid::Uuid::new_v4(),
            "candidate_incomplete:amount_in_wei_zero",
        );
        assert!(!gap.passed);
        assert!(
            attempts() > before,
            "counted_gap must keep counting attempts, or the skip/attempt split is meaningless"
        );
    }
}
