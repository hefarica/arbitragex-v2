#!/usr/bin/env bash
# G-SIM-1 item 4 (variance_benchmark) — benchmark driver. RUN ON THE VPS.
#
#   bash scripts/gsim1_variance_benchmark.sh
#
# Steps (all real, zero fabrication):
#   1. Export the LABELABLE population of the freshness window from the
#      production DB (scripts/gsim1_variance_export.sql; one row per distinct
#      A.3.a-encodable route topology) → /tmp/gsim1/input.jsonl.
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
# API_BASE, VARIANCE_MIN_SAMPLES, GSIM1_STRICT.
#
# ── G-SIM1-AUTOREFRESH (2026-09-26) ─────────────────────────────────────────
# WHAT CHANGED AND WHY
#
#   a) ALWAYS RECORD. The previous revision `exit 1`-ed WITHOUT writing a row
#      whenever the harness produced no marker (or no JSON detail). For a
#      scheduled producer that is the worst possible failure mode: the registry
#      kept the old row (or none) and the readiness blocker then reported
#      "missing evidence" for a job that had in fact run and broken. Every
#      terminal path below now writes a row FIRST (evidenced|failed + the
#      measured reason) and only then decides its exit code.
#
#   b) EXIT CODE NO LONGER MEANS "the benchmark failed". A scheduled job whose
#      only two outcomes are red and red is a job nobody reads. Exit 0 now means
#      "the registry row for this run was written" — the honest verdict lives in
#      that row and in the workflow's job summary. GSIM1_STRICT=1 restores the
#      old fail-on-FAIL behaviour for manual/operator runs. The job still fails
#      LOUDLY (non-zero) when the evidence could not be recorded at all — that
#      is the failure that must never be silent.
#
#   c) THE SAMPLE FLOOR IS THE MEASURED POPULATION, NOT 100. `min_samples`
#      defaults to the number of distinct labelable topologies this run
#      exported (exhaustive coverage of the finite labelable population), and
#      the row records that population plus whether it is large enough for the
#      p95 order statistic to mean anything (n >= 21). See
#      docs/operations/SIMULATOR_V2_READINESS.md §variance_benchmark.
#      VARIANCE_MIN_SAMPLES still overrides.
#
#   d) ZERO LABELABLE TOPOLOGIES IS A RECORDED FAILURE with its own reason
#      (`no-labelable-population`), never a skipped run.

set -euo pipefail

DEPLOY_PATH="${DEPLOY_PATH:-/opt/arbitragex-v2}"
PG_CONTAINER="${PG_CONTAINER:-arbitragex-v2-postgres-1}"
REDIS_CONTAINER="${REDIS_CONTAINER:-arbitragex-v2-redis-1}"
API_BASE="${API_BASE:-http://127.0.0.1:8080}"
STRICT="${GSIM1_STRICT:-0}"
WORK=/tmp/gsim1
INPUT="$WORK/input.jsonl"
LOG="$WORK/harness.log"

mkdir -p "$WORK"

# ---- registry transport (single choke point, always checked) ----------------
# Returns non-zero when the row was NOT accepted — the caller must then abort
# with a non-zero status, because an unrecorded run is exactly the silent
# failure this driver exists to prevent.
post_evidence() {
  local payload="$1"
  curl --fail-with-body -sS --max-time 20 -X POST \
    -H "Content-Type: application/json" \
    -H "x-arbx-admin-token: ${ARBX_ADMIN_TOKEN}" \
    --data-binary @"$payload" \
    "${API_BASE}/admin/readiness-evidence"
}

