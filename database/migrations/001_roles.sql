-- ArbitrageX v2 — Migration 001: Roles + extensions
-- Idempotent: safe to re-run.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

DO $$ BEGIN
    CREATE ROLE arbx_migrator WITH LOGIN PASSWORD 'arbx_migrator_dev_only' CREATEDB;
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE ROLE arbx_rw WITH LOGIN PASSWORD 'arbx_rw_dev_only';
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE ROLE arbx_ro WITH LOGIN PASSWORD 'arbx_ro_dev_only';
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

GRANT CONNECT ON DATABASE arbitragex TO arbx_migrator, arbx_rw, arbx_ro;
