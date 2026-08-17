#!/usr/bin/env bash
# PGBLOAT-02 — daily retention for the `opportunities` table (keep N days).
#
# Context (2026-08-15): opportunities had 15.8M rows / 6.9GB, 7.57M of them
# older than 30 days (flood-era emissions). This script prunes old rows so the
# DELETE never starves the hot path (searcher inserts ~100 rows/s).
#
# FREEZE-02 (2026-08-17): FREEZE-01 RECURRED via this exact script. The cron
# (`17 4 * * *`) ran the previous single-shot
#   `SET synchronous_commit=off; WITH del AS (DELETE ... < cutoff) ...`
# which held RowExclusiveLock on opportunities for ~41 min (FK cascades to
# risk_events etc. on ~8.6M rows/day). A `CREATE INDEX IF NOT EXISTS` queued
# behind it for ShareLock — and PG's fair lock queue then starved every INSERT
# behind the queued DDL → feed frozen 26 min (04:32–04:58Z), cured by surgical
# termination. Lock discipline per
# docs/incidents/2026-08-17-PIPELINE-FREEZE-PURGE-LOCKS.md §4.1:
#   - lock_timeout per statement   → never queue indefinitely behind a DDL.
#   - statement_timeout per batch  → bound slow batches (FK cascade storms).
#   - batches committed separately → each lock hold is seconds; INSERTs and
#     DDL interleave BETWEEN batches, so no fair-queue starvation is possible.
#   - RUN_BUDGET_S caps the run    → leftover backlog drains on later runs.
#
# PREREQUISITE: index idx_opp_detected_at ON opportunities (detected_at) must
# exist (created CONCURRENTLY 2026-08-15) — the batch subselect is index-driven;
# without it every batch seq-scans.
#
# Usage:   pg_retention.sh [retention_days]
# Env:     BATCH_ROWS (10000) · RUN_BUDGET_S (480) · LOCK_TIMEOUT (5s)
#          STATEMENT_TIMEOUT (60s) · PG_CONTAINER
# Install: VPS host crontab, e.g.
#          17 4 * * * /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
set -euo pipefail

RETENTION_DAYS="${1:-30}"
PG_CONTAINER="${PG_CONTAINER:-$(docker ps --format '{{.Names}}' | grep -m1 -i postgres)}"
BATCH_ROWS="${BATCH_ROWS:-10000}"
RUN_BUDGET_S="${RUN_BUDGET_S:-480}"
LOCK_TIMEOUT="${LOCK_TIMEOUT:-5s}"
STATEMENT_TIMEOUT="${STATEMENT_TIMEOUT:-60s}"

# Fail fast if the required index is missing (worst case without it = full
# seq-scan per run; the index also serves dashboard MAX/ORDER BY queries).
if ! docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SELECT 1 FROM pg_class WHERE relname = 'idx_opp_detected_at'" | grep -q 1; then
  echo "FATAL: idx_opp_detected_at missing — refusing to run unindexed" >&2
  exit 1
fi

# synchronous_commit=off: a crash mid-run simply loses uncommitted deletes,
# which the next run redoes — never worth an fsync-per-commit here.
deadline=$(( $(date +%s) + RUN_BUDGET_S ))
total=0
batches=0
while :; do
  if [ "$(date +%s)" -ge "$deadline" ]; then
    echo "$(date -u +%FT%TZ) pg_retention: run budget ${RUN_BUDGET_S}s reached — remaining backlog drains next run"
    break
  fi
  deleted=$(docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
    "SET lock_timeout = '${LOCK_TIMEOUT}'; SET statement_timeout = '${STATEMENT_TIMEOUT}'; SET synchronous_commit = off; \
     WITH del AS (DELETE FROM opportunities WHERE id IN ( \
       SELECT id FROM opportunities WHERE detected_at < now() - interval '${RETENTION_DAYS} days' \
       LIMIT ${BATCH_ROWS}) RETURNING 1) SELECT count(*) FROM del" | tail -1) || {
      echo "$(date -u +%FT%TZ) pg_retention: batch aborted (lock/statement timeout after ${batches} batches, ${total} rows) — exiting cleanly, backlog drains next run"
      break
    }
  if [ "$deleted" = "0" ]; then
    break
  fi
  total=$(( total + deleted ))
  batches=$(( batches + 1 ))
  # Yield to the hot path between batches: brief pause lets INSERTs (and any
  # queued DDL) acquire/release between our short lock holds.
  sleep 0.3
done

echo "$(date -u +%FT%TZ) pg_retention: deleted=${total} batches=${batches} older_than=${RETENTION_DAYS}d budget=${RUN_BUDGET_S}s"