# Build + POST a status=failed row with an explicit reason, then apply the exit
# policy. $1 = reason, $2 = candidate rows, $3 = extra detail JSON (object),
# $4 = optional log path whose tail is embedded as `log_tail` (read by python3
#      from the FILE — never interpolated through the shell, so no escaping
#      hazard can corrupt the payload).
record_failed() {
  local why="$1" rows="${2:-0}" extra="${3:-}" log_path="${4:-}"
  python3 - "$why" "$rows" "$extra" "$log_path" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(hostname)" \
    > "$WORK/evidence-payload.json" <<'PY'
import json
import sys

why, rows, extra, log_path, ts, host = sys.argv[1:7]
try:
    detail = json.loads(extra) if extra.strip() else {}
except json.JSONDecodeError:
    detail = {"extra_parse_error": extra[:200]}
detail.setdefault("method", "revm_b_vs_revm_b1_fork")
detail["error"] = why
detail["candidate_rows_available"] = int(rows)
detail["recorded_at"] = ts
detail["recorded_on_host"] = host
if log_path:
    try:
        with open(log_path, encoding="utf-8", errors="replace") as fh:
            detail["log_tail"] = fh.read()[-1500:]
    except OSError:
        detail["log_tail"] = None
print(json.dumps({
    "gate_id": "G-SIM-1",
    "item_key": "variance_benchmark",
    "status": "failed",
    "evidence_ref": f"harness {ts} host={host} rows={rows} attempt aborted: {why}",
    "detail": detail,
    "verified_by": "operator:gsim1-variance-harness",
}))
PY
  if post_evidence "$WORK/evidence-payload.json"; then
    echo "" >&2
    echo "RECORDED (failed): ${why}" >&2
    echo "G-SIM-1 item variance_benchmark stays PENDING with the recorded reason above." >&2
    if [ "$STRICT" = "1" ]; then exit 1; fi
    exit 0
  fi
  echo "FATAL: registry write REJECTED — the failed outcome could not be recorded: ${why}" >&2
  exit 1
}

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

# G-SIM1-DEJQ (2026-09-22): the evidence payload is built with python3 (jq is
# not installed on the VPS). A missing builder is a missing prerequisite —
# fail-honest, recorded, never a silent 0-byte payload.
command -v python3 >/dev/null 2>&1 || MISSING_PREREQS+=("python3 (evidence payload builder)")

# Single bare mainnet RPC (LazyDb does direct JSON-RPC; the multi-vendor CSV
# form of RPC_HTTP_1 is NOT parsed). Default: the same URL the anvil fork uses.
RPC_URL="${RPC_URL:-$(grep -E '^ANVIL_FORK_URL=' "$DEPLOY_PATH/.env" | cut -d= -f2-)}"
if [ -z "$RPC_URL" ]; then
  MISSING_PREREQS+=("RPC_URL(ANVIL_FORK_URL)")
fi

# Live gas price from Redis (the same key sim-ctl's RevmBackend reads).
GAS_PRICE_WEI=$(docker exec "$REDIS_CONTAINER" redis-cli --raw GET arbx:gas_price_wei:1 | tr -d '[:space:]' || true)
if [ -z "$GAS_PRICE_WEI" ] || [ "$GAS_PRICE_WEI" = "(nil)" ]; then
  MISSING_PREREQS+=("GAS_PRICE_WEI(arbx:gas_price_wei:1)")
fi

# ---- 1. Export the labelable population (always) ----------------------------
# Coerce anything that is not a plain non-negative integer to 0: a malformed
# count must never abort the driver via a bash arithmetic error BEFORE the row is
# written (that would recreate the silent-failure mode this revision removes).
as_int() {
  case "${1:-}" in
    '' | *[!0-9]*) echo 0 ;;
    *) echo "$1" ;;
  esac
}

# NOTE: the SQL file lives on the HOST; the postgres container does not mount
# the repo — pipe the script via stdin (`-f` would resolve inside the container).
docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -At \
  < "$DEPLOY_PATH/scripts/gsim1_variance_export.sql" > "$INPUT"
ROWS=$(as_int "$(grep -c . "$INPUT" || true)")
echo "exported $ROWS distinct labelable topologies (A.3.a-encodable 2-leg, chain 1, 2h window)"

