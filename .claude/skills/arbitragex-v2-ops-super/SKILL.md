---
name: arbitragex-v2-ops-super
description: "[ES · superset con doctrine.md] Documentación operacional completa del proyecto ArbitrageX-v2 (hefarica/arbitragex-v2). Use para acceder a la arquitectura del sistema, comprender el runtime de cartuchos FASE OMEGA (Rhai), desplegar código al VPS, ejecutar tareas comunes, diagnosticar problemas, gestionar servicios Docker, y operar el pipeline de detección MEV. Incluye reglas inmutables (Zero-Mocks, Fail-Honest, Deployment Workflow), estructura del monorepo Rust/TypeScript, host bindings, contrato universal de cartuchos, hot-reload vía Redis PubSub, y configuración de producción."
---

# ArbitrageX V2 Operations

## Overview

ArbitrageX-v2 es un sistema de arbitraje descentralizado multi-chain con motor dinámico de estrategias (FASE OMEGA). El núcleo es un binario Rust (`searcher-rs`) permanentemente agnóstico que ejecuta estrategias `.rhai` en caliente sin recompilación. Toda evolución de estrategias ocurre en cartuchos Rhai inyectados vía Redis PubSub.

**Repositorio:** `hefarica/arbitragex-v2`

## Quick Reference

| Componente | Detalle |
|-----------|---------|
| VPS Producción | Hetzner CX43, `195.201.235.70`, Falkenstein DE |
| OS | Ubuntu, 8 vCPU, 16 GB RAM, 160 GB SSD |
| SSH Alias | `arbx` |
| Ruta VPS | `/opt/arbitragex-v2` |
| Compose File | `docker/compose.prod.yml` |
| Runtime Motor | Rhai (FASE OMEGA) — sandboxed, max 1M ops |
| Paper Mode | `true` por defecto (safety) |
| Kill Switch | Enabled por defecto (fail-closed) |
| Rust Edition | 2021, rust-version 1.85 |
| DB | PostgreSQL 15 (`arbitragex`) |
| Cache/PubSub | Redis 7.2 |
| Chains Habilitadas | Ethereum (chain_id=1) activa; Optimism, Polygon, Arbitrum, Base configurables |

## Reglas Inmutables

### RULE 00 — Doctrina Zero Mocks

**PROHIBIDO** inyectar datos falsos en cualquier capa. Frontend renderiza lo que devuelve la API. Backend solo usa fuentes veraces (Mempool, RPC, PostgreSQL, Redis). Si falta dato → Fail-Fast o Fail-Honest con observation.

### RULE 01 — Deployment Workflow

```
[LOCAL: Windows] → [GIT: Commit & Push] → [VPS: Deploy]
```

- **LOCAL:** Solo edición, tests (`vitest`, `cargo test`, `tsc --noEmit`). NO Docker. NO servicios backend.
- **VPS:** Docker compose, stack completo.
- **Flujo:**
  ```bash
  # Local
  git add . && git commit -m "descripción" && git push origin main
  # VPS
  ssh arbx
  cd /opt/arbitragex-v2
  git pull origin main
  docker compose -f docker/compose.prod.yml up -d --build
  ```

### RULE 02 — Fail-Honest Pattern (R8)

`None` = no computado. `Some(0.0)` = computado y exactamente cero. Si no hay datos reales, registrar observation con razón exacta y detener. NUNCA fabricar una Opportunity.

## Project Structure (Monorepo)

