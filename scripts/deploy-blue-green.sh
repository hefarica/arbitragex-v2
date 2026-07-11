#!/bin/bash
# Blue-Green Zero-Downtime Deployment Script
# Para ejecutar en VPS via SSH

set -euo pipefail

VPS_HOST="${VPS_HOST:-arbx}"
DEPLOY_PATH="${DEPLOY_PATH:-/opt/arbitragex-v2}"
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.prod.yml}"
HEALTH_URL="${HEALTH_URL:-http://localhost:8080/health}"

echo "🚀 BLUE-GREEN DEPLOYMENT STARTED"
echo "================================"
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Target: $VPS_HOST:$DEPLOY_PATH"
echo ""

# 1. Guardar commit actual (Blue)
echo "🔵 CAPTURING BLUE STATE..."
ssh "$VPS_HOST" "cd $DEPLOY_PATH && git rev-parse HEAD" > /tmp/blue-commit.txt
BLUE_COMMIT=$(cat /tmp/blue-commit.txt)
echo "   Blue commit: $BLUE_COMMIT"

# 2. Fetch latest main
echo "⬇️ FETCHING GREEN..."
ssh "$VPS_HOST" "cd $DEPLOY_PATH && git fetch github main"

# 3. Reset to main (Green)
echo "🟢 SWITCHING TO GREEN..."
ssh "$VPS_HOST" "cd $DEPLOY_PATH && git reset --hard github/main"
GREEN_COMMIT=$(ssh "$VPS_HOST" "cd $DEPLOY_PATH && git rev-parse HEAD")
echo "   Green commit: $GREEN_COMMIT"

if [ "$BLUE_COMMIT" == "$GREEN_COMMIT" ]; then
    echo "⚠️  No changes detected. Skipping deploy."
    exit 0
fi

# 4. Pull images and build
echo "🐳 BUILDING GREEN IMAGES..."
ssh "$VPS_HOST" "cd $DEPLOY_PATH && docker compose -f $COMPOSE_FILE pull"
ssh "$VPS_HOST" "cd $DEPLOY_PATH && docker compose -f $COMPOSE_FILE up -d --build --remove-orphans"

# 5. Healthcheck
echo "🏥 HEALTHCHECK (max 20 attempts)..."
HC_OK=0
for i in $(seq 1 20); do
    if ssh "$VPS_HOST" "curl -fsS --max-time 5 $HEALTH_URL >/dev/null 2>&1"; then
        echo "   ✅ Healthcheck PASSED (attempt $i)"
        HC_OK=1
        break
    fi
    echo "   ⏳ Waiting for health ($i/20)..."
    sleep 5
done

if [ "$HC_OK" -ne 1 ]; then
    echo ""
    echo "❌ HEALTHCHECK FAILED - INITIATING ROLLBACK"
    echo "============================================"
    ssh "$VPS_HOST" "cd $DEPLOY_PATH && git reset --hard $BLUE_COMMIT"
    ssh "$VPS_HOST" "cd $DEPLOY_PATH && docker compose -f $COMPOSE_FILE up -d --build"
    echo "✅ Rollback complete to: $BLUE_COMMIT"
    exit 1
fi

# 6. Save blue commit for potential future rollback
echo "$BLUE_COMMIT" > /tmp/last-known-good-commit.txt
ssh "$VPS_HOST" "cat > $DEPLOY_PATH/.last-known-good-commit" < /tmp/blue-commit.txt

echo ""
echo "✅ BLUE-GREEN DEPLOYMENT COMPLETED SUCCESSFULLY"
echo "================================================"
echo "Blue (previous): $BLUE_COMMIT"
echo "Green (current): $GREEN_COMMIT"
echo "Healthcheck:     PASSED"
echo "Timestamp:       $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""
echo "Next: Run Playwright E2E validation"
