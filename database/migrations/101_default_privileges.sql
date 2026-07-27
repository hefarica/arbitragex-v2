-- ArbitrageX v2 — Migration 101: Default privileges for future tables
-- Idempotent: safe to re-run (ALTER DEFAULT PRIVILEGES is declarative; GRANT
-- is idempotent in PostgreSQL).
--
-- H5 fix: migrations 003+ GRANT privileges on each table at creation time, but
-- on a FRESH install the api-server roles (arbx_rw / arbx_ro) can hit
-- `permission denied for table <t>` (HTTP 500/503) if any table is created
-- without its explicit GRANT, or before the per-table GRANT runs.
--
-- security-auditor note (FAIL-1): the migration runner (database/
-- run_migrations.sh) connects as the `postgres` superuser, NOT as
-- arbx_migrator. ALTER DEFAULT PRIVILEGES only applies to tables created by
-- the role it names. We therefore cover BOTH roles that may own future tables:
--   * CURRENT_USER / postgres  — the actual runner (and legacy default).
--   * arbx_migrator            — the intended migrator role going forward.
-- Either way, tables created from now on inherit the correct grants.

-- Default privileges for tables created by the role running this migration
-- (postgres under the current runner). Plain "IN SCHEMA" form = FOR CURRENT_USER.
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO arbx_rw;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO arbx_ro;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, UPDATE ON SEQUENCES TO arbx_rw;

-- Default privileges for tables created by arbx_migrator (forward-looking: if
-- the runner is later switched to authenticate as arbx_migrator).
ALTER DEFAULT PRIVILEGES FOR ROLE arbx_migrator IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO arbx_rw;
ALTER DEFAULT PRIVILEGES FOR ROLE arbx_migrator IN SCHEMA public
    GRANT SELECT ON TABLES TO arbx_ro;
ALTER DEFAULT PRIVILEGES FOR ROLE arbx_migrator IN SCHEMA public
    GRANT SELECT, UPDATE ON SEQUENCES TO arbx_rw;

-- Backfill: cover tables/sequences that already exist (harmless if granted).
GRANT USAGE ON SCHEMA public TO arbx_rw, arbx_ro;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO arbx_rw;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO arbx_ro;
GRANT SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA public TO arbx_rw;
