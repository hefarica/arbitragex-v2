# OMEGA Pipeline Deployment Guide

**Last Updated:** 2026-07-11
**Version:** 2.0
**Target Environment:** VPS (Hetzner/DigitalOcean/AWS)

## Prerequisites

### Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 8 cores | 16+ cores (searcher-rs is CPU-intensive) |
| RAM | 32 GB | 64+ GB |
| Disk | 500 GB SSD | 1+ TB NVMe |
| Network | 1 Gbps | 10 Gbps (low latency to RPC providers) |

### Software Requirements

- Docker 24.0+ with Docker Compose v2
- Git 2.40+
- OpenSSL 3.0+ (for Vault TLS)
- UFW or iptables (firewall)
- ssh-keygen (for deployment keys)

### Network Requirements

| Port | Service | Direction | Notes |
|------|---------|-----------|-------|
| 22 | SSH | Inbound | Restrict to operator IPs |
| 80 | HTTP | Inbound | Redirects to HTTPS |
| 443 | HTTPS | Inbound | Cloudflare origin pull |
| 8787 | Edge Worker | Inbound | Public API endpoint |
| 9090 | Prometheus | Localhost only | Internal metrics |
| 3000 | Grafana | Localhost only | Internal dashboard |

## Environment Variables

### Required Variables (Production)

Create `.env` in project root with these variables:

```bash
# ============================================
# TIER 0: Critical Secrets (Vault-backed in production)
# ============================================

# PostgreSQL
cat<<'EOF'
POSTGRES_PASSWORD=$(openssl rand -base64 32)
ARBX_MIGRATOR_PASSWORD=$(openssl rand -base64 32)
ARBX_RW_PASSWORD=$(openssl rand -base64 32)
ARBX_RO_PASSWORD=$(openssl rand -base64 32)
EOF

# API Tokens (min 32 bytes entropy)
ARBX_ADMIN_TOKEN=$(openssl rand -base64 48)
ARBX_EDGE_TOKEN=$(openssl rand -base64 48)
ARBX_SERVICE_TOKEN=$(openssl rand -base64 48)

# JWT Secret
JWT_SECRET=$(openssl rand -base64 64)

# MinIO
MINIO_ROOT_USER=arbitragex_admin
MINIO_ROOT_PASSWORD=$(openssl rand -base64 32)

# ============================================
# TIER 1: External Service Credentials
# ============================================

# RPC Endpoints (Alchemy/Infura/QuickNode)
# Primary
RPC_WS_1=wss://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY

# Backup (optional but recommended)
RPC_WS_2=wss://mainnet.infura.io/ws/v3/YOUR_INFURA_KEY
RPC_HTTP_2=https://mainnet.infura.io/v3/YOUR_INFURA_KEY

# Anvil Fork URL (for simulations)
ANVIL_FORK_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY

# WalletConnect (optional)
NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID=your_project_id

# GitHub Token (for token icon enrichment)
GITHUB_TOKEN=ghp_your_personal_access_token

# ============================================
# TIER 2: Configuration
# ============================================

# Environment
NODE_ENV=production
RUST_LOG=info
LOG_LEVEL=info

# Public URLs (update with your domain/IP)
NEXT_PUBLIC_EDGE_URL=https://your-domain.com:8787
NEXT_PUBLIC_WS_URL=wss://your-domain.com:8080

# Chain Configuration
ENRICHER_CHAINS=1,137,42161  # Ethereum, Polygon, Arbitrum

# Price Oracles (default: off until configured)
ARBX_DEXSCREENER_ORACLE=off
ARBX_GECKOTERMINAL_ORACLE=off

# Trading Mode (paper/shadow/live)
ARBX_TRADE_MODE=paper

# Database URL (constructed from passwords above)
DATABASE_URL=postgres://arbx_rw:${ARBX_RW_PASSWORD}@postgres:5432/arbitragex
DATABASE_READONLY_URL=postgres://arbx_ro:${ARBX_RO_PASSWORD}@postgres:5432/arbitragex
REDIS_URL=redis://redis:6379
```

