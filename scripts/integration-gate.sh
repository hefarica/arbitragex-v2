#!/usr/bin/env bash
# Integration Gate — Session C (Integrador + Guardián Doctrinal).
#
# Run BEFORE any commit-to-main or VPS deploy. It is the single serialization
# point for the 3-session split (A=searcher-rs/math-engine, B=shared-rs/relays/
# api/frontend, C=this gate + deploy). It writes NO feature code; it only
# verifies. Exit 0 = GREEN (safe to merge/deploy), non-zero = RED (abort).
#
# Usage:  bash scripts/integration-gate.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
fail=0
red() { echo "  RED — $*"; fail=1; }

echo "== 1. DOCTRINE: no live-execution wiring outside the dormant homes =="
# Forbidden patterns. Legit homes: relays-client (paper-gated, dormant executor)
# and the OMEGA SEAL in main.rs (which NAMES these keys only to panic on them).
PATTERN='LocalWallet|send_bundle|eth_sendBundle|eth_send_raw_transaction|eth_sendRawTransaction|ARBX_TESTNET_PRIVATE_KEY|ARBX_EXECUTOR_PRIVATE_KEY|EXECUTOR_PRIVATE_KEY'
hits=$(git grep -nE "$PATTERN" -- backend/ frontend/ 2>/dev/null \
  | grep -vE '^backend/relays-client/' \
  | grep -vE '^backend/searcher-rs/src/main\.rs:' \
  | grep -vE '/(tests?|__tests__)/' \
  | awk -F: '{ c=$0; sub(/^[^:]*:[^:]*:/,"",c); gsub(/^[ \t]+/,"",c); if (c !~ /^(\/\/|\/\*|\*|#)/) print }' \
  || true)
  # The awk filter drops PURE-COMMENT matches (lines whose code portion starts
  # with //, //!, /*, *, or #). Many legit files NAME these patterns in doc/comments
  # precisely to assert they are NOT wired (chain_client, rpc_multiplexer, the
  # execution_worker stub, sed-core's simulation-only connectors). Real wiring —
  # e.g. `let w = LocalWallet::from_str(..)` — is CODE and still trips the gate.
if [ -n "$hits" ]; then red "live-wiring outside dormant paths:"; echo "$hits" | sed 's/^/    /'; else echo "  OK (only dormant relays-client + OMEGA SEAL)"; fi

echo "== 2. OMEGA SEAL intact (must still panic on testnet/executor keys) =="
git grep -q "ARBX_TESTNET_PRIVATE_KEY" -- backend/searcher-rs/src/main.rs \
  && echo "  OK (seal guard present)" || red "OMEGA SEAL testnet-key guard missing — Block-4 line broken"

echo "== 3. Clause-34 not silently active (Block 4 governance) =="
if git grep -qi "TESTNET EXECUTION TIER" -- CLAUDE.md 2>/dev/null; then
  red "Clause 34 found in CLAUDE.md — testnet-exec governance was adopted. Requires operator decision + cred rotation FIRST."
else echo "  OK (no testnet-exec clause in active CLAUDE.md)"; fi

echo "== 4. RUST build + tests (touched crates) =="
( cd backend && cargo test -p searcher-rs -p shared-rs -p math-engine --lib 2>&1 | tail -4 ) || red "rust tests failed"

echo "== 5. TS typecheck =="
node node_modules/typescript/bin/tsc --noEmit -p backend/api-server/tsconfig.json >/dev/null 2>&1 \
  && echo "  api-server tsc OK" || red "api-server tsc failed"
node frontend/node_modules/typescript/bin/tsc --noEmit -p frontend/tsconfig.json >/dev/null 2>&1 \
  && echo "  frontend tsc OK" || red "frontend tsc failed"

echo "== 6. opps:detected invariant (run on VPS before+after deploy) =="
echo "  ssh arbx 'docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected'  — must NEVER decrease in shadow"

echo "== 7. Frontend E2E (read-only, live target) — 'evidencia dura' =="
# Reuses tests/e2e/playwright.live.config.ts: read-only by default — the mutating
# spec (admin-mutating-live.spec.ts) auto-SKIPS unless ARBX_ADMIN_TOKEN is set, so
# the gate observes panels rendering real backend data + honest states WITHOUT
# touching state. Runs ONLY when a live URL is provided: C runs it post-deploy;
# A/B local gate runs skip it cleanly. One-time setup:
#   (cd tests/e2e && npm ci && npm run install-browsers)
if [ -n "${ARBX_FRONTEND_URL:-}" ]; then
  # `test:live` = the READ-ONLY subset (functional-live + admin-gate-live +
  # honest-state-live); it deliberately EXCLUDES admin-mutating-live. No state
  # is touched even if ARBX_ADMIN_TOKEN happens to be set.
  ( cd tests/e2e && npm run test:live 2>&1 | tail -10 ) \
    || red "frontend live E2E failed (panels not rendering / honest-state regression)"
else
  echo "  SKIP — set ARBX_FRONTEND_URL=https://edge-arbx.ape-tv.net (or the VPS edge) to run the live read-only frontend E2E"
fi

echo "------------------------------------------------------------"
if [ "$fail" -ne 0 ]; then echo "INTEGRATION GATE: RED — do NOT merge/deploy"; exit 1; else echo "INTEGRATION GATE: GREEN"; fi
