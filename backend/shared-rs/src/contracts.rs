//! Canonical data contracts. Mirror `configs/schemas/*.json`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Canonical strategy identity. The 5 base families PLUS every cartridge `.rhai`
/// filename stem (e.g. `mev_01_001_dex_dex_arbitrage`) — **each cartridge IS a
/// canonical `strategy_kind`**. Backed by a `String` because the 264-cartridge set
/// is dynamic (auto-generated in `shared-ts/contracts/strategy-kinds.ts`); an enum
/// of 264 variants is impractical. Serializes as the inner snake_case string,
/// matching the canonical TS `StrategyKind` zod enum (5 base + 264 cartridges).
/// Construct base families via the associated functions; cartridge identities via
/// [`StrategyKind::cartridge`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StrategyKind(pub String);

impl StrategyKind {
    pub fn dex_arb() -> Self {
        Self("dex_arb".to_string())
    }
    pub fn triangular() -> Self {
        Self("triangular".to_string())
    }
    pub fn backrun() -> Self {
        Self("backrun".to_string())
    }
    pub fn liquidation() -> Self {
        Self("liquidation".to_string())
    }
    pub fn flashloan_arb() -> Self {
        Self("flashloan_arb".to_string())
    }
    /// A cartridge identity — the `.rhai` filename stem (a canonical strategy_kind).
    pub fn cartridge(stem: impl Into<String>) -> Self {
        Self(stem.into())
    }
    /// The canonical snake_case string (persisted to Postgres, emitted to streams).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: Uuid,
    pub chain_id: u64,
    pub strategy_kind: StrategyKind,
    pub dex_a: String,
    pub dex_b: Option<String>,
    pub pair_symbol: String,
    pub token_in: String,
    pub token_out: String,
    /// big int as decimal string
    pub amount_in_wei: String,
    /// **GROSS** profit in USD as emitted by scanner.rs — before gas, relay fees,
    /// LP fees, slippage, failure buffer, or any other cost component.
    /// Set by `compute_gross_usd_for_spread()` (renamed from `compute_usd_profit_for_spread`).
    /// None when the oracle could not price the tokens (R8 fail-honest).
    ///
    /// **Do NOT use this field as the net profit figure.** It overstates realised
    /// profit by 20-40% on Ethereum mainnet (relay bribe alone is 10-50% of gross).
    /// The canonical net figure after all 8 cost components is
    /// `net_expected_profit_usd` (populated by the spine evaluator). The
    /// pre-execute checklist Check 7 uses `net_expected_profit_usd`; this field
    /// is the DB-persistent gross column and must not be removed or renamed.
    pub expected_profit_usd: Option<f64>,
    /// Net profit in USD after ALL cost components (gas + LP fees + slippage +
    /// flash-loan fee + failure buffer + capital cost + ops overhead).
    /// Populated by the spine evaluator (`calc_net_profit_and_roi`).
    /// None for opportunities that bypassed the spine path (cold-start, pre-spine rows).
    /// submit_engine Check 7 uses this field; falls back to `expected_profit_usd`
    /// when None (R8 fail-honest — same behaviour as before spine path ran).
    #[serde(default)]
    pub net_expected_profit_usd: Option<f64>,
    pub roi_pct: Option<f64>,
    pub risk_score: Option<f64>,
    pub block_number: Option<u64>,
    /// Diagnostic reason when the opportunity was rejected by a pre-execution
    /// gate (allowlist, strategy, math, risk policy). NULL when the opp passed
    /// all gates OR when a row predates the BUG-2/3 + observability sprint.
    /// Stored as plain text for flexibility — frontend renders verbatim.
    /// See `RejectReason` enum in `prioritization-spine::decision` for the
    /// canonical set of values.
    #[serde(default)]
    pub rejection_reason: Option<String>,
    /// Real strategy cartridge id (e.g. `mev_08_018_liquidation_auction`) when the
    /// opportunity came from a Rhai cartridge. None for the core engines. Lets the
    /// dashboard show the REAL strategy name/family instead of flattening every
    /// cartridge to a generic `strategy_kind`, and lets the card grid key on the
    /// true strategy so rows never collapse into gaps.
    #[serde(default)]
    pub cartridge_id: Option<String>,
    /// WO-CARDS-COMPLETE-01 (operator order 2026-09-17): which detector
    /// produced this row — the core engine name (`dex_engine`,
    /// `triangular_engine`, `liquidation_engine`, …) or the cartridge
    /// stem/id when it came from the cartridge layer (same value as
    /// `cartridge_id`). Set at CONSTRUCTION, never at emit. None only for
    /// rows that predate the field (R8 fail-honest — never fabricated).
    #[serde(default)]
    pub detector_id: Option<String>,
    /// WO-CARDS-COMPLETE-01 (operator order 2026-09-17): wall-clock
    /// milliseconds between `detected_at` and entry into the emit path —
    /// stamped by `OpportunityEmitter` before serialize/publish on BOTH the
    /// accepted and rejected paths. None for pre-field rows or when the
    /// span is negative (clock step guard, R8).
    #[serde(default)]
    pub pipeline_latency_ms: Option<u64>,
    /// ALWAYS-COMPUTE (operator mandate 2026-09-27): the ONE complete
    /// economics computation object, built BEFORE the pass/fail decision and
    /// persisted on BOTH branches — "COMPUTED ≠ PROFITABLE; FAIL must mean
    /// 'se hizo el cálculo y no cumple el criterio', not 'no tengo números'".
    ///
    /// `computation_status` distinguishes the three honest states:
    ///   "computed" — every field the rejecting path actually computed is
    ///                present (a rejected row with a real quote carries its
    ///                full gross/costs/net/roi arithmetic).
    ///   "partial"  — some real figures exist (e.g. the engine probe gross)
    ///                but the cost decomposition never ran; absent fields are
    ///                null WITH their reason in `not_computed_reasons`.
    ///   "error"    — a technical failure (no quote at all: missing reserves,
    ///                RPC down, no config). NO numbers, only the reason. This
    ///                is the ONLY state where numeric fields may be null
    ///                without a producer-side computation having happened.
    ///
    /// R8: `null` in a numeric field means "not computed on this path" and is
    /// always explained in `not_computed_reasons`; `0.0` means computed and
    /// exactly zero. Mode-invariant (§34.1): identical in paper/testnet/live.
    /// None only when the `ARBX_ALWAYS_COMPUTE_ECONOMICS` knob is OFF
    /// (revert posture) or on pre-field rows.
    #[serde(default)]
    pub economics: Option<EconomicsComputation>,
    pub detected_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

/// ALWAYS-COMPUTE: one per-leg entry of the computation object's `legs`
/// ledger — exact wei strings (NOT f64: precision loss above 2^53), aligned
/// with `RouteMetadata`'s per-hop arrays (leg i+1's input IS leg i's output).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputedLeg {
    /// Token entering this leg (address, verbatim from the route plan).
    pub token_in: String,
    /// Token leaving this leg.
    pub token_out: String,
    /// Exact wei entering the leg (decimal string).
    pub amount_in_wei: String,
    /// Exact wei leaving the leg (decimal string).
    pub amount_out_wei: String,
}

