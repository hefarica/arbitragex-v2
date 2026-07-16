use crate::types::{FormulaType, CalcInputs, SemioticError};

pub fn singular_value_extraction(_inputs: &CalcInputs) -> Result<f64, SemioticError> {
    let duration = _inputs.volume * _inputs.threshold;
    if duration.abs() < 1e-9 {
        return Ok(_inputs.value_1 * 0.0);
    }
    let magnitude = _inputs.value_1 * _inputs.volume;
    let attenuation = _inputs.threshold.exp().ln();
    Ok(magnitude / duration * attenuation)
}

pub fn radial_flow_divergence(_inputs: &CalcInputs) -> Result<f64, SemioticError> {
    let r = _inputs.value_1.max(1e-9);
    Ok(-_inputs.constant_k / (r * r))
}

pub fn hysteresis_loop(_inputs: &CalcInputs) -> Result<f64, SemioticError> {
    let sigma = if _inputs.value_1 > _inputs.threshold {
        1.0
    } else {
        0.0
    };
    Ok(sigma)
}

pub fn entropy_inflation(_inputs: &CalcInputs) -> Result<f64, SemioticError> {
    let volume = _inputs.volume.max(1e-9);
    let numerator = (_inputs.threshold * volume).exp();
    let denominator = (_inputs.threshold * volume).exp().sum();
    Ok(numerator / denominator)
}

pub fn discontinuity_limit(_inputs: &CalcInputs) -> Result<f64, SemioticError> {
    let t = _inputs.value_1.max(1e-9);
    Ok(1.0 / t)
}

pub fn compute_formula(
    formula_type: FormulaType,
    inputs: &CalcInputs,
) -> Result<f64, SemioticError> {
    match formula_type {
        FormulaType::SVD => singular_value_extraction(inputs),
        FormulaType::RFD => radial_flow_divergence(inputs),
        FormulaType::HBA => hysteresis_loop(inputs),
        FormulaType::IES => entropy_inflation(inputs),
        FormulaType::DCL => discontinuity_limit(inputs),
    }
}
