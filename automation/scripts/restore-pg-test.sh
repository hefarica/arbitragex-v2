#!/usr/bin/env bash
# restore-pg-test.sh — smoke-test the latest Postgres backup by restoring it
# into a throw-away database inside the live postgres container.
#
# This script is intentionally lightweight: it works with unencrypted pg_dump
# custom-format files (the decrypted output of restore-pg.sh, or a plain
# `pg_dump --format=custom` produced during CI). It is NOT a substitute for
# restore-pg.sh in a real incident — that script handles the full
# decrypt+stage+promote workflow.
#
# INF-6: Phase 3 INF batch — 2026-05-07
#
# Usage (on VPS, as the deploying user):
#   BACKUP_FILE=/var/backups/arbx/latest.dump \
#   COMPOSE_FILE=docker/compose.prod.yml \
#     ./automation/scripts/restore-pg-test.sh
#
# Exit codes:
#   0  — restore verified, temp DB dropped
#   1  — usage / pre-condition failure
#   2  — restore failed
#   3  — table count below threshold (suspicious backup)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

log()  { printf '[%s] restore-pg-test: %s\n' "$(date -u +%H:%M:%S)" "$*"; }
die()  { log "FATAL: $*"; exit 1; }
fail() { log "ERROR: $*"; exit "${2:-2}"; }

BACKUP_FILE="${BACKUP_FILE:-}"
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.prod.yml}"
TEST_DB="${TEST_DB:-arbitragex_restore_test_$$}"
MIN_TABLES="${MIN_TABLES:-10}"  # minimum number of tables we expect after restore

# ── 1. Pre-conditions ────────────────────────────────────────────────────────

if [[ -z "$BACKUP_FILE" ]]; then
  # Try to auto-discover the latest unencrypted dump in the backup dir.
  BACKUP_DIR="${BACKUP_DIR:-/var/backups/arbx}"
  BACKUP_FILE="$(ls -t "$BACKUP_DIR"/pg-*.sql.custom 2>/dev/null | head -1 || true)"
  if [[ -z "$BACKUP_FILE" ]]; then
    die "BACKUP_FILE not set and no pg-*.sql.custom found in ${BACKUP_DIR}. " \
        "Either set BACKUP_FILE or provide a decrypted dump."
  fi
  log "auto-discovered backup: $BACKUP_FILE"
fi

[[ -f "$BACKUP_FILE" ]] || die "backup file not found: $BACKUP_FILE"

DUMP_BYTES=$(stat -c %s "$BACKUP_FILE" 2>/dev/null || stat -f %z "$BACKUP_FILE")
log "backup size: ${DUMP_BYTES} bytes"
[[ "${DUMP_BYTES:-0}" -gt 1024 ]] || die "backup file suspiciously small (${DUMP_BYTES} bytes)"

# Identify the postgres container name from compose.
CONTAINER="$(docker compose --env-file .env -f "$COMPOSE_FILE" ps -q postgres 2>/dev/null || true)"
if [[ -z "$CONTAINER" ]]; then
  # fallback: use well-known name pattern
  CONTAINER="$(docker ps --filter name=postgres --format '{{.Names}}' | head -1 || true)"
fi
[[ -n "$CONTAINER" ]] || die "postgres container not found. Is the stack running?"
log "postgres container: $CONTAINER"

# ── 2. Create temp DB ────────────────────────────────────────────────────────

cleanup() {
  log "cleanup: dropping $TEST_DB (if it exists)"
  docker exec "$CONTAINER" \
    psql -U postgres -c "DROP DATABASE IF EXISTS $TEST_DB;" 2>/dev/null || true
}
trap cleanup EXIT

log "creating test DB: $TEST_DB"
docker exec "$CONTAINER" \
  psql -U postgres -c "CREATE DATABASE $TEST_DB;" \
  || fail "failed to create temp DB $TEST_DB"

# ── 3. Restore into temp DB ──────────────────────────────────────────────────

log "restoring dump into $TEST_DB"
docker exec -i "$CONTAINER" \
  pg_restore --no-owner --no-acl --exit-on-error \
             --username postgres --dbname "$TEST_DB" \
  < "$BACKUP_FILE" \
  || fail "pg_restore failed for $BACKUP_FILE"

# ── 4. Verify: table count ────────────────────────────────────────────────────

TABLE_COUNT="$(docker exec "$CONTAINER" \
  psql -U postgres -d "$TEST_DB" -Atqc \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public';")"

log "table count in restored DB: ${TABLE_COUNT}"

if [[ "${TABLE_COUNT:-0}" -lt "$MIN_TABLES" ]]; then
  fail "table count ${TABLE_COUNT} is below threshold ${MIN_TABLES} — backup may be corrupt" 3
fi

# ── 5. Spot-check key table row counts ───────────────────────────────────────

log "spot-checking row counts:"
docker exec "$CONTAINER" \
  psql -U postgres -d "$TEST_DB" -c "
  SELECT 'opportunities' AS tbl, count(*) FROM opportunities
  UNION ALL SELECT 'pools',             count(*) FROM pools
  UNION ALL SELECT 'executions',        count(*) FROM executions
  UNION ALL SELECT 'audit_log',         count(*) FROM audit_log
  ORDER BY 1;
" 2>/dev/null || log "WARNING: one or more spot-check tables are absent (may be OK if schema evolved)"

# ── 6. Done (trap will drop the test DB) ─────────────────────────────────────

log "PASS — backup $BACKUP_FILE restored cleanly into $TEST_DB (${TABLE_COUNT} tables)"
