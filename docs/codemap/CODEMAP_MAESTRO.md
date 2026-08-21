# CODEMAP_MAESTRO.md — ArbitrageX v2

> Sistema explicado de arriba abajo usando solo hechos citados de los artefactos 00-07 + 04b.
> SHA: `35627908` · Vivo: 2026-08-14 · Generado: FASE 8 del codemap loop v3 (+ FASE 4$ cadenas de dinero).

## §CAPACIDAD ECONÓMICA (v3, LEY M7/M8)

### Resumen ejecutivo

**El sistema NO gana dinero hoy.** Es una máquina de detección de arbitraje en paper-shadow mode con capital $0 y cero ejecuciones on-chain. Pero NO es una máquina rota — es una máquina cara con la carretera 90% pavimentada y 2 bloqueos específicos en el camino al primer dólar.

| Dimensión | Valor | Etiqueta |
|---|---|---|
| Estrategias OPERATIVAS en paper | **0** | ninguna pasa GATES (safety_below_threshold 100%) |
| Estrategias PARCIAL-4 (GATES roto) | **2** (dex_arb + 264 cartridges) | detectan pero safety score < umbral |
| Estrategias PARCIAL-1 (DETECTA roto) | **2** (triangular_arb, flashloan_arb) | engine nunca invocado (B-02) |
| Estrategias DORMIDAS | **11** (liquidation + 10 engines) | código completo, deshabilitado |
| P&L SIMULADO 24h | 6 filas, $10.67 avg | SIMULADO (paper_trade_runs) |
| P&L REAL | **$0** | 0 executions, 0 settlement on-chain |
| Capital expuesto | **$0** | paper_only, kill-switch disabled |

### Los 2 bloqueos que separan del dinero

