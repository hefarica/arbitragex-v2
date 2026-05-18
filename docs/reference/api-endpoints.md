# API Reference

> **Document Type**: Reference (Diátaxis Framework)
>
> Reference documents provide precise, factual information — the "what" and "how" without the "why". They are optimized for quick lookup. For background context, see the Explanation documents. For step-by-step instructions, see the How-To guides.

## Base URL

| Environment | URL |
|------------|-----|
| Production (VPS) | `http://195.201.235.70:8080` |
| Edge (CF Worker) | `https://api.arbitragex.io` |
| Local dev | `http://localhost:8080` |

## Authentication

### Admin Token

Used for all `/admin/*` endpoints. Pass via `Authorization: Bearer` header.

```bash
curl -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" \
  http://localhost:8080/admin/config
```

Token validation:
- Minimum 32 bytes of entropy
- Compared in constant time to prevent timing attacks
- Rate limited to 30 requests per minute per IP on `/admin/*`

### Edge Token

Used for the `/internal/audit/auth` endpoint. Reserved for the Cloudflare Worker edge.

### Service Token

Used for inter-service communication (`POST /api/system/runtime-ack`). Pass via `x-arbx-service-token` header.

## Endpoint Catalog

### Health & Status

#### `GET /health`

API server health check.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | None |
| **Cache** | None |

**Response** (200 OK):
```json
{
  "status": "ok",
  "service": "api-server",
  "version": "0.1.0",
  "started_at": "2026-05-17T10:00:00Z",
  "uptime_seconds": 86400
}
```

**Error response** (503):
```json
{
  "status": "error",
  "service": "api-server",
  "detail": "database unreachable"
}
```

**Usage notes**: Called by Docker health checks, load balancers, and Prometheus blackbox exporter.

---

#### `GET /api/health`

Alias for `/health` — conforms to REST convention for `/api/*` prefixed health checks.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | None |
| **Cache** | None |

**Response**: Identical to `GET /health`.

---

#### `GET /metrics`

Prometheus metrics endpoint.

| | |
|---|---|
| **Auth** | None (binds to localhost only in prod) |
| **Rate limit** | None |
| **Cache** | None |

**Response** (200 OK, `text/plain`):
```
# HELP arbx_http_requests_total Total HTTP requests
# TYPE arbx_http_requests_total counter
arbx_http_requests_total{service="api-server",method="GET",path="/health",status="200"} 1234

# HELP arbx_http_request_duration_seconds HTTP request duration
# TYPE arbx_http_request_duration_seconds histogram
arbx_http_request_duration_seconds_bucket{service="api-server",le="0.01"} 987
arbx_http_request_duration_seconds_bucket{service="api-server",le="0.05"} 1200
arbx_http_request_duration_seconds_bucket{service="api-server",le="+Inf"} 1234
```

---

#### `GET /status`

Public read-only snapshot of system health and kill-switch state.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | None |
| **Cache** | None |

