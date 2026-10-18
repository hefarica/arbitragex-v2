#!/usr/bin/env bash
# G-OMEGA-BASE-01 — .omega-base-sha pin validity gate (drift D-05).
#
# WHY: the file pinned 70db8183… — an object that EXISTS in the repository but
# is unreachable from main (the history was rewritten: no common ancestor with
# the current tip), so it can never serve as a base. Worse, a shallow or
# single-branch clone reports it as "Not a valid object name", which is how the
# original finding was mis-diagnosed as "absent from history".
#
# WHAT THIS GATE PINS
#   1. The pin is a syntactically valid 40-hex commit id.
#   2. The pinned object is actually present in this clone.
#   3. The pin equals the audited programme base recorded in the inventory
#      (single source of truth: docs/reaintegration/00-INVENTORY.json → repo.head),
#      so the pin can never silently drift from the audited base again.
#
# Exit 0 = PASS, exit 1 = FAIL.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; fail=1; fi; }

say "G-OMEGA-BASE-01 — .omega-base-sha pin validity gate"

PIN_FILE=".omega-base-sha"
INVENTORY="docs/reaintegration/00-INVENTORY.json"

if [ ! -f "$PIN_FILE" ]; then
  say "  FAIL  $PIN_FILE does not exist"
  say ""; say "G-OMEGA-BASE-01: FAIL"; exit 1
fi

PIN=$(sed -n 's/^BASE_SHA=\(.*\)$/\1/p' "$PIN_FILE" | head -1 | tr -d '[:space:]')
say "  info  pinned BASE_SHA = ${PIN:-<empty>}"

# 1. syntax
printf '%s' "$PIN" | grep -Eq '^[0-9a-f]{40}$'
chk "pin is a 40-hex commit id" $?

# 2. object present in this clone
git cat-file -e "${PIN}^{commit}" 2>/dev/null
chk "pinned commit is present as an object in this clone" $?

# 3. equals the audited programme base (inventory.repo.head)
AUDITED=$(python3 - <<'PY'
import json
print(json.load(open("docs/reaintegration/00-INVENTORY.json"))["repo"]["head"])
PY
)
say "  info  audited programme base (inventory.repo.head) = $AUDITED"
[ "$PIN" = "$AUDITED" ]
chk "pin equals the audited programme base" $?

say ""
if [ "$fail" = "0" ]; then say "G-OMEGA-BASE-01: PASS"; else say "G-OMEGA-BASE-01: FAIL"; fi
exit "$fail"
