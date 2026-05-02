-- ArbitrageX v2 — Migration 001b: Apply role passwords from environment.
-- Idempotent: safe to re-run (ALTER ROLE ... PASSWORD is always overwrite).
--
-- This migration reads passwords from psql variables (:arbx_migrator_pw,
-- :arbx_rw_pw, :arbx_ro_pw) injected by migrate.sh via `psql -v`.
--
-- In development: migrate.sh injects defaults from compose.dev.yml env.
-- In production: migrate.sh reads from the env vars set by Vault/docker secrets.
-- Either way, NO password literal lives in this SQL file.
--
-- IMPORTANT: psql variable interpolation (:'var') does NOT work inside
-- dollar-quoted blocks ($$ ... $$). We use plain SQL ALTER ROLE statements
-- where psql substitutes the variable before sending to the server.
--
-- arbx-no-hardcode-doctrine: COMPLIANT (no productive literals).

-- The :'var' syntax produces a safely single-quoted string literal from a
-- psql variable. psql resolves it before sending to the server.
ALTER ROLE arbx_migrator WITH PASSWORD :'arbx_migrator_pw';
ALTER ROLE arbx_rw       WITH PASSWORD :'arbx_rw_pw';
ALTER ROLE arbx_ro       WITH PASSWORD :'arbx_ro_pw';
