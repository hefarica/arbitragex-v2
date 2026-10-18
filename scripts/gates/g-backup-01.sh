#!/usr/bin/env bash
# G-BACKUP-01 — exclusion gate for backup/recovery trees.
#
# WHY: HEAD fe62cdce ships 305 git-tracked paths under *_backup / _recovery /
# *.backup / *.bak. A naive `find -name page.tsx | wc -l` returns 111 while the
# live App Router has 59; route/E2E/build counts and "70 navigable routes"
# audit figures are therefore polluted (drift D-01).
#
# WHAT THIS GATE PINS
#   1. The canonical exclusion predicate is declared once, here.
#   2. No backup tree is reachable from the build/typecheck manifests
#      (frontend/tsconfig.json include[], next.config.js, root+frontend
#      package.json workspaces).
#   3. The live frontend route count is computed on the FILTERED set and is
#      stable (59 page.tsx at HEAD fe62cdce).
#   4. Legitimate source files whose NAME contains "recovery" are NOT excluded
#      (e.g. backend/sim-ctl/tests/simwire02_pel_recovery.rs).
#
# Exit 0 = PASS, exit 1 = FAIL. Every check prints an explicit line.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; fail=1; fi; }

# --- canonical exclusion predicate (single source of truth) -----------------
# A path is BACKUP-CLASS iff:
#   - any path segment is exactly `_recovery`, or matches `*_backup` as a
#     DIRECTORY, or
#   - the basename ends in `.backup` or `.bak`.
# Files that merely contain the substring "recovery"/"backup" in their name are
# NOT backup-class (e.g. simwire02_pel_recovery.rs).
backup_class() {
  awk -F/ '{
    for (i=1;i<NF;i++) if ($i=="_recovery" || $i ~ /_backup$/) { print; next }
    b=$NF; if (b ~ /\.(backup|bak)$/) print
  }'
}

say "G-BACKUP-01 — backup/recovery exclusion gate"
say "repo root: $ROOT"
say ""

# --- 1. inventory of backup-class tracked paths -----------------------------
TRACKED_BACKUP=$(git ls-files | backup_class)
N_TRACKED=$(printf '%s\n' "$TRACKED_BACKUP" | grep -c . || true)
say "1) backup-class paths tracked in git: $N_TRACKED"
printf '%s\n' "$TRACKED_BACKUP" | sed 's/^/     /' | sort -u | head -12
say "     ... (full list: docs/reaintegration/evidence/G-BACKUP-01.tracked.txt)"
printf '%s\n' "$TRACKED_BACKUP" | sort -u > docs/reaintegration/evidence/G-BACKUP-01.tracked.txt

# --- 2. backup trees must not be reachable from build/typecheck -------------
INC=$(python3 - <<'PY'
import json,re,pathlib
ts=json.load(open("frontend/tsconfig.json"))
inc=ts.get("include",[])+ts.get("exclude",[])
bad=[p for p in inc if re.search(r"_backup|_recovery|\.backup|\.bak",p)]
print("BAD" if bad else "OK", ";".join(inc))
PY
)
case "$INC" in BAD*) chk "frontend/tsconfig.json include/exclude has no backup path" 1;; *) chk "frontend/tsconfig.json include/exclude has no backup path" 0;; esac

NC=$(python3 - <<'PY'
import re
src=open("frontend/next.config.js").read()
print("BAD" if re.search(r"_backup|_recovery|\.backup|\.bak",src) else "OK")
PY
)
case "$NC" in BAD*) chk "frontend/next.config.js references no backup path" 1;; *) chk "frontend/next.config.js references no backup path" 0;; esac

WS=$(python3 - <<'PY'
import json
bad=[]
for f in ("package.json","frontend/package.json"):
    d=json.load(open(f))
    w=d.get("workspaces",[])
    for p in w:
        if any(k in p for k in ("_backup","_recovery",".backup",".bak")): bad.append(f+":"+p)
print("BAD" if bad else "OK")
PY
)
case "$WS" in BAD*) chk "package.json workspaces reference no backup path" 1;; *) chk "package.json workspaces reference no backup path" 0;; esac

# --- 3. live route count on the FILTERED set --------------------------------
# keep = all page.tsx minus backup-class ones
N_ALL=$(find frontend/app -name page.tsx | wc -l | tr -d ' ')
N_DROP=$(find frontend/app -name page.tsx | backup_class | wc -l | tr -d ' ')
N_PAGES=$((N_ALL - N_DROP))
# sanity: the same count computed by explicit pruning
N_PAGES2=$(find frontend/app -name page.tsx -not -path '*_backup*' -not -path '*_recovery*' | wc -l | tr -d ' ')
say ""
say "3) live App Router page.tsx (filtered): $N_PAGES  [all=$N_ALL, backup-class dropped=$N_DROP] (cross-check: $N_PAGES2)"
[ "$N_PAGES" = "$N_PAGES2" ]; chk "filtered route count is stable across two computations" $?
[ "$N_PAGES" = "59" ]; chk "live page.tsx count == 59 (HEAD fe62cdce)" $?

# --- 4. legitimate '*_recovery*' source files survive the predicate ---------
KEEP=$(printf '%s\n' "backend/sim-ctl/tests/simwire02_pel_recovery.rs" | backup_class | wc -l | tr -d ' ')
[ "$KEEP" = "0" ]; chk "legitimate source simwire02_pel_recovery.rs is NOT backup-class" $?

# --- 5. repo-wide pollution pinned (the D-01 symptom) -----------------------
ALL_RW=$(find . -not -path './.git/*' -name page.tsx | wc -l | tr -d ' ')
DROP_RW=$(find . -not -path './.git/*' -name page.tsx | backup_class | wc -l | tr -d ' ')
say ""
say "5) repo-wide page.tsx: all=$ALL_RW, backup-class=$DROP_RW, filtered=$((ALL_RW-DROP_RW))"
[ "$DROP_RW" -gt 0 ]; chk "backup-class page.tsx ARE present repo-wide (drift is real, gate has teeth)" $?
[ "$((ALL_RW-DROP_RW))" = "59" ]; chk "repo-wide filtered page.tsx count == 59" $?

say ""
if [ "$fail" = "0" ]; then say "G-BACKUP-01: PASS"; else say "G-BACKUP-01: FAIL"; fi
exit "$fail"
