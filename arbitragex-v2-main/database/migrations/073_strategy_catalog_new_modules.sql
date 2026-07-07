-- Migration 073: Register new strategy modules in the catalog
-- Modules: Backrunning no-extractivo, Spatial Cross-Pool, CEX-DEX Convergence
-- Date: 2026-05-22
-- Revised: 2026-06-07 — rewritten against the REAL strategy_catalog schema.
--
-- WHY THIS WAS REWRITTEN (fix-forward, same class as the 019/053 chain bug):
--   The original 073 INSERTed columns (strategy_key, classification, tags) that
--   strategy_catalog (migration 028 + 035) never defines, and INSERTed into a
--   `strategy_configs` TABLE that does not exist — `strategy_configs` is a JSONB
--   COLUMN on trading_config (migration 056), not a table. The migrate.sh chain
--   therefore aborted here (ON_ERROR_STOP=1), turning the integration CI gate RED.
--   Real schema (the contract strategy-catalog.ts reads): kind (PK), display_name,
--   description, category (NOT NULL CHECK), is_implemented, is_default, risk_level
--   (NOT NULL CHECK), capital_required, expected_complexity, competitive_advantage,
--   ethical_constraint, skill_refs TEXT[], requires_flashloan, chains_supported,
--   lifecycle_status (CHECK incl. 'scaffold').
--
-- Doctrine: catalog rows are metadata only. Runtime enable lives in
--   trading_config.enabled_strategies and per-strategy config in
--   trading_config.strategy_configs JSONB — set by the operator via the admin
--   PUT path, NEVER seeded by a migration (arbx-no-hardcode-doctrine). is_implemented
--   stays FALSE until the searcher hot-path actually runs the engine (fail-honest).

-- 1. Backrunning no-extractivo (zero-victim residual arbitrage)
--    Ethics: PERMITTED per arbx-mev-ethics-gate (#2 backrun that does not extract
--    from the originator). The user's swap is already settled; we only level the
--    residual post-state imbalance. If sim detects ANY impact on the target user's
--    price, the bundle is discarded — zero front-running.
INSERT INTO strategy_catalog (
    kind, display_name, description, category, is_implemented, is_default,
    risk_level, ethical_constraint, lifecycle_status, requires_flashloan
) VALUES (
    'backrun_no_extractive',
    'Backrunning No-Extractivo (Zero Victim)',
    'Captura el arbitraje residual estrictamente después de que un swap masivo ha finalizado. '
    'La transacción del usuario ya ha recibido su precio de liquidación. '
    'El motor detecta el desequilibrio post-estado y ejecuta un reverse-swap para nivelar el pool. '
    'Condición de fallo: si la simulación detecta cualquier impacto en el precio del usuario objetivo, '
    'el bundle se descarta inmediatamente. Zero front-running garantizado.',
    'mev-share', FALSE, FALSE,
    'medium', 'none', 'scaffold', FALSE
) ON CONFLICT (kind) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status;

-- 2. Spatial Cross-Pool Synchronization
--    Ethics: PERMITTED per arbx-mev-ethics-gate (#1 cross-pool arbitrage that
--    restores natural price divergence between isolated pools).
INSERT INTO strategy_catalog (
    kind, display_name, description, category, is_implemented, is_default,
    risk_level, ethical_constraint, lifecycle_status, requires_flashloan
) VALUES (
    'spatial_cross_pool',
    'Sincronización Topológica Cross-Pool',
    'Arbitraje espacial entre dos pools de liquidez aislados (ej. Uniswap V3 vs Sushiswap V2). '
    'Opera exclusivamente con datos de estado confirmados. No lee intenciones de usuarios. '
    'Ejecuta flashloan atómico: compra en el pool deprimido, venta en el pool sobrecalentado. '
    'Motor de enrutamiento espacial puro, optimizado en Yul para minimizar calldatas.',
    'atomic', FALSE, FALSE,
    'medium', 'none', 'scaffold', TRUE
) ON CONFLICT (kind) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status;

-- 3. CEX-DEX Hybrid Convergence
--    Ethics: PERMITTED per arbx-mev-ethics-gate (#6 CEX-DEX arbitrage where the
--    CEX side is our own account and no on-chain user is harmed).
INSERT INTO strategy_catalog (
    kind, display_name, description, category, is_implemented, is_default,
    risk_level, ethical_constraint, lifecycle_status, requires_flashloan
) VALUES (
    'hybrid_cex_dex',
    'Convergencia Híbrida CEX-DEX',
    'Arbitraje del diferencial macroeconómico entre precio off-chain (CEX: Binance/Bybit) '
    'y precio on-chain (DEX). Utiliza infraestructura propietaria. Ningún usuario on-chain se ve afectado. '
    'El sistema actúa como Market Maker orgánico. Requiere inventario propio pre-fijado en exchanges. '
    'Incluye circuit breakers estrictos para caídas de API CEX o latencia RPC excesiva.',
    'cross-venue', FALSE, FALSE,
    'high', 'none', 'scaffold', FALSE
) ON CONFLICT (kind) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status;

-- 4. Per-strategy runtime config: NOT seeded here.
--    `strategy_configs` is a JSONB column on trading_config (migration 056), keyed
--    by strategy_kind, set by the operator via PUT /admin/trading-config and hot-
--    mirrored to Redis. Seeding operator config in a migration would violate
--    arbx-no-hardcode-doctrine; the column defaults to '{}' and the operator
--    enables + configures each strategy at runtime.
