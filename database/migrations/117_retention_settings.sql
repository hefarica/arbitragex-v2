-- 117_retention_settings.sql — DAPP-ARCHIVE-UI-01 (2026-09-04)
--
-- Operator-facing control table for the retention/archive policy
-- (ARBX-RETENTION-01, docs/RETENTION_POLICY.md). Today a single key:
--
--   archive_auto {"enabled": false}
--     true  → the nightly cron (scripts/pg_retention.sh) archives each range
--             (COPY → zstd) BEFORE purging it. Fail-honest: archive failed →
--             that table is NOT purged that night.
--     false → purge proceeds without archiving (default, current behavior).
--
-- Written by api-server's /api/admin/archive/auto (admin-gated, audit
-- logged); read by the cron script AND by /api/admin/archive/status. New
-- table (not in the lint HOT_TABLES set); idempotent for re-runs.

SET statement_timeout = '2min';

CREATE TABLE IF NOT EXISTS retention_settings (
    key        text PRIMARY KEY,
    value      jsonb NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO retention_settings (key, value)
VALUES ('archive_auto', '{"enabled": false}'::jsonb)
ON CONFLICT (key) DO NOTHING;

COMMENT ON TABLE retention_settings IS
  'Operator toggles for the retention/archive policy (DAPP-ARCHIVE-UI-01); SSOT for archive_auto, read by scripts/pg_retention.sh nightly cron';