```
arbitragex-v2/
├── backend/
│   ├── searcher-rs/        # Core Rust: scanner, orchestrator, engines, cartridge runtime
│   │   ├── src/cartridge/  # FASE OMEGA: runner, host_bindings, contract, subscriber, types
│   │   ├── src/engines/    # dex, triangular, liquidation, flashloan, cex_dex, spatial, svs
│   │   ├── src/workers/    # pool_sync, price, gas_oracle, execution, heartbeat, HFT mempool
│   │   ├── src/connectors/ # mempool_listener, reserve_reader, rpc_multiplexer
│   │   ├── cartridges/     # dex_arb.rhai, triangular_arb.rhai, liquidation.rhai
│   │   └── tests/          # cartridge_e2e, orchestrator_parallel, multistep_fork
│   ├── sim-ctl/            # Simulation controller (Anvil/REVM fork)
│   ├── relays-client/      # Flashbots/relay bundle submission
│   ├── recon/              # Reconciliation & PnL tracking
│   ├── token-enricher/     # Token metadata enrichment (multicall)
│   ├── math-engine/        # AMM math, Bellman-Ford, optimization
│   ├── prioritization-spine/ # Scoring & config-aware evaluation
│   ├── simulator-v2/       # REVM simulation runner
│   ├── sed-core/           # Sequential Equilibrium Dispatcher (quantitative)
│   ├── shared-rs/          # Shared types, config, health, metrics, killswitch
│   └── api-server/         # TypeScript API (Express/Fastify)
├── frontend/               # Next.js App Router + React
├── edge/                   # Cloudflare Worker (Edge proxy)
├── contracts/              # Solidity (Foundry)
├── database/
│   ├── migrations/         # 090_cartridge_registry.sql + 70+ migrations
│   └── seed/
├── docker/                 # compose.prod.yml, compose.dev.yml
├── configs/                # app.toml (canonical config, schema-validated)
├── monitoring/             # Prometheus, Grafana, Loki, Thanos
├── .agent/                 # 100 skills + 2 rules (Zero Mocks, Deployment)
├── CLAUDE.md               # Agent instructions (reglas, arquitectura, skills map)
└── ONBOARDING.md           # Guía de onboarding operacional
```

## Docker Services (Production)

| Service | Image/Build | Port | Resources | Health |
|---------|-------------|------|-----------|--------|
| postgres | postgres:15 | 5432 (local) | — | `pg_isready` |
| redis | redis:7.2 | 6379 (local) | — | `redis-cli PING` |
| searcher-rs | Build from backend/ | 9001 (health) | 4G RAM, 2 CPU | `/health` |
| sim-ctl | Build from backend/ | 3003 | 2G RAM, 1.5 CPU | `/health` |
| relays-client | Build from backend/ | 3005 | 1G RAM, 1 CPU | `/health` |
| recon | Build from backend/ | 3004 | — | `/health` |
| token-enricher | Build from backend/ | 3006 | — | `/health` |
| api-server | Build from backend/ | 8080 | — | `/health` |
| frontend | Build from frontend/ | 5173 | — | — |
| edge | Build from edge/ | 8787 | — | `/health` |
| prometheus | prom/prometheus | 9090 (local) | — | `/-/healthy` |
| grafana | grafana/grafana | 3000 (local) | — | `/api/health` |
| loki | grafana/loki | 3100 (local) | — | `/ready` |
| vault | hashicorp/vault:1.18.1 | 8200 (local) | 512M | `vault status` |
| minio | minio/minio | 9000/9001 (local) | 512M | `/minio/health/live` |
| thanos-sidecar/store/query | quay.io/thanos/thanos | 10901-10904 | 256-512M | `/-/healthy` |

### Gestión de Servicios

```bash
# Estado
docker compose -f docker/compose.prod.yml ps

# Logs
docker compose -f docker/compose.prod.yml logs --tail=100 searcher-rs

# Rebuild y restart
docker compose -f docker/compose.prod.yml up -d --build searcher-rs

# Restart sin rebuild
docker compose -f docker/compose.prod.yml restart searcher-rs sim-ctl relays-client
```

## Cartridge Runtime (FASE OMEGA)

El runtime de cartuchos es el corazón del sistema. Permite inyectar estrategias dinámicas sin recompilar Rust.

### Contrato Universal

Todo `.rhai` DEBE implementar exactamente estas 3 funciones:

| Función | Aridad | Retorna |
|---------|--------|---------|
| `fn init_strategy()` | 0 | Map con metadata |
| `fn evaluate_opportunity(pool_data)` | 1 | Map con `is_opportunity`, `estimated_profit`, `confidence` |
| `fn build_payload(opportunity)` | 1 | Map con `target_contract`, `calldata`, `gas_limit` |

