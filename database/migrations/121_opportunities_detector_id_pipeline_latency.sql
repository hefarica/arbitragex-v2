-- 121_opportunities_detector_id_pipeline_latency.sql
--
-- WO-CARDS-COMPLETE-01 (operator order 2026-09-17): every opportunity row —
-- accepted AND rejected — carries two emit-time-known fields on the wire:
--   detector_id          TEXT    — which detector produced the row
--                                  (core engine name or cartridge stem).
--   pipeline_latency_ms  BIGINT  — wall-clock ms between detected_at and
--                                  entry into the emit path.
-- Additive only: NULL for all pre-existing rows (R8 fail-honest — never
-- backfilled with fabricated values).
--
-- RERUN-LOCK-SAFETY (same doctrine as migration 102): opportunities has
-- continuous live writers, so the no-op path is catalog-guarded — PostgreSQL
-- takes the table lock BEFORE evaluating IF NOT EXISTS, and an unguarded
-- no-op starves against the runner's lock_timeout=10s.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'detector_id'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN detector_id TEXT';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'pipeline_latency_ms'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN pipeline_latency_ms BIGINT';
  END IF;
END $$;
