-- ArbitrageX v2 — Migration 001: Roles + extensions
-- Idempotent: safe to re-run.
--
-- NOTE: Roles are created WITHOUT passwords here. Migration 001b applies
-- passwords from environment variables. This ensures no literal secret
-- lives in the Git repository (arbx-no-hardcode-doctrine).

CREATE EXTENSION IF NOT EXISTS pgcrypto;

DO $$ BEGIN
    CREATE ROLE arbx_migrator WITH LOGIN CREATEDB;
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE ROLE arbx_rw WITH LOGIN;
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE ROLE arbx_ro WITH LOGIN;
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

GRANT CONNECT ON DATABASE arbitragex TO arbx_migrator, arbx_rw, arbx_ro;
