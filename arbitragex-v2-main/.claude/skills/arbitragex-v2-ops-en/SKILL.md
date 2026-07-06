---
name: arbitragex-v2-ops-en
description: "[EN] Deploy, manage, and operate ArbitrageX-v2 MEV searcher on Hetzner VPS. Includes GitHub access, SSH tunneling, Docker orchestration, cartridge runtime management, and multi-chain strategy deployment. Use for: VPS operations, code deployment, cartridge injection, monitoring, and troubleshooting."
license: "Proprietary - ArbitrageX Team"
---

# ArbitrageX-v2 Operations Skill

Complete operational knowledge for managing the ArbitrageX-v2 MEV searcher infrastructure, including deployment, authentication, and cartridge runtime management.

## Quick Reference

| Component | Access | Status |
|---|---|---|
| **VPS** | SSH: `195.201.235.70` (Hetzner CX43) | Running |
| **GitHub** | `hefarica/arbitragex-v2` | Main branch |
| **Frontend** | `https://edge-arbx.ape-tv.net` | Cloudflare Tunnel |
| **API** | `http://127.0.0.1:8080` (internal) | Docker network |
| **Searcher-RS** | `http://127.0.0.1:9001/health` | Rhai cartridge runtime |

## Authentication

### SSH Access to VPS

**Key Location:** Stored in Manus secrets (ED25519 key)

**Connection:**
```bash
ssh -i ~/.ssh/hetzner_arbx root@195.201.235.70
```

**Key Details:**
- User: `root`
- Host: `195.201.235.70` (Falkenstein, Germany)
- Specs: 8 vCPU, 16 GB RAM, 160 GB SSD
- SSH Key: ED25519 (stored securely)

### GitHub Access

**Token:** Stored in Manus secrets (ghp_...)

**Usage:**
```bash
git remote set-url origin https://<TOKEN>@github.com/hefarica/arbitragex-v2.git
git push origin main
```

**Permissions:** Full repo access (code, actions, secrets)

## Project Structure

```
/opt/arbitragex-v2/
├── backend/
│   ├── searcher-rs/          # Core MEV searcher (Rust + Rhai)
│   │   ├── src/cartridge/    # Cartridge runtime (FASE OMEGA)
│   │   ├── cartridges/       # .rhai strategy files
│   │   └── tests/            # E2E integration tests
│   ├── api-server/           # REST API + WebSocket
│   └── Cargo.toml            # Workspace dependencies
├── frontend/                 # Next.js UI
├── database/                 # PostgreSQL migrations
├── docker/                   # Docker Compose configs
├── edge/                     # Reverse proxy (Node.js)
└── docs/                     # Architecture & guides
```

## Docker Services

All services run via `docker compose -f docker/compose.prod.yml`:

| Service | Port | Status | Purpose |
|---|---|---|---|
| `searcher-rs` | 9001 | Healthy | MEV detection + cartridge runtime |
| `api-server` | 8080 | Healthy | REST API + admin routes |
| `edge` | 8787 | Healthy | Reverse proxy for frontend |
| `frontend` | 5173 | Healthy | Next.js UI |
| `postgres` | 5432 | Healthy | State + cartridge registry |
| `redis` | 6379 | Healthy | PubSub + cache |

## Cartridge Runtime (FASE OMEGA)

### What It Is

Dynamic strategy engine: Rhai scripts (cartridges) execute MEV strategies without recompilation. Supports any EVM chain.

### Key Files

- **Runtime:** `backend/searcher-rs/src/cartridge/runner.rs` (sandboxed Rhai engine)
- **Contract:** `backend/searcher-rs/src/cartridge/contract.rs` (validates required functions)
- **Host Bindings:** `backend/searcher-rs/src/cartridge/host_bindings.rs` (20+ native functions)
- **Cartridges:** `backend/searcher-rs/cartridges/*.rhai` (strategy implementations)

### Required Cartridge Functions

Every cartridge MUST implement:

```rhai
fn init_strategy() {
  // Initialize strategy state
}

fn evaluate_opportunity(opportunity) {
  // Return: { is_opportunity: bool, estimated_profit: f64, confidence: f64, metadata: map }
}

fn build_payload(opportunity) {
  // Return: { tx_data: string, gas_estimate: i64, slippage_bps: i64 }
}
```

### Deploying a Cartridge

1. **Write** the `.rhai` file in `backend/searcher-rs/cartridges/`
2. **Test** locally: `bash backend/searcher-rs/scripts/test_cartridges.sh --quick`
3. **Commit** and push to main
4. **Inject** via API or Redis PubSub

### Chain Support

All cartridges are **chain-agnostic** by default:
- `target_chains: []` = universal (all chains)
- New chains activate cartridges automatically
- No code changes needed

## Deployment Workflow

