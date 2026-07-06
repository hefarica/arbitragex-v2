use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScoringError {
    #[error("Negative expected profit")]
    NegativeProfit,
    #[error("Missing data required for scoring")]
    MissingData,
}

#[derive(Error, Debug)]
pub enum GateError {
    #[error("Validation failed")]
    ValidationFailed,
}
