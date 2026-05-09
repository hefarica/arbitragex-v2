use serde::{Deserialize, Serialize};
use crate::decision::{ExecutionDecision, RejectReason};

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
}

impl CostBreakdown {
    /// Construct and compute `total_cost_usd` from component scalars.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gas_usd: f64,
        lp_fees_usd: f64,
        flashloan_fee_usd: f64,
        slippage_buffer_usd: f64,
        failure_buffer_usd: f64,
        capital_cost_usd: f64,
        ops_overhead_usd: f64,
    ) -> Self {
        let total_cost_usd = gas_usd
            + lp_fees_usd
            + flashloan_fee_usd
            + slippage_buffer_usd
            + failure_buffer_usd
            + capital_cost_usd
            + ops_overhead_usd;
        Self {
            gas_usd,
            lp_fees_usd,
            flashloan_fee_usd,
            slippage_buffer_usd,
            failure_buffer_usd,
            capital_cost_usd,
            ops_overhead_usd,
            total_cost_usd,
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
