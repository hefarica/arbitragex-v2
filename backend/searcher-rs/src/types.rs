//! Core types for searcher-rs.
//!
//! Defines types specific to the searcher that are not re-exported from shared-rs,
//! particularly gate-related types and opportunity candidate structures.

use serde::{Deserialize, Serialize};

/// Opportunity candidate for gate evaluation.
///
/// This is a minimal subset of fields needed by gates to evaluate
/// whether an opportunity should proceed. It carries:
/// - Yield estimates (net/gross)
/// - Gas pricing
/// - Usage estimates
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpportunityCandidate {
    /// Net yield after all costs (USD)
    pub net_yield: Option<f64>,

    /// Gross yield before costs (USD)
    pub gross_yield: Option<f64>,

    /// Gas price estimate (USD per ETH unit)
    pub gas_price: Option<f64>,

    /// Estimated gas units for execution
    pub gas_used_estimate: Option<u64>,
}

/// Execution decision outcomes.
///
/// Represents the final decision after all gates have evaluated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionDecision {
    /// Proceed with execution
    Execute,

    /// Requires deeper simulation before execution
    SimulateDeeper,

    /// Hold pending further evaluation
    Hold,

    /// Reject this opportunity
    Reject,
}

// Re-export canonical rejection reasons from the shared gate layer to avoid
// duplicate/incompatible type definitions (Rule 00: single source of truth).
pub use crate::shared::gates::RejectReason;
