-- ArbitrageX v2 — Migration 033: opportunities fail-honest + cross-chain slots
-- A. Garantizar nullable en 3 columnas de profit/risk (idempotente).
-- B. Agregar slots cross-chain (populated NULL en Sub-Proyecto A).

ALTER TABLE opportunities
  ALTER COLUMN expected_profit_usd DROP NOT NULL,
  ALTER COLUMN roi_pct             DROP NOT NULL,
  ALTER COLUMN risk_score          DROP NOT NULL;

ALTER TABLE opportunities
  ADD COLUMN IF NOT EXISTS chain_id_out      INTEGER       NULL,
  ADD COLUMN IF NOT EXISTS bridge            TEXT          NULL,
  ADD COLUMN IF NOT EXISTS bridge_fee_usd    NUMERIC(20,8) NULL;

DO $$ BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'chk_cross_chain_distinct'
  ) THEN
    ALTER TABLE opportunities
      ADD CONSTRAINT chk_cross_chain_distinct
      CHECK (chain_id_out IS NULL OR chain_id_out <> chain_id);
  END IF;
END $$;
