# ArbitrageX V2 — Architecture Reference

## System Topology

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         CLOUDFLARE CDN                                   │
│                   (WAF, DDoS, Global Distribution)                       │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────────────┐
│                    EDGE WORKER (Cloudflare Worker)                        │
│           Port 8787 — SSR proxy, route handling, assets                  │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
          ┌────────────────────┼────────────────────────┐
          │                    │                        │
┌─────────▼─────────┐ ┌───────▼────────┐  ┌───────────▼──────────────┐
│  FRONTEND         │ │  API SERVER    │  │  SEARCHER-RS (Rust)       │
│  Next.js App      │ │  Port 8080     │  │  Port 9001 (health)       │
│  Port 5173        │ │  REST + WS     │  │  Core Detection Engine    │
│  React + TS       │ │  TypeScript    │  │  Orchestrator + Engines   │
└───────────────────┘ └───────┬────────┘  └───────────┬──────────────┘
                              │                       │
                    ┌─────────┴───────────────────────┘
                    │
     ┌──────────────┼──────────────────────────────────┐
     │              │                                  │
┌────▼─────┐  ┌────▼──────────┐  ┌────────────────────▼───────────────┐
│ POSTGRES │  │ REDIS 7.2     │  │ CARTRIDGE RUNTIME (FASE OMEGA)      │
│ 15       │  │ Cache+PubSub  │  │ Rhai Engine — Sandboxed              │
│ Port 5432│  │ Port 6379     │  │ Host Bindings → Redis/RPC            │
└──────────┘  └───────────────┘  │ Hot-Reload via PubSub                │
                                 │ 3 Cartridges: dex_arb, triangular,   │
                                 │   liquidation                         │
                                 └─────────────────────────────────────┘
```

## Backend Workspace (Rust Crates)

| Crate | Propósito |
|-------|-----------|
| `searcher-rs` | Core: scanner, orchestrator, engines, cartridge runtime, workers |
| `shared-rs` | Tipos compartidos, config, health, metrics, killswitch, RPC failover |
| `sim-ctl` | Simulation controller (Anvil/REVM fork manager) |
| `relays-client` | Flashbots/relay bundle submission |
| `recon` | Reconciliation, PnL tracking, anomaly detection |
| `token-enricher` | Token metadata enrichment via multicall |
| `math-engine` | AMM math, Bellman-Ford, convex optimization |
| `prioritization-spine` | Scoring, config-aware evaluation, network signals |
| `simulator-v2` | REVM simulation runner, sequence runner |
| `sed-core` | Sequential Equilibrium Dispatcher (allocator, eigenstate, hedger) |

**Workspace dependencies clave:** `rhai`, `revm 3.5`, `alloy 1.0`, `ethers 2`, `tokio`, `redis`, `sqlx 0.7`, `axum 0.7`, `prometheus`

## Orchestrator Pipeline (Hot-Path)

El orchestrator es el entry point para cada `RouteIntent` decodificado de la mempool:

```
Mempool TX → Scanner → RouteIntent Decoder
                              ↓
                    ┌─── ImpactIndex ───┐
                    │                   │
              ImpactSet (pools afectados)
                    │
    ┌───────────────┼───────────────────────────┐
    │               │               │           │
┌───▼────┐  ┌──────▼─────┐  ┌──────▼────┐  ┌───▼──────────┐
│DexEngine│  │Triangular  │  │Liquidation│  │FlashloanEngine│
│(V2/V3)  │  │Engine      │  │Engine     │  │(wrapper)      │
└───┬─────┘  └──────┬─────┘  └──────┬────┘  └───┬──────────┘
    │               │               │           │
    └───────────────┼───────────────┘           │
                    ↓                           │
         base_candidates ←──────────────────────┘
                    ↓
      ConfigAwareEvaluator (scoring, gates)
                    ↓
         ┌──── accepted ────┐
         │                  │
    OpportunityEmitter   Rejected (logged)
         │
    Redis XADD → PostgreSQL → Dashboard
