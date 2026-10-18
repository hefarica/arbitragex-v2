#!/usr/bin/env bash
# G-PAGE-INV-01 — apex page-inventory cardinality gate (drift D-06 / INV-1).
#
# WHY: frontend/lib/apex/page_inventory.ts declares EXPECTED_PAGE_COUNT and
# documents INV-1 ("si esta constante difiere de PAGES.length, la auditoría
# falla"), but nothing executed that invariant: the constant said 37 while the
# array held 38 descriptors and the live App Router has 59 pages.
#
# WHAT THIS GATE PINS
#   1. EXPECTED_PAGE_COUNT == number of descriptors in PAGES (INV-1 holds).
#   2. The descriptor count is stable at the value this iteration recorded.
#   3. The gap between descriptors and live pages is REPORTED every run, and
#      must be declared in the inventory (D-08) — an undeclared gap fails.
#
# Exit 0 = PASS, exit 1 = FAIL.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; fail=1; fi; }

say "G-PAGE-INV-01 — apex page-inventory cardinality gate"

read -r DECLARED ENTRIES <<EOF
$(python3 - <<'PY'
import re
src = open("frontend/lib/apex/page_inventory.ts").read()
m = re.search(r"export const EXPECTED_PAGE_COUNT\s*=\s*(\d+)", src)
declared = int(m.group(1)) if m else -1
i = src.index("export const PAGES")
j = src.index("[", i)
depth = 0; k = j; entries = 0
while True:
    c = src[k]
    if c == "[": depth += 1
    elif c == "]":
        depth -= 1
        if depth == 0: break
    elif c == "{" and depth == 1:
        entries += 1
    k += 1
print(declared, entries)
PY
)
EOF
say "  info  EXPECTED_PAGE_COUNT = $DECLARED, PAGES descriptors = $ENTRIES"

# 1. INV-1 holds
[ "$DECLARED" = "$ENTRIES" ]; chk "INV-1: EXPECTED_PAGE_COUNT == PAGES.length" $?

# 2. stable descriptor count for this iteration
[ "$ENTRIES" = "38" ]; chk "PAGES descriptors == 38 (value pinned by this iteration)" $?

# 3. the descriptors-vs-live gap must stay declared
LIVE=$(find frontend/app -name page.tsx -not -path '*_backup*' -not -path '*_recovery*' | wc -l | tr -d ' ')
say "  info  live App Router page.tsx = $LIVE, descriptors = $ENTRIES, gap = $((LIVE - ENTRIES))"
python3 - <<'PY'
import json, sys
inv = json.load(open("docs/reaintegration/00-INVENTORY.json"))
ids = [d["id"] for d in inv.get("drift", [])]
sys.exit(0 if "D-08" in ids else 1)
PY
chk "descriptors-vs-live gap is declared in the inventory (D-08)" $?

say ""
if [ "$fail" = "0" ]; then say "G-PAGE-INV-01: PASS"; else say "G-PAGE-INV-01: FAIL"; fi
exit "$fail"