/// ALWAYS-COMPUTE (operator mandate 2026-09-27): the complete economics
/// computation object. Built by the sizing kernel's callers (orchestrator,
/// cartridge path) BEFORE the accept/reject decision from the components the
/// kernel already computed (`SizedCandidate` + sheet-07 `RouteNetEconomics`),
/// and stamped at the emit boundary for producers that never reached sizing.
///
/// Closure identity: when `gross_profit_usd` and the cost components are
/// present, `net_profit_usd == gross_profit_usd − total_cost_usd` holds by
/// construction (same components, one subtraction order) — the same identity
/// the frontend ledger (`opportunity-ledger.ts`) demands before painting a
/// ladder, so a FAIL card renders the full arithmetic instead of dashes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EconomicsComputation {
    /// "computed" | "partial" | "error" (see `Opportunity::economics` docs).
    pub computation_status: String,
    /// When `computation_status == "error"`: the technical reason no numbers
    /// exist (e.g. "missing_reserves_pool_b"). Null otherwise.
    #[serde(default)]
    pub error_reason: Option<String>,
    /// Capital deployed into the cycle, exact wei (decimal string).
    #[serde(default)]
    pub amount_in_wei: Option<String>,
    /// Final cycle output, exact wei (decimal string; last leg's amount_out).
    #[serde(default)]
    pub amount_out_wei: Option<String>,
    /// `amount_in_wei` priced in USD at the token price the kernel used.
    #[serde(default)]
    pub amount_in_usd: Option<f64>,
    /// `amount_in_usd + gross_profit_usd` (identity: out − in == gross).
    #[serde(default)]
    pub amount_out_usd: Option<f64>,
    /// GROSS over input in USD (out − in, post pool fees) at the sized amount.
    #[serde(default)]
    pub gross_profit_usd: Option<f64>,
    // ── Cost components (USD). Present = computed; null + reason = not
    // computed on this path; 0.0 = computed and exactly zero (R8).
    /// Gas cost of the atomic bundle.
    #[serde(default)]
    pub gas_usd: Option<f64>,
    /// DEX/LP fees as a SEPARATE line — null with reason
    /// "included_in_amount_out_post_fee" because pool fees are already inside
    /// `amount_out` (never double-counted as a fabricated line).
    #[serde(default)]
    pub dex_fees_usd: Option<f64>,
    /// Flash-loan fee (financing mode bps on the borrowed principal).
    #[serde(default)]
    pub flash_fee_usd: Option<f64>,
    /// Builder/relay bribe. 0.0 is its TRUE computed value today (no bid path
    /// exists in the shadow/paper termini — same doctrine as sheet-07 col Q).
    #[serde(default)]
    pub bribe_usd: Option<f64>,
    /// Slippage as a separate line — null with reason "priced_by_amm_curve"
    /// (the kernel's quote already walks the curve; a separate slippage line
    /// would double-count price impact).
    #[serde(default)]
    pub slippage_usd: Option<f64>,
    /// Other costs (ops overhead per attempt).
    #[serde(default)]
    pub other_costs_usd: Option<f64>,
    /// Σ of the present cost components (never sums a null).
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    /// `gross_profit_usd − total_cost_usd` — the kernel's own net figure.
    #[serde(default)]
    pub net_profit_usd: Option<f64>,
    /// `amount_in_usd > 0 ? net/amount_in_usd*100 : null` — null ONLY when the
    /// division is impossible (R8), never a fabricated 0.
    #[serde(default)]
    pub roi_pct: Option<f64>,
    /// Operator's simulation target floor (`simulation_target_profit_usd`),
    /// when configured on the chain's trading_config at compute time.
    #[serde(default)]
    pub target_net_usd: Option<f64>,
    /// `net_profit_usd − target_net_usd` (negative = short of the floor).
    #[serde(default)]
    pub target_delta_usd: Option<f64>,
    /// `net_profit_usd >= target_net_usd` — the pass/fail criterion itself.
    #[serde(default)]
    pub meets_target: Option<bool>,
    /// Block the quote/reserves were read at.
    #[serde(default)]
    pub quote_block: Option<u64>,
    /// Block of the exact atomic simulation — null on the searcher path
    /// (revm simulation is sim-ctl's job; never fabricated here).
    #[serde(default)]
    pub simulation_block: Option<u64>,
    /// Per-leg wei ledger (exact strings), aligned with route_plan legs.
    #[serde(default)]
    pub legs: Vec<ComputedLeg>,
    /// R8: field name → why it is null. A null WITHOUT an entry here is a bug.
    #[serde(default)]
    pub not_computed_reasons: std::collections::BTreeMap<String, String>,
}

