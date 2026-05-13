-- ArbitrageX v2 — Migration 060: B0.3 risk_events.chain_id
--
-- Footgun F3 (audit 2026-05-13): risk_events has no chain_id column, so
-- circuit breaker trips, kill-switch events, blacklist hits, and
-- degradations cannot be traced to the chain that emitted them. Multi-
-- chain operation makes this a critical traceability gap.
--
-- Fix: add chain_id BIGINT (nullable for retro-compat with legacy events),
-- plus an index on (chain_id, created_at DESC) for the dashboard query
-- pattern "show me the last N events for chain X".
--
-- Backward compatibility:
--   - Existing rows have chain_id = NULL → reported as "legacy_unknown_chain"
--     in the API. NEW inserts MUST include chain_id (enforced at API layer,
--     not at DB CHECK constraint to avoid blocking legacy backfills).
--
-- Rollback: see bottom of file.

BEGIN;

-- Add the column (nullable for legacy rows).
ALTER TABLE risk_events
  ADD COLUMN IF NOT EXISTS chain_id BIGINT;

-- Index for the dashboard query pattern.
CREATE INDEX IF NOT EXISTS idx_risk_events_chain_id_created_at
  ON risk_events (chain_id, created_at DESC);

-- Documentation comment on the column for psql \d output.
COMMENT ON COLUMN risk_events.chain_id IS
  'B0.3 (migration 060, 2026-05-13): chain that emitted the event. '
  'NULL on legacy rows (pre-migration); new inserts MUST populate. '
  'Reported as legacy_unknown_chain when NULL.';

COMMIT;

-- ─── ROLLBACK (run manually if needed) ─────────────────────────────────────
-- BEGIN;
--   DROP INDEX IF EXISTS idx_risk_events_chain_id_created_at;
--   ALTER TABLE risk_events DROP COLUMN IF EXISTS chain_id;
-- COMMIT;
