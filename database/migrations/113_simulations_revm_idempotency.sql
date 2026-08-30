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
-- Fails loudly if pre-existing revm duplicates exist (there are none
-- pre-flip: the revm CHECK only landed in 112 and no revm writer is
-- deployed). Single DDL statement = atomic by itself — no BEGIN/COMMIT
-- wrapper needed.
CREATE UNIQUE INDEX IF NOT EXISTS simulations_revm_idempotency_uq
  ON simulations (opportunity_id)
  WHERE simulator = 'revm';
