-- ArbitrageX v2 — Migration 025: WebSocket Trigger
-- Broadcasts new opportunities in real-time via PostgreSQL NOTIFY.

CREATE OR REPLACE FUNCTION notify_new_opportunity() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify('opportunities_channel', row_to_json(NEW)::text);
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): DROP/CREATE TRIGGER take
-- ShareRowExclusive on the hot opportunities table on every re-run even when
-- the trigger already exists. Catalog-guarded no-op path takes no table lock
-- (lint-migration-rerun-lock-safety.sh). Trigger definition changes ship as a
-- NEW migration (forward-only doctrine), so the name-guard is sufficient.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_trigger
    WHERE tgname = 'trg_notify_opportunity'
      AND tgrelid = 'public.opportunities'::regclass
  ) THEN
    EXECUTE 'CREATE TRIGGER trg_notify_opportunity AFTER INSERT ON opportunities FOR EACH ROW EXECUTE FUNCTION notify_new_opportunity()';
  END IF;
END $$;
