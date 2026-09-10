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
use ethers::core::types::transaction::eip2718::TypedTransaction;
use ethers::prelude::*;
use shared_rs::contracts::{Opportunity, SimulationResult, SimulatorKind};
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
            return Self::not_implemented(
                id,
                trace_id,
                "anvil_fork_not_configured (set ANVIL_URL + profile sim)",
            );
        };

        // Build probe.
        let probe = match build_probe(opp, self.signer_from) {
            Ok(p) => p,
            // BR-00 (2026-09-07): D-SIM-01 -- the reason now names the EXACT
            // kind it refused (base kind or cartridge stem, never collapsed).
            // Closes the BR-00-VERIFY 2.3 observability gap: a stray
            // ":dex_arb" suffix on one of these rows is now visible evidence
            // of upstream kind drift instead of an undiagnosable aggregate.
            Err(BuildError::UnsupportedStrategy(kind)) => {
                return Self::not_implemented(
                    id,
                    trace_id,
                    &format!("strategy_not_simulatable_in_s4:{}", kind.as_str()),
                );
            }
            // BR-00 (2026-09-07): cyclic routes are a DISTINCT structural gap --
            // the single-hop probe cannot represent a closed route -- so they
            // carry their own reason family, also per-kind. Both families are
            // classified as capability gaps (persistence.rs): a simulator
            // shape limit is never a market verdict on the opportunity.
            Err(BuildError::CyclicRouteNotRepresentable(kind)) => {
                return Self::not_implemented(
                    id,
                    trace_id,
                    &format!(
                        "strategy_cyclic_route_not_simulatable_in_s4:{}",
                        kind.as_str()
                    ),
                );
            }
            Err(BuildError::UnsupportedChain(c)) => {
                return Self::not_implemented(
                    id,
                    trace_id,
                    &format!("chain_{c}_not_supported_in_s4"),
                );
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
        // BR-00 (2026-09-07): decode by ABI shape (the probe tx shape is
        // chosen by the router, never by the strategy kind) -- every
        // structurally-built probe decodes identically.
        let actual_out = decode_amount_out(&output);
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
            fail_reason: if passed {
                None
            } else if slippage_pct.is_none() {
                // BR-00 (2026-09-07): R8 fail-honest -- an undecodable probe
                // output is NOT a slippage verdict; the old code folded it
                // into "slippage_too_high", claiming a measurement that never
                // happened. Classified as a capability gap in persistence.rs.
                Some("output_undecodable".into())
            } else {
                Some("slippage_too_high".into())
            },
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
// BR-00 (2026-09-07): decode by ABI SHAPE, not by kind string -- the probe tx
// shape is chosen by the ROUTER (V2 => uint[], V3 => uint256), never by the
// strategy kind, so every structurally-built probe decodes the same way
// regardless of kind or cartridge stem.
// Array shape goes FIRST: a uint[] head word is the 32-byte data offset, so
// decoding it as a bare uint256 "succeeds" with the constant 32 -- the old
// uint256-first order (pre-existing, kind-gated to dex_arb) misread every V2
// probe return as amountOut=32. A bare uint256 (V3) can only decode as an
// array when its value is exactly 32 AND more words follow, so the fallback
// order is unambiguous in both directions.
fn decode_amount_out(output: &Bytes) -> Option<U256> {
    if let Ok(toks) = abi_decode(&[ParamType::Array(Box::new(ParamType::Uint(256)))], output) {
        if let Some(arr) = toks.first().and_then(|t| t.clone().into_array()) {
            return arr.last().and_then(|t| t.clone().into_uint());
        }
    }
    if let Ok(toks) = abi_decode(&[ParamType::Uint(256)], output) {
        if let Some(t) = toks.first() {
            return t.clone().into_uint();
        }
    }
    None
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
    let diff = if actual >= amt_in {
        U256::zero()
    } else {
        amt_in - actual
    };
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod br00_decode_tests {
    use super::*;
    use ethers::abi::{encode as abi_encode, Token};

    /// V3 exactInputSingle returns a bare uint256 amountOut.
    #[test]
    fn decode_v3_uint256_shape() {
        let out = Bytes::from(abi_encode(&[Token::Uint(U256::from(123_456u64))]));
        assert_eq!(decode_amount_out(&out), Some(U256::from(123_456u64)));
    }

    /// V2 swapExactTokensForTokens returns uint[] amounts -- the LAST element
    /// is the final out (single-hop probes return a 1-element array).
    #[test]
    fn decode_v2_array_shape_takes_last() {
        let arr = Token::Array(vec![
            Token::Uint(U256::from(1u64)),
            Token::Uint(U256::from(2u64)),
        ]);
        let out = Bytes::from(abi_encode(&[arr]));
        assert_eq!(decode_amount_out(&out), Some(U256::from(2u64)));
    }

    /// Garbage / empty output returns None -- fail-closed, never a
    /// fabricated amountOut (R8: None = not computed).
    #[test]
    fn decode_garbage_returns_none() {
        assert_eq!(decode_amount_out(&Bytes::from(vec![0xde, 0xad])), None);
        assert_eq!(decode_amount_out(&Bytes::default()), None);
    }
}
