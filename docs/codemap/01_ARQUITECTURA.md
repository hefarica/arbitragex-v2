# 01_ARQUITECTURA.md — Esqueleto y Arranque

> SHA: `35627908` · Vivo: 2026-08-14T04:05:00Z · 7/7 servicios UP

## Diagrama de arquitectura (repo + vivo)

```
Browser (arbx.ape-tv.net)
  │
  ├── HTML/JS ──────────────► nginx ──► Frontend Next.js (:5173, SSR)
  │                                      └─ SSR fetch ──► edge (:8787, INTERNAL_EDGE_URL)
  ├── /api/* ───────────────► nginx ──► Edge Worker Hono (:8787)
  │                                      ├─ proxy read ──► api-server (:8080)
  │                                      ├─ proxy admin (cookie/header)
  │                                      └─ Redis KV (edge:worker:* prefix)
  └── WS /socket.io ─────────► nginx ──► api-server (:8080) [directo, NO via edge]
  
api-server (:8080) — Express TS
  ├─ PostgreSQL (postgres:5432)
  │   └─ 19+ tablas (opportunities, paper_trade_runs, audit_log, trading_config, ...)
  ├─ Redis (redis:6379)
  │   ├─ Streams: arbx:opps:detected, arbx:hot:detected, arbx:hot:simulated, arbx:scoring:scored
  │   ├─ Pub/Sub: arbx:config:*:reload (hot-reload chains/pools/capital/...)
  │   └─ KV: readiness snapshot (15s TTL), heartbeat snapshot, token prices
  ├─ Service health probes ──► 6 servicios backend
  ├─ Paper Trade Archiver (consumer arbx:opps:detected → paper_trade_runs)
  ├─ Paper Executor (consumer arbx:hot:simulated)
  ├─ Scoring Archiver (consumer arbx:scoring:scored)
  └─ ~56 HTTP routes (index.ts:56 app.get/post)

Servicios backend (Docker network):
  searcher-rs  (:9001) — Rust — WS subscribe pending-tx → decode → enrich → orchestrator V2 → emit
  selector-api (:3002) — TS   — scoring spine (confidence scoring, gates, emit arbx:scoring:scored)
  sim-ctl      (:3003) — Rust — REVM simulator (paper-shadow execution simulation)
  recon        (:3004) — Rust — post-execution reconciliation (drift tracking)
  relays-client(:3005) — Rust — execution terminus (broadcast gate, paper/live mode switch)
  math-engine  (:3006) — Rust — §IV math evidence vectors + posterior scoring
  token-enricher(:9004)— Rust — token metadata + logo resolution (DexScreener fallback)
```

## Entrypoints por servicio

| Servicio | Lenguaje | Entrypoint | Archivo:línea |
|---|---|---|---|
| api-server | TS | `createServer(app)` | `backend/api-server/src/index.ts:1466` |
| edge worker | TS | `serve({ fetch: app.fetch })` | `edge/worker/src/node-server.ts:14` |
| frontend | TS | `next start` (Next.js) | `frontend/Dockerfile` (CMD) |
| searcher-rs | Rust | `#[tokio::main] async fn main()` | `backend/searcher-rs/src/main.rs:217` |
| selector-api | TS | `app.listen(PORT)` | `backend/selector-api/src/index.ts:167` |
| sim-ctl | Rust | `#[tokio::main] async fn main()` | `backend/sim-ctl/src/main.rs:438` |
| recon | Rust | `#[tokio::main] async fn main()` | `backend/recon/src/main.rs:183` |
| relays-client | Rust | `#[tokio::main] async fn main()` | `backend/relays-client/src/main.rs:108` |
| math-engine | Rust | `#[tokio::main] async fn main()` | `backend/math-engine/src/main.rs:21` |
| token-enricher | Rust | `#[tokio::main] async fn main()` | `backend/token-enricher/src/main.rs:430` |

## Mapa de servicios (compose ↔ vivo)

| Servicio | Puerto (compose) | VIVO | Health |
|---|---|---|---|
| postgres | 5432 | ✅ CONFIRMADO (api-server queries exitosas) | service_healthy |
| redis | 6379 | ✅ CONFIRMADO (streams activos) | service_healthy |
| api-server | 8080 | ✅ CONFIRMADO (`/api/status` 200) | — |
| edge | 8787 | ✅ CONFIRMADO (`/health` 200) | depends api-server |
| frontend | 5173 | ✅ CONFIRMADO (24/24 páginas 200) | depends edge |
| searcher-rs | 9001 | ✅ CONFIRMADO (`/api/status` services.searcher-rs.ok=true) | — |
| selector-api | 3002 | ✅ CONFIRMADO | — |
| sim-ctl | 3003 | ✅ CONFIRMADO | — |
| recon | 3004 | ✅ CONFIRMADO | — |
| relays-client | 3005 | ✅ CONFIRMADO | — |
| math-engine | 3006 | ✅ CONFIRMADO | — |
| token-enricher | 9004 | ✅ CONFIRMADO | — |

