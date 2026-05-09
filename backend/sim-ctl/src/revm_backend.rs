//! `RevmBackend` — wraps `simulator_v2::SimulatorV2` behind `SimulatorBackend`.
//!
//! ## Current state (Sprint 4, Tasks 4.2 + 4.3 pending)
//!
//! `SimulatorV2::simulate` deliberately returns `SimError::NotImplemented` until
//! the `lazy_db` (Task 4.2) and `revm_runner` (Task 4.3) sub-modules land.
//! `RevmBackend` translates that into an honest `SimulationResult { passed: false,
//! fail_reason: Some("revm_not_implemented_sprint4") }` — per R8 Fail-Honest.
//! No fabrication, no silent pass.
//!
//! ## Mempool state injection
//!
//! `simulator_v2::CandidateInput` has no mempool-snapshot field yet. Full mempool
//! injection (inject pending-tx state before REVM execution) is **flagged as
//! follow-up** — it requires Tasks 4.2/4.3 to land first, then a new field
//! `pending_txs: Vec<PendingTx>` in `CandidateInput`. This dispatch wires the
//! backend connection; mempool enrichment is a separate task.
//!
//! ## Backend selection
//!
//! Operator opts in via `SIM_BACKEND=revm`. Default remains `anvil`.

use async_trait::async_trait;
use chrono::Utc;
use shared_rs::contracts::{Opportunity, SimulationResult, SimulatorKind};
use simulator_v2::{CandidateInput, SimError, Simulator, SimulatorV2};
use std::sync::Arc;
use tracing::warn;

use crate::simulator_backend::{SimulationError, SimulatorBackend};

/// REVM-based simulator. Wraps `simulator_v2::SimulatorV2`.
///
/// The sync `Simulator::simulate` call is dispatched via `spawn_blocking` so
/// it never blocks the tokio executor — important for an HFT pipeline where
/// the async runtime must stay responsive.
pub struct RevmBackend {
    core: Arc<SimulatorV2>,
}

impl RevmBackend {
    /// Construct from the `REVM_RPC_URL` env var. Fails if the var is unset in
    /// non-development environments (no-hardcode doctrine).
    pub fn from_env() -> anyhow::Result<Self> {
        let rpc_url = std::env::var("REVM_RPC_URL").unwrap_or_default();
        if rpc_url.is_empty() {
            warn!(
                event = "revm_backend.no_rpc_url",
                "REVM_RPC_URL not set — RevmBackend constructed but RPC calls will fail \
                 until Tasks 4.2/4.3 land (SimulatorV2 is currently a stub)"
            );
        }
        Ok(Self {
            core: Arc::new(SimulatorV2::new(rpc_url)),
        })
    }
}

#[async_trait]
impl SimulatorBackend for RevmBackend {
    async fn simulate(&self, opp: &Opportunity) -> Result<SimulationResult, SimulationError> {
        let id = opp.id;
        let trace_id = opp.trace_id;

        // Translate Opportunity → CandidateInput.
        // from/to are 20-byte arrays; parse from hex strings.
        let from = parse_addr_bytes(&opp.token_in).unwrap_or([0u8; 20]);
        let to = parse_addr_bytes(&opp.token_out).unwrap_or([0u8; 20]);
        let value_wei: u128 = opp.amount_in_wei.parse().unwrap_or(0);

        let candidate = CandidateInput {
            chain_id: opp.chain_id,
            block_number: opp.block_number.unwrap_or(0),
            from,
            to,
            calldata: Vec::new(), // Tasks 4.2/4.3 will populate from route encoding
            value_wei,
        };

        // Dispatch sync REVM call off the tokio executor.
        let core = Arc::clone(&self.core);
        let result = tokio::task::spawn_blocking(move || core.simulate(&candidate))
            .await
            .map_err(|e| SimulationError::Infra(format!("spawn_blocking join: {e}")))?;

        Ok(translate_result(id, trace_id, result))
    }

    fn name(&self) -> &str {
        "revm-v2"
    }
}

