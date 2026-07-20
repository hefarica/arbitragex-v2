use crate::types::{FormulaType, InputSanitizer, TranslationResponse};
use axum::{extract::Query, routing::get, Json, Router};
use std::collections::HashMap;

pub fn create_router() -> Router {
    Router::new().route("/api/translate", get(handle_translate))
}

async fn handle_translate(
    Query(params): Query<HashMap<String, String>>,
) -> Json<TranslationResponse> {
    let word = params.get("word").unwrap_or(&"".to_string()).clone();
    let sanitized = InputSanitizer::sanitize(&word);

    let translation_type = match sanitized.as_str() {
        "sacar" | "sacudir" | "extraer" => FormulaType::SVD,
        "drenar" | "lavar" | "cagar dinero" => FormulaType::RFD,
        "manipular" | "controlar" | "carril" => FormulaType::HBA,
        "inflar" | "pump" | "hacer pump" => FormulaType::IES,
        "rug pull" | "scam" | "trap" => FormulaType::DCL,
        _ => {
            return Json(TranslationResponse {
                source_word: sanitized,
                translation_type: FormulaType::DCL,
                mathematical_expression: "\\text{UNKNOWN}".to_string(),
                semantic_explanation:
                    "Term not in precomputed dictionary. Extend formulas.rs to add mapping."
                        .to_string(),
            });
        }
    };

    let (expression, explanation) = match translation_type {
        FormulaType::SVD => (
            "V_{aggressor} = V_{victim} \\times \\sum_{i=1}^{n} \\rho_i \\cdot \\mathbb{1}_{\\lambda_i > \\tau}".to_string(),
            "Singular Value Decomposition: Extracts value eigencomponents from victim manifold, reassigning to aggressor domain".to_string(),
        ),
        FormulaType::RFD => (
            "\\nabla \\cdot L(r) = -\\frac{k}{r^2}".to_string(),
            "Radial Flow Divergence: Liquidity drains radially outward with inverse-square attenuation".to_string(),
        ),
        FormulaType::HBA => (
            "\\sigma_{state} = \\begin{cases} 1 & \\text{if } x > x_{threshold} \\\\ 0 & \\text{otherwise} \\end{cases}".to_string(),
            "Hysteresis Loop Analysis: Binary state toggle with deadband — once switched, requires threshold crossing to revert".to_string(),
        ),
        FormulaType::IES => (
            "P(y|x) = \\frac{e^{\\theta \\cdot V}}{\\sum e^{\\theta \\cdot V}}".to_string(),
            "Entropy Inflation: Stochastic state probability amplified by artificial volume injection".to_string(),
        ),
        FormulaType::DCL => (
            "\\lim_{\\epsilon \\to 0^+} \\frac{V(t)}{t} = \\infty \\text{ but } \\lim_{\\epsilon \\to 0^+} V(t+\\epsilon) = 0".to_string(),
            "Limit Discontinuity: Value explodes at time t yet collapses to zero at t+ε — characteristic of exit scams".to_string(),
        ),
    };

    Json(TranslationResponse {
        source_word: sanitized,
        translation_type,
        mathematical_expression: expression,
        semantic_explanation: explanation,
    })
}
