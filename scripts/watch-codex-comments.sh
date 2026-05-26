#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-hefarica/arbitragex-v2}"
PR="${2:-103}"
INTERVAL="${3:-60}"

while true; do
  echo "[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] Syncing Codex bot comments..."
  bash scripts/sync-codex-comments.sh "$REPO" "$PR" || true
  sleep "$INTERVAL"
done
