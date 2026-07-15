//! Gate subsystem shared types and traits.
//!
//! Defines the core abstractions for gate evaluation:
//! - `GateOutcome`: Result of gate evaluation (reject/pass with metadata)
//! - `GateLogic`: Trait for implementing gate logic
//!
//! These types are used by:
//! - `crate::gates`: Concrete gate implementations (MacroMevGate, etc.)
//! - Orchestrator: High-level gate coordination
//!
//! Rule 00 compliance: All gates produce deterministic outcomes based on
//! real data. No mocked telemetry allowed.

use serde::{Deserialize, Serialize};

/// Outcome of a gate evaluation.
///
/// Gates are pure functions that evaluate an opportunity and produce either:
/// - `reject: true` → opportunity blocked with reason and mitigation
/// - `reject: false` (via None return) → opportunity passes this gate
///
/// ## Fail-Honest Pattern (Rule 08)
/// - `reject: true` → gate computed and determined rejection is appropriate
/// - `Some(outcome)` with `reject: false` → computed but no rejection needed
/// - `None` → no computation performed (gate disabled or data missing)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GateOutcome {
    /// Whether this gate rejects the opportunity
    pub reject: bool,

    /// Machine-readable rejection reason
    pub reason: RejectReason,

    /// Recommended mitigation action
    pub mitigation: String,

    /// Whether operator can override this gate decision
    pub can_override: bool,

    /// Hash of gate computation for audit trail
    pub gate_score_hash: u64,
}

impl GateOutcome {
    /// Create a hard-rejection outcome (cannot be overridden)
    pub fn hard_reject(reason: RejectReason, mitigation: impl Into<String>) -> Self {
        Self {
            reject: true,
            reason,
            mitigation: mitigation.into(),
            can_override: false,
            gate_score_hash: 0,
        }
    }

    /// Create a soft-rejection outcome (can be overridden by operator)
    pub fn soft_reject(reason: RejectReason, mitigation: impl Into<String>) -> Self {
        Self {
            reject: true,
            reason,
            mitigation: mitigation.into(),
            can_override: true,
            gate_score_hash: 0,
        }
    }
}

/// Trait for implementing gate evaluation logic.
///
/// Gates are pure, deterministic functions.
pub trait GateLogic {
    /// Configuration type for this gate
    type Config;

    /// Candidate/opportunity type being evaluated
    type Candidate;

    /// Create a new gate instance with the given configuration
    fn new_config(config: Self::Config) -> Self;

    /// Evaluate a candidate opportunity against this gate.
    fn evaluate(&self, candidate: &Self::Candidate, config: &Self::Config) -> Option<GateOutcome>;
}

/// Rejection reasons for gate evaluations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RejectReason {
    /// Net yield insufficient to cover gas costs
    InsufficientProfitToCoverGas,

    /// High probability of MEV confiscation
    HighConfiscationRisk,

    /// Pool/reserve state is stale
    StaleState,

    /// Token risk score exceeds maximum threshold
    TokenRiskTooHigh,

    /// Slippage estimate exceeds configured maximum
    ExcessiveSlippage,

    /// Gas price volatility too high
    GasVolatilityHigh,

    /// Simulation failed
    SimulationFailed,

    /// Bundle simulation failed
    BundleSimulationFailed,

    /// Kill switch active
    KillSwitchActive,

    /// Rate limit exceeded
    RateLimitExceeded,

    /// Circuit breaker open
    CircuitBreakerOpen,

    /// Opportunity expired
    OpportunityExpired,
}

impl RejectReason {
    /// Get stable tag for metrics and dashboards
    pub fn tag(&self) -> &'static str {
        match self {
            RejectReason::InsufficientProfitToCoverGas => "insufficient_profit",
            RejectReason::HighConfiscationRisk => "high_confiscation_risk",
            RejectReason::StaleState => "stale_state",
            RejectReason::TokenRiskTooHigh => "token_risk_high",
            RejectReason::ExcessiveSlippage => "excessive_slippage",
            RejectReason::GasVolatilityHigh => "gas_volatility_high",
            RejectReason::SimulationFailed => "simulation_failed",
            RejectReason::BundleSimulationFailed => "bundle_simulation_failed",
            RejectReason::KillSwitchActive => "kill_switch_active",
            RejectReason::RateLimitExceeded => "rate_limit_exceeded",
            RejectReason::CircuitBreakerOpen => "circuit_breaker_open",
            RejectReason::OpportunityExpired => "opportunity_expired",
        }
    }
}
