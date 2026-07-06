# REST API Reference

The ArbitrageX v2 REST API provides programmatic access to all platform functions. The API is implemented in Rust using Axum and follows RESTful conventions with JSON request and response bodies.

| Property | Value |
|----------|-------|
| **Base URL** | `http://localhost:3000` (local) |
| **Content-Type** | `application/json` |
| **Authentication** | API Key (`X-API-Key` header) |
| **Rate Limit** | 1000 requests/minute |
| **Version** | `v1` (path prefix) |

---

## Authentication

All endpoints except `/health` require an API key:

```bash
curl -H "X-API-Key: <your-api-key>" http://localhost:3000/api/v1/opportunities
```

Generate an API key:

```bash
openssl rand -hex 32
```

Set in `.env`:

```bash
AX_API_KEY=<your-generated-key>
```

---

## Endpoints

### System Endpoints

#### GET `/health`

Returns system health status.

| Property | Value |
|----------|-------|
| Auth | None |
| Rate Limit | Exempt |

**Response:**

```json
{
  "status": "healthy",
  "version": "2.0.0",
  "mode": "paper",
  "uptime_seconds": 86400,
  "containers": {
    "total": 21,
    "healthy": 21,
    "degraded": 0
  },
  "services": {
    "postgres": "connected",
    "redis": "connected",
    "grpc": "connected"
  },
  "timestamp": "2024-01-15T09:23:47Z"
}
```

#### GET `/api/v1/mode`

Returns current execution mode configuration.

| Property | Value |
|----------|-------|
| Auth | Required |
| Rate Limit | 100/min |

**Response:**

```json
{
  "mode": "paper",
  "ghost_protocol": true,
  "live_execution": false,
  "capital_at_risk": "0.00 USD",
  "simulation_engine": "revm",
  "paper_balance": "100000.00 USD",
  "supported_modes": ["paper", "live"],
  "switch_available": true
}
```

#### GET `/api/v1/system/rpc-status`

Returns status of all configured RPC endpoints.

**Response:**

```json
{
  "primary": {
    "url": "https://eth-mainnet.g.alchemy.com/v2/...",
    "healthy": true,
    "latency_ms": 38,
    "block_height": 18945233,
    "block_drift": 0,
    "requests_per_minute": 142,
    "errors_per_minute": 0
  },
  "fallbacks": [
    {
      "url": "https://mainnet.infura.io/v3/...",
      "healthy": true,
      "latency_ms": 52,
      "block_height": 18945233,
      "requests_per_minute": 23,
      "errors_per_minute": 1
    }
  ]
}
```

#### GET `/api/v1/system/metrics`

Returns Prometheus-formatted metrics for external scraping.

**Response:**

```text
# HELP ax_opportunities_detected_total Total opportunities detected
# TYPE ax_opportunities_detected_total counter
ax_opportunities_detected_total{strategy="triangular_arb_v2"} 4521
ax_opportunities_detected_total{strategy="cycle_arb"} 3892

# HELP ax_paper_trades_executed_total Total paper trades executed
# TYPE ax_paper_trades_executed_total counter
ax_paper_trades_executed_total{status="success"} 3847
ax_paper_trades_executed_total{status="reverted"} 124
```

---

### Opportunity Endpoints

#### GET `/api/v1/opportunities`

List detected opportunities.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | integer | No | Max results (default: 20, max: 100) |
| `strategy` | string | No | Filter by strategy ID |
| `chain` | string | No | Filter by chain (`ethereum`, `arbitrum`, `base`) |
| `min_profit` | float | No | Minimum net profit in USD |
| `from` | ISO timestamp | No | Start time filter |
| `to` | ISO timestamp | No | End time filter |

**Response:**

```json
{
  "opportunities": [
    {
      "op_id": "ax-opp-7f3a9e2d",
      "timestamp": "2024-01-15T09:23:47.123Z",
      "strategy": "triangular_arb_v2",
      "chain": "ethereum",
      "paper_mode": true,
      "pools": [
        {
          "dex": "uniswap_v3",
          "address": "0x8ad599c3a0ff1e08...",
          "token_in": "WETH",
          "token_out": "USDC",
          "fee": 0.0005
        }
      ],
      "input_amount": "1.500000000000000000",
      "expected_output": "1.523000000000000000",
      "expected_profit_usd": "45.67",
      "gas_estimate": 285000,
      "gas_cost_usd": "12.34",
      "net_profit_usd": "33.33",
      "confidence": 0.94,
      "ttl_ms": 4500
    }
  ],
  "total": 142,
  "page": 1,
  "limit": 20
}
```

#### GET `/api/v1/opportunities/:op_id`

Get a single opportunity by ID.

**Response:**

