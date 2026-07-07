-- ArbitrageX v2 — Migration 025: WebSocket Trigger
-- Broadcasts new opportunities in real-time via PostgreSQL NOTIFY.

CREATE OR REPLACE FUNCTION notify_new_opportunity() RETURNS trigger AS $$
BEGIN
  PERFORM pg_notify('opportunities_channel', row_to_json(NEW)::text);
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_notify_opportunity ON opportunities;

CREATE TRIGGER trg_notify_opportunity
AFTER INSERT ON opportunities
FOR EACH ROW
EXECUTE FUNCTION notify_new_opportunity();
