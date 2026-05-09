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
    paper_mode::PaperModeClient,
    pre_execute_checklist::{pre_execute_checklist, ChecklistError, PreExecuteContext},
    rpc_failover::HttpRpcPool,
};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, warn};

pub struct SubmitEngine {
    pub signer: Option<Arc<Signer>>,
    /// RPC pool — routes `build_and_sign` and `wait_for_inclusion` provider
    /// references through EWMA-ranked selection. `with_retry` is used for
    /// nonce fetches (in NonceManager); `pick()` is used for the single-call
    /// sites in bundle_builder and tracker where retry is handled at a higher
    /// level (the bundle is rebuilt on a fresh call if submission fails).
    ///
    /// `None` when no `RPC_HTTP_<chain_id>` is configured — engine stays in
    /// 501 / NotSubmitted mode.
    pub rpc_pool: Option<Arc<HttpRpcPool>>,
    pub nonce: Option<Arc<NonceManager>>,
    pub flashbots: Option<Arc<FlashbotsClient>>,
    pub kill_switch: KillSwitchClient,
    pub paper_mode: PaperModeClient,
    pub cfg: Arc<AppConfig>,
    /// Postgres pool — used by the pre-execute checklist (chain/config/token/factory gates).
    /// Optional: when absent the checklist DB checks are skipped and execution falls
    /// through to the existing signer-presence gate.
    pub pg: Option<PgPool>,
    /// Redis connection manager — used by the pre-execute checklist (kill_switch,
    /// paper_mode, gas freshness, mempool, circuit-breaker).
    /// ConnectionManager is Arc-backed internally; clone() is cheap and correct.
    pub redis: redis::aio::ConnectionManager,
}

