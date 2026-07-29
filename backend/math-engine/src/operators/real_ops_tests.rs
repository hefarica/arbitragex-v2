//! Validation tests for the 3 real math operators (op_13 regression,
//! op_16 kelly, op_22 monte carlo). These replaced the old stubs with real
//! formulas — assert they compute honest values on synthetic market states
//! and return None (fail-honest) on insufficient data.

#[cfg(test)]
mod tests {
    use crate::operators::{
        MarketState, OperatorRegistry, TopologicalOperator,
        op_13_regression::RegressionOperator, op_16_kelly::KellyOperator,
        op_22_monte_carlo::MonteCarloOperator,
    };
    use std::collections::HashMap;

    fn state_from_prices(prices: &[f64]) -> MarketState {
        MarketState {
            price_matrix: prices.iter().map(|p| vec![*p]).collect(),
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: HashMap::new(),
        }
    }

    fn trending_up_state() -> MarketState {
        // Clear uptrend: 100 → 110 in unit steps.
        state_from_prices(&[100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0])
    }

    fn volatile_state() -> MarketState {
        // Mixed wins/losses for Kelly.
        state_from_prices(&[100.0, 102.0, 99.0, 103.0, 98.0, 104.0, 100.0, 105.0, 101.0, 106.0])
    }

    // ── op_22 Monte Carlo (GBM) ─────────────────────────────────────────────
    #[test]
    fn monte_carlo_computes_on_valid_state() {
        let op = MonteCarloOperator::new();
        let out = op.evaluate(&trending_up_state());
        assert_eq!(out.operator_id, 22);
        assert!(out.scalar_value.is_some(), "expected computed std_dev");
        let sd = out.scalar_value.unwrap();
        assert!(sd >= 0.0, "std_dev must be non-negative");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert!(out.metadata.contains_key("expected_pv"));
        assert!(out.metadata.contains_key("p5_tail"));
    }

    #[test]
    fn monte_carlo_none_on_insufficient_data() {
        let op = MonteCarloOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0])); // < 3 prices
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    #[test]
    fn monte_carlo_deterministic_per_state() {
        let op = MonteCarloOperator::new();
        let a = op.evaluate(&trending_up_state());
        let b = op.evaluate(&trending_up_state());
        // Same block_number + same prices → same seed → identical output.
        assert_eq!(a.scalar_value, b.scalar_value);
    }

    // ── op_16 Kelly ─────────────────────────────────────────────────────────
    #[test]
    fn kelly_computes_on_mixed_state() {
        let op = KellyOperator::new();
        let out = op.evaluate(&volatile_state());
        assert_eq!(out.operator_id, 16);
        assert!(out.scalar_value.is_some());
        let f = out.scalar_value.unwrap();
        assert!((0.0..=1.0).contains(&f), "f_star must be clamped to [0,1]");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert!(out.metadata.contains_key("b_odds"));
    }

    #[test]
    fn kelly_none_when_no_losses() {
        let op = KellyOperator::new();
        // All wins → no losses → b undefined → None.
        let out = op.evaluate(&trending_up_state());
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── op_13 Regression (least squares) ────────────────────────────────────
    #[test]
    fn regression_computes_slope_on_trend() {
        let op = RegressionOperator::new();
        let out = op.evaluate(&trending_up_state());
        assert_eq!(out.operator_id, 13);
        assert!(out.scalar_value.is_some());
        let slope = out.metadata.get("slope").copied().unwrap_or(0.0);
        assert!(slope > 0.0, "uptrend must yield positive slope");
        let r2 = out.metadata.get("r_squared").copied().unwrap_or(0.0);
        assert!(r2 > 0.9, "clean linear trend must have high R² (got {r2})");
        assert_eq!(out.metadata.get("direction"), Some(&1.0)); // UP
    }

    #[test]
    fn regression_none_on_insufficient_data() {
        let op = RegressionOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0, 101.0])); // < 3
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── Registry integration ────────────────────────────────────────────────
    #[test]
    fn registry_dispatches_the_three_real_ops() {
        let registry = OperatorRegistry::new();
        for id in [13u8, 16, 22] {
            let out = registry.dispatch(id, &trending_up_state());
            assert!(out.is_some(), "operator {id} must be registered");
            assert_eq!(out.unwrap().operator_id, id);
        }
    }
}
