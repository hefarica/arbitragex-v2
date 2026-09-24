use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScoringError {
    #[error("Negative expected profit")]
    NegativeProfit,
    #[error("Missing data required for scoring")]
    MissingData,
    /// MATH-07: non-finite evidence (NaN/Inf in any scoring input) — the
    /// ranker needs a total order, so this is an Err, never a NaN score.
    #[error("Non-finite evidence value (NaN/Inf) rejected")]
    InvalidEvidence,
}

#[derive(Error, Debug)]
pub enum GateError {
    #[error("Validation failed")]
    ValidationFailed,
}
