#!/bin/bash
# ArbitrageX v2 — Deployment Script
#
# Audit N1 (2026-05-10 re-run): this script previously claimed "Production"
# but invoked compose.dev.yml. Catastrophic operator confusion vector.
#
# Now: explicit COMPOSE_FILE selection (default: dev). Operator must opt-in
# to prod via `COMPOSE_FILE=docker/compose.prod.yml ./scripts/deploy.sh up -d`
# OR rename to scripts/deploy-dev.sh / scripts/deploy-prod.sh.
#
# Always exports .env vars before docker compose to ensure proper interpolation.
set -euo pipefail

cd /opt/arbitragex-v2

# Compose file selection — default DEV, operator overrides for PROD.
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.dev.yml}"

# Sanity: file must exist
if [ ! -f "$COMPOSE_FILE" ]; then
    echo "ERROR: COMPOSE_FILE='$COMPOSE_FILE' does not exist" >&2
    echo "  Available: $(ls docker/compose*.yml 2>/dev/null | tr '\n' ' ')" >&2
    exit 1
fi

# Sanity: refuse to run prod compose unless explicit confirmation
if [[ "$COMPOSE_FILE" == *"prod"* ]]; then
    if [ "${CONFIRM_PROD_DEPLOY:-}" != "true" ]; then
        echo "ERROR: prod compose detected ($COMPOSE_FILE) — set CONFIRM_PROD_DEPLOY=true to proceed" >&2
        echo "  Reason: prevent accidental prod deploys (audit N1, 2026-05-10)" >&2
        exit 1
    fi
fi

echo "=== ArbitrageX v2 Deploy ==="
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Compose file: $COMPOSE_FILE"
echo "Args: $*"

# Export .env vars for compose ${} interpolation
# shellcheck disable=SC2046
export $(grep -v '^#' .env | grep -v '^$' | xargs)

# Pull latest (best-effort; do not fail deploy if upstream unreachable)
git pull origin main 2>/dev/null || echo "WARN: git pull failed; continuing with local HEAD"

# Deploy
docker compose --env-file .env -f "$COMPOSE_FILE" "$@"

echo "=== Deploy complete ($COMPOSE_FILE) ==="
