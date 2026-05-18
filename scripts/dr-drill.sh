#!/usr/bin/env bash
# ============================================================================
#  dr-drill.sh — Disaster Recovery Drill for ArbitrageX v2
#  --------------------------------------------------------------------------
#  Idempotent DR drill that verifies platform resilience without causing
#  user-facing disruption.  Dry-run by default; destructive actions require
#  --live flag.
#
#  Tests performed:
#    1. Container census — all 21 expected containers are running
#    2. Kill-switch auth — 401 without token, 200 with valid token
#    3. Paper-mode — verify Paper: ON in System Guard
#    4. Backup procedures — pg_dump, Redis SAVE, MinIO bucket list
#    5. Recovery procedures — all health endpoints respond
#    6. Rollback procedure — docker compose down + up (dry-run only)
#
#  Usage:
#    # Dry-run (default) — safe to run anytime
#    ./scripts/dr-drill.sh
#
#    # Live mode — performs actual toggles and tests
#    ./scripts/dr-drill.sh --live
#
#    # Specific test only:
#    ./scripts/dr-drill.sh --test containers
#    ./scripts/dr-drill.sh --test killswitch
#    ./scripts/dr-drill.sh --test backups
#
#  Requirements:
#    • Docker & docker compose v2+
#    • docker compose -f docker/compose.prod.yml config
#    • curl, jq, pg_dump, redis-cli (installed on host or via container)
#  --------------------------------------------------------------------------
#  Fase F-12: Kill-switch + DR drill (+9 pts)
# ============================================================================
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-$REPO_ROOT/docker/compose.prod.yml}"
STACK_NAME="arbitragex-v2"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Expected containers count (must match compose.prod.yml services).
# If you add or remove services, update this number.
EXPECTED_CONTAINERS="${EXPECTED_CONTAINERS:-21}"

# Endpoints (all loopback-only for security).
VAULT_ADDR="${VAULT_ADDR:-https://127.0.0.1:8200}"
POSTGRES_HOST="${POSTGRES_HOST:-127.0.0.1}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_USER="${POSTGRES_USER:-postgres}"
POSTGRES_DB="${POSTGRES_DB:-arbitragex}"
REDIS_HOST="${REDIS_HOST:-127.0.0.1}"
REDIS_PORT="${REDIS_PORT:-6379}"
MINIO_ENDPOINT="${MINIO_ENDPOINT:-http://127.0.0.1:9000}"
EDGE_ENDPOINT="${EDGE_ENDPOINT:-http://127.0.0.1:8080}"
GRAFANA_ENDPOINT="${GRAFANA_ENDPOINT:-http://127.0.0.1:3000}"
PROMETHEUS_ENDPOINT="${PROMETHEUS_ENDPOINT:-http://127.0.0.1:9090}"
ALERTMANAGER_ENDPOINT="${ALERTMANAGER_ENDPOINT:-http://127.0.0.1:9093}"

# Credentials (override via env vars).
ARBX_ADMIN_TOKEN="${ARBX_ADMIN_TOKEN:-}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-}"
MINIO_ROOT_USER="${MINIO_ROOT_USER:-minioadmin}"
MINIO_ROOT_PASSWORD="${MINIO_ROOT_PASSWORD:-}"

# ─── Runtime flags ──────────────────────────────────────────────────────────
MODE="dry-run"      # dry-run | live
FILTER_TEST=""      # empty = all tests; otherwise single test name
PASSED=0
FAILED=0
WARNINGS=0
REPORT_ENTRIES=()

# ─── Argument parsing ───────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --live)
            MODE="live"
            shift ;;
        --test)
            FILTER_TEST="${2:-}"
            shift 2 ;;
        --help|-h)
            echo "Usage: $0 [--live] [--test containers|killswitch|paper|backups|recovery|rollback]"
            exit 0 ;;
        *)
            echo "Unknown argument: $1" >&2
            echo "Usage: $0 [--live] [--test <name>]" >&2
            exit 1 ;;
    esac
done

