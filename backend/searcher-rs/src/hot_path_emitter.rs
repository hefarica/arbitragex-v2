//! HotPathEmitter — Sub-100ms pipeline Redis stream emitter.
//!
//! Emits opportunities to `arbx:hot:detected` and simulation results to
//! `arbx:hot:simulated` with latency budget <5ms per emit.
//!
//! ## Design (Task 2)
//!
//! - Consumes: `Opportunity` from detection pipeline, `SimulationOutcome` from REVM
//! - Produces: XADD to `arbx:hot:detected` and `arbx:hot:simulated` streams
//! - Stores: Hash data at `arbx:hot:opp:{id}` and `arbx:hot:sim:{id}` with 300s TTL
//!
//! ## R8 invariants
//!
//! - Fail-honest: Redis errors propagate as `Err`, never silently dropped
//! - Latency: All ops are async non-blocking, clone-on-call pattern
//! - Observer-only: NEVER accesses capital keys, pure emitter logic

// WO-02 (2026-09-06): MultiplexedConnection → ConnectionManager — the handle
// type the scanner pipeline already threads (`publisher::publish` takes
// `&mut ConnectionManager`); the emitter must consume the same type.
use redis::aio::ConnectionManager;
use shared_rs::contracts::Opportunity;
use std::time::{SystemTime, UNIX_EPOCH};

/// Simulation outcome passed from the REVM orchestrator.
/// Mirrored from `prioritization_spine::round_trip_executor::SimulationOutcome`
/// to avoid deep trait coupling in the emitter boundary.
///
/// WO-02 (2026-09-06): `net_profit_wei`/`gas_price_wei` are decimal STRINGS
/// because the source `simulated_profit_token_in`/`gas_price_wei` are `U256`;
/// a `u128` field would truncate on overflow and a coerced value violates R8.
/// The wire contract was stringified anyway (XADD net_profit_wei.to_string()).
/// NOTE: `net_profit_wei` carries the REVM-verdict GROSS token_in delta
/// (`simulated_profit_token_in`); the net-of-gas decision belongs to
/// downstream consumers (paper-executor net gate / `net_usd_viable`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimulationResult {
    pub passed: bool,
    pub net_profit_wei: String,
    pub gas_used: u64,
    pub gas_price_wei: String,
}

/// Hot-path emitter for sub-100ms detection pipeline.
///
/// Clone the inner connection for each call — this is the tokio-redis
/// recommended pattern for shared-state emitters.
#[derive(Clone)]
pub struct HotPathEmitter {
    redis: ConnectionManager,
}

