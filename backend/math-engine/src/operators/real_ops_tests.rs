//! Validation tests for the 3 real math operators (op_13 regression,
//! op_16 kelly, op_22 monte carlo). These replaced the old stubs with real
//! formulas — assert they compute honest values on synthetic market states
//! and return None (fail-honest) on insufficient data.

#[cfg(test)]
mod tests {
    use crate::operators::{
        MarketState, OperatorRegistry, TopologicalOperator,
        op_02_pca::PCAOperator, op_03_eigen::EigenOperator,
        op_04_von_neumann::VonNeumannOperator, op_08_kalman::KalmanOperator,
        op_10_welford::WelfordOperator, op_13_regression::RegressionOperator,
        op_15_golden_section::GoldenSectionOperator,
        op_16_kelly::KellyOperator, op_20_gradient_descent::GradientDescentOperator,
        op_21_newton::NewtonOperator, op_22_monte_carlo::MonteCarloOperator,
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

    // ── multi-asset helper para operadores espectrales (m≥2) ─────────────────
    fn multi_asset_state() -> MarketState {
        // 5 snapshots × 3 activos; asset0 y asset1 colineales (100→104, 200→208),
        // asset2 débil → covarianza no trivial + un modo dominante.
        MarketState {
            price_matrix: vec![
                vec![100.0, 200.0, 50.0],
                vec![101.0, 202.0, 49.0],
                vec![102.0, 204.0, 51.0],
                vec![103.0, 206.0, 48.0],
                vec![104.0, 208.0, 52.0],
            ],
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: HashMap::new(),
        }
    }

    // ── op_02 PCA ───────────────────────────────────────────────────────────
    #[test]
    fn pca_computes_explained_variance_ratio() {
        let op = PCAOperator::new();
        let out = op.evaluate(&multi_asset_state());
        assert_eq!(out.operator_id, 2);
        assert!(out.scalar_value.is_some(), "PCA must compute on multi-asset state");
        let rho1 = out.scalar_value.unwrap();
        assert!(rho1 > 0.0 && rho1 <= 1.0 + 1e-9, "rho1 in (0,1]: {rho1}");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert_eq!(out.vector_result.as_ref().unwrap().len(), 3, "3 eigenvalues");
    }
    #[test]
    fn pca_none_on_insufficient_data() {
        let op = PCAOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0])); // n<2
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── op_03 Eigen ─────────────────────────────────────────────────────────
    #[test]
    fn eigen_computes_spectral_radius() {
        let op = EigenOperator::new();
        let out = op.evaluate(&multi_asset_state());
        assert_eq!(out.operator_id, 3);
        assert!(out.scalar_value.is_some());
        let lam_max = out.scalar_value.unwrap();
        assert!(lam_max > 0.0, "lambda_max > 0: {lam_max}");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
    }
    #[test]
    fn eigen_none_on_insufficient_data() {
        let op = EigenOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── op_04 Von Neumann entropy ───────────────────────────────────────────
    #[test]
    fn von_neumann_entropy_in_range() {
        let op = VonNeumannOperator::new();
        let out = op.evaluate(&multi_asset_state());
        assert_eq!(out.operator_id, 4);
        assert!(out.scalar_value.is_some());
        let s = out.scalar_value.unwrap();
        // S(ρ) ∈ [0, ln m] = [0, ln 3]
        assert!(s >= 0.0 && s <= 3.0_f64.ln() + 1e-9, "S in [0, ln m]: {s}");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert!(out.metadata.contains_key("purity"));
    }
    #[test]
    fn von_neumann_none_on_insufficient_data() {
        let op = VonNeumannOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── op_08 Kalman ────────────────────────────────────────────────────────
    #[test]
    fn kalman_computes_mispricing_zscore() {
        let op = KalmanOperator::new();
        let out = op.evaluate(&volatile_state()); // n=10 single-asset
        assert_eq!(out.operator_id, 8);
        assert!(out.scalar_value.is_some(), "Kalman must compute on n>=3 series");
        let z = out.scalar_value.unwrap();
        assert!(z >= 0.0, "mispricing z-score non-negative: {z}");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert!(out.metadata.contains_key("filtered_mid"));
    }
    #[test]
    fn kalman_none_on_insufficient_data() {
        let op = KalmanOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0, 101.0])); // n<3
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── helper: estado con pool CPMM con edge (γ·p_pool > 1) ─────────────────
    fn pool_state() -> MarketState {
        // r1/r0 = 1.01 ⇒ γ·p_pool = 0.997·1.01 ≈ 1.00697 > 1 (edge detectable).
        // price_matrix col0 ≈ 1.01 ⇒ p_ref ≈ 1.01 (gas cost scale).
        MarketState {
            price_matrix: vec![vec![1.01], vec![1.01], vec![1.01]],
            liquidity_reserves: vec![(1_000_000.0, 1_010_000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: HashMap::new(),
        }
    }

    // ── op_10 Welford (online variance) ──────────────────────────────────────
    #[test]
    fn welford_computes_volatility() {
        let op = WelfordOperator::new();
        let out = op.evaluate(&volatile_state());
        assert_eq!(out.operator_id, 10);
        assert!(out.scalar_value.is_some(), "Welford must compute σ on n>=2 returns");
        let sigma = out.scalar_value.unwrap();
        assert!(sigma >= 0.0, "std_dev non-negative: {sigma}");
        assert!(sigma.is_finite());
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        // vector_result = [mean, variance, n]; variance ≈ σ².
        let v = out.vector_result.as_ref().unwrap();
        assert_eq!(v.len(), 3);
        let var = v[1];
        assert!((var - sigma * sigma).abs() < 1e-9, "variance ≈ σ²");
    }
    #[test]
    fn welford_none_on_insufficient_data() {
        let op = WelfordOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0])); // <3 prices ⇒ <2 returns
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── op_15 Golden Section (maximize CPMM net yield) ───────────────────────
    #[test]
    fn golden_section_finds_optimal_yield() {
        let op = GoldenSectionOperator::new();
        let out = op.evaluate(&pool_state());
        assert_eq!(out.operator_id, 15);
        assert!(out.scalar_value.is_some(), "golden-section must find x* on edge pool");
        let f_star = out.scalar_value.unwrap();
        assert!(f_star.is_finite());
        // vector_result = [x*, f*, r0]; x* ∈ (0, r0].
        let v = out.vector_result.as_ref().unwrap();
        let x_star = v[0];
        assert!(x_star > 0.0 && x_star <= 1_000_000.0, "x* in (0, r0]: {x_star}");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
    }
    #[test]
    fn golden_section_none_without_reserves() {
        let op = GoldenSectionOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0, 101.0])); // empty reserves
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── op_20 Gradient Descent (minimize Σ(returns−θ)² → mean) ───────────────
    #[test]
    fn gradient_descent_converges_to_mean() {
        let op = GradientDescentOperator::new();
        let out = op.evaluate(&trending_up_state());
        assert_eq!(out.operator_id, 20);
        assert!(out.scalar_value.is_some(), "GD must converge on n>=2 returns");
        let theta = out.scalar_value.unwrap();
        // θ* converge al mean(returns) reportado por el propio operador.
        let mean = out.metadata.get("mean").copied().expect("metadata.mean present");
        assert!((theta - mean).abs() < 1e-6, "θ* ≈ mean(returns)={mean}: {theta}");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        let grad_norm = out.metadata.get("grad_norm").copied().unwrap_or(1.0);
        assert!(grad_norm < 1e-4, "grad_norm ≈ 0 at convergence: {grad_norm}");
    }
    #[test]
    fn gradient_descent_none_on_insufficient_data() {
        let op = GradientDescentOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0])); // <3 prices
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── op_21 Newton-Raphson (break-even size root) ──────────────────────────
    #[test]
    fn newton_finds_break_even_size() {
        let op = NewtonOperator::new();
        let out = op.evaluate(&pool_state());
        assert_eq!(out.operator_id, 21);
        assert!(out.scalar_value.is_some(), "Newton must find break-even root on edge pool");
        let x = out.scalar_value.unwrap();
        assert!(x > 0.0 && x <= 1_000_000.0, "break-even x* in (0, r0]: {x}");
        // residual f(x*) ≈ 0.
        let v = out.vector_result.as_ref().unwrap();
        let residual = v[1];
        assert!(residual.abs() < 1e-6, "|f(x*)| ≈ 0: {residual}");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
    }
    #[test]
    fn newton_none_without_reserves() {
        let op = NewtonOperator::new();
        let out = op.evaluate(&state_from_prices(&[100.0, 101.0])); // empty reserves
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }
    #[test]
    fn newton_none_without_edge() {
        let op = NewtonOperator::new();
        // r1/r0 = 1.0 ⇒ γ·p_pool = 0.997 < 1 ⇒ sin edge ⇒ None.
        let no_edge = MarketState {
            price_matrix: vec![vec![1.0]],
            liquidity_reserves: vec![(1_000_000.0, 1_000_000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: HashMap::new(),
        };
        let out = op.evaluate(&no_edge);
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── Registry integration para los 4 nuevos ──────────────────────────────
    #[test]
    fn registry_dispatches_spectral_and_kalman() {
        let registry = OperatorRegistry::new();
        for id in [2u8, 3, 4, 8] {
            let out = registry.dispatch(id, &multi_asset_state());
            assert!(out.is_some(), "operator {id} must be registered");
            assert_eq!(out.unwrap().operator_id, id);
        }
    }

    // ── Registry integration para los 4 optimizadores numéricos ──────────────
    #[test]
    fn registry_dispatches_numerical_optimizers() {
        let registry = OperatorRegistry::new();
        for id in [10u8, 15, 20, 21] {
            let out = registry.dispatch(id, &pool_state());
            assert!(out.is_some(), "operator {id} must be registered");
            assert_eq!(out.unwrap().operator_id, id);
        }
    }

    // ── Smoke comprehensivo: los 31 operadores dispatchean + son fail-honest ─
    // Ejercita TODOS los operadores (10 preexistentes + 21 recién implementados)
    // sobre un estado rico (multi-asset + reservas con edge + features de queueing)
    // y verifica el contrato fail-honest: scalar Some⇒finito, None⇒honesto, nunca
    // pánico, nunca un NaN/Inf fabricado.
    #[test]
    fn all_31_operators_dispatch_and_are_fail_honest() {
        let registry = OperatorRegistry::new();
        let mut features = HashMap::new();
        // op_23 queueing estable (ρ<1): 0.5 llegadas/bloque, 12s/bloque.
        features.insert("mempool_arrivals_per_block".to_string(), 0.5);
        features.insert("block_time_sec".to_string(), 12.0);
        features.insert("block_time_variance_sec2".to_string(), 4.0);
        features.insert("fee_bps".to_string(), 30.0); // op_15/op_21/op_26 fee
        let rich = MarketState {
            // 7 snapshots × 3 activos →光谱ales/game/control tienen data.
            price_matrix: vec![
                vec![100.0, 200.0, 50.0],
                vec![101.0, 202.0, 49.0],
                vec![102.0, 204.0, 51.0],
                vec![103.0, 206.0, 48.0],
                vec![104.0, 208.0, 52.0],
                vec![105.0, 210.0, 50.0],
                vec![106.0, 212.0, 51.0],
            ],
            // Pool primario con edge (r1/r0=1.05) + referencia 1:1 → op_26 edge.
            liquidity_reserves: vec![(1_000_000.0, 1_050_000.0), (500_000.0, 500_000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features,
        };
        for id in 1u8..=31u8 {
            let out = registry.dispatch(id, &rich);
            assert!(out.is_some(), "operator {id} must be registered + dispatch");
            let o = out.unwrap();
            assert_eq!(o.operator_id, id, "dispatch id mismatch for {id}");
            // Fail-honest contract: Some⇒finito, None⇒honesto (nunca NaN/Inf).
            if let Some(v) = o.scalar_value {
                assert!(v.is_finite(), "operator {id} scalar must be finite: {v}");
            }
        }
    }
}
