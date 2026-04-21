//! Submit engine — the orchestrator that ties signer, nonce_manager,
//! bundle_builder, relay_flashbots and tracker into one decision path.
//!
//! Returns an ExecutionResult for every opportunity — pass, reject, paper-mode,
//! or not-submitted (never fabricates tx_hash).

use crate::bundle_builder::{build_and_sign, BuildError};
use crate::nonce_manager::NonceManager;
use crate::relay_flashbots::FlashbotsClient;
use crate::signer::Signer;
use crate::tracker::{wait_for_inclusion, InclusionOutcome};
use chrono::Utc;
use ethers::prelude::*;
use shared_rs::{
    config::AppConfig,
    contracts::{ExecutionResult, ExecutionStatus, Opportunity},
    killswitch::KillSwitchClient,
};
use std::sync::Arc;
use tracing::{info, warn};

pub struct SubmitEngine {
    pub signer: Option<Arc<Signer>>,
    pub provider: Option<Arc<Provider<Http>>>,
    pub nonce: Option<Arc<NonceManager>>,
    pub flashbots: Option<Arc<FlashbotsClient>>,
    pub kill_switch: KillSwitchClient,
    pub cfg: Arc<AppConfig>,
}

impl SubmitEngine {
    pub async fn execute(&self, opp: &Opportunity) -> ExecutionResult {
        // 1. Kill-switch
        if self.kill_switch.is_enabled().await {
            warn!(event = "submit.blocked", opp_id = %opp.id, reason = "kill_switch");
            return Self::dropped(opp, "kill_switch_on");
        }

        // 2. Signer presence — without it, we cannot submit anything real.
        let Some(signer) = self.signer.clone() else {
            return Self::not_submitted(opp, "signer_not_configured");
        };
        let Some(provider) = self.provider.clone() else {
            return Self::not_submitted(opp, "rpc_not_configured");
        };
        let Some(nonce) = self.nonce.clone() else {
            return Self::not_submitted(opp, "nonce_manager_not_initialized");
        };

        // 3. Paper mode — honored both in config AND env (env overrides).
        let paper_env = std::env::var("ARBX_PAPER_MODE").ok()
            .map(|v| v.to_ascii_lowercase() == "true").unwrap_or(false);
        let paper_cfg = self.cfg.execution.paper_mode;
        let paper = paper_cfg || paper_env;

        // 4. Build + sign.
        let bundle = match build_and_sign(
            opp,
            signer.as_ref(),
            provider.as_ref(),
            nonce.as_ref(),
            self.cfg.execution.max_value_eth,
            self.cfg.execution.target_block_offset,
        ).await {
            Ok(b) => b,
            Err(BuildError::ValueExceedsCap { value_eth, cap_eth }) => {
                warn!(event = "submit.value_cap_exceeded", opp_id = %opp.id,
                      value_eth, cap_eth);
                return ExecutionResult {
                    opportunity_id: opp.id,
                    status: ExecutionStatus::Dropped,
                    tx_hash: None, relay_used: None, block_included: None,
                    gas_used_wei: None, actual_profit_usd: None,
                    error_message: Some(format!("value_cap_exceeded: {value_eth} ETH > {cap_eth}")),
                    submitted_at: Utc::now(), trace_id: opp.trace_id,
                };
            }
            Err(e) => {
                return Self::not_submitted(opp, &format!("build_error: {e}"));
            }
        };

        info!(
            event = "bundle.built",
            opp_id = %opp.id,
            from = %bundle.from,
            nonce = bundle.nonce,
            target_block = bundle.target_block,
            paper_mode = paper,
        );

        // 5. Paper mode short-circuit.
        if paper {
            info!(event = "paper_mode.skip_submit", opp_id = %opp.id,
                  tx_hash_would_be = %format!("0x{:x}", bundle.tx_hash),
                  would_submit_to = "flashbots");
            return ExecutionResult {
                opportunity_id: opp.id,
                status: ExecutionStatus::NotSubmitted,
                tx_hash: None,
                relay_used: Some("paper_mode".into()),
                block_included: None,
                gas_used_wei: None,
                actual_profit_usd: None,
                error_message: Some("paper_mode_enabled".into()),
                submitted_at: Utc::now(),
                trace_id: opp.trace_id,
            };
        }

        // 6. Submit to Flashbots.
        let Some(fb) = self.flashbots.clone() else {
            return Self::not_submitted(opp, "flashbots_client_not_initialized");
        };
        let send_res = fb.send_bundle(signer.as_ref(), &bundle).await;
        let relay_used = "flashbots".to_string();
        let submitted_at = Utc::now();

        match send_res {
            Ok(r) => {
                if let Some(e) = r.error {
                    warn!(event = "submit.relay_err", opp_id = %opp.id, code = e.code, msg = %e.message);
                    return ExecutionResult {
                        opportunity_id: opp.id,
                        status: ExecutionStatus::Dropped,
                        tx_hash: None, relay_used: Some(relay_used),
                        block_included: None, gas_used_wei: None, actual_profit_usd: None,
                        error_message: Some(format!("relay_rejected: {}", e.message)),
                        submitted_at, trace_id: opp.trace_id,
                    };
                }
                let bundle_hash = r.result.and_then(|x| x.bundle_hash);
                info!(event = "submit.ok", opp_id = %opp.id, bundle_hash = ?bundle_hash);

                // 7. Wait for inclusion.
                let outcome = wait_for_inclusion(
                    provider.as_ref(),
                    bundle.tx_hash,
                    bundle.target_block,
                    self.cfg.execution.max_inclusion_wait_blocks,
                    1000,
                ).await;

                match outcome {
                    InclusionOutcome::Included { block, gas_used } => ExecutionResult {
                        opportunity_id: opp.id,
                        status: ExecutionStatus::Included,
                        tx_hash: Some(format!("0x{:x}", bundle.tx_hash)),
                        relay_used: Some(relay_used),
                        block_included: Some(block),
                        gas_used_wei: Some(gas_used.to_string()),
                        actual_profit_usd: None, // S6 computes this from traces
                        error_message: None,
                        submitted_at, trace_id: opp.trace_id,
                    },
                    InclusionOutcome::Reverted { block } => ExecutionResult {
                        opportunity_id: opp.id,
                        status: ExecutionStatus::Reverted,
                        tx_hash: Some(format!("0x{:x}", bundle.tx_hash)),
                        relay_used: Some(relay_used),
                        block_included: Some(block),
                        gas_used_wei: None, actual_profit_usd: None,
                        error_message: Some("on_chain_revert".into()),
                        submitted_at, trace_id: opp.trace_id,
                    },
                    InclusionOutcome::Dropped => ExecutionResult {
                        opportunity_id: opp.id,
                        status: ExecutionStatus::Dropped,
                        tx_hash: Some(format!("0x{:x}", bundle.tx_hash)),
                        relay_used: Some(relay_used),
                        block_included: None,
                        gas_used_wei: None, actual_profit_usd: None,
                        error_message: Some("inclusion_timeout".into()),
                        submitted_at, trace_id: opp.trace_id,
                    },
                }
            }
            Err(e) => {
                warn!(event = "submit.http_err", opp_id = %opp.id, error = %e);
                ExecutionResult {
                    opportunity_id: opp.id,
                    status: ExecutionStatus::Dropped,
                    tx_hash: None, relay_used: Some(relay_used),
                    block_included: None, gas_used_wei: None, actual_profit_usd: None,
                    error_message: Some(format!("relay_http_error: {e}")),
                    submitted_at, trace_id: opp.trace_id,
                }
            }
        }
    }

    fn not_submitted(opp: &Opportunity, reason: &str) -> ExecutionResult {
        ExecutionResult {
            opportunity_id: opp.id,
            status: ExecutionStatus::NotSubmitted,
            tx_hash: None, relay_used: None, block_included: None,
            gas_used_wei: None, actual_profit_usd: None,
            error_message: Some(reason.to_string()),
            submitted_at: Utc::now(),
            trace_id: opp.trace_id,
        }
    }

    fn dropped(opp: &Opportunity, reason: &str) -> ExecutionResult {
        ExecutionResult {
            opportunity_id: opp.id,
            status: ExecutionStatus::Dropped,
            tx_hash: None, relay_used: None, block_included: None,
            gas_used_wei: None, actual_profit_usd: None,
            error_message: Some(reason.to_string()),
            submitted_at: Utc::now(),
            trace_id: opp.trace_id,
        }
    }
}
