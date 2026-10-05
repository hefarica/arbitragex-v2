#!/usr/bin/env bash
# ============================================================================
# GAS-COVERAGE-01 — paper gas-coverage invariant query (FAIL-HONEST, NO PURGE).
#
# Answers the ONE question the A.6 gas breaker cannot answer from a COUNT(*):
#   "of the runs in this window, which ones were POSSIBLE to measure, and how
#    many of those actually carry a measurement?"
#
# Why it exists (measured in the code, not assumed):
#   · backend/api-server/src/routes/risk-circuit-breakers.ts:593-607 builds the
#     A.6 window as rowsInWindow = COUNT(*) and withActualGas =
#     COUNT(actual_gas_cost_usd). COUNT(*) is every row of the window — including
#     rows the drift-tracker can never select and rows that settled WITHOUT a
#     broadcast (economic/market rejects burn no gas at all, so gas is N/A, not
#     missing). Comparing those two numbers produces a "partial coverage" WARN
#     that no amount of waiting can clear.
#   · backend/recon/src/drift_tracker.rs:452-461 — `compute_gas_cost_usd` ends in
#     `let _ = eth; None` unconditionally: the only writer of
#     actual_gas_cost_usd stores NULL for EVERY resolved row. The numerator is 0
#     by construction until that producer is wired to an ETH-USD price.
#   · database/migrations/126_paper_trade_runs_gas_coverage_ledger.sql adds the
#     per-row state (`gas_measurement_state`) this script audits.
#
# READ-ONLY by default. `--apply` re-derives the ledger state of rows that are
# stale or NULL (no DELETE, no TRUNCATE, no data loss; WHERE ... IS DISTINCT FROM
# <target> so a converged run updates 0 rows). It never reclassifies a genuine
# hole into `not_applicable`.
#
# Modo:
#   bash scripts/paper-gas-coverage-audit.sh                      # audit, 24h, chain 1
#   bash scripts/paper-gas-coverage-audit.sh --chain 1 --hours 24 # explicit
#   bash scripts/paper-gas-coverage-audit.sh --apply              # converge + audit
#   bash scripts/paper-gas-coverage-audit.sh --apply --max-attempts 10
#
# Exit codes (so a cron/readiness probe can gate on it):
#   0 = every possible row measured              (PASS)
#   2 = coverage debt: possible rows unmeasured  (WARN — producer/config issue)
#   3 = impossible rows present                  (operator: upstream defect, not wait)
#   4 = empty window or zero measured rows       (NOT_AVAILABLE — absence, not $0)
#   1 = usage / connection / query error
#
# Env (defaults mirror the runtime knobs — see backend/recon/src/drift_tracker.rs:59-66):
#   PG_CONTAINER (arbitragex-v2-postgres-1), PGUSER, PGDB (arbitragex)
#   ARBX_GAS_COVERAGE_MAX_ATTEMPTS (default: $ARBX_DRIFT_TRACKER_MAX_ATTEMPTS or the
#     runtime default 10 — drift_tracker.rs:64). It is a DECLARED parameter, not a
#     measurement: pass the value your runtime is actually configured with.
# ============================================================================
set -uo pipefail

PG_CONTAINER="${PG_CONTAINER:-arbitragex-v2-postgres-1}"
PGUSER="${PGUSER:-postgres}"
PGDB="${PGDB:-arbitragex}"
CHAIN_ID="1"
WINDOW_HOURS="24"
APPLY=0
MAX_ATTEMPTS="${ARBX_GAS_COVERAGE_MAX_ATTEMPTS:-${ARBX_DRIFT_TRACKER_MAX_ATTEMPTS:-10}}"
MAX_ROWS="${ARBX_GAS_COVERAGE_MAX_ROWS:-50000}"

while [ $# -gt 0 ]; do
  case "$1" in
    --apply)         APPLY=1 ;;
    --chain)         CHAIN_ID="${2:-}"; shift ;;
    --hours)         WINDOW_HOURS="${2:-}"; shift ;;
    --max-attempts)  MAX_ATTEMPTS="${2:-}"; shift ;;
    -h|--help)       sed -n '2,45p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
  shift
done

