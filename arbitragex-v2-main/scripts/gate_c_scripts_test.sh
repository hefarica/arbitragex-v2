#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Gate C — behavioral tests for the A.4 / A.5 scripts (doctrine guards).
#
# Asserts the env/marker guards WITHOUT running the real fork test or touching
# the real A.4 marker (uses a temp marker via GATE_C_A4_MARKER). Pure guard-path
# checks — they exit before any cargo/psql work.
#
# Usage:  bash scripts/gate_c_scripts_test.sh
# ---------------------------------------------------------------------------
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
A4="${REPO_ROOT}/scripts/run_a4_fork_validation.sh"
A5="${REPO_ROOT}/scripts/activate_paper_shadow.sh"
TMP_MARKER="$(mktemp -u)"
PASS=0
FAIL=0

ok()   { echo "  ok  - $*"; PASS=$((PASS+1)); }
bad()  { echo "  FAIL- $*"; FAIL=$((FAIL+1)); }

# 6. A.4 fails without RPC_HTTP_1 -------------------------------------------
( unset RPC_HTTP_1; EXECUTOR_1=0xexec bash "$A4" ) >/dev/null 2>&1
[ $? -ne 0 ] && ok "A.4 fails without RPC_HTTP_1" || bad "A.4 should fail without RPC_HTTP_1"

# 7. A.4 fails without EXECUTOR_1 (RPC set; must exit before cargo) ---------
( unset EXECUTOR_1; RPC_HTTP_1=http://rpc.example bash "$A4" ) >/dev/null 2>&1
[ $? -ne 0 ] && ok "A.4 fails without EXECUTOR_1" || bad "A.4 should fail without EXECUTOR_1"

# 8. A.5 fails without the A.4 marker --------------------------------------
( GATE_C_A4_MARKER="${TMP_MARKER}.absent" bash "$A5" ) >/dev/null 2>&1
[ $? -ne 0 ] && ok "A.5 fails without A.4 marker" || bad "A.5 should fail without A.4 marker"

# 9. A.5 prints the paper-shadow envs when the marker exists ----------------
echo "A.4 PASSED (test)" > "${TMP_MARKER}"
OUT="$( GATE_C_A4_MARKER="${TMP_MARKER}" bash "$A5" 2>/dev/null )"
RC=$?
rm -f "${TMP_MARKER}"
if [ $RC -eq 0 ] && echo "$OUT" | grep -q "ARBX_SCORING_ENABLED=true" && echo "$OUT" | grep -q "ARBX_SCORING_HARD_GATE=false"; then
  ok "A.5 prints paper-shadow envs with marker present"
else
  bad "A.5 should print envs (rc=$RC) with marker present"
fi

echo
echo "gate_c_scripts_test: ${PASS} passed, ${FAIL} failed"
[ "$FAIL" -eq 0 ]
