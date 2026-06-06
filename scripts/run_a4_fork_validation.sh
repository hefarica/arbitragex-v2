#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Gate C — A.4 fork validation runner (READ-ONLY / fork only).
#
# Runs the ignored `multistep_fork` test against a REAL mainnet ARCHIVE RPC,
# then records the passing evidence into `gate_c_validation` (the channel the
# api-server dashboard reads). This is the ONLY way the dashboard flips
# `a4_state` from A4_PENDING to A4_PASSED — never code alone. A green `cargo
# test` exit code is NOT sufficient: the runner also verifies the test actually
# drove REVM (the A4_OUTCOME guard below), so a skipped / 0-test run can never
# be recorded as a pass.
#
# DOCTRINE (INVIOLABLE): fork validation only. NO contract edits, NO executor
# deploy, NO mainnet broadcast, NO signer, NO private key, NO capital. The
# round-trip sim calls DEX routers directly with `from = EXECUTOR_1`, prefunding
# that address purely via in-memory storage cheats — so EXECUTOR_1 is just a
# caller label (any non-zero, code-less address). Nothing is ever signed or sent.
#
# Usage:
#   RPC_HTTP_1=<archive_rpc_url> EXECUTOR_1=<0xexecutor> [DATABASE_URL=...] \
#     bash scripts/run_a4_fork_validation.sh
# ---------------------------------------------------------------------------
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AUDIT_DIR="${REPO_ROOT}/audits/gate-c"
MARKER="${GATE_C_A4_MARKER:-${AUDIT_DIR}/A4_FORK_VALIDATION_PASSED}"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
LOG="${AUDIT_DIR}/a4_fork_validation_${TS}.log"

mkdir -p "${AUDIT_DIR}"

fail() { echo "A.4 ABORTED: $*" >&2; exit 1; }

# --- 1. Require real RPC + executor + gas price (no invention) -------------
[ -n "${RPC_HTTP_1:-}" ] || fail "RPC_HTTP_1 is not set (need a single bare archive RPC URL; LazyDb does direct JSON-RPC — the multi-vendor CSV form is NOT parsed here)."
[ -n "${EXECUTOR_1:-}" ] || fail "EXECUTOR_1 is not set (any non-zero, code-less address; used only as the caller label — an address with on-chain code risks EIP-3607 rejection)."
[ -n "${SIM_ORCHESTRATOR_GAS_PRICE_WEI:-}" ] || fail "SIM_ORCHESTRATOR_GAS_PRICE_WEI is not set (decimal wei; the test requires it for net-profit accounting)."

echo "A.4 fork validation @ ${TS}"
echo "  RPC_HTTP_1 = (set, ${#RPC_HTTP_1} chars)"
echo "  EXECUTOR_1 = ${EXECUTOR_1}"
echo "  log        = ${LOG}"

# --- 2. Run the ignored fork test (fork only, no broadcast) ----------------
# `--features v2-simulator` is explicit on purpose: the test is gated by
# `#[cfg(feature = "v2-simulator")]`. Without the feature it compiles to
# nothing, `--ignored` runs 0 tests, and cargo still exits 0 — a false "pass".
set +e
cargo test --manifest-path "${REPO_ROOT}/backend/Cargo.toml" \
  --package searcher-rs --features v2-simulator \
  multistep_fork -- --ignored --nocapture 2>&1 | tee "${LOG}"
STATUS=${PIPESTATUS[0]}
set -e

if [ "${STATUS}" -ne 0 ]; then
  echo "A.4 FAILED (exit ${STATUS}). See ${LOG}. No evidence recorded." >&2
  exit 1
fi

# --- 2b. Anti-hollow-pass guard: exit code 0 is NOT enough -----------------
# Cargo exits 0 even when 0 tests ran (feature off, name filtered, or the test
# early-returned because an env var was missing). The test prints exactly one
# `A4_OUTCOME=SIM_SUCCESS|SIM_REVERT` line ONLY after it actually drove REVM
# against the fork, and libtest prints `N passed` with N>=1. Require BOTH or
# refuse to record any evidence (RULE 00 / arbx-simulation-mandatory).
if ! grep -qE 'A4_OUTCOME=(SIM_SUCCESS|SIM_REVERT)' "${LOG}"; then
  fail "exit 0 but no A4_OUTCOME marker — REVM dispatch did not run (feature off? env skipped? 0 tests?). No evidence recorded. See ${LOG}."
fi
if ! grep -qE 'test result: ok\. [1-9][0-9]* passed' "${LOG}"; then
  fail "exit 0 but libtest recorded 0 passing tests (filtered/ignored). No evidence recorded. See ${LOG}."
fi

# --- 3. On pass: write marker + record DB evidence -------------------------
printf 'A.4 fork validation PASSED at %s\nlog: %s\n' "${TS}" "${LOG}" > "${MARKER}"
echo "A.4 PASSED — marker written: ${MARKER}"

INSERT_SQL="INSERT INTO gate_c_validation (gate, status, evidence_ref) VALUES ('a4_fork_validation', 'passed', 'a4_fork_validation_${TS}.log');"

if [ -n "${DATABASE_URL:-}" ] && command -v psql >/dev/null 2>&1; then
  if psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c "${INSERT_SQL}"; then
    echo "Recorded gate_c_validation row (dashboard a4_state → A4_PASSED)."
  else
    echo "WARN: psql INSERT failed. Record it manually on the VPS DB:" >&2
    echo "  ${INSERT_SQL}" >&2
  fi
else
  echo "DATABASE_URL not set (or psql unavailable). Record the evidence on the VPS DB:"
  echo "  ${INSERT_SQL}"
fi

echo "A.4 DONE."
