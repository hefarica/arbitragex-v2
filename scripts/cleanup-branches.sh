#!/usr/bin/env bash
# cleanup-branches.sh — Git branch retention policy (auditoría PERF-STACK 2026-09-20, Deuda 2).
#
# Deletes remote branches that are MERGED into main and older than RETENTION_DAYS.
# DRY-RUN by default: nothing is deleted unless --apply is passed.
#
# Safety rules (operator standing orders):
#   - codex/* is NEVER touched (operator order 2026-09-20: "nunca borrar codex/*",
#     already-merged codex history is recovered via refs/pull/N/head — keep them anyway).
#   - main / master / HEAD / the current branch are never touched.
#   - Unmerged branches are only LISTED, never deleted — deleting them is a
#     per-branch human decision, never a retention policy.
#
# Usage:
#   scripts/cleanup-branches.sh                  # dry-run, 30-day retention
#   scripts/cleanup-branches.sh --apply          # actually delete merged+stale
#   RETENTION_DAYS=14 scripts/cleanup-branches.sh
set -euo pipefail

RETENTION_DAYS="${RETENTION_DAYS:-30}"
APPLY=0
for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

PROTECTED='^(main|master|HEAD)$|^codex/'
CURRENT_BRANCH="$(git branch --show-current)"
CUTOFF=$(( $(date +%s) - RETENTION_DAYS * 86400 ))

[ -n "$CURRENT_BRANCH" ] && PROTECTED="^(main|master|HEAD|${CURRENT_BRANCH//\//\\/})$|^codex/"

git fetch --prune origin >/dev/null 2>&1 || git fetch origin >/dev/null

merged_stale=()
unmerged_stale=()
while read -r ref ts; do
  branch="${ref#refs/remotes/origin/}"
  [ "$branch" = "HEAD" ] && continue
  echo "$branch" | grep -Eq "$PROTECTED" && continue
  [ "$ts" -lt "$CUTOFF" ] || continue
  if git merge-base --is-ancestor "$ref" origin/main 2>/dev/null; then
    merged_stale+=("$branch")
  else
    unmerged_stale+=("$branch")
  fi
done < <(git for-each-ref --format='%(refname) %(committerdate:unix)' refs/remotes/origin)

echo "== merged into main + stale (> ${RETENTION_DAYS}d): ${#merged_stale[@]} =="
for b in "${merged_stale[@]}"; do echo "  DEL $b"; done
echo "== unmerged + stale (LIST ONLY): ${#unmerged_stale[@]} =="
for b in "${unmerged_stale[@]}"; do echo "  keep $b (unmerged)"; done

if [ "$APPLY" -ne 1 ]; then
  echo "DRY-RUN — pass --apply to delete the ${#merged_stale[@]} merged-stale branches."
  exit 0
fi

[ "${#merged_stale[@]}" -eq 0 ] && { echo "nothing to delete"; exit 0; }
for b in "${merged_stale[@]}"; do
  git push origin --delete "$b"
done
echo "deleted ${#merged_stale[@]} branches."
