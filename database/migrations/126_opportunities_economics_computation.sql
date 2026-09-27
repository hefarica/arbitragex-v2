-- 126_opportunities_economics_computation.sql
--
-- ALWAYS-COMPUTE (operator mandate 2026-09-27): every opportunity row —
-- accepted AND rejected — carries the ONE complete economics computation
-- object on the `economics` JSONB column:
--   computation_status: "computed" | "partial" | "error"
--   amounts (wei + usd), gross_profit_usd, per-component costs, total_cost_usd,
--   net_profit_usd, roi_pct, target_net_usd / target_delta_usd / meets_target,
--   quote_block / simulation_block, per-leg ComputedLeg[], not_computed_reasons
-- (see shared-rs/src/contracts.rs::EconomicsComputation for the full doc).
--
-- "COMPUTED ≠ PROFITABLE. FAIL debe significar 'se hizo el cálculo y no cumple
-- el criterio', no 'no tengo números'. Si el resultado da -$50, la card debe
-- decir -$50, no NO COMPUTADO."
--
-- Additive only: NULL for all pre-existing rows (R8 fail-honest — never
-- backfilled with fabricated values). Revert posture: the searcher stops
-- writing the object when ARBX_ALWAYS_COMPUTE_ECONOMICS=false; the column
-- itself is harmless (nullable, no index, no constraint).
--
-- RERUN-LOCK-SAFETY (same doctrine as migrations 102/121): opportunities has
-- continuous live writers, so the no-op path is catalog-guarded — PostgreSQL
-- takes the table lock BEFORE evaluating IF NOT EXISTS, and an unguarded
-- no-op starves against the runner's lock_timeout=10s.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'economics'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN economics JSONB';
  END IF;
END $$;
