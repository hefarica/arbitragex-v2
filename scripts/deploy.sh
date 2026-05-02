#!/bin/bash
# ArbitrageX v2 — Production Deployment Script
# Always exports .env vars before docker compose to ensure proper interpolation
set -e

cd /opt/arbitragex-v2

echo "=== ArbitrageX v2 Deploy ==="
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Export .env vars for compose ${} interpolation
export $(grep -v '^#' .env | grep -v '^$' | xargs)

# Pull latest
git pull origin main 2>/dev/null || true

# Deploy
docker compose -f docker/compose.dev.yml "$@"

echo "=== Deploy complete ==="