case "$CHAIN_ID" in ''|*[!0-9]*) echo "[gas-coverage] FATAL: --chain must be an integer (chain_id is part of the identity; there is no global default)" >&2; exit 1 ;; esac
case "$WINDOW_HOURS" in ''|*[!0-9]*) echo "[gas-coverage] FATAL: --hours must be an integer" >&2; exit 1 ;; esac
case "$MAX_ATTEMPTS" in ''|*[!0-9]*) echo "[gas-coverage] FATAL: max attempts must be an integer" >&2; exit 1 ;; esac

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }

# Single round-trip, unaligned, no interactive session; ON_ERROR_STOP makes an SQL
# error return a non-zero status instead of a plausible-looking number.
psql_q() {
  docker exec -i "$PG_CONTAINER" psql -U "$PGUSER" -d "$PGDB" -X -v ON_ERROR_STOP=1 -qAt -F'|' -c "$1"
}
psql_script() {  # multi-statement script on stdin (no implicit txn wrapper around CONCURRENTLY-doctrine DDL)
  docker exec -i "$PG_CONTAINER" psql -U "$PGUSER" -d "$PGDB" -X -v ON_ERROR_STOP=1 -qAt -F'|'
}

# --- 0. Preconditions: the ledger must exist (migration 126) ----------------
have_col="$(psql_q "SELECT COUNT(*) FROM information_schema.columns
                     WHERE table_schema='public' AND table_name='paper_trade_runs'
                       AND column_name='gas_measurement_state';")"
rc=$?
if [ $rc -ne 0 ]; then
  log "[gas-coverage] FATAL: cannot reach PostgreSQL (container=$PG_CONTAINER db=$PGDB user=$PGUSER). No measurement performed."
  exit 1
fi
if [ "${have_col:-0}" != "1" ]; then
  log "[gas-coverage] FATAL: paper_trade_runs.gas_measurement_state is absent."
  log "[gas-coverage]        Apply database/migrations/126_paper_trade_runs_gas_coverage_ledger.sql first."
  exit 1
fi

# `0` is legitimate: no row in the EVIDENCE SCOPE (90d, where the 051..116 columns
# were populated) has reached the terminal attempt count yet. It is not a
# conservative substitute for 0 in production — the bound only matters once the
# ladder has actually terminated rows, and then it is measured.
observed_max="$(psql_q "SELECT COALESCE(max(sim_attempts), 0) FROM paper_trade_runs
                         WHERE created_at >= now() - interval '90 days';")"
if [ "${observed_max:-0}" -gt "${MAX_ATTEMPTS:-0}" ]; then
  log "[gas-coverage] NOTE: runtime knob (max_attempts=$MAX_ATTEMPTS) is below the observed terminal count (max sim_attempts=$observed_max)"
  log "[gas-coverage]       rows above the knob are already terminal; pass --max-attempts $observed_max to classify them as impossible"
fi

# --- 1. Optional convergence (idempotent, no DELETE) ------------------------
if [ "$APPLY" = "1" ]; then
  log "[gas-coverage] --apply: re-deriving stale/NULL ledger rows (chain=$CHAIN_ID hours=$WINDOW_HOURS max_attempts=$MAX_ATTEMPTS limit=$MAX_ROWS)"
  if ! psql_script <<SQL
SET lock_timeout = '5s';
SET statement_timeout = '120s';
WITH target AS (
  SELECT id, gas_measurement_state AS prev,
         CASE
           WHEN actual_gas_cost_usd IS NOT NULL           THEN 'measured'
           WHEN gas_measurement_state = 'measured'        THEN 'unavailable'
           WHEN sim_fail_family IN ('economic','market')  THEN 'not_applicable'
           WHEN calibration_eligible IS FALSE             THEN 'impossible'
           WHEN sim_block_number IS NULL                  THEN 'impossible'
           WHEN opportunity_id IS NULL                    THEN 'impossible'
           WHEN actual_timestamp IS NOT NULL              THEN 'unavailable'
           WHEN sim_attempts >= ${MAX_ATTEMPTS}           THEN 'impossible'
           ELSE 'not_attempted'
         END AS next
    FROM paper_trade_runs
   WHERE chain_id = ${CHAIN_ID}
     AND created_at >= now() - make_interval(hours => ${WINDOW_HOURS})
   ORDER BY created_at DESC
   LIMIT ${MAX_ROWS}
)
UPDATE paper_trade_runs p
   SET gas_measurement_state = t.next,
       gas_measurement_updated_at = now()
  FROM target t
 WHERE p.id = t.id
   AND t.next IS DISTINCT FROM t.prev