```json
{
  "op_id": "ax-opp-7f3a9e2d",
  "timestamp": "2024-01-15T09:23:47.123Z",
  "strategy": "triangular_arb_v2",
  "chain": "ethereum",
  "paper_mode": true,
  "pools": [...],
  "input_amount": "1.500000000000000000",
  "expected_output": "1.523000000000000000",
  "expected_profit_usd": "45.67",
  "gas_estimate": 285000,
  "gas_cost_usd": "12.34",
  "net_profit_usd": "33.33",
  "confidence": 0.94,
  "ttl_ms": 4500,
  "raw_data": {
    "block_number": 18945231,
    "gas_price_gwei": 18.5,
    "eth_price_usd": 2200.0
  }
}
```

---

### Paper Trade Endpoints

#### POST `/api/v1/paper/trade`

Execute a paper trade simulation.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `op_id` | string | Yes | Opportunity ID to execute |
| `input_amount` | string | Yes | Input amount in wei |
| `max_slippage_bps` | integer | No | Max slippage basis points (default: 50) |
| `priority_fee_gwei` | float | No | Priority fee for gas (default: 2.0) |

**Response:**

```json
{
  "paper_trade_id": "ax-paper-tx-a1b2c3d4",
  "op_id": "ax-opp-7f3a9e2d",
  "status": "success",
  "timestamp": "2024-01-15T09:23:48.891Z",
  "simulation": {
    "engine": "revm",
    "fork_block": 18945231,
    "execution_time_ms": 12,
    "traces": 3
  },
  "input": {
    "token": "WETH",
    "amount": "1.500000000000000000"
  },
  "output": {
    "token": "WETH",
    "amount": "1.521400000000000000"
  },
  "profit": {
    "gross_wei": "21400000000000000",
    "gross_usd": "44.73",
    "gas_cost_usd": "12.18",
    "net_usd": "32.55"
  },
  "execution": {
    "route": ["uniswap_v3", "sushiswap", "curve"],
    "actual_slippage_bps": 12,
    "gas_used": 278432,
    "effective_gas_price_gwei": 18.5,
    "revert_reason": null
  },
  "vs_expected": {
    "output_delta_bps": -10,
    "profit_delta_bps": -23,
    "within_tolerance": true
  }
}
```

#### GET `/api/v1/paper/trades`

List paper trade history.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | integer | No | Max results (default: 20) |
| `offset` | integer | No | Pagination offset |
| `from` | ISO timestamp | No | Start time |
| `to` | ISO timestamp | No | End time |
| `status` | string | No | Filter: `success`, `reverted`, `expired` |
| `strategy` | string | No | Filter by strategy |

**Response:**

```json
{
  "trades": [
    {
      "paper_trade_id": "ax-paper-tx-a1b2c3d4",
      "op_id": "ax-opp-7f3a9e2d",
      "status": "success",
      "timestamp": "2024-01-15T09:23:48.891Z",
      "strategy": "triangular_arb_v2",
      "input_token": "WETH",
      "input_amount": "1.500000000000000000",
      "output_token": "WETH",
      "output_amount": "1.521400000000000000",
      "net_profit_usd": "32.55",
      "gas_used": 278432,
      "gas_cost_usd": "12.18"
    }
  ],
  "total": 3847,
  "page": 1,
  "limit": 20,
  "summary": {
    "total_trades": 3847,
    "successful": 3847,
    "reverted": 124,
    "expired": 45,
    "total_net_profit_usd": "124567.89",
    "avg_profit_per_trade_usd": "31.47"
  }
}
```

#### GET `/api/v1/paper/trades/:trade_id`

Get a single paper trade with full execution trace.

---

### Strategy Endpoints

#### GET `/api/v1/strategies`

List all registered strategies.

**Response:**

```json
{
  "strategies": [
    {
      "id": "triangular_arb_v2",
      "name": "Triangular Arbitrage V2",
      "version": "2.1.0",
      "active": true,
      "protocols": ["uniswap_v3", "sushiswap", "curve"],
      "min_profit_usd": 5.0,
      "max_gas_cost_usd": 50.0,
      "total_opportunities": 4521,
      "total_trades": 1284,
      "win_rate": 0.94,
      "avg_profit_usd": 28.5
    }
  ],
  "total": 9
}
```

#### GET `/api/v1/strategies/:id`

Get strategy details and performance metrics.

**Response:**

```json
{
  "id": "triangular_arb_v2",
  "name": "Triangular Arbitrage V2",
  "version": "2.1.0",
  "active": true,
  "protocols": ["uniswap_v3", "sushiswap", "curve"],
  "min_profit_usd": 5.0,
  "max_gas_cost_usd": 50.0,
  "configuration": {
    "max_slippage_bps": 50,
    "timeout_ms": 5000,
    "batch_size": 100
  },
  "performance_24h": {
    "opportunities": 142,
    "executed": 89,
    "successful": 84,
    "reverted": 5,
    "total_profit_usd": "2456.78",
    "avg_execution_ms": 12
  },
  "performance_7d": {
    "opportunities": 984,
    "executed": 623,
    "successful": 589,
    "total_profit_usd": "17189.45",
    "win_rate": 0.945
  }
}
```

