#!/bin/bash
# ArbitrageX v2 — Run all migrations (auto-discovery, idempotent)
#
# Discovers every database/migrations/*.sql in numeric order and applies it.
# Each file MUST be idempotent (ADD COLUMN IF NOT EXISTS, CREATE INDEX IF NOT
# EXISTS, DO $$ BEGIN ... EXCEPTION WHEN duplicate_object). This makes the
# script safe to re-run on every deploy without tracking applied state.
#
# Replaces the legacy hand-enumerated list (which stopped at 024 and silently
# dropped 025..102). The init container (database/init/001_init.sql) only runs
# on first boot; this script is the canonical path for post-boot schema sync.
set -euo pipefail

# ANTI-FREEZE FASE 2 - MIGRATION LOCKGUARD (2026-08-17, FREEZE-01 RCA #359):
# a deploy re-applied migration 003 whose CREATE INDEX (no CONCURRENTLY)
# queued behind the retention DELETE and froze the pipeline 21h. EVERY
# statement here now runs with lock_timeout=10s + statement_timeout=10min
# via PGOPTIONS: a migration that cannot take its lock in 10s FAILS FAST
# (visible deploy error) instead of queueing against live inserts. A
# migration that legitimately needs longer overrides per-session with
# `SET statement_timeout = '...'` at the top of its own file (see 105_).
MIG_LOCK_OPTS="-c lock_timeout=10s -c statement_timeout=10min"

PGUSER=postgres
PGDB=arbitragex
CONTAINER="${PG_CONTAINER:-arbitragex-v2-postgres-1}"
MIG_DIR="${MIGRATIONS_DIR:-/opt/arbitragex-v2/database/migrations}"

run_sql() {
  docker exec -e PGOPTIONS="$MIG_LOCK_OPTS" "$CONTAINER" psql -U "$PGUSER" -d "$PGDB" -v ON_ERROR_STOP=1 -c "$1"
}

run_file() {
  # -v ON_ERROR_STOP=1 makes any SQL error abort the script (fail-fast).
  # Inject the psql variables that 001b_role_passwords.sql expects (:'arbx_*_pw')
  # so password-setting migrations resolve without a literal in the SQL file
  # (arbx-no-hardcode-doctrine). Migrations that don't use :'var' ignore them.
  # In production these should come from Vault/docker secrets; here we use the
  # same dev defaults the script sets via ALTER ROLE above.
  local MIG_PW="${ARBX_MIGRATOR_PW:-arbx_migrator_dev_only}"
  local RW_PW="${ARBX_RW_PW:-arbx_rw_dev_only}"
  local RO_PW="${ARBX_RO_PW:-arbx_ro_dev_only}"
  docker exec -i -e PGOPTIONS="$MIG_LOCK_OPTS" "$CONTAINER" psql -U "$PGUSER" -d "$PGDB" \
    -v ON_ERROR_STOP=1 -v VERBOSITY=verbose \
    -v arbx_migrator_pw="$MIG_PW" \
    -v arbx_rw_pw="$RW_PW" \
    -v arbx_ro_pw="$RO_PW" \
    < "$MIG_DIR/$1"
}

echo "=== Creating roles ==="
run_sql "CREATE EXTENSION IF NOT EXISTS pgcrypto;"
run_sql "DO \$\$ BEGIN CREATE ROLE arbx_migrator WITH LOGIN CREATEDB; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$;"
run_sql "DO \$\$ BEGIN CREATE ROLE arbx_rw WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$;"
run_sql "DO \$\$ BEGIN CREATE ROLE arbx_ro WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$;"
run_sql "GRANT CONNECT ON DATABASE arbitragex TO arbx_migrator, arbx_rw, arbx_ro;"
run_sql "ALTER ROLE arbx_migrator WITH PASSWORD 'arbx_migrator_dev_only';"
run_sql "ALTER ROLE arbx_rw WITH PASSWORD 'arbx_rw_dev_only';"
run_sql "ALTER ROLE arbx_ro WITH PASSWORD 'arbx_ro_dev_only';"

echo "=== Running schema migrations (auto-discovered, idempotent) ==="
# Glob every .sql in numeric order. printf+sort -V gives 002,003,...,099,100,101,102.
# 001_init.sql lives in database/init/ (first-boot only) and is intentionally
# excluded — it has no schema, just pgcrypto.
mapfile -t FILES < <(find "$MIG_DIR" -maxdepth 1 -type f -name '*.sql' -printf '%f\n' | sort -V)

if [ "${#FILES[@]}" -eq 0 ]; then
  echo "WARNING: no migration files found in $MIG_DIR — schema will be stale."
  exit 1
fi

APPLIED=0
SKIPPED=0
# Unique restricted logs prevent concurrent runs from overwriting each other's
# diagnostics. Never retry arbitrary non-transactional or data migrations.
LOG_DIR=$(mktemp -d)
trap 'rm -rf "$LOG_DIR"' EXIT
for f in "${FILES[@]}"; do
  # Each migration is wrapped so a failure aborts the whole deploy (fail-fast).
  # Idempotency is the migration author's responsibility (IF NOT EXISTS).
  attempts=1
  if [ "$f" = "103_math_evidence_scoring.sql" ]; then
    attempts=3 # Entire file is transactional; lock failures roll back all DDL.
  fi
  succeeded=0
  for ((attempt=1; attempt<=attempts; attempt++)); do
    if run_file "$f" >"$LOG_DIR/$f.log" 2>&1; then
      succeeded=1
      break
    fi
    if ! grep -Eq 'ERROR:  (55P03|40P01):' "$LOG_DIR/$f.log" || [ "$attempt" -eq "$attempts" ]; then
      break
    fi
    echo "  -> RETRY $f (transient lock, attempt $attempt/$attempts)"
    sleep "$((attempt * 2))"
  done
  if [ "$succeeded" -eq 1 ]; then
    echo "  -> OK   $f"
    APPLIED=$((APPLIED + 1))
  else
    # Re-run of an idempotent migration should never fail. A real failure here
    # means a non-idempotent migration OR a genuine schema error — either way
    # the deploy MUST abort to avoid code/DB desync.
    echo "  -> FAIL $f"
    cat "$LOG_DIR/$f.log"
    # Metadata only: no SQL text, credentials or client addresses in CI logs.
    run_sql "SELECT pid, state, wait_event_type, wait_event,
                    age(clock_timestamp(), xact_start) AS transaction_age,
                    pg_blocking_pids(pid) AS blocked_by
               FROM pg_stat_activity
              WHERE datname = current_database() AND pid <> pg_backend_pid()
              ORDER BY xact_start NULLS LAST LIMIT 20;" || true
    exit 1
  fi
done
echo "  ($APPLIED migration files processed)"

echo "=== Granting permissions ==="
run_sql "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO arbx_rw;"
run_sql "GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO arbx_rw;"
run_sql "GRANT SELECT ON ALL TABLES IN SCHEMA public TO arbx_ro;"
run_sql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO arbx_rw;"
run_sql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO arbx_rw;"
run_sql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO arbx_ro;"

echo "=== DONE: schema synchronized with HEAD ==="
