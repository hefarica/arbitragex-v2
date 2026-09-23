-- Migration 124: opportunities WS trigger — amount_in_wei as EXACT text
--
-- FRONT-01 (workspace-extreme-audit-2026-09-24): row_to_json(NEW) serializes
-- NUMERIC(78,0) as an UNQUOTED JSON number. The api-server relays that payload
-- to the `opportunities` WS room verbatim (websocket.ts broadcastOpportunity),
-- and the browser JSON.parse turns it into a float64 — silently corrupting any
-- wei value above 2^53 (~9.007e15; every realistic 18-decimal amount).
-- Example: 1234567890123456789 -> 1234567890123456800 (-11 wei).
--
-- The REST route already casts `amount_in_wei::text` (opportunities-live.ts);
-- this aligns the WS NOTIFY payload with that exact-string contract by
-- re-wrapping the field as text via jsonb_set before pg_notify.
-- Same channel + trigger name as migration 025 — this replaces the function.
--
-- RERUN-LOCK-SAFETY: guarded DROP TRIGGER IF EXISTS takes no catalog-enforced
-- exclusive lock when absent (lint-migration-rerun-lock-safety.sh pattern).

BEGIN;

CREATE OR REPLACE FUNCTION notify_new_opportunity() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify(
    'opportunities_channel',
    jsonb_set(
      to_jsonb(NEW),
      '{amount_in_wei}',
      to_jsonb(NEW.amount_in_wei::text)
    )::text
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_notify_opportunity ON opportunities;

CREATE TRIGGER trg_notify_opportunity
AFTER INSERT ON opportunities
FOR EACH ROW
EXECUTE FUNCTION notify_new_opportunity();

COMMIT;
