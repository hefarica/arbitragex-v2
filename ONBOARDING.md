# ArbitrageX v2 — Quick start (local)

From zero to live dashboard with real mempool data in ≈ 5 minutes.

## Prerequisites

- Docker + Docker Compose v2
- Node 20+ (only if you plan to run `npm` commands outside of compose)
- A free account with an EVM RPC provider for real mempool detection.
  See **Step 2** below for recommended providers and URL patterns.
  Optional for UI-only sandbox.

## Step 1 — One-shot bootstrap

Brings up the dev stack, waits for Postgres, applies all 18 migrations,
probes every service's `/health`, and tells you exactly what's left to do.

```bash
bash automation/scripts/bootstrap-local.sh
```

Expected tail of output when everything is healthy:

```
  ✓ all 17 expected tables present
  ✓ api-server — http://localhost:8080/health
  ✓ edge — http://localhost:8787/health
  ✓ frontend — http://localhost:5173
  ✓ selector-api — http://localhost:3002/health
  ✓ sim-ctl — http://localhost:3003/health
  ✓ recon — http://localhost:3004/health
  ✓ relays-client — http://localhost:3005/health
  ✓ searcher-rs — http://localhost:9001/health
  ✓ prometheus — http://localhost:9090/-/healthy
  ✓ grafana — http://localhost:3000/api/health
  ✓ alertmanager — http://localhost:9093/-/healthy
```

At this point the operator console at http://localhost:5173 works but every
data-backed page shows an honest empty state — the platform has no RPC yet,
so there are no opportunities to detect. This is expected.

## Step 2 — Attach a real RPC (optional, needed for live detection)

### Recommended EVM RPC providers

