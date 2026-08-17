#!/usr/bin/env bash
# lint-migration-index-locks.sh — FREEZE-01 / ANTI-FREEZE FASE 2.2 (2026-08-17)
#
# Doctrine gate: a migration that builds an index on an ALREADY-POPULATED
# table WITHOUT CONCURRENTLY takes a lock that blocks INSERTs for the whole
# build (FREEZE-01: 21h pipeline silence via exactly this shape).
#
# Rule: every CREATE [UNIQUE] INDEX in database/migrations/*.sql must use
# CONCURRENTLY, UNLESS the indexed table is CREATEd in the SAME file (a brand
# new table is empty — a non-concurrent build is instantaneous and safe).
#
# Legacy carve-outs (frozen history, do not extend):
#   - migrations numbered < 105 are PRE-DOCTRINE history: not rewritten
#     (reformatting frozen migrations adds churn without changing prod),
#     but any NEW migration touching them must comply.
#   - database/init/* is first-boot only (empty DB) — out of scope.
set -euo pipefail

MIG_DIR="$(cd "$(dirname "$0")/../../database/migrations" && pwd)"
FAIL=0

while IFS= read -r f; do
  base="$(basename "$f")"
  num="${base%%[_.]*}"
  # Skip pre-doctrine history (numeric part < 105).
  if [[ "$num" =~ ^[0-9]+$ ]] && [ "$num" -lt 105 ]; then
    continue
  fi
  # Tables created in this same file are exempt (empty at build time).
  created="$(grep -oiE 'CREATE TABLE (IF NOT EXISTS )?[a-z_]+' "$f" | awk '{print $NF}' | tr '[:upper:]' '[:lower:]' | sort -u || true)"
  # Every CREATE [UNIQUE] INDEX line (excluding `--` comment lines)...
  while IFS= read -r -u 3 line; do
    # Strip "N:" prefix, skip pure comment lines (the RCA narrative in
    # migration headers legitimately quotes the offending shape).
    body="$(echo "$line" | sed 's/^[0-9]*://')"
    if echo "$body" | grep -qE '^\s*--'; then
      continue
    fi
    # ... must contain CONCURRENTLY ...
    if echo "$body" | grep -qiE 'CREATE (UNIQUE )?INDEX' && ! echo "$body" | grep -qi 'CONCURRENTLY'; then
      # ... unless its table is created in the same file.
      tbl="$(echo "$body" | grep -oiE 'ON [a-z_]+' | awk '{print $2}' | tr '[:upper:]' '[:lower:]' || true)"
      if [ -n "$tbl" ] && echo "$created" | grep -qx "$tbl"; then
        continue
      fi
      echo "FAIL: $base: non-CONCURRENT index build on existing table:" >&2
      echo "  $line" >&2
      echo "  (use CREATE INDEX CONCURRENTLY, or CREATE the table in this file)" >&2
      FAIL=1
    fi
  done 3< <(grep -inE 'CREATE (UNIQUE )?INDEX' "$f" || true)
done < <(find "$MIG_DIR" -maxdepth 1 -type f -name '*.sql' | sort)

if [ "$FAIL" -ne 0 ]; then
  echo "lint-migration-index-locks: FAILED (see above)" >&2
  exit 1
fi
echo "lint-migration-index-locks: OK (no non-CONCURRENT index builds on existing tables in migrations >= 105)"
