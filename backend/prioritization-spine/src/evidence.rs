use serde::{Deserialize, Serialize};
use crate::decision::{ExecutionDecision, RejectReason};

/// Identifies how the component-4 failure-risk buffer was computed.
///
/// Surfaced in `CostBreakdown.p_fail_source` so the dashboard can show:
///   - "p_fail = 12% (statistical, 387 samples)" — operator knows this is data-driven.
///   - "p_fail ≈ 0.1% (proxy, no history)" — operator knows to wait for more data.
///
/// Serializes transparently as a tagged JSON object so the frontend can
/// branch on `{ type: "Statistical", p: 0.12, sample_count: 387 }` vs
/// `{ type: "Proxy", buffer_usd: 1.23 }`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PFailSource {
    /// Revert probability sourced from `strategy_scores.revert_rate`.
    /// `p` is in [0.0, 1.0].  `sample_count` is the window's `sample_count`
    /// column value at query time — allows the dashboard to display confidence.
    Statistical { p: f64, sample_count: i64 },
    /// Legacy proxy: `amount_in_usd × config.failure_risk_buffer_pct`.
    /// Used at cold-start or when `sample_count < 10`.
    Proxy { buffer_usd: f64 },
}

/// Itemised cost breakdown for a single evaluated opportunity.
///
/// Populated by `ConfigAwareEvaluator::evaluate()` and stored inside
/// `OpportunityEvidence` so dashboards and downstream analytics can show
/// which cost component consumed how much of the gross profit.
///
/// ### Components (Sprint A)
/// | Field | Component # | Sprint |
/// |-------|-------------|--------|
/// | gas_usd | 1 | Pre-Sprint A (existing) |
/// | lp_fees_usd | 2 | Sprint A |
/// | flashloan_fee_usd | existing | Pre-Sprint A |
/// | slippage_buffer_usd | 3 | Sprint A (real impact or proxy) |
/// | failure_buffer_usd | 4 proxy | Pre-Sprint A |
/// | capital_cost_usd | 6 | Sprint A |
/// | ops_overhead_usd | 7 | Sprint A |
///
/// Components 4 (real failure probability), 5 (protocol revenue share), and
/// 9 (reorg risk) are deferred to Sprint B/C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// Estimated gas cost in USD (component 1).
    pub gas_usd: f64,
    /// Explicit LP fee sum across all route hops (component 2).
    /// `Σ(amount_through_leg_usd × fee_bps_leg / 10_000)`.
    pub lp_fees_usd: f64,
    /// Flash-loan protocol fee in USD (if strategy uses a flash loan).
    pub flashloan_fee_usd: f64,
    /// Slippage cost in USD (component 3).
    /// Derived from real V2 price-impact when reserves are available;
    /// falls back to `max_slippage_pct × expected_amount_out_usd` otherwise.
    pub slippage_buffer_usd: f64,
    /// Failure/reversal risk buffer in USD (component 4 proxy).
    pub failure_buffer_usd: f64,
    /// Opportunity cost of capital locked during block execution (component 6).
    /// Zero for flash-loan strategies.
    pub capital_cost_usd: f64,
    /// Amortised per-attempt infra/server overhead in USD (component 7).
    pub ops_overhead_usd: f64,
    /// Sum of all cost components above (`Σ components 1–7`).
    pub total_cost_usd: f64,
    /// How the component-4 failure buffer was computed (Sprint B).
    /// `Statistical` → live data from `strategy_scores`; `Proxy` → flat config %.
    /// Dashboard uses this to distinguish data-driven from heuristic cost.
    pub p_fail_source: PFailSource,

    // --- Component 5 (Sprint C): copy-trade / front-run probability buffer ---
    /// Heuristic copy-trade buffer in USD (component 5).
    /// Formula: `p_copied × gross_profit_usd_proxy`.
    /// Zero when `p_copied_used` is `None` (no `dex_chain_metrics` row) or
    /// when `p_copied` evaluates to `0.0` (pool volume below threshold).
    pub copied_buffer_usd: f64,
    /// The `p_copied` value that was applied, for dashboard surfacing.
    /// `None`  → no `dex_chain_metrics` row for the weakest pool; cost not added (R8).
    /// `Some(p)` → heuristic was applied; `copied_buffer_usd = p × gross_proxy`.
    pub p_copied_used: Option<f64>,

    // --- Component 8 (C2 fix, audit re-run #2 2026-05-10): relay bribe ---
    /// Estimated relay/builder bribe in USD (component 8).
    ///
    /// Sourced from the EWMA of observed `coinbase_diff_wei` values per
    /// `(chain_id, strategy_kind)`. Cold-start floor: `max(gross × 0.05, $0.50)`.
    ///
    /// Zero for L2 chains (sequencer inclusion is deterministic; no bribe needed).
    /// For Ethereum mainnet, this is typically 10-50% of gross profit.
    pub relay_fee_usd: f64,
}

