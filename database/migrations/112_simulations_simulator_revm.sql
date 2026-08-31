-- Migration 112: simulations.simulator accepts 'revm' (SIMWIRE-02b)
--
-- The CHECK from migration 004 predates simulator-v2 and only allows
-- ('anvil','tenderly','hardhat','not_implemented'). SIMWIRE-02's B2c path
-- persists SimulatorKind::Revm → every route-aware row would violate the
-- constraint and fail at insert ("insert simulation"), so the flip
-- SIM_BACKEND=revm would be structurally broken at the DB layer.
--
-- SIMWIRE-02c P1-8: wrapped in an explicit transaction. Under psql
-- autocommit the bare DROP-then-ADD left a constraint-less window between
-- the two statements (any concurrent insert could land a simulator value
-- outside both the old and the new CHECK). Atomic swap instead.
--
-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): the runner has NO applied-state
-- ledger — every file re-runs on every deploy, so the DROP-then-ADD cycle
-- took AccessExclusiveLock on the now-hot simulations table (sim-ctl writes
-- continuously post-SIMWIRE-02c) even when the CHECK already allowed 'revm'.
-- The definition-check below skips the swap entirely in the steady state:
-- no table lock on the no-op path (lint-migration-rerun-lock-safety.sh).
-- If the definition ever needs to change again, ship a NEW migration
-- (forward-only), do not edit the CHECK list here.
BEGIN;
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'simulations_simulator_check'
      AND conrelid = 'public.simulations'::regclass
      AND pg_get_constraintdef(oid) LIKE '%revm%'
  ) THEN
    RAISE NOTICE '112: simulations_simulator_check already allows revm — no-op';
  ELSE
    EXECUTE 'ALTER TABLE simulations DROP CONSTRAINT IF EXISTS simulations_simulator_check';
    EXECUTE 'ALTER TABLE simulations ADD CONSTRAINT simulations_simulator_check CHECK (simulator IN (''anvil'',''tenderly'',''hardhat'',''not_implemented'',''revm''))';
  END IF;
END $$;
COMMIT;
