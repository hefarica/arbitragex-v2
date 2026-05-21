#!/usr/bin/env bash
set -euo pipefail

# deploy.sh — Idempotent deploy script for arbitragex-v2 VPS
# Usage: ./deploy.sh [environment]

ENV="${1:-prod}"
REPO_DIR="/opt/arbitragex-v2"
HEALTH_URL="http://localhost:8080/health"
MAX_WAIT=180

log() { echo "[$(date +%H:%M:%S)] $*"; }

# 1. Check prerequisites
log "Checking prerequisites..."
command -v docker >/dev/null || { log "❌ Docker not installed"; exit 1; }
command -v docker compose >/dev/null || { log "❌ Docker Compose not found"; exit 1; }
command -v git >/dev/null || { log "❌ Git not installed"; exit 1; }
log "✅ Prerequisites OK"

# 2. Clone or pull
if [ -d "$REPO_DIR/.git" ]; then
  log "Pulling latest..."
  cd "$REPO_DIR"
  git stash
  git pull origin main
  git stash pop || true
else
  log "Cloning repository..."
  git clone https://github.com/hefarica/arbitragex-v2.git "$REPO_DIR"
  cd "$REPO_DIR"
fi

# 3. Environment setup
if [ ! -f .env ]; then
  log "⚠️  .env not found, copying from example"
  cp .env.example .env || { log "❌ No .env.example"; exit 1; }
fi

# 4. Pull and deploy
log "Deploying ($ENV)..."
docker compose -f "docker-compose.$ENV.yml" pull
docker compose -f "docker-compose.$ENV.yml" up -d --remove-orphans

# 5. Wait for healthchecks
log "Waiting for healthchecks (max ${MAX_WAIT}s)..."
for i in $(seq 1 $((MAX_WAIT / 5))); do
  if curl -fsS --max-time 5 "$HEALTH_URL" >/dev/null 2>&1; then
    log "✅ API healthy"
    break
  fi
  if [ "$i" -eq $((MAX_WAIT / 5)) ]; then
    log "❌ Health check failed — rolling back"
    docker compose -f "docker-compose.$ENV.yml" down
    git reset --hard HEAD@{1}
    docker compose -f "docker-compose.$ENV.yml" up -d
    log "✅ Rollback complete"
    exit 1
  fi
  sleep 5
  echo -n "."
done

# 6. Status
log "Deployment complete"
docker compose -f "docker-compose.$ENV.yml" ps --format "table {{.Service}}\t{{.Status}}\t{{.Ports}}"
log "✅ All services deployed successfully"