**Hooks opcionales (lifecycle):**
- `fn on_activate()` — cuando el cartucho se activa
- `fn on_deactivate()` — cuando se pausa
- `fn on_new_block(block_number)` — en cada nuevo bloque

### Metadata Requerida (init_strategy)

Campos obligatorios: `name`, `version`, `author`, `description`

Campos operacionales recomendados: `category`, `target_chains` ([] = todas), `min_eval_interval_ms`, `supported_protocols`, `risk_profile`, `capital_requirement`

### Sandboxing

```rust
MAX_OPERATIONS: 1_000_000      // Previene loops infinitos
MAX_CALL_STACK_DEPTH: 64       // Previene recursión infinita
MAX_STRING_SIZE: 65_536        // 64 KB
MAX_ARRAY_SIZE: 4_096
MAX_MAP_SIZE: 1_024
MAX_MODULES: 0                 // Sin imports externos
// `eval` deshabilitado (sin ejecución dinámica de código)
```

Un error en un cartucho NUNCA crashea el nodo. Se marca como `Failed` y se excluye.

### Host Bindings (Funciones Nativas)

| Función | Fuente | Latencia |
|---------|--------|----------|
| `get_reserves(pool_addr)` | Redis cache | <1ms |
| `get_token_meta(token_addr)` | Redis cache | <1ms |
| `get_pool_index(token_a, token_b)` | Redis cache | <1ms |
| `simulate_swap(amount, path)` | RPC (cached) | 5-50ms |
| `get_base_fee()` | Redis/mempool | <1ms |
| `get_block_number()` | Redis/chain | <1ms |
| `get_timestamp()` | System clock | <1μs |
| `get_chain_id()` | Config | <1μs |
| `log_quantum(level, message)` | Redis telemetry | fire&forget |
| `emit_signal(signal_type, data)` | Redis PubSub | fire&forget |
| `math_sqrt(x)` | Native f64 | <1μs |
| `math_abs(x)` | Native f64 | <1μs |
| `math_min(a, b)` | Native f64 | <1μs |
| `math_max(a, b)` | Native f64 | <1μs |
| `math_pow(base, exp)` | Native f64 | <1μs |
| `to_wei(amount, decimals)` | Pure conversion | <1μs |
| `from_wei(amount, decimals)` | Pure conversion | <1μs |

Todos los bindings son **READ-ONLY**. Sin acceso a filesystem, red, o procesos.

### Hot-Reload Pipeline

```
UI (Strategy Forge) → API Server → Postgres + Redis PUBLISH
                                         ↓
                             CartridgeSubscriber
                                         ↓
                             CartridgeRunner.load_cartridge()
                                         ↓
                             Active in HashMap (zero downtime)
```

- **Canal Redis:** `arbx:cartridge:injection`
- **Source Key:** `arbx:cartridge:source:<cartridge_id>`
- **ACK Channel:** `arbx:cartridge:ack`
- **Events:** `inject`, `update`, `remove`, `pause`, `resume`
- **Dedup:** Por `content_hash` (SHA-256). Mismo hash = skip recompilation.
- **Latencia:** <100ms desde notificación hasta activo.

### Inyectar Nuevo Cartucho

```bash
# 1. Escribir el .rhai
cat > /opt/arbitragex-v2/backend/searcher-rs/cartridges/nueva_estrategia.rhai << 'EOF'
fn init_strategy() { #{ name: "...", version: "1.0.0", author: "...", description: "..." } }
fn evaluate_opportunity(pool_data) { #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0 } }
fn build_payload(opportunity) { #{ target_contract: "EXECUTOR", calldata: "0x", gas_limit: 300000 } }
EOF

# 2. Vía API (hot-reload sin restart)
curl -X POST http://localhost:8080/admin/cartridges/inject \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"slug": "nueva_estrategia", "source": "<rhai_source>"}'
```

