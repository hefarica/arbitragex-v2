# ArbitrageX-v2 Architecture

## System Overview

ArbitrageX-v2 is a modular MEV searcher built on Rust + Rhai with multi-chain support and dynamic strategy injection.

```
┌─────────────────────────────────────────────────────────────┐
│                    CLOUDFLARE TUNNEL                         │
│              (edge-arbx.ape-tv.net → VPS)                   │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    NGINX REVERSE PROXY                       │
│                   (195.201.235.70:80/443)                   │
├─────────────────────────────────────────────────────────────┤
│  /api/*     → http://127.0.0.1:8787  (Edge)                │
│  /socket.io → http://127.0.0.1:8787  (Edge WebSocket)      │
│  /*         → http://127.0.0.1:5173  (Frontend)            │
└─────────────────────────────────────────────────────────────┘
                              ↓
        ┌─────────────────────┴─────────────────────┐
        ↓                                           ↓
   ┌─────────────┐                         ┌──────────────┐
   │   EDGE      │                         │   FRONTEND   │
   │ (Node.js)   │                         │  (Next.js)   │
   │ Port 8787   │                         │  Port 5173   │
   └─────────────┘                         └──────────────┘
        ↓ (adminProxy)
   ┌─────────────────────────────────────────────────────────┐
   │              API SERVER (Node.js/Express)               │
   │                  Port 8080 (internal)                   │
   │  - REST routes (/api/v1/*)                             │
   │  - WebSocket (/socket.io)                              │
   │  - Admin routes (/admin/*)                             │
   │  - Cartridge Forge (/api/v1/cartridges/*)              │
   └─────────────────────────────────────────────────────────┘
        ↓
   ┌─────────────────────────────────────────────────────────┐
   │           SEARCHER-RS (Rust + Rhai Runtime)             │
   │                  Port 9001 (internal)                   │
   ├─────────────────────────────────────────────────────────┤
   │ Scanner Loop:                                           │
   │  1. Fetch reserves (all chains)                         │
   │  2. Execute cartridges (dynamic strategies)             │
   │  3. Evaluate opportunities                              │
   │  4. Build payloads                                      │
   │  5. Publish to Redis                                    │
   ├─────────────────────────────────────────────────────────┤
   │ Cartridge Runtime (FASE OMEGA):                         │
   │  - Rhai engine (sandboxed)                              │
   │  - Host bindings (20+ native functions)                 │
   │  - Contract validation (3 required functions)           │
   │  - Hot-reload via Redis PubSub                          │
   └─────────────────────────────────────────────────────────┘
        ↓
   ┌─────────────────────────────────────────────────────────┐
   │              INFRASTRUCTURE SERVICES                     │
   ├─────────────────────────────────────────────────────────┤
   │ PostgreSQL (5432):                                      │
   │  - State persistence                                    │
   │  - Cartridge registry                                   │
   │  - Audit logs                                           │
   │  - Metrics                                              │
   │                                                          │
   │ Redis (6379):                                           │
   │  - PubSub (cartridge events, topology)                  │
   │  - Cache (reserves, pool data)                          │
   │  - Session store                                        │
   │                                                          │
   │ Prometheus (9090):                                      │
   │  - Metrics collection                                   │
   │                                                          │
   │ Grafana (3000):                                         │
   │  - Dashboard visualization                              │
   │                                                          │
   │ Loki (3100):                                            │
   │  - Log aggregation                                      │
   └─────────────────────────────────────────────────────────┘
```

## Data Flow: Strategy Execution

```
1. Scanner Loop (searcher-rs)
   ├─ Fetch reserves from all chains
   ├─ Emit "reserves:updated" to Redis
   └─ Trigger cartridge evaluation

2. Cartridge Evaluation (Rhai Runtime)
   ├─ Load compiled cartridge AST
   ├─ Call init_strategy() [once per boot]
   ├─ For each opportunity:
   │  ├─ Call evaluate_opportunity(opp)
   │  ├─ If is_opportunity == true:
   │  │  └─ Call build_payload(opp)
   │  └─ Publish result to Redis
   └─ Record metrics (evaluations, opportunities, errors)

3. Opportunity Publishing
   ├─ Emit "opportunities:new" to Redis
   ├─ Store in PostgreSQL (audit)
   └─ Alert API Server (WebSocket)

4. Payload Execution (external)
   ├─ Receive payload from Redis
   ├─ Simulate on-chain
   ├─ Submit to mempool
   └─ Record result (success/failure)
```

## Cartridge Runtime Details

### Sandboxing

- **max_operations:** 1,000,000 (prevents infinite loops)
- **max_array_size:** 4,096 (prevents memory bombs)
- **max_string_size:** 65,536 bytes
- **Forbidden:** imports, eval(), filesystem, network (except via host bindings)

### Host Bindings (Native Functions)

Available to cartridges:

```rhai
// Reserve data
get_reserves(chain_id) → [{ token, amount, price }]
get_token_metadata(chain_id, token_address) → { decimals, symbol, name }

// Pool data
get_pool_index(chain_id, pool_address) → { dex, tokens, fee }
get_pool_reserves(chain_id, pool_address) → { token0, token1, reserve0, reserve1 }

// Math utilities
calculate_output_amount(input, reserve_in, reserve_out) → f64
calculate_price_impact(input, reserve_in, reserve_out) → f64
sqrt(x) → f64
pow(x, y) → f64

// Logging
log_info(message)
log_warn(message)
log_error(message)

// Chain info
get_chain_id() → i64
get_chain_name() → string
get_block_number() → i64
```

### Contract Validation

Every cartridge must export:

