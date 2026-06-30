# How to Deploy ArbitrageX v2 to a VPS

> **Document Type**: How-To Guide (Diátaxis Framework)
>
> How-To guides are goal-oriented and provide step-by-step instructions for achieving a specific outcome. They assume the reader knows what they want to accomplish. For conceptual background, see the Explanation documents. For precise API details, see the Reference.

## Goal

Deploy the complete ArbitrageX v2 platform — all 21 containers — to a VPS (Virtual Private Server) using Docker Compose, with secrets managed by HashiCorp Vault and health verification via automated checks.

## Prerequisites

| Requirement | Version | Notes |
|------------|---------|-------|
| VPS with public IP | Any | Minimum 4 vCPU, 16 GB RAM, 100 GB SSD |
| Ubuntu 22.04 LTS | 22.04 or newer | Other distros may work but are untested |
| Docker Engine | 24.x or newer | Install via Docker's official repository |
| Docker Compose | 2.20+ (plugin) | Must support `docker compose` (not legacy `docker-compose`) |
| Git | 2.34+ | For cloning the repository |
| OpenSSH | 8.x+ | For remote access |
| Domain (optional) | — | For Cloudflare Tunnel; IP-only access works |

## Step 1: Provision the VPS

### 1.1 Create the Server

Provision a VPS with these minimum specifications:

```
CPU:     4 vCPU cores
RAM:     16 GB
Storage: 100 GB SSD (expand to 200 GB for long-term metrics)
Network: 1 Gbps, public IPv4
OS:      Ubuntu 22.04 LTS
```

Recommended providers: Hetzner (CX41), DigitalOcean (General Purpose 4vCPU/16GB), or OVH (VPS Comfort).

### 1.2 Initial Server Setup

```bash
# SSH into the VPS (replace with your actual IP)
ssh root@<VPS_IP>

# Update system packages
apt update && apt upgrade -y

# Install essential tools
apt install -y curl wget git ufw fail2ban age

# Create operator user
useradd -m -s /bin/bash operator
usermod -aG sudo operator

# Set password for operator
passwd operator

# Configure SSH (disable root login, key-only auth)
sed -i 's/^#*PermitRootLogin.*/PermitRootLogin no/' /etc/ssh/sshd_config
sed -i 's/^#*PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config
systemctl restart sshd

# Set up UFW firewall
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp   # SSH
ufw allow 80/tcp   # HTTP (redirect to HTTPS)
ufw allow 443/tcp  # HTTPS
ufw --force enable

# Install Docker (official repository)
curl -fsSL https://get.docker.com | sh
systemctl enable docker
systemctl start docker

# Add operator to docker group
usermod -aG docker operator

# Create application directory
mkdir -p /opt/arbitragex-v2
chown operator:operator /opt/arbitragex-v2
```

## Step 2: Clone and Configure the Repository

### 2.1 Clone as Operator User

```bash
# Switch to operator user
su - operator

# Clone the repository
cd /opt/arbitragex-v2
git clone https://github.com/arbitragex/arbitragex-v2.git .

# Verify the branch
git branch --show-current
# Expected: main
```

### 2.2 Create Required Directories

```bash
# Create directories for secrets, backups, and logs
sudo mkdir -p /run/secrets/arbx
sudo mkdir -p /var/backups/arbx
sudo mkdir -p /var/log/arbx
sudo chown -R operator:operator /run/secrets/arbx /var/backups/arbx /var/log/arbx

# Set restrictive permissions
chmod 700 /run/secrets/arbx
chmod 700 /var/backups/arbx
```

### 2.3 Prepare the Environment

Copy the example environment file:

```bash
cd /opt/arbitragex-v2
cp .env.example .env
```

Edit `.env` with your values. At minimum, set these variables:

```bash
# Required for the stack to start
ENV=production
NODE_ENV=production

# Database (set strong passwords)
POSTGRES_PASSWORD=$(openssl rand -base64 32)
ARBX_MIGRATOR_PASSWORD=$(openssl rand -base64 32)
ARBX_RW_PASSWORD=$(openssl rand -base64 32)
ARBX_RO_PASSWORD=$(openssl rand -base64 32)
DATABASE_URL="postgres://arbx_rw:${ARBX_RW_PASSWORD}@postgres:5432/arbitragex"

# Redis
REDIS_URL=redis://redis:6379

# Admin tokens (generate strong random values)
ARBX_ADMIN_TOKEN=$(openssl rand -hex 32)
ARBX_EDGE_TOKEN=$(openssl rand -hex 32)
ARBX_SERVICE_TOKEN=$(openssl rand -hex 32)

# RPC endpoints (required for S2+)
RPC_HTTP_1="alchemy=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY"
RPC_WS_1="alchemy=wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY"

# JWT
JWT_SECRET=$(openssl rand -hex 32)
```

