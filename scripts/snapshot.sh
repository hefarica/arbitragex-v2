#!/usr/bin/env bash
set -euo pipefail

# snapshot.sh — Genera snapshot del estado actual del repositorio

REPO="${1:-.}"
TS=$(date +%Y%m%d_%H%M%S)
OUT="$REPO/.audit/snapshot-$TS"

echo "📸 Generating snapshot to $OUT ..."
mkdir -p "$OUT"

cd "$REPO"

# 1. Directory tree
echo "  📝 tree.txt"
find . -not -path './.git/*' -not -path './node_modules/*' -not -path './frontend/node_modules/*' -not -path './frontend/.next/*' -not -path './backend/target/*' | sort > "$OUT/tree.txt"

# 2. Git state
echo "  📝 git_state.txt"
{
  echo "=== Branch ==="
  git branch --show-current
  echo ""
  echo "=== SHA ==="
  git rev-parse HEAD
  echo ""
  echo "=== Status ==="
  git status --short
  echo ""
  echo "=== Recent commits ==="
  git log --oneline -10
  echo ""
  echo "=== Remotes ==="
  git remote -v
} > "$OUT/git_state.txt"

# 3. Docker state
echo "  📝 docker_state.txt"
{
  echo "=== Containers ==="
  docker ps --format "{{.Names}}\t{{.Status}}" 2>/dev/null || echo "Docker not available"
  echo ""
  echo "=== Images ==="
  docker images --format "{{.Repository}}:{{.Tag}}\t{{.Size}}" 2>/dev/null | head -20 || echo "N/A"
  echo ""
  echo "=== Volumes ==="
  docker volume ls 2>/dev/null || echo "N/A"
} > "$OUT/docker_state.txt"

echo "✅ Snapshot saved to $OUT/"
echo "   - tree.txt ($(wc -l < "$OUT/tree.txt") lines)"
echo "   - git_state.txt ($(wc -l < "$OUT/git_state.txt") lines)"
echo "   - docker_state.txt ($(wc -l < "$OUT/docker_state.txt") lines)"