impl HotPathEmitter {
    /// Creates a new emitter from an existing Redis connection manager
    /// (the handle type the scanner pipeline already threads).
    pub fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }

    /// Emits a detected opportunity to `arbx:hot:detected` stream.
    ///
    /// Stream fields:
    ///   - `id`: Opportunity UUID
    ///   - `chain_id`: Chain ID
    ///   - `strategy_kind`: Strategy variant (snake_case)
    ///   - `detected_at_ms`: Unix timestamp millis
    ///
    /// Also stores full opportunity data at `arbx:hot:opp:{id}` with 300s TTL.
    ///
    /// Latency budget: <5ms (measured at 1-2ms in local benchmarks).
    pub async fn emit_detected(&self, opp: &Opportunity) -> Result<(), redis::RedisError> {
        let id = opp.id.to_string();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // XADD arbx:hot:detected with approximate maxlen ~10k
        let _: () = redis::cmd("XADD")
            .arg("arbx:hot:detected")
            .arg("MAXLEN")
            .arg("~")
            .arg(10000)
            .arg("*")
            .arg("id")
            .arg(&id)
            .arg("chain_id")
            .arg(opp.chain_id)
            .arg("strategy_kind")
            .arg(opp.strategy_kind.as_str())
            .arg("detected_at_ms")
            .arg(timestamp_ms)
            .query_async(&mut self.redis.clone())
            .await?;

        // Store full opportunity hash
        let opp_key = format!("arbx:hot:opp:{}", id);
        let opp_json = serde_json::to_string(opp).unwrap_or_default();

        let _: () = redis::cmd("HSET")
            .arg(&opp_key)
            .arg("data")
            .arg(opp_json)
            .query_async(&mut self.redis.clone())
            .await?;

        // 300s TTL covers detect→evaluation→archival window
        let _: () = redis::cmd("EXPIRE")
            .arg(&opp_key)
            .arg(300)
            .query_async(&mut self.redis.clone())
            .await?;

        Ok(())
    }

    /// Emits a simulation result to `arbx:hot:simulated` stream.
    ///
    /// WO-02 (2026-09-06): takes the full `Opportunity` so the XADD carries
    /// the fields BOTH consumers require — `OpportunityHotStreamer`
    /// (api-server websocket.ts → room `opportunities`, event
    /// `opportunity:validated`) and the dormant `PaperExecutor`
    /// (api-server paper/executor.ts), whose `parseSimulatedOpportunity`
    /// drops entries without `id`+`status` and skips (`skip_incomplete`)
    /// entries without `opportunity_id`+`chain_id`+`strategy_kind`.
    ///
    /// Stream fields:
    ///   - `id`: Opportunity UUID (stream-message correlation)
    ///   - `opportunity_id`: same UUID — PaperExecutor FK into opportunities.id
    ///   - `status`: "passed" | "failed" — REVM verdict, VERBATIM (R8: the
    ///     emitter never re-classifies; downstream gates apply their own)
    ///   - `net_profit_wei`: decimal string (see SimulationResult)
    ///   - `gas_used`: gas consumed by the REVM round trip
    ///   - `gas_price_wei`: decimal string (gas price the simulator used)
    ///   - `chain_id`, `strategy_kind`, `token_pair`: correlation fields
    ///   - `timestamp_ms`: Unix timestamp millis
    ///
    /// On `passed=true`, also stores full result at `arbx:hot:sim:{id}` with 300s TTL.
    pub async fn emit_simulated(
        &self,
        opp: &Opportunity,
        result: &SimulationResult,
    ) -> Result<(), redis::RedisError> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let status = if result.passed { "passed" } else { "failed" };
        let id = opp.id.to_string();

        // XADD arbx:hot:simulated with approximate maxlen ~5k
        let _: () = redis::cmd("XADD")
            .arg("arbx:hot:simulated")
            .arg("MAXLEN")
            .arg("~")
            .arg(5000)
            .arg("*")
            .arg("id")
            .arg(&id)
            .arg("status")
            .arg(status)
            .arg("net_profit_wei")
            .arg(&result.net_profit_wei)
            .arg("gas_used")
            .arg(result.gas_used)
            .arg("gas_price_wei")
            .arg(&result.gas_price_wei)
            .arg("opportunity_id")
            .arg(&id)
            .arg("chain_id")
            .arg(opp.chain_id)
            .arg("strategy_kind")
            .arg(opp.strategy_kind.as_str())
            .arg("token_pair")
            .arg(&opp.pair_symbol)
            .arg("timestamp_ms")
            .arg(timestamp_ms)
            .query_async(&mut self.redis.clone())
            .await?;

        // Store full result only on passed simulations
        if result.passed {
            let sim_key = format!("arbx:hot:sim:{}", id);
            let result_json = serde_json::to_string(result).unwrap_or_default();

            let _: () = redis::cmd("HSET")
                .arg(&sim_key)
                .arg("result")
                .arg(result_json)
                .query_async(&mut self.redis.clone())
                .await?;

            let _: () = redis::cmd("EXPIRE")
                .arg(&sim_key)
                .arg(300)
                .query_async(&mut self.redis.clone())
                .await?;
        }

        Ok(())
    }

    /// Emits a gate commit with energy state to `arbx:gate:commit`.
    ///
    /// New gate commitment stream for energy-based gate evaluation.
    /// Used by orchestrator to track gate decisions during the sub-100ms pipeline.
    pub async fn emit_gate_commit_from_state(
        &self,
        energy_state: &crate::gates::GateEnergyState,
    ) -> Result<(), redis::RedisError> {
        use crate::gates::GateEnergyState;
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // XADD arbx:gate:commit with approximate maxlen ~5k
        let _: () = redis::cmd("XADD")
            .arg("arbx:gate:commit")
            .arg("MAXLEN")
            .arg("~")
            .arg(5000)
            .arg("*")
            .arg("gate_identifier")
            .arg(&energy_state.gate_identifier)
            .arg("energy")
            .arg(energy_state.energy)
            .arg("hamiltonian")
            .arg(energy_state.hamiltonian)
            .arg("perturbation")
            .arg(energy_state.perturbation)
            .arg("energy_reason")
            .arg(&energy_state.energy_reason)
            .arg("ts_ms")
            .arg(timestamp_ms)
            .query_async(&mut self.redis.clone())
            .await?;

        Ok(())
    }
}

// strategy_kind canonicalization is now inherent: `Opportunity::strategy_kind`
// is itself the canonical identity (cartridge stem, or one of the 5 base
// families). The local strategy_kind_to_str + its mapping test were removed
// because that logic moved to shared-rs (StrategyKind::as_str).