| Provider | Free tier | Signup | HTTP URL pattern | WSS URL pattern |
|---|---|---|---|---|
| **Alchemy** | 300M CU/month | [dashboard.alchemy.com](https://dashboard.alchemy.com/) | `https://eth-mainnet.g.alchemy.com/v2/<KEY>` | `wss://eth-mainnet.g.alchemy.com/v2/<KEY>` |
| **Infura** | 100k req/day | [app.infura.io](https://app.infura.io/) | `https://mainnet.infura.io/v3/<KEY>` | `wss://mainnet.infura.io/ws/v3/<KEY>` |
| **Ankr** | 1M req/month | [ankr.com/rpc](https://www.ankr.com/rpc/) | `https://rpc.ankr.com/eth/<KEY>` | `wss://rpc.ankr.com/eth/ws/<KEY>` |

Pick any provider, create a free account, and copy the HTTP + WSS endpoints.

```bash
# 1. Edit .env in the repo root — the key is YOUR credential, never committed:
RPC_HTTP_1=<YOUR_PROVIDER_HTTP_URL>
RPC_WS_1=<YOUR_PROVIDER_WS_URL>

# 2. Restart only the services that need it:
docker compose -f docker/compose.dev.yml restart searcher-rs sim-ctl relays-client

# 3. Confirm the scanner connected to the wire:
docker compose -f docker/compose.dev.yml logs --tail=30 searcher-rs | grep scanner
#    Expect: event="scanner.subscribed" chain_id=1
#    NOT:    event="scanner.no_rpc"  or  event="scanner.idle"

# 4. Confirm detection started:
curl -s http://localhost:9001/metrics | grep 'arbx_opportunity_total'
#    Counter should be incrementing every few seconds.
```

Within ~ 60 s `/opportunities` fills with rows, Grafana "Detection pipeline"
becomes active, and the `arbx_opportunity_total{status="detected"}` counter
climbs.

## Step 3 — Seed non-sensitive catalogs (optional, for `/config` to show relays)

The platform boots fine with empty catalogs. When you want `/config` to show
relays instead of "—", and selector-api to start scoring opportunities, seed
the operator-owned tables. Get your admin token:

```bash
# Your admin token is generated locally and written to .env — doctrine forbids
# the system generating it for you. See configs/secrets.policy.md.
export ARBX_ADMIN_TOKEN=$(grep '^ARBX_ADMIN_TOKEN=' .env | cut -d'=' -f2)
```

### 3a — Add the Flashbots relay (paper-mode, so no keys required yet)

```bash
curl -X POST http://localhost:8080/admin/relays \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{
    "name":"flashbots",
    "chain_id":1,
    "endpoint":"https://relay.flashbots.net",
    "auth_scheme":"x-flashbots-signature",
    "enabled":true,
    "priority":10,
    "notes":"primary relay, paper-mode only"
  }'
```

### 3b — Seed scoring weights (the operator owns this decision)

Values below are a reasonable STARTING POINT; the operator reviews and
signs off in `/config` before flipping paper-mode off.

```bash
docker compose -f docker/compose.dev.yml exec -T postgres \
  psql -U postgres -d arbitragex <<'SQL'
INSERT INTO scoring_weights
  (chain_id, version,
   weight_liquidity, weight_depth, weight_safety,
   weight_slippage,  weight_gas,   weight_risk,
   min_accept_score, description, activated_by)
VALUES
  (1, 1,
   0.20, 0.15, 0.20,
   0.15, 0.15, 0.15,
   55.0, 'initial weights — paper-mode starting point', 'local-operator');
SQL
```

### 3c — Seed execution policy (paper-mode cap = 0 ETH, fail-closed)

```bash
docker compose -f docker/compose.dev.yml exec -T postgres \
  psql -U postgres -d arbitragex <<'SQL'
INSERT INTO execution_policy
  (chain_id, version,
   paper_mode, private_only,
   max_value_eth, max_gas_price_gwei, max_slippage_pct,
   max_parallel_executions,
   description, activated_by)
VALUES
  (1, 1,
   TRUE, TRUE,
   0.0, 200.0, 1.5,
   8,
   'paper-mode bootstrap — zero capital exposure', 'local-operator');
SQL
```

## What's still pending after Step 3

These are all behind credentials the operator provides (progressive
solicitation phases 2–5 in `docs/governance/DATA-MATRIX.md`):

| Pending | Unlocks | Onboarding phase |
|---------|---------|------------------|
| Slack webhook URL | `warning` alerts route to Slack | 2 |
| PagerDuty integration key | `critical` pages on-call | 5 |
| Flashbots signer key (zero-balance) | bundle signing for paper-mode tests | 4 |
| Cloudflare API token + domain | public Worker deploy + Tunnel admin access | 4 |
| Backblaze B2 creds + age pubkey | off-site encrypted backups | 5 |
| Vault seal keys (3-of-5 Shamir) | secrets move out of `.env` file into Vault | 3 |

None of these block "seeing the platform work." They block going to paper
trading in the open and then to real capital.

## Common troubleshooting

- **Frontend shows "edge error"** → `curl http://localhost:8787/health`. If
  it fails, `docker compose -f docker/compose.dev.yml logs edge`.
- **`/opportunities` stays empty even after Step 2** → scanner is probably
  in `scanner.idle` (kill-switch ON) or `scanner.no_rpc` (RPC env not in
  container). Check `docker compose ... logs searcher-rs`.
- **Migration 007 fails with "relation opportunities does not exist"** →
  you applied migrations in the wrong order. `bootstrap-local.sh` enforces
  lexical order; the manual fallback in `migrate.sh` does too.
- **`bootstrap-local.sh` exits 4 with some services down** → ordinary; some
  Rust services take ~ 40 s to build on the first run. Re-run the script;
  it's idempotent.
- **Warning `Function components cannot be given refs` in browser console**
  → stale `.next/` cache. `rm -rf frontend/.next && cd frontend && npm run dev`
  then hard-reload (Ctrl+Shift+R).

## See also

- [`docs/governance/NO-HARDCODE-DOCTRINE.md`](docs/governance/NO-HARDCODE-DOCTRINE.md) — why nothing productive lives in code.
- [`docs/governance/DATA-MATRIX.md`](docs/governance/DATA-MATRIX.md) — every datum the platform needs, per module.
- [`docs/runbooks/`](docs/runbooks/) — what to do when an alert fires.
- [`docs/superpowers/specs/`](docs/superpowers/specs/) — per-sprint designs.
