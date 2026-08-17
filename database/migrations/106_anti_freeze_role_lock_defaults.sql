-- 106_anti_freeze_role_lock_defaults.sql
--
-- FREEZE-01 / ANTI-FREEZE FASE 4 (2026-08-17): server-side defaults that make
-- the freeze PHYSICALLY IMPOSSIBLE by default — "regla en un doc" -> ley física.
--
-- RCA (#359): every lock-wait in the freeze chain was a session with NO
-- lock_timeout (retention DELETE, migration CREATE INDEX, manual psql).
-- Scripts got explicit guards in FASE 1 (#368) and FASE 2 (#372); this
-- migration vaccinates EVERY FUTURE SESSION at the role level, including
-- forgotten manual psql.
--
-- ROLE REALITY (L4, 2026-08-17 prod): all 18 live connections use role
-- `postgres` (superuser); arbx_rw/arbx_ro exist but are UNUSED. Therefore
-- the load-bearing default is on `postgres`; the arbx_* roles get the same
-- protection for the day services migrate to them.
--
-- DESIGN DECISION (documented deviation from the FASE-4 draft): we set ONLY
-- lock_timeout globally — NOT statement_timeout. A global statement_timeout
-- on the superuser would also kill legitimate long-running analytical queries
-- (api-server recon/history aggregates over 15M+ rows). The freeze mechanism
-- was always a LOCK WAIT, never a slow statement; statements are bounded
-- explicitly where it matters (retention: 20min; migrations: 10min via
-- PGOPTIONS). Lock waits are bounded HERE, everywhere, forever.
--
-- Idempotent: ALTER ROLE ... SET is naturally idempotent.
-- Override for legit maintenance: `SET lock_timeout = '...'` in-session
-- (exactly what pg_retention.sh and run_migrations.sh now do).

ALTER ROLE postgres   SET lock_timeout = '5s';
ALTER ROLE arbx_rw    SET lock_timeout = '5s';
ALTER ROLE arbx_ro    SET lock_timeout = '10s';
ALTER ROLE arbx_migrator SET lock_timeout = '10s';

-- Verification (post-deploy L4, run manually or via monitoring):
--   SELECT r.rolname, d.setconfig FROM pg_roles r
--   JOIN pg_db_role_setting d ON d.setrole = r.oid AND d.setdatabase = 0
--   WHERE r.rolname IN ('postgres','arbx_rw','arbx_ro','arbx_migrator');
