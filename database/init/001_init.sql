-- ArbitrageX v2 — Postgres init
-- Mounted at /docker-entrypoint-initdb.d; runs ONCE on fresh cluster.
-- Creates extensions; actual schema lives in database/migrations/*.sql applied via migrate.sh.

CREATE EXTENSION IF NOT EXISTS pgcrypto;
