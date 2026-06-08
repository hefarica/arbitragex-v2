# Environment Variables Reference

ArbitrageX v2 configuration is controlled entirely through environment variables. This document lists every variable, its purpose, whether it is required, and its default value.

---

## Execution Mode

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_MODE` | Yes | `paper` | Execution mode: `paper` (simulated) or `live` (on-chain) |
| `AX_CHAIN` | Yes | `ethereum` | Primary chain: `ethereum`, `arbitrum`, `base` |
| `AX_CHAIN_SECONDARY` | No | — | Comma-separated secondary chains |

## RPC Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_RPC_PRIMARY` | Yes (live) | — | Primary RPC endpoint URL |
| `AX_RPC_FALLBACK_1` | No | — | First fallback RPC endpoint |
| `AX_RPC_FALLBACK_2` | No | — | Second fallback RPC endpoint |
| `AX_RPC_FALLBACK_3` | No | — | Third fallback RPC endpoint |
| `AX_RPC_ARBITRUM` | No | — | Arbitrum RPC endpoint |
| `AX_RPC_BASE` | No | — | Base RPC endpoint |
| `AX_RPC_POOL_ENABLED` | No | `false` | Enable RPC connection pooling |
| `AX_RPC_TIMEOUT_MS` | No | `5000` | RPC request timeout in milliseconds |
| `AX_RPC_MAX_RETRIES` | No | `3` | Maximum RPC retry attempts |

## Database

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `POSTGRES_USER` | Yes | `ax_user` | PostgreSQL username |
| `POSTGRES_PASSWORD` | Yes | — | PostgreSQL password |
| `POSTGRES_DB` | No | `arbitragex` | Database name |
| `DATABASE_MAX_CONNECTIONS` | No | `20` | Connection pool maximum size |
| `DATABASE_TIMEOUT_MS` | No | `30000` | Query timeout in milliseconds |

## Redis

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `REDIS_URL` | Yes | `redis://ax-redis-primary:6379` | Redis connection string |
| `REDIS_PASSWORD` | No | — | Redis password (if auth enabled) |
| `REDIS_POOL_SIZE` | No | `10` | Redis connection pool size |
| `REDIS_TIMEOUT_MS` | No | `5000` | Redis operation timeout |

## Wallet & Keys

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_WALLET_ADDRESS` | Live only | — | Ethereum wallet address |
| `AX_PRIVATE_KEY` | Live only | — | Wallet private key (encrypted at rest) |
| `AX_FIREBLOCKS_VAULT_ID` | No | — | Fireblocks vault ID |
| `AX_FIREBLOCKS_API_KEY` | No | — | Fireblocks API key |
| `AX_API_KEY` | Yes | — | REST API authentication key |
| `JWT_SECRET` | No | — | JWT signing secret for session tokens |

## Strategy Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_MIN_PROFIT_USD` | No | `5.0` | Minimum profit threshold in USD |
| `AX_MAX_GAS_COST_USD` | No | `50.0` | Maximum gas cost tolerance in USD |
| `AX_SLIPPAGE_TOLERANCE_BPS` | No | `50` | Default slippage tolerance in basis points |
| `AX_MAX_CONCURRENT_SIMULATIONS` | No | `16` | Max parallel REVM simulations |
| `AX_STRATEGY_EVAL_INTERVAL_MS` | No | `100` | Strategy evaluation interval |

## Risk Limits

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_DAILY_CAPITAL_LIMIT_USD` | No | `10000` | Daily capital deployment limit |
| `AX_SINGLE_TRADE_MAX_USD` | No | `2000` | Maximum capital per single trade |
| `AX_COOLDOWN_SECONDS` | No | `30` | Minimum seconds between trades |
| `AX_MAX_REVERT_RATE_PCT` | No | `15.0` | Maximum acceptable revert rate |

## Logging & Telemetry

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `RUST_LOG` | No | `info` | Rust log level (error, warn, info, debug, trace) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | No | — | OpenTelemetry collector endpoint |
| `OTEL_SERVICE_NAME` | No | `arbitragex-v2` | Service name for traces |
| `PROMETHEUS_RETENTION_DAYS` | No | `30` | Metrics retention period |
| `JAEGER_ENABLED` | No | `true` | Enable Jaeger distributed tracing |

## Network

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_WS_PORT` | No | `8080` | WebSocket gateway port |
| `AX_API_PORT` | No | `3000` | REST API server port |
| `AX_GRPC_PORT` | No | `50051` | gRPC control plane port |
| `AX_BIND_ADDRESS` | No | `0.0.0.0` | Network bind address |

## Paper Mode

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_PAPER_BALANCE_USD` | No | `100000` | Starting virtual balance |
| `AX_PAPER_FORK_BLOCK` | No | — | Specific block to fork (empty = latest) |
| `AX_PAPER_CACHE_SIZE` | No | `10000` | Simulation result cache entries |

## Security

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AX_RATE_LIMIT_REQ_PER_MIN` | No | `1000` | API rate limit per minute |
| `AX_CORS_ALLOWED_ORIGINS` | No | `*` | CORS allowed origins |
| `AX_ENABLE_SWAGGER` | No | `false` | Enable OpenAPI/Swagger UI |

---

## Example Production .env

```bash
# === Execution ===
AX_MODE=paper
AX_CHAIN=ethereum
AX_CHAIN_SECONDARY=arbitrum,base

# === RPC ===
AX_RPC_PRIMARY=https://eth-mainnet.g.alchemy.com/v2/KEY
AX_RPC_FALLBACK_1=https://mainnet.infura.io/v3/KEY
AX_RPC_POOL_ENABLED=true
AX_RPC_TIMEOUT_MS=5000

# === Database ===
DATABASE_URL=postgresql://ax_prod:STRONG_PASS@ax-postgres:5432/arbitragex
POSTGRES_PASSWORD=STRONG_PASS
DATABASE_MAX_CONNECTIONS=20

# === Redis ===
REDIS_URL=redis://ax-redis-primary:6379

# === Keys ===
AX_API_KEY=GENERATED_WITH_OPENSSL_RAND
JWT_SECRET=GENERATED_WITH_OPENSSL_RAND

# === Strategy ===
AX_MIN_PROFIT_USD=5.0
AX_MAX_GAS_COST_USD=50.0
AX_SLIPPAGE_TOLERANCE_BPS=50

# === Risk ===
AX_DAILY_CAPITAL_LIMIT_USD=10000
AX_SINGLE_TRADE_MAX_USD=2000

# === Logging ===
RUST_LOG=info
OTEL_EXPORTER_OTLP_ENDPOINT=http://ax-jaeger:4317

# === Paper ===
AX_PAPER_BALANCE_USD=100000
```
