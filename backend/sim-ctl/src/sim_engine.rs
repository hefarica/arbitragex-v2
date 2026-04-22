//! Simulation engine — orchestrates fork snapshot, tx build, eth_call,
//! result extraction. Pure orchestration; persistence + stream publish live
//! in the consumer.
//!
//! Output is always a SimulationResult, even for unsupported / error cases —
//! those just carry `passed = false` and a documented `fail_reason`.

use crate::fork_manager::ForkManager;
use crate::tx_builder::{build_probe, BuildError};
use chrono::Utc;
use ethers::abi::{decode as abi_decode, ParamType};
use ethers::prelude::*;
use ethers::core::types::transaction::eip2718::TypedTransaction;
use shared_rs::contracts::{Opportunity, SimulationResult, SimulatorKind, StrategyKind};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};

pub struct SimEngine {
    pub fork: Option<ForkManager>,
    pub signer_from: Address,
    pub timeout: Duration,
    pub max_slippage_for_pass_pct: f64,
}

impl SimEngine {
    pub async fn simulate(&self, opp: &Opportunity) -> SimulationResult {
        let trace_id = opp.trace_id;
        let id = opp.id;

        // Guard: if no fork (no ANVIL_URL or failed to connect), honest 501-style result.
        let Some(fork) = &self.fork else {
            return Self::not_implemented(id, trace_id,
                "anvil_fork_not_configured (set ANVIL_URL + profile sim)");
        };

        // Build probe.
        let probe = match build_probe(opp, self.signer_from) {
            Ok(p) => p,
            Err(BuildError::UnsupportedStrategy(_)) => {
                return Self::not_implemented(id, trace_id, "strategy_not_simulatable_in_s4");
            }
            Err(BuildError::UnsupportedChain(c)) => {
                return Self::not_implemented(id, trace_id, &format!("chain_{c}_not_supported_in_s4"));
            }
            Err(e) => return Self::failed(id, trace_id, &format!("build_error: {e}")),
        };

        // Snapshot + eth_call + revert.
        let handle = match fork.acquire().await {
            Ok(h) => h,
            Err(e) => {
                warn!(event = "sim.acquire_failed", error = %e);
                return Self::failed(id, trace_id, "fork_acquire_failed");
            }
        };

        let tx_req = TransactionRequest::new()
            .from(probe.from)
            .to(probe.to)
            .value(probe.value)
            .data(probe.data.clone())
            .gas(probe.gas_cap);
        let typed: TypedTransaction = tx_req.into();

        let call_fut = async {
            let out = fork.provider.call(&typed, None).await?;
            let gas = fork.provider.estimate_gas(&typed, None).await?;
            Ok::<(Bytes, U256), ethers::providers::ProviderError>((out, gas))
        };

        let result = timeout(self.timeout, call_fut).await;

        let release_res = handle.release().await;
        if let Err(e) = release_res {
            warn!(event = "sim.revert_failed", error = %e);
        }

        let (output, gas_used) = match result {
            Ok(Ok((o, g))) => (o, g),
            Ok(Err(e)) => {
                // ProviderError can be a revert. Extract reason when possible.
                let fail_reason = extract_revert_reason(&e);
                return Self::failed(id, trace_id, &fail_reason);
            }
            Err(_) => return Self::failed(id, trace_id, "sim_timeout"),
        };

        // Decode output → slippage vs opp.expected.
        let actual_out = decode_amount_out(&output, opp);
        let slippage_pct = compute_slippage(opp, actual_out);

        let passed = slippage_pct.is_some_and(|s| s <= self.max_slippage_for_pass_pct);

        SimulationResult {
            opportunity_id: id,
            passed,
            gas_estimate_wei: Some(gas_used.to_string()),
            gas_price_wei: None, // derived at submit time in S5
            slippage_pct,
            revert_risk_pct: if passed { Some(0.5) } else { Some(50.0) },
            simulated_profit_usd: None, // computed from counter-trade in S5
            simulator: SimulatorKind::Anvil,
            fail_reason: if passed { None } else { Some("slippage_too_high".into()) },
            simulated_at: Utc::now(),
            trace_id,
        }
    }

    fn not_implemented(id: uuid::Uuid, trace_id: uuid::Uuid, reason: &str) -> SimulationResult {
        SimulationResult {
            opportunity_id: id,
            passed: false,
            gas_estimate_wei: None,
            gas_price_wei: None,
            slippage_pct: None,
            revert_risk_pct: None,
            simulated_profit_usd: None,
            simulator: SimulatorKind::NotImplemented,
            fail_reason: Some(reason.to_string()),
            simulated_at: Utc::now(),
            trace_id,
        }
    }

    fn failed(id: uuid::Uuid, trace_id: uuid::Uuid, reason: &str) -> SimulationResult {
        SimulationResult {
            opportunity_id: id,
            passed: false,
            gas_estimate_wei: None,
            gas_price_wei: None,
            slippage_pct: None,
            revert_risk_pct: Some(100.0),
            simulated_profit_usd: None,
            simulator: SimulatorKind::Anvil,
            fail_reason: Some(reason.to_string()),
            simulated_at: Utc::now(),
            trace_id,
        }
    }
}

/// Attempts to decode an amountOut from eth_call return bytes. For V2 router,
/// the return is uint[] with one element per hop; last element is the final out.
/// For V3 exactInputSingle, return is a single uint256 = amountOut.
fn decode_amount_out(output: &Bytes, opp: &Opportunity) -> Option<U256> {
    match opp.strategy_kind {
        StrategyKind::DexArb => {
            if let Ok(toks) = abi_decode(&[ParamType::Uint(256)], output) {
                if let Some(t) = toks.first() {
                    return t.clone().into_uint();
                }
            }
            if let Ok(toks) = abi_decode(&[ParamType::Array(Box::new(ParamType::Uint(256)))], output) {
                if let Some(arr) = toks.first().and_then(|t| t.clone().into_array()) {
                    return arr.last().and_then(|t| t.clone().into_uint());
                }
            }
            None
        }
        _ => None,
    }
}

/// Rough slippage: |expected_profit_proxy - realized_out| / expected_profit_proxy * 100.
/// In S4 we don't have an "expected_out" on the opportunity, so slippage is a
/// relative measure. Accepts None when not decodable.
fn compute_slippage(opp: &Opportunity, actual_out: Option<U256>) -> Option<f64> {
    let actual = actual_out?;
    let amt_in = U256::from_dec_str(&opp.amount_in_wei).ok()?;
    if amt_in.is_zero() {
        return Some(100.0);
    }
    // Without a price oracle, we express slippage as (amt_in - actual) / amt_in * 100
    // when tokens are same-decimals. S6 will introduce a proper quote-based slippage.
    let diff = if actual >= amt_in { U256::zero() } else { amt_in - actual };
    let pct = (diff.as_u128() as f64 / amt_in.as_u128() as f64) * 100.0;
    debug!(event = "sim.slippage_heuristic", pct);
    Some(pct)
}

fn extract_revert_reason(e: &ethers::providers::ProviderError) -> String {
    let s = e.to_string();
    if s.contains("revert") || s.contains("execution reverted") {
        // Grab content after "reverted: " if present.
        if let Some(idx) = s.find("reverted:") {
            return s[idx..].chars().take(200).collect();
        }
    }
    format!("rpc_error: {}", &s[..s.len().min(200)])
}
