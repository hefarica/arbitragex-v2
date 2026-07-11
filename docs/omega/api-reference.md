# OMEGA Pipeline API Reference

**Last Updated:** 2026-07-11
**Version:** 2.0
**Base URL:** `http://localhost:8787` (edge) / `http://localhost:8080` (api-server)

## Authentication

### Admin Token

Most administrative endpoints require an admin token:

```bash
# Header method (CLI/tools)
curl -H "x-arbx-admin-token: YOUR_ADMIN_TOKEN" \
  http://localhost:8787/admin/killswitch

# Cookie method (browser)
# First establish session:
curl -X POST http://localhost:8787/admin/session \
  -H "Content-Type: application/json" \
  -d '{"token": "YOUR_ADMIN_TOKEN"}'
# Sets httpOnly cookie: arbx_admin_session
```

### Edge Token

Internal service communication uses the edge token:

```bash
curl -H "x-arbx-edge-token: YOUR_EDGE_TOKEN" \
  http://localhost:8080/api/internal/endpoint
```

## Edge Endpoints (Hot Path)

### Health and Status

#### GET /api/v1/health
Fast health check for load balancers.

**Response:**
```json
{
  "ok": true,
  "service": "edge-worker",
  "env": "production"
}
```

**Latency Target:** <10ms

---

#### GET /status
Full system status with upstream health.

**Query Parameters:**
| Name | Type | Description |
|------|------|-------------|
| format | string | `json` for API clients (default if Accept header ambiguous) |

**Response:**
```json
{
  "ok": true,
  "upstreams": {
    "selector-api": {"ok": true, "status": 200},
    "sim-ctl": {"ok": true, "status": 200},
    "recon": {"ok": true, "status": 200},
    "relays-client": {"ok": true, "status": 200},
    "searcher-rs": {"ok": true, "status": 200}
  },
  "kill_switch": {
    "enabled": false,
    "reason": null,
    "triggered_by": null,
    "triggered_at": null
  }
}
```

**Cache:** 2 seconds (KV-backed)

---

### Opportunities

#### GET /api/opportunities/live
Live opportunities stream (cached snapshot).

**Query Parameters:**
| Name | Type | Default | Description |
|------|------|---------|-------------|
| chain_id | integer | 1 | Filter by chain ID |
| limit | integer | 50 | Max results (max 100) |

**Response:**
```json
{
  "opportunities": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "chain_id": 1,
      "strategy_kind": "triangular",
      "token_path": ["0xWETH...", "0xUSDC...", "0xDAI...", "0xWETH..."],
      "expected_profit_usd": 12.50,
      "expected_roi_pct": 0.125,
      "detected_at": "2026-07-11T10:30:00Z",
      "status": "detected"
    }
  ],
  "meta": {
    "total": 150,
    "chain_id": 1,
    "cached_at": "2026-07-11T10:30:05Z"
  }
}
```

**Cache:** 2 seconds

---

### Scanner Operations

#### GET /api/scanner/heartbeat
Pipeline funnel state from searcher-rs.

**Query Parameters:**
| Name | Type | Default | Description |
|------|------|---------|-------------|
| chain_id | integer | 1 | Target chain |

**Response:**
```json
{
  "chain_id": 1,
  "timestamp": "2026-07-11T10:30:00Z",
  "pipeline_latency_ms": 45,
  "funnel": {
    "pending_received": 1000,
    "decoded_ok": 850,
    "enriched": 420,
    "passed_all_gates": 15,
    "persisted_to_pg": 12,
    "rejected_by_gates": {
      "TokenNotAllowed": 200,
      "UnknownTokenPrice": 150,
      "AnomalousMath": 5,
      "Other": 50
    }
  },
  "mempool": {
    "connected": true,
    "tx_per_sec": 15.5,
    "avg_gas_price_gwei": 25.3
  }
}
```

**Cache:** 5 seconds

---

### Readiness and System State

#### GET /api/readiness
17-item readiness checklist.

**Response:**
```json
{
  "overall": "green",
  "checks": [
    {"id": "topology_vault", "status": "green", "detail": "Connected"},
    {"id": "credentials", "status": "green", "detail": "17/17 present"},
    {"id": "market_topology", "status": "yellow", "detail": "3 chains configured, 1 syncing"},
    {"id": "resolution_engines", "status": "green", "detail": "searcher-rs running"}
  ]
}
```

