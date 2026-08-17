#!/usr/bin/env bash
# PGBLOAT-02 — daily retention for the `opportunities` table (keep N days).
#
# Context (2026-08-15): opportunities had 15.8M rows / 6.9GB, 7.57M of them
# older than 30 days (flood-era emissions). This script prunes old rows so the
# DELETE never takes a long lock (searcher inserts ~100 rows/s).
#
# ═══════════════════════════════════════════════════════════════════════════
# FREEZE-01 / ANTI-FREEZE FASE 1 — LOCKGUARD (2026-08-17)
#
# RCA (docs/incidents/2026-08-17-PIPELINE-FREEZE-PURGE-LOCKS.md, #359): this
# script's DELETE ran with NO lock_timeout and, while it held its lock, a
# deploy re-applied migration 003 whose CREATE INDEX (no CONCURRENTLY) queued
# behind it; the searcher's ~100 inserts/s queued behind THAT; the pool
# saturated and the pipeline went silent for 21h. "Don't run long locks" was
# a documented rule without enforcement — it lasted 26 hours.
#
# This script is now FAIL-OPERATIONAL by design:
#   1. lock_timeout=5s     — if the DELETE can't acquire its lock in 5s the
#                            statement is CANCELLED and the run SKIPS (exit 0,
#                            "purge skipped: lock busy — retry tomorrow").
#                            Retention is re-runnable; a frozen pipeline is not.
#   2. statement_timeout=20min — hard ceiling on the DELETE itself.
#   3. Pre-flight guard    — if pg_stat_activity shows an active migration /
#                            CREATE INDEX / DDL touching opportunities, skip
#                            before even asking for a lock.
# A skip is NOT an error: tomorrow's cron redoes the work. The only fatal
# conditions are structural (missing index, unreachable DB, unexpected error).
#
# PREREQUISITE: index idx_opp_detected_at ON opportunities (detected_at) must
# exist (created CONCURRENTLY 2026-08-15) — otherwise every run seq-scans.
#
# Usage:   pg_retention.sh [retention_days]
# Install: VPS host crontab, e.g.
#          17 4 * * * /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
#
# VERIFIED (2026-08-17, PR evidence): with an ACCESS EXCLUSIVE lock held on
# opportunities by a test session, this script skips in ~5s with exit 0 and
# the pipeline keeps inserting. See PR body for the transcript.
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail

RETENTION_DAYS="${1:-30}"
PG_CONTAINER="${PG_CONTAINER:-$(docker ps --format '{{.Names}}' | grep -m1 -i postgres)}"
LOCK_TIMEOUT="${LOCK_TIMEOUT:-5s}"
STMT_TIMEOUT="${STMT_TIMEOUT:-20min}"

log() { echo "$(date -u +%FT%TZ) pg_retention: $*"; }

# Fail fast if the required index is missing (worst case without it = full
# seq-scan per run; the index also serves dashboard MAX/ORDER BY queries).
if ! docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SELECT 1 FROM pg_class WHERE relname = 'idx_opp_detected_at'" | grep -q 1; then
  echo "FATAL: idx_opp_detected_at missing — refusing to run unindexed" >&2
  exit 1
fi

# ── Guard 3: pre-flight — skip if maintenance/DDL is already running against
# opportunities (a queued CREATE INDEX behind our DELETE is exactly the
# FREEZE-01 chain; if one is already active we must NOT add a lock on top).
ACTIVE_DDL=$(docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SELECT count(*) FROM pg_stat_activity WHERE datname='arbitragex' AND state='active'
   AND (query ~* 'CREATE (UNIQUE )?INDEX|ALTER TABLE|run_migrations|CREATE INDEX CONCURRENTLY')
   AND query !~* 'pg_stat_activity'")
if [ "$ACTIVE_DDL" != "0" ]; then
  log "purge skipped: ${ACTIVE_DDL} active DDL/migration statement(s) detected — retry tomorrow"
  exit 0
fi

# ── Guards 1+2: lock_timeout + statement_timeout bound the DELETE.
# synchronous_commit=off: a crash mid-run simply loses uncommitted deletes,
# which tomorrow's run redoes — never worth an fsync-per-commit here.
# The lock_timeout error surfaces on stderr as
#   "canceling statement due to lock timeout" (SQLSTATE 55P03 class)
# which we treat as SKIP (exit 0); anything else is a real failure (exit 1).
set +e
DELETE_OUT=$(docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SET lock_timeout = '${LOCK_TIMEOUT}'; SET statement_timeout = '${STMT_TIMEOUT}'; SET synchronous_commit = off;
   WITH del AS (DELETE FROM opportunities \
   WHERE detected_at < now() - interval '${RETENTION_DAYS} days' RETURNING 1) \
   SELECT count(*) FROM del" 2>&1)
PSQL_RC=$?
set -e

if [ $PSQL_RC -eq 0 ]; then
  DELETED=$(echo "$DELETE_OUT" | tail -1)
  log "deleted=${DELETED} older_than=${RETENTION_DAYS}d"
  exit 0
fi

if echo "$DELETE_OUT" | grep -qi "lock timeout"; then
  # FAIL-OPERATIONAL: someone else holds the lock (long reader, manual psql,
  # deploy-time DDL). Retention is idempotent — today's batch simply waits
  # for tomorrow. NEVER queue behind a lock; NEVER kill the holder.
  log "purge skipped: lock busy (lock_timeout=${LOCK_TIMEOUT}) — retry tomorrow"
  exit 0
fi

if echo "$DELETE_OUT" | grep -qi "statement timeout"; then
  # The DELETE itself exceeded 20min — abnormal (steady-state is ~1 day of
  # rows). Surface it loudly but do not break the cron chain.
  log "purge aborted: statement_timeout=${STMT_TIMEOUT} exceeded — investigate table size/index"
  exit 0
fi

echo "$(date -u +%FT%TZ) pg_retention FATAL: unexpected psql failure (rc=${PSQL_RC}):" >&2
echo "$DELETE_OUT" >&2
exit 1