**Response** (200 OK):
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
  },
  "timestamp": "2026-05-17T14:30:00Z"
}
```

**Response header** (when kill-switch is armed):
```
x-arbx-system-guard: ARMED — 2026-05-17T14:32:00Z — High revert rate detected
```

---

#### `GET /api/v1/readiness`

Comprehensive readiness checklist with 17 dynamic checks.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | None |
| **Cache** | 5 seconds (bypass with `?force=true`) |

**Query parameters**:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `force` | boolean | `false` | Bypass cache, re-run all checks |

**Response** (200 OK):
```json
{
  "score": 17,
  "total": 17,
  "percentage": 100,
  "timestamp": "2026-05-17T14:30:00Z",
  "checks": [
    { "id": "v-db-1", "name": "Database connectivity", "passed": true, "latency_ms": 2 },
    { "id": "v-db-2", "name": "Database migrations current", "passed": true, "latency_ms": 5 },
    { "id": "v-db-3", "name": "Connection pool healthy", "passed": true, "latency_ms": 1 },
    { "id": "v-redis-1", "name": "Redis connectivity", "passed": true, "latency_ms": 1 },
    { "id": "v-redis-2", "name": "Redis pub/sub", "passed": true, "latency_ms": 2 },
    { "id": "g-rpc-1", "name": "RPC primary reachable", "passed": true, "latency_ms": 45 },
    { "id": "g-rpc-2", "name": "RPC failover reachable", "passed": true, "latency_ms": 52 },
    { "id": "g-tok-1", "name": "Token enricher healthy", "passed": true, "latency_ms": 8 },
    { "id": "g-sim-1", "name": "Simulator healthy", "passed": true, "latency_ms": 12 },
    { "id": "g-pap-1", "name": "Paper mode configured", "passed": true, "latency_ms": 1 },
    { "id": "g-pec-1", "name": "Per-chain paper mode", "passed": true, "latency_ms": 2 },
    { "id": "v-at-1", "name": "Audit log writable", "passed": true, "latency_ms": 3 },
    { "id": "g-fl-1", "name": "Simulation pipeline", "passed": true, "latency_ms": 15 },
    { "id": "v-mon-1", "name": "Prometheus scraping", "passed": true, "latency_ms": 20 },
    { "id": "v-mon-2", "name": "Grafana accessible", "passed": true, "latency_ms": 8 },
    { "id": "runbook", "name": "Operator runbook available", "passed": true, "latency_ms": 0 },
    { "id": "m5-maturity", "name": "Doctrinal maturity gate", "passed": true, "latency_ms": 0 }
  ]
}
```

**Response header**:
```
x-arbx-cache: MISS   # or HIT if served from cache
```

---

### Kill-Switch

#### `POST /admin/killswitch`

Toggle the global kill-switch. Every toggle is recorded in the audit log.

| | |
|---|---|
| **Auth** | Admin token (Bearer) |
| **Rate limit** | 30 req/min/IP |
| **Cache** | None |

**Request body**:
```json
{
  "enabled": true,
  "reason": "High revert rate — investigating strategy S-0042",
  "triggered_by": "operator:john-doe"
}
```

**Field reference**:

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| `enabled` | boolean | Yes | `true` or `false` | `true` = arm, `false` = disarm |
| `reason` | string | No | Max 500 chars | Human-readable explanation for the action |
| `triggered_by` | string | No | Max 200 chars | Defaults to `x-arbx-actor` header or `"admin"` |

**Request headers**:
```
Authorization: Bearer <ARBX_ADMIN_TOKEN>
Content-Type: application/json
x-arbx-actor: operator:john-doe
```

**Response** (200 OK):
```json
{
  "enabled": true,
  "reason": "High revert rate — investigating strategy S-0042",
  "triggered_by": "operator:john-doe",
  "updated_at": "2026-05-17T14:32:00Z"
}
```

**Error responses**:

| Status | Code | Description |
|--------|------|-------------|
| 400 | `invalid_request` | Request body failed Zod validation |
| 401 | `unauthorized` | Missing or invalid admin token |
| 429 | `rate_limited` | Exceeded 30 req/min; retry after 60s |

---

#### `GET /admin/config`

Read current effective configuration (secrets redacted).

| | |
|---|---|
| **Auth** | Admin token (Bearer) |
| **Rate limit** | 30 req/min/IP |
| **Cache** | None |

**Response** (200 OK):
```json
{
  "system": {
    "kill_switch_enabled_default": true,
    "paper_mode_default": true
  },
  "risk": {
    "max_exposure_eth": 10.0,
    "max_slippage_pct": 0.5,
    "min_profit_bps": 50
  },
  "execution": {
    "paper_mode": true,
    "paper_mode_per_chain": {
      "1": { "enabled": true, "source": "per_chain" },
      "137": { "enabled": true, "source": "per_chain" },
      "42161": { "enabled": true, "source": "per_chain" }
    },
    "paper_mode_all_chains_in_paper": true
  },
  "observability": {
    "log_level": "info",
    "metrics_enabled": true
  },
  "chains": [
    { "chain_id": 1, "name": "ethereum", "enabled": true },
    { "chain_id": 137, "name": "polygon", "enabled": true },
    { "chain_id": 42161, "name": "arbitrum", "enabled": true }
  ],
  "relays": {
    "flashbots": { "enabled": true, "priority": 1 },
    "bloxroute": { "enabled": false, "priority": 2 }
  },
  "scoring": {
    "profit_weight": 0.35,
    "liquidity_weight": 0.25,
    "safety_weight": 0.20,
    "historical_weight": 0.15,
    "speed_weight": 0.05
  },
  "token_safety": {
    "goplus_enabled": true,
    "min_score": 70
  },
  "circuit_breakers": {
    "revert_rate": { "threshold": 5.0, "window_sec": 60 },
    "relay_timeout": { "threshold": 3, "window_sec": 30 }
  }
}
```

---

### Opportunities

#### `GET /api/v1/scanner/heartbeat`

Opportunity detection heartbeat — returns recent activity metrics.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | 60 req/min/IP |
| **Cache** | 10 seconds |

**Response** (200 OK):
```json
{
  "timestamp": "2026-05-17T14:30:00Z",
  "window_seconds": 60,
  "opportunities_detected": 12,
  "opportunities_filtered": 8,
  "opportunities_scored": 4,
  "opportunities_simulated": 3,
  "opportunities_submitted": 0,
  "detection_rate_per_min": 12,
  "last_opportunity_at": "2026-05-17T14:29:45Z"
}
```

---

### Executions

#### `GET /api/v1/executions/recent`

Recent execution history.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | 60 req/min/IP |
| **Cache** | 5 seconds |

**Query parameters**:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 20 | Max 100 |
| `status` | string | — | Filter: `simulated`, `reverted`, `submitted`, `included` |
| `strategy` | string | — | Filter by strategy ID |

**Response** (200 OK):
```json
{
  "items": [
    {
      "id": "exec-uuid-1",
      "strategy_id": "S-0042",
      "chain_id": 1,
      "status": "simulated",
      "profit_eth": "0.0012",
      "gas_used": 145000,
      "gas_price_gwei": 25,
      "tokens_in": [{"address": "0x...", "symbol": "WETH", "amount": "1.0"}],
      "tokens_out": [{"address": "0x...", "symbol": "USDC", "amount": "1850.0"}],
      "pools": ["0x8ad599c..."],
      "relay": null,
      "paper_mode": true,
      "created_at": "2026-05-17T14:29:30Z"
    }
  ],
  "total": 142,
  "limit": 20,
  "offset": 0
}
```

---

### Risk

#### `GET /api/v1/risk/alerts`

Active risk alerts and circuit breaker states.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | 60 req/min/IP |
| **Cache** | 5 seconds |

**Response** (200 OK):
```json
{
  "alerts": [
    {
      "id": "alert-1",
      "severity": "warning",
      "type": "high_revert_rate",
      "message": "Revert rate at 3.2% — approaching threshold of 5%",
      "created_at": "2026-05-17T14:25:00Z"
    }
  ],
  "circuit_breakers": [
    { "name": "revert_rate", "state": "closed", "value": 0 },
    { "name": "relay_timeout", "state": "closed", "value": 0 }
  ],
  "kill_switch": {
    "enabled": false,
    "reason": null
  },
  "risk_score": 25
}
```

---

### Recon

#### `GET /api/v1/recon/summary`

Reconciliation summary for the current period.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | 60 req/min/IP |
| **Cache** | 10 seconds |

**Response** (200 OK):
```json
{
  "period": "24h",
  "opportunities": { "detected": 1240, "scored": 480, "simulated": 320, "submitted": 0 },
  "executions": { "simulated": 320, "reverted": 4, "submitted": 0, "included": 0 },
  "simulation_success_rate": 98.75,
  "revert_rate": 1.23,
  "profit_hypothetical_eth": 0.42,
  "avg_gas_cost_gwei": 24.5,
  "top_strategy": "S-0042",
  "updated_at": "2026-05-17T14:30:00Z"
}
```

---

#### `GET /api/v1/recon/timeseries`

Time-series data for Grafana or custom dashboards.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | 30 req/min/IP |
| **Cache** | 5 seconds |

**Query parameters**:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `hours` | integer | 24 | Range: 1-168 (1 hour to 7 days) |
| `bucket` | string | `5m` | One of: `1m`, `5m`, `15m`, `1h` |
| `metric` | string | `opportunities` | One of: `opportunities`, `executions`, `profit`, `revert_rate` |

**Response** (200 OK):
```json
{
  "metric": "opportunities",
  "bucket": "5m",
  "points": [
    { "timestamp": "2026-05-17T14:00:00Z", "value": 12 },
    { "timestamp": "2026-05-17T14:05:00Z", "value": 15 },
    { "timestamp": "2026-05-17T14:10:00Z", "value": 8 }
  ]
}
```

---

### Relays

#### `GET /api/v1/relays`

Public relay configuration and status.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | 60 req/min/IP |
| **Cache** | 30 seconds |

**Response** (200 OK):
```json
{
  "relays": [
    {
      "id": "flashbots",
      "name": "Flashbots Protect",
      "enabled": true,
      "priority": 1,
      "endpoint": "https://relay.flashbots.net",
      "status": "healthy",
      "last_success_at": "2026-05-17T14:25:00Z"
    },
    {
      "id": "bloxroute",
      "name": "BloXroute",
      "enabled": false,
      "priority": 2,
      "status": "disabled",
      "last_success_at": null
    }
  ]
}
```

---

### Onboarding

#### `GET /api/v1/onboarding/status`

Current onboarding step and completion status.

| | |
|---|---|
| **Auth** | None |
| **Rate limit** | 60 req/min/IP |
| **Cache** | None |

**Response** (200 OK):
```json
{
  "current_step": 1,
  "total_steps": 8,
  "step_name": "vault_init",
  "completed_steps": [],
  "required_actions": [
    "Initialize Vault",
    "Unseal Vault with 3 of 5 keys"
  ],
  "next_step_url": "/admin/onboarding/1/complete"
}
```

---

### Admin — Audit

#### `GET /admin/audit`

Query the audit log with filtering and pagination.

| | |
|---|---|
| **Auth** | Admin token (Bearer) |
| **Rate limit** | 30 req/min/IP |
| **Cache** | None |

**Query parameters**:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `action` | string | — | Filter by action type |
| `actor` | string | — | Filter by actor |
| `from` | string | — | ISO 8601 start date |
| `to` | string | — | ISO 8601 end date |
| `limit` | integer | 50 | Max 500 |
| `offset` | integer | 0 | Pagination offset |

**Response** (200 OK):
```json
{
  "items": [
    {
      "id": 42,
      "action": "killswitch.armed",
      "actor": "operator:john-doe",
      "target_kind": "killswitch",
      "target_id": "global",
      "before_state": { "enabled": false },
      "after_state": { "enabled": true, "reason": "High revert rate" },
      "ip_address": "203.0.113.1",
      "trace_id": "uuid-trace-1",
      "created_at": "2026-05-17T14:32:00Z"
    }
  ],
  "total": 156,
  "limit": 50,
  "offset": 0
}
```

---

### Admin — Blacklist

#### `GET /admin/blacklist/tokens`

List blacklisted token contracts.

| | |
|---|---|
| **Auth** | Admin token (Bearer) |
| **Rate limit** | 30 req/min/IP |

**Response** (200 OK):
```json
{
  "tokens": [
    {
      "chain_id": 1,
      "address": "0xBadToken...",
      "reason": "honeypot",
      "added_at": "2026-05-01T10:00:00Z",
      "added_by": "operator:jane-doe"
    }
  ]
}
```

#### `POST /admin/blacklist/tokens`

Add a token to the blacklist.

**Request body**:
```json
{
  "chain_id": 1,
  "address": "0xBadTokenAddress",
  "reason": "honeypot detected"
}
```

#### `DELETE /admin/blacklist/tokens/:chain/:addr`

Remove a token from the blacklist.

---

### Admin — Circuit Breakers

#### `GET /admin/circuit_breakers`

List all circuit breakers and their current states.

| | |
|---|---|
| **Auth** | Admin token (Bearer) |
| **Rate limit** | 30 req/min/IP |

**Response** (200 OK):
```json
{
  "breakers": [
    { "name": "revert_rate", "state": "closed", "threshold": 5.0, "current_value": 1.2 },
    { "name": "relay_timeout", "state": "closed", "threshold": 3, "current_value": 0 }
  ]
}
```

#### `POST /admin/circuit_breakers/:name/trip`

Manually trip a circuit breaker.

#### `POST /admin/circuit_breakers/:name/reset`

Manually reset a circuit breaker.

---

### Admin — Paper Mode

#### `POST /admin/config/paper-mode`

Toggle paper mode per chain.

**Request body**:
```json
{
  "chain_id": 1,
  "enabled": true,
  "reason": "Investigating mainnet opportunity discrepancies"
}
```

---

### WebSocket

The API server exposes a WebSocket gateway for real-time opportunity updates.

#### Connection

```javascript
const socket = io('ws://195.201.235.70:8080');

