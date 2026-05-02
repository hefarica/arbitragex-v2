//! Matemáticas Puras para AMMs (Automated Market Makers)

/// Calcula el `amount_out` para un trade exacto en un pool UniV2 (x * y = k)
/// Asume que los reserves y amounts ya están escalados/normalizados (mismo decimal) o manejados externamente.
pub fn calc_univ2_amount_out(
    amount_in: f64,
    reserve_in: f64,
    reserve_out: f64,
    fee_pct: f64,
) -> f64 {
    if amount_in <= 0.0 || reserve_in <= 0.0 || reserve_out <= 0.0 {
        return 0.0;
    }
    
    // El fee_multiplier es (1 - fee_pct), típicamente 0.997 (para 0.3%)
    let fee_multiplier = 1.0 - fee_pct;
    let amount_in_with_fee = amount_in * fee_multiplier;
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in + amount_in_with_fee;
    
    numerator / denominator
}

/// Estima el Price Impact de un swap en un pool UniV2.
pub fn calc_univ2_price_impact(
    amount_in: f64,
    reserve_in: f64,
    fee_pct: f64,
) -> f64 {
    if amount_in <= 0.0 || reserve_in <= 0.0 {
        return 0.0;
    }
    
    let fee_multiplier = 1.0 - fee_pct;
    let amount_in_with_fee = amount_in * fee_multiplier;
    
    // Price Impact en Constant Product Market Maker = amount_in_with_fee / (reserve_in + amount_in_with_fee)
    amount_in_with_fee / (reserve_in + amount_in_with_fee)
}

/// Aproximación simplificada de matemática de Ticks para UniV3.
/// En la realidad on-chain se usaría Q64.96. Esto es para estimación rápida pre-trade.
pub fn approx_univ3_amount_out(
    amount_in: f64,
    liquidity_active: f64,
    current_sqrt_price_x96: f64,
    _target_sqrt_price_x96: f64, // simplified
    _fee_pct: f64,
) -> f64 {
    // ESTO ES UN PLACEHOLDER MATEMÁTICO para cálculos de alto nivel off-chain.
    // La ejecución on-chain usa `getAmountOut` del Quoter.
    if liquidity_active <= 0.0 || current_sqrt_price_x96 <= 0.0 {
        return 0.0;
    }
    // Implementación mínima para satisfacer el type checker y estructura
    let mut amount_out = amount_in * 0.99; // Dummy logic
    if amount_out < 0.0 {
        amount_out = 0.0;
    }
    amount_out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_univ2_amount_out() {
        // Pool con 1000 ETH (reserve_in) y 2_000_000 USDC (reserve_out)
        // Precio aprox 2000 USDC por ETH.
        // Metemos 1 ETH con 0.3% fee
        let out = calc_univ2_amount_out(1.0, 1000.0, 2_000_000.0, 0.003);
        // Numerator: (1 * 0.997) * 2000000 = 1994000
        // Denominator: 1000 + 0.997 = 1000.997
        // Out: 1994000 / 1000.997 = 1992.0139
        assert!((out - 1992.0139).abs() < 0.01);
    }
    
    #[test]
    fn test_univ2_price_impact() {
        let impact = calc_univ2_price_impact(100.0, 1000.0, 0.003);
        // amount_in_fee = 99.7
        // impact = 99.7 / 1099.7 = ~9.06%
        assert!((impact - 0.09066).abs() < 0.001);
    }
}
