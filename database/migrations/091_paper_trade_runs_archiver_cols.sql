-- ArbitrageX v2 — Migration 091: paper_trade_runs archiver columns
--
-- The Shadow Archiver (api-server) persists detected-opportunity sim telemetry
-- into paper_trade_runs. The 051 schema lacks two fields the archiver records:
--   reason      — the opportunity's rejection_reason (NULL when accepted).
--   route_hash  — a deterministic fingerprint of the route (from real payload
--                 fields), for grouping paper runs by route over time.
-- Additive + nullable + idempotent: safe on existing prod DBs (no backfill,
-- no NOT NULL). Runs after 051 (lexical 091 > 051).

BEGIN;

ALTER TABLE paper_trade_runs ADD COLUMN IF NOT EXISTS reason     TEXT;
ALTER TABLE paper_trade_runs ADD COLUMN IF NOT EXISTS route_hash TEXT;

-- Access pattern: aggregate paper runs by route fingerprint.
CREATE INDEX IF NOT EXISTS idx_paper_trade_runs_route_hash
    ON paper_trade_runs(route_hash)
    WHERE route_hash IS NOT NULL;

DO $$
BEGIN
    RAISE NOTICE 'Migration 091: paper_trade_runs.reason + route_hash columns ready for the Shadow Archiver.';
END $$;

COMMIT;
