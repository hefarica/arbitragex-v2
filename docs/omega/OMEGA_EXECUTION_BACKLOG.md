# OMEGA v5 — Backlog Ejecutable (FASE 0/1, aterrizado al repo real)

> Reemplaza el backlog del `OMEGA_V5_INGESTION_REPORT.md` previo (que tomaba rutas-plantilla literales).
> Construido cruzando lo REAL del workbook (BRANCHES&PRs, DEV ENGINE) con el estado en vivo de GitHub
> (`gh pr list`, 2026-05-21) y la estructura real del repo. Estados de PR en vivo, no del workbook stale.

## Estado base (verdad)
- `main` HEAD = `d9bea59` (PR #103 mergeado hoy). CI de main: 9 checks reales gating (verde).
- **21 PRs abiertos.** Ninguno mergeable trivialmente: BLOCKED/UNKNOWN/BEHIND/DIRTY.
- Cada merge a `main` exige: CI verde + protección (1 review + 9 checks + conversation-resolution + enforce_admins=true) → requiere desbloqueo de gobernanza por humano, como en #103.

## P0 — Dependabot (deuda de dependencias; "P0" del workbook)
| ID | PR | Tipo | Estado en vivo | Bloqueo real | Acción |
|---|---|---|---|---|---|
| DB-1 | #104 | npm-minor-patch group (8) | BLOCKED | CI/checks o protección | revisar diff, correr `npm test`, desbloquear+merge |
| DB-2 | #102 | cargo `rand_distr` 0.4.3→0.6.0 | UNKNOWN | checks no corridos | gatillar CI; `cargo test -p <crate>` |
| DB-3 | #101 | cargo `nalgebra` 0.32.6→0.34.2 | UNKNOWN | idem | idem (riesgo breaking: API nalgebra) |
| DB-4 | #100 | cargo `alloy-sol-types` 1.5.7→1.6.0 | UNKNOWN | idem | idem |
| DB-5 | #99 | npm `express` + `@types/express` | BLOCKED | CI/checks | `npm test` api-server/edge |
| DB-6 | #98 | `@types/node` 20→24 | BLOCKED | typecheck (Node 24 types) | `tsc --noEmit` workspaces |
| DB-7 | #96 | `actions/upload-artifact` 4→7 | BEHIND | rama detrás de main | update branch → CI → merge (toca workflow → falta scope `workflow`) |
| DB-8 | #95 | `appleboy/ssh-action` 1.0.3→1.2.5 | **DIRTY** | conflicto de merge | resolver conflicto → CI (toca workflow → falta scope) |

> Nota: #96 y #95 tocan `.github/workflows/*` → el token actual **no tiene scope `workflow`** ⇒ no puedo push. Bloqueo de permiso documentado.

## P1/P2 — PRs OMEGA de feature (requieren CI verde individual)
| ID | PR | Tema | Estado | Acción |
|---|---|---|---|---|
| F-1 | #87 | sed-core feature gates (unblock CI inyecc. 22-27) — **P0 real** | UNKNOWN | desbloquea a #81–#86; revisar primero |
| F-2 | #88 | CI/CD + Governance + Runbooks (10 artefactos) | UNKNOWN | revisar; toca workflows (scope) |
| F-3 | #94 | e2e smoke contra prod URL | UNKNOWN | requiere URL prod viva |
| F-4 | #79 | Frontend hardening (17 fases) | UNKNOWN | revisar CI frontend |
| F-5..F-10 | #81–#86 | Inyecciones 22-27 (FHE, ePBS, GPU-EVM, honeypot, PQC, state-locker) | UNKNOWN | dependen de #87; alta complejidad/criptografía — revisión profunda |
| F-11 | #75 | fix VPS_SSH secret name | UNKNOWN | toca deploy workflow (scope) |
| F-12 | #56 | audit-vps-wiring.yml | UNKNOWN | workflow (scope) |
| F-13 | #54 | "Add files via upload" | UNKNOWN | revisar contenido (PR genérico) |

## P2/P3 — Gaps de componente (DEV ENGINE → rutas reales)
| ID | Componente | Gap (workbook) | Ruta real | Gobernado por |
|---|---|---|---|---|
| C-1 | Arbitrage/oportunidades | filtros slippage + profit-threshold | `backend/prioritization-spine`, `backend/searcher-rs` | `arbx-net-profit-gate` |
| C-2 | Bundle/flashloan | simulación REVM previa a broadcast | `backend/simulator-v2`, `backend/sim-ctl` | `arbx-simulation-mandatory` |
| C-3 | Contracts | auditoría seguridad; quitar debug en prod | `contracts/` | `arbx-contract-atomicity-rules` |
| C-4 | Backend API | rate-limit + authz; cero creds en código | `backend/api-server` | `arbx-no-hardcode-doctrine` |
| C-5 | Frontend | realtime WS vs polling | `frontend/` (ya hay `useWebSocket`) | — |
| C-6 | Observabilidad | Prometheus/Grafana + latencia | infra | — |
| C-7 | DB | índices/particionado | `database/migrations` | `arbx-rpc-failover` n/a |
| C-8 | CI/CD | tests integración + coverage >80%; tests de contratos | workflows + `contracts/test` | falta scope `workflow` |

## Restricciones que condicionan la ejecución (honestas)
1. **Merge a main** = gobernanza por humano cada vez (review + checks + conversation-resolution + enforce_admins). Como en #103.
2. **Token sin scope `workflow`** ⇒ no puedo pushear cambios en `.github/workflows/*` (afecta #95, #96, #75, #56, #88, C-8).
3. **RULE 01: sin Docker local (Windows)** ⇒ tests testcontainers/Docker (backend integración, E2E) sólo validables en CI/VPS.
4. **No capital/mainnet/live** ⇒ C-1/C-2/C-3 paran en paper/fork/REVM; nada de ejecución real.
5. El workbook estima **"semanas"**: cerrar 0.72→1.0 (21 PRs + 8 gaps) es un programa incremental, no un turno.

## Orden de ataque recomendado (incremental, evidencia-first)
1. **#87** (sed-core feature gates) — desbloquea inyecciones; alto leverage.
2. **Dependabot cargo** (#100, #102; #101 con cuidado por breaking de nalgebra) — sin tocar workflows.
3. **Dependabot npm** (#98, #99, #104) — `tsc`/`npm test` verdes.
4. **#96/#95** (workflows) — **bloqueado por scope de token**: dejar diagnóstico + diff listo para humano.
5. Gaps de componente C-1..C-8 — uno por PR, con su gate de skill y validación fork/paper.

Cada item sigue el loop: rama → fix → `cargo/npm/forge` verde local (lo posible sin Docker) → commit → push → CI → desbloqueo gobernanza → merge → verificar main.