**Cache:** 15 seconds

---

#### GET /api/readiness/blockers
Flat list of blockers preventing go-live.

**Response:**
```json
{
  "blockers": [
    {
      "id": "VPS_IP_EXPOSURE",
      "severity": "high",
      "description": "VPS IP in 23 tracked files",
      "remediation": "Audit and rotate per SECURITY.md"
    }
  ],
  "count": 1
}
```

---

#### GET /api/readiness/decision
Go/no-go verdict with next actions.

**Response:**
```json
{
  "verdict": "go_a5",
  "reason": "All critical checks pass",
  "next_action": "Enable ARBX_DEXSCREENER_ORACLE=active",
  "estimated_eta": "2026-07-11T12:00:00Z"
}
```

---

### Metrics and Analytics

#### GET /api/v1/metrics/entropy
Real-time entropy metrics (latency-optimized).

**Response:**
```json
{
  "entropy_snapshot": {
    "mempool_tx_per_sec": 15.5,
    "mempool_avg_gas_price_gwei": 25.3,
    "mempool_entropy_score": 0.73,
    "reserve_divergence_max": 0.02
  },
  "timestamp": "2026-07-11T10:30:00Z"
}
```

**Latency Target:** <30ms (pass-through, no cache)

---

#### GET /api/operations/kpi
PMI/EVM KPIs for operations dashboard.

**Query Parameters:**
| Name | Type | Default | Description |
|------|------|---------|-------------|
| chain_id | integer | 1 | Filter by chain |

**Response:**
```json
{
  "chain_id": 1,
  "period": "24h",
  "opportunities_detected": 1250,
  "opportunities_simulated": 875,
  "opportunities_archived": 620,
  "avg_profit_usd": 8.45,
  "total_paper_pnl_usd": 5234.50,
  "hit_rate_pct": 49.6
}
```

---

### Admin Endpoints

#### POST /admin/session
Establish admin session via httpOnly cookie.

**Request:**
```json
{
  "token": "YOUR_ADMIN_TOKEN"
}
```

**Response:**
```json
{
  "ok": true,
  "expires_at": 1720693800000
}
```

**Rate Limit:** 5 attempts/minute, lockout after 10 failures

---

#### POST /admin/session/logout
Clear admin session.

**Response:**
```json
{
  "ok": true
}
```

---

#### GET /api/killswitch/status
Current kill-switch state.

**Response:**
```json
{
  "enabled": false,
  "reason": null,
  "triggered_by": null,
  "triggered_at": null,
  "auto_reset_at": null
}
```

---

#### POST /api/killswitch/:action
Activate or deactivate kill-switch.

**URL Parameters:**
| Name | Value | Description |
|------|-------|-------------|
| action | `activate` \| `deactivate` | Kill-switch action |

**Request:**
```json
{
  "reason": "operator_maintenance",
  "triggered_by": "operator"
}
```

**Response:**
```json
{
  "ok": true,
  "enabled": true,
  "triggered_at": "2026-07-11T10:30:00Z"
}
```

---

#### GET /admin/audit
Query audit log.

**Query Parameters:**
| Name | Type | Description |
|------|------|-------------|
| from | ISO8601 | Start timestamp |
| to | ISO8601 | End timestamp |
| action | string | Filter by action type |

**Response:**
```json
{
  "entries": [
    {
      "timestamp": "2026-07-11T10:30:00Z",
      "actor": "operator",
      "action": "killswitch_activate",
      "resource": "killswitch",
      "details": {"reason": "scheduled_maintenance"}
    }
  ],
  "pagination": {
    "total": 150,
    "limit": 100,
    "offset": 0
  }
}
```

---

### Configuration

#### GET /api/config/current
Public configuration snapshot.

**Response:**
```json
{
  "chains": {
    "1": {
      "name": "Ethereum Mainnet",
      "paper_mode": true,
      "enabled_strategies": ["triangular", "dex_arb"],
      "min_profit_usd": 5.0
    }
  },
  "features": {
    "websocket": true,
    "cartridge_forge": true,
    "dexscreener_oracle": false
  }
}
```

