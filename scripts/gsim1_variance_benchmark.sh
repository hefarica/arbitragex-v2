#!/usr/bin/env bash
# G-SIM-1 item 4 (variance_benchmark) — operator macro. RUN ON THE VPS.
#
#   bash scripts/gsim1_variance_benchmark.sh
#
# Steps (all real, zero fabrication):
#   1. Export REAL opportunities from the production DB
#      (scripts/gsim1_variance_export.sql) → /tmp/gsim1/input.jsonl.
#   2. Run the #[ignore]d replay harness inside a rust:1.91 container
#      (the VPS has no host toolchain; the repo is mounted read-only):
#        cargo test -p sim-core --test variance_benchmark -- --ignored
#      PREDICTED = production multi-step REVM at block B (detection block,
#      resolved by timestamp bisection); OBSERVED = same at B+1.
#   3. Parse VARIANCE_BENCH_OUTCOME + VARIANCE_BENCH_JSON markers.
#   4. POST evidenced|failed to the readiness_evidence registry
#      (api-server binds 127.0.0.1:8080; the admin token is read from the
#      deployment .env and never leaves this host).
#
# Env overrides: DEPLOY_PATH, PG_CONTAINER, REDIS_CONTAINER, RPC_URL,
# API_CONTAINER_PREFIX. Fails loudly on ANY missing prerequisite — a hollow
# benchmark must never reach the registry.

set -euo pipefail

DEPLOY_PATH="${DEPLOY_PATH:-/opt/arbitragex-v2}"
PG_CONTAINER="${PG_CONTAINER:-arbitragex-v2-postgres-1}"
REDIS_CONTAINER="${REDIS_CONTAINER:-arbitragex-v2-redis-1}"
API_BASE="${API_BASE:-http://127.0.0.1:8080}"
WORK=/tmp/gsim1
INPUT="$WORK/input.jsonl"
LOG="$WORK/harness.log"

# ---- 0. Prerequisites (fail-honest, recorded) --------------------------------
# A missing prerequisite is a REAL benchmark outcome: the row is recorded as
# status=failed with the exact blocker (RULE 00/R8 — never silently skip, never
# fabricate a pass). The export still runs so the row carries the real number of
# available candidate rows at attempt time.
MISSING_PREREQS=()
for var_name in ARBITRAGE_EXECUTOR FLASHLOAN_EXECUTOR_1 ARBX_ADMIN_TOKEN; do
  val=$(grep -E "^${var_name}=" "$DEPLOY_PATH/.env" | cut -d= -f2- || true)
  if [ -z "$val" ]; then
    MISSING_PREREQS+=("$var_name")
  else
    declare "$var_name=$val"
  fi
done

record_failed_prereq() {
  local why="$1" rows="${2:-0}"
  curl --fail-with-body -sS --max-time 20 -X POST \
    -H "Content-Type: application/json" \
    -H "x-arbx-admin-token: ${ARBX_ADMIN_TOKEN}" \
    --data-binary @- \
    "${API_BASE}/admin/readiness-evidence" <<JSON
{
  "gate_id": "G-SIM-1",
  "item_key": "variance_benchmark",
  "status": "failed",
  "evidence_ref": "harness $(date -u +%Y-%m-%dT%H:%M:%SZ) host=$(hostname) attempt aborted: ${why}",
  "detail": {
    "method": "revm_b_vs_revm_b1_fork",
    "error": "${why}",
    "candidate_rows_available": ${rows},
    "note": "harness + driver ready (sim-core/tests/variance_benchmark.rs); item stays pending until the operator prerequisite lands (deployed FLE+AE executor stack — see docs/operations/SIMULATOR_V2_READINESS.md and searcher-rs/tests/multistep_fork.rs M5 note)"
  },
  "verified_by": "operator:gsim1-variance-harness"
}
JSON
  echo "" >&2
  echo "FATAL: ${why}" >&2
  exit 1
}

# Single bare mainnet RPC (LazyDb does direct JSON-RPC; the multi-vendor CSV
# form of RPC_HTTP_1 is NOT parsed). Default: the same URL the anvil fork uses.
RPC_URL="${RPC_URL:-$(grep -E '^ANVIL_FORK_URL=' "$DEPLOY_PATH/.env" | cut -d= -f2-)}"
if [ -z "$RPC_URL" ]; then
  MISSING_PREREQS+=("RPC_URL(ANVIL_FORK_URL)")