> **Security note**: Never commit the `.env` file. It is listed in `.gitignore` by default.

### 2.4 Generate Age Encryption Key

```bash
# Generate age key for backup encryption
age-keygen -o /root/arbx.age-identity
chmod 600 /root/arbx.age-identity
```

## Step 3: Deploy via GitHub Actions (Recommended)

### 3.1 Configure GitHub Secrets

In your GitHub repository, add these secrets under **Settings > Secrets and variables > Actions**:

| Secret Name | Description |
|-------------|-------------|
| `VPS_HOST` | `<VPS_IP>` |
| `VPS_USER` | `operator` |
| `VPS_SSH_KEY` | Private SSH key for the operator user |
| `ARBX_ADMIN_TOKEN` | Generated admin token |
| `ARBX_EDGE_TOKEN` | Generated edge token |
| `ARBX_SERVICE_TOKEN` | Generated service token |
| `POSTGRES_PASSWORD` | PostgreSQL superuser password |
| `ARBX_RW_PASSWORD` | Database read-write password |
| `ARBX_RO_PASSWORD` | Database read-only password |
| `RPC_HTTP_1` | Primary RPC HTTP endpoint |
| `RPC_WS_1` | Primary RPC WebSocket endpoint |
| `JWT_SECRET` | JWT signing secret |
| `GRAFANA_ADMIN_PASSWORD` | Grafana admin password |

### 3.2 Trigger Deployment

```bash
# From your local machine
git push origin main

# Then trigger the workflow via GitHub UI or CLI
gh workflow run deploy.yml -f environment=production
```

The GitHub Actions workflow will:

1. Build all Docker images
2. Push images to the registry
3. SSH into the VPS
4. Pull the latest code
5. Run database migrations
6. Start all services in dependency order
7. Run health checks
8. Report deployment status

### 3.3 Manual Deployment (Alternative)

If GitHub Actions is not configured, deploy manually:

```bash
# SSH into VPS
ssh operator@<VPS_IP>

cd /opt/arbitragex-v2

# Pull latest code
git pull origin main

# Build all images
docker compose -f docker/compose.prod.yml build

# Initialize Vault (first-time only)
# See docs/runbooks/vault-unseal.md for the init procedure
docker compose -f docker/compose.prod.yml up -d vault
# ... perform vault init and unseal ...

# Start all services
docker compose -f docker/compose.prod.yml --env-file /run/secrets/arbx.env up -d

# Wait for startup
sleep 30

# Check status
docker compose -f docker/compose.prod.yml ps
```

## Step 4: Verify Deployment

### 4.1 Check All Containers

```bash
docker compose -f docker/compose.prod.yml ps
```

Expected output:
```
NAME                    IMAGE                              STATUS
arbitragex-v2-postgres  postgres:15                        Up 2 minutes (healthy)
arbitragex-v2-redis     redis:7.2                          Up 2 minutes (healthy)
arbitragex-v2-vault     hashicorp/vault                    Up 2 minutes
... (all 21 containers)
```

### 4.2 Check Health Endpoints

```bash
# API server health
curl -s http://localhost:8080/health | jq .
# Expected: {"status":"ok","service":"api-server","version":"0.1.0"}

# Edge health
curl -s http://localhost:8787/api/health | jq .
# Expected: {"status":"ok","service":"edge"}

# Selector API health
curl -s http://localhost:3002/health | jq .
# Expected: {"status":"ok","service":"selector-api"}
```

### 4.3 Check Full Status

```bash
curl -s http://localhost:8080/status | jq .
```

Expected (all services `ok: true`):
```json
{
  "ok": true,
  "services": {
    "selector-api": { "ok": true, "status": 200 },
    "sim-ctl": { "ok": true, "status": 200 },
    "recon": { "ok": true, "status": 200 },
    "relays-client": { "ok": true, "status": 200 },
    "searcher-rs": { "ok": true, "status": 200 }
  },
  "killswitch": {
    "enabled": false,
    "reason": null,
    "triggered_by": null,
    "updated_at": null
  }
}
```

### 4.4 Check Readiness Score

```bash
curl -s http://localhost:8080/api/v1/readiness | jq '.score'
# Expected: 17 (or close to it during initial startup)
```

### 4.5 Verify Grafana

```bash
# Test Grafana is accessible
curl -s -u admin:$GRAFANA_ADMIN_PASSWORD http://localhost:3000/api/health | jq .
# Expected: {"database": "ok", "version": "10.x.x"}
```

Open the Platform Overview dashboard:
```bash
# Via SSH tunnel (recommended)
ssh -L 3000:localhost:3000 operator@<VPS_IP>
# Then open http://localhost:3000 in your browser
```

