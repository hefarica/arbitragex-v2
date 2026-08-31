#!/usr/bin/env bash
# lint-migration-rerun-lock-safety.sh — GEN-CI-FAIL remediation (2026-08-30)
#
# Doctrine gate: the migration runner (database/run_migrations.sh) has NO
# applied-state ledger — it re-runs EVERY migration file on EVERY deploy to
# keep repo and DB from drifting apart. That makes "idempotent" DDL a hot-path
# statement against LIVE tables, and PostgreSQL acquires the table lock BEFORE
# the IF NOT EXISTS / IF EXISTS existence check:
#
#   CREATE INDEX IF NOT EXISTS ... ON <hot>   -> ShareLock        (blocks INSERT/DELETE)
#   ALTER TABLE <hot> ADD COLUMN IF NOT ...  -> AccessExclusiveLock
#   ALTER TABLE <hot> DROP CONSTRAINT ...    -> AccessExclusiveLock
#   DROP/CREATE TRIGGER ... ON <hot>         -> ShareRowExclusiveLock
#
# With the runner's FREEZE-01 lockguard (lock_timeout=10s, which must NEVER be
# raised) a purge/delete burst on the hot table turns the no-op re-run into a
# deploy abort. Observed: deploy of ac08da8b attempt 2 aborted at [4/9]
# MIGRATION GATE FAILED — "canceling statement due to lock timeout" on
# idx_opp_status_time (003:30) while the index existed in prod.
#
# Rule: on HOT tables (continuous live writers at deploy time), lock-taking
# DDL must be catalog-guarded — the no-op path may request NO table lock.
# The compliant shape is a DO $$ block whose IF reads a catalog
# (pg_indexes / information_schema.columns / pg_trigger / pg_constraint) and
# whose EXECUTE '...' carries the DDL only for the genuinely-absent case.
# CREATE INDEX CONCURRENTLY remains compliant (ShareUpdateExclusiveLock does
# not conflict with writers). ALTER TABLE <hot> SET/RESET (...) (reloptions)
# is exempt for the same reason.
#
# Detection: one awk pass per file strips `--` comments, joins statements on
# ';', and evaluates each statement. A chunk whose FIRST keywords are bare DDL
# is checked; chunks starting with DO/IF/EXECUTE are the guarded form and are
# exempt by construction.
#
# HOT list rationale: tables with verified continuous writers at gate time —
# opportunities (searcher INSERTs + purge DELETEs), simulations (sim-ctl),
# paper_trade_runs (paper archiver). Extend the list when another table gains
# a continuous writer; the same retrofit pattern applies.
#
# Self-test: `lint-migration-rerun-lock-safety.sh --selftest` runs the rule
# against an embedded violation fixture and MUST fail (red side of the
# red/green regression proof).
set -euo pipefail

MIG_DIR="$(cd "$(dirname "$0")/../../database/migrations" && pwd)"
HOT_TABLES='opportunities|simulations|paper_trade_runs'

