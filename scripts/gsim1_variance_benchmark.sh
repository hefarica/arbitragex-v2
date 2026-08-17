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

# ---- 0. Prerequisites (fail-honest) ----------------------------------------
for var_name in ARBITRAGE_EXECUTOR FLASHLOAN_EXECUTOR_1 ARBX_ADMIN_TOKEN; do
  val=$(grep -E "^${var_name}=" "$DEPLOY_PATH/.env" | cut -d= -f2- || true)
  if [ -z "$val" ]; then
    echo "FATAL: $var_name missing from $DEPLOY_PATH/.env — refusing to run" >&2
    exit 1
  fi
  declare "$var_name=$val"
done

# Single bare mainnet RPC (LazyDb does direct JSON-RPC; the multi-vendor CSV
# form of RPC_HTTP_1 is NOT parsed). Default: the same URL the anvil fork uses.
RPC_URL="${RPC_URL:-$(grep -E '^ANVIL_FORK_URL=' "$DEPLOY_PATH/.env" | cut -d= -f2-)}"
if [ -z "$RPC_URL" ]; then
  echo "FATAL: no bare RPC URL (set RPC_URL or ANVIL_FORK_URL in .env)" >&2
  exit 1
fi

# Live gas price from Redis (the same key sim-ctl's RevmBackend reads).
GAS_PRICE_WEI=$(docker exec "$REDIS_CONTAINER" redis-cli --raw GET arbx:gas_price_wei:1 | tr -d '[:space:]')
if [ -z "$GAS_PRICE_WEI" ] || [ "$GAS_PRICE_WEI" = "(nil)" ]; then
  echo "FATAL: Redis arbx:gas_price_wei:1 empty — gas_oracle_worker not publishing" >&2
  exit 1
fi

# ---- 1. Export real opportunities -------------------------------------------
mkdir -p "$WORK"
docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -At \
  -f "$DEPLOY_PATH/scripts/gsim1_variance_export.sql" > "$INPUT"
ROWS=$(grep -c . "$INPUT" || true)
echo "exported $ROWS candidate rows"
if [ "$ROWS" -lt 100 ]; then
  echo "FATAL: fewer than 100 exportable rows — run during active market hours" >&2
  exit 1
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
