# Tutorial: Getting Started with ArbitrageX v2

This tutorial walks you through the complete setup of ArbitrageX v2 on your local machine. By the end, you will have all 21 containers running in Paper mode, the dashboard accessible in your browser, and a verified health status across every service.

**Estimated time:** 15 minutes
**Prerequisites:** Linux, macOS, or WSL2 on Windows

---

## What You Will Accomplish

- Clone the ArbitrageX v2 repository
- Install Docker and Docker Compose if needed
- Configure environment variables for local Paper mode
- Start all 21 containers
- Verify each container reports healthy
- Access the web dashboard
- Run your first health-check command

---

## Step 1: Prerequisites

ArbitrageX v2 requires the following tools on your system. Verify each before proceeding.

### Docker Engine

Docker Engine 24.0 or newer with BuildKit support:

```bash
docker --version
# Expected: Docker version 24.0.x or newer
docker buildx version
# Expected: github.com/docker/buildkit v0.12.x or newer
```

If Docker is not installed, follow the [official installation guide](https://docs.docker.com/engine/install/) for your platform.

### Docker Compose

Docker Compose v2 (plugin format) is required:

```bash
docker compose version
# Expected: Docker Compose version v2.20.x or newer
```

The legacy `docker-compose` (v1) command is not supported.

### Git

```bash
git --version
# Expected: git version 2.40.x or newer
```

### System Resources

| Resource | Minimum | Recommended |
|----------|---------|-------------|
| CPU Cores | 4 | 8 |
| RAM | 8 GB | 16 GB |
| Disk (SSD) | 20 GB free | 50 GB free |
| Network | Broadband | Low-latency (< 50ms to RPC) |

### Ports

The following ports must be free on your host machine:

| Port | Service | Purpose |
|------|---------|---------|
| 3000 | REST API | Programmatic access |
| 3001 | Grafana | Metrics dashboards |
| 5432 | PostgreSQL | Database access |
| 6379 | Redis | Cache and pub/sub |
| 8080 | WebSocket Gateway | Real-time stream |
| 9090 | Prometheus | Metrics scraping |
| 16686 | Jaeger | Distributed tracing UI |
| 50051 | gRPC | Control plane |

If any port is in use, remap it in `docker-compose.override.yml` after cloning.

---

## Step 2: Clone the Repository

Clone the ArbitrageX v2 monorepo and navigate into it:

```bash
git clone https://github.com/arbitragex/arbitragex-v2.git
cd arbitragex-v2
```

The repository structure follows a monorepo layout:

```
arbitragex-v2/
├── Cargo.toml                 # Rust workspace root
├── docker-compose.yml         # 21-container orchestration
├── .env.example               # Environment variable template
├── contracts/                 # Foundry Solidity contracts
│   ├── src/
│   └── test/
├── crates/                    # Rust workspace crates
│   ├── ax-gw-websocket/
│   ├── ax-api-rest/
│   ├── ax-strategy-eval/
│   ├── ax-opportunity/
│   ├── ax-ghost-protocol/
│   ├── ax-evm-exec/
│   ├── ax-bundle-forge/
│   ├── ax-mempool-watcher/
│   ├── ax-rpc-router/
│   ├── ax-quoting-engine/
│   ├── ax-risk-guard/
│   └── ax-telemetry/
├── web/                       # Next.js dashboard
│   ├── app/
│   └── components/
├── e2e/                       # Playwright E2E tests
└── docs/                      # This documentation
```

---

## Step 3: Environment Configuration

Copy the example environment file and review the settings:

```bash
cp .env.example .env
```

For local Paper mode operation, the defaults in `.env.example` are sufficient. Open `.env` and confirm these critical settings:

```bash
# Mode configuration
AX_MODE=paper              # Paper mode — no real transactions
AX_CHAIN=ethereum          # Primary chain target
AX_RPC_PRIMARY=""          # Leave empty for dev; uses devnet fallback

# Database
DATABASE_URL=postgresql://ax_user:ax_pass@ax-postgres:5432/arbitragex

# Redis
REDIS_URL=redis://ax-redis-primary:6379

# Logging
RUST_LOG=info
NEXT_TELEMETRY_DISABLED=1
```

> **Note on Paper Mode:** `AX_MODE=paper` ensures the Ghost Protocol intercepts all execution paths and simulates transactions without broadcasting them to the network. This is the safest default for learning and development.

---

## Step 4: Build and Start All Containers

Build all container images and start the full stack:

```bash
docker compose build
```

This build step compiles the Rust workspace, the Next.js frontend, and the Foundry contract artifacts. Initial compilation may take 5–10 minutes depending on your hardware.

Once the build completes, start all services:

```bash
docker compose up -d
```

The `-d` flag runs containers in detached mode. You will see output indicating each container is created and started:

```
[+] Running 21/21
 ✔ Container ax-redis-primary     Started
 ✔ Container ax-postgres          Started
 ✔ Container ax-grpc-control      Started
 ✔ Container ax-strategy-eval     Started
 ✔ Container ax-opportunity       Started
 ✔ Container ax-ghost-protocol    Started
 ✔ Container ax-evm-exec-1        Started
 ✔ Container ax-evm-exec-2        Started
 ✔ Container ax-evm-exec-3        Started
 ✔ Container ax-bundle-forge      Started
 ... (remaining containers)
```

---

## Step 5: Health Checks

### 5.1 Check Container Status

Verify all 21 containers are running:

```bash
docker compose ps
```

Expected output shows all services with status `Up` and health state `healthy`:

```
NAME                 STATUS          PORTS
ax-redis-primary     Up 30s (healthy)  0.0.0.0:6379->6379/tcp
ax-postgres          Up 30s (healthy)  0.0.0.0:5432->5432/tcp
ax-api-rest          Up 30s (healthy)  0.0.0.0:3000->3000/tcp
ax-strategy-eval     Up 30s (healthy)
ax-ghost-protocol    Up 30s (healthy)
... (all 21 healthy)
```

### 5.2 Check Service Logs

If any container is not healthy, inspect its logs:

```bash
docker compose logs --tail=50 <container-name>
```

Example — check the strategy evaluator:

```bash
docker compose logs --tail=30 ax-strategy-eval
```

You should see log lines indicating successful initialization:

```
ax-strategy-eval  | [INFO  ax_strategy_eval] Strategy evaluator starting
ax-strategy-eval  | [INFO  ax_strategy_eval] Loaded 8 active strategies
ax-strategy-eval  | [INFO  ax_strategy_eval] Connected to Redis at redis://ax-redis-primary:6379
ax-strategy-eval  | [INFO  ax_strategy_eval] Listening for opportunities on channel: ax.ops
```

### 5.3 API Health Endpoint

The REST API exposes a health check endpoint:

```bash
curl http://localhost:3000/health
```

Expected response:

```json
{
  "status": "healthy",
  "version": "2.0.0",
  "mode": "paper",
  "containers": {
    "total": 21,
    "healthy": 21,
    "degraded": 0
  },
  "services": {
    "postgres": "connected",
    "redis": "connected",
    "grpc": "connected"
  }
}
```

### 5.4 WebSocket Connection Test

Verify the WebSocket gateway is accepting connections:

```bash
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Key: <example-websocket-key>" \
  -H "Sec-WebSocket-Version: 13" \
  http://localhost:8080/ws
```

You should receive a `101 Switching Protocols` response.

---

## Step 6: Access the Dashboard

Open your browser and navigate to:

```
http://localhost:3000
```

The ArbitrageX v2 dashboard loads with the **Paper Mode** banner visible at the top of the page, confirming no real capital is at risk.

### Dashboard Sections

| Section | URL | Description |
|---------|-----|-------------|
| Overview | `http://localhost:3000` | System status, recent opportunities, P&L summary |
| Strategies | `/strategies` | Active strategy list with performance metrics |
| Trades | `/trades` | Paper trade history with execution details |
| Mempool | `/mempool` | Live mempool visualization |
| Metrics | `http://localhost:3001` | Grafana dashboards (external) |
| Traces | `http://localhost:16686` | Jaeger trace explorer (external) |

---

## Step 7: Run a Quick Verification

Execute the built-in verification script that exercises all critical paths:

```bash
./scripts/verify-local.sh
```

This script performs the following checks:

1. Pings each container's health endpoint
2. Verifies database connectivity and migration status
3. Confirms Redis pub/sub channels are active
4. Submits a test paper trade through the full pipeline
5. Validates metrics are flowing to Prometheus

Expected output:

```
[verify] Checking container health ... OK (21/21)
[verify] Checking database migrations ... OK (v2.0.0)
[verify] Checking Redis channels ... OK (4 channels)
[verify] Submitting paper trade ... OK (op_id: ax-paper-abc123)
[verify] Checking Prometheus metrics ... OK (142 metrics)
[verify] All checks passed. System ready.
```

---

## Common Issues

### Port Already in Use

If a port conflict occurs, create `docker-compose.override.yml`:

```yaml
services:
  ax-api-rest:
    ports:
      - "3002:3000"  # Remap host port 3000 -> 3002
```

### Build Fails on Low Memory

If the Rust build fails with out-of-memory errors, limit parallel jobs:

```bash
CARGO_BUILD_JOBS=2 docker compose build
```

### PostgreSQL Permission Errors

If PostgreSQL fails to start with permission errors, fix the data directory:

```bash
sudo chown -R 999:999 ./data/postgres
docker compose up -d ax-postgres
```

---

## Next Steps

Your local ArbitrageX v2 instance is running in Paper mode. Continue to [Tutorial 2: Your First Paper Trade](02-first-paper-trade.md) to learn how to read opportunity data, understand the Ghost Protocol output, and interpret paper trade results.
