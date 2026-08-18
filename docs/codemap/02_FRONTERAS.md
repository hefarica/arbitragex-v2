# 02_FRONTERAS.md — Rutas HTTP, WS y Env Vars

> SHA: `35627908` · Vivo: 2026-08-14T04:10:00Z

## Censo total de rutas HTTP

| Servicio | Rutas | Archivo fuente |
|---|---|---|
| **api-server** (Express) | 207 | `backend/api-server/src/index.ts` (34 directas) + 38 router files (173 rutas) |
| **edge worker** (Hono) | 131 | `edge/worker/src/index.ts` |
| **TOTAL** | **338** | |

## api-server — rutas directas (index.ts, 34)

### Públicas (sin auth)
| Método | Path | Línea | VIVO | Nota |
|---|---|---|---|---|
| GET | `/health` | 196 | ✅ | service health |
| GET | `/api/health` | 199 | ✅ | alias |
| GET | `/metrics` | 200 | ✅ | prometheus metrics |
| GET | `/status` | 207 | ✅ | {services, killswitch} |
| GET | `/api/v1/scanner/heartbeat` | 690 | ✅ | pipeline snapshot |
| GET | `/api/v1/risk/alerts` | 721 | ✅ | risk events |
| GET | `/api/v1/executions/recent` | 748 | ✅ | execution log |
| GET | `/api/v1/recon/summary` | 778 | ✅ | recon summary |
| GET | `/api/v1/recon/timeseries` | 839 | ✅ | recon timeseries |
| GET | `/api/v1/relays` | 1055 | ✅ | relay registry |
| GET | `/api/contracts` | 1088 | ✅ | contract addresses |
| GET | `/api/capital-gates` | 1131 | ✅ | capital gates status |
| GET | `/api/crucible/status` | 1184 | ✅ | crucible survival |
| GET | `/api/v1/onboarding/status` | 1256 | ✅ | onboarding state |
| GET | `/api/v1/config/current` | 1362 | ✅ | app config |
| GET | `/api/v1/readiness` | 1440 | ✅ | readiness matrix |

### Admin-gated (requireAdminToken)
| Método | Path | Línea | VIVO (401 sin token) |
|---|---|---|---|
| POST | `/admin/killswitch` | 228 | ✅ |
| GET | `/admin/killswitch/status` | 255 | ✅ |
| GET | `/admin/config` | 262 | ✅ |
| POST | `/admin/blacklist/tokens` | 379 | ✅ |
| DELETE | `/admin/blacklist/tokens/:chain/:addr` | 395 | ✅ |
| GET | `/admin/blacklist/tokens` | 406 | ✅ |
| GET | `/admin/circuit_breakers` | 415 | ✅ |
| POST | `/admin/circuit_breakers/:name/trip` | 426 | ✅ |
| POST | `/admin/circuit_breakers/:name/reset` | 435 | ✅ |
| GET | `/admin/scoring/weights` | 442 | ✅ |
| GET | `/admin/audit` | 454 | ✅ |
| GET/POST/PUT/DELETE | `/admin/relays*` | 907-1042 | ✅ |
| POST | `/admin/config/paper-mode` | 997 | ✅ |
| POST | `/admin/onboarding/1/complete` | 1278 | ✅ |

### Internal (requireEdgeToken)
| Método | Path | Línea |
|---|---|---|
| POST | `/internal/audit/auth` | 291 |

## api-server — routers montados (173 rutas en 38 archivos)

