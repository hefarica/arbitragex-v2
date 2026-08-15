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
# Usage:   pg_retention.sh [retention_days] [batch_size]
# Install: VPS host crontab, e.g.
#          17 4 * * * /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
set -euo pipefail

RETENTION_DAYS="${1:-30}"
BATCH="${2:-10000}"
PG_CONTAINER="${PG_CONTAINER:-$(docker ps --format '{{.Names}}' | grep -m1 -i postgres)}"

# Fail fast if the required index is missing (retention without it = seq-scan storm).
if ! docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SELECT 1 FROM pg_class WHERE relname = 'idx_opp_detected_at'" | grep -q 1; then
  echo "FATAL: idx_opp_detected_at missing — refusing to seq-scan 6.9GB per batch" >&2
  exit 1
fi

TOTAL=0
while :; do
  DELETED=$(docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
    "WITH del AS (DELETE FROM opportunities WHERE id IN \
     (SELECT id FROM opportunities WHERE detected_at < now() - interval '${RETENTION_DAYS} days' \
      LIMIT ${BATCH}) RETURNING 1) SELECT count(*) FROM del")
  TOTAL=$((TOTAL + DELETED))
  [ "$DELETED" -lt "$BATCH" ] && break
  sleep 1
done

echo "$(date -u +%FT%TZ) pg_retention: deleted=${TOTAL} older_than=${RETENTION_DAYS}d batch=${BATCH}"