**Cache:** 30 seconds

---

#### GET /api/trading-config
Per-chain trading parameters.

**Query Parameters:**
| Name | Type | Default | Description |
|------|------|---------|-------------|
| chain_id | integer | 1 | Target chain |

**Response:**
```json
{
  "chain_id": 1,
  "capital_usd": 10000.0,
  "min_profit_usd": 5.0,
  "max_slippage_pct": 1.0,
  "simulation_capital_usd": 10000.0,
  "caps_per_token": {
    "WETH": 5000.0,
    "USDC": 10000.0
  },
  "caps_per_strategy": {
    "dex_arb": 10000.0,
    "triangular": 2000.0
  }
}
```

---

#### PUT /admin/trading-config/:chain_id
Update trading configuration.

**Request:**
```json
{
  "capital_usd": 15000.0,
  "min_profit_usd": 10.0,
  "caps_per_token": {
    "WETH": 7500.0
  }
}
```

**Response:**
```json
{
  "ok": true,
  "chain_id": "1",
  "applied_at": "2026-07-11T10:30:00Z"
}
```

---

## WebSocket Events

### Connection

```javascript
import { io } from 'socket.io-client';

const socket = io('ws://localhost:8080', {
  auth: {
    token: 'YOUR_ADMIN_TOKEN'
  }
});

socket.on('connect', () => {
  console.log('Connected');
  // Subscribe to rooms
  socket.emit('subscribe:convergence');
  socket.emit('subscribe:opportunities');
});
```

### Rooms

| Room | Subscribe Event | Description |
|------|-----------------|-------------|
| `convergence` | `subscribe:convergence` | Pipeline metrics |
| `opportunities` | `subscribe:opportunities` | New opportunities |
| `telemetry` | `subscribe:telemetry` | Cartridge telemetry |
| `route_discovery` | `subscribe:route_discovery` | Route discovery analytics |
| `runtime_ack` | `subscribe:runtime_ack` | Config change acks |

### Events

#### convergence_signal
Real-time pipeline state.

```json
{
  "entropy_snapshot": {
    "mempool_tx_per_sec": 15.5,
    "mempool_avg_gas_price_gwei": 25.3,
    "mempool_entropy_score": 0.73,
    "reserve_divergence_max": 0.02
  },
  "pipeline_latency_ms": 45,
  "opportunities_detected": 1250,
  "simulations_run": 875,
  "simulations_success": 620,
  "timestamp": "2026-07-11T10:30:00Z",
  "schema_version": 2
}
```

---

#### opportunity
New opportunity detected.

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "chain_id": 1,
  "strategy_kind": "triangular",
  "token_path": ["0xWETH...", "0xUSDC...", "0xDAI...", "0xWETH..."],
  "amounts": ["1000000000000000000", "2500000000", "2495000000", "999000000000000000"],
  "expected_profit_usd": 12.50,
  "expected_roi_pct": 0.125,
  "detected_at": "2026-07-11T10:30:00Z",
  "pools": ["0xPool1...", "0xPool2...", "0xPool3..."]
}
```

---

#### telemetry
Cartridge telemetry message.

```json
{
  "cartridge_id": "spanning_tree_v2",
  "level": "info",
  "message": "Pool index refreshed",
  "ts": "2026-07-11T10:30:00Z",
  "pools_count": 1500
}
```

---

#### runtime_ack
Configuration change acknowledgment.

```json
{
  "event_id": "evt_12345",
  "resource": "trading_config",
  "chain_id": 1,
  "idempotency_key": "key_67890",
  "config_hash_before": "abc123",
  "config_hash_after": "def456",
  "worker_id": "searcher-rs-001",
  "layer": "searcher_rs",
  "status": "applied",
  "latency_ms": 150
}
```

---

## Redis Stream Formats

### arbx:hot:detected

**Producer:** searcher-rs
**Consumers:** paper-executor-g0, ws-emitter-g0
**MAXLEN:** ~10000

**Fields:**
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Opportunity identifier |
| chain_id | integer | Target blockchain |
| strategy_kind | string | `triangular`, `dex_arb`, `backrun`, `liquidation`, `flashloan_arb` |
| token_path | JSON array | Token addresses in path order |
| amounts | JSON array | Input amounts for each hop |
| detected_at_ms | integer | Unix timestamp (milliseconds) |

**Example:**
```redis
XADD arbx:hot:detected MAXLEN ~ 10000 * \
  id 550e8400-e29b-41d4-a716-446655440000 \
  chain_id 1 \
  strategy_kind triangular \
  token_path '["0xWETH...","0xUSDC...","0xDAI...","0xWETH..."]' \
  amounts '["1000000000000000000","2500000000","2495000000","999000000000000000"]' \
  detected_at_ms 1720693800000