RETURNING 1;
SQL
  then
    log "[gas-coverage] FATAL: --apply failed (see psql error above). Ledger NOT converged."
    exit 1
  fi
  log "[gas-coverage] --apply: converge statement done (no rows returned = already converged)."
fi

# --- 2. The coverage ledger for the A.6 window ------------------------------
# `possible` is the honest replacement for COUNT(*): rows that (a) must carry a
# gas measurement — `not_applicable` is excluded because no broadcast happened, so
# there is no gas to observe — and (b) are still resolvable — `impossible` is
# excluded and reported separately so an upstream defect cannot hide inside a
# shrunken denominator.
AGG_SQL="
WITH w AS (
  SELECT * FROM paper_trade_runs
   WHERE chain_id = ${CHAIN_ID}
     AND created_at >= now() - make_interval(hours => ${WINDOW_HOURS})
)
SELECT
  count(*),
  count(actual_gas_cost_usd),
  count(*) FILTER (WHERE gas_measurement_state IN ('measured','not_attempted','unavailable')),
  count(*) FILTER (WHERE gas_measurement_state = 'not_applicable'),
  count(*) FILTER (WHERE gas_measurement_state = 'impossible'),
  count(*) FILTER (WHERE gas_measurement_state = 'unavailable'),
  count(*) FILTER (WHERE gas_measurement_state = 'not_attempted'),
  count(*) FILTER (WHERE gas_measurement_state IS NULL),
  COALESCE(sum(actual_gas_cost_usd), 0)::text
FROM w;"

AGG="$(psql_q "$AGG_SQL")"
if [ $? -ne 0 ] || [ -z "${AGG:-}" ]; then
  log "[gas-coverage] FATAL: coverage query failed — no numbers reported (absence, not zero)."
  exit 1
fi
ROWS="${AGG%%|*}"; rest="${AGG#*|}"
MEASURED="${rest%%|*}"; rest="${rest#*|}"
POSSIBLE="${rest%%|*}"; rest="${rest#*|}"
NA="${rest%%|*}"; rest="${rest#*|}"
IMPOSSIBLE="${rest%%|*}"; rest="${rest#*|}"
UNAVAIL="${rest%%|*}"; rest="${rest#*|}"
NOTATT="${rest%%|*}"; rest="${rest#*|}"
UNCLASSIFIED="${rest%%|*}"; rest="${rest#*|}"
SUM_USD="${rest%%|*}"

# --- 3. Invariant checks (whole table; fail loudly, never adjust a number) ---
VIOLATIONS=0
check() { # check <label> <sql-returning-a-nonzero-row-count>
  local n
  n="$(psql_q "$2")"
  if [ "${n:-ERR}" != "0" ]; then
    log "[gas-coverage] INVARIANT VIOLATION: $1 — count=${n:-<query error>} (must be 0)"
    VIOLATIONS=$((VIOLATIONS+1))
  fi
}
# I3: measured state and value must agree in both directions.
check "measured_without_value (state says measured, value NULL)" \
  "SELECT count(*) FROM paper_trade_runs WHERE gas_measurement_state='measured' AND actual_gas_cost_usd IS NULL;"
check "value_without_measured_state (value present, state is not measured)" \
  "SELECT count(*) FROM paper_trade_runs WHERE actual_gas_cost_usd IS NOT NULL AND gas_measurement_state IS DISTINCT FROM 'measured';"
# I4: a settled row with no classification is a hole in the ledger.
check "resolved_but_unclassified (actual_timestamp set, state NULL)" \
  "SELECT count(*) FROM paper_trade_runs WHERE actual_timestamp IS NOT NULL AND gas_measurement_state IS NULL;"
