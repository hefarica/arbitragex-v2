#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# M5 — A.4 fork validation wrapper.
#
# This does NOT reimplement anything: it DELEGATES to the canonical, doctrine-
# compliant runner scripts/run_a4_fork_validation.sh (fork-only, anti-hollow-pass:
# it refuses to record a pass unless the test prints A4_OUTCOME AND libtest reports
# >=1 passing test). READ-ONLY: mainnet fork sim, no broadcast, no deploy, no
# signer, no key, no capital.
#
# HONEST GATE STATUS (do NOT report un-instrumented gates as green):
#   * GATE A.4 (fork validation) ......... REAL — enforced by the runner.
#   * latency p95 < 500ms ................ NOT INSTRUMENTED — the test emits
#       gas_used, not a p95 distribution. Needs a dedicated benchmark harness.
#       Below we record a single wall-clock OBSERVATION, which is NOT a p95.
#   * "10 successful sims" ............... NOT INSTRUMENTED — the test runs ONE
#       canonical WETH->USDC->WETH route today. N-route (N=10) needs the test
#       parameterized over a route table.
#
# Env (passed through to the runner):
#   RPC_HTTP_1                       single bare archive RPC URL (required)
#   EXECUTOR_1                       any non-zero, code-less address (caller label)
#   SIM_ORCHESTRATOR_GAS_PRICE_WEI   decimal wei (required for net-profit accounting)
# ---------------------------------------------------------------------------
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNNER="${ROOT}/scripts/run_a4_fork_validation.sh"

if [ ! -f "${RUNNER}" ]; then
  echo "M5 ABORT: canonical runner not found at ${RUNNER}." >&2
  exit 1
fi

START_MS="$(date -u +%s%3N 2>/dev/null || echo "$(( $(date -u +%s) * 1000 ))")"
bash "${RUNNER}"
END_MS="$(date -u +%s%3N 2>/dev/null || echo "$(( $(date -u +%s) * 1000 ))")"

echo "----------------------------------------------------------------------"
echo "M5 A.4 wall-clock: $((END_MS - START_MS)) ms  (OBSERVATION — NOT a p95 latency gate)"
echo "M5 validate summary:"
echo "  - A.4 fork gate ........... PASS (enforced by run_a4_fork_validation.sh)"
echo "  - p95-latency < 500ms ..... NOT-INSTRUMENTED (follow-up: benchmark harness)"
echo "  - 10-successful-sims ...... NOT-INSTRUMENTED (follow-up: parameterize route table)"
echo "----------------------------------------------------------------------"