impl CostBreakdown {
    /// Construct and compute `total_cost_usd` from component scalars.
    ///
    /// C2 fix (audit re-run #2 2026-05-10): `relay_fee_usd` added as
    /// component 8 — relay/builder bribe EWMA from `coinbase_diff_wei`.
    /// `total_cost_usd` now sums all 8 components.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gas_usd: f64,
        lp_fees_usd: f64,
        flashloan_fee_usd: f64,
        slippage_buffer_usd: f64,
        failure_buffer_usd: f64,
        capital_cost_usd: f64,
        ops_overhead_usd: f64,
        p_fail_source: PFailSource,
        copied_buffer_usd: f64,
        p_copied_used: Option<f64>,
        relay_fee_usd: f64,
    ) -> Self {
        let total_cost_usd = gas_usd
            + lp_fees_usd
            + flashloan_fee_usd
            + slippage_buffer_usd
            + failure_buffer_usd
            + capital_cost_usd
            + ops_overhead_usd
            + copied_buffer_usd
            + relay_fee_usd; // component 8
        Self {
            gas_usd,
            lp_fees_usd,
            flashloan_fee_usd,
            slippage_buffer_usd,
            failure_buffer_usd,
            capital_cost_usd,
            ops_overhead_usd,
            total_cost_usd,
            p_fail_source,
            copied_buffer_usd,
            p_copied_used,
            relay_fee_usd,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityEvidence {
    pub chain_id: u64,
    pub block_number: u64,
    pub rpc_url_hash: String,
    pub rpc_latency_ms: u64,
    pub state_read_timestamp: i64,
    pub pool_addresses: Vec<String>,
    pub token_addresses: Vec<String>,
    pub dex_adapters: Vec<String>,
    pub route_fingerprint: String,
    pub amount_in: f64,
    pub expected_amount_out: f64,
    pub min_amount_out: f64,
    pub gross_profit: f64,
    pub gas_units_estimated: u64,
    pub gas_price: f64,
    pub gas_cost: f64,
    pub bribe: f64,
    pub flashloan_fee: f64,
    pub net_expected_profit: f64,
    pub roi_net: f64,
    pub simulation_status: String,
    pub simulation_trace_hash: Option<String>,
    pub bundle_simulation_status: Option<String>,
    pub token_risk_score: f64,
    pub liquidity_confidence: f64,
    pub state_freshness_ms: u64,
    pub landing_probability: f64,
    pub final_score: f64,
    pub decision: ExecutionDecision,
    pub reject_reason: Option<RejectReason>,

    /// Itemised cost breakdown (Sprint A).
    /// Populated by `ConfigAwareEvaluator::evaluate()` after
    /// `calc_net_profit_and_roi` returns. Consumed by:
    ///   - Dashboard cost-decomposition charts.
    ///   - `prioritization-spine` scorer for partial-cost-weighted scoring.
    ///   - Downstream analytics / profit attribution.
    pub cost_breakdown: CostBreakdown,
}
