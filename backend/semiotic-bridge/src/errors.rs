use thiserror::Error;

#[derive(Debug, Error)]
pub enum SemioticError {
    #[error("Invalid input: {source}")]
    InvalidInput { source: String },

    #[error("Formula computation failed: {details}")]
    ComputationFailed { details: String },

    #[error("Latency exceeded: {actual_ns}ns > {max_ns}ns")]
    LatencyExceeded { actual_ns: u128, max_ns: u128 },

    #[error("No matching formula found for word: {word}")]
    NoMatchingFormula { word: String },

    #[error("Functional injection detected in input")]
    InjectionDetected,
}

impl SemioticError {
    pub fn is_timeout(&self) -> bool {
        matches!(self, SemioticError::LatencyExceeded { .. })
    }

    pub fn to_observation(&self) -> String {
        self.to_string()
    }
}