/// Translate `simulator_v2` result → `shared_rs::SimulationResult`.
fn translate_result(
    opportunity_id: uuid::Uuid,
    trace_id: uuid::Uuid,
    result: Result<simulator_v2::SimResult, SimError>,
) -> SimulationResult {
    match result {
        Ok(sim) => {
            let passed = sim.net_profit_wei > 0;
            SimulationResult {
                opportunity_id,
                passed,
                gas_estimate_wei: Some(sim.gas_used.to_string()),
                gas_price_wei: None,
                slippage_pct: None,
                revert_risk_pct: if passed { Some(0.5) } else { Some(50.0) },
                simulated_profit_usd: None, // USD conversion deferred to Sprint 5
                simulator: SimulatorKind::Revm,
                fail_reason: if passed {
                    None
                } else {
                    Some("revm_net_profit_non_positive".into())
                },
                simulated_at: Utc::now(),
                trace_id,
            }
        }
        Err(SimError::NotImplemented) => {
            // R8 Fail-Honest: stub not yet implemented → honest not-implemented result.
            SimulationResult {
                opportunity_id,
                passed: false,
                gas_estimate_wei: None,
                gas_price_wei: None,
                slippage_pct: None,
                revert_risk_pct: None,
                simulated_profit_usd: None,
                simulator: SimulatorKind::Revm,
                fail_reason: Some("revm_not_implemented_sprint4".into()),
                simulated_at: Utc::now(),
                trace_id,
            }
        }
        Err(SimError::Reverted(reason)) => SimulationResult {
            opportunity_id,
            passed: false,
            gas_estimate_wei: None,
            gas_price_wei: None,
            slippage_pct: None,
            revert_risk_pct: Some(100.0),
            simulated_profit_usd: None,
            simulator: SimulatorKind::Revm,
            fail_reason: Some(format!("revm_reverted: {}", &reason[..reason.len().min(200)])),
            simulated_at: Utc::now(),
            trace_id,
        },
        Err(SimError::Provider(msg)) => SimulationResult {
            opportunity_id,
            passed: false,
            gas_estimate_wei: None,
            gas_price_wei: None,
            slippage_pct: None,
            revert_risk_pct: Some(100.0),
            simulated_profit_usd: None,
            simulator: SimulatorKind::Revm,
            fail_reason: Some(format!("revm_provider_error: {}", &msg[..msg.len().min(200)])),
            simulated_at: Utc::now(),
            trace_id,
        },
    }
}

/// Parse a `0x`-prefixed 40-char hex address string into 20 bytes.
/// Returns `None` (not zero-address) on parse failure so callers can decide.
fn parse_addr_bytes(s: &str) -> Option<[u8; 20]> {
    let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
    if cleaned.len() != 40 {
        return None;
    }
    let bytes = hex::decode(cleaned).ok()?;
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Some(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared_rs::contracts::StrategyKind;
    use uuid::Uuid;

    fn make_opp() -> shared_rs::contracts::Opportunity {
        shared_rs::contracts::Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::DexArb,
            dex_a: "uniswap-v2".into(),
            dex_b: None,
            pair_symbol: "WETH/USDC".into(),
            token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".into(),
            token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into(),
            amount_in_wei: "1000000000000000000".into(),
            expected_profit_usd: Some(5.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: Some(19_000_000),
            rejection_reason: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    /// REVM backend returns an honest not-implemented result (not a panic, not a
    /// fabricated pass) until Tasks 4.2/4.3 land.
    #[tokio::test]
    async fn test_revm_backend_basic_simulate() {
        std::env::set_var("REVM_RPC_URL", "http://localhost:8545");
        let backend = RevmBackend::from_env().expect("from_env");
        let opp = make_opp();
        let result = backend.simulate(&opp).await.expect("no infra error");

        assert!(!result.passed, "stub must not pass");
        assert_eq!(
            result.fail_reason.as_deref(),
            Some("revm_not_implemented_sprint4"),
            "should surface stub reason"
        );
        assert!(
            matches!(result.simulator, SimulatorKind::Revm),
            "simulator tag must be Revm"
        );
        assert_eq!(result.opportunity_id, opp.id);
    }

    #[test]
    fn test_translate_not_implemented() {
        let id = Uuid::new_v4();
        let tid = Uuid::new_v4();
        let r = translate_result(id, tid, Err(SimError::NotImplemented));
        assert!(!r.passed);
        assert_eq!(r.fail_reason.as_deref(), Some("revm_not_implemented_sprint4"));
    }

    #[test]
    fn test_translate_positive_profit() {
        let id = Uuid::new_v4();
        let tid = Uuid::new_v4();
        let sim_result = simulator_v2::SimResult {
            net_profit_wei: 1_000_000,
            gas_used: 150_000,
            trace_hash: [0u8; 32],
        };
        let r = translate_result(id, tid, Ok(sim_result));
        assert!(r.passed);
        assert_eq!(r.gas_estimate_wei.as_deref(), Some("150000"));
        assert!(matches!(r.simulator, SimulatorKind::Revm));
    }

    #[test]
    fn test_translate_zero_profit() {
        let id = Uuid::new_v4();
        let tid = Uuid::new_v4();
        let sim_result = simulator_v2::SimResult {
            net_profit_wei: 0,
            gas_used: 100_000,
            trace_hash: [0u8; 32],
        };
        let r = translate_result(id, tid, Ok(sim_result));
        assert!(!r.passed, "zero profit must not pass");
    }

    #[test]
    fn test_parse_addr_bytes_valid() {
        let addr = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
        assert!(parse_addr_bytes(addr).is_some());
    }

    #[test]
    fn test_parse_addr_bytes_invalid() {
        assert!(parse_addr_bytes("not_an_address").is_none());
        assert!(parse_addr_bytes("0xshort").is_none());
    }
}
