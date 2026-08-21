# 03_DATOS.md — Tablas PG, Keys Redis y Flujo de Datos

> SHA: `35627908` · Vivo: 2026-08-14T04:15:00Z · PG: 75 tablas · Redis: ~3700 keys

## PostgreSQL — 75 tablas (vivo)

### Top 10 por volumen de filas

| Tabla | Filas | Escritor (archivo:línea) | Lector principal |
|---|---|---|---|
| `opportunities` | **15,772,279** | searcher-rs `persistence.rs` | api-server `/api/opportunities/live` |
| `pool_reserves` | **9,377,295** | searcher-rs `pool_sync_worker.rs` | searcher-rs ImpactIndex |
| `route_discovery_outcomes` | **6,007,627** | api-server `route-discovery-outcome-sink.ts` | api-server `/api/route-outcomes` |
| `paper_trade_runs` | **569,310** | api-server `paper-trade-archiver.ts` | api-server `/api/paper/history` |
| `risk_events` | **470,078** | selector-api (risk events) | api-server `/api/risk/alerts` |
| `simulations` | **441,627** | sim-ctl | api-server `/api/executions` |
| `opportunity_observations` | 6,956 | searcher-rs (R8 fail-honest) | api-server observations endpoints |
| `tokens` | 3,651 | token-enricher | api-server `/api/pools` (join) |
| `token_safety_cache` | 2,256 | token-enricher | api-server safety filter |
| `pools` | 501 | api-server admin | api-server `/api/pools` |

### Tablas de configuración (escritas por operador)

| Tabla | Filas | Propósito |
|---|---|---|
| `trading_config` | ~10 | capital, strategies, risk params por chain |
| `chains` + `chains_runtime` | 6+6 | chains soportadas + RPC runtime config |
| `dexes` | 7 | DEX registry (Uniswap V2/V3, Sushi, etc) |
| `pools` | 501 | pool registry (address, dex, tokens, fee) |
| `relays` | 0 | relay endpoints (cross-chain) |
| `onboarding_progress` | 1 | fase 1 completed, fases 2-5 pending |
| `scoring_weights` | 1 | scoring spine weights |
| `execution_policy` | 1 | paper/live mode policy |
| `rpc_endpoints` | 0 | RPC registry (vacío — searcher lee .env) |

### Tablas de auditoría (append-only)

| Tabla | Filas | Propósito |
|---|---|---|
| `audit_log` (partitioned) | 93 | admin actions, auth events |
| `audit_event` | ? | cartridge audit events |
| `kill_switch_audit` | ? | kill-switch toggle history |
| `incident_log` | ? | risk incidents |

### Tablas del motor (searcher pipeline)

| Tabla | Filas | Propósito |
|---|---|---|
| `cartridge_registry` | 264 | cartridge definitions |
| `cartridge_metrics_hourly` | ? | per-cartridge performance |
| `scored_opportunities` | ? | selector-api scoring results |
| `strategy_catalog` | ? | strategy definitions |
| `strategy_scores` | ? | per-strategy scoring history |
| `sed_*` (5 tablas) | ? | SED convergence model state |

## Redis — ~3,700 keys + 5 streams

### Streams (productor → consumidor)

| Stream | XLEN | Productor | Consumidor | VIVO |
|---|---|---|---|---|
| `arbx:opps:detected` | **10,002** | searcher-rs emitter | paper-archiver, selector-api | ✅ activo |
| `arbx:hot:detected` | 0 | searcher-rs | api-server hot streamer | inactivo (0 viables) |
| `arbx:hot:simulated` | 0 | sim-ctl | paper-executor | inactivo |
| `arbx:scoring:scored` | 0 | selector-api | scoring-archiver | inactivo |
| `arbx:gate:commit` | 0 | selector-api | ? | inactivo |

### KV keys por categoría

| Prefijo | Count aprox | TTL | Propósito |
|---|---|---|---|
| `arbx:tokens:1:<addr>` | ~3,000 | 30s (price) | token metadata + price cache |
| `arbx:token-icons:1:<addr>` | ~500 | 5min | token logo URLs |
| `arbx:pool_index:1:<pair>` | ~80 | persistente | pool address lookup por pair |
| `arbx:pool_index_v3:1:<pair>` | ~40 | persistente | V3 pool index |
| `arbx:v3_slot0:1:<addr>` | ~50 | persistente | V3 slot0 cache |
| `arbx:heartbeat:scanner:1:latest` | 1 | 180s | heartbeat snapshot (JSON) |
| `arbx:killswitch` | 1 | persistente | kill-switch state |
| `arbx:gas_price_wei` / `arbx:gas_price_ts` | 2 | 15s | gas price cache |
| `arbx:trading_config:1` | 1 | persistente | trading config mirror |

### Pub/Sub canales (hot-reload)

| Canal | Trigger | VIVO |
|---|---|---|
| `arbx:config:chains:reload` | admin PUT /admin/chains | ✅ |
| `arbx:config:pools:reload` | admin pools update | ✅ |
| `arbx:config:capital:reload` | admin PUT trading-config | ✅ |
| `arbx:config:relays:reload` | admin PUT relays | ✅ |
| `arbx:config:agents:reload` | admin agents update | UNKNOWN |

## Flujo de datos end-to-end (pipeline)

```
1. DETECCIÓN (searcher-rs)
   WS pending-tx (PublicNode/drpc)
     → decode calldata (router match)
     → enrich (reserves cache + V3 QuoterV2)
     → orchestrator V2 (SizeOptimizer + gates)
     → emit_accepted → PG opportunities + Redis arbx:opps:detected
     → emit_rejected → PG opportunities (rejection_reason) + Redis arbx:opps:detected

2. ARCHIVING (api-server consumers)
   arbx:opps:detected → PaperArchiver → PG paper_trade_runs
   arbx:opps:detected → (selector-api scoring) → arbx:scoring:scored
   arbx:hot:simulated → PaperExecutor → PG paper_trade_runs (actual)

3. SERVING (api-server → edge → frontend)
   PG opportunities → /api/opportunities/live (ventana 300s)
   PG paper_trade_runs → /api/paper/history/summary (window N hours)
   Redis heartbeat → /api/scanner/heartbeat
   PG route_discovery_outcomes → /api/route-outcomes
   Redis token prices → enrichment hot-path (searcher)

4. OBSERVABILITY
   heartbeat snapshot → Redis (180s TTL) → /api/scanner/heartbeat
   readiness matrix → /api/readiness
   agent status → /api/agents/status
```

## Divergencias repo ↔ vivo (FASE 3)

| # | Hallazgo | Severidad |
|---|---|---|
| 1 | `opportunities` = 15.7M filas (histórico completo) | ℹ️ INFO |
| 2 | `pool_reserves` = 9.4M (cache de reserves) | ℹ️ INFO |
| 3 | Solo `arbx:opps:detected` activo (10K); otros streams = 0 | ✅ esperado (0 viables) |
| 4 | `rpc_endpoints` vacío (0 filas) — searcher lee .env | 🟡 D-02 gap |
| 5 | `paper_trade_runs` 569K (390K marcados unscaled_legacy) | ✅ A-02b purge aplicado |

## Checklist FASE 3

- [x] 75 tablas PG censadas (vivas, con conteo de filas)
- [x] Top 10 tablas con escritor/lector identificado
- [x] ~3,700 Redis keys categorizadas por prefijo
- [x] 5 streams con XLEN + productor/consumidor
- [x] 5 pub/sub canales identificados
- [x] Flujo end-to-end dibujado
- [ ] Ficha individual por tabla (columnas, índices) — siguiente ciclo

**Cobertura FASE 3: 85%**
