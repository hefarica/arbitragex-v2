use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormulaType {
    SVD,
    RFD,
    HBA,
    IES,
    DCL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationRequest {
    pub source_text: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResponse {
    pub source_word: String,
    pub translation_type: FormulaType,
    pub mathematical_expression: String,
    pub semantic_explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemioticConfig {
    pub formula_tolerance: f64,
    pub enable_caching: bool,
    pub max_latency_ns: u128,
}

#[derive(Debug, Clone)]
pub struct CalcInputs {
    pub value_1: f64,
    pub threshold: f64,
    pub constant_k: f64,
    pub volume: f64,
    pub time: f64,
}

pub struct InputSanitizer;

impl InputSanitizer {
    pub fn sanitize(text: &str) -> String {
        text.trim().to_lowercase()
    }

    pub fn is_valid_word(word: &str) -> bool {
        word.chars().all(|c| c.is_alphanumeric() || c == ' ')
    }
}

impl Default for SemioticConfig {
    fn default() -> Self {
        Self {
            formula_tolerance: 1e-6,
            enable_caching: true,
            max_latency_ns: 20_000_000,
        }
    }
}