### Generating Secure Secrets

```bash
#!/bin/bash
# generate_secrets.sh - Run this once to generate secrets

export POSTGRES_PASSWORD=$(openssl rand -base64 32)
export ARBX_MIGRATOR_PASSWORD=$(openssl rand -base64 32)
export ARBX_RW_PASSWORD=$(openssl rand -base64 32)
export ARBX_RO_PASSWORD=$(openssl rand -base64 32)
export ARBX_ADMIN_TOKEN=$(openssl rand -base64 48)
export ARBX_EDGE_TOKEN=$(openssl rand -base64 48)
export ARBX_SERVICE_TOKEN=$(openssl rand -base64 48)
export JWT_SECRET=$(openssl rand -base64 64)
export MINIO_ROOT_PASSWORD=$(openssl rand -base64 32)

cat > .env << EOF
# Generated $(date -Iseconds)
POSTGRES_PASSWORD=$POSTGRES_PASSWORD
ARBX_MIGRATOR_PASSWORD=$ARBX_MIGRATOR_PASSWORD
ARBX_RW_PASSWORD=$ARBX_RW_PASSWORD
ARBX_RO_PASSWORD=$ARBX_RO_PASSWORD
ARBX_ADMIN_TOKEN=$ARBX_ADMIN_TOKEN
ARBX_EDGE_TOKEN=$ARBX_EDGE_TOKEN
ARBX_SERVICE_TOKEN=$ARBX_SERVICE_TOKEN
JWT_SECRET=$JWT_SECRET
MINIO_ROOT_USER=arbitragex_admin
MINIO_ROOT_PASSWORD=$MINIO_ROOT_PASSWORD
EOF

echo "Secrets generated. Now edit .env to add your RPC endpoints."
```

## Docker Compose Configuration

### Production Compose Overview

The production stack uses `docker/compose.prod.yml` with these services:

| Service | Memory Limit | CPU Limit | Purpose |
|---------|--------------|-----------|---------|
| postgres | 4G | 2.0 | Data persistence |
| redis | 2G | 1.0 | Streams, cache, pub/sub |
| searcher-rs | 4G | 2.0 | Opportunity detection |
| api-server | 1G | 1.0 | API gateway, WebSocket |
| edge | 512M | 0.5 | Public edge proxy |
| sim-ctl | 2G | 1.5 | REVM simulation |
| relays-client | 1G | 1.0 | Bundle submission |
| recon | 1G | 1.0 | Risk analysis |
| selector-api | 512M | 0.5 | Token safety scoring |
| token-enricher | 512M | 0.5 | Metadata enrichment |

### Customizing Resource Limits

Edit `docker/compose.prod.yml` service definitions:

```yaml
services:
  searcher-rs:
    deploy:
      resources:
        limits:
          memory: 8G  # Increase for high-frequency detection
          cpus: '4.0'
        reservations:
          memory: 1G
```

### Override Files

Use override files for environment-specific changes:

```bash
# Staging overrides (staging-specific env vars)
docker compose -f docker/compose.prod.yml -f docker/compose.staging.override.yml up -d

# Loopback overrides (local development with external services)
docker compose -f docker/compose.prod.yml -f docker/compose.loopback.override.yml up -d
```

## VPS Deployment Steps

### Step 1: Server Preparation

```bash
# Update system
apt-get update && apt-get upgrade -y

# Install Docker
curl -fsSL https://get.docker.com | sh
systemctl enable docker
systemctl start docker

# Install Docker Compose v2
docker compose version  # Verify v2.20+

# Create deploy user
useradd -m -s /bin/bash arbx
usermod -aG docker arbx

# Create directories
mkdir -p /opt/arbitragex-v2
chown arbx:arbx /opt/arbitragex-v2
```

