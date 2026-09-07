-- WO-04 (2026-09-06): parameterize the api-server LP-fee proxy default
-- (was hardcoded 0.003 at backend/api-server/src/simulation/computeSimulatedNet.ts:139).
-- DEFAULT preserves deployed behavior exactly (rows existing and new = 0.0030).
-- Doctrine (ROUTES_CROWN_JEWEL rule 4): on-chain per-leg fee tiers remain the
-- source of truth; this column is the operator-governed proxy for routes whose
-- legs the api-server hot path cannot resolve. Bounds mirror the zod schema.
ALTER TABLE trading_config
    ADD COLUMN IF NOT EXISTS lp_fee_default_pct NUMERIC(6,4) NOT NULL DEFAULT 0.0030
    CHECK (lp_fee_default_pct >= 0 AND lp_fee_default_pct <= 0.5);
