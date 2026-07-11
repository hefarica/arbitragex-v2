# OMEGA Pipeline Architecture

**Last Updated:** 2026-07-11
**Version:** 2.0
**Status:** Production (Paper Mode)

## Overview

The OMEGA Pipeline is a sub-100ms end-to-end latency system for detecting, simulating, and executing Holonomic Loop Resolutions (multi-DEX atomic circular trades) across EVM-compatible networks. The pipeline follows the C-S-E architecture (Collector-Strategy-Executor) with rigorous fail-honest semantics throughout.

## System Architecture

### High-Level Data Flow

```mermaid
flowchart LR
    subgraph Detection["Phase 1: Detection (<20ms)"]
        MEM["Mempool WS\nAlchemy/Infura"]
        SR["searcher-rs\n3 Detection Engines"]
        HPE["HotPathEmitter"]
    end

    subgraph Stream["Phase 2: Redis Streams (<5ms)"]
        RD["Redis Streams\narbx:hot:*"]
    end

    subgraph Simulation["Phase 3: Simulation (<30ms)"]
        SIM["REVM Fork\nAnvil/sim-ctl"]
        SCORING["Bayesian Scoring\nprioritization-spine"]
    end

    subgraph Distribution["Phase 4: Distribution (<10ms)"]
        API["api-server\nWebSocket Gateway"]
        WS["Socket.IO\nRooms"]
    end

    subgraph Execution["Phase 5: Paper Execution (<15ms)"]
        ARCH["PaperTradeArchiver"]
        PG[("PostgreSQL\nOpportunities")]
    end

    subgraph Edge["Edge Layer"]
        EDGE["Cloudflare Worker\n:8787"]
        FE["Frontend\nDashboard"]
    end

    MEM -->|"mempool_tx"| SR
    SR -->|"Opportunity"| HPE
    HPE -->|"XADD"| RD
    RD -->|"XREAD"| SIM
    SIM -->|"SimResult"| SCORING
    SCORING -->|"ScoredOpp"| RD
    RD -->|"Pub/Sub"| API
    API -->|"broadcast"| WS
    API -->|"INSERT"| ARCH
    ARCH -->|"persist"| PG
    WS -->|"WSS"| EDGE
    EDGE -->|"HTTPS"| FE
```

### Component Breakdown

#### 1. searcher-rs (Collector Layer)

**Purpose:** Ultra-low-latency mempool monitoring and opportunity detection.

**Engines:**
| Engine | Function | Latency Target |
|--------|----------|----------------|
| `spanning_tree` | Graph-based path discovery across liquidity manifolds | <10ms |
| `cross_chain` | Cross-domain delta reconciliation (L1/L2) | <15ms |
| `liquidation` | Lending protocol position monitoring | <20ms |

**Key Modules:**
- `hot_path_emitter.rs` - Redis stream emission (<5ms budget)
- `cartridge_boot.rs` - Dynamic strategy cartridge loading
- `orchestrator.rs` - Opportunity lifecycle management
- `scoring_pipeline.rs` - Pre-simulation filtering

**Fail-Honest Behavior:**
- Redis connection failure → ERROR log + retry with backoff
- RPC latency >500ms → Circuit breaker triggers gate closure
- No opportunities detected → Empty stream (never synthetic data)

#### 2. Redis Streams (Hot Path v2)

**Stream Topology:**

```mermaid
flowchart TB
    subgraph Producer["Producers"]
        SED["searcher-rs"]
        SIM["sim-ctl"]
        API["api-server"]
    end

    subgraph Streams["Redis Streams"]
        D["arbx:hot:detected\nMAXLEN ~10000"]
        SIMS["arbx:hot:simulated\nMAXLEN ~5000"]
        EXEC["arbx:hot:paper_executed\nMAXLEN ~1000"]
    end

    subgraph Consumers["Consumer Groups"]
        CG1["paper-executor-g0"]
        CG2["ws-emitter-g0"]
        CG3["archiver-g0"]
    end

    SED -->|"XADD"| D
    SIM -->|"XADD"| SIMS
    API -->|"XADD"| EXEC
    D -->|"XREADGROUP"| CG1
    D -->|"XREADGROUP"| CG2
    SIMS -->|"XREADGROUP"| CG3
```

**Stream Schemas:**