1. **B-02 deserialize (fix en #335)** — sin mempool feed, los engines triangular/flashloan/10-dormant nunca se invocan. El fix ya está en cola de merge. **Esto desbloquea el pipeline mempool-V2 completo.**

2. **Price oracle sin credenciales** — 0 Alchemy hits + 0 CoinGecko hits → safety score 42.2 < threshold 50 → **100% de detecciones rechazadas en gates**. Provisionar una key desbloquea el gate para TODO lo que ya detecta.

### Camino al primer dólar real (6 pasos)

```
#335 deploy → Alchemy key → viables > 0 → onboarding 2-5 → flip paper→live → broadcast
```

Detalle completo en [04b_CADENAS_DE_DINERO.md](04b_CADENAS_DE_DINERO.md).

### Peso muerto arquitectónico (honestidad)

- `/translator` (endpoint 404) y `/live-testnet` (no prioridad): candidatos a retirar
- 3 Dockerfiles legacy en `infra/docker/`: obsoletos
- ~73K docs .md históricos: archivo, no código (no pesan en runtime)

## 1. Qué es ArbitrageX v2

Un sistema de detección y ejecución de arbitraje on-chain para Ethereum mainnet que opera en **paper-shadow mode** (capital expuesto = $0, sin broadcast on-chain). El pipeline detecta oportunidades de arbitraje en el mempool, las evalúa con matemática estocástica, las simula en REVM, y persiste el ciclo completo en PostgreSQL para análisis de drift.

**Arquitectura C-S-E**: Collector (searcher-rs Rust) → Strategy Engine (TS control-plane) → Executor (paper-shadow).

## 2. Los números

| Dimensión | Cifra |
|---|---|
| Archivos repo (excl. node_modules/backups) | 8,871 |
| Servicios Docker | 15 (10 app + 5 infra) |
| Rutas HTTP | 338 (207 api-server + 131 edge worker) |
| Tablas PostgreSQL | 75 |
| Streams Redis | 5 (1 activo) |
| Keys Redis | ~3,700 |
| Páginas frontend | 57 (55×200 + 2 redirect) |
| Workflows CI | 48 (14 required) |
| Cartuchos Rhai | 264 |
| Contratos Solidity | 45 |
| Docs .md | 173 |

## 3. Arquitectura viva (verificada 2026-08-14)

```
                    Browser
                      │
          ┌───────────┼───────────┐
          ▼           ▼           ▼
    arbx.ape-tv.net  arbx.ape-tv.net  edge-arbx.ape-tv.net
    (Next.js :5173)  (/api/* → nginx)  (edge worker :8787)
          │           │                │
          │           ▼                │
          │      edge Hono :8787 ─────┘
          │           │
          │           ▼
          │      api-server :8080 (Express TS)
          │       ├─ PostgreSQL :5432 (75 tablas)
          │       ├─ Redis :6379 (5 streams, ~3700 keys)
          │       └─ Redis pub/sub (config hot-reload)
          │
          └─ WS /socket.io ──► api-server :8080

    api-server health-probes:
      searcher-rs :9001  (Rust — detección pipeline)
      selector-api :3002 (TS — scoring spine)
      sim-ctl :3003      (Rust — REVM simulator)
      recon :3004        (Rust — reconciliación)
      relays-client :3005 (Rust — execution terminus)
      math-engine :3006  (Rust — §IV Bayesian)
      token-enricher :9004 (Rust — token metadata)
```

## 4. El pipeline ( searcher-rs → PG → api-server → edge → frontend)

```
1. DETECT    WS subscribe (PublicNode) → pending-tx hash
2. DECODE    calldata decode (univ2/univ3 router) → RouteIntent[]
3. ENRICH    reserves cache + V3 QuoterV2 → StrategyCandidate
4. SIZE      SizeOptimizer (Kelly + golden-section + gas floor) → SizedCandidate
5. GATE      10 OptimizeRejectReason (non_positive, cap_clamp, gas_floor, kelly, etc.)
6. EMIT      emit_accepted → PG opportunities + Redis arbx:opps:detected
             emit_rejected → PG opportunities (rejection_reason) + Redis
7. ARCHIVE   paper-archiver → PG paper_trade_runs
             scoring-archiver → PG scored_opportunities
             route-discovery-sink → PG route_discovery_outcomes
8. SERVE     api-server → edge → frontend (57 páginas)
```

**Estado actual:** pipeline en 0 (B-02 deserialize fix pendiente deploy #335). Tras deploy, `pending_received > 0`.

## 5. Grupos de datos

### PostgreSQL (75 tablas)
| Grupo | Tablas clave | Volumen |
|---|---|---|
| **Pipeline** | opportunities, paper_trade_runs, scored_opportunities, simulations | 15.7M + 569K + ? |
| **Reserves** | pool_reserves, pools | 9.4M + 501 |
| **Discovery** | route_discovery_outcomes, route_legs | 6M |
| **Risk** | risk_events, incident_log, token_safety_cache | 470K + ? |
| **Config** | trading_config, chains, dexes, pools, rpc_endpoints | ~10 |
| **Audit** | audit_log (partitioned), audit_event, kill_switch_audit | 93 |
| **SED** | sed_opportunities, sed_entropy_metrics, sed_eigenstates, etc. | 5 tablas |
| **Cartridges** | cartridge_registry, cartridge_metrics_hourly | 264 |

### Redis (~3,700 keys + 5 streams)
| Categoría | Keys | Función |
|---|---|---|
| Token cache | arbx:tokens:* (~3,000) | metadata + price 30s TTL |
| Token icons | arbx:token-icons:* (~500) | logo URLs 5min TTL |
| Pool index | arbx:pool_index:* (~80) | pair → pool address lookup |
| V3 cache | arbx:v3_slot0:* (~50) | V3 slot0 state |
| Heartbeat | arbx:heartbeat:scanner:1 | 180s TTL snapshot |
| Streams | arbx:opps:detected (10K activo) | pipeline comunicación |
| Pub/sub | arbx:config:*:reload (5 canales) | hot-reload config |

## 6. Frontend (57 páginas)

**55×200 · 2×307 redirects (por diseño) · 0 errores**

| Categoría | Páginas | Estado datos |
|---|---|---|
| Pipeline | /, /opportunities, /executions, /paper/history, /recon | honest empty (0 viables) |
| Infra | /status, /risk, /killswitch, /readiness, /monitor | datos reales |
| Registry | /pools, /chains, /rpcs, /dex-registry, /wallets | datos reales (87 pools ACTIVE) |
| Admin | /audit-logs, /admin/* | cookie-gated correcto |
| Onboarding | /onboarding/1-5 | Fase 1 completa, 2-5 pending |

## 7. Infraestructura

| Componente | Detalle |
|---|---|
| **VPS** | Hetzner, IP 195.201.235.70, /opt/arbitragex-v2 |
| **Proxy** | Cloudflare → nginx (fuera del repo) |
| **CI** | GitHub Actions, 14 required checks, enforce_admins=true |
| **Deploy** | auto-deploy CI post-merge + `scripts/deploy.sh` manual |
| **Monitoring** | Prometheus + Grafana + Loki + Alertmanager + Thanos + MinIO |
| **Domains** | arbx.ape-tv.net (frontend), edge-arbx.ape-tv.net (API) |

## 8. Divergencias repo ↔ vivo (13, consolidadas)

| Severidad | Cantidad | Clave |
|---|---|---|
| 🔴 CRÍTICO | 1 | B-02 deserialize fix pendiente deploy (#335) |
| 🟡 BAJA | 5 | CSP web3modal global, HSTS, /wallet placeholder, /translator 404, nginx fuera repo |
| ℹ️ INFO | 6 | kill-switch disabled, 338 vs 26 endpoints doc'd, rpc_endpoints vacío, 3 Dockerfiles legacy, prompt FANTASÍA en docs, auto-deploy flaky |
| ✅ ESPERADO | 1 | solo 1 stream activo (0 viables) |

## 9. UNKNOWNs restantes (5, meta <10)

| # | Pregunta | Por qué es UNKNOWN |
|---|---|---|
| 1 | ¿Qué SHA exacto corre cada servicio Docker individual? | Solo verificable con docker inspect (no desde fuera) |
| 2 | ¿TTL exacto de arbx:tokens:* keys? | Solo verificable con TTL comando (no desde GET) |
| 3 | ¿Qué consume arbx:gate:commit stream? | Código no localiza el consumidor |
| 4 | ¿nginx config real (upstreams, TLS, rate limits)? | Fuera del repo, no accesible desde fuera |
| 5 | ¿Siempre estuvieron los 12 engines definidos o algunos son stubs? | Requiere profundizar cada engine |

## 10. HALLAZGOS (resumen por severidad)

### CRÍTICO
1. **B-02 deserialize**: PublicNode/drpc envían pending-tx como JSON objects, ethers-rs drop all → pipeline en 0. Fix en #335 (raw JSON subscribe).

### GOBERNANZA
2. **nginx config fuera del repo**: la topología de producción no es reproducible desde el código.
3. **API_CONTRACTS.md cubre 8%** del contrato real (26/338 endpoints).

### DOC DEBT
4. **ARQUITECTURA_TECNICA/ROADMAP/OPERATOR_RUNBOOK** tienen prompt "PREDATOR" embebido — son FANTASÍA parcial.

## 11. Mapa de dependencias de módulos

```
shared-ts (Zod contracts)
  ├─→ frontend (schemas, api-client)
  ├─→ edge worker (contract validation)
  └─→ api-server (OpportunitySchema)

searcher-rs
  ├─→ shared-rs (shared types)
  ├─→ PostgreSQL (opportunities, pool_reserves)
  ├─→ Redis (streams arbx:opps:*, config pub/sub)
  └─→ RPC (WS pending-tx + HTTP QuoterV2)

api-server
  ├─→ PostgreSQL (todas las tablas)
  ├─→ Redis (KV cache + streams consumer)
  ├─→ searcher-rs / selector-api / sim-ctl / recon / relays-client / math-engine / token-enricher (health probes)
  └─→ edge worker (upstream proxy)

edge worker
  ├─→ api-server (proxy)
  ├─→ Redis (KV cache edge:worker:*)
  └─→ frontend (serves /api/*)

frontend
  ├─→ edge worker (REST /api/*)
  ├─→ api-server (WS /socket.io)
  └─→ WalletConnect (solo /wallet, lazy mount)
```

## Checklist final de cobertura

```
Censo repo: archivos clasificados            [100%]  ✅ 8,871
Censo vivo: superficies catalogadas          [100%]  ✅ 14 endpoints + 57 páginas + headers
Archivos de CÓDIGO con ficha                 [ 30%]  entrypoints + engines + gates fichados
Rutas HTTP: handler localizado               [ 80%]  338 censadas, 80% fichadas
Rutas HTTP: verificadas en vivo              [100%]  ✅ 14/14 canónicas
Canales WS/streams mapeados                  [100%]  ✅ 5 streams + 1 gateway
Variables de entorno censadas                [100%]  ✅ ~25 vars
Tablas PG + keys Redis mapeadas              [ 85%]  ✅ 75 tablas + 3700 keys
Servicios con diagrama                       [100%]  ✅ 10/10
Páginas frontend (repo + render vivo)        [100%]  ✅ 57/57
Workflows CI/CD analizados                   [100%]  ✅ 48 (14 required)
Docs .md etiquetados                         [ 85%]  ✅ clave etiquetados
Divergencias repo↔vivo resueltas o abiertas  [100%]  ✅ 13 catalogadas
UNKNOWNs abiertos                             [  5]   ✅ <10

% TOTAL ≈ 90%
```

---
*Generado por FASE 8 del DEEP RESEARCH CODEMAP loop. Artefactos fuente: 00_CENSO, 00b_CENSO_VIVO, 01_ARQUITECTURA, 02_FRONTERAS, 03_DATOS, 04_MOTOR, 05_FRONTEND, 06_INFRA, 07_CONFRONTA.*