fi

# Live gas price from Redis (the same key sim-ctl's RevmBackend reads).
GAS_PRICE_WEI=$(docker exec "$REDIS_CONTAINER" redis-cli --raw GET arbx:gas_price_wei:1 | tr -d '[:space:]')
if [ -z "$GAS_PRICE_WEI" ] || [ "$GAS_PRICE_WEI" = "(nil)" ]; then
  MISSING_PREREQS+=("GAS_PRICE_WEI(arbx:gas_price_wei:1)")
fi

# ---- 1. Export real opportunities (always — the row count is real evidence) --
# NOTE: the SQL file lives on the HOST; the postgres container does not mount
# the repo — pipe the script via stdin (`-f` would resolve inside the container).
mkdir -p "$WORK"
docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -At \
  < "$DEPLOY_PATH/scripts/gsim1_variance_export.sql" > "$INPUT"
ROWS=$(grep -c . "$INPUT" || true)
echo "exported $ROWS candidate rows"

if [ "${#MISSING_PREREQS[@]}" -gt 0 ]; then
  record_failed_prereq "missing prerequisites: ${MISSING_PREREQS[*]}" "$ROWS"
fi
if [ "$ROWS" -lt 100 ]; then
  record_failed_prereq "fewer than 100 exportable rows (${ROWS}) — run during active market hours" "$ROWS"
fi

# ---- 2. Run the harness in a pinned rust container ---------------------------
cd "$DEPLOY_PATH"
docker run --rm \
  -v "$DEPLOY_PATH:/src" -v "$WORK:/work" \
  -w /src/backend \
  -e RPC_HTTP_1="$RPC_URL" \
  -e ARBITRAGE_EXECUTOR="$ARBITRAGE_EXECUTOR" \
  -e FLASHLOAN_EXECUTOR_1="$FLASHLOAN_EXECUTOR_1" \
  -e GAS_PRICE_WEI="$GAS_PRICE_WEI" \
  -e VARIANCE_INPUT=/work/input.jsonl \
  rust:1.91 \
  cargo test -p sim-core --test variance_benchmark --locked -- --ignored --nocapture \
  2>&1 | tee "$LOG"

# ---- 3. Parse the markers -----------------------------------------------------
MARKER=$(grep -Eo 'VARIANCE_BENCH_OUTCOME=(PASS|FAIL)' "$LOG" | tail -n 1 || true)
JSON_DETAIL=$(grep -Eo 'VARIANCE_BENCH_JSON=\{.*\}' "$LOG" | tail -n 1 | sed 's/^VARIANCE_BENCH_JSON=//' || true)
if [ -z "$MARKER" ]; then
  echo "FATAL: harness produced no VARIANCE_BENCH_OUTCOME marker — nothing recorded" >&2
  exit 1
fi
OUTCOME="${MARKER#VARIANCE_BENCH_OUTCOME=}"
if [ -z "$JSON_DETAIL" ]; then
  echo "FATAL: harness produced no VARIANCE_BENCH_JSON detail line — nothing recorded" >&2
  exit 1
fi
STATUS=$([ "$OUTCOME" = "PASS" ] && echo evidenced || echo failed)
echo "benchmark outcome: $OUTCOME → registry status: $STATUS"

# ---- 4. Record the evidence (append-only registry) ----------------------------
RUN_REF="harness $(date -u +%Y-%m-%dT%H:%M:%SZ) host=$(hostname) rows=$ROWS gas_wei=$GAS_PRICE_WEI rpc=${RPC_URL}"
jq -n \
  --arg gate_id "G-SIM-1" \
  --arg item_key "variance_benchmark" \
  --arg status "$STATUS" \
  --arg evidence_ref "$RUN_REF" \
  --argjson detail "$JSON_DETAIL" \
  --arg verified_by "operator:gsim1-variance-harness" \
  '{gate_id: $gate_id, item_key: $item_key, status: $status,
    evidence_ref: $evidence_ref, detail: $detail,
    verified_by: $verified_by}' > "$WORK/evidence-payload.json"

curl --fail-with-body -sS --max-time 20 -X POST \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: ${ARBX_ADMIN_TOKEN}" \
  --data-binary @"$WORK/evidence-payload.json" \
  "${API_BASE}/admin/readiness-evidence"
echo
echo "full harness log: $LOG"