### 4.6 Verify Prometheus Targets

```bash
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job, health}'
```

All targets should show `"health": "up"`.

## Step 5: Troubleshooting Common Issues

### Issue: Vault is Sealed

**Symptoms**: Services fail to start, `vault-agent` in restart loop.

**Solution**:
```bash
# Check Vault status
docker compose -f docker/compose.prod.yml exec vault vault status

# If sealed, unseal with 3 of 5 keys
# See docs/runbooks/vault-unseal.md for full procedure
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# Enter key share 1
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# Enter key share 2
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# Enter key share 3
```

### Issue: Database Connection Failures

**Symptoms**: `api-server`, `searcher-rs` show ` unhealthy`, logs show `connection refused`.

**Solution**:
```bash
# Check PostgreSQL is running
docker compose -f docker/compose.prod.yml logs postgres | tail -20

# Verify database exists and migrations ran
docker compose -f docker/compose.prod.yml exec postgres psql -U postgres -c "\\l"

# Check if pg_isready responds
docker compose -f docker/compose.prod.yml exec postgres pg_isready -U postgres

# If migrations failed, run manually
docker compose -f docker/compose.prod.yml run --rm api-server npm run migrate
```

### Issue: Redis Unreachable

**Symptoms**: Kill-switch state errors, pub/sub failures.

**Solution**:
```bash
# Check Redis
docker compose -f docker/compose.prod.yml exec redis redis-cli PING
# Expected: PONG

# Check Redis memory
docker compose -f docker/compose.prod.yml exec redis redis-cli INFO memory | grep used_memory_human
```

### Issue: RPC Provider Failures

**Symptoms**: `searcher-rs` shows no opportunities detected, `NoOpportunitiesDetected` alert fires.

**Solution**:
```bash
# Test RPC connectivity from searcher-rs container
docker compose -f docker/compose.prod.yml exec searcher-rs wget -qO- \
  "http://<RPC_HTTP_1>/" -O /dev/null

# Check RPC provider status page (Alchemy, Infura)
# Verify API key is valid and rate limits not exceeded

# If RPC is down, check failover
curl -s http://localhost:8080/api/v1/readiness | jq '.checks[] | select(.id | contains("rpc"))'
```

### Issue: High Memory Usage / OOM

**Symptoms**: Containers killed by OOM killer, `OutOfMemory` in logs.

**Solution**:
```bash
# Check memory usage
docker stats --no-stream --format "table {{.Name}}\t{{.MemUsage}}\t{{.MemPerc}}"

# Check host memory
free -h

# If host memory is exhausted:
# 1. Add swap (temporary):
sudo fallocate -l 4G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile

# 2. Or upgrade VPS to higher memory tier
# 3. Or reduce container memory limits in docker/compose.prod.yml
```

### Issue: Grafana Shows No Data

**Symptoms**: Dashboard panels are empty or show "No data".

**Solution**:
```bash
# Check Prometheus targets
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job, health, lastError}'

# Check Thanos is writing to MinIO
docker compose -f docker/compose.prod.yml exec minio mc ls local/thanos

# Verify Promtail is shipping logs
docker compose -f docker/compose.prod.yml logs promtail | tail -20
```

### Issue: Deployment Rollback

If the deployment fails catastrophically and you need to rollback:

```bash
cd /opt/arbitragex-v2

# Stop all containers
docker compose -f docker/compose.prod.yml down

# Checkout previous known-good commit
git log --oneline -10  # find the previous commit
git checkout <previous-commit-hash>

# Rebuild and restart
docker compose -f docker/compose.prod.yml build
docker compose -f docker/compose.prod.yml --env-file /run/secrets/arbx.env up -d
```

## Post-Deployment Checklist

- [ ] All 21 containers show `STATUS: Up (healthy)`
- [ ] `/health` endpoint returns `status: ok` for api-server and edge
- [ ] `/status` endpoint shows all services `ok: true`
- [ ] `/api/v1/readiness` score is 14+ (target: 17/17)
- [ ] Grafana dashboard loads with data
- [ ] Prometheus targets all show `up`
- [ ] Kill-switch is in expected state (armed or disarmed)
- [ ] Paper mode is enabled (per-chain configuration)
- [ ] Vault is unsealed and vault-agent is rendering secrets
- [ ] Alertmanager is routing alerts to Slack
- [ ] First database backup completed
- [ ] Operator credentials tested (admin token, Grafana login)

## Related

- `docs/explanation/architecture-overview.md`
- `docs/reference/api-endpoints.md`
- `docs/runbooks/vault-unseal.md`
- `docs/runbooks/kill-switch-activation.md`
- `docs/adr/003-vault-secrets-management.md`
