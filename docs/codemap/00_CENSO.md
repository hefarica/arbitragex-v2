# 00_CENSO.md — Censo Total del Repo

> SHA: `35627908401cc8e2df2258292bad720764cb073e` (2026-08-13 06:40:58 UTC)
> Timestamp censo: 2026-08-14T03:58:25Z
> Exclusiones: node_modules, .git, target, dist, .next, _backup*, app_backup, .omega_extracted

## Censo por categoría

| Categoría | Archivos | Descripción |
|---|---|---|
| **CÓDIGO-FE** | ~430 tsx + 375 ts | Frontend Next.js 14 App Router (56 páginas) |
| **CÓDIGO-EDGE** | 11 ts | Edge worker Hono + dev-local Express |
| **CÓDIGO-API** | 138 ts | api-server (Express + PG + Redis) |
| **CÓDIGO-RS** | 345 rs | searcher-rs + sim-ctl + recon + selector-api + relays-client + math-engine + token-enricher |
| **CÓDIGO-SOL** | 45 sol | Contratos Solidity (Uniswap forks, executor, flash loan) |
| **SHARED-TS** | 18 ts | Contratos Zod + tipos compartidos |
| **Rhai** | 271 rhai | Cartuchos de estrategia (264 + 7 base) |
| **CONFIG** | ~100 yml + env + toml | compose, CI workflows, app config |
| **INFRA** | 20+ Dockerfiles | Multi-stage builds por servicio |
| **TEST** | ~300 test/spec | Unit + integration + e2e |
| **DATO** | 94 sql | Migraciones PG (001-103+) |
| **DOC** | ~400 md | Diseño, runbooks, auditorías, handoffs |
| **SCRIPT** | ~100 sh | Deploy, env-deploy, lint, ethics-guard |

## Censo por directorio top-level

| Directorio | Archivos | Categoría dominante |
|---|---|---|
| contracts/ | 2630 | SOL + lib (openzeppelin) |
| backend/ | 1082 | RS + TS (api-server) |
| frontend/ | 621 | TSX + TS + CSS |
| tools/ | 583 | JSON (chain catalogs, configs) |
| docs/ | 202 | MD (diseño, auditorías) |
| database/ | 96 | SQL migrations |
| scripts/ | 71 | SH (deploy, env) |
| edge/ | 21 | TS (worker + dev-local) |
| shared-ts/ | 20 | TS (contracts) |
| monitoring/ | 24 | YML (prometheus, grafana, loki) |

## Servicios del repo (compose.prod.yml)

| Servicio | Puerto | Lenguaje | Dockerfile |
|---|---|---|---|
| frontend | 5173 | TS (Next.js) | frontend/Dockerfile |
| edge | 8787 | TS (Hono worker) | edge/worker/Dockerfile.node |
| api-server | 8080 | TS (Express) | backend/api-server/Dockerfile |
| searcher-rs | 9001 | Rust | backend/searcher-rs/Dockerfile |
| selector-api | 3002 | TS (Express) | backend/selector-api/Dockerfile |
| sim-ctl | 3003 | TS (Express) | backend/sim-ctl/Dockerfile |
| recon | 3004 | TS (Express) | backend/recon/Dockerfile |
| relays-client | 3005 | Rust | backend/relays-client/Dockerfile |
| math-engine | 3006 | TS (Express) | backend/math-engine/Dockerfile |
| token-enricher | 9004 | Rust | backend/token-enricher/Dockerfile |
| postgres | 5432 | — | postgres:16 |
| redis | 6379 | — | redis:7 |
| prometheus | 9090 | — | monitoring/ |
| grafana | 3000 | — | monitoring/ |
| loki | 3100 | — | monitoring/ |

## Estado del censo

- [x] Total archivos clasificados por categoría
- [x] Directorios top-level inventariados
- [x] Servicios del compose identificados
- [ ] Fichas individuales de archivos de código (FASE 1+)
- [ ] Clasificación TRIVIAL/GENERADO/MUERTO? (FASE 1+)

**Cobertura censo repo: 100% (categoría) → 15% (fichas individuales)**
