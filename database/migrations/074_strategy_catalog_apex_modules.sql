-- Migration 074: Register APEX Phase 1.5 thermodynamic strategy modules
-- Modules: SVS, Triangular Atomic, Funding Rate Alignment
-- Date: 2026-05-22
-- Revised: 2026-06-07 — rewritten against the REAL strategy_catalog schema
--   (see 073 header for the full root-cause / fix-forward rationale).
-- Constraint: All evaluation is server-side in searcher-rs. No external proxies.
--
-- ⛔ strat_dlp_01 (Provisión Liquidez Determinística EKF) is DELIBERATELY OMITTED
--   from the catalog. It is a Just-In-Time concentrated-liquidity strategy that
--   predicts the next swap's landing tick (via an EKF) and injects liquidity there
--   to capture that swap's fee — i.e. JIT displacement of an existing, non-consenting
--   third-party LP's accrued fee. That is PROHIBITED by arbx-mev-ethics-gate (the
--   "Liquidez JIT que desplaza la fee acumulada de un LP existente" clause). The
--   platform does not advertise a prohibited strategy as runnable.
--
--   TRUTH-IN-RECORD: unlike jit_v3_worker.rs — fully DELETED in the 2026-06-06
--   MEV-ethics audit (commit 9317c07) — the dlp_engine.rs / dlp_worker.rs code is
--   still PRESENT in-tree behind the non-default `experimental-engines` feature
--   (compiled out of the production build; default = ["v2-simulator"]). Its full
--   deletion plus a mev-ethics-lint rule against `strat_dlp_01` / EKF-tick JIT
--   injection are tracked as a REQUIRED follow-up. Do NOT re-add strat_dlp_01 to
--   the catalog without an explicit written ethics override per the gate's Override
--   protocol.

-- 1. Estabilización de Volatilidad Estocástica (SVS)
--    Ethics: PERMITTED per arbx-mev-ethics-gate — captures the residual rebalance
--    AFTER convergence (post-state); no specific user is targeted. Falls under the
--    PERMITTED categories "backrun that does not extract from the originator" and
--    "statistical arbitrage on public on-chain data".
INSERT INTO strategy_catalog (
    kind, display_name, description, category, is_implemented, is_default,
    risk_level, ethical_constraint, lifecycle_status, requires_flashloan
) VALUES (
    'strat_svs_01',
    'Estabilización de Volatilidad Estocástica',
    'Amortigua volatilidad asimilando impacto macro y redistribuyendo post-convergencia. '
    'Detecta shocks de precio en pools correlacionados y captura el rebalanceo residual '
    'después de la convergencia. Toda la evaluación matricial reside en searcher-rs.',
    'stat', FALSE, FALSE,
    'high', 'none', 'scaffold', FALSE
) ON CONFLICT (kind) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status;

-- 2. Descomposición Triangular Atómica
--    Ethics: PERMITTED per arbx-mev-ethics-gate — atomic triangular arbitrage
--    within a single manifold on confirmed state; no user intent is read.
INSERT INTO strategy_catalog (
    kind, display_name, description, category, is_implemented, is_default,
    risk_level, ethical_constraint, lifecycle_status, requires_flashloan
) VALUES (
    'strat_tri_01',
    'Descomposición Triangular Atómica',
    'Nivelación de ineficiencias de enrutamiento A->B->C en un solo manifold. '
    'Variante APEX del motor triangular Phase 9: bundle atómico de 3 hops en '
    'un único DEX manifold, minimizando gas y eliminando race conditions entre pools. '
    'Spot product S = γ³·∏(R_out/R_in) calculado server-side.',
    'atomic', FALSE, FALSE,
    'medium', 'none', 'scaffold', TRUE
) ON CONFLICT (kind) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status;

-- 3. Alineación de Tasa de Financiamiento (Funding Rate)
--    Ethics: PERMITTED per arbx-mev-ethics-gate (#5 statistical arbitrage on public
--    data + #6 own positions) — delta-neutral long spot + short perp, no victim.
INSERT INTO strategy_catalog (
    kind, display_name, description, category, is_implemented, is_default,
    risk_level, ethical_constraint, lifecycle_status, requires_flashloan
) VALUES (
    'strat_fr_01',
    'Alineación de Tasa de Financiamiento',
    'Captura de delta entre Perps (CEX/DEX) y Spot mediante cobertura atómica. '
    'Detecta divergencias entre funding rates de futuros perpetuos y la tasa implícita '
    'del spot/borrow market. Construye posición delta-neutral: long spot + short perp. '
    'Circuit breaker integrado para caídas de API CEX.',
    'derivatives', FALSE, FALSE,
    'high', 'none', 'scaffold', FALSE
) ON CONFLICT (kind) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    lifecycle_status = EXCLUDED.lifecycle_status;

-- 4. Per-strategy runtime config: NOT seeded here (see 073 §4). Operator sets
--    trading_config.strategy_configs JSONB via the admin PUT path; the column
--    defaults to '{}'. Seeding it in a migration violates arbx-no-hardcode-doctrine.
