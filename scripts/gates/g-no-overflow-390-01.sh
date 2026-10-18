#!/usr/bin/env bash
# G-NO-OVERFLOW-390-01 — no horizontal overflow at 390px, and nothing hidden to
# fake it (PR-E).
#
# TWO CONDITIONS, both required (the second is the anti-cheat):
#   (1) for EVERY route, `documentElement.scrollWidth - clientWidth <= 2` at a
#       390x844 viewport;
#   (2) for every route, the hydrated `body.innerText.length` must not drop more
#       than 5% versus the PRE-FIX baseline recorded in
#       docs/reaintegration/evidence/P5-scan-4viewports.json. Clipping content
#       with `overflow-x: hidden` (or `display:none`) to make (1) pass shows up
#       here as lost text.
#
# The sweep REUSES scripts/p5/p5-live-scan.mjs (no second scanner).
#
# Usage: scripts/gates/g-no-overflow-390-01.sh [base_url] [baseline_scan.json]
# Exit: 0 PASS, 1 FAIL, 2 SKIP (no node/playwright/baseline).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

BASE="${1:-http://127.0.0.1:3111}"
# Baseline = the SAME environment, PRE-FIX (local build) so the text comparison
# is not polluted by backend/API differences between environments.
# Fallback: the live pre-fix sweep committed in docs/reaintegration/evidence/.
BASELINE="${2:-${P5_BASELINE:-$ROOT/docs/reaintegration/evidence/P5-scan-4viewports.json}}"
ROUTES="$ROOT/docs/reaintegration/evidence/routes.json"
OUT="${P5_OUT:-/tmp/g-no-overflow-390.json}"
FAIL=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; FAIL=1; fi; }

say "G-NO-OVERFLOW-390-01 — 390px overflow + text-parity anti-cheat"
say "base: $BASE"

for f in "$BASELINE" "$ROUTES"; do
  [ -f "$f" ] || { say "  SKIP  missing $f"; exit 2; }
done
command -v node >/dev/null 2>&1 || { say "  SKIP  node absent"; exit 2; }
[ -d "$ROOT/tests/e2e/node_modules/playwright" ] || { say "  SKIP  playwright absent in tests/e2e"; exit 2; }

say "  ... sweeping the committed route matrix at 390 (reusing scripts/p5/p5-live-scan.mjs)"
# The scanner is the single source of truth in scripts/p5/; it is COPIED into
# tests/e2e for the run so `playwright` resolves from that node_modules (ESM
# resolves relative to the script file, not the cwd).
SCANNER="$ROOT/tests/e2e/.g-no-overflow-scanner.mjs"
cp "$ROOT/scripts/p5/p5-live-scan.mjs" "$SCANNER"
(cd "$ROOT/tests/e2e" && P5_BASE="$BASE" P5_OUT="$OUT" P5_VIEWPORTS=390 P5_ROUTES="$ROUTES" \
  PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_BROWSERS_PATH:-$HOME/.cache/ms-playwright}" \
  node "$SCANNER" >/dev/null 2>&1)
RC=$?
rm -f "$SCANNER"
[ "$RC" = "0" ]; chk "(sweep) scanner completed" $?
[ -f "$OUT" ]; chk "(sweep) result file written" $?

python3 - "$OUT" "$BASELINE" <<'PY'
import json, sys
scan = json.load(open(sys.argv[1]))
base = json.load(open(sys.argv[2]))
cur = {r["route"]: r for r in scan["results"]}
prev = {r["route"]: r for r in base["results"] if r["viewport"] == "390"}

over = []
lost = []
missing = []
for route in sorted(set(prev) | set(cur)):
    c = cur.get(route)
    b = prev.get(route)
    if c is None or b is None:
        missing.append(route); continue
    ov = c.get("scroll_overflow")
    if ov is None:
        missing.append(route); continue
    if ov > 2:
        over.append((route, ov))
    bt, bb = c.get("body_text_len") or 0, b.get("body_text_len") or 0
    if bb > 0 and bt < bb * 0.95:
        lost.append((route, bt, bb))

print(f"  info  routes compared: {len(set(prev) & set(cur))} (baseline {len(prev)}, sweep {len(cur)})")
if missing:
    print(f"  FAIL  (1)/(2) routes missing from one side: {missing[:6]}")
if over:
    print("  FAIL  (1) horizontal overflow > 2px at 390:")
    for r, o in sorted(over, key=lambda x: -x[1])[:14]:
        print(f"          {o:6d}px  {r}")
else:
    print("  PASS  (1) every route has scrollWidth - clientWidth <= 2 at 390")
if lost:
    print("  FAIL  (2) text LOSS vs baseline (content hidden to pass (1)?):")
    for r, bt, bb in sorted(lost)[:14]:
        print(f"          {r}: {bt} vs {bb} baseline ({(bt/bb-1)*100:+.1f}%)")
else:
    print("  PASS  (2) no route lost >5% of its text vs the pre-fix baseline")
sys.exit(1 if (over or lost or missing) else 0)
PY
FAIL=$?

say ""
if [ "$FAIL" = "0" ]; then say "G-NO-OVERFLOW-390-01: PASS"; else say "G-NO-OVERFLOW-390-01: FAIL"; fi
exit "$FAIL"