// `StrategyKind` is the newtype above (each cartridge IS a canonical
// strategy_kind via `StrategyKind::cartridge(stem)`). No separate
// "canonical_strategy_kind" mapping is needed — the field itself carries the
// cartridge identity; persistence binds `strategy_kind.as_str()`.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulatorKind {
    Anvil,
    Tenderly,
    Hardhat,
    /// In-memory REVM simulator (simulator-v2 crate). Opt-in via SIM_BACKEND=revm.
    Revm,
    NotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub opportunity_id: Uuid,
    pub passed: bool,
    pub gas_estimate_wei: Option<String>,
    pub gas_price_wei: Option<String>,
    pub slippage_pct: Option<f64>,
    pub revert_risk_pct: Option<f64>,
    pub simulated_profit_usd: Option<f64>,
    pub simulator: SimulatorKind,
    pub fail_reason: Option<String>,
    pub simulated_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Submitted,
    Included,
    Reverted,
    Dropped,
    Replaced,
    NotImplemented,
    /// S5+: paper-mode or pre-submit reject (e.g., value cap exceeded).
    /// No on-chain side effect; `tx_hash` is always null.
    NotSubmitted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub opportunity_id: Uuid,
    pub status: ExecutionStatus,
    pub tx_hash: Option<String>,
    pub relay_used: Option<String>,
    pub block_included: Option<u64>,
    pub gas_used_wei: Option<String>,
    pub actual_profit_usd: Option<f64>,
    pub error_message: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconReport {
    pub opportunity_id: Uuid,
    pub execution_id: Option<Uuid>,
    pub tx_hash: Option<String>,
    pub chain_id: u64,

    pub expected_amount_out_wei: Option<String>,
    pub actual_amount_out_wei: Option<String>,
    pub variance_native_units: Option<String>,
    pub variance_pct: Option<f64>,

    pub expected_profit_usd: f64,
    pub actual_profit_usd: f64,
    pub pnl_source: String,

    pub actual_gas_used_wei: Option<String>,
    pub actual_gas_price_wei: Option<String>,
    pub fail_reason: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

/// Canonical 501 payload for unimplemented paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotImplementedPayload {
    pub error: &'static str,
    pub requires: Vec<&'static str>,
    pub sprint: &'static str,
    pub detail: String,
}

