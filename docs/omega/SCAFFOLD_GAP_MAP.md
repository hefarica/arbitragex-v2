# OMEGA v5 — Scaffold Gap Map (FASE 0)

> Corrige el punto más débil del workbook (y del `OMEGA_V5_INGESTION_REPORT.md` previo): la hoja
> `FILE REGISTRY` y la `CHECKLIST` usan una **plantilla genérica de "MEV bot"** cuyas rutas **no existen**
> en este repo. Aquí se cruza cada ruta-plantilla contra el repo REAL (verificado con `ls`/`find` 2026-05-21)
> y se clasifica según el prompt: A=ruta equivalente · B=crear · C=workbook desactualizado · D=deuda · E=gap real que bloquea CI.

## Veredicto global
El repo real es un **monorepo Rust+TS** mucho más construido que la plantilla del workbook:
`backend/` tiene **12 crates/servicios** (`api-server`, `math-engine`, `searcher-rs`, `prioritization-spine`,
`recon`, `relays-client`, `sed-core`, `selector-api`, `shared-rs`, `sim-ctl`, `simulator-v2`, `token-enricher`),
más `contracts/` (Foundry), `edge/{worker,dev-local}`, `frontend/` (Next.js App Router) y `docker/compose.*.yml`.
**Ninguna** ruta-plantilla ausente es un gap real que bloquee CI — son **C (workbook desactualizado)** con **A (equivalente real)**.

## Mapa FILE REGISTRY (plantilla) → repo real
| Ruta en workbook | ¿Existe? | Equivalente real | Clase |
|---|---|---|---|
| `src/main.rs` | ❌ | `backend/searcher-rs/src/` (+ otros crates) — no hay `src/` plano en raíz | C + A |
| `src/arbitrage/mod.rs` | ❌ | `backend/searcher-rs/`, `backend/prioritization-spine/` | C + A |
| `src/mev/flashbots.rs` | ❌ | `backend/relays-client/` (cliente de relays/Flashbots) | C + A |
| `src/chains/ethereum.rs` | ❌ | config multi-chain en `backend/*` + `contracts/script/DeployMultichain.s.sol` | C + A |
| `src/chains/bsc.rs` | ❌ | idem (chains por config, no por archivo) | C + A |
| `contracts/FlashLoan.sol` | ❌ | `contracts/` (Foundry; nombres reales en `contracts/src`/`script`) | C + A |
| `contracts/ArbitrageBot.sol` | ❌ | `contracts/` (executor real con otro nombre) | C + A |
| `backend/src/index.ts` | ❌ | **`backend/api-server/src/index.ts`** ✅ (existe) | A |
| `frontend/pages/index.tsx` | ❌ | `frontend/app/` (App Router, no `pages/`) | C + A |
| `scripts/deploy.py` | ❌ | `contracts/script/Deploy*.s.sol` (Foundry, no Python) | C + A |
| `docker-compose.yml` | ❌ | **`docker/compose.dev.yml` / `compose.prod.yml`** ✅ | A |
| `Cargo.toml` (raíz) | ❌ | **`backend/Cargo.toml`** (workspace) ✅ | A |
| `package.json` (raíz) | ✅ | existe (workspaces) | OK |
| `.github/workflows/ci.yml` | ✅ | existe | OK |

## CHECKLIST — placeholders inválidos (NO ejecutar literal)
| Item workbook | Problema | Acción correcta |
|---|---|---|
| "Compilar módulo Rust → `src/main.ru`" | `.ru` no es Rust; ruta inexistente | `cd backend && cargo check --workspace` (crates reales) — C |
| "Compilar módulo TypeScript → `src/main.ty`" | `.ty` inválido | `npm run typecheck --workspaces` — C |
| "Compilar módulo Solidity → `src/main.so`" | `.so` inválido | `cd contracts && forge build` — C |
| "Clonar y configurar repo → `.git/config`" | el repo ya está clonado y operativo | N/A (ya hecho) — C |
| "Levantar servicios → `docker-compose up`" | usa `docker/compose.dev.yml`; **RULE 01 prohíbe Docker local en Windows** | sólo en VPS/CI — D (bloqueo de entorno, no del repo) |

## Gaps REALES (no plantilla) dignos de backlog
Estos sí salen de señales reales (DEV ENGINE + estado del repo), aterrizados a rutas reales — ver `OMEGA_EXECUTION_BACKLOG.md`:
- Filtros de slippage/profit-threshold y net-profit gate en el motor de oportunidades (`backend/prioritization-spine`, `backend/searcher-rs`).
- Simulación REVM real previa a broadcast (gobernada por skills `arbx-simulation-mandatory`).
- Rate-limiting/authz en `backend/api-server` (parte ya existe; auditar).
- WebSocket vs polling en `frontend/` (ya hay `useWebSocket`/socket lifecycle; auditar realtime).
- Observabilidad Prometheus/Grafana, índices/particionado DB, cobertura de tests de integración/contratos.
- **Higiene de protección de rama**: 4 required checks obsoletos fueron retirados de `main` (`cargo audit`, `npm audit`, `gitleaks`, `playwright`) — si se quieren, hay que **crear los workflows reales** que los emitan (E real, accionable).

## Conclusión
No se crea ningún archivo-plantilla (`src/main.rs`, etc.): sería inventar estructura que el repo ya resuelve
de otra forma. El trabajo real está en **PRs abiertos + gaps de componentes**, no en "scaffold faltante".