```

---

### arbx:hot:simulated

**Producer:** sim-ctl
**Consumers:** archiver-g0
**MAXLEN:** ~5000

**Fields:**
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Reference to original opportunity |
| status | string | `passed` or `failed` |
| net_profit_wei | string | Net topological yield (wei as string) |
| gas_used | integer | Gas consumption estimate |
| timestamp_ms | integer | Simulation completion time |

**Example:**
```redis
XADD arbx:hot:simulated MAXLEN ~ 5000 * \
  id 550e8400-e29b-41d4-a716-446655440000 \
  status passed \
  net_profit_wei 12500000000000000 \
  gas_used 285000 \
  timestamp_ms 1720693800045
```

---

### arbx:hot:paper_executed

**Producer:** api-server (PaperTradeArchiver)
**Consumers:** analytics-g0
**MAXLEN:** ~1000

**Fields:**
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Opportunity identifier |
| execution_time_ms | integer | Duration from detection to archival |
| paper_pnl_usd | float | Paper topological yield in USD |
| status | string | `success`, `failed`, or `rejected` |

**Example:**
```redis
XADD arbx:hot:paper_executed MAXLEN ~ 1000 * \
  id 550e8400-e29b-41d4-a716-446655440000 \
  execution_time_ms 45 \
  paper_pnl_usd 12.50 \
  status success
```

---

### arbx:cartridge:telemetry

**Producer:** searcher-rs cartridges
**Consumers:** api-server WebSocket

**Fields:**
| Field | Type | Description |
|-------|------|-------------|
| cartridge_id | string | Cartridge identifier |
| level | string | `trace`, `debug`, `info`, `warn`, `error` |
| message | string | Log message |
| ts | ISO8601 | Timestamp |

---

### arbx:route_discovery:telemetry

**Producer:** searcher-rs route discovery
**Consumers:** api-server WebSocket

**Fields:**
| Field | Type | Description |
|-------|------|-------------|
| event | string | Event type: `tick`, `route_candidate`, `strategy_applicability`, `rejected`, `route_intent.emitted` |
| ... | varies | Event-specific fields |

---

### arbx:killswitch:changes

**Producer:** KillSwitch API
**Consumers:** All services

**Payload:** `1` (armed) or `0` (disarmed)

---

## Error Responses

### Standard Error Format

```json
{
  "error": "error_code",
  "message": "Human-readable description",
  "detail": "Additional context (optional)"
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `rate_limited` | 429 | Request rate exceeded |
| `locked_out` | 429 | Too many auth failures |
| `missing_admin_token` | 401 | Admin token required |
| `invalid_admin_token` | 401 | Token validation failed |
| `sybil_rejected` | 403 | ASN on deny-list |
| `abuse_rejected` | 403 | High threat score |
| `not_found` | 404 | Resource not found |
| `internal_error` | 500 | Unexpected server error |

---

## Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| General API | 120 | 60 seconds |
| Admin session | 5 | 60 seconds |
| Lockout threshold | 10 failures | 15 minutes |

**Headers:**
- `x-ratelimit-remaining`: Requests remaining
- `x-ratelimit-admin-session-remaining`: Admin session attempts remaining

---

## Related Documentation

- [Pipeline Architecture](./pipeline-architecture.md) - System design
- [Runbook](./runbook.md) - Operational procedures
- [Deployment Guide](./deployment-guide.md) - Environment setup
- [Redis Schema](../redis-schema/hot-path-v2.md) - Detailed stream schemas

---

*Document maintained by OMEGA API Team. Version bumps follow semantic versioning for breaking changes.*