run_rule() {
  # $1 = file, $2 = hot-table alternation. Prints FAIL lines; exit status 0/1.
  awk -v hot="$2" -v file="$(basename "$1")" '
    function is_hot(t) { return t ~ ("^(" hot ")$") }
    function fail(msg, stmt) {
      short = substr(stmt, 1, 160)
      gsub(/\t/, " ", short)
      printf "FAIL: %s: %s\n  %s\n", file, msg, short
      bad = 1
    }
    {
      line = $0
      i = index(line, "--")
      if (i > 0) line = substr(line, 1, i - 1)
      gsub(/\r/, "", line)
      gsub(/^[ \t]+|[ \t]+$/, "", line)
      if (line == "") next
      stmt = stmt (stmt == "" ? "" : " ") line
      while ((cut = index(stmt, ";")) > 0) {
        s = substr(stmt, 1, cut - 1)
        stmt = substr(stmt, cut + 1)
        gsub(/^[ \t]+|[ \t]+$/, "", s)
        gsub(/[ \t]+/, " ", s)
        if (s == "") continue
        low = tolower(s)

        if (low ~ /^alter table [a-z_]+/) {
          tbl = low
          sub(/^alter table /, "", tbl)
          sub(/[^a-z_].*$/, "", tbl)
          if (is_hot(tbl) && low !~ ("^alter table " tbl " (set|reset) \\(")) {
            fail("unguarded ALTER TABLE on hot table \x27" tbl "\x27 (catalog-guard it: DO $$ IF ... THEN EXECUTE \x27ALTER TABLE ...\x27; see 003/033/099)", s)
          }
          continue
        }
        if (low ~ /^create (unique )?index/) {
          if (low ~ /concurrently/) continue
          tbl = ""
          if (match(low, / on [a-z_]+/)) {
            tbl = substr(low, RSTART + 4, RLENGTH - 4)
          }
          if (tbl != "" && is_hot(tbl)) {
            fail("unguarded non-CONCURRENT CREATE INDEX on hot table \x27" tbl "\x27 (catalog-guard via pg_indexes, or use CONCURRENTLY; see 003/051)", s)
          }
          continue
        }
        if (low ~ /^(drop trigger|create trigger)/) {
          tbl = ""
          if (match(low, / on [a-z_]+/)) {
            tbl = substr(low, RSTART + 4, RLENGTH - 4)
          }
          if (tbl != "" && is_hot(tbl)) {
            fail("unguarded TRIGGER DDL on hot table \x27" tbl "\x27 (catalog-guard via pg_trigger; see 025/107)", s)
          }
          continue
        }
      }
    }
    END {
      gsub(/^[ \t]+|[ \t]+$/, "", stmt)
      if (stmt != "") print "NOTE: " file ": trailing statement without semicolon (ignored)"
      exit bad ? 1 : 0
    }
  ' "$1"
}

selftest() {
  local fixture
  fixture="$(mktemp /tmp/lint-rerun-fixture.XXXXXX.sql)"
  cat > "$fixture" <<'FIX'
-- fixture: one violation of each shape + compliant forms
CREATE TABLE IF NOT EXISTS t_example (id INT);

CREATE INDEX IF NOT EXISTS idx_bad ON opportunities(status);

ALTER TABLE opportunities ADD COLUMN IF NOT EXISTS bad_col TEXT;

DROP TRIGGER IF EXISTS trg_bad ON simulations;

-- compliant: concurrent
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_ok1 ON paper_trade_runs(created_at);
-- compliant: reloptions (writer-compatible lock)
ALTER TABLE opportunities SET (autovacuum_vacuum_scale_factor = 0.05);
-- compliant: guarded
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_ok2') THEN
    EXECUTE 'CREATE INDEX idx_ok2 ON opportunities(trace_id)';
  END IF;
END $$;
-- not hot: exempt
CREATE INDEX IF NOT EXISTS idx_cold ON executions(status);
FIX
  local out rc=0
  out="$(run_rule "$fixture" "$HOT_TABLES" 2>&1)" || rc=$?
  local n
  n="$(printf '%s\n' "$out" | grep -c '^FAIL:' || true)"
  rm -f "$fixture"
  if [ "$rc" -eq 0 ] || [ "$n" -ne 3 ]; then
    echo "SELFTEST FAILED: expected 3 violations, rc=$rc, got $n:" >&2
    printf '%s\n' "$out" >&2
    return 1
  fi
  echo "selftest OK: rule fires on all 3 violation shapes and passes compliant forms ($n/3 flagged)"
}

if [ "${1:-}" = "--selftest" ]; then
  selftest
  exit $?
fi

FAIL=0
while IFS= read -r f; do
  if ! run_rule "$f" "$HOT_TABLES"; then
    FAIL=1
  fi
done < <(find "$MIG_DIR" -maxdepth 1 -type f -name '*.sql' | sort)

if [ "$FAIL" -ne 0 ]; then
  echo "lint-migration-rerun-lock-safety: FAILED (see above)" >&2
  exit 1
fi
echo "lint-migration-rerun-lock-safety: OK (no unguarded lock-taking DDL re-runs on hot tables: $(echo "$HOT_TABLES" | tr '|' ' '))"