### Step 2: SSH Key Setup

```bash
# On local machine, generate deployment key
ssh-keygen -t ed25519 -C "arbx-deploy" -f ~/.ssh/arbx_deploy

# Copy public key to server
ssh-copy-id -i ~/.ssh/arbx_deploy.pub arbx@YOUR_VPS_IP

# Test connection
ssh -i ~/.ssh/arbx_deploy arbx@YOUR_VPS_IP
```

### Step 3: Repository Clone

```bash
# On VPS as arbx user
cd /opt/arbitragex-v2
git clone https://github.com/hefarica/arbitragex-v2.git .

# Or use SSH for private repos
git clone git@github.com:hefarica/arbitragex-v2.git .
```

### Step 4: Environment Setup

```bash
cd /opt/arbitragex-v2

# Generate secrets
./scripts/generate_secrets.sh  # Or run commands from above

# Edit .env with your RPC endpoints
nano .env

# Verify required variables are set
./scripts/verify_env.sh
# Or manually check:
grep -E "^(RPC_WS_1|RPC_HTTP_1|NEXT_PUBLIC_EDGE_URL)=" .env
```

### Step 5: Data Plane Bootstrap

```bash
# Start PostgreSQL and Redis first
docker compose -f docker/compose.prod.yml up -d postgres redis

# Wait for health checks
docker compose -f docker/compose.prod.yml exec postgres pg_isready -U postgres -d arbitragex
docker compose -f docker/compose.prod.yml exec redis redis-cli PING

# Verify data volumes
docker volume ls | grep arbitragex
```

### Step 6: Vault Initialization (Optional but Recommended)

```bash
# Start Vault
docker compose -f docker/compose.prod.yml up -d vault

# Initialize (save the unseal keys!)
docker compose -f docker/compose.prod.yml exec vault vault operator init \
  -key-shares=5 -key-threshold=3

# Unseal (run 3 times with different keys)
docker compose -f docker/compose.prod.yml exec vault vault operator unseal KEY1
docker compose -f docker/compose.prod.yml exec vault vault operator unseal KEY2
docker compose -f docker/compose.prod.yml exec vault vault operator unseal KEY3

# Store root token securely (use for initial setup only)
export VAULT_ROOT_TOKEN=YOUR_ROOT_TOKEN
```

### Step 7: Core Services Deployment

```bash
# Deploy searcher-rs and dependencies
docker compose -f docker/compose.prod.yml up -d searcher-rs

# Wait for scanner initialization (30-60 seconds)
sleep 30
docker logs searcher-rs --tail 20 | grep -i "boot.complete\|scanner.ready"

# Deploy remaining backend services
docker compose -f docker/compose.prod.yml up -d sim-ctl selector-api recon relays-client token-enricher

# Deploy API and edge layers
docker compose -f docker/compose.prod.yml up -d api-server edge frontend
```

### Step 8: Observability Stack

```bash
# Deploy monitoring
docker compose -f docker/compose.prod.yml up -d prometheus grafana loki promtail

# Deploy Thanos for long-term metrics
docker compose -f docker/compose.prod.yml up -d minio thanos-sidecar thanos-store thanos-query

# Initialize MinIO bucket for Thanos
# Access MinIO console at http://localhost:9001 (via SSH tunnel)
# Create bucket: arbx-metrics
```

### Step 9: SSL/TLS Setup (Let's Encrypt)

```bash
# Install certbot
apt-get install -y certbot

# Obtain certificate (adjust domain)
certbot certonly --standalone -d your-domain.com

# Create certificate directory for services
mkdir -p /opt/arbitragex-v2/certs
cp /etc/letsencrypt/live/your-domain.com/fullchain.pem /opt/arbitragex-v2/certs/
cp /etc/letsencrypt/live/your-domain.com/privkey.pem /opt/arbitragex-v2/certs/

# Update compose to mount certificates (if using native TLS)
# Or use Cloudflare for SSL termination at edge
```

