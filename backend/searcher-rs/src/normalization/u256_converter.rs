use ethers::types::U256;
use std::cmp;

/// PUENTE ISOMÓRFICO U256 → f64
/// Fase 1: Conversión a f64 con pérdida controlada de precisión
/// Fase 2: Normalización por escala de token (decimals)
///
/// INVARIANTE: El valor f64 resultante representa la cantidad HUMANA-LEÍBLE
/// del token (ej: 1.5 ETH, no 1500000000000000000 wei).
///
/// LÍMITE TÉRMICO: U256 max ≈ 1.15e77. f64 max ≈ 1.79e308.
/// No hay overflow, pero sí pérdida de precisión en los 15 dígitos menos significativos.
/// Para operaciones de convergencia estocástica, esto es ACEPTABLE porque operamos en rangos 1e-18 a 1e12.

const F64_MANTISSA_BITS: u32 = 53;

#[derive(Debug)]
pub enum NormalizationError {
    Overflow,
    InvalidDecimals,
}

impl std::fmt::Display for NormalizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NormalizationError::Overflow => write!(f, "U256 value exceeds f64 representable range"),
            NormalizationError::InvalidDecimals => {
                write!(f, "Token decimals out of valid range [0, 18]")
            }
        }
    }
}

impl std::error::Error for NormalizationError {}

/// Fase 1: U256 crudo → f64 bruto (escala wei)
pub fn u256_to_f64_raw(value: U256) -> Result<f64, NormalizationError> {
    // Estrategia: convertir usando string intermedio para máxima fidelidad
    // en el rango válido de f64.
    let s = value.to_string();
    match s.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(NormalizationError::Overflow),
    }
}

/// Fase 2: Normalización por decimales del token
/// decimals: 18 para ETH/WETH, 6 para USDC/USDT, etc.
pub fn u256_to_f64_normalized(value: U256, decimals: u8) -> Result<f64, NormalizationError> {
    if decimals > 18 {
        return Err(NormalizationError::InvalidDecimals);
    }

    let raw = u256_to_f64_raw(value)?;
    let divisor = 10_f64.powi(decimals as i32);

    Ok(raw / divisor)
}

/// Versión simplificada (asume 18 decimales para ETH-native)
pub fn u256_to_f64_normalized_default(value: U256) -> Result<f64, NormalizationError> {
    u256_to_f64_normalized(value, 18)
}

/// Conversión inversa (f64 → U256) — USAR SOLO para simulación dry-run
/// donde necesitamos re-escalar para enviar a la EVM.
pub fn f64_to_u256_denormalized(value: f64, decimals: u8) -> Result<U256, NormalizationError> {
    if decimals > 18 || value < 0.0 || !value.is_finite() {
        return Err(NormalizationError::InvalidDecimals);
    }

    let multiplier = 10_f64.powi(decimals as i32);
    let scaled = value * multiplier;

    // Truncar a entero — la EVM no acepta fracciones de wei
    let scaled_u128 = scaled as u128;
    Ok(U256::from(scaled_u128))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_eth() {
        let one_eth = U256::from(10).pow(U256::from(18));
        let normalized = u256_to_f64_normalized(one_eth, 18).unwrap();
        assert!((normalized - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_usdc_amount() {
        let one_usdc = U256::from(1_000_000); // 1 USDC = 1e6 units
        let normalized = u256_to_f64_normalized(one_usdc, 6).unwrap();
        assert!((normalized - 1.0).abs() < 1e-10);
    }
}
