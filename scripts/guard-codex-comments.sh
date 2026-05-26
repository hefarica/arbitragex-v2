#!/usr/bin/env bash
set -euo pipefail

TASKS_FILE="${1:-.agents/github/codex-pr-103-tasks.md}"

if [ ! -f "$TASKS_FILE" ]; then
  echo "Codex task matrix not found: $TASKS_FILE"
  echo "Run scripts/sync-codex-comments.sh first."
  exit 1
fi

if grep -E '\| P1 \|' "$TASKS_FILE" | grep -E '\| PENDING \|' >/dev/null; then
  echo "Blocked: pending P1 Codex comments exist."
  grep -E '\| P1 \|' "$TASKS_FILE" | grep -E '\| PENDING \|' || true
  exit 1
fi

echo "Codex guard passed: no pending P1 comments."