# I5: not_applicable must be evidence-backed (economic/market family) — never a
#     container for "we could not compute it".
check "not_applicable_without_evidence (no economic/market family)" \
  "SELECT count(*) FROM paper_trade_runs WHERE gas_measurement_state='not_applicable' AND COALESCE(sim_fail_family,'') NOT IN ('economic','market');"
# I6: a row classified impossible cannot also carry a measurement.
check "impossible_but_measured" \
  "SELECT count(*) FROM paper_trade_runs WHERE gas_measurement_state='impossible' AND actual_gas_cost_usd IS NOT NULL;"

# --- 4. Verdict + operator-facing report ------------------------------------
echo "──────────────────────────────────────────────────────────────────────────────"
echo "GAS-COVERAGE-01 — paper_trade_runs actual-gas coverage"
echo "  scope            chain_id=${CHAIN_ID}  window=${WINDOW_HOURS}h  (identity = chain + window; never global)"
echo "  window rows      ${ROWS}"
echo "  measured         ${MEASURED}   (actual_gas_cost_usd IS NOT NULL — real observations)"
echo "  possible         ${POSSIBLE}   (measured + not_attempted + unavailable = honest denominator)"
echo "  not_applicable   ${NA}   (settled without broadcast: no gas burned, nothing to measure)"
echo "  impossible       ${IMPOSSIBLE}   (structurally unresolvable — upstream defect, NOT a wait)"
echo "  unavailable      ${UNAVAIL}   (evaluated, value not computable — BUG SURFACE)"
echo "  not_attempted    ${NOTATT}   (drift scan has not reached them yet)"
echo "  unclassified     ${UNCLASSIFIED}   (gas_measurement_state IS NULL — run --apply)"
echo "  observed gas     \$${SUM_USD} USD over the measured rows only"
echo "  API pair today   measured=${MEASURED} expected=${ROWS}  <-- COUNT(*) denominator; the gap includes ${NA}+${IMPOSSIBLE} rows no measurement can ever fill"
if [ "${POSSIBLE:-0}" -gt 0 ]; then
  echo "  honest coverage  ${MEASURED}/${POSSIBLE} = $(awk -v m="$MEASURED" -v p="$POSSIBLE" 'BEGIN{printf "%.4f", m/p}') (denominator excludes no-broadcast and impossible rows)"
else
  echo "  honest coverage  undefined (possible=0) — absence is not 0%"
fi
if [ "$VIOLATIONS" -gt 0 ]; then
  echo "  invariants       ${VIOLATIONS} VIOLATION(S) — see the log lines above"
else
  echo "  invariants       all passed (I3 value<->state, I4 resolved classified, I5 N/A evidence-backed, I6 impossible unmeasured)"
fi
echo "──────────────────────────────────────────────────────────────────────────────"

if [ "${ROWS:-0}" -eq 0 ]; then
  log "[gas-coverage] verdict=NOT_AVAILABLE (empty window: burn undefined, NOT \$0)"
  exit 4
fi
if [ "${MEASURED:-0}" -eq 0 ]; then
  log "[gas-coverage] verdict=NOT_AVAILABLE (0 measured rows; producer check: recon drift_tracker::compute_gas_cost_usd currently returns None unconditionally)"
  exit 4
fi
if [ "$VIOLATIONS" -gt 0 ]; then
  log "[gas-coverage] verdict=WARN (invariant violations)"
  exit 2
fi
if [ "${UNAVAIL:-0}" -gt 0 ] || [ "${UNCLASSIFIED:-0}" -gt 0 ]; then
  log "[gas-coverage] verdict=WARN (coverage debt: unavailable=${UNAVAIL} unclassified=${UNCLASSIFIED})"
  exit 2
fi
if [ "${IMPOSSIBLE:-0}" -gt 0 ]; then
  log "[gas-coverage] verdict=WARN (${IMPOSSIBLE} impossible row(s) in the window: fix the fixture/route producer, do NOT wait)"
  exit 3
fi
if [ "${POSSIBLE:-0}" -gt 0 ] && [ "${MEASURED:-0}" -eq "${POSSIBLE:-0}" ]; then
  log "[gas-coverage] verdict=PASS (every possible row carries a measurement)"
  exit 0
fi
log "[gas-coverage] verdict=WARN (not_attempted=${NOTATT} still pending the drift scan)"
exit 2
