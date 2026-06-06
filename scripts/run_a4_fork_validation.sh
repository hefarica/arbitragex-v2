#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Gate C — A.4 fork validation runner (READ-ONLY / fork only).
#
# Runs the ignored `multistep_fork` test against a REAL archive RPC + a deployed
# EXECUTOR address, then records the passing evidence into `gate_c_validation`
# (the channel the api-server dashboard reads). This is the ONLY way the
# dashboard flips `a4_state` from A4_PENDING to A4_PASSED — never code alone.
#
# DOCTRINE (INVIOLABLE): fork validation only. NO contract edits, NO executor
# deploy, NO mainnet broadcast, NO signer, NO private key, NO capital. The
# EXECUTOR_1 address is READ for storage-layout checks; nothing is sent.
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

# --- 1. Require real RPC + executor (no invention) -------------------------
[ -n "${RPC_HTTP_1:-}" ] || fail "RPC_HTTP_1 is not set (need a real archive RPC URL)."
[ -n "${EXECUTOR_1:-}" ] || fail "EXECUTOR_1 is not set (need a deployed executor address)."

echo "A.4 fork validation @ ${TS}"
echo "  RPC_HTTP_1 = (set, ${#RPC_HTTP_1} chars)"
echo "  EXECUTOR_1 = ${EXECUTOR_1}"
echo "  log        = ${LOG}"

# --- 2. Run the ignored fork test (fork only, no broadcast) ----------------
set +e
cargo test --manifest-path "${REPO_ROOT}/backend/Cargo.toml" \
  --package searcher-rs multistep_fork -- --ignored --nocapture 2>&1 | tee "${LOG}"
STATUS=${PIPESTATUS[0]}
set -e

if [ "${STATUS}" -ne 0 ]; then
  echo "A.4 FAILED (exit ${STATUS}). See ${LOG}. No evidence recorded." >&2
  exit 1
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
