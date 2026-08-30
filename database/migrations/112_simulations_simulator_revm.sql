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
-- Idempotent: DROP IF EXISTS + re-ADD (re-running replaces the constraint
-- with the identical definition).
BEGIN;
ALTER TABLE simulations DROP CONSTRAINT IF EXISTS simulations_simulator_check;
ALTER TABLE simulations
  ADD CONSTRAINT simulations_simulator_check
  CHECK (simulator IN ('anvil','tenderly','hardhat','not_implemented','revm'));
COMMIT;