| Stream | Fields | Producer | Consumer |
|--------|--------|----------|----------|
| `arbx:hot:detected` | id, chain_id, strategy_kind, token_path[], amounts[], detected_at_ms | searcher-rs | paper-executor-g0, ws-emitter-g0 |
| `arbx:hot:simulated` | id, status, net_profit_wei, gas_used, timestamp_ms | sim-ctl | archiver-g0 |
| `arbx:hot:paper_executed` | id, execution_time_ms, paper_pnl_usd, status | api-server | analytics-g0 |

#### 3. api-server (Strategy Layer)

**Purpose:** WebSocket gateway, paper trade archival, and control plane.

**Key Components:**
- `websocket.ts` - Socket.IO room management with authentication
- `paper/` - Paper trade archival and history API
- `routes/opportunities-live.ts` - Live opportunity streaming
- `readiness/` - 17-item readiness verification

**WebSocket Rooms:**
| Room | Event | Purpose |
|------|-------|---------|
| `convergence` | `convergence_signal` | Real-time pipeline metrics |
| `opportunities` | `opportunity` | New opportunity alerts |
| `telemetry` | `telemetry` | Cartridge telemetry feed |
| `route_discovery` | `route_discovery_telemetry` | Route discovery analytics |
| `runtime_ack` | `runtime_ack` | Configuration change acknowledgments |

#### 4. edge (Cloudflare Worker)

**Purpose:** Public-facing edge layer with KV-backed rate limiting and caching.

**Latency-Optimized Routes (<30ms target):**
- `GET /api/v1/health` - Pass-through health check
- `GET /api/v1/metrics/entropy` - Real-time entropy metrics
- `GET /status` - System status with 2s KV cache
- `GET /api/opportunities/live` - Live opportunities with 2s KV cache

**Security Features:**
- KV-backed cross-isolate rate limiting (120 req/min/IP)
- ASN-based threat filtering
- 401 lockout after 10 consecutive failures (15min window)
- httpOnly cookie session management

#### 5. Frontend (Next.js)

**Purpose:** Operator control plane and real-time dashboard.

**Key Features:**
- WebSocket client with automatic reconnection
- Pipeline Funnel widget (heartbeat visualization)
- Kill-switch control with confirmation dialogs
- Paper trade history and analytics

## Latency Budget

### End-to-End Breakdown

| Phase | Component | Target | Max | Measurement Point |
|-------|-----------|--------|-----|-------------------|
| Mempool Reception | searcher-rs WS | 5ms | 10ms | `mempool_received_at` |
| Detection | spanning_tree engine | 15ms | 25ms | `detected_at_ms` |
| Stream Emit | HotPathEmitter | 2ms | 5ms | Redis `XADD` latency |
| Redis Write | `arbx:hot:detected` | 1ms | 3ms | Redis `SLOWLOG` |
| Simulation | sim-ctl REVM | 25ms | 40ms | `sim_completed_at` |
| Scoring | Bayesian allocator | 5ms | 10ms | `scored_at_ms` |
| WebSocket Emit | api-server | 2ms | 5ms | `ws_broadcast_at` |
| Edge Proxy | Cloudflare Worker | 5ms | 10ms | `cf_colo` header |
| **TOTAL** | **End-to-End** | **<60ms** | **<100ms** | `pipeline_latency_ms` |

### Latency Monitoring

```promql
# Pipeline p99 latency
histogram_quantile(0.99, 
  rate(arbx_pipeline_latency_ms_bucket[5m])
)

# Stream backlog by consumer group
redis_stream_length{stream="arbx:hot:detected"}

# Consumer group pending count
redis_stream_pending{stream="arbx:hot:detected", group="paper-executor-g0"}
```

## Data Flow Detailed

### Opportunity Lifecycle

```mermaid
sequenceDiagram
    participant MEM as Mempool
    participant SR as searcher-rs
    participant RD as Redis
    participant SIM as sim-ctl
    participant API as api-server
    participant PG as PostgreSQL
    participant WS as WebSocket

    MEM->>SR: pendingTransaction (WS)
    activate SR
    SR->>SR: Decode + Enrich
    SR->>SR: spanning_tree discovery
    SR->>RD: XADD arbx:hot:detected
    deactivate SR

    activate RD
    RD->>SIM: XREADGROUP (paper-executor-g0)
    deactivate RD

    activate SIM
    SIM->>SIM: REVM fork simulation
    SIM->>SIM: Gas estimation
    SIM->>RD: XADD arbx:hot:simulated
    deactivate SIM

    RD->>API: XREADGROUP (ws-emitter-g0)
    activate API
    API->>API: Bayesian scoring
    API->>PG: INSERT opportunities
    API->>WS: broadcast opportunity
    deactivate API

    WS->>FE: Socket.IO event
```

