#!/usr/bin/env bash
# G-WALLET-PROJECTID-01 — /wallet must not hand a placeholder project id to the
# WalletConnect / Reown SDK (PR-A, SEV-1).
#
# THREE CONDITIONS (all must hold):
#   (a) the placeholder project-id literal appears in ZERO built client chunks;
#   (b) a headless visit to /wallet produces NO >=400 response from
#       pulse.walletconnect.org or api.web3modal.org;
#   (c) the degraded state is DECLARED in the first (server) render — the SSR
#       HTML carries the notice testid and the machine reason (R8: never empty,
#       never fabricated).
#
# The third condition is what makes this gate non-gameable: hiding the connector
# without declaring why would fail (c).
#
# Usage: scripts/gates/g-wallet-projectid-01.sh [base_url]
#   base_url defaults to a locally started `next start` (must already be up).
# Exit: 0 PASS, 1 FAIL, 2 SKIP (no build/server available).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

BASE="${1:-http://127.0.0.1:3111}"
PLACEHOLDER="walletconnect_project_id_missing"
FAIL=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; FAIL=1; fi; }

say "G-WALLET-PROJECTID-01 — /wallet placeholder project id"
say "base: $BASE"
say ""

# ---------- (a) built client chunks ----------
CHUNKS="$ROOT/frontend/.next/static/chunks"
if [ ! -d "$CHUNKS" ]; then
  say "  SKIP  frontend/.next/static/chunks absent — run 'next build' first (declared debt, not silently green)"
  exit 2
fi
N=$(find "$CHUNKS" -name '*.js' | wc -l | tr -d ' ')
HITS=$(grep -rl "$PLACEHOLDER" "$CHUNKS" 2>/dev/null | wc -l | tr -d ' ')
say "  info  built client chunks: $N"
[ "$HITS" = "0" ]; chk "(a) 0 built chunks contain the placeholder project-id literal (found $HITS)" $?

# ---------- (c) declared in the first render ----------
HTML=$(curl -s -m 25 "$BASE/wallet")
[ -n "$HTML" ]; chk "(c) /wallet responded with HTML" $?
printf '%s' "$HTML" | grep -q 'walletconnect-degraded-notice'
chk "(c) SSR HTML carries the degraded-notice testid" $?
printf '%s' "$HTML" | grep -q 'walletconnect_not_configured'
chk "(c) SSR HTML carries the machine reason (declared, not fabricated)" $?

# ---------- (b) no upstream calls with a bogus id ----------
if ! command -v node >/dev/null 2>&1; then
  say "  SKIP  node absent — cannot run the headless probe"
  exit 2
fi
PROBE="$ROOT/tests/e2e/.g-walletprobe.mjs"  # inside tests/e2e so `playwright` resolves from its node_modules
cat > "$PROBE" <<'EOF'
import { chromium } from "playwright";
const base = process.argv[2];
const bad = [];
const browser = await chromium.launch();
const ctx = await browser.newContext();
const page = await ctx.newPage();
page.on("response", (r) => {
  const u = r.url();
  if ((u.includes("pulse.walletconnect.org") || u.includes("api.web3modal.org")) && r.status() >= 400) {
    bad.push(`${r.status()} ${u.slice(0, 140)}`);
  }
});
await page.goto(base + "/wallet", { waitUntil: "load", timeout: 45000 });
await page.waitForTimeout(4000); // give any SDK time to phone home
const declared = await page.locator('[data-testid="walletconnect-degraded-notice"]').count();
const reason = await page.locator('[data-testid="walletconnect-degraded-reason"]').innerText().catch(() => "");
await browser.close();
console.log(JSON.stringify({ bad, declared, reason }));
EOF
OUT=$(cd "$ROOT/tests/e2e" 2>/dev/null && node "$PROBE" "$BASE" 2>/dev/null)
rm -f "$PROBE"
BAD=$(printf '%s' "$OUT" | python3 -c "import json,sys;print(len(json.load(sys.stdin).get('bad',[])))" 2>/dev/null || echo "-1")
say "  info  headless probe: ${OUT:0:200}"
[ "$BAD" = "0" ]; chk "(b) no >=400 response from pulse.walletconnect.org / api.web3modal.org" $?
DECLARED=$(printf '%s' "$OUT" | python3 -c "import json,sys;print(json.load(sys.stdin).get('declared',0))" 2>/dev/null || echo 0)
[ "$DECLARED" -ge 1 ] 2>/dev/null; chk "(c) hydrated DOM keeps the declaration" $?

say ""
if [ "$FAIL" = "0" ]; then say "G-WALLET-PROJECTID-01: PASS"; else say "G-WALLET-PROJECTID-01: FAIL"; fi
exit "$FAIL"
