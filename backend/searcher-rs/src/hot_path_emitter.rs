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

    /// Builds the detected-emit command set: XADD stream entry + HSET opp
    /// hash + EXPIRE 300s.
    ///
    /// WO-7 (PERF-STACK-2026-09-20): the three sequential `query_async`
    /// round trips collapse into ONE non-atomic pipeline (3 commands,
    /// 1 RTT). `.atomic()` is deliberately NOT used — without MULTI/EXEC
    /// the server executes commands as they arrive, so a mid-connection
    /// drop may leave any prefix applied, exactly as the sequential form
    /// did. Wire values are byte-identical (see `wo7_tests`).
    fn detected_pipeline(opp: &Opportunity, timestamp_ms: u64) -> redis::Pipeline {
        let id = opp.id.to_string();
        let opp_key = format!("arbx:hot:opp:{}", id);
        let opp_json = serde_json::to_string(opp).unwrap_or_default();

        let mut pipe = redis::pipe();
        pipe.cmd("XADD")
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
            .ignore();
        pipe.cmd("HSET")
            .arg(&opp_key)
            .arg("data")
            .arg(opp_json)
            .ignore();
        pipe.cmd("EXPIRE").arg(&opp_key).arg(300).ignore();
        pipe
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
    /// Latency budget: <5ms. WO-7: one pipeline RTT (was three sequential).
    pub async fn emit_detected(&self, opp: &Opportunity) -> Result<(), redis::RedisError> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // WO-7: XADD + HSET + EXPIRE in ONE pipeline round trip.
        let _: () = Self::detected_pipeline(opp, timestamp_ms)
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

#[cfg(test)]
mod wo7_tests {
    use super::*;
    use shared_rs::contracts::StrategyKind;
    use uuid::Uuid;

    fn fixture_opp() -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::triangular(),
            dex_a: "fixture_dex".into(),
            dex_b: None,
            pair_symbol: "A/B".into(),
            token_in: "0x1".into(),
            token_out: "0x2".into(),
            amount_in_wei: "1000".into(),
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
            trace_id: Uuid::new_v4(),
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// WO-7: exactly 3 commands, executed as ONE round trip.
    #[test]
    fn detected_pipeline_is_three_commands() {
        let opp = fixture_opp();
        let pipe = HotPathEmitter::detected_pipeline(&opp, 1_700_000_000_000u64);
        assert_eq!(pipe.cmd_iter().count(), 3, "3 commands, 1 RTT");
    }

    /// WO-7: NOT atomic — an atomic pipeline would prepend MULTI as the
    /// first RESP command. Its absence proves no MULTI/EXEC wrapping, so
    /// mid-connection-drop semantics match the old sequential form.
    #[test]
    fn detected_pipeline_is_not_atomic() {
        let opp = fixture_opp();
        let packed = HotPathEmitter::detected_pipeline(&opp, 0).get_packed_pipeline();
        assert!(
            !packed.starts_with(b"*1\r\n$5\r\nMULTI\r\n"),
            "pipeline must not open with MULTI"
        );
    }

    /// WO-7: wire contract byte-identical to the sequential form (stream
    /// key, MAXLEN ~10000, opp hash key, HSET data, EXPIRE 300, strategy
    /// kind, timestamp).
    #[test]
    fn detected_pipeline_carries_exact_wire_fields() {
        let opp = fixture_opp();
        let ts = 1_700_000_000_123u64;
        let packed = HotPathEmitter::detected_pipeline(&opp, ts).get_packed_pipeline();
        assert!(contains(&packed, b"XADD"));
        assert!(contains(&packed, b"arbx:hot:detected"));
        assert!(contains(&packed, b"MAXLEN"));
        assert!(contains(&packed, b"10000"));
        let key = format!("arbx:hot:opp:{}", opp.id).into_bytes();
        assert!(contains(&packed, &key));
        assert!(contains(&packed, b"HSET"));
        assert!(contains(&packed, b"EXPIRE"));
        assert!(contains(&packed, b"300"));
        let sk = opp.strategy_kind.as_str().as_bytes().to_vec();
        assert!(contains(&packed, &sk));
        assert!(contains(&packed, ts.to_string().as_bytes()));
    }
}
