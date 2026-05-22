-- Migration 073: Register new strategy modules in the catalog
-- Modules: Backrunning no-extractivo, Spatial Cross-Pool, CEX-DEX Convergence
-- Date: 2026-05-22

-- 1. Register Backrunning no-extractivo
INSERT INTO strategy_catalog (
    strategy_key,
    display_name,
    description,
    classification,
    lifecycle_status,
    tags,
    created_at
) VALUES (
    'backrun_no_extractive',
    'Backrunning No-Extractivo (Zero Victim)',
    'Captura el arbitraje residual estrictamente después de que un swap masivo ha finalizado. '
    'La transacción del usuario ya ha recibido su precio de liquidación. '
    'El motor detecta el desequilibrio post-estado y ejecuta un reverse-swap para nivelar el pool. '
    'Condición de fallo: si la simulación detecta cualquier impacto en el precio del usuario objetivo, '
    'el bundle se descarta inmediatamente. Zero front-running garantizado.',
    'PERMITTED',
    'scaffold',
    ARRAY['backrun', 'post-state-reconciliation', 'zero-victim', 'mev-neutral'],
    NOW()
) ON CONFLICT (strategy_key) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status,
    tags = EXCLUDED.tags;

-- 2. Register Spatial Cross-Pool Synchronization
INSERT INTO strategy_catalog (
    strategy_key,
    display_name,
    description,
    classification,
    lifecycle_status,
    tags,
    created_at
) VALUES (
    'spatial_cross_pool',
    'Sincronización Topológica Cross-Pool',
    'Arbitraje espacial entre dos pools de liquidez aislados (ej. Uniswap V3 vs Sushiswap V2). '
    'Opera exclusivamente con datos de estado confirmados. No lee intenciones de usuarios. '
    'Ejecuta flashloan atómico: compra en el pool deprimido, venta en el pool sobrecalentado. '
    'Motor de enrutamiento espacial puro, optimizado en Yul para minimizar calldatas.',
    'PERMITTED',
    'scaffold',
    ARRAY['spatial-arbitrage', 'atomic-routing', 'dex-to-dex', 'flashloan', 'cross-pool'],
    NOW()
) ON CONFLICT (strategy_key) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status,
    tags = EXCLUDED.tags;

-- 3. Register CEX-DEX Hybrid Convergence
INSERT INTO strategy_catalog (
    strategy_key,
    display_name,
    description,
    classification,
    lifecycle_status,
    tags,
    created_at
) VALUES (
    'hybrid_cex_dex',
    'Convergencia Híbrida CEX-DEX',
    'Arbitraje del diferencial macroeconómico entre precio off-chain (CEX: Binance/Bybit) '
    'y precio on-chain (DEX). Utiliza infraestructura propietaria. Ningún usuario on-chain se ve afectado. '
    'El sistema actúa como Market Maker orgánico. Requiere inventario propio pre-fijado en exchanges. '
    'Incluye circuit breakers estrictos para caídas de API CEX o latencia RPC excesiva.',
    'PERMITTED',
    'scaffold',
    ARRAY['hybrid-liquidity', 'stat-arb', 'hedging', 'cex-dex', 'market-maker'],
    NOW()
) ON CONFLICT (strategy_key) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status,
    tags = EXCLUDED.tags;

-- 4. Add strategy_configs entries with default parameters
INSERT INTO strategy_configs (strategy_key, config_json, activated_by, created_at)
VALUES
(
    'backrun_no_extractive',
    '{"min_profit_usd": 5.0, "max_gas_price_gwei": 150.0, "phase": 1, "emit_candidates": true, "zero_victim_check": true}',
    'system_migration_073',
    NOW()
),
(
    'spatial_cross_pool',
    '{"min_profit_usd": 10.0, "min_divergence_bps": 15, "flashloan_enabled": true, "phase": 1, "emit_candidates": true}',
    'system_migration_073',
    NOW()
),
(
    'hybrid_cex_dex',
    '{"min_profit_usd": 15.0, "min_spread_bps": 20, "cex_provider": "binance", "circuit_breaker_enabled": true, "phase": 1, "emit_candidates": false}',
    'system_migration_073',
    NOW()
)
ON CONFLICT (strategy_key) DO NOTHING;