### Step 10: Firewall Configuration

```bash
# UFW setup
ufw default deny incoming
ufw default allow outgoing

# Allow SSH (restrict to your IP)
ufw allow from YOUR_IP to any port 22

# Allow HTTP/HTTPS
ufw allow 80/tcp
ufw allow 443/tcp

# Allow edge (if not behind Cloudflare)
ufw allow 8787/tcp

# Allow internal services (localhost only)
ufw allow from 127.0.0.1 to any port 5432  # PostgreSQL
ufw allow from 127.0.0.1 to any port 6379  # Redis
ufw allow from 127.0.0.1 to any port 8080  # api-server
ufw allow from 127.0.0.1 to any port 9090  # Prometheus
ufw allow from 127.0.0.1 to any port 3000  # Grafana

# Enable firewall
ufw enable
```

## Verification Checklist

### Deployment Verification

```bash
# 1. All containers running
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Health}}"
# Expected: All containers "Up" and "healthy"

# 2. Health endpoints respond
curl -s http://localhost:8787/api/v1/health | jq '.ok'  # Should be true
curl -s http://localhost:8080/api/health | jq '.ok'      # Should be true
curl -s http://localhost:9001/health | jq '.ok'          # Should be true

# 3. Data plane connectivity
docker exec redis redis-cli PING  # Should return PONG
docker exec postgres pg_isready -U postgres -d arbitragex  # Should return "accepting connections"

# 4. Opportunities table exists
docker exec postgres psql -U postgres -d arbitragex -c "SELECT COUNT(*) FROM opportunities;"

# 5. Redis streams accessible
docker exec redis redis-cli XLEN arbx:hot:detected  # Should return 0 or positive integer

# 6. WebSocket accepts connections
# Use browser DevTools or:
npm install -g wscat
wscat -c "ws://localhost:8080" -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN"
# Should connect without errors
```

### Smoke Tests

```bash
# Test 1: Edge to API connectivity
curl -s http://localhost:8787/api/opportunities/live | jq '.opportunities'

# Test 2: Scanner heartbeat
curl -s http://localhost:8787/api/scanner/heartbeat | jq '.pipeline_latency_ms'

# Test 3: Readiness check
curl -s http://localhost:8787/api/readiness | jq '.overall'

# Test 4: Kill-switch state
curl -s http://localhost:8787/api/killswitch/status -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" | jq '.enabled'

# Test 5: Token prices (if configured)
docker exec redis redis-cli HGETALL arbx:token_prices:1 | head -10
```

### Log Verification

```bash
# Check for errors in all services
docker logs searcher-rs --tail 100 | grep -i error
docker logs api-server --tail 100 | grep -i error
docker logs edge --tail 100 | grep -i error

# Expected: No ERROR logs (WARN is acceptable)
```

## Post-Deployment Configuration

### 1. Grafana Dashboard Setup

```bash
# Access Grafana via SSH tunnel
ssh -L 3000:localhost:3000 arbx@YOUR_VPS_IP

# Open http://localhost:3000 in browser
# Default credentials: admin/admin (change on first login)

# Import dashboards from monitoring/grafana/dashboards/
# Data source: Prometheus (http://prometheus:9090)
```

### 2. Alertmanager Configuration

Edit `monitoring/alertmanager/alertmanager.yml`:

```yaml
global:
  slack_api_url: 'YOUR_SLACK_WEBHOOK_URL'

route:
  receiver: 'default'
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'

receivers:
  - name: 'default'
    slack_configs:
      - channel: '#arbx-alerts'
        title: 'OMEGA Pipeline Alert'
  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'YOUR_PAGERDUTY_KEY'
```

Restart alertmanager:
```bash
docker compose -f docker/compose.prod.yml restart alertmanager
```

### 3. Token Watchlist Configuration

