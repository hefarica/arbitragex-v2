-- ArbitrageX v2 — Migration 002: schema_migrations control table
-- Tracks which migrations have been applied. Idempotent.

CREATE TABLE IF NOT EXISTS schema_migrations (
  version TEXT PRIMARY KEY,
  applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  checksum TEXT,
  applied_by TEXT NOT NULL DEFAULT CURRENT_USER
);

COMMENT ON TABLE schema_migrations IS 'Ledger of applied database migrations. Managed by automation/scripts/migrate.sh.';