**7/7 servicios backend UP. Kill-switch: `disabled` (paper-only, postura segura).**

## Comunicación entre servicios

### Redis Streams (productor → consumidor)

| Stream | Productor | Consumidor | Archivo productor:línea |
|---|---|---|---|
| `arbx:opps:detected` | searcher-rs (emit_accepted) | paper-archiver (api-server), selector-api | `opportunity_emitter.rs:250` |
| `arbx:hot:detected` | searcher-rs | api-server (hot streamer) | `opportunity_emitter.rs` |
| `arbx:hot:simulated` | sim-ctl | paper-executor (api-server) | `sim-ctl/src/` |
| `arbx:scoring:scored` | selector-api | scoring-archiver (api-server) | `selector-api/src/` |
| `arbx:gate:commit` | selector-api | ? | `selector-api/src/` |

### Redis Pub/Sub (config hot-reload)

| Canal | Trigger | Consumidor |
|---|---|---|
| `arbx:config:chains:reload` | admin PUT /admin/chains | searcher-rs (ChainSupervisor) |
| `arbx:config:pools:reload` | admin PUT pools | searcher-rs (ImpactIndex) |
| `arbx:config:capital:reload` | admin PUT trading-config | searcher-rs (TradingConfigClient) |
| `arbx:config:relays:reload` | admin PUT relays | relays-client |

### HTTP inter-servicio (Docker network)

| Caller | Target | Propósito |
|---|---|---|
| api-server | searcher-rs:9001 | health probe, heartbeat |
| api-server | selector-api:3002 | health probe |
| api-server | sim-ctl:3003 | health probe |
| api-server | recon:3004 | health probe |
| api-server | relays-client:3005 | health probe |
| api-server | math-engine:3006 | health probe |
| api-server | token-enricher:9004 | health probe |
| selector-api | math-engine:3006 | evidence vectors |
| edge worker | api-server:8080 | proxy upstream |

## ENV VARS críticas (boot-required)

| Var | Servicio | Propósito |
|---|---|---|
| `REDIS_URL` | api-server, searcher-rs | Redis connection |
| `DATABASE_URL` | api-server | PostgreSQL pool |
| `ARBX_ADMIN_TOKEN` | api-server, edge | Admin auth (≥32 bytes entropy) |
| `ARBX_EDGE_TOKEN` | edge, api-server | Edge↔api-server auth |
| `ARBX_SERVICE_TOKEN` | api-server | Inter-service POST auth |
| `RPC_WS_1` | searcher-rs | WebSocket pending-tx subscription |
| `RPC_HTTP_1` | searcher-rs | HTTP RPC (V3 QuoterV2 multicall) |
| `ARBX_ORCHESTRATOR_MODE` | searcher-rs | v1/v2/shadow/off (default: v2) |
| `ARBX_MEMPOOL_MODE` | searcher-rs | auto/firehose/filtered/block/disabled |
| `NEXT_PUBLIC_EDGE_URL` | frontend (baked) | Edge URL for client fetches |
| `INTERNAL_EDGE_URL` | frontend (runtime) | Edge URL for SSR fetches |

## Divergencias repo ↔ vivo (FASE 1)

| # | Hallazgo | Severidad |
|---|---|---|
| 1 | Kill-switch `disabled` (compose) vs `enabled` en lecturas previas | ℹ️ INFO — postura paper-only segura |
| 2 | sim-ctl/recon/math-engine son Rust (no TS como pensaba el censo inicial) | ℹ️ INFO — corrección de categoría |

## Checklist FASE 1

- [x] Árbol anotado (directorios + archivos clasificados)
- [x] Entrypoints de cada servicio (10 servicios)
- [x] Mapa de servicios (puertos, DNS Docker, dependencias)
- [x] Diagrama repo + diagrama live
- [x] Cruce compose ↔ /api/status (7/7 UP)
- [x] Redis streams + pub/sub mapeados
- [x] ENV vars críticas censadas (11 boot-required)
- [ ] Diagrama renderizado (mermaid/SVG) — siguiente ciclo

**Cobertura FASE 1: 90%**
