-- ArbitrageX v2 — Migration 107: WebSocket UPDATE trigger
-- Binance-style streaming (operator directive 2026-08-18): an emitted card must
-- update IN REAL TIME when its data changes — only the changed row pushes.
--
-- Migration 025 already broadcasts INSERTs (new detections) via
-- pg_notify('opportunities_channel', ...). This adds the UPDATE leg: when the
-- pipeline computes economics after acceptance, a simulation result lands, the
-- status transitions, or a paper execution writes back its values (execution-
-- time values PREVAIL and overwrite any earlier ones), the row UPDATE now
-- notifies and the WebSocket pushes the fresh row to every subscribed card.
--
-- Lightweight by construction:
--   - WHEN (OLD.* IS DISTINCT FROM NEW.*) — PostgreSQL row-level change
--     detection: a no-op UPDATE (same values) never notifies.
--   - Per-row, not per-statement — one notify per actually-changed row.
--   - The frontend store upserts by id in place (card position preserved;
--     React.memo + business-equality comparator re-renders only that card).
--
-- Note (payload size): pg_notify caps the payload at 8000 bytes. row_to_json(NEW)
-- for an opportunity with populated route_metadata stays well under that today
-- (the INSERT trigger in migration 025 has run this shape in prod without
-- incident). If a future column ever pushes rows past the cap, trim the payload
-- here — do NOT widen it silently.
--
-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): DROP/CREATE TRIGGER take
-- ShareRowExclusive on the hot opportunities table on every re-run even when
-- the trigger already exists. Catalog-guarded no-op path takes no table lock
-- (lint-migration-rerun-lock-safety.sh). Trigger definition changes ship as a
-- NEW migration (forward-only doctrine), so the name-guard is sufficient.

CREATE OR REPLACE FUNCTION notify_updated_opportunity() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify('opportunities_channel', row_to_json(NEW)::text);
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_trigger
    WHERE tgname = 'trg_notify_opportunity_update'
      AND tgrelid = 'public.opportunities'::regclass
  ) THEN
    EXECUTE 'CREATE TRIGGER trg_notify_opportunity_update AFTER UPDATE ON opportunities FOR EACH ROW WHEN (OLD.* IS DISTINCT FROM NEW.*) EXECUTE FUNCTION notify_updated_opportunity()';
  END IF;
END $$;
