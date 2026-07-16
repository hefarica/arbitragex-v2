pub mod types;
pub mod formulas;
pub mod api;
pub mod errors;

pub use api::create_router;
pub use types::{TranslationRequest, TranslationResponse, InputSanitizer};
pub use formulas::{SVD, RFD, HBA, IES, DCL};
pub use errors::SemioticError;

static WORD_TO_FORMULA_MAP: &[(&str, FormulaType)] = &[
    ("sacar", FormulaType::SVD),
    ("drenar", FormulaType::RFD),
    ("manipular", FormulaType::HBA),
    ("inflar", FormulaType::IES),
    ("rug pull", FormulaType::DCL),
];

pub const FORMULA_PRECISION: f64 = 1e-6;

pub fn guard_formula_latency(latency_ns: u128) -> Result<(), SemioticError> {
    const MAX_LATENCY_NS: u128 = 20_000_000;
    if latency_ns > MAX_LATENCY_NS {
        return Err(SemioticError::LatencyExceeded {
            actual_ns: latency_ns,
            max_ns: MAX_LATENCY_NS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_formula_mappings_exist() {
        assert!(!WORD_TO_FORMULA_MAP.is_empty());
    }
}
