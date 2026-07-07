#!/usr/bin/env bash
# backup-offsite.sh — push the latest encrypted backups to Backblaze B2
# (or any S3-compatible endpoint rclone can talk to).
#
# Doctrine: nothing about the destination is hardcoded. The operator provides
# an rclone remote name (set up out-of-band via `rclone config`) and a bucket
# path. No provider defaults.
#
# Usage:
#   BACKUP_DIR=/var/backups/arbx \
#   RCLONE_REMOTE=arbx-b2 \
#   RCLONE_BUCKET=arbx-backups/prod \
#     ./backup-offsite.sh

set -euo pipefail

BACKUP_DIR="${BACKUP_DIR:?BACKUP_DIR required}"
RCLONE_REMOTE="${RCLONE_REMOTE:?RCLONE_REMOTE required (rclone config name)}"
RCLONE_BUCKET="${RCLONE_BUCKET:?RCLONE_BUCKET required (e.g. arbx-backups/prod)}"

command -v rclone >/dev/null 2>&1 || { echo "missing tool: rclone" >&2; exit 2; }

if ! rclone lsd "$RCLONE_REMOTE:" >/dev/null 2>&1; then
  echo "backup-offsite: rclone remote '$RCLONE_REMOTE' unreachable. Re-run rclone config." >&2
  exit 3
fi

# Only push *.age and *.age.sha256. Never push the plaintext dump or gz tmps
# (they never hit $BACKUP_DIR in the first place — enforced by backup-pg.sh
# which cleans its tmpdir — but double-check with a filter).
FILTER_OPTS=(--include "pg-*.sql.custom.gz.age" --include "pg-*.sql.custom.gz.age.sha256")

echo "[$(date -u +%FT%TZ)] backup-offsite: pushing to $RCLONE_REMOTE:$RCLONE_BUCKET"
rclone copy "$BACKUP_DIR" "$RCLONE_REMOTE:$RCLONE_BUCKET" \
  "${FILTER_OPTS[@]}" \
  --transfers 2 \
  --checkers 4 \
  --no-traverse \
  --retries 3 \
  --log-level NOTICE

# Verify by listing what made it.
COUNT="$(rclone ls "$RCLONE_REMOTE:$RCLONE_BUCKET" | grep -c 'pg-.*\.age$' || true)"
echo "[$(date -u +%FT%TZ)] backup-offsite: $COUNT encrypted backups present on remote"