```

### Engines Disponibles

| Engine | Archivo | Función |
|--------|---------|---------|
| DexEngine | `engines/dex_engine.rs` | Arbitraje V2/V3 entre pools |
| TriangularEngine | `engines/triangular_engine.rs` | Ciclos A→B→C→A |
| LiquidationEngine | `engines/liquidation_engine.rs` | Liquidaciones Aave/Compound |
| FlashloanEngine | `engines/flashloan_engine.rs` | Wrapper flash loan para rutas |
| CexDexEngine | `engines/cex_dex_engine.rs` | Arbitraje CEX↔DEX |
| SpatialEngine | `engines/spatial_engine.rs` | Arbitraje espacial multi-venue |
| SvsEngine | `engines/svs_engine.rs` | State Variable Simulation |
| BackrunEngine | `engines/backrun_engine.rs` | Backrunning de TXs |
| DlpEngine | `engines/dlp_engine.rs` | Dynamic Liquidity Provisioning |
| FundingRateEngine | `engines/funding_rate_engine.rs` | Funding rate arbitrage |
| TriangularAtomicEngine | `engines/triangular_atomic_engine.rs` | Triangular atómico |

### Workers (Background Tasks)

| Worker | Función |
|--------|---------|
| `pool_sync_worker` | Sincroniza reservas de pools |
| `price_worker` | Actualiza precios |
| `gas_oracle_worker` | Monitorea gas prices |
| `execution_worker` | Ejecuta bundles aprobados |
| `heartbeat_worker` | Health monitoring |
| `hft_mempool_listener` | Escucha mempool HFT |
| `jit_v3_worker` | JIT liquidity V3 |
| `liquidation_worker` | Monitorea health factors |
| `rpc_health_worker` | Verifica salud de RPCs |
| `flashloan_arb_worker` | Flash loan arbitrage |
| `funding_rate_worker` | Funding rate monitoring |
| `triangular_worker` | Triangular detection |
| `triangular_atomic_worker` | Atomic triangular |
| `cex_dex_worker` | CEX-DEX monitoring |
| `spatial_worker` | Spatial arbitrage |
| `svs_worker` | SVS monitoring |
| `backrun_worker` | Backrun detection |
| `dlp_worker` | DLP monitoring |

## Database Schema (Key Tables)

### cartridge_registry

```sql
CREATE TABLE cartridge_registry (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug            TEXT UNIQUE NOT NULL,
    name            TEXT NOT NULL,
    version         TEXT NOT NULL DEFAULT '1.0.0',
    author          TEXT NOT NULL DEFAULT 'operator',
    description     TEXT NOT NULL DEFAULT '',
    category        TEXT NOT NULL DEFAULT 'uncategorized',
    source_code     TEXT NOT NULL,
    content_hash    TEXT NOT NULL,
    target_chains   JSONB NOT NULL DEFAULT '[]'::jsonb,
    state           TEXT NOT NULL DEFAULT 'active',
    -- CHECK (state IN ('active','paused','failed','archived'))
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_compiled_at    TIMESTAMPTZ,
    compilation_errors  TEXT,
    total_evaluations   BIGINT NOT NULL DEFAULT 0,
    total_opportunities BIGINT NOT NULL DEFAULT 0,
    total_errors        BIGINT NOT NULL DEFAULT 0,
    last_evaluation_at  TIMESTAMPTZ
);
```

### cartridge_audit_log

```sql
CREATE TABLE cartridge_audit_log (
    id              BIGSERIAL PRIMARY KEY,
    cartridge_id    UUID NOT NULL REFERENCES cartridge_registry(id),
    event_type      TEXT NOT NULL,  -- 'inject','update','pause','resume','archive','error'
    actor           TEXT NOT NULL,
    details         JSONB DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### cartridge_metrics_hourly

```sql
CREATE TABLE cartridge_metrics_hourly (
    id              BIGSERIAL PRIMARY KEY,
    cartridge_id    UUID NOT NULL REFERENCES cartridge_registry(id),
    chain_id        BIGINT NOT NULL,
    hour            TIMESTAMPTZ NOT NULL,
    evaluations     BIGINT NOT NULL DEFAULT 0,
    opportunities   BIGINT NOT NULL DEFAULT 0,
    errors          BIGINT NOT NULL DEFAULT 0,
    avg_eval_ms     DOUBLE PRECISION,
    max_eval_ms     DOUBLE PRECISION,
    total_profit_usd DOUBLE PRECISION DEFAULT 0.0,
    UNIQUE(cartridge_id, chain_id, hour)
);
```

## Redis Key Schema

| Pattern | Contenido | TTL |
|---------|-----------|-----|
| `arbx:pool_reserves:{chain_id}:{pool_addr}` | JSON reservas | Variable |
| `arbx:tokens:{chain_id}:{token_addr}` | JSON metadata token | 1h |
| `arbx:pool_index:{chain_id}:{token_lo}:{token_hi}` | JSON array pool addrs | 1h |
| `arbx:sim_cache:{chain_id}:{amount}:{path}` | JSON sim result | 30s |
| `arbx:cartridge:source:{cartridge_id}` | Full .rhai source | Persistent |
| `arbx:cartridge:injection` | PubSub channel (events) | — |
| `arbx:cartridge:ack` | PubSub channel (ACKs) | — |
| `arbx:cartridge:signals` | PubSub channel (cartridge signals) | — |
| `arbx:cartridge:telemetry` | PubSub channel (log_quantum) | — |
| `arbx:opps:detected` | Stream de oportunidades | — |

## Configuration (configs/app.toml)

```toml
[system]
env = "production-like"
kill_switch_enabled_default = true
service_name_prefix = "arbx"

[risk]
max_revert_rate_pct = 5.0
max_execution_variance_pct = 20.0
min_token_safety_score = 70
simulation_required_for_new_routes = true
max_gas_price_gwei = 200.0
max_slippage_pct = 1.5

[execution]
private_only = true
paper_mode = true
max_parallel_executions = 8
max_value_eth = 1.0
flashbots_submit_timeout_ms = 5000

[scoring]
min_accept_score = 55.0
weight_liquidity = 0.20
weight_depth = 0.15
weight_safety = 0.20
weight_slippage = 0.15
weight_gas = 0.15
weight_risk = 0.15

[simulation]
provider = "not_implemented"  # "anvil" | "tenderly"
sim_timeout_ms = 3000
gas_limit_safety_factor = 1.3

[[chains]]
chain_id = 1
name = "ethereum"
enabled = true

[[chains]]
chain_id = 42161
name = "arbitrum"
enabled = false

[[circuit_breakers]]
name = "token_safety_api"
threshold = 5
window_ms = 60000
cooldown_ms = 120000
```

## Monitoring Stack

| Componente | Puerto | Función |
|-----------|--------|---------|
| Prometheus | 9090 | Métricas scraping |
| Grafana | 3000 | Dashboards |
| Loki | 3100 | Log aggregation |
| Promtail | — | Log shipping |
| Thanos Sidecar | 10901 | Long-term storage |
| Thanos Query | 10904 | Federated queries |
| MinIO | 9000/9001 | Object storage (Thanos) |
| Vault | 8200 | Secrets management |

### Métricas Prometheus Clave

```
arbx_opportunity_total{status="detected|accepted|rejected"}
arbx_candidate_total
arbx_engine_errors_total{engine="dex|triangular|liquidation"}
arbx_simulation_failed_total
arbx_decoded_intents_total
arbx_impacted_routes_total
arbx_rejected_config_total
arbx_rejected_no_profit_total
arbx_opportunities_published_total
```

## Security Model

- **Paper Mode:** Default `true`. Solo el operador lo desactiva manualmente.
- **Kill Switch:** Fail-closed. Habilitado por defecto.
- **Private Only:** Todas las TXs vía Flashbots/private relays.
- **Vault:** HashiCorp Vault para secrets (Shamir 3-of-5).
- **Cartridge Sandboxing:** Sin filesystem, sin red, sin eval, max 1M ops.
- **Admin Token:** Generado localmente por operador, nunca por el sistema.
- **TLS:** Vault con TLS 1.2+, MinIO con health checks.
- **Circuit Breakers:** 4 configurados (token_safety, db_writes, stream_consumer, sim_engine).