impl NotImplementedPayload {
    pub fn new(
        requires: Vec<&'static str>,
        sprint: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            error: "not_implemented",
            requires,
            sprint,
            detail: detail.into(),
        }
    }
}

// =============================================================================
// ALWAYS-COMPUTE (operator mandate 2026-09-27): wire-contract tests
// =============================================================================
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod economics_wire_tests {
    use super::*;

    fn base_opp() -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::dex_arb(),
            dex_a: "uniswap_v2".to_owned(),
            dex_b: None,
            pair_symbol: "WETH/USDC".to_owned(),
            token_in: "0xweth".to_owned(),
            token_out: "0xusdc".to_owned(),
            amount_in_wei: "1000".to_owned(),
            expected_profit_usd: None,
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: Some(42),
            rejection_reason: None,
            cartridge_id: None,
            detector_id: Some("dex_engine".to_owned()),
            pipeline_latency_ms: None,
            economics: None,
            detected_at: DateTime::parse_from_rfc3339("2026-09-27T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            trace_id: Uuid::new_v4(),
        }
    }

    /// Additive wire: a LEGACY payload (pre-126 JSON, no `economics` key)
    /// still deserializes — existing consumers and old stream replays keep
    /// working (no renames, no removals).
    #[test]
    fn legacy_payload_without_economics_deserializes() {
        let json = serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "chain_id": 1u64,
            "strategy_kind": "dex_arb",
            "dex_a": "uniswap_v2",
            "dex_b": null,
            "pair_symbol": "WETH/USDC",
            "token_in": "0xa",
            "token_out": "0xb",
            "amount_in_wei": "0",
            "expected_profit_usd": null,
            "net_expected_profit_usd": null,
            "roi_pct": null,
            "risk_score": null,
            "block_number": null,
            "rejection_reason": "missing_reserves_pool_a",
            "detected_at": "2026-09-27T00:00:00Z",
            "trace_id": Uuid::new_v4().to_string()
        });
        let opp: Opportunity = serde_json::from_value(json).expect("legacy payload must parse");
        assert!(opp.economics.is_none());
    }

    /// The complete object round-trips: every field, the per-leg ledger, and
    /// the R8 reasons map survive serialization verbatim.
    #[test]
    fn economics_computation_round_trips() {
        let mut opp = base_opp();
        opp.economics = Some(EconomicsComputation {
            computation_status: "computed".to_owned(),
            error_reason: None,
            amount_in_wei: Some("1000".to_owned()),
            amount_out_wei: Some("990".to_owned()),
            amount_in_usd: Some(2350.0),
            amount_out_usd: Some(2362.5),
            gross_profit_usd: Some(12.5),
            gas_usd: Some(0.18),
            dex_fees_usd: None,
            flash_fee_usd: Some(2.12),
            bribe_usd: Some(0.0),
            slippage_usd: None,
            other_costs_usd: Some(0.01),
            total_cost_usd: Some(2.31),
            net_profit_usd: Some(-50.0),
            roi_pct: Some(-2.13),
            target_net_usd: Some(25.0),
            target_delta_usd: Some(-75.0),
            meets_target: Some(false),
            quote_block: Some(42),
            simulation_block: None,
            legs: vec![ComputedLeg {
                token_in: "0xa".to_owned(),
                token_out: "0xb".to_owned(),
                amount_in_wei: "1000".to_owned(),
                amount_out_wei: "990".to_owned(),
            }],
            not_computed_reasons: std::collections::BTreeMap::from([(
                "dex_fees_usd".to_owned(),
                "included_in_amount_out_post_fee".to_owned(),
            )]),
        });
        let json = serde_json::to_value(&opp).unwrap();
        assert_eq!(json["economics"]["computation_status"], "computed");
        assert_eq!(json["economics"]["net_profit_usd"], -50.0);
        assert_eq!(json["economics"]["bribe_usd"], 0.0, "true computed zero");
        assert!(json["economics"]["dex_fees_usd"].is_null());
        let back: Opportunity = serde_json::from_value(json).unwrap();
        assert_eq!(back.economics, opp.economics);
    }

    /// An absent object serializes as an explicit null key (presence-stable
    /// wire), never as a missing key.
    #[test]
    fn absent_economics_serializes_as_null_key() {
        let json = serde_json::to_value(base_opp()).unwrap();
        assert!(json.get("economics").is_some());
        assert!(json["economics"].is_null());
    }
}
