#!/usr/bin/env bash
# backup-pg.sh — encrypted, off-device-ready Postgres backup.
#
# Flow:
#   1. pg_dump --format=custom --no-owner --no-acl
#   2. gzip -9
#   3. age -r <operator-pubkey> (asymmetric encryption; only the operator's
#      private key can decrypt).
#   4. Write to $BACKUP_DIR/pg-<YYYYMMDD-HHMM>.sql.custom.gz.age
#
# Doctrine: the recipient pubkey is an operator input (Phase 5 onboarding).
# We never embed a default pubkey here. Without AGE_RECIPIENT the script
# refuses to run — no unencrypted backups on disk.
#
# Usage:
#   BACKUP_DIR=/var/backups/arbx \
#   AGE_RECIPIENT=$(cat /etc/arbx/backup-recipient.age.pub) \
#   DATABASE_READONLY_URL=postgres://arbx_ro:...@postgres:5432/arbitragex \
#     ./backup-pg.sh

set -euo pipefail

BACKUP_DIR="${BACKUP_DIR:?BACKUP_DIR required, e.g. /var/backups/arbx}"
AGE_RECIPIENT="${AGE_RECIPIENT:?AGE_RECIPIENT required (operator public key, age1... format)}"
DATABASE_URL_FOR_DUMP="${DATABASE_READONLY_URL:-${DATABASE_URL:?need DATABASE_READONLY_URL or DATABASE_URL}}"
RETAIN_DAYS="${RETAIN_DAYS:-14}"

for tool in pg_dump gzip age; do
  command -v "$tool" >/dev/null 2>&1 || { echo "missing tool: $tool" >&2; exit 2; }
done

mkdir -p "$BACKUP_DIR"

STAMP="$(date -u +%Y%m%d-%H%M)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

DUMP_TMP="$TMP_DIR/pg-$STAMP.sql.custom"
GZ_TMP="$TMP_DIR/pg-$STAMP.sql.custom.gz"
OUT="$BACKUP_DIR/pg-$STAMP.sql.custom.gz.age"

echo "[$(date -u +%FT%TZ)] backup-pg: dumping to $DUMP_TMP"
pg_dump --format=custom --no-owner --no-acl \
        --file="$DUMP_TMP" "$DATABASE_URL_FOR_DUMP"

DUMP_BYTES=$(stat -c %s "$DUMP_TMP" 2>/dev/null || stat -f %z "$DUMP_TMP")
if [ "${DUMP_BYTES:-0}" -lt 1024 ]; then
  echo "backup-pg: dump file suspiciously small ($DUMP_BYTES bytes) — refusing" >&2
  exit 3
fi
echo "[$(date -u +%FT%TZ)] backup-pg: dump size ${DUMP_BYTES} bytes"

echo "[$(date -u +%FT%TZ)] backup-pg: gzipping"
gzip -9 -c "$DUMP_TMP" > "$GZ_TMP"

echo "[$(date -u +%FT%TZ)] backup-pg: encrypting with age"
age -r "$AGE_RECIPIENT" -o "$OUT" "$GZ_TMP"

# Hash for offsite integrity check.
SHA256="$(sha256sum "$OUT" | cut -d' ' -f1)"
echo "$SHA256  $(basename "$OUT")" > "$OUT.sha256"

echo "[$(date -u +%FT%TZ)] backup-pg: wrote $OUT (sha256 $SHA256)"

# Retention — local only. Offsite retention is managed by rclone/B2 lifecycle.
find "$BACKUP_DIR" -maxdepth 1 -type f -name 'pg-*.sql.custom.gz.age' \
  -mtime +"$RETAIN_DAYS" -print -delete || true
find "$BACKUP_DIR" -maxdepth 1 -type f -name 'pg-*.sql.custom.gz.age.sha256' \
  -mtime +"$RETAIN_DAYS" -print -delete || true

echo "[$(date -u +%FT%TZ)] backup-pg: done"
