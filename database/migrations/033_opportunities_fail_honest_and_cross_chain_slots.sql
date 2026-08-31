-- ArbitrageX v2 — Migration 033: opportunities fail-honest + cross-chain slots
-- A. Garantizar nullable en 3 columnas de profit/risk (idempotente).
-- B. Agregar slots cross-chain (populated NULL en Sub-Proyecto A).

-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): ALTER TABLE takes
-- AccessExclusiveLock on the hot opportunities table before any IF NOT
-- EXISTS / DROP NOT NULL check. Catalog-guarded no-op path takes no table
-- lock (lint-migration-rerun-lock-safety.sh).
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'expected_profit_usd' AND is_nullable = 'NO'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ALTER COLUMN expected_profit_usd DROP NOT NULL';
  END IF;
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'roi_pct' AND is_nullable = 'NO'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ALTER COLUMN roi_pct DROP NOT NULL';
  END IF;
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'risk_score' AND is_nullable = 'NO'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ALTER COLUMN risk_score DROP NOT NULL';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'chain_id_out'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN chain_id_out INTEGER NULL';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'bridge'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN bridge TEXT NULL';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'bridge_fee_usd'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN bridge_fee_usd NUMERIC(20,8) NULL';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'chk_cross_chain_distinct'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD CONSTRAINT chk_cross_chain_distinct CHECK (chain_id_out IS NULL OR chain_id_out <> chain_id)';
  END IF;
END $$;