```bash
# Add tokens to watchlist via SQL
docker exec -i postgres psql -U postgres -d arbitragex << EOF
UPDATE tokens SET is_watched = true WHERE symbol IN ('WETH', 'USDC', 'USDT', 'DAI', 'WBTC');
EOF

# Or use the frontend at /strategies tab
```

### 4. Price Oracle Activation

```bash
# Enable DexScreener oracle (edit .env)
ARBX_DEXSCREENER_ORACLE=active
ARBX_MIN_PRICE_LIQUIDITY_USD=50000
DEXSCREENER_PRICE_INTERVAL_MS=15000

# Restart token-enricher
docker compose -f docker/compose.prod.yml up -d --force-recreate token-enricher
```

## Rollback Procedures

### Service Rollback

```bash
# Rollback to previous image
docker compose -f docker/compose.prod.yml pull searcher-rs
docker compose -f docker/compose.prod.yml up -d --force-recreate searcher-rs

# Or use specific version
docker compose -f docker/compose.prod.yml stop searcher-rs
docker run -d --name searcher-rs-backup your-registry/searcher-rs:v1.2.3
```

### Database Rollback

```bash
# Restore from backup (requires prior backup)
docker exec -i postgres psql -U postgres -d arbitragex < backup_$(date +%Y%m%d).sql

# Or use pg_restore for custom format
docker exec -i postgres pg_restore -U postgres -d arbitragex backup.dump
```

### Full Stack Rollback

```bash
# Emergency: return to last known good state
docker compose -f docker/compose.prod.yml down
git checkout stable
docker compose -f docker/compose.prod.yml up -d
```

## Maintenance Windows

### Scheduled Maintenance Procedure

```bash
# 1. Arm kill-switch 5 minutes before
curl -X POST http://localhost:8787/admin/killswitch \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"enabled": true, "reason": "scheduled_maintenance", "triggered_by": "operator"}'

# 2. Wait for in-flight operations to complete
sleep 60

# 3. Perform maintenance (updates, config changes, etc.)

# 4. Verify system health
./scripts/smoke_test.sh

# 5. Disarm kill-switch
curl -X POST http://localhost:8787/admin/killswitch \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"enabled": false, "reason": "maintenance_complete", "triggered_by": "operator"}'
```

## Troubleshooting Deployment Issues

### Issue: Containers fail to start

```bash
# Check logs
docker compose -f docker/compose.prod.yml logs --tail 50 searcher-rs

# Common causes:
# 1. Missing env variables
grep -E "^[^#].*=" .env | wc -l  # Should be > 20

# 2. Port conflicts
netstat -tlnp | grep -E "(8080|8787|9001|5432|6379)"

# 3. Insufficient resources
docker system df  # Check disk space
free -h           # Check memory
```

### Issue: searcher-rs exits immediately

```bash
# Check for missing DATABASE_URL
docker logs searcher-rs --tail 20 | grep -i "database\|env"

# Verify database connectivity
docker exec postgres psql -U arbx_rw -d arbitragex -c "SELECT 1"

# Check RPC endpoints
curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  $RPC_HTTP_1 | jq '.result'
```

### Issue: Frontend shows "Connection Error"

```bash
# Check edge is running
docker ps | grep edge

# Check edge can reach api-server
docker exec edge wget -qO- http://api-server:8080/health

# Check CORS configuration in edge worker
# Verify NEXT_PUBLIC_EDGE_URL in .env matches actual URL
```

## Related Documentation

- [Pipeline Architecture](./pipeline-architecture.md) - System design
- [Runbook](./runbook.md) - Operational procedures
- [API Reference](./api-reference.md) - Endpoints and events
- [Vault Setup](../operations/VAULT_SETUP.md) - Secret management
- [Database Migrations](../../database/migrations/README.md) - Schema management

---

*Document maintained by OMEGA DevOps Team. Test deployment procedures in staging before production use.*