### 1. Code Changes

```bash
cd /opt/arbitragex-v2
git add .
git commit -m "feat: description"
git push origin main
```

### 2. Rebuild Docker Image

```bash
docker compose -f docker/compose.prod.yml build searcher-rs
```

**Note:** First build takes ~4 minutes (full Rust compilation). Cargo.lock must include all dependencies.

### 3. Restart Service

```bash
docker compose -f docker/compose.prod.yml up -d searcher-rs
```

### 4. Verify Health

```bash
docker exec arbitragex-v2-searcher-rs-1 wget -qO- http://localhost:9001/health
```

Expected: `{"ok":true,"service":"searcher-rs","version":"0.1.0","uptime_s":...}`

## Common Tasks

### Inject a Cartridge via Redis PubSub

```bash
docker exec arbitragex-v2-redis-1 redis-cli PUBLISH cartridge:events '{
  "event_type": "inject",
  "cartridge_id": "my-strategy",
  "actor": "admin",
  "payload": {
    "slug": "my-strategy",
    "name": "My Strategy",
    "version": "1.0.0",
    "author": "Team",
    "category": "dex_arb",
    "target_chains": [],
    "source_code": "fn init_strategy() { ... }"
  }
}'
```

### Check Cartridge Registry (PostgreSQL)

```bash
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c \
  "SELECT slug, name, state, target_chains FROM cartridge_registry;"
```

### View Searcher-RS Logs

```bash
docker logs arbitragex-v2-searcher-rs-1 --tail 50 -f
```

### Rebuild Frontend

```bash
docker compose -f docker/compose.prod.yml build frontend
docker compose -f docker/compose.prod.yml up -d frontend
```

## Troubleshooting

### Build Fails: "Cargo.lock needs to be updated"

**Cause:** New dependencies added but lockfile not regenerated.

**Fix:**
```bash
docker run --rm -v /opt/arbitragex-v2/backend:/build -w /build \
  rust:1.91-slim-bookworm cargo generate-lockfile
```

### Edge Service Returns 404

**Cause:** Cartridge routes not added to edge proxy.

**Fix:** Add routes to `/opt/arbitragex-v2/edge/dev-local/src/index.ts`:
```typescript
app.get("/api/cartridges", (req, res) => adminProxy("/api/v1/cartridges", req, res, "GET"));
app.post("/api/cartridges/inject", (req, res) => adminProxy("/api/v1/cartridges/inject", req, res, "POST"));
```

### Searcher-RS Crashes

**Check logs:**
```bash
docker logs arbitragex-v2-searcher-rs-1 | grep -i error
```

**Common causes:**
- Database connection failure (check `DATABASE_URL`)
- Redis unavailable (check `REDIS_URL`)
- Cartridge compilation error (check `compilation_errors` in DB)

## Environment Variables

**Required in `.env` or docker-compose:**

```bash
DATABASE_URL=postgresql://arbx_rw:PASSWORD@postgres:5432/arbitragex
REDIS_URL=redis://redis:6379
RPC_WS_1=wss://eth-mainnet.g.alchemy.com/v2/KEY
RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/KEY
ARBX_CONFIG_PATH=/app/configs/app.toml
NEXT_PUBLIC_EDGE_URL=https://edge-arbx.ape-tv.net
```

## Monitoring

### Health Checks

- **Searcher-RS:** `curl http://127.0.0.1:9001/health`
- **API Server:** `curl http://127.0.0.1:8080/api/health`
- **Edge:** `curl http://127.0.0.1:8787/health`
- **Frontend:** `curl http://127.0.0.1:5173/`

### Metrics

- Prometheus: `http://127.0.0.1:9090`
- Grafana: `http://127.0.0.1:3000`
- Loki logs: `http://127.0.0.1:3100`

## References

For detailed information, see:

- **Architecture:** Read `references/architecture.md`
- **Cartridge API:** Read `references/cartridge_api.md`
- **Deployment Scripts:** See `scripts/` directory
- **Full Docs:** `/opt/arbitragex-v2/docs/FASE_OMEGA_CARTRIDGE_RUNTIME.md`

## Security Notes

- **SSH Key:** Never commit or expose. Stored in Manus secrets only.
- **GitHub Token:** Has full repo access. Rotate if compromised.
- **Cartridge Sandboxing:** max_ops=1M, max_array=4K, max_string=64K. No imports, no eval().
- **Database:** Passwords in `.env`, not in code. Use read-only roles for non-admin access.

## Support

For issues:
1. Check logs: `docker logs <service-name>`
2. Verify connectivity: `docker exec <service> curl http://localhost:<port>/health`
3. Check database: `docker exec postgres psql -U postgres -d arbitragex -c "SELECT 1;"`
4. Consult `/opt/arbitragex-v2/docs/` for architecture details