socket.on('connect', () => {
  console.log('Connected to ArbitrageX WSS');
});

socket.on('opportunity', (opp) => {
  console.log('New opportunity:', opp);
});

socket.on('ghost_execution', (exec) => {
  console.log('Ghost execution:', exec);
});

socket.on('runtime_ack', (ack) => {
  console.log('Runtime acknowledgment:', ack);
});
```

#### Events

| Event | Direction | Description |
|-------|-----------|-------------|
| `opportunity` | Server → Client | New opportunity detected and scored |
| `ghost_execution` | Server → Client | Paper-mode execution event |
| `runtime_ack` | Server → Client | System runtime acknowledgment from searcher |
| `killswitch_change` | Server → Client | Kill-switch state changed |
| `runtime_ack` | Client → Server | Acknowledge receipt (optional) |

---

## Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `unauthorized` | 401 | Missing or invalid authentication token |
| `invalid_request` | 400 | Request body or parameters failed validation |
| `rate_limited` | 429 | Too many requests; retry after specified duration |
| `not_found` | 404 | Requested resource does not exist |
| `internal_error` | 500 | Unexpected server error |
| `service_unavailable` | 503 | Upstream service unavailable |
| `verifier_error` | 500 | Readiness check failed internally |

## Rate Limits

| Endpoint Prefix | Limit | Window |
|----------------|-------|--------|
| `/health`, `/api/health` | No limit | — |
| `/metrics` | No limit | — |
| `/status` | No limit | — |
| `/api/v1/*` (public) | 60 requests | 60 seconds |
| `/api/v1/recon/timeseries` | 30 requests | 60 seconds |
| `/admin/*` | 30 requests | 60 seconds |
| `/admin/audit` | 30 requests | 60 seconds |

Rate limit headers (on limited endpoints):
```
RateLimit-Limit: 30
RateLimit-Remaining: 27
RateLimit-Reset: 1715956800
```

## Pagination

List endpoints support cursor-based pagination:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 50 | Items per page (max 500) |
| `offset` | integer | 0 | Items to skip |

Response envelope:
```json
{
  "items": [...],
  "total": 156,
  "limit": 50,
  "offset": 0
}
```

## Response Headers

All responses include:

| Header | Description |
|--------|-------------|
| `x-arbx-trace-id` | Unique request trace ID for distributed tracing |
| `x-arbx-system-guard` | Kill-switch state banner (when armed) |
| `x-arbx-cache` | Cache status: `HIT`, `MISS`, or `BYPASS` |
| `x-arbx-version` | API server version |

## Related

- `docs/explanation/architecture-overview.md`
- `docs/how-to/deploy-to-vps.md`
- `docs/adr/002-kill-switch-fail-closed.md`
- `docs/runbooks/kill-switch-activation.md`
