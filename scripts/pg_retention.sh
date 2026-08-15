#!/usr/bin/env bash
# PGBLOAT-02 — daily retention for the `opportunities` table (keep N days).
#
# Context (2026-08-15): opportunities had 15.8M rows / 6.9GB, 7.57M of them
# older than 30 days (flood-era emissions). This script prunes old rows in
# batches so the DELETE never takes a long lock (searcher inserts ~100 rows/s).
#
# PREREQUISITE: index idx_opp_detected_at ON opportunities (detected_at) must
# exist (created CONCURRENTLY 2026-08-15) — otherwise every batch seq-scans.
#
# Usage:   pg_retention.sh [retention_days]
# Install: VPS host crontab, e.g.
#          17 4 * * * /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
#
# FORMULATION NOTE (measured 2026-08-15): the batched
# `DELETE WHERE id IN (SELECT id ... LIMIT n)` shape forces an index-driven
# RANDOM-IO path (~93 cold reads/s single-threaded, 5+ min per 10k batch —
# the one-time 7.5M purge would have taken ~2.6 days). A single direct
# `DELETE WHERE detected_at < X` lets the planner seq-scan (readahead, ~min
# for the whole table). Daily steady-state (~1 day of rows) is small enough
# that either plan completes well within the maintenance window.
set -euo pipefail

RETENTION_DAYS="${1:-30}"
PG_CONTAINER="${PG_CONTAINER:-$(docker ps --format '{{.Names}}' | grep -m1 -i postgres)}"

# Fail fast if the required index is missing (worst case without it = full
# seq-scan per run; the index also serves dashboard MAX/ORDER BY queries).
if ! docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SELECT 1 FROM pg_class WHERE relname = 'idx_opp_detected_at'" | grep -q 1; then
  echo "FATAL: idx_opp_detected_at missing — refusing to run unindexed" >&2
  exit 1
fi

# synchronous_commit=off: a crash mid-run simply loses uncommitted deletes,
# which tomorrow's run redoes — never worth an fsync-per-commit here.
DELETED=$(docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SET synchronous_commit = off; WITH del AS (DELETE FROM opportunities \
   WHERE detected_at < now() - interval '${RETENTION_DAYS} days' RETURNING 1) \
   SELECT count(*) FROM del" | tail -1)

echo "$(date -u +%FT%TZ) pg_retention: deleted=${DELETED} older_than=${RETENTION_DAYS}d"