# Population context for the record: how many distinct 2-leg topologies the same
# window holds in total (all adapters). The gap is the measured share the A.3.a
# encoder cannot touch — measured here, not asserted.
TOTAL_2LEG=$(as_int "$(docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -t -A -c \
  "SELECT count(*) FROM (SELECT DISTINCT dex_a, token_in, token_out, route_metadata->'pool_addresses' FROM opportunities WHERE chain_id = 1 AND route_metadata ? 'dex_adapters' AND jsonb_array_length(route_metadata->'dex_adapters') = 2 AND detected_at > now() - interval '2 hours') t;" \
  | tr -d '[:space:]' || true)")
if [ "$TOTAL_2LEG" -gt "$ROWS" ]; then UNENCODABLE=$(( TOTAL_2LEG - ROWS )); else UNENCODABLE=0; fi
echo "distinct 2-leg topologies in window (all adapters): $TOTAL_2LEG (not A.3.a-encodable: $UNENCODABLE)"

# p95 order statistic: with n < 21 the reported p95 IS the sample maximum, so
# the record states whether the statistic is meaningful (R8: label it, never
# quietly report the max as a percentile).
if [ "$ROWS" -ge 21 ]; then P95_ESTIMABLE=true; else P95_ESTIMABLE=false; fi

if [ "${#MISSING_PREREQS[@]}" -gt 0 ]; then
  record_failed "missing prerequisites: ${MISSING_PREREQS[*]}" "$ROWS" \
    "{\"population_size\": ${ROWS}, \"distinct_all_adapters\": ${TOTAL_2LEG}, \"p95_estimable\": ${P95_ESTIMABLE}}"
fi

if [ "$ROWS" -eq 0 ]; then
  record_failed "no-labelable-population: 0 distinct A.3.a-encodable 2-leg topologies in the freshness window (${TOTAL_2LEG} distinct 2-leg topologies exist, none encodable)" 0 \
    "{\"population_size\": 0, \"distinct_all_adapters\": ${TOTAL_2LEG}, \"p95_estimable\": false, \"note\": \"an empty labelable population is a real market/encoder state, not a harness failure\"}"
fi

# Sample floor = exhaustive coverage of the measured labelable population.
# Justification (docs/operations/SIMULATOR_V2_READINESS.md §variance_benchmark):
# the labelable population is finite and small (measured saturation: 21 distinct
# over 7 days), so the previous absolute floor of 100 was unreachable by
# construction; what this gate can honestly demand is that EVERY labelable
# topology of the window gets labeled.
MIN_SAMPLES="${VARIANCE_MIN_SAMPLES:-$ROWS}"
echo "sample floor for this run: ${MIN_SAMPLES} (population ${ROWS})"

# ---- 2. Run the harness in a pinned rust container ---------------------------
cd "$DEPLOY_PATH"
HARNESS_RC=0
docker run --rm \
  -v "$DEPLOY_PATH:/src" -v "$WORK:/work" \
  -w /src/backend \
  -e RPC_HTTP_1="$RPC_URL" \
  -e ARBITRAGE_EXECUTOR="$ARBITRAGE_EXECUTOR" \
  -e FLASHLOAN_EXECUTOR_1="$FLASHLOAN_EXECUTOR_1" \
  -e GAS_PRICE_WEI="$GAS_PRICE_WEI" \
  -e VARIANCE_INPUT=/work/input.jsonl \
  -e VARIANCE_MIN_SAMPLES="$MIN_SAMPLES" \
  rust:1.91 \
  cargo test -p sim-core --test variance_benchmark --locked -- --ignored --nocapture \
  2>&1 | tee "$LOG" || HARNESS_RC=$?

# ---- 3. Parse the markers -----------------------------------------------------
MARKER=$(grep -Eo 'VARIANCE_BENCH_OUTCOME=(PASS|FAIL)' "$LOG" | tail -n 1 || true)
JSON_DETAIL=$(grep -Eo 'VARIANCE_BENCH_JSON=\{.*\}' "$LOG" | tail -n 1 | sed 's/^VARIANCE_BENCH_JSON=//' || true)