#### PUT `/api/v1/strategies/:id/toggle`

Enable or disable a strategy.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `active` | boolean | Yes | Enable or disable |

---

### Mempool Endpoints

#### GET `/api/v1/mempool/stream-status`

Returns mempool watcher connection status.

**Response:**

```json
{
  "connected": true,
  "pending_tx_count": 847,
  "monitored_addresses": 12,
  "bytes_per_second": 45023,
  "uptime_seconds": 86400,
  "reconnects": 2
}
```

#### GET `/api/v1/mempool/pending`

List pending transactions relevant to monitored strategies.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | integer | No | Max results (default: 50) |
| `address` | string | No | Filter by sender address |

---

### Dashboard Endpoints

#### GET `/api/v1/dashboard/summary`

Returns dashboard overview data.

**Response:**

```json
{
  "system": {
    "mode": "paper",
    "containers_healthy": 21,
    "containers_total": 21,
    "uptime_hours": 24.5
  },
  "performance": {
    "trades_24h": 89,
    "profit_24h_usd": "2456.78",
    "win_rate_24h": 0.94,
    "avg_execution_ms": 12
  },
  "opportunities": {
    "detected_24h": 142,
    "avg_confidence": 0.91,
    "top_strategy": "triangular_arb_v2"
  },
  "risk": {
    "exposure_usd": "0.00",
    "daily_cap_used_pct": 0,
    "revert_rate": 0.06
  }
}
```

---

### WebSocket Endpoint

#### GET `/ws`

WebSocket endpoint for real-time event streams.

**Sub-protocols:**

| Channel | Path | Events |
|---------|------|--------|
| Opportunities | `/ws/opportunities` | New opportunity detections |
| Trades | `/ws/trades` | Trade execution updates |
| Mempool | `/ws/mempool` | Pending transaction stream |
| System | `/ws/system` | Container health, mode changes |

**Connect:**

```bash
websocat ws://localhost:8080/ws/opportunities
```

**Message format:**

```json
{
  "event": "opportunity.detected",
  "timestamp": "2024-01-15T09:23:47.123Z",
  "data": { ...opportunity object... }
}
```

---

## Error Responses

All errors follow this structure:

```json
{
  "error": {
    "code": "R8-E-001",
    "message": "Invalid opportunity ID format",
    "details": {
      "field": "op_id",
      "provided": "invalid-id",
      "expected": "ax-opp-[a-f0-9]{8}"
    },
    "request_id": "req-uuid-1234",
    "timestamp": "2024-01-15T09:23:47Z"
  }
}
```

### HTTP Status Codes

| Code | Meaning | Typical Cause |
|------|---------|---------------|
| 200 | OK | Successful GET/PUT |
| 201 | Created | Successful POST |
| 400 | Bad Request | Invalid parameters |
| 401 | Unauthorized | Missing or invalid API key |
| 403 | Forbidden | Insufficient permissions |
| 404 | Not Found | Resource does not exist |
| 409 | Conflict | Resource state conflict |
| 422 | Unprocessable Entity | Semantic validation failure |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Unexpected server error |
| 502 | Bad Gateway | Upstream service unavailable |
| 503 | Service Unavailable | System in maintenance |

### Error Code Reference

| Error Code | HTTP Status | Description |
|------------|-------------|-------------|
| `R8-E-001` | 400 | Invalid request parameter |
| `R8-E-002` | 400 | Missing required field |
| `R8-E-003` | 401 | Invalid API key |
| `R8-E-004` | 404 | Opportunity not found |
| `R8-E-005` | 404 | Trade not found |
| `R8-E-006` | 404 | Strategy not found |
| `R8-E-007` | 409 | Opportunity expired |
| `R8-E-008` | 409 | Mode transition in progress |
| `R8-E-009` | 422 | Profit below threshold |
| `R8-E-010` | 422 | Risk limit exceeded |
| `R8-E-011` | 422 | Insufficient paper balance |
| `R8-E-012` | 429 | Rate limit exceeded |
| `R8-E-013` | 500 | Simulation engine error |
| `R8-E-014` | 500 | Database connection failure |
| `R8-E-015` | 502 | RPC endpoint unavailable |
| `R8-E-016` | 503 | System in maintenance mode |

---

## Rate Limiting Headers

Every response includes rate limit headers:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 987
X-RateLimit-Reset: 1705312200
```

When the limit is exceeded:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 45
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1705312245
```
