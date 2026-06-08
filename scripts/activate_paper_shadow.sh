#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Gate C — A.5 paper-shadow activation (READ-ONLY guidance + verification).
#
# Documents the paper-shadow environment and verifies the Gate-C telemetry is
# accumulating, so posterior priors can be calibrated from REAL flow. Requires
# A.4 to have PASSED first (the marker from run_a4_fork_validation.sh).
#
# DOCTRINE (INVIOLABLE): paper / shadow only. NO live trading, NO signer, NO
# broadcast, NO capital. This script does not start services; it prints the
# exact env to run the existing paper-shadow stack and verifies the DB.
#
# Usage:  [DATABASE_URL=...] bash scripts/activate_paper_shadow.sh
# ---------------------------------------------------------------------------
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MARKER="${GATE_C_A4_MARKER:-${REPO_ROOT}/audits/gate-c/A4_FORK_VALIDATION_PASSED}"

fail() { echo "A.5 ABORTED: $*" >&2; exit 1; }

# --- 1. A.4 must have passed first -----------------------------------------
[ -f "${MARKER}" ] || fail "A.4 not passed (missing ${MARKER}). Run scripts/run_a4_fork_validation.sh first."
echo "A.4 marker present: ${MARKER}"
echo

# --- 2. Paper-shadow environment (paper only; never live) ------------------
cat <<'ENV'
Paper-shadow environment (export these for the searcher + api-server, then run
the existing shadow stack — NO live flags, capital = 0):

  export ARBX_PAPER_MODE=true
  export ARBX_PAPER_TRADE=true
  export ARBX_ORCHESTRATOR_MODE=shadow
  export ARBX_PAPER_SHADOW_ENABLED=true
  export ARBX_SCORING_ENABLED=true
  export ARBX_SCORING_HARD_GATE=false
  export ARBX_SCORING_ARCHIVER_MODE=on     # api-server persists scored_opportunities
  export ARBX_PAPER_ARCHIVER_MODE=on       # api-server persists paper_trade_runs
  export ARBX_PAPER_SHADOW_MIN_DAYS=7
ENV
echo

# --- 3. Verify the Gate-C telemetry is accumulating ------------------------
if [ -n "${DATABASE_URL:-}" ] && command -v psql >/dev/null 2>&1; then
  echo "== paper_trade_runs =="
  psql "${DATABASE_URL}" -c "SELECT COUNT(*) AS rows, MAX(created_at) AS last FROM paper_trade_runs;" || true
  echo "== scored_opportunities =="
  psql "${DATABASE_URL}" -c "SELECT COUNT(*) AS rows, AVG(posterior_prob) AS avg_posterior, MIN(created_at) AS first, MAX(created_at) AS last FROM scored_opportunities;" || true
  echo "== bayesian_priors (calibrated when observation_count >= 30) =="
  psql "${DATABASE_URL}" -c "SELECT COUNT(*) AS pairs, COUNT(*) FILTER (WHERE observation_count >= 30) AS calibrated FROM bayesian_priors;" || true
else
  echo "DATABASE_URL not set (or psql unavailable) — skipping live verification."
  echo "Run these on the VPS DB to track warming progress."
fi
echo

# --- 4. Day-7 calibration review query -------------------------------------
cat <<'SQL'
Day-7 calibration review (run against the VPS DB after >=7 days of shadow):

  SELECT * FROM gate_c_metrics ORDER BY total_scored DESC LIMIT 20;

A.5 reaches CALIBRATED only once bayesian_priors holds per-pair priors with
observation_count >= ARBX_CALIBRATION_MIN_OBSERVATIONS (30). Until then the
dashboard honestly reports PAPER_SHADOW_WARMING / CALIBRATED_CANDIDATE.
SQL

echo "A.5 guidance complete (no services started; paper/shadow only)."
