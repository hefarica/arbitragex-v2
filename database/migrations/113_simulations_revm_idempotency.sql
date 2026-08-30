-- Migration 113: partial unique index for revm redelivery idempotency (SIMWIRE-02c P1-5)
--
-- XAUTOCLAIM redelivers a PEL entry whose final XACK failed AFTER
-- persist+XADD already succeeded; without a uniqueness guard the consumer
-- would re-persist the verdict and republish the opportunity (double
-- paper-trade downstream). With this index,
-- persistence::insert_simulation's
--   ON CONFLICT (opportunity_id) WHERE simulator = 'revm' DO NOTHING
-- turns the redelivery into a no-op (rows_affected()==0 → caller skips
-- the downstream XADD — exactly-once publish).
--
-- PARTIAL (WHERE simulator = 'revm') on purpose: legacy anvil history
-- legitimately holds multiple attempts per opportunity (per-attempt
-- diagnostics); only the revm flip demands one-verdict-per-opportunity.
--
-- CONCURRENTLY per doctrine (lint-migration-index-locks, FREEZE-01
-- lesson): simulations is a populated live table (~640k rows) — a plain
-- build would block INSERTs for the whole build. The runner applies files
-- statement-by-statement (no single-transaction wrapper), so CONCURRENTLY
-- is legal here; the partial predicate matches ZERO pre-flip rows (no
-- revm writer is deployed — the CHECK only landed in 112), so the build
-- is near-instant and cannot fail on duplicates. If it ever fails midway
-- it leaves an INVALID index: recovery is
--   DROP INDEX CONCURRENTLY IF EXISTS simulations_revm_idempotency_uq;
-- followed by re-running this file (documented, not automated — same
-- note as 105/111).
CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS simulations_revm_idempotency_uq
  ON simulations (opportunity_id)
  WHERE simulator = 'revm';
