//! Matemáticas para enrutamiento (Multi-hop, Bellman-Ford, Cycle Detection)

/// Representa una pierna (salto) dentro de una ruta de arbitraje.
#[derive(Debug, Clone)]
pub struct RouteLeg {
    pub token_in: String,
    pub token_out: String,
    pub rate: f64, // Ej: 1 Token_In = X Token_Out (antes de fees, o incluyendo fees dependiendo del contexto)
    pub fee_pct: f64,
    pub gas_cost_usd: f64,
}

/// Calcula el producto de tasas a lo largo de una ruta para determinar si existe una oportunidad bruta.
pub fn calc_route_rate_product(legs: &[RouteLeg]) -> f64 {
    let mut product = 1.0;
    for leg in legs {
        product *= leg.rate * (1.0 - leg.fee_pct);
    }
    product
}

/// Convierte una tasa en un "peso" para algoritmos de detección de ciclos de peso negativo (Bellman-Ford).
/// Si queremos maximizar el producto r1 * r2 * r3 > 1,
/// equivale a minimizar -ln(r1) - ln(r2) - ln(r3) < 0.
pub fn rate_to_bellman_ford_weight(rate: f64, fee_pct: f64) -> f64 {
    if rate <= 0.0 {
        return f64::INFINITY;
    }
    let effective_rate = rate * (1.0 - fee_pct);
    if effective_rate <= 0.0 {
        return f64::INFINITY;
    }
    -(effective_rate.ln())
}

/// Valida de forma rápida si una ruta (ej: A -> B -> C -> A) es rentable en bruto.
/// Es el paso previo antes de hacer la simulación profunda de Liquidez y Slippage.
pub fn is_profitable_cycle(legs: &[RouteLeg], total_risk_buffer_pct: f64) -> bool {
    let product = calc_route_rate_product(legs);
    // Para ser rentable, el producto debe ser > 1.0 + buffers
    product > (1.0 + total_risk_buffer_pct)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(token_in: &str, token_out: &str, rate: f64, fee_pct: f64) -> RouteLeg {
        RouteLeg {
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            rate,
            fee_pct,
            gas_cost_usd: 0.0,
        }
    }

    #[test]
    fn test_profitable_cycle() {
        let legs = vec![
            RouteLeg {
                token_in: "USDC".to_string(),
                token_out: "WETH".to_string(),
                rate: 1.0 / 2000.0,
                fee_pct: 0.003,
                gas_cost_usd: 2.0,
            },
            RouteLeg {
                token_in: "WETH".to_string(),
                token_out: "WBTC".to_string(),
                rate: 2000.0 / 60000.0,
                fee_pct: 0.003,
                gas_cost_usd: 2.0,
            },
            RouteLeg {
                token_in: "WBTC".to_string(),
                token_out: "USDC".to_string(),
                rate: 65000.0,
                fee_pct: 0.003,
                gas_cost_usd: 2.0,
            },
        ];

        // (1/2000) * (2000/60000) * 65000 = 65000 / 60000 = 1.0833
        // Fees: 0.997^3 = 0.991
        // Expected product = 1.0833 * 0.991 = 1.0736 (7.36% profit)
        let product = calc_route_rate_product(&legs);
        assert!(product > 1.07);

        assert!(is_profitable_cycle(&legs, 0.02)); // is 7% > 2% buffer? Yes
    }

    // ──────────────────────────────────────────────────────────────────────
    // OMEGA-8/M4 Fase 6: blinda math-engine route_math
    // ──────────────────────────────────────────────────────────────────────

    /// Empty route: product is the multiplicative identity 1.0, so
    /// is_profitable_cycle returns false unless buffer < 0 (which is
    /// nonsensical for risk). Ensures no divide-by-zero / panic.
    #[test]
    fn empty_route_yields_product_one_and_no_arbitrage() {
        let legs: Vec<RouteLeg> = vec![];
        let product = calc_route_rate_product(&legs);
        assert_eq!(product, 1.0);
        assert!(!is_profitable_cycle(&legs, 0.0));
        assert!(!is_profitable_cycle(&legs, 0.01));
    }

    /// Cycle with no arbitrage: round-trip price product == 1.0 (minus fees)
    /// must NOT be classified as profitable.
    #[test]
    fn cycle_without_arbitrage_is_not_profitable() {
        // 1 USDC = 1/2000 WETH; 1 WETH = 2000 USDC → product = 1.0.
        // Adding fees: 0.997^2 < 1.0 → not profitable.
        let legs = vec![
            leg("USDC", "WETH", 1.0 / 2000.0, 0.003),
            leg("WETH", "USDC", 2000.0, 0.003),
        ];
        let product = calc_route_rate_product(&legs);
        assert!(product < 1.0, "with fees, balanced cycle must be < 1");
        assert!(!is_profitable_cycle(&legs, 0.0));
    }

    /// Bellman-Ford weight is +∞ for non-positive rates (impossible to
    /// extract value from a zero-priced pool — fail-honest).
    #[test]
    fn bellman_ford_weight_zero_rate_is_inf() {
        assert!(rate_to_bellman_ford_weight(0.0, 0.0).is_infinite());
        assert!(rate_to_bellman_ford_weight(-1.0, 0.0).is_infinite());
        // Fee of 1.0 (100%) zeroes the effective rate, also → +∞.
        assert!(rate_to_bellman_ford_weight(2.0, 1.0).is_infinite());
    }

    /// Bellman-Ford weight is finite and negative for a profitable rate
    /// > 1 (after fees). This is the standard cycle-negative-weight setup.
    #[test]
    fn bellman_ford_weight_profitable_rate_is_negative() {
        let w = rate_to_bellman_ford_weight(1.05, 0.003);
        assert!(w.is_finite());
        assert!(w < 0.0, "profitable rate must produce negative weight: {w}");
    }

    /// Extreme fees collapse the product to 0 — must still not panic,
    /// must return product = 0.0.
    #[test]
    fn extreme_fees_collapse_product_to_zero() {
        let legs = vec![
            leg("A", "B", 2.0, 1.0), // 100% fee
            leg("B", "A", 0.5, 0.0),
        ];
        let product = calc_route_rate_product(&legs);
        assert_eq!(product, 0.0);
        assert!(!is_profitable_cycle(&legs, 0.0));
    }

    /// Repeated tokens in the route (e.g. A→A direct quote) — the rate
    /// product formula does NOT special-case this; the caller is
    /// responsible for cycle detection. We assert the math composes as
    /// expected and does not panic on identical token labels.
    #[test]
    fn repeated_tokens_compose_arithmetically() {
        let legs = vec![leg("A", "A", 1.001, 0.0), leg("A", "A", 1.001, 0.0)];
        let product = calc_route_rate_product(&legs);
        assert!((product - 1.001 * 1.001).abs() < 1e-12);
    }
}
