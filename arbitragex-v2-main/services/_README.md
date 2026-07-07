# Services (`services/`)

This directory contains containerized services that form the ArbitrageX V2
runtime. Each service is independently deployable and communicates via
HTTP or WebSocket.

## Services

| Service | Image | Port | Description |
|---------|-------|------|-------------|
| `api` | `ghcr.io/arbitragex/api` | 8080 | REST API server |
| `engine` | `ghcr.io/arbitragex/engine` | — | Opportunity discovery |
| `executor` | `ghcr.io/arbitragex/executor` | — | Transaction execution |
| `risk` | `ghcr.io/arbitragex/risk` | — | Risk and circuit breaker |
| `ws` | `ghcr.io/arbitragex/ws` | 8081 | WebSocket gateway |

## Docker Compose

```bash
# Start all services
docker compose up -d

# View logs
docker compose logs -f api

# Restart a service
docker compose restart engine
```

## Health Checks

Each service exposes health endpoints:

- `/health` — Composite health of all dependencies
- `/ready` — Readiness probe for orchestrators
- `/live` — Liveness probe for orchestrators

## Configuration

Services are configured via environment variables (see `.env.example`):

| Variable | Default | Description |
|----------|---------|-------------|
| `API_PORT` | 8080 | HTTP server port |
| `WS_PORT` | 8081 | WebSocket server port |
| `RUST_LOG` | info | Log level |
| `RPC_URLS` | — | Comma-separated RPC endpoints |

## Deployment

Services deploy as Kubernetes Deployments in the `arbitragex-prod` namespace.
See `pipelines/_README.md` for CI/CD details.