## Orchestrator Pipeline

El orchestrator procesa cada `RouteIntent` decodificado de la mempool:

1. Convierte intent en `ImpactSet` vía `ImpactIndex`
2. Fan-out a engines: `DexEngine`, `TriangularEngine`, `LiquidationEngine`
3. `FlashloanEngine` envuelve rutas net-positive
4. `ConfigAwareEvaluator` evalúa cada `StrategyCandidate`
5. `OpportunityEmitter` publica aceptados/rechazados vía Redis

**Invariante crítico:** El orchestrator NUNCA escribe strategy strings literales. Todo viene de `StrategyLabel` retornado por un engine.

## Configuration (configs/app.toml)

Archivo canónico validado contra schema JSON al boot. Secretos NUNCA aquí.

```toml
[system]
kill_switch_enabled_default = true   # fail-closed
[risk]
max_gas_price_gwei = 200.0
max_slippage_pct = 1.5
simulation_required_for_new_routes = true
[execution]
paper_mode = true                    # SAFETY DEFAULT
private_only = true
max_parallel_executions = 8
max_value_eth = 1.0                  # hard cap per-bundle
[scoring]
min_accept_score = 55.0
```

## Troubleshooting

### Pipeline de Diagnóstico (R7)

```bash
# 1. ¿Searcher detecta?
docker logs searcher-rs --tail 200 | grep -i 'simulator.success'
# 2. ¿Redis recibe?
docker exec redis redis-cli XLEN arbx:opps:detected
# 3. ¿PostgreSQL recibe?
docker exec postgres psql -U postgres -d arbitragex -c 'SELECT MAX(detected_at) FROM opportunities;'
# 4. ¿API-server sirve?
curl localhost:8787/api/opportunities/live | head
```

### Scanner no conecta

```bash
docker compose -f docker/compose.prod.yml logs --tail=30 searcher-rs | grep scanner
# Esperar: event="scanner.subscribed" chain_id=1
# Si: event="scanner.no_rpc" → falta RPC_WS_1 en .env
# Si: event="scanner.idle" → kill-switch ON
```

### Frontend muestra "edge error"

```bash
curl http://localhost:8787/health
# Si falla → docker compose ... logs edge
```

### Oportunidades vacías

Verificar que scanner no está en `idle` (kill-switch) o `no_rpc`. Luego seguir pipeline R7.

## Environment Variables (Requeridas)

```bash
# Base de datos
DATABASE_URL=postgresql://postgres:$POSTGRES_PASSWORD@postgres:5432/arbitragex
POSTGRES_PASSWORD=<secret>
ARBX_MIGRATOR_PASSWORD=<secret>
ARBX_RW_PASSWORD=<secret>
ARBX_RO_PASSWORD=<secret>

# Redis
REDIS_URL=redis://redis:6379

# RPC (OBLIGATORIOS para detección)
RPC_WS_1=wss://eth-mainnet.g.alchemy.com/v2/<KEY>
RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/<KEY>

# Admin
ARBX_ADMIN_TOKEN=<operator-generated>

# Config
ARBX_CONFIG_PATH=/app/configs/app.toml
```

## Monitoring & Health

```bash
# Health check global
curl http://localhost:9001/health   # searcher-rs
curl http://localhost:8080/health   # api-server
curl http://localhost:8787/health   # edge worker

# Métricas Prometheus
curl http://localhost:9001/metrics | grep arbx_opportunity_total

# Métricas clave
# arbx_opportunity_total{status="detected"}
# arbx_candidate_total
# arbx_engine_errors_total
# arbx_simulation_failed_total
```

## References

Para documentación detallada, consultar:

- **`references/architecture.md`** — Diagrama del sistema, flujo de datos, engines, pipeline completo
- **`references/cartridge_api.md`** — Contrato universal, host bindings completos, pool_data shape, ejemplos de cartuchos reales
- **`references/doctrine.md`** — Reglas inmutables, CLAUDE.md condensado, reglas anti-reincidencia R1-R8
