-- Migration 074: Register APEX Phase 1.5 thermodynamic strategy modules
-- Modules: SVS, DLP, Triangular Atomic, Funding Rate Alignment
-- Date: 2026-05-22
-- Constraint: All evaluation is server-side in searcher-rs. No external proxies.

-- 1. Register strategies in strategy_catalog
INSERT INTO strategy_catalog (
    strategy_key,
    display_name,
    description,
    classification,
    lifecycle_status,
    tags,
    created_at
) VALUES
(
    'strat_svs_01',
    'Estabilización de Volatilidad Estocástica',
    'Amortigua volatilidad asimilando impacto macro y redistribuyendo post-convergencia. '
    'Detecta shocks de precio en pools correlacionados y captura el rebalanceo residual '
    'después de la convergencia. Toda la evaluación matricial reside en searcher-rs.',
    'PERMITTED',
    'scaffold',
    ARRAY['volatility_smoothing', 'post-convergence', 'server-side', 'stochastic'],
    NOW()
),
(
    'strat_dlp_01',
    'Provisión Liquidez Determinística EKF',
    'Inyección pasiva JIT en tick exacto para evitar fragmentación de estado. '
    'Usa un Extended Kalman Filter (EKF) para predecir el próximo tick de precio '
    'y posicionar liquidez exactamente donde el siguiente swap aterrizará. '
    'Estado EKF mantenido server-side en searcher-rs.',
    'PERMITTED',
    'scaffold',
    ARRAY['dynamic_liquidity', 'jit', 'ekf', 'v3-ticks', 'server-side'],
    NOW()
),
(
    'strat_tri_01',
    'Descomposición Triangular Atómica',
    'Nivelación de ineficiencias de enrutamiento A->B->C en un solo manifold. '
    'Variante APEX del motor triangular Phase 9: bundle atómico de 3 hops en '
    'un único DEX manifold, minimizando gas y eliminando race conditions entre pools. '
    'Spot product S = γ³·∏(R_out/R_in) calculado server-side.',
    'PERMITTED',
    'scaffold',
    ARRAY['atomic_arb', 'triangular', 'single-manifold', 'server-side', 'calldata-optimized'],
    NOW()
),
(
    'strat_fr_01',
    'Alineación de Tasa de Financiamiento',
    'Captura de delta entre Perps (CEX/DEX) y Spot mediante cobertura atómica. '
    'Detecta divergencias entre funding rates de futuros perpetuos y la tasa implícita '
    'del spot/borrow market. Construye posición delta-neutral: long spot + short perp. '
    'Circuit breaker integrado para caídas de API CEX.',
    'PERMITTED',
    'scaffold',
    ARRAY['hedging_arb', 'funding-rate', 'perps', 'delta-neutral', 'circuit-breaker'],
    NOW()
)
ON CONFLICT (strategy_key) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status,
    tags = EXCLUDED.tags;

-- 2. Register strategy_configs with default parameters
INSERT INTO strategy_configs (strategy_key, config_json, activated_by, created_at)
VALUES
(
    'strat_svs_01',
    '{
        "min_profit_usd": 8.0,
        "min_shock_pct": 2.0,
        "lookback_blocks": 10,
        "phase": "1.5",
        "emit_candidates": false,
        "server_side_only": true
    }',
    'system_migration_074',
    NOW()
),
(
    'strat_dlp_01',
    '{
        "min_profit_usd": 6.0,
        "process_noise_q": 0.001,
        "measurement_noise_r": 0.1,
        "max_p_trace": 50.0,
        "phase": "1.5",
        "emit_candidates": false,
        "server_side_only": true
    }',
    'system_migration_074',
    NOW()
),
(
    'strat_tri_01',
    '{
        "min_profit_usd": 7.0,
        "min_spot_product": 1.001,
        "atomic_bundle": true,
        "phase": "1.5",
        "emit_candidates": false,
        "server_side_only": true
    }',
    'system_migration_074',
    NOW()
),
(
    'strat_fr_01',
    '{
        "min_profit_usd": 12.0,
        "min_rate_diff_pct": 0.01,
        "max_data_age_secs": 3600,
        "circuit_breaker_secs": 30,
        "cex_provider": "binance",
        "phase": "1.5",
        "emit_candidates": false,
        "server_side_only": true
    }',
    'system_migration_074',
    NOW()
)
ON CONFLICT (strategy_key) DO NOTHING;
