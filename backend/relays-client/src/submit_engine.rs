//! Submit engine — the orchestrator that ties signer, nonce_manager,
//! bundle_builder, relay_flashbots and tracker into one decision path.
//!
//! Returns an ExecutionResult for every opportunity — pass, reject, paper-mode,
//! or not-submitted (never fabricates tx_hash).

use crate::bundle_builder::{build_and_sign, BuildError};
use crate::multi_relay::MultiRelayClient;
use crate::nonce_manager::NonceManager;
use crate::persistence::insert_paper_trade_run;
use crate::relay_flashbots::FlashbotsClient;
use crate::relay_no_submit_sim::{self, NoSubmitReport};
use crate::signer::Signer;
use crate::tracker::{wait_for_inclusion, InclusionOutcome};
use chrono::Utc;
use prioritization_spine::ValidatedPlan;
use redis::AsyncCommands as _;
use shared_rs::rpc_failover::AlloyHttpProvider;
use shared_rs::{
    config::AppConfig,
    contracts::{ExecutionResult, ExecutionStatus, Opportunity},
    killswitch::KillSwitchClient,
    paper_mode::PaperModeClient,
    pre_execute_checklist::{
        conservative_slippage_estimate, load_max_slippage_pct, pre_execute_checklist,
        relay_fee_ewma_key, resolve_route_factories, ChecklistError, PreExecuteContext,
        RELAY_FEE_EWMA_ALPHA, RELAY_FEE_EWMA_TTL_SECS,
    },
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
    /// BE-05: direct reference to the Flashbots client used exclusively for
    /// `eth_callBundle` re-simulation. Separate from `multi_relay` because the
    /// multi-relay abstraction erases per-backend type identity via
    /// `Arc<dyn RelayBackend>` — we cannot downcast it without unsafe.
    ///
    /// `None` when Flashbots is not configured. When absent, the re-sim step
    /// is skipped and the bundle proceeds directly to broadcast.
    pub flashbots_for_callbundle: Option<Arc<FlashbotsClient>>,
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
        // ARBX-R-0001 — a REJECTED opportunity NEVER trades. First statement of
        // the engine: covers the paper no-signer short-circuit, the checklist
        // PaperModeActive branch, the post-checklist paper path AND the live
        // broadcast path with ONE gate (mode-invariant, §34.1 — the terminus
        // differs by mode, this refusal does not).
        //
        // `rejection_reason: Some(_)` is the single rejection signal — the same
        // rule persistence::status_from_rejection_reason uses to derive
        // status='rejected'. The executor REFUSES the row and NEVER relabels
        // it (no status overwrite, no reason stripping) to make it tradable.
        // Before this gate the paper paths recorded a paper_trade_run for
        // whatever arrived — d9's 6h JOIN showed 434/434 ledger rows were
        // REJECTED opportunities (gap 1.4ms–6.4s), so the "paper history"
        // was a mirror of the reject queue, not of the viable market.
        // -----------------------------------------------------------------------
        if let Some(reason) = Self::rejection_refusal(opp) {
            info!(
                event = "executor.rejected_not_traded",
                opp_id = %opp.id,
                chain_id = opp.chain_id,
                reason = %reason,
                "opportunity is rejected — no paper_trade_run, no broadcast, no relabel (R-0001)"
            );
            return ExecutionResult {
                opportunity_id: opp.id,
                status: ExecutionStatus::NotSubmitted,
                tx_hash: None,
                relay_used: None,
                block_included: None,
                gas_used_wei: None,
                actual_profit_usd: None,
                error_message: Some(format!("rejected_not_traded:{}", reason)),
                submitted_at: Utc::now(),
                trace_id: opp.trace_id,
            };
        }

        // Hoisted here so it is in scope for both the pre-execute checklist
        // (PreExecuteContext.our_address) and the post-broadcast pending-tx
        // tracker (CODE-4: SET/DEL arbx:pending_tx:<addr>).
        // An empty address (no signer) maps to key "arbx:pending_tx:" which
        // will never exist, so Check 11 passes — same semantics as before.
        let our_address: String = self
            .signer
            .as_ref()
            .map(|s| format!("0x{}", hex::encode(s.address.as_bytes())))
            .unwrap_or_default();

        // -----------------------------------------------------------------------
        // Paper mode — computed here (before the checklist) so it can be
        // threaded into `resolve_profit_for_checklist` which must refuse gross
        // fallback in live mode (C1 fix, audit re-run #2 2026-05-10).
        //
        // C1 design: in LIVE mode (paper=false), if net_expected_profit_usd
        // is None the checklist returns Err(NetProfitUnknown) — gross fallback
        // is forbidden because it overstates net profit by 20-40%.
        // In PAPER mode the gross fallback is allowed but emits a warn so
        // operators see the gap.  Paper-mode data collection must not stall
        // just because spine hasn't processed a cold-start row yet.
        // -----------------------------------------------------------------------
        let paper_env = std::env::var("ARBX_PAPER_MODE")
            .ok()
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        // B0.2 (2026-05-13): per-chain papermode read. Each opportunity carries
        // its chain_id; we read `arbx:papermode:<chain_id>` so a flip in Chain X
        // never affects Chain Y. Falls back to legacy `arbx:papermode` for 30
        // days from 2026-05-13 (PaperModeClient::state_for_chain handles the
        // fallback chain internally with PaperModeSource attribution).
        let paper_dynamic = self.paper_mode.is_enabled_for_chain(opp.chain_id).await;
        let paper = paper_dynamic || paper_env;

        // Paper-only sink without a capital signer (Slice 1 / SSH-CI/CD paper path).
        // The checklist branch below also returns PaperModeActive → paper_trade_runs,
        // but only after building PreExecuteContext. If paper is armed and there is
        // no signer, we must never reach build_and_sign / broadcast. Record the run
        // and exit here so the simulated-stream consumer stays useful on paper VPS
        // nodes that intentionally omit FLASHBOTS_SIGNER_KEY.
        if paper && self.signer.is_none() {
            info!(
                event = "paper_mode.no_signer_short_circuit",
                opp_id = %opp.id,
                chain_id = opp.chain_id,
                "paper mode + no signer — recording paper_trade_run, no broadcast"
            );
            if let Some(ref pg_pool) = self.pg {
                if let Err(e) = insert_paper_trade_run(pg_pool, opp).await {
                    warn!(
                        event = "paper_trade.insert_failed",
                        opp_id = %opp.id,
                        error = %e,
                        "failed to insert paper_trade_run (no-signer paper path) — non-fatal"
                    );
                }
            }
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
            let route_tokens: Vec<String> =
                vec![opp.token_in.to_lowercase(), opp.token_out.to_lowercase()];

            // route_factories (Package #6 — was hollow `Vec::new()`):
            // Opportunity carries dex_a/dex_b as exchange NAMES (e.g.
            // "uniswap_v2", "aave-v3:0x..."), not factory hex addresses. The
            // `factories` table is keyed on address. We now resolve each dex
            // name to its factory address via the SAME `dexes`/`factories`
            // registry check 9 validates against (DB JOIN by normalised name
            // + chain_id; embedded hex addresses in compound labels pass
            // through verbatim). Unknown dex names are passed through honestly
            // so check 9 fails closed (FactoryInactive) — never a silent pass.
            // Empty labels (a missing dex_b on a single-DEX route) are dropped.
            let dex_names: Vec<String> = std::iter::once(&opp.dex_a)
                .chain(opp.dex_b.as_ref())
                .cloned()
                .collect();
            let route_factories: Vec<String> =
                resolve_route_factories(pg_pool, opp.chain_id, &dex_names).await;

            // C1 fix: `resolve_profit_for_checklist` now takes `paper` to
            // enforce net-only in live mode.  If it returns Err we propagate
            // directly — no fallback to gross in live mode.
            let profit_for_checklist = match Self::resolve_profit_for_checklist(opp, paper) {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        event = "submit.net_profit_unknown",
                        opp_id = %opp.id,
                        chain_id = opp.chain_id,
                        error = %e,
                        "net_expected_profit_usd is None in live mode — rejecting (gross fallback forbidden)"
                    );
                    return Self::dropped(opp, &format!("checklist_blocked: {e}"));
                }
            };

            let mut redis_conn = self.redis.clone();

            // expected_slippage_pct (Package #6 — was hollow `0.0`):
            // No real per-route slippage value is reachable here: the spine's
            // `RouteLeg.estimated_slippage_pct` is not yet plumbed through to
            // Opportunity, and `SimulationResult.slippage_pct` is not available
            // at the submit site. We compute a presence-asserting estimate
            // from the operator's `max_slippage_pct` (loaded from the SAME
            // `trading_config` row check 10 compares against, so the estimate
            // stays consistent with the cap). `conservative_slippage_estimate`
            // returns `max * 0.5` for a positive finite max — engaging check
            // 10 — and `+inf` (fail-closed) when max is absent / non-positive
            // / non-finite, so a misconfigured or missing cap blocks broadcast
            // rather than silently passing.
            //
            // SOURCE: shared-rs::pre_execute_checklist::load_max_slippage_pct
            // (trading_config row) + conservative_slippage_estimate. When a
            // real slippage value lands on Opportunity (future Sprint), prefer
            // it over this conservative proxy.
            let max_slippage_pct = load_max_slippage_pct(pg_pool, opp.chain_id).await;
            let expected_slippage_pct = conservative_slippage_estimate(max_slippage_pct);

            let mut ctx = PreExecuteContext {
                chain_id: opp.chain_id,
                route_tokens: &route_tokens,
                route_factories: &route_factories,
                // C1 fix (2026-05-10): use resolve_profit_for_checklist which
                // in live mode rejects gross-only rows (NetProfitUnknown).
                // In paper mode the gross fallback is permitted with a warn log.
                expected_profit_usd: profit_for_checklist,
                // estimated_gas_usd is 0.0 because expected_profit_usd now carries NET
                // profit (gas already deducted by spine). Setting it to non-zero would
                // double-deduct gas and incorrectly block valid opportunities.
                estimated_gas_usd: 0.0,
                expected_slippage_pct,
                our_address: &our_address,
                pg: pg_pool,
                redis: &mut redis_conn,
                // Canonical per-chain paper-mode read (B0.2 / Package #3): the
                // checklist reuses the SAME PaperModeClient as the engine's
                // pre-checklist read at line 106 so check 2 sees per-chain
                // arming (`arbx:papermode:<chain_id>`) instead of the legacy
                // global-only read.
                paper_mode_client: &self.paper_mode,
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
                    // BE-3.4: record paper-trade run for drift tracking.
                    // Non-fatal: log warning on error, never block execution path.
                    if let Some(ref pg_pool) = self.pg {
                        if let Err(e) = insert_paper_trade_run(pg_pool, opp).await {
                            warn!(
                                event = "paper_trade.insert_failed",
                                opp_id = %opp.id,
                                error = %e,
                                "failed to insert paper_trade_run (BE-3.4) — non-fatal"
                            );
                        }
                    }
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
        let provider: Arc<AlloyHttpProvider> = match rpc_pool.pick() {
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

        // 3. Paper mode — already computed above (C1 fix: moved before checklist).

        // 3.5. M2 carrier-B read (2b) — FAIL-CLOSED ValidatedPlan gate.
        //
        // The wrapped flash tx broadcast by `build_and_sign` is encoded from the
        // EXACT inputs sim-ctl validated (`arbx:validated_plan:<opp.id>`, written
        // by searcher-rs on SIM_SUCCESS, TTL 300s). We read it back here so the
        // broadcast reproduces byte-identical calldata (sim↔exec parity).
        //
        // SAFETY GUARANTEE: fail CLOSED. If the key is ABSENT (sim never
        // validated this opp, or the plan TTL'd out) or does NOT parse, we do
        // NOT broadcast — there is no validated plan to honor, so emitting any
        // tx would be unvalidated capital. We return Dropped with an explicit
        // reason + a structured warn. The producer side is intentionally
        // fail-SOFT (a Redis hiccup there just skips persist); this read being
        // fail-CLOSED is the asymmetry that makes "broadcast ⇒ validated" hold.
        let validated_plan: ValidatedPlan = {
            let plan_key = format!("arbx:validated_plan:{}", opp.id);
            let mut redis_conn = self.redis.clone();
            let raw: Option<String> = match redis_conn.get::<_, Option<String>>(&plan_key).await {
                Ok(v) => v,
                Err(e) => {
                    // Treat a Redis read error as ABSENT → fail-closed. We cannot
                    // prove the plan exists, so we must not broadcast.
                    warn!(
                        event = "submit.validated_plan_read_error",
                        opp_id = %opp.id,
                        key = %plan_key,
                        error = %e,
                        "ValidatedPlan read failed — fail-closed, no broadcast"
                    );
                    return Self::dropped(opp, "validated_plan_missing");
                }
            };
            let Some(plan_json) = raw else {
                warn!(
                    event = "submit.validated_plan_missing",
                    opp_id = %opp.id,
                    key = %plan_key,
                    "no ValidatedPlan in Redis — fail-closed, no broadcast"
                );
                return Self::dropped(opp, "validated_plan_missing");
            };
            match serde_json::from_str::<ValidatedPlan>(&plan_json) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        event = "submit.validated_plan_parse_error",
                        opp_id = %opp.id,
                        key = %plan_key,
                        error = %e,
                        "ValidatedPlan failed to parse — fail-closed, no broadcast"
                    );
                    return Self::dropped(opp, "validated_plan_parse_error");
                }
            }
        };

        // 4. Build + sign the wrapped flash tx from the validated plan.
        let bundle = match build_and_sign(
            opp,
            &validated_plan,
            signer.as_ref(),
            provider.as_ref(),
            nonce.as_ref(),
            self.cfg.execution.max_value_eth,
            self.cfg.execution.target_block_offset,
        )
        .await
        {
            Ok(b) => b,
            Err(BuildError::ValueExceedsCap { value_eth, cap_eth }) => {
                warn!(event = "submit.value_cap_exceeded", opp_id = %opp.id,
                      value_eth, cap_eth);
                return ExecutionResult {
                    opportunity_id: opp.id,
                    status: ExecutionStatus::Dropped,
                    tx_hash: None,
                    relay_used: None,
                    block_included: None,
                    gas_used_wei: None,
                    actual_profit_usd: None,
                    error_message: Some(format!("value_cap_exceeded: {value_eth} ETH > {cap_eth}")),
                    submitted_at: Utc::now(),
                    trace_id: opp.trace_id,
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

        // 4.5 A.7 private-relay no-submit validation (zero network egress).
        //
        // Every bundle that exists in scope is shape-validated locally against
        // the three doctrine relay wire-schemas (flashbots / mev-blocker /
        // titan) and the payload copy is discarded immediately — relay_no_submit_sim
        // imports no I/O, which IS the zero-egress proof. Strictly BEFORE the
        // paper short-circuit (5), eth_callBundle (5.5) and broadcast (6), so
        // no network touch can precede the local validation. The module logs
        // `relay_sim.no_submit.validated` (info, one per run) + `.detail`
        // (debug) itself; this arm only adds the drop decision.
        let no_submit_report = relay_no_submit_sim::validate_and_discard(
            relay_no_submit_sim::SimBundleParams::from_signed_bundle(&bundle),
        );
        match Self::classify_no_submit(&no_submit_report, paper) {
            NoSubmitDecision::LogOnly => {}
            NoSubmitDecision::DropAllSchemasRejected => {
                warn!(
                    event = "relay_sim.no_submit.drop",
                    opp_id = %opp.id,
                    summary = %no_submit_report.summary(),
                    "bundle rejected by ALL relay wire-schemas — dropping before any egress"
                );
                return Self::dropped(opp, "relay_no_submit_all_schemas_rejected");
            }
        }

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
            // BE-3.4: record paper-trade run for drift tracking.
            // Non-fatal: log warning on error, never block execution path.
            if let Some(ref pg_pool) = self.pg {
                if let Err(e) = insert_paper_trade_run(pg_pool, opp).await {
                    warn!(
                        event = "paper_trade.insert_failed",
                        opp_id = %opp.id,
                        error = %e,
                        "failed to insert paper_trade_run (BE-3.4) — non-fatal"
                    );
                }
            }
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

        // 5.5. BE-05: eth_callBundle re-simulation against latest chain state.
        //
        // Call `eth_callBundle` on the signed transaction RIGHT BEFORE broadcast.
        // Between sim-ctl simulation and now, pool reserves may have shifted,
        // gas may have spiked, or a competing arb may have consumed the spread.
        // This step catches the obvious failures (route reverts, slippage exceeded)
        // which are the majority of preventable wasted-gas losses.
        //
        // IMPORTANT: this is a CHECK, not a guarantee. eth_callBundle simulates
        // against `stateBlockNumber: latest`, but by the time the signed tx
        // reaches the builder for inclusion, "latest" has advanced. A passing
        // simulation is necessary but not sufficient for on-chain success.
        //
        // On endpoint failure: ABORT (fail-closed, OMEGA PHOENIX §4C#3). The
        // bundle is dropped rather than broadcast un-re-simulated — see the
        // `Err(e)` arm below. The on-chain inclusion check (step 7) remains the
        // canonical gate for bundles that DO pass re-sim.
        if let Some(ref flashbots) = self.flashbots_for_callbundle {
            let target_block = bundle.target_block;
            let sim_result = flashbots
                .call_bundle(signer.as_ref(), &bundle.tx_raw_hex, target_block)
                .await;
            // Pure classifier (BE-05 fail-closed, OMEGA PHOENIX §4C#3): isolates
            // the three-way re-sim decision so it is unit-testable without the
            // I/O surfaces (Flashbots HTTP, Redis EWMA, broadcast). Side-effects
            // (logging, EWMA write, ExecutionResult return) live in the arms.
            match Self::classify_callbundle_result(sim_result) {
                CallBundleDecision::Drop { reason } => {
                    warn!(
                        event = "be05.callbundle_reverted",
                        opp_id = %opp.id,
                        reason = %reason,
                        "eth_callBundle shows revert — aborting broadcast to save gas"
                    );
                    return Self::dropped(opp, &reason);
                }
                CallBundleDecision::Proceed { sim } => {
                    info!(
                        event = "be05.callbundle_passed",
                        opp_id = %opp.id,
                        total_gas_used = sim.total_gas_used,
                        coinbase_diff_wei = sim.coinbase_diff_wei,
                        "eth_callBundle simulation passed — proceeding to broadcast"
                    );

                    // C2 fix (audit re-run #2 2026-05-10): write coinbase_diff_wei
                    // EWMA to Redis so the spine's cost model can incorporate the
                    // observed bribe on subsequent evaluations of similar opps.
                    //
                    // Key: `arbx:relay_fee_ewma:{chain_id}:{strategy_kind}`
                    // Value: EWMA of coinbase_diff_wei in raw wei (f64 stringified).
                    // Conversion to USD happens at the read site (spine) using
                    // TradingConfigState.base_token_price_usd — relays-client is
                    // intentionally decoupled from the ETH/USD price oracle.
                    //
                    // Non-fatal: if the EWMA write fails (Redis hiccup), the spine
                    // falls back to the cold-start doctrine floor on the next eval.
                    // R8 fail-honest: we only update when coinbase_diff_wei > 0.
                    if sim.coinbase_diff_wei > 0 {
                        let strategy_kind_str =
                            format!("{:?}", opp.strategy_kind).to_ascii_lowercase();
                        let ewma_key = relay_fee_ewma_key(opp.chain_id, &strategy_kind_str);
                        let observed_wei = sim.coinbase_diff_wei as f64;
                        let mut redis_conn = self.redis.clone();

                        // Read previous EWMA.
                        let prev_ewma: f64 = redis_conn
                            .get::<_, Option<String>>(&ewma_key)
                            .await
                            .unwrap_or(None)
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(observed_wei); // cold-start: seed with first obs

                        // Apply EWMA: new = alpha * observed + (1 - alpha) * prev.
                        let new_ewma = RELAY_FEE_EWMA_ALPHA * observed_wei
                            + (1.0 - RELAY_FEE_EWMA_ALPHA) * prev_ewma;

                        let set_result: Result<(), redis::RedisError> = redis_conn
                            .set_ex(&ewma_key, new_ewma.to_string(), RELAY_FEE_EWMA_TTL_SECS)
                            .await;

                        match set_result {
                            Ok(()) => {
                                info!(
                                    event = "c2.relay_fee_ewma_updated",
                                    opp_id = %opp.id,
                                    chain_id = opp.chain_id,
                                    strategy = %strategy_kind_str,
                                    observed_wei,
                                    prev_ewma,
                                    new_ewma,
                                    "relay fee EWMA updated from eth_callBundle observation"
                                );
                            }
                            Err(e) => {
                                // Non-fatal: log and continue. Spine uses doctrine floor.
                                warn!(
                                    event = "c2.relay_fee_ewma_write_failed",
                                    opp_id = %opp.id,
                                    error = %e,
                                    "failed to write relay fee EWMA to Redis — non-fatal"
                                );
                            }
                        }
                    }
                }
                CallBundleDecision::Abort { reason } => {
                    // BE-05 fail-CLOSED: re-sim endpoint itself failed (Flashbots
                    // outage, transient HTTP/RPC error). DROP the bundle instead
                    // of broadcasting un-re-simulated — previously this was
                    // fail-honest (warn + proceed) which spent gas on bundles
                    // the safety check could not validate.
                    warn!(
                        event = "be05.callbundle_endpoint_error",
                        opp_id = %opp.id,
                        reason = %reason,
                        "eth_callBundle endpoint error — aborting broadcast (fail-closed)"
                    );
                    return Self::dropped(opp, &reason);
                }
            }
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

        // CODE-4: Check 11 backing — mark our address as having a pending tx.
        // SET with TTL 180 s (3 min) covers the maximum Flashbots inclusion
        // window. If the SET fails (Redis hiccup) we silently ignore — the key
        // simply won't exist, Check 11 passes as before (fail-honest / R8).
        // TTL is the hard backstop: even if DEL (below) is never reached, the
        // key expires and future broadcasts are unblocked.
        {
            let pending_tx_key = format!("arbx:pending_tx:{}", our_address);
            let mut redis_conn = self.redis.clone();
            let _: Result<(), redis::RedisError> = redis_conn
                .set_ex(
                    &pending_tx_key,
                    bundle_hash.as_deref().unwrap_or("submitted"),
                    180u64,
                )
                .await;
        }

        // 7. Wait for inclusion — chain is the canonical source of truth.
        // Convert ethers H256 → alloy B256 (both are [u8; 32]); this bridge
        // will be removed once bundle_builder.rs is fully migrated to alloy.
        let tx_hash_b256 = alloy::primitives::B256::from_slice(bundle.tx_hash.as_bytes());
        let outcome = wait_for_inclusion(
            provider.as_ref(),
            tx_hash_b256,
            bundle.target_block,
            self.cfg.execution.max_inclusion_wait_blocks,
            1000,
        )
        .await;

        // CODE-4: Check 11 cleanup — tx resolved (included, reverted, or
        // dropped). Delete the pending tracker so the next broadcast is not
        // blocked. If DEL fails, the 180 s TTL acts as the backstop (R8).
        {
            let pending_tx_key = format!("arbx:pending_tx:{}", our_address);
            let mut redis_conn = self.redis.clone();
            let _: Result<(), redis::RedisError> = redis_conn.del(&pending_tx_key).await;
        }

        match outcome {
            InclusionOutcome::Included { block, gas_used } => {
                // N7 (audit 2026-05-10): emit arbx_bundle_included_* counters so
                // the BundleSnipedRateHigh alert in alerts.rules.yml can fire.
                //
                // Profitable proxy: net_expected_profit_usd > 0, falling back to
                // expected_profit_usd > 0. Both fields originate from the spine
                // evaluator. Until Sprint 6 recon writes realized profit back via
                // trace reconciliation, this is the best available signal.
                // False negatives (expected positive but actually sniped) are
                // acceptable — the alert catches the aggregate ratio, not
                // individual bundles.
                let profitable = opp
                    .net_expected_profit_usd
                    .or(opp.expected_profit_usd)
                    .map(|p| p > 0.0)
                    .unwrap_or(false);
                shared_rs::metrics::record_inclusion(opp.chain_id, &relay_used, profitable);

                ExecutionResult {
                    opportunity_id: opp.id,
                    status: ExecutionStatus::Included,
                    tx_hash: Some(format!("0x{:x}", bundle.tx_hash)),
                    relay_used: Some(relay_used),
                    block_included: Some(block),
                    // gas_used is u64 (alloy 1.0 receipt.gas_used type).
                    gas_used_wei: Some(gas_used.to_string()),
                    actual_profit_usd: None, // S6 computes this from traces
                    error_message: None,
                    submitted_at,
                    trace_id: opp.trace_id,
                }
            }
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
    ///
    /// C1 fix (audit re-run #2, 2026-05-10):
    ///
    /// - **Live mode** (`paper_mode = false`): if `net_expected_profit_usd` is
    ///   `None` the function returns `Err(ChecklistError::NetProfitUnknown)`.
    ///   Falling back to the gross `expected_profit_usd` is forbidden in live mode
    ///   because gross overstates net profit by 20-40% (relay bribe, LP fees, and
    ///   slippage are not yet deducted), causing cold-start opportunities with
    ///   negative net to pass the floor gate unchallenged.
    ///
    /// - **Paper mode** (`paper_mode = true`): the gross fallback is allowed.
    ///   A `tracing::warn!(event="checklist.gross_fallback")` is emitted so the
    ///   operator can see the gap in observability without blocking paper-trade
    ///   data collection.
    ///
    /// When `net_expected_profit_usd` is present it is always preferred over
    /// gross regardless of `paper_mode` — this is unchanged from H2.
    /// ARBX-R-0001: refusal decision for REJECTED opportunities. Pure + total.
    ///
    /// `Some(reason)` → `execute()` refuses the opp at its FIRST statement:
    /// no `paper_trade_run`, no checklist, no broadcast, and NO relabeling
    /// (the row keeps its honest `rejection_reason`). `None` → the normal
    /// pipeline proceeds.
    ///
    /// This mirrors `persistence::status_from_rejection_reason` on the
    /// searcher side (`Some(_) → status='rejected'`) — same predicate, one
    /// derivation point per side; a shared-rs consolidation is a noted
    /// follow-up, not smuggled into this regression fix.
    pub(crate) fn rejection_refusal(opp: &Opportunity) -> Option<&str> {
        opp.rejection_reason.as_deref()
    }

    pub(crate) fn resolve_profit_for_checklist(
        opp: &Opportunity,
        paper_mode: bool,
    ) -> Result<f64, ChecklistError> {
        if let Some(net) = opp.net_expected_profit_usd {
            // Net field present — canonical path, no ambiguity.
            return Ok(net);
        }
        // net_expected_profit_usd is None — spine has not evaluated this row.
        if paper_mode {
            // Gross fallback permitted in paper mode; warn so the gap is visible.
            warn!(
                event = "checklist.gross_fallback",
                opp_id = %opp.id,
                gross_usd = ?opp.expected_profit_usd,
                "net_expected_profit_usd is None — using gross as fallback \
                 (paper mode: safe for observation, not for live capital)"
            );
            Ok(opp.expected_profit_usd.unwrap_or(0.0))
        } else {
            // Live mode: refuse gross fallback — the opportunity must be
            // re-scored by the spine before live capital is committed.
            Err(ChecklistError::NetProfitUnknown)
        }
    }

    fn not_submitted(opp: &Opportunity, reason: &str) -> ExecutionResult {
        ExecutionResult {
            opportunity_id: opp.id,
            status: ExecutionStatus::NotSubmitted,
            tx_hash: None,
            relay_used: None,
            block_included: None,
            gas_used_wei: None,
            actual_profit_usd: None,
            error_message: Some(reason.to_string()),
            submitted_at: Utc::now(),
            trace_id: opp.trace_id,
        }
    }

    fn dropped(opp: &Opportunity, reason: &str) -> ExecutionResult {
        ExecutionResult {
            opportunity_id: opp.id,
            status: ExecutionStatus::Dropped,
            tx_hash: None,
            relay_used: None,
            block_included: None,
            gas_used_wei: None,
            actual_profit_usd: None,
            error_message: Some(reason.to_string()),
            submitted_at: Utc::now(),
            trace_id: opp.trace_id,
        }
    }

    /// Pure classifier for the `eth_callBundle` re-sim result (BE-05).
    ///
    /// Maps the raw `Result<CallBundleResult, anyhow::Error>` into one of three
    /// decisions so the `execute()` body can dispatch side-effects (logging,
    /// EWMA write, ExecutionResult return) without re-deriving the logic. Kept
    /// side-effect-free so it is directly unit-testable.
    ///
    /// - `Proceed`  → re-sim passed (no tx reverted): broadcast continues.
    /// - `Drop`     → re-sim returned a revert: abort bundle, save gas.
    /// - `Abort`    → re-sim endpoint itself failed (Flashbots outage / RPC
    ///   error): DROP the bundle fail-closed (OMEGA PHOENIX §4C#3) instead of
    ///   broadcasting un-re-simulated.
    fn classify_callbundle_result(
        sim_result: anyhow::Result<crate::relay_flashbots::CallBundleResult>,
    ) -> CallBundleDecision {
        match sim_result {
            Ok(sim) if sim.any_failed() => {
                let reasons: Vec<String> = sim
                    .tx_results
                    .iter()
                    .filter_map(|t| t.error.clone().or_else(|| t.revert.clone()))
                    .collect();
                CallBundleDecision::Drop {
                    reason: format!("eth_callBundle_revert: {}", reasons.join("; ")),
                }
            }
            Ok(sim) => CallBundleDecision::Proceed { sim },
            Err(e) => CallBundleDecision::Abort {
                reason: format!("eth_callBundle_re-sim_unavailable: {e}"),
            },
        }
    }

    /// Pure classifier for the A.7 no-submit relay-shape validation (step 4.5).
    ///
    /// - `LogOnly`                → paper mode, or at least one doctrine relay
    ///   schema accepts the wire-shape: the report is informational (already
    ///   logged by `relay_no_submit_sim`) and the flow continues.
    /// - `DropAllSchemasRejected` → non-paper AND all three relay schemas
    ///   rejected the bundle wire-shape: fail-closed BEFORE any egress —
    ///   broadcasting a bundle no doctrine relay would accept is guaranteed
    ///   wasted gas (same posture as the BE-05 abort arm below).
    ///
    /// Paper mode is deliberately `LogOnly` even on total rejection: the paper
    /// terminus must keep recording runs with its existing semantics
    /// (`paper_mode.skip_submit` + the paper_trade_runs insert); the
    /// shape-rejection detail rides in the `relay_sim.no_submit.*` events.
    fn classify_no_submit(report: &NoSubmitReport, paper: bool) -> NoSubmitDecision {
        if !paper && report.accepted_count() == 0 {
            NoSubmitDecision::DropAllSchemasRejected
        } else {
            NoSubmitDecision::LogOnly
        }
    }
}

/// Three-way decision for the BE-05 `eth_callBundle` re-simulation step.
///
/// See [`SubmitEngine::classify_callbundle_result`] for the semantics. `Proceed`
/// carries the passing `CallBundleResult` so the caller can extract EWMA inputs
/// (coinbase_diff_wei); the other arms carry a fully-formed reason string that
/// becomes the `ExecutionResult.error_message`.
#[derive(Debug)]
enum CallBundleDecision {
    /// Re-sim passed — broadcast continues. Holds the result so the caller can
    /// read `coinbase_diff_wei` for the relay-fee EWMA update.
    Proceed {
        sim: crate::relay_flashbots::CallBundleResult,
    },
    /// Re-sim returned a revert — drop the bundle to save gas.
    Drop { reason: String },
    /// Re-sim endpoint itself failed — drop the bundle fail-closed (do NOT
    /// broadcast un-re-simulated). OMEGA PHOENIX §4C#3.
    Abort { reason: String },
}

/// Two-way decision for the A.7 no-submit relay-shape validation (step 4.5).
///
/// See [`SubmitEngine::classify_no_submit`] for the semantics.
#[derive(Debug, PartialEq, Eq)]
enum NoSubmitDecision {
    /// Report is informational — flow continues (paper mode, or ≥1 accept).
    LogOnly,
    /// Non-paper + all three relay schemas rejected the wire-shape — drop the
    /// bundle before any egress (fail-closed).
    DropAllSchemasRejected,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test module — panics are acceptable
mod tests {
    use super::*;
    use chrono::Utc;
    use shared_rs::contracts::{Opportunity, StrategyKind};
    use uuid::Uuid;

    fn make_opp(gross: Option<f64>, net: Option<f64>) -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::dex_arb(),
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
            cartridge_id: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    /// ARBX-R-0001 repro: a REJECTED opportunity must be refused by the
    /// executor — no paper_trade_run, no broadcast. Before the gate, the
    /// paper paths recorded a run for EVERY opp that arrived: d9's 6h JOIN
    /// showed 434/434 ledger rows were rejected opportunities. The refusal
    /// decision is pure and total so this regression anchors at the exact
    /// predicate `execute()` gates on (first statement).
    #[test]
    fn r0001_rejected_opportunity_is_refused() {
        let mut opp = make_opp(Some(52.0), Some(7.0));
        // The shape of the incident flood: a gate rejection with its reason.
        opp.rejection_reason = Some("NegativeNetProfit:gas_floor_breach".into());
        assert_eq!(
            SubmitEngine::rejection_refusal(&opp),
            Some("NegativeNetProfit:gas_floor_breach"),
            "rejected opp must be refused with its honest reason"
        );
    }

    /// ARBX-R-0001 counterpart: a viable opp (rejection_reason None) is NOT
    /// refused — the gate must never over-block the viable market.
    #[test]
    fn r0001_viable_opportunity_is_not_refused() {
        let opp = make_opp(Some(52.0), Some(7.0));
        assert_eq!(
            SubmitEngine::rejection_refusal(&opp),
            None,
            "viable opp (rejection_reason NULL) must reach the normal pipeline"
        );
    }

    /// H2 regression: net field takes precedence over gross (both modes).
    /// $52 gross / $7 net — Check 7 must see $7, not $52.
    #[test]
    fn h2_net_field_takes_precedence_over_gross() {
        let opp = make_opp(Some(52.0), Some(7.0));
        // Net present — paper_mode flag is irrelevant.
        let profit_live = SubmitEngine::resolve_profit_for_checklist(&opp, false)
            .expect("net present → no error in live mode");
        let profit_paper = SubmitEngine::resolve_profit_for_checklist(&opp, true)
            .expect("net present → no error in paper mode");
        assert!(
            (profit_live - 7.0).abs() < f64::EPSILON,
            "live mode: expected 7.0 (net), got {profit_live}"
        );
        assert!(
            (profit_paper - 7.0).abs() < f64::EPSILON,
            "paper mode: expected 7.0 (net), got {profit_paper}"
        );
    }

    /// C1 regression (audit re-run #2): in live mode (paper_mode=false) with
    /// net_expected_profit_usd=None, the checklist MUST return NetProfitUnknown.
    /// Falling back to gross in live mode overstates profit by 20-40% and can
    /// pass cold-start opportunities with negative net through the floor gate.
    #[test]
    fn c1_live_mode_net_absent_returns_net_profit_unknown() {
        let opp = make_opp(Some(52.0), None);
        let result = SubmitEngine::resolve_profit_for_checklist(&opp, false /* live */);
        assert!(
            matches!(result, Err(ChecklistError::NetProfitUnknown)),
            "C1: live mode with net=None must return Err(NetProfitUnknown), got: {:?}",
            result
        );
    }

    /// C1 regression: in paper mode (paper_mode=true) with net=None, the gross
    /// fallback IS allowed (returns Ok with gross value) so paper-trade data
    /// collection is not blocked by cold-start rows.
    #[test]
    fn c1_paper_mode_net_absent_falls_back_to_gross() {
        let opp = make_opp(Some(52.0), None);
        let result = SubmitEngine::resolve_profit_for_checklist(&opp, true /* paper */);
        let profit =
            result.expect("paper mode with net=None must return Ok (gross fallback allowed)");
        assert!(
            (profit - 52.0).abs() < f64::EPSILON,
            "C1 paper mode: expected 52.0 (gross fallback), got {profit}"
        );
    }

    /// C1 regression: in paper mode with BOTH fields absent, fallback to 0.0.
    /// The 0.0 value will fail the profit floor gate — safe direction.
    #[test]
    fn c1_paper_mode_both_absent_falls_back_to_zero() {
        let opp = make_opp(None, None);
        let profit = SubmitEngine::resolve_profit_for_checklist(&opp, true /* paper */)
            .expect("paper mode both-None must return Ok(0.0)");
        assert!(
            profit == 0.0,
            "paper mode both-absent: expected 0.0 (floor-fail-safe), got {profit}"
        );
    }

    /// C1 regression: in live mode with BOTH fields absent, must also return
    /// NetProfitUnknown (no net means no live execution permitted).
    #[test]
    fn c1_live_mode_both_absent_returns_net_profit_unknown() {
        let opp = make_opp(None, None);
        let result = SubmitEngine::resolve_profit_for_checklist(&opp, false /* live */);
        assert!(
            matches!(result, Err(ChecklistError::NetProfitUnknown)),
            "C1: live mode both-None must return Err(NetProfitUnknown), got: {:?}",
            result
        );
    }

    /// Net profit below zero is propagated correctly (negative net must fail floor).
    /// Both modes: net present → always returned regardless of paper_mode.
    #[test]
    fn h2_negative_net_propagated_correctly() {
        let opp = make_opp(Some(52.0), Some(-3.0));
        let profit_live = SubmitEngine::resolve_profit_for_checklist(&opp, false)
            .expect("net present → no error in live mode");
        let profit_paper = SubmitEngine::resolve_profit_for_checklist(&opp, true)
            .expect("net present → no error in paper mode");
        assert!(
            (profit_live - (-3.0)).abs() < f64::EPSILON,
            "live: expected -3.0 (negative net), got {profit_live}"
        );
        assert!(
            (profit_paper - (-3.0)).abs() < f64::EPSILON,
            "paper: expected -3.0 (negative net), got {profit_paper}"
        );
    }

    // ─── Package #6 — checks 9 & 10 load-bearing wiring ───────────────────
    //
    // Proves the gates are no longer hollow WITHOUT a live DB: the helpers
    // invoked at the call site (resolve_route_factories for check 9,
    // conservative_slippage_estimate for check 10) are exercised against the
    // SAME shapes `Opportunity.dex_a` / `dex_b` actually carry. The pure
    // pieces (normalize_dex_name_for_factory_lookup, conservative_slippage_estimate)
    // are covered exhaustively in shared-rs; here we assert the *contract*
    // the submit_engine wiring depends on: a non-empty factory list flows to
    // check 9 (so an unknown factory would block), and a real >0 slippage
    // estimate flows to check 10.

    use shared_rs::pre_execute_checklist::{
        conservative_slippage_estimate, normalize_dex_name_for_factory_lookup, DexLookupToken,
    };

    /// Check 9 contract: an unknown DEX name resolves to a pass-through that
    /// check 9's `factories` address-lookup will NOT find → FactoryInactive.
    /// Pre-fix this slot was hollow (empty vec → check 9 iterated zero
    /// entries → always Ok). The factory list handed to check 9 is now
    /// NON-EMPTY for any non-empty dex_a.
    #[test]
    fn pkg6_check9_factory_resolution_is_load_bearing() {
        // Any scanner-emitted dex_a — even a bogus one — classifies to a
        // non-empty lookup token, so resolve_route_factories (which drops only
        // empty labels) yields at least one entry. That entry, when handed to
        // check 9, either matches a factories row (active dex) or fails
        // closed (FactoryInactive). The hollow-vec short-circuit is gone.
        let opp = make_opp(Some(10.0), Some(2.0));
        assert!(!opp.dex_a.is_empty(), "fixture dex_a must be non-empty");

        let tok = normalize_dex_name_for_factory_lookup(&opp.dex_a);
        // "uniswap_v2" → CanonicalName("uniswapv2") (non-empty).
        assert!(
            matches!(&tok, DexLookupToken::CanonicalName(n) if !n.is_empty()),
            "dex_a `{}` must resolve to a non-empty token so check 9 receives a \
             real factory key, got {tok:?}",
            opp.dex_a
        );

        // An embedded address (compound label) yields a non-empty Address.
        let compound = "aave-v3:0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f";
        let tok2 = normalize_dex_name_for_factory_lookup(compound);
        assert!(
            matches!(&tok2, DexLookupToken::Address(a) if !a.is_empty()),
            "compound dex label must yield a non-empty Address for check 9"
        );

        // Garbage with enough alphanumerics ("??bogus??" → "bogus") yields a
        // non-empty CanonicalName; the fail-closed guarantee is delivered at
        // the next stage (resolve_route_factories finds no matching dexes row
        // → passes the raw through → check 9 finds no factories row →
        // FactoryInactive). Either way check 9 receives a NON-EMPTY entry, so
        // the hollow empty-vec short-circuit is gone. The load-bearing
        // property here is non-emptiness across all three variants.
        let tok3 = normalize_dex_name_for_factory_lookup("??bogus??");
        let non_empty = match &tok3 {
            DexLookupToken::Address(a) => !a.is_empty(),
            DexLookupToken::CanonicalName(n) => !n.is_empty(),
            DexLookupToken::Unknown(s) => !s.is_empty(),
        };
        assert!(
            non_empty,
            "garbage dex label must yield a non-empty token so check 9 \
             receives an entry and fails closed, got {tok3:?}"
        );
    }

    /// Check 10 contract: a real >0 slippage estimate flows for any positive
    /// finite configured cap, and fail-closed (+inf) for absent/incoherent
    /// caps. Pre-fix this was a hard-coded 0.0 (always passed check 10).
    #[test]
    fn pkg6_check10_slippage_estimate_is_real_and_positive() {
        // A representative operator cap (1.5% from app.toml) → estimate 0.75%.
        let est = conservative_slippage_estimate(Some(1.5));
        assert!(
            est > 0.0 && est.is_finite(),
            "check 10 must receive a real finite >0 estimate, got {est}"
        );
        // Missing cap → fail-closed (+inf), NOT 0.0.
        let fail_closed = conservative_slippage_estimate(None);
        assert!(
            fail_closed.is_infinite() && fail_closed.is_sign_positive(),
            "missing cap must fail closed (+inf), got {fail_closed}"
        );
    }

    // ─── BE-05 fail-closed (OMEGA PHOENIX §4C#3) ───────────────────────────
    //
    // Regression coverage for the eth_callBundle re-sim decision. The endpoint
    // must NOT be mocked (no live HTTP) — the pure classifier is exercised
    // directly. The contract: a bundle that the re-sim endpoint cannot confirm
    // is DROPPED, never handed to the broadcaster.

    use crate::relay_flashbots::{CallBundleResult, TxCallResult};

    fn sim_passing() -> CallBundleResult {
        CallBundleResult {
            bundle_hash: Some("0xpass".into()),
            coinbase_diff_wei: 1_000_000_000_000_000,
            total_gas_used: 250_000,
            tx_results: vec![TxCallResult {
                tx_hash: "0xabc".into(),
                gas_used: 250_000,
                error: None,
                revert: None,
            }],
        }
    }

    fn sim_reverted(reason: &str) -> CallBundleResult {
        CallBundleResult {
            bundle_hash: Some("0xrev".into()),
            coinbase_diff_wei: 0,
            total_gas_used: 100_000,
            tx_results: vec![TxCallResult {
                tx_hash: "0xabc".into(),
                gas_used: 100_000,
                error: None,
                revert: Some(reason.into()),
            }],
        }
    }

    /// BE-05 success path: re-sim confirms (no revert) → classifier returns
    /// `Proceed` and the bundle WOULD reach broadcast. The sim payload is
    /// preserved so the caller can read coinbase_diff_wei for the EWMA update.
    #[test]
    fn be05_resim_passed_classifies_proceed() {
        let sim = sim_passing();
        let decision = SubmitEngine::classify_callbundle_result(Ok(sim.clone()));
        match decision {
            CallBundleDecision::Proceed { sim: passed_sim } => {
                assert_eq!(passed_sim.total_gas_used, sim.total_gas_used);
                assert_eq!(passed_sim.coinbase_diff_wei, sim.coinbase_diff_wei);
                assert!(
                    !passed_sim.any_failed(),
                    "passing sim must not flag failure"
                );
            }
            other => panic!("BE-05 passing re-sim must Proceed, got {other:?}"),
        }
    }

    /// BE-05 clean-reject path: re-sim returned a revert → classifier returns
    /// `Drop`. Preserved from pre-fix behavior — only the endpoint-error path
    /// changed. The reason carries the revert string for the ExecutionResult.
    #[test]
    fn be05_resim_reverted_classifies_drop() {
        let sim = sim_reverted("INSUFFICIENT_OUTPUT_AMOUNT");
        let decision = SubmitEngine::classify_callbundle_result(Ok(sim));
        match decision {
            CallBundleDecision::Drop { reason } => {
                assert!(
                    reason.contains("eth_callBundle_revert"),
                    "Drop reason must identify the revert path, got: {reason}"
                );
                assert!(
                    reason.contains("INSUFFICIENT_OUTPUT_AMOUNT"),
                    "Drop reason must carry the revert string, got: {reason}"
                );
            }
            other => panic!("BE-05 reverted re-sim must Drop, got {other:?}"),
        }
    }

    /// BE-05 fail-CLOSED regression (the bug this package closes): when the
    /// eth_callBundle endpoint itself errors (Flashbots outage / HTTP 5xx /
    /// RPC error), the classifier MUST return `Abort` — NOT `Proceed`. The
    /// `execute()` body maps `Abort` to `Self::dropped(...)`, so the bundle is
    /// never handed to the broadcaster. Pre-fix this path fell through to the
    /// multi-relay broadcast with only a warn log, spending gas on an
    /// un-re-simulated bundle.
    #[test]
    fn be05_resim_endpoint_error_classifies_abort_not_proceed() {
        let endpoint_err = anyhow::anyhow!("flashbots http 503: upstream timeout");
        let decision = SubmitEngine::classify_callbundle_result(Err(endpoint_err));
        match decision {
            CallBundleDecision::Abort { reason } => {
                assert!(
                    reason.starts_with("eth_callBundle_re-sim_unavailable:"),
                    "Abort reason must identify the unavailable-endpoint path, got: {reason}"
                );
                assert!(
                    reason.contains("flashbots http 503"),
                    "Abort reason must carry the underlying error, got: {reason}"
                );
            }
            CallBundleDecision::Proceed { .. } => {
                panic!(
                    "BE-05 endpoint error must Abort (fail-closed), NOT Proceed — \
                     broadcasting un-re-simulated bundles spends gas (OMEGA PHOENIX §4C#3)"
                );
            }
            other => panic!("BE-05 endpoint error must Abort, got {other:?}"),
        }
    }

    // ── A.7 no-submit classifier (step 4.5) ──────────────────────────────────

    /// Synthetic report: first `accepts` relays accept the shape, the rest
    /// reject it (verdict order: flashbots, mev_blocker, titan).
    fn no_submit_report(accepts: usize) -> NoSubmitReport {
        use crate::relay_no_submit_sim::RelayVerdict;
        let verdict = |accept: bool| {
            if accept {
                RelayVerdict::AcceptedShape
            } else {
                RelayVerdict::RejectedShape("synthetic shape mismatch".into())
            }
        };
        NoSubmitReport {
            flashbots: verdict(accepts >= 1),
            mev_blocker: verdict(accepts >= 2),
            titan: verdict(accepts >= 3),
        }
    }

    #[test]
    fn a7_all_schemas_rejected_drops_only_in_non_paper() {
        let rejected = no_submit_report(0);
        assert_eq!(rejected.accepted_count(), 0);
        assert_eq!(
            SubmitEngine::classify_no_submit(&rejected, false),
            NoSubmitDecision::DropAllSchemasRejected,
            "non-paper + 0/3 accepts must fail-closed BEFORE any egress"
        );
    }

    #[test]
    fn a7_paper_mode_is_log_only_even_when_all_schemas_reject() {
        assert_eq!(
            SubmitEngine::classify_no_submit(&no_submit_report(0), true),
            NoSubmitDecision::LogOnly,
            "paper terminus keeps recording runs with existing semantics; \
             shape-rejection detail rides in relay_sim.no_submit.* events"
        );
    }

    #[test]
    fn a7_partial_acceptance_is_log_only() {
        for accepts in 1..=3 {
            assert_eq!(
                SubmitEngine::classify_no_submit(&no_submit_report(accepts), false),
                NoSubmitDecision::LogOnly,
                "≥1 accepting relay schema must never drop ({accepts}/3 accepts)"
            );
        }
    }
}
