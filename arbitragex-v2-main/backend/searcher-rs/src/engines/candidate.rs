// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! `StrategyCandidate` — the universal output type for every strategy engine.
//!
//! Moved from `engines/dex_engine.rs` to `engines/candidate.rs` so that
//! `triangular_engine` and `flashloan_engine` (Phases 9-10) can reference it
//! without creating a cyclic dependency. `dex_engine.rs` now re-imports from
//! here.
//!
//! ## R8 invariants
//!
//! - `gross_profit_usd = None` when ANY token cannot be priced.
//! - `net_expected_profit_usd = None` at engine time; filled by the spine
//!   evaluator downstream.
//! - `rejection_reason` is `Some(...)` for rejected candidates, `None` for
//!   accepted ones.
//! - `base_strategy` is `Some(...)` only on `FlashloanArb`-wrapped variants
//!   (Phase 10). Always `None` from other engines.

use crate::strategy_label::StrategyLabel;
use ethers::types::H256;
use prioritization_spine::route_plan::RoutePlan;
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::contracts::Opportunity;

/// A single candidate produced by an engine.
///
/// Carries everything the orchestrator needs to evaluate the candidate via
/// `ConfigAwareEvaluator` and emit it via `OpportunityEmitter`. Candidates
/// with a `rejection_reason` still flow through `emit_rejected` so the
/// operator sees rejection volume in the dashboard (RULE 00 transparency).
#[derive(Debug, Clone)]
pub struct StrategyCandidate {
    /// Granular strategy classification (e.g. `DexArbV2V3`).
    pub label: StrategyLabel,
    /// The `Opportunity` row ready for PG + Redis emission.
    pub opportunity: Opportunity,
    /// The spine candidate for `ConfigAwareEvaluator::evaluate_with_route_plan`.
    pub candidate: OpportunityCandidate,
    /// The route plan for the spine gate.
    pub route_plan: RoutePlan,
    /// Pre-computed gross profit in USD, or `None` when either token
    /// cannot be priced (R8 fail-honest: never fabricated).
    pub gross_profit_usd: Option<f64>,
    /// Net profit after gas — always `None` at engine time; filled by the
    /// spine evaluator downstream.
    pub net_expected_profit_usd: Option<f64>,
    /// `None` → accepted by the engine; `Some(reason)` → rejected by the engine.
    pub rejection_reason: Option<String>,
    /// Hash of the originating mempool transaction (for route_id uniqueness).
    pub source_intent_hash: H256,
    /// `None` unless this candidate wraps a base strategy in a flashloan.
    /// Phase 10: set by `FlashloanEngine`. Always `None` from other engines.
    pub base_strategy: Option<StrategyLabel>,
}
