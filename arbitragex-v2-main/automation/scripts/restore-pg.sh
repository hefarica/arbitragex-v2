#!/usr/bin/env bash
# restore-pg.sh — decrypt + stage-restore a Postgres backup produced by
# backup-pg.sh. Never restores directly over the live DB.
#
# Usage:
#   AGE_IDENTITY=/root/arbx.age-identity \
#   DATABASE_MIGRATOR_URL=postgres://arbx_migrator:...@postgres:5432/arbitragex \
#   STAGING_DB=arbitragex_restore_staging \
#     ./restore-pg.sh /var/backups/arbx/pg-YYYYMMDD-HHMM.sql.custom.gz.age
#
# The script:
#   1. Verifies the backup file + its sidecar sha256.
#   2. Decrypts via age into a tmp dir.
#   3. Ungzips.
#   4. CREATE DATABASE $STAGING_DB (must not pre-exist unless --force).
#   5. pg_restore into $STAGING_DB.
#   6. Prints row counts for a few sanity-check tables.
#   7. Does NOT swap live ↔ staging. The operator does that manually per
#      docs/runbooks/db-restore.md after verifying the staged data.

set -euo pipefail

BACKUP_FILE="${1:-}"
if [ -z "$BACKUP_FILE" ]; then
  echo "usage: $0 /path/to/pg-<stamp>.sql.custom.gz.age" >&2
  exit 2
fi

AGE_IDENTITY="${AGE_IDENTITY:?AGE_IDENTITY required (path to operator age private key)}"
DATABASE_MIGRATOR_URL="${DATABASE_MIGRATOR_URL:?DATABASE_MIGRATOR_URL required}"
STAGING_DB="${STAGING_DB:-arbitragex_restore_staging}"
FORCE="${FORCE:-0}"

for tool in age gzip pg_restore psql sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || { echo "missing tool: $tool" >&2; exit 2; }
done

if [ ! -f "$BACKUP_FILE" ]; then
  echo "restore-pg: file not found: $BACKUP_FILE" >&2
  exit 3
fi

# Integrity check if sidecar exists.
if [ -f "$BACKUP_FILE.sha256" ]; then
  echo "[restore-pg] verifying sha256"
  (cd "$(dirname "$BACKUP_FILE")" && sha256sum -c "$(basename "$BACKUP_FILE").sha256") \
    || { echo "restore-pg: sha256 mismatch — refusing" >&2; exit 4; }
else
  echo "[restore-pg] WARNING: no .sha256 sidecar next to the backup (continuing)"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

GZ="$TMP_DIR/restore.sql.custom.gz"
DUMP="$TMP_DIR/restore.sql.custom"

echo "[restore-pg] decrypting with age"
age -d -i "$AGE_IDENTITY" -o "$GZ" "$BACKUP_FILE"

echo "[restore-pg] gzip integrity check"
gzip -t "$GZ"

echo "[restore-pg] ungzipping"
gunzip -c "$GZ" > "$DUMP"

DUMP_BYTES=$(stat -c %s "$DUMP" 2>/dev/null || stat -f %z "$DUMP")
echo "[restore-pg] dump size $DUMP_BYTES bytes"

# Check whether STAGING_DB already exists.
EXISTS=$(psql "$DATABASE_MIGRATOR_URL" -Atqc \
  "select 1 from pg_database where datname = '$STAGING_DB'")
if [ "$EXISTS" = "1" ]; then
  if [ "$FORCE" != "1" ]; then
    echo "restore-pg: staging DB '$STAGING_DB' already exists. Re-run with FORCE=1 to DROP+recreate." >&2
    exit 5
  fi
  echo "[restore-pg] FORCE=1 — dropping existing staging DB"
  psql "$DATABASE_MIGRATOR_URL" -c "DROP DATABASE $STAGING_DB;"
fi

echo "[restore-pg] creating staging DB $STAGING_DB"
psql "$DATABASE_MIGRATOR_URL" -c "CREATE DATABASE $STAGING_DB;"

# Build a staging URL by swapping the dbname in $DATABASE_MIGRATOR_URL.
STAGING_URL="$(printf '%s' "$DATABASE_MIGRATOR_URL" | sed -E 's|/[^/?]+(\?.*)?$|/'"$STAGING_DB"'\1|')"

echo "[restore-pg] pg_restore into $STAGING_DB"
pg_restore --no-owner --no-acl --exit-on-error \
           --dbname "$STAGING_URL" \
           "$DUMP"

echo "[restore-pg] sanity — row counts in staging:"
psql "$STAGING_URL" -c "
  SELECT 'opportunities' AS tbl, count(*) FROM opportunities
  UNION ALL SELECT 'executions',       count(*) FROM executions
  UNION ALL SELECT 'risk_events',      count(*) FROM risk_events
  UNION ALL SELECT 'audit_log',        count(*) FROM audit_log
  UNION ALL SELECT 'strategy_scores',  count(*) FROM strategy_scores
  ORDER BY 1;
"
echo "[restore-pg] last executions:"
psql "$STAGING_URL" -c "
  SELECT submitted_at, status, relay_name FROM executions
  ORDER BY submitted_at DESC LIMIT 5;
"

echo
echo "[restore-pg] DONE. Staging DB ready as $STAGING_DB."
echo "Next: inspect manually. To promote, follow docs/runbooks/db-restore.md §Remediation."