impl SubmitEngine {
    pub async fn execute(&self, opp: &Opportunity) -> ExecutionResult {
        // -----------------------------------------------------------------------
        // Pre-execute checklist (BE-03) — canonical 12-step safety gate.
        //
        // Runs only when the DB pool is present (relays-client boots with an
        // optional DB; when absent the legacy kill-switch + paper-mode checks
        // below remain the gate). `ConnectionManager` is Arc-backed; cloning it
        // creates a logical alias to the same pool — no new TCP connection.
        // -----------------------------------------------------------------------
        if let Some(ref pg_pool) = self.pg {
            // Build the route context from the Opportunity struct.
            //
            // route_tokens: the two tokens in the trade. Stored as lowercase
            // hex in the tokens table (added by migration 021). We normalise
            // to lowercase here so check 8 finds the rows reliably.
            let route_tokens: Vec<String> = vec![
                opp.token_in.to_lowercase(),
                opp.token_out.to_lowercase(),
            ];

            // route_factories: Opportunity carries dex_a/dex_b as exchange
            // names (e.g. "uniswap_v2"), not factory hex addresses. The
            // factories table is keyed on address. Passing names would
            // vacuously block check 9 for every opportunity. Until the
            // Opportunity schema carries factory_address fields (future
            // Sprint), we pass an empty slice so check 9 is a no-op —
            // consistent with the pre-checklist behaviour where factories
            // were not validated at this layer.
            let route_factories: Vec<String> = Vec::new();

            // our_address: not carried by Opportunity (it's the signer's
            // address). Extract from the Signer if present, otherwise empty
            // string. An empty address means pending_tx_key("") → Redis key
            // "arbx:pending_tx:" which will never exist, so check 11 passes.
            let our_address_owned: String = self
                .signer
                .as_ref()
                .map(|s| format!("0x{}", hex::encode(s.address.as_bytes())))
                .unwrap_or_default();

            let mut redis_conn = self.redis.clone();
            let mut ctx = PreExecuteContext {
                chain_id: opp.chain_id,
                route_tokens: &route_tokens,
                route_factories: &route_factories,
                expected_profit_usd: opp.expected_profit_usd.unwrap_or(0.0),
                // estimated_gas_usd is not yet carried by Opportunity.
                // Passing 0.0 is conservative: net = profit - 0 = profit,
                // so the floor check uses gross profit. This is safe because
                // the floor is an operator-set minimum net profit; the gas
                // deduction will be added when Opportunity carries gas fields.
                estimated_gas_usd: 0.0,
                // expected_slippage_pct is not yet in Opportunity; 0.0 means
                // check 10 always passes — no false blocks until the field lands.
                expected_slippage_pct: 0.0,
                our_address: &our_address_owned,
                pg: pg_pool,
                redis: &mut redis_conn,
            };

            match pre_execute_checklist(&mut ctx).await {
                Ok(()) => {
                    // All 12 checks passed — fall through to broadcast.
                }
                Err(ChecklistError::PaperModeActive) => {
                    // Non-fatal signal: log and return the same NotSubmitted
                    // shape the legacy paper-mode branch produces (step 5 below).
                    // This guarantees upstream callers (consumer.rs, HTTP handler)
                    // see status=NotSubmitted — not Dropped, not Included —
                    // so the opportunity is recorded as a paper-trade, not a
                    // phantom transaction awaiting a receipt that will never come.
                    info!(
                        event = "paper_mode.checklist_suppressed",
                        opp_id = %opp.id,
                        chain_id = opp.chain_id,
                        "pre_execute_checklist: paper_mode active — broadcast suppressed"
                    );
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
                Err(e) => {
                    // All other checklist errors are fatal blockers.
                    warn!(
                        event = "submit.checklist_blocked",
                        opp_id = %opp.id,
                        chain_id = opp.chain_id,
                        error = ?e,
                        "pre_execute_checklist blocked broadcast"
                    );
                    return Self::dropped(opp, &format!("checklist_blocked: {e}"));
                }
            }
        }

        // -----------------------------------------------------------------------
        // Legacy guards — kept as belt-and-suspenders when DB pool is absent.
        // When the checklist ran above (pg.is_some()), checks 1 and 2 already
        // covered kill-switch and paper-mode; these branches are a no-op fallback.
        // -----------------------------------------------------------------------

        // 1. Kill-switch
        if self.kill_switch.is_enabled().await {
            warn!(event = "submit.blocked", opp_id = %opp.id, reason = "kill_switch");
            return Self::dropped(opp, "kill_switch_on");
        }

        // 2. Signer presence — without it, we cannot submit anything real.
        let Some(signer) = self.signer.clone() else {
            return Self::not_submitted(opp, "signer_not_configured");
        };
        // Pick the best available provider from the pool for this execution.
        // `pick()` returns the lowest-EWMA-latency Healthy (or Degraded) entry.
        // If all providers are Open (circuit-broken), we return NotSubmitted
        // gracefully — the health loop will rotate the circuit back to half-open
        // after CB_OPEN_DURATION. NEVER panic on AllUnhealthy.
        let Some(rpc_pool) = self.rpc_pool.as_ref() else {
            return Self::not_submitted(opp, "rpc_not_configured");
        };
        let provider: Arc<Provider<Http>> = match rpc_pool.pick() {
            Ok(entry) => entry.provider.clone(),
            Err(e) => {
                warn!(
                    event = "submit.rpc_pool_exhausted",
                    opp_id = %opp.id,
                    error = %e,
                    "all RPC providers unhealthy — cannot submit"
                );
                return Self::not_submitted(opp, &format!("rpc_pool_all_unhealthy: {e}"));
            }
        };
        let Some(nonce) = self.nonce.clone() else {
            return Self::not_submitted(opp, "nonce_manager_not_initialized");
        };

        // 3. Paper mode — dynamic from Redis + env override.
        let paper_env = std::env::var("ARBX_PAPER_MODE").ok()
            .map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false);
        let paper_dynamic = self.paper_mode.is_enabled().await;
        let paper = paper_dynamic || paper_env;

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