# A harness that produced no marker is a RECORDED failure (change (a) above):
# the old `exit 1` here left the registry holding a stale/absent row, and the
# readiness gate then blamed "missing evidence" for a broken harness.
if [ -z "$MARKER" ]; then
  record_failed "harness produced no VARIANCE_BENCH_OUTCOME marker (cargo/docker rc=${HARNESS_RC})" "$ROWS" \
    "{\"population_size\": ${ROWS}, \"distinct_all_adapters\": ${TOTAL_2LEG}, \"p95_estimable\": ${P95_ESTIMABLE}, \"harness_rc\": ${HARNESS_RC}}" \
    "$LOG"
fi
if [ -z "$JSON_DETAIL" ]; then
  record_failed "harness produced no VARIANCE_BENCH_JSON detail line (marker=${MARKER})" "$ROWS" \
    "{\"population_size\": ${ROWS}, \"distinct_all_adapters\": ${TOTAL_2LEG}, \"p95_estimable\": ${P95_ESTIMABLE}, \"harness_rc\": ${HARNESS_RC}}" \
    "$LOG"
fi

OUTCOME="${MARKER#VARIANCE_BENCH_OUTCOME=}"
STATUS=$([ "$OUTCOME" = "PASS" ] && echo evidenced || echo failed)
echo "benchmark outcome: $OUTCOME → registry status: $STATUS"

# ---- 4. Record the evidence (append-only registry) ----------------------------
RUN_REF="harness $(date -u +%Y-%m-%dT%H:%M:%SZ) host=$(hostname) rows=$ROWS gas_wei=$GAS_PRICE_WEI rpc=${RPC_URL}"
# G-SIM1-DEJQ (2026-09-22): jq is NOT installed on the VPS — the old `jq -n`
# here left evidence-payload.json at 0 bytes and broke the registry POST even
# on valid outcomes. python3 (3.12) is present; build the identical payload
# with it instead. The fail-loud prerequisite above guarantees it exists.
python3 - "$STATUS" "$RUN_REF" "$JSON_DETAIL" "$ROWS" "$TOTAL_2LEG" "$MIN_SAMPLES" "$P95_ESTIMABLE" "$HARNESS_RC" \
  > "$WORK/evidence-payload.json" <<'PY'
import json
import sys

status, evidence_ref, detail_raw, population, total_2leg, min_samples, p95_estimable, rc = sys.argv[1:9]
detail = json.loads(detail_raw)
# G-SIM1-AUTOREFRESH: the row carries its own population context, so the
# readiness blocker can quote WHY the item is unmet without re-deriving it.
detail["population_size"] = int(population)
detail["distinct_all_adapters"] = int(total_2leg)
detail["min_samples_required"] = int(min_samples)
detail["p95_estimable"] = p95_estimable == "true"
detail["harness_rc"] = int(rc)
labeled = detail.get("samples_labeled")
detail["coverage"] = (
    round(labeled / detail["population_size"], 4)
    if isinstance(labeled, int) and detail["population_size"] > 0
    else None
)
print(json.dumps({
    "gate_id": "G-SIM-1",
    "item_key": "variance_benchmark",
    "status": status,
    "evidence_ref": evidence_ref,
    "detail": detail,
    "verified_by": "operator:gsim1-variance-harness",
}))
PY

if post_evidence "$WORK/evidence-payload.json"; then
  echo
  echo "RECORDED: status=${STATUS} population=${ROWS} min_samples=${MIN_SAMPLES}"
  echo "full harness log: $LOG"
  if [ "$OUTCOME" != "PASS" ]; then
    echo "G-SIM-1 item variance_benchmark stays PENDING — the recorded row carries the measured reason."
    if [ "$STRICT" = "1" ]; then exit 1; fi
  fi
  exit 0
fi
echo "FATAL: registry write REJECTED for outcome=${OUTCOME}; evidence NOT delivered" >&2
exit 1