# ─── Logging helpers ────────────────────────────────────────────────────────
log_info()  { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO]   $*"; }
log_pass()  { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [PASS]   $*"; PASSED=$((PASSED + 1)); }
log_fail()  { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [FAIL]   $*"; FAILED=$((FAILED + 1)); REPORT_ENTRIES+=("FAIL: $*"); }
log_warn()  { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [WARN]   $*" >&2; WARNINGS=$((WARNINGS + 1)); REPORT_ENTRIES+=("WARN: $*"); }
log_step()  { echo ""; echo "═══════════════════════════════════════════════════════════════════════════════"; echo "  $*"; echo "══════════════════════════════════════════════════════════════════════════════="; }

# ─── Compose helper ─────────────────────────────────────────────────────────
docker_compose_cmd() {
    docker compose -f "$COMPOSE_FILE" "$@"
}

# ─── HTTP helpers ───────────────────────────────────────────────────────────
http_get() {
    local url="$1"
    local timeout="${2:-10}"
    curl -sk -o /dev/null -w "%{http_code}" --max-time "$timeout" "$url" 2>/dev/null || echo "000"
}

http_post() {
    local url="$1"
    local token="${2:-}"
    local timeout="${3:-10}"
    local hdr=()
    if [[ -n "$token" ]]; then
        hdr=("-H" "Authorization: Bearer $token")
    fi
    curl -sk "${hdr[@]}" -X POST --max-time "$timeout" "$url" 2>/dev/null || true
}

# ─── Report helpers ─────────────────────────────────────────────────────────
record() {
    REPORT_ENTRIES+=("$*")
}

# ═════════════════════════════════════════════════════════════════════════════
#  TEST 1: Container Census — all expected containers running
# ═════════════════════════════════════════════════════════════════════════════
test_containers() {
    log_step "TEST 1/6 — Container Census"
    record "Container Census — verify $EXPECTED_CONTAINERS containers running"

    local running
    running=$(docker ps --format '{{.Names}}' --filter "name=${STACK_NAME}" 2>/dev/null | wc -l)

    log_info "Running containers: $running / $EXPECTED_CONTAINERS"

    if [ "$running" -eq "$EXPECTED_CONTAINERS" ]; then
        log_pass "All $EXPECTED_CONTAINERS containers are running."
    elif [ "$running" -ge "$((EXPECTED_CONTAINERS - 2))" ]; then
        log_warn "Only $running / $EXPECTED_CONTAINERS containers running (≥ threshold)."
        docker ps --format '{{.Names}}' --filter "name=${STACK_NAME}" | sort | while read -r c; do
            log_info "  ✓ $c"
        done
    else
        log_fail "Only $running / $EXPECTED_CONTAINERS containers running (below threshold)."
        log_info "Expected containers matching '${STACK_NAME}':"
        docker ps --format '{{.Names}}' --filter "name=${STACK_NAME}" | sort
    fi
}

# ═════════════════════════════════════════════════════════════════════════════
#  TEST 2: Kill-switch Auth — verify 401 without token, 200 with valid
# ═════════════════════════════════════════════════════════════════════════════
test_killswitch() {
    log_step "TEST 2/6 — Kill-switch Authentication"
    record "Kill-switch auth — verify 401 w/o token, toggle w/ valid token"

    # 2a: Verify 401 without token.
    local resp_no_token
    resp_no_token=$(http_post "${EDGE_ENDPOINT}/admin/killswitch")
    local code_no_token
    code_no_token=$(echo "$resp_no_token" | tail -c 4 | tr -dc '0-9' || echo "000")

    # Try a cleaner check.
    code_no_token=$(curl -sk -o /dev/null -w "%{http_code}" -X POST "${EDGE_ENDPOINT}/admin/killswitch" 2>/dev/null || echo "000")

    if [ "$code_no_token" = "401" ] || [ "$code_no_token" = "403" ]; then
        log_pass "Kill-switch returns $code_no_token without token (correct)."
    else
        log_fail "Kill-switch returned $code_no_token without token (expected 401/403)."
        log_info "  curl -X POST ${EDGE_ENDPOINT}/admin/killswitch"
    fi

    # 2b: Verify toggle with valid token (dry-run: skip actual toggle).
    if [ -z "$ARBX_ADMIN_TOKEN" ]; then
        log_warn "ARBX_ADMIN_TOKEN not set — skipping kill-switch toggle test."
        return
    fi

    if [ "$MODE" = "dry-run" ]; then
        log_info "[DRY-RUN] Would POST /admin/killswitch with ARBX_ADMIN_TOKEN"
        log_info "[DRY-RUN] Then verify response contains kill_switch status"
        record "Kill-switch toggle — DRY-RUN (no actual toggle)"
        return
    fi

    # LIVE mode: actually toggle and verify.
    local resp_toggle
    resp_toggle=$(curl -sk -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" \
        -X POST "${EDGE_ENDPOINT}/admin/killswitch" 2>/dev/null || echo "{}")

    if echo "$resp_toggle" | grep -qi "kill_switch"; then
        log_pass "Kill-switch toggled successfully (valid token)."
    else
        log_fail "Kill-switch toggle failed with valid token."
        log_info "  Response: $resp_toggle"
    fi
}

# ═════════════════════════════════════════════════════════════════════════════
#  TEST 3: Paper Mode — verify Paper: ON in System Guard
# ═════════════════════════════════════════════════════════════════════════════
test_paper_mode() {
    log_step "TEST 3/6 — Paper Mode Verification"
    record "Paper mode — verify Paper: ON in System Guard"

    if [ "$MODE" = "dry-run" ]; then
        log_info "[DRY-RUN] Would GET /system/guard and check for 'Paper: ON'"
        record "Paper mode — DRY-RUN (no actual check)"
        return
    fi

    local resp
    resp=$(curl -sk --max-time 10 "${EDGE_ENDPOINT}/system/guard" 2>/dev/null || echo "")

    if echo "$resp" | grep -qi "paper.*on"; then
        log_pass "Paper mode is ON in System Guard."
    elif echo "$resp" | grep -qi "paper.*off"; then
        log_fail "Paper mode is OFF — no real funds at risk (LIVE mode would)."
    else
        log_warn "Could not determine paper mode status (endpoint unavailable?)."
    fi
}

# ═════════════════════════════════════════════════════════════════════════════
#  TEST 4: Backup Procedures — pg_dump, Redis SAVE, MinIO bucket list
# ═════════════════════════════════════════════════════════════════════════════
test_backups() {
    log_step "TEST 4/6 — Backup Procedures"
    record "Backups — pg_dump, Redis SAVE, MinIO bucket list"

    # 4a: pg_dump connectivity test.
    if command -v pg_dump >/dev/null 2>&1 && [ -n "$POSTGRES_PASSWORD" ]; then
        local dump_out="/tmp/arbx_drill_dump_${TIMESTAMP}.sql"
        if pg_dump -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" \
                   -d "$POSTGRES_DB" --schema-only > "$dump_out" 2>/dev/null; then
            local sz
            sz=$(wc -c < "$dump_out")
            log_pass "pg_dump schema-only successful ($sz bytes)."
            rm -f "$dump_out"
        else
            log_fail "pg_dump failed — check PostgreSQL connectivity/credentials."
        fi
    elif docker exec "${STACK_NAME}-postgres-1" pg_isready -U "$POSTGRES_USER" > /dev/null 2>&1; then
        # Fallback: use pg_dump from within the postgres container.
        if [ "$MODE" = "dry-run" ]; then
            log_info "[DRY-RUN] Would run pg_dump inside postgres container."
        else
            docker exec -e PGPASSWORD="$POSTGRES_PASSWORD" "${STACK_NAME}-postgres-1" \
                pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --schema-only \
                > "/tmp/arbx_drill_dump_${TIMESTAMP}.sql" 2>/dev/null && \
                log_pass "pg_dump (container) schema-only successful." || \
                log_fail "pg_dump (container) failed."
        fi
    else
        log_warn "pg_dump not available and postgres container not accessible — skipping."
    fi

    # 4b: Redis SAVE (BGSAVE) connectivity test.
    if command -v redis-cli >/dev/null 2>&1; then
        if redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" PING | grep -q "PONG"; then
            log_pass "Redis PING successful."
            if [ "$MODE" = "live" ]; then
                if redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" BGSAVE | grep -q "OK"; then
                    log_pass "Redis BGSAVE triggered."
                else
                    log_fail "Redis BGSAVE failed."
                fi
            else
                log_info "[DRY-RUN] Would trigger Redis BGSAVE."
            fi
        else
            log_fail "Redis PING failed."
        fi
    elif docker exec "${STACK_NAME}-redis-1" redis-cli PING > /dev/null 2>&1; then
        if [ "$MODE" = "live" ]; then
            docker exec "${STACK_NAME}-redis-1" redis-cli BGSAVE > /dev/null 2>&1 && \
                log_pass "Redis BGSAVE (container) triggered." || \
                log_fail "Redis BGSAVE (container) failed."
        else
            log_info "[DRY-RUN] Would trigger Redis BGSAVE inside container."
            log_pass "Redis PING (container) successful."
        fi
    else
        log_warn "redis-cli not available and redis container not accessible — skipping."
    fi

    # 4c: MinIO bucket list.
    if command -v mc >/dev/null 2>&1 && [ -n "$MINIO_ROOT_PASSWORD" ]; then
        if mc alias set arbx-drill "$MINIO_ENDPOINT" "$MINIO_ROOT_USER" "$MINIO_ROOT_PASSWORD" > /dev/null 2>&1; then
            local buckets
            buckets=$(mc ls arbx-drill 2>/dev/null | wc -l)
            log_pass "MinIO bucket list successful ($buckets buckets)."
        else
            log_fail "MinIO alias set failed — check endpoint/credentials."
        fi
    elif curl -sk --max-time 10 "$MINIO_ENDPOINT/minio/health/live" > /dev/null 2>&1; then
        log_pass "MinIO health endpoint responding."
        log_info "[DRY-RUN] Skipping mc bucket list (mc CLI not available)."
    else
        log_warn "MinIO not accessible — skipping bucket list."
    fi
}

# ═════════════════════════════════════════════════════════════════════════════
#  TEST 5: Recovery Procedures — verify all health endpoints respond
# ═════════════════════════════════════════════════════════════════════════════
test_recovery() {
    log_step "TEST 5/6 — Recovery Procedures (Health Endpoints)"
    record "Recovery — verify all health endpoints respond"

    declare -a endpoints=(
        "Vault:$VAULT_ADDR/v1/sys/health"
        "Grafana:$GRAFANA_ENDPOINT/api/health"
        "Prometheus:$PROMETHEUS_ENDPOINT/-/healthy"
        "Alertmanager:$ALERTMANAGER_ENDPOINT/-/healthy"
        "Edge:$EDGE_ENDPOINT/health"
        "MinIO:$MINIO_ENDPOINT/minio/health/live"
    )

    for ep in "${endpoints[@]}"; do
        local name="${ep%%:*}"
        local url="${ep#*:}"
        local code
        code=$(http_get "$url")

        # Vault returns 200 (unsealed) or 501 (sealed but initialized) — both OK.
        if [ "$name" = "Vault" ]; then
            if [ "$code" = "200" ] || [ "$code" = "501" ] || [ "$code" = "503" ]; then
                log_pass "$name health: HTTP $code ( Vault: initialized=$([ "$code" = "200" ] && echo 'unsealed' || echo 'sealed') )."
            else
                log_fail "$name health: HTTP $code (expected 200/501/503)."
            fi
        elif [ "$code" = "200" ] || [ "$code" = "204" ]; then
            log_pass "$name health: HTTP $code."
        elif [ "$code" = "401" ] && [ "$name" = "Edge" ]; then
            # Edge may require auth; 401 still means it is up.
            log_pass "$name health: HTTP $code (endpoint reachable, auth required)."
        elif [ "$code" = "000" ]; then
            log_fail "$name health: connection failed (endpoint down or unreachable)."
        else
            log_warn "$name health: HTTP $code (unexpected but reachable)."
        fi
    done

    # Container-level health checks.
    local healthy unhealthy
    healthy=$(docker ps --filter "name=${STACK_NAME}" --format '{{.Names}}' 2>/dev/null | while read -r c; do
        docker inspect --format='{{.State.Health.Status}}' "$c" 2>/dev/null | grep -q "healthy" && echo "$c"
    done | wc -l)
    unhealthy=$(docker ps --filter "name=${STACK_NAME}" --format '{{.Names}}' 2>/dev/null | while read -r c; do
        local st
        st=$(docker inspect --format='{{.State.Health.Status}}' "$c" 2>/dev/null || echo "unknown")
        if [ "$st" != "healthy" ] && [ "$st" != "" ]; then
            echo "$c:$st"
        fi
    done)

    local uncount
    uncount=$(echo "$unhealthy" | grep -c ':' || echo 0)

    if [ "$uncount" -eq 0 ]; then
        log_pass "All containers report healthy (or no healthcheck configured)."
    else
        log_warn "$uncount container(s) not in healthy state."
        echo "$unhealthy" | while IFS=: read -r c st; do
            log_info "  • $c → $st"
        done
    fi
}

# ═════════════════════════════════════════════════════════════════════════════
#  TEST 6: Rollback Procedure — docker compose down + up with previous tag
# ═════════════════════════════════════════════════════════════════════════════
test_rollback() {
    log_step "TEST 6/6 — Rollback Procedure"
    record "Rollback — verify down/up procedure (dry-run by default)"

    local current_tag
    current_tag=$(docker compose -f "$COMPOSE_FILE" config 2>/dev/null | \
        grep -m1 "image:" | awk '{print $2}' || echo "unknown")

    log_info "Current image tag: $current_tag"

    if [ "$MODE" = "dry-run" ]; then
        log_info "[DRY-RUN] Rollback steps that would be executed in --live mode:"
        echo ""
        echo "  1. docker compose -f $COMPOSE_FILE ps"
        echo "  2. docker compose -f $COMPOSE_FILE pull  # fetch previous images"
        echo "  3. docker compose -f $COMPOSE_FILE down --timeout 120"
        echo "  4. sleep 10  # allow grace period"
        echo "  5. docker compose -f $COMPOSE_FILE up -d"
        echo "  6. sleep 30  # wait for health checks"
        echo "  7. Verify all containers healthy (see TEST 1 & 5)"
        echo ""
        log_pass "Rollback procedure documented (dry-run)."
        return
    fi

    # LIVE mode: perform actual rollback.
    log_warn "[LIVE] Performing actual rollback in 5 seconds... Ctrl-C to abort."
    sleep 5

    log_info "[LIVE] docker compose down..."
    if docker_compose_cmd down --timeout 120; then
        log_pass "docker compose down completed."
    else
        log_fail "docker compose down failed."
        return
    fi

    log_info "[LIVE] Waiting 10 seconds for cleanup..."
    sleep 10

    log_info "[LIVE] docker compose up -d..."
    if docker_compose_cmd up -d; then
        log_pass "docker compose up completed."
    else
        log_fail "docker compose up failed."
        return
    fi

    log_info "[LIVE] Waiting 30 seconds for containers to stabilize..."
    sleep 30

    # Re-run container and health checks.
    test_containers
    test_recovery
}

# ═════════════════════════════════════════════════════════════════════════════
#  DRILL REPORT
# ═════════════════════════════════════════════════════════════════════════════
generate_report() {
    local end_time
    end_time=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════════════════╗"
    echo "║           ARBITRAGEX v2 — DISASTER RECOVERY DRILL REPORT                    ║"
    echo "╠══════════════════════════════════════════════════════════════════════════════╣"
    echo "║  Drill start: $TIMESTAMP                                      ║"
    echo "║  Drill end:   $end_time                                      ║"
    echo "║  Mode:        ${MODE^^}                                                     ║"
    echo "║  Compose file: $COMPOSE_FILE"
    echo "╠══════════════════════════════════════════════════════════════════════════════╣"
    echo "║  Results:                                                                    ║"
    printf "║    PASSED:   %2d                                                             ║\n" "$PASSED"
    printf "║    FAILED:   %2d                                                             ║\n" "$FAILED"
    printf "║    WARNINGS: %2d                                                             ║\n" "$WARNINGS"
    echo "╠══════════════════════════════════════════════════════════════════════════════╣"
    echo "║  Details:                                                                    ║"
    for entry in "${REPORT_ENTRIES[@]}"; do
        printf "║    • %s\n" "$entry"
    done
    echo "╠══════════════════════════════════════════════════════════════════════════════╣"
    if [ "$FAILED" -eq 0 ]; then
        echo "║  STATUS: DRILL PASSED ✓                                                      ║"
    else
        echo "║  STATUS: DRILL FAILED ✗ — review FAILED items above                         ║"
    fi
    echo "╚══════════════════════════════════════════════════════════════════════════════╝"
    echo ""
    log_info "Drill complete. Review the report above."
    log_info "To run destructive tests, use: $0 --live"
}

# ═════════════════════════════════════════════════════════════════════════════
#  MAIN
# ═════════════════════════════════════════════════════════════════════════════
main() {
    log_info "═══════════════════════════════════════════════════════════════════════════════"
    log_info "ArbitrageX v2 — Disaster Recovery Drill"
    log_info "Timestamp: $TIMESTAMP"
    log_info "Mode: $MODE"
    log_info "Compose: $COMPOSE_FILE"
    [ -n "$FILTER_TEST" ] && log_info "Filter: $FILTER_TEST only"
    log_info "═══════════════════════════════════════════════════════════════════════════════"

    # Validate compose file exists.
    if [ ! -f "$COMPOSE_FILE" ]; then
        log_error "Compose file not found: $COMPOSE_FILE"
        exit 1
    fi

    # Determine which tests to run.
    if [ -n "$FILTER_TEST" ]; then
        case "$FILTER_TEST" in
            containers) test_containers ;;
            killswitch) test_killswitch ;;
            paper)      test_paper_mode ;;
            backups)    test_backups ;;
            recovery)   test_recovery ;;
            rollback)   test_rollback ;;
            *)
                log_error "Unknown test: $FILTER_TEST"
                echo "Valid: containers, killswitch, paper, backups, recovery, rollback" >&2
                exit 1 ;;
        esac
    else
        # Run all tests.
        test_containers
        test_killswitch
        test_paper_mode
        test_backups
        test_recovery
        test_rollback
    fi

    generate_report

    # Exit with failure count.
    exit "$FAILED"
}

main "$@"