### Fail-Honest Behavior Explained

The OMEGA Pipeline implements **Rule R8 (Fail-Honest)** throughout:

1. **No Synthetic Data:** If a detection fails, no opportunity is emitted. The stream entry contains `rejection_reason`, never fabricated data.

2. **None vs Zero:**
   - `None` = Not computed (simulation pending)
   - `Some(0.0)` = Computed and exactly zero (valid result)

3. **Observation Pattern:** When data is missing, the system emits an observation with the exact reason:
   - `impact_zero` - No state divergence detected
   - `discovery_failed` - Pool discovery returned empty
   - `discovery_no_pool_found` - Known tokens but no connecting pool
   - `missing_reserves` - RPC returned empty reserves
   - `unknown_token_price` - No price oracle for token pair
   - `no_base_candidates` - No viable base tokens for path
   - `watchlist_empty` - No tokens in configured watchlist

4. **Redis Failure Mode:** If Redis is unavailable:
   - searcher-rs → ERROR log + memory queue (bounded)
   - api-server → 503 response (never cached stale data)
   - edge → Pass-through to api-server (no KV cache)

## Component Interactions

### Inter-Service Communication

```mermaid
flowchart LR
    subgraph External["External"]
        RPC["Alchemy/Infura\nRPC HTTP/WS"]
        CF["Cloudflare\nCDN + WAF"]
    end

    subgraph Services["Core Services"]
        SR["searcher-rs\n:9001"]
        SA["selector-api\n:3002"]
        SC["sim-ctl\n:3003"]
        RE["recon\n:3004"]
        RC["relays-client\n:3005"]
        AS["api-server\n:8080"]
    end

    subgraph Data["Data Plane"]
        RD["Redis\n:6379"]
        PG["PostgreSQL\n:5432"]
    end

    subgraph Edge["Edge Layer"]
        EW["edge-worker\n:8787"]
    end

    RPC <-->|"WS + HTTP"| SR
    SR <-->|"Pub/Sub"| RD
    SR <-->|"XADD/XREAD"| RD
    SC <-->|"HTTP"| SR
    SC <-->|"Redis"| RD
    AS <-->|"HTTP"| SA
    AS <-->|"HTTP"| SC
    AS <-->|"HTTP"| RE
    AS <-->|"HTTP"| RC
    AS <-->|"HTTP"| SR
    AS <-->|"SQL"| PG
    AS <-->|"Redis"| RD
    EW <-->|"HTTP"| AS
    CF <-->|"HTTPS"| EW
```

### Authentication Flow

```mermaid
sequenceDiagram
    participant FE as Frontend
    participant EDGE as Edge Worker
    participant API as api-server
    participant KS as KillSwitch

    FE->>EDGE: POST /admin/session
    EDGE->>API: Forward + edge token
    API->>API: Validate admin token
    API->>EDGE: Set-Cookie: arbx_admin_session
    EDGE->>FE: httpOnly cookie set

    FE->>EDGE: GET /api/killswitch/status
    EDGE->>EDGE: Resolve cookie
    EDGE->>API: Forward + admin token
    API->>KS: Query state
    KS->>API: Current state
    API->>EDGE: State JSON
    EDGE->>FE: Response
```

## External Dependencies

| Service | Purpose | Version | Critical Path |
|---------|---------|---------|---------------|
| Redis | Streams, Pub/Sub, Cache | 7.2 | Yes |
| PostgreSQL | Persistence, Analytics | 15 | Yes |
| Alchemy | RPC + Mempool WS | Latest | Yes |
| Cloudflare | Edge, KV, D1 | Workers | Yes |
| Prometheus | Metrics | v2.55 | No |
| Grafana | Visualization | v11.4 | No |
| Vault | Secrets | 1.18.1 | Boot only |

## Related Documentation

- [Runbook](./runbook.md) - Operational procedures and troubleshooting
- [Deployment Guide](./deployment-guide.md) - Environment setup and deployment
- [API Reference](./api-reference.md) - Endpoints and WebSocket events
- [Redis Schema](../redis-schema/hot-path-v2.md) - Detailed stream schemas
- [ADR-001](../adr/001-paper-mode-architecture.md) - Paper mode architecture decision
- [ADR-002](../adr/002-kill-switch-fail-closed.md) - Kill-switch design

---

*Document maintained by OMEGA Architecture Team. For updates, follow the documentation change process in `docs/governance/DATA-MATRIX.md`.*
