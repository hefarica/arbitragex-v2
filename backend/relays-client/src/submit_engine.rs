//! Submit engine — the orchestrator that ties signer, nonce_manager,
//! bundle_builder, relay_flashbots and tracker into one decision path.
//!
//! Returns an ExecutionResult for every opportunity — pass, reject, paper-mode,
//! or not-submitted (never fabricates tx_hash).

use crate::bundle_builder::{build_and_sign, BuildError};
use crate::multi_relay::MultiRelayClient;
use crate::nonce_manager::NonceManager;
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
    /// Multi-relay broadcast client (BE-06). Holds all configured backends
    /// (Flashbots, BloXRoute, Titan, …). `None` when no relay is configured.
    /// Broadcasting to N relays increases inclusion probability without risk of
    /// double-inclusion — the signed tx is identical to all relays; the chain
    /// will include it at most once.
    pub multi_relay: Option<Arc<MultiRelayClient>>,
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
                // H2 fix (2026-05-08): use spine-computed net profit when available.
                // net_expected_profit_usd is populated by scanner.rs after
                // calc_net_profit_and_roi runs through the spine evaluator path.
                // Fallback to gross (expected_profit_usd) for pre-spine rows only —
                // those are blocked by paper_mode (Check 2) before reaching Check 7
                // in practice. The fallback preserves prior behaviour for cold-start
                // rows; new opportunities always have the net field set. R8 fail-honest:
                // 0.0 as last resort means the floor check will likely block the opp,
                // which is the safe direction (under-execute, not over-execute).
                expected_profit_usd: Self::resolve_profit_for_checklist(opp),
                // estimated_gas_usd is 0.0 because expected_profit_usd now carries NET
                // profit (gas already deducted by spine). Setting it to non-zero would
                // double-deduct gas and incorrectly block valid opportunities.
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
            let relay_names = self
                .multi_relay
                .as_ref()
                .map(|mr| mr.backend_names())
                .unwrap_or_else(|| "none".to_string());
            info!(event = "paper_mode.skip_submit", opp_id = %opp.id,
                  tx_hash_would_be = %format!("0x{:x}", bundle.tx_hash),
                  would_submit_to = %relay_names);
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

        // 6. Multi-relay broadcast (BE-06).
        //
        // Send the bundle to ALL configured relays in parallel. Any single
        // acceptance is sufficient — the on-chain inclusion poll (step 7)
        // is the canonical gate. Multiple relays accepting the same bundle
        // is safe: the tx is signed once; it can be included on-chain at most
        // once regardless of how many builders receive it.
        let Some(multi_relay) = self.multi_relay.clone() else {
            return Self::not_submitted(opp, "multi_relay_not_configured");
        };
        let broadcast_result = multi_relay.broadcast(&bundle, signer.as_ref()).await;
        let submitted_at = Utc::now();

        if !broadcast_result.any_success() {
            // All relays rejected or timed out — log each failure and drop.
            for (name, reason) in &broadcast_result.failures {
                warn!(
                    event = "submit.all_relays_failed",
                    opp_id = %opp.id,
                    relay = %name,
                    reason = %reason,
                );
            }
            return ExecutionResult {
                opportunity_id: opp.id,
                status: ExecutionStatus::Dropped,
                tx_hash: None,
                relay_used: None,
                block_included: None,
                gas_used_wei: None,
                actual_profit_usd: None,
                error_message: Some(format!(
                    "all_relays_failed: {}",
                    broadcast_result
                        .failures
                        .iter()
                        .map(|(n, r)| format!("{n}={r}"))
                        .collect::<Vec<_>>()
                        .join(";")
                )),
                submitted_at,
                trace_id: opp.trace_id,
            };
        }

        let relay_used = broadcast_result.relay_used_str();
        let bundle_hash = broadcast_result.first_bundle_hash().map(|s| s.to_string());
        info!(
            event = "submit.ok",
            opp_id = %opp.id,
            relays_accepted = %relay_used,
            bundle_hash = ?bundle_hash,
        );

        // 7. Wait for inclusion — chain is the canonical source of truth.
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
                submitted_at,
                trace_id: opp.trace_id,
            },
            InclusionOutcome::Reverted { block } => ExecutionResult {
                opportunity_id: opp.id,
                status: ExecutionStatus::Reverted,
                tx_hash: Some(format!("0x{:x}", bundle.tx_hash)),
                relay_used: Some(relay_used),
                block_included: Some(block),
                gas_used_wei: None,
                actual_profit_usd: None,
                error_message: Some("on_chain_revert".into()),
                submitted_at,
                trace_id: opp.trace_id,
            },
            InclusionOutcome::Dropped => ExecutionResult {
                opportunity_id: opp.id,
                status: ExecutionStatus::Dropped,
                tx_hash: Some(format!("0x{:x}", bundle.tx_hash)),
                relay_used: Some(relay_used),
                block_included: None,
                gas_used_wei: None,
                actual_profit_usd: None,
                error_message: Some("inclusion_timeout".into()),
                submitted_at,
                trace_id: opp.trace_id,
            },
        }
    }

    /// Pure helper: resolve which profit figure Check 7 should use.
    /// Extracted to make the H2 contract testable without I/O.
    #[inline]
    pub(crate) fn resolve_profit_for_checklist(opp: &Opportunity) -> f64 {
        opp.net_expected_profit_usd
            .or(opp.expected_profit_usd)
            .unwrap_or(0.0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared_rs::contracts::{Opportunity, StrategyKind};
    use uuid::Uuid;

    fn make_opp(gross: Option<f64>, net: Option<f64>) -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::DexArb,
            dex_a: "uniswap_v2".into(),
            dex_b: None,
            pair_symbol: "WETH/USDC".into(),
            token_in: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".into(),
            token_out: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(),
            amount_in_wei: "1000000000000000000".into(),
            expected_profit_usd: gross,
            net_expected_profit_usd: net,
            roi_pct: None,
            risk_score: None,
            block_number: Some(12_000_000),
            rejection_reason: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    /// H2 regression: net field takes precedence over gross.
    /// $52 gross / $7 net — Check 7 must see $7, not $52.
    #[test]
    fn h2_net_field_takes_precedence_over_gross() {
        let opp = make_opp(Some(52.0), Some(7.0));
        let profit = SubmitEngine::resolve_profit_for_checklist(&opp);
        assert!(
            (profit - 7.0).abs() < f64::EPSILON,
            "expected 7.0 (net), got {profit}"
        );
    }

    /// When net field is absent (pre-spine row), gross is used as fallback.
    #[test]
    fn h2_fallback_to_gross_when_net_absent() {
        let opp = make_opp(Some(52.0), None);
        let profit = SubmitEngine::resolve_profit_for_checklist(&opp);
        assert!(
            (profit - 52.0).abs() < f64::EPSILON,
            "expected 52.0 (gross fallback), got {profit}"
        );
    }

    /// When both fields are absent, returns 0.0 (safe — will fail the floor check).
    #[test]
    fn h2_both_absent_returns_zero() {
        let opp = make_opp(None, None);
        let profit = SubmitEngine::resolve_profit_for_checklist(&opp);
        assert!(
            profit == 0.0,
            "expected 0.0 (fail-safe), got {profit}"
        );
    }

    /// Net profit below zero is propagated correctly (negative net must fail floor).
    #[test]
    fn h2_negative_net_propagated_correctly() {
        let opp = make_opp(Some(52.0), Some(-3.0));
        let profit = SubmitEngine::resolve_profit_for_checklist(&opp);
        assert!(
            (profit - (-3.0)).abs() < f64::EPSILON,
            "expected -3.0 (negative net), got {profit}"
        );
    }
}