```rhai
// Initialize strategy (called once at boot)
fn init_strategy() { }

// Evaluate if opportunity exists (called per opportunity)
fn evaluate_opportunity(opportunity) → {
  is_opportunity: bool,
  estimated_profit: f64,
  confidence: f64,
  metadata: map
}

// Build transaction payload (called if is_opportunity == true)
fn build_payload(opportunity) → {
  tx_data: string,
  gas_estimate: i64,
  slippage_bps: i64
}
```

## Multi-Chain Support

### Chain Configuration

Each chain has:
- RPC endpoints (HTTP + WebSocket)
- Pool factories (Uniswap V2/V3, Curve, etc.)
- Token registry
- Gas price oracle

### Cartridge Chain Agnosticism

Cartridges specify `target_chains`:

```json
{
  "slug": "dex-arb",
  "target_chains": [],  // Empty = all chains
  "source_code": "..."
}
```

When a new chain is added:
1. Cartridge automatically activates
2. Scanner evaluates it for the new chain
3. No code changes needed

## Hot-Reload Pattern

### Redis PubSub Channels

**`cartridge:events`** - Cartridge lifecycle events

```json
{
  "event_type": "inject|update|pause|resume|remove",
  "cartridge_id": "dex-arb",
  "actor": "admin|deploy-script",
  "payload": { ... }
}
```

**`topology:updated`** - Chain/pool topology changes

```json
{
  "event_type": "chain_added|chain_removed|pool_added",
  "chain_id": 1,
  "details": { ... }
}
```

### Subscriber Flow

```
1. CartridgeSubscriber listens to cartridge:events
2. Receives inject event
3. Validates cartridge contract
4. Compiles Rhai AST
5. Stores in CartridgeRegistry (PostgreSQL)
6. Caches in memory (HashMap<String, CompiledCartridge>)
7. Scanner picks up on next evaluation cycle
```

## Database Schema

### cartridge_registry

```sql
id              uuid PRIMARY KEY
slug            text UNIQUE NOT NULL
name            text NOT NULL
version         text NOT NULL (default '1.0.0')
author          text NOT NULL
description     text
category        text (default 'custom')
source_code     text NOT NULL
content_hash    text UNIQUE NOT NULL
target_chains   jsonb (default '[]')
state           enum ('active', 'paused', 'archived')
min_eval_interval_ms  int (default 100)
created_by      text (default 'system')
created_at      timestamp
updated_at      timestamp
last_compiled_at  timestamp
compilation_errors  text
total_evaluations  bigint (default 0)
total_opportunities  bigint (default 0)
total_errors    bigint (default 0)
last_evaluation_at  timestamp
```

### cartridge_audit_log

```sql
id              uuid PRIMARY KEY
cartridge_id    uuid FOREIGN KEY
event_type      text (inject|update|pause|resume|remove)
actor           text
old_state       jsonb
new_state       jsonb
created_at      timestamp
```

### cartridge_metrics_hourly

```sql
id              uuid PRIMARY KEY
cartridge_id    uuid FOREIGN KEY
hour            timestamp
evaluations     bigint
opportunities   bigint
errors          bigint
avg_execution_ms  float
```

## Deployment Pipeline

```
1. Developer commits to main
   └─ GitHub Actions trigger

2. Build stage
   ├─ Compile Rust (searcher-rs)
   ├─ Build Node.js (api-server, edge, frontend)
   └─ Generate Docker images

3. Push to registry
   └─ Tag with commit SHA

4. Deploy to VPS
   ├─ docker compose pull
   ├─ docker compose build
   ├─ docker compose up -d
   └─ Health checks

5. Verification
   ├─ Searcher-RS health: /health endpoint
   ├─ API Server health: /api/health
   ├─ Frontend: /
   └─ Cartridge registry: SELECT COUNT(*) FROM cartridge_registry
```

## Performance Characteristics

| Component | Throughput | Latency | Notes |
|---|---|---|---|
| Cartridge eval | 1000 ops/s | 1-10ms | Per cartridge, per opportunity |
| Reserve fetch | 100 chains/s | 50-200ms | Parallel RPC calls |
| Opportunity publish | 10K ops/s | <1ms | Redis PubSub |
| DB query | 1000 ops/s | 5-50ms | Connection pooled |
| Rhai AST compile | 1-10 cartridges/s | 100-500ms | One-time at injection |

## Security Model

### Authentication

- **Admin routes:** Session token (httpOnly cookie) or `X-ArbX-Admin-Token` header
- **WebSocket:** Token via `sec-websocket-protocol` header or query param
- **API:** Public routes (no auth), admin routes (gated)

### Authorization

- **Admin:** Full access to config, credentials, cartridges
- **Operator:** Read-only access to metrics, logs
- **Public:** Read-only access to opportunities, status

### Cartridge Isolation

- **Sandboxed execution:** No filesystem, network, or import access
- **Operation limits:** 1M max ops per evaluation
- **Memory limits:** 4K array size, 64K string size
- **Timeout:** 10s per evaluation (configurable)

## Monitoring & Observability

### Metrics Exported

- `searcher_cartridge_evaluations_total` (counter)
- `searcher_cartridge_opportunities_total` (counter)
- `searcher_cartridge_errors_total` (counter)
- `searcher_cartridge_execution_ms` (histogram)
- `searcher_reserves_fetch_ms` (histogram)
- `searcher_scanner_cycle_ms` (histogram)

### Logs

- **Level:** info, warn, error, debug
- **Format:** JSON (structured)
- **Aggregation:** Loki
- **Retention:** 30 days

### Alerts

- Cartridge compilation error
- Cartridge error rate > 5%
- Scanner cycle time > 5s
- Reserve fetch timeout
- Database connection failure