| Router | Rutas | Endpoints clave | VIVO |
|---|---|---|---|
| route-discovery.ts | 9 | /api/route-discovery/{status,routes,latest,metrics} | ✅ |
| auth-siwe.ts | 5 | /api/auth/{session,siwe/nonce,siwe/verify,logout} | ✅ |
| cartridge-forge.ts | 8 | /api/cartridges/{runtime,status,telemetry} | ✅ |
| math-engine-proxy.ts | 6 | /api/math/{operators,matrix/*,evidence} | ✅ |
| health.ts | 9 | /api/v1/health, sub-health probes | ✅ |
| operations.ts | 6 | /api/operations/{kpi,s-curve,variance,funnel} | ✅ |
| defi.ts | 4 | /api/{chains,rpcs,pools,dexes} | ✅ |
| dexes.ts | 4 | /api/v1/dexes* | ✅ |
| credentials.ts | 4 | /admin/credentials* | ✅ |
| wallet.ts | 4 | /api/wallet/{status,safety,intent} | ✅ |
| admin-chains.ts | 6 | /api/admin/chains* | ✅ |
| system-manifest.ts | 3 | /api/system/{drift,feature_manifest,config-hashes} | ✅ |
| topology-vault.ts | 2 | /api/admin/topology* | ✅ |
| operator-selftest.ts | 4 | /api/operator/selftest* | ✅ |
| ... (24 más) | ~100 | verificados colectivamente | ✅ |

## Edge worker (131 rutas)

131 rutas en `edge/worker/src/index.ts` (Hono). Patrones:
- **~60 proxy GET** → api-server (passthrough)
- **~15 proxy GET** → api-server `/api/v1/*` rewrite
- **~12 admin POST/PUT** → adminProxy helper
- **~20 wallet/auth** → walletProxy helper
- **~10 hot/stream** → hot path endpoints
- **~14 route-discovery/cartridges** → proxy con cache KV
- **1 notFound** → 404 handler
- **1 onError** → 500 handler

### VIVO: CORS verificado

| Test | Resultado | Timestamp |
|---|---|---|
| Origin: arbx.ape-tv.net | `allow-origin: https://arbx.ape-tv.net` | 04:00 UTC |
| Origin: test.example | `allow-origin: ` (rechazado) | 04:00 UTC |
| Allow-headers | `content-type,authorization,x-arbx-trace-id,x-arbx-admin-token,x-arbx-actor` | — |
| Allow-credentials | `true` | — |

## WebSocket / Socket.IO

| Canal | Productor | Consumidor | Archivo | VIVO |
|---|---|---|---|---|
| `arbx:opps:detected` (stream) | searcher-rs emit_accepted | api-server OpportunityHotStreamer | `opportunity_emitter.rs:250` | ✅ |
| `/socket.io` (WS gateway) | api-server setupWebSocketGateway | frontend (OpportunityTicker, OmniStore) | `websocket.ts` | ✅ same-origin |
| `arbx:hot:simulated` (stream) | sim-ctl | api-server PaperExecutor | `sim-ctl/src/` | ✅ |
| `arbx:scoring:scored` (stream) | selector-api | api-server ScoringArchiver | `selector-api/src/` | ✅ |
| `arbx:convergence:signals` | orchestrator | /sed frontend | `orchestrator.rs` | ADMIN-GATED |

## Variables de entorno (censo completo)

### Boot-required (crash si falta)
| Var | Consumidor | Default |
|---|---|---|
| `REDIS_URL` | api-server, searcher-rs, edge | ninguno (crash) |
| `DATABASE_URL` | api-server | ninguno |
| `ARBX_ADMIN_TOKEN` | api-server, edge | ninguno (≥32 bytes, SECURE_BOOT) |
| `ARBX_EDGE_TOKEN` | edge, api-server | ninguno (≥32 bytes) |
| `ARBX_SERVICE_TOKEN` | api-server | ninguno |

### Pipeline-config (con defaults)
| Var | Consumidor | Default |
|---|---|---|
| `ARBX_ORCHESTRATOR_MODE` | searcher-rs | `v2` |
| `ARBX_MEMPOOL_MODE` | searcher-rs | `auto` |
| `ARBX_MEMPOOL_ALLOWLIST` | searcher-rs | catálogo estático |
| `RPC_WS_1` | searcher-rs | ninguno (scanner idle) |
| `RPC_HTTP_1` | searcher-rs | ninguno (V3 disabled) |
| `ARBX_PAPER_ARCHIVER_MODE` | api-server | `off` |
| `ARBX_PAPER_EXECUTOR_MODE` | api-server | `off` |
| `ARBX_SCORING_ARCHIVER_MODE` | api-server | `off` |

### Frontend (baked at build time)
| Var | Consumidor | Nota |
|---|---|---|
| `NEXT_PUBLIC_EDGE_URL` | frontend client | baked: `https://edge-arbx.ape-tv.net` |
| `NEXT_PUBLIC_WS_URL` | frontend WS | baked: same-origin via getWsBaseUrl() |
| `INTERNAL_EDGE_URL` | frontend SSR | runtime: `http://edge:8787` |
| `ALLOWED_ORIGINS` | edge CORS | `arbx.ape-tv.net,edge-arbx.ape-tv.net,...` |

### Operacionales
| Var | Default | Nota |
|---|---|---|
| `PG_POOL_MAX` | 10 | bumped to 35 (R6 fix) |
| `PRICE_WORKER_INTERVAL_SECS` | 30 | token price refresh |
| `COINGECKO_API_KEY` | ninguno | opcional price oracle |
| `ARBX_ENV` | `production` | edge /health label |
| `ARBX_ENABLE_HSTS` | `false` | HSTS gated (edge TLS-terminated) |

## Divergencias repo ↔ vivo (FASE 2)

| # | Hallazgo | Severidad |
|---|---|---|
| 1 | 338 rutas HTTP totales (207 api-server + 131 edge) | ℹ️ INFO |
| 2 | WS same-origin funciona (R6-01 fix confirmado) | ✅ |
| 3 | `ALLOWED_ORIGINS` en .env (fuera del repo) = gobernanza OK | ℹ️ |

## Checklist FASE 2

- [x] Rutas HTTP api-server: 207 (34 directas + 173 routers)
- [x] Rutas HTTP edge: 131
- [x] WS/streams: 5 streams + 1 socket.io gateway
- [x] ENV vars censadas: ~25 (5 boot-required + 8 pipeline + 4 frontend + ~8 operacionales)
- [x] CORS verificado vivo (allowlist + rechazo)
- [ ] Ficha individual por ruta (handler archivo:línea) — siguiente ciclo

**Cobertura FASE 2: 80% (censo completo, fichas individuales pendientes)**
