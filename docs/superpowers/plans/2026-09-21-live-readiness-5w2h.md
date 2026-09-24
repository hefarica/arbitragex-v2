# LIVE-READINESS 100% — Plan 5W2H de completitud end-to-end

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cerrar TODAS las actividades pendientes que declara `https://arbx.ape-tv.net/live-readiness` con evidencia real end-to-end — sin forzar avisos — hasta que el veredicto `NO_GO` solo dependa del sign-off formal A.9 del operador.

**Architecture:** La página consume 5 endpoints (`/api/readiness/decision`, `/api/readiness/blockers`, `/api/go-no-go/status`, `/api/scoring/status`, `/api/risk/circuit-breakers/status`) que se alimentan de verificaciones server-side reales (readiness_evidence PG registry, Prometheus, sim-ctl /capabilities). Cerrar un aviso = producir la evidencia que el verificador exige, jamás editar el verificador. La dependencia estructural más profunda: casi todos los avisos de datos (paper runs, calibración, variance benchmark) requieren un **pipeline de detección→simulación→paper ledger VIVO**, hoy con 0 viables/sims gated — esa es la raíz y va primero.

**Tech Stack:** searcher-rs/sim-ctl/relays-client (Rust), api-server (TS/Express), readiness_evidence PG (migración `database/migrations/104_readiness_evidence.sql`), GitHub Actions (workflows `sim-*-evidence*.yml`), VPS Hetzner (`ssh arbx`, `/opt/arbitragex-v2`), Hermes local (gang, ACP).

## Global Constraints

- §32/§33 permanentes: audit/scaffold/shadow/read-only. SIN executor activo, SIN wallets/keys nuevas, SIN broadcast on-chain, SIN `live: true`. Capital expuesto = $0 en todo el plan.
- Aviso ≠ cosmética: cada ítem se cierra con el artefacto reproducible que su verificador lee (evidencia ≤30d en `readiness_evidence`, runs PG, contadores Prometheus). PROHIBIDO tocar verdes existentes (regresión).
- Solo fuentes gratuitas (RPC públicos/CI ya configurado; jamás proponer servicios de pago).
- Fuses: A.9 quorum-2, `ARBX_SIMULATOR_V2_READY`, umbrales `ARBX_CB_*`/`ARBX_RISK_*`, creación de signer = **[OPERADOR]** — el plan los prepara/documenta, nunca los ejecuta.
- No-git-until-final-gate: commits/PRs/deploy solo en gates explícitos aprobados.
- Evidence registry: `POST/GET /admin/readiness-evidence` (header `x-arbx-admin-token`); rows >30d = stale STRICT.

---

## 5W2H

| W/H | Respuesta |
|---|---|
| **What** | 4 blockers LIVE (G-SIM-1 critical, G-PAP-1, G-DISK-1, A.9 pending) + 5 breakers no-PASS (executor WARN, sim_error PAUSED, drawdown/revert_rate/gas_burn NOT_AVAILABLE) + scoring sin calibrar (`calibrated_pairs:0`) + secuencia de ignición 0/4 + socket kill-switch DISCONNECTED en la página. Todo end-to-end, sin maquillaje. |
| **Why** | Orden del operador 2026-09-21: la página ES la guía de completitud de la app; objetivo supremo = ganancias reales con 100% valores reales. El veredicto hoy es `NO_GO` por trabajo real faltante, no por bugs de UI. |
| **Who** | OMEGA orquesta; gang vía Hermes local (ACP, charter BOARD por fase); VPS ops vía `ssh arbx`; **[OPERADOR]**: sign-off A.9 quorum-2, presencia/creación de signer (Paso 2), umbrales ARBX_CB_MAX_GAS_BURN_USD/ARBX_RISK_*, decisión de flip (nunca automática). |
| **When** | Fases 0→4 (abajo). **Critical path = el reloj de 7 días de G-PAP-1**: hay que garantizar continuidad paper DESDE YA (cada hora de stall alarga el gate). Fase 1 (pipeline vivo) desbloporta variance benchmark + breakers + calibración. |
| **Where** | Repo local (árbol principal + worktree `price-canonical-cex`), VPS `/opt/arbitragex-v2` (ssh arbx), GitHub Actions (dispatch manual), dominio `arbx.ape-tv.net` (verificación pasiva). |
| **How** | Por fases con gate reproducible cada una; cada aviso se cierra produciendo SU evidencia (CI workflows del registry, runs PG, probes on-chain read-only); verificación final = los 5 endpoints + panel renderizando sin regresión de los verdes. |
| **How much** | $0 capital, $0 servicios de pago. Tokens: gang con presupuesto respawn 8, oleadas ≤4. Disco VPS: objetivo <85% (hoy 87.4%, 18.8GB libres). |

---

## Inventario pendiente (verbatim de los endpoints, 2026-09-21T11:10Z)

| # | Ítem | Severidad | Estado | Fuente |
|---|---|---|---|---|
| 1 | G-SIM-1: flag prematuro, checklist 2/7; faltan/stale >30d: `modules_merged`, `fork_suite`, `variance_benchmark`, `eth_callbundle_staging`, `second_signoff` | critical | blocked | `/api/readiness/blockers` |
| 2 | G-PAP-1: paper ≥7 días continuos + reporte ("not enough days"; detecciones presentes, última 0.0h) | high | blocked | ídem |
| 3 | G-DISK-1: disco VPS 87.4% (warn 85, crit 95), 18.8GB libres | high | blocked | ídem |
| 4 | A.9: sign-off formal pendiente; ledger `5189814f…`, `awaiting_first`, quorum-2 sin firmas | critical | pending | `/api/go-no-go/status` |
| 5 | executor_breaker WARN: falta probe on-chain `getCode(EXECUTOR_1)`/`owner()`/`paused()` ("not yet implemented") | — | WARN | `/api/risk/circuit-breakers/status` |
| 6 | sim_error_breaker PAUSED (derivado de G-SIM-1) | — | PAUSED | ídem |
| 7 | drawdown_breaker NOT_AVAILABLE: 0 paper runs (<100 req., <24h req.) | — | N/A | ídem |
| 8 | revert_rate_breaker NOT_AVAILABLE: 0 runs en ventana 24h | — | N/A | ídem |
| 9 | gas_burn_breaker NOT_AVAILABLE: `ARBX_CB_MAX_GAS_BURN_USD` y `ARBX_RISK_{NAV_USD,WINDOW_SECS,MIN_SAMPLES,DD_TIERS,GAS_CAP_USD}` sin setear + sin samples | — | N/A | ídem |
| 10 | Scoring: `calibrated_pairs:0`; blocker `a5_paper_shadow_not_executed` (correr `scripts/activate_paper_shadow.sh` ≥7d) | medium | open | `/api/scoring/status` |
| 11 | Ignición 0/4: Paso 1 vault RPC+WSS verificado server-side; Paso 2 signer presente (redacted); Paso 3 chains/DEX/pools con counts reales; Paso 4 ≥1 motor de resolución habilitado | — | pending | panel `/live-readiness` |
| 12 | Header: kill-switch socket DISCONNECTED; routes/runtime_ack/pairs/quote_anchor CONNECTING | — | caído | panel `/live-readiness` |

**Ya verde (PROTEGER, no tocar):** go_a4, go_a5, A.6/A.7 resueltos, A.8 wired, 5 breakers PASS, latency/rpc 9/11 alive, blacklist 1732 rows, scoring pipeline `WIRED_RUNTIME_SAMPLE` con 1000 scored recientes.

---

## Fases y tareas

### FASE 0 — Higiene y verificación (sin código de feature; despeja ruido y protege el reloj)

**Task 0.1 — Diagnóstico del socket kill-switch DISCONNECTED (ítem 12)**
- Files: `frontend/lib/hooks/*` (solo lectura), `backend/api-server/src/prices-stream.ts` y gateway WS (solo lectura).
- Steps: reproducir en browser (verificar transporte con `socket.io.engine.transport.name` — gotcha E2E memorizado); si es WS del dominio caído → R7 sobre api-server (`docker logs api-server`); si es solo estado inicial de la página → documentar fail-honest en el BOARD y pasar.
- Gate: explicación del DISCONNECTED con evidencia (log o screenshot), sin regresión del room `prices:*` (6/6 tests siguen verdes).

**Task 0.2 — Disco VPS <85% (ítem 3, G-DISK-1) [VPS-OPS]**
- Comandos (ssh arbx): `df -h /`; `docker system df`; prune del builder (`docker builder prune`) según patrón DISK-GUARD-01/prune 21GB del incidente 09-19; verificar PG NO se toca (jamás purge <2GB; TRUNCATE solo si procede y está autorizado).
- Gate: `df -h` <85% + `/api/readiness/blockers` sin `readiness_g_disk_1` + PG sirviendo (`SELECT MAX(detected_at) FROM opportunities`).

**Task 0.3 — Continuidad paper YA (arranca el reloj de G-PAP-1, ítems 2/7/8/10) [VPS-OPS]**
- Verificar `scripts/activate_paper_shadow.sh` corriendo en VPS (o activarlo); vigilar stall con el watchdog #624 (SCANNER-STALL-01 ya landed).
- Gate: paper ledger acumulando runs >0 por hora; `last 0.0h` se mantiene; sin crash-loops (lección FLIPPER #2).

### FASE 1 — Pipeline vivo: detección→sim→paper con datos reales (raíz de los avisos de datos)

> Continúa los BOARD existentes — NO se duplica trabajo: forensic (`audits/forensic-engine-2026-09-21/GOAL-WORKORDERS.md`, orden FE1→merge→FE4→FE5→FE7→FE8→FE9) y Stream A Binance WS (F3 cards join, F4 CexDex Phase 2, F5 deploy+R7+E2E). El gate de esta fase es medible: **≥1 oportunidad viable→simulada→paper-run por día** y el funnel `v3_quote_unavailable` (84%, RPC saturado) por debajo del bloqueo.

**Task 1.1 — WO-FE1** exporter `arbx.pricebus.export.v1` (worktree price-canonical-cex, tras FE2 ya DONE) + test con vector externo (§12.4) + validación Hermes.
**Task 1.2 — Merge rama PriceBus → main** (fast-forward posible; ANTES reconciliar dualismo binance_stream_worker vs binance_ws según PLAN-9FRONTERAS §0/§6.2).
**Task 1.3 — WO-FE4..FE9** según file-claims del PLAN-9FRONTERAS §3 (fuente única de precios, per-hop quote, evidence, packet, outbox, wire→card con `presentRealField`).
**Task 1.4 — Stream A F3+F4** (cards CEX join con R10; CexDex `StrategyKind` + quoter multicall + RouteMetadata §IV A1).
**Task 1.5 — F5 deploy por-servicio** (R3 `--env-file` + `--no-cache`) + R7 + E2E Playwright.
- Gate FASE 1: funnel 24h con viables>0 y `arbx_simulation_total` flow >0 (esto además vuelve GREEN la capa 3 de G-SIM-1 y alimenta 7/8/9/10).

### FASE 2 — G-SIM-1: re-producción de las 5 evidencias stale (ítem 1)

> Mecanismo exacto (`docs/operations/SIMULATOR_V2_READINESS.md` + `backend/api-server/src/readiness/verifiers/g-sim-1.ts`): registry `readiness_evidence` 7 item_keys, frescura STRICT 30d, `POST /admin/readiness-evidence` con `evidence_ref` + `verified_by`.

**Task 2.1 — `modules_merged`**: push a main tocando `backend/simulator-v2/**` dispara `sim-evidence-unit-tests.yml` (auto). Gate: `GET /admin/readiness-evidence?gate_id=G-SIM-1` muestra item fresh.
**Task 2.2 — `unit_tests`+`dep_tree` refresh** (mismo workflow; hoy 2/7 fresh — se re-produce con 2.1).
**Task 2.3 — `fork_suite`**: `workflow_dispatch` de `.github/workflows/sim-fork-evidence.yml` (suite ignorada `fork_mainnet.rs`; usa RPC configurado en secrets — gratuito ya provisto). Gate: `FORK_SUITE_OUTCOME=PASS` + row fresh.
**Task 2.4 — `eth_callbundle_staging`**: `workflow_dispatch` de `sim-staging-callbundle.yml` — round-trip REAL `eth_callBundle` a Flashbots simulate (ephemeral signer, zero-value, SIN broadcast — cumple §33 read-only). Gate: `totalGasUsed > 0` + row fresh.
**Task 2.5 — `variance_benchmark` [VPS-OPS]**: `bash scripts/gsim1_variance_benchmark.sh` EN el VPS — exige ≥100 pares de oportunidades REALES (export `gsim1_variance_export.sql`) → **depende de FASE 1**. Gate: mean drift <5%, método `revm_b_vs_revm_b1_fork`, row fresh.
**Task 2.6 — `second_signoff` [OPERADOR/INGENIERO 2]**: revisión de corrección numérica del profit-calc por un segundo ingeniero + POST admin del sign-off con `verified_by`. El plan prepara el paquete de review (diffs + tests); la firma es humana.
- Gate FASE 2: `/api/readiness/blockers` sin `readiness_g_sim_1`; `/api/risk/circuit-breakers/status` con `sim_error_breaker` fuera de PAUSED.

### FASE 3 — Breakers de datos + executor probe + scoring (ítems 5/7/8/9/10)

**Task 3.1 — executor_breaker probe (ítem 5)**
- Files: `backend/api-server/src/risk/` (nuevo probe read-only), lee `EXECUTOR_1` del env.
- Implementa: `eth_getCode(EXECUTOR_1)` + `eth_call owner()` + `eth_call paused()` vía RPC público (HTTP read-only, §33). `null` bytecode = NOT_AVAILABLE honesto; nunca PASS fabricado.
- TDD: test con fixture RPC (wiremock patrón existente) → implement → vitest + `tsc --noEmit`.
- Gate: breaker sale de WARN con estado real derivado del probe.

**Task 3.2 — Breakers de acumulación (ítems 7/8)**: SIN código nuevo por defecto — se llenan solos cuando FASE 0.3 + FASE 1 producen ≥100 runs/≥24h. Vigilar por panel; solo si el contador tiene un bug de wiring (0 runs pese a paper activo) → fix raíz con anomalía ID.
**Task 3.3 — gas_burn thresholds [OPERADOR]**: documentar propuesta de valores (`ARBX_CB_MAX_GAS_BURN_USD`, `ARBX_RISK_NAV_USD/WINDOW_SECS/MIN_SAMPLES/DD_TIERS/GAS_CAP_USD`) en el BOARD para decisión del operador; el seteo va al `.env` del VPS (nunca versionado).
**Task 3.4 — Scoring calibración (ítem 10)**: con paper runs acumulándose, `calibrated_pairs` sube solo; verificar priors archiver. Gate: `calibrated_pairs > 0` y blocker `a5_paper_shadow_not_executed` resuelto a los 7 días continuos.

### FASE 4 — Ignición 0/4 + A.9 runbook (ítems 4/11) — preparación, ejecución operador

**Task 4.1 — Trazar los 4 pasos de ignición a sus endpoints reales** (el panel los consume; identificar el endpoint por paso en el código de `/live-readiness`) y producir un diagnóstico por paso: qué verifica, qué falta, qué es accionable por código vs [OPERADOR] (Paso 2 signer = presencia redacted, NUNCA crear keys — §32).
**Task 4.2 — Accionables de código de ignición** (p.ej. Paso 3 counts reales ya existen en PG: verificar wiring; Paso 4 habilitar motor en paper/shadow si procede).
**Task 4.3 — A.9 [OPERADOR]**: regenerar ledger (`GET /api/v1/go-no-go/ledger`), runbook curl listo en el panel (ya existe); DOS operadores firman (`POST /admin/go-no-go/sign-off`). El plan llega hasta dejar todo lo demás verde y el ledger regenerado — la firma y cualquier flip son del operador (§34.3/§34.5).

---

## Orden de dependencias (resumen)

```
FASE 0 (paralela, hoy) ──┐
FASE 1 (pipeline vivo) ──┼──► FASE 2 (G-SIM-1 evidencias; 2.5 necesita ≥100 opps)
        │                │        │
        └── paper runs acumulan ──┴──► FASE 3 (breakers 7/8/9/10 se llenan; 3.1 paralelo ya)
                                          └──► FASE 4 (ignición + A.9 [OPERADOR])
```

## Criterio de éxito final (sin flips)

`/api/readiness/decision` → `go_live:false` por ÚNICA razón `A.9 GO/NO-GO formal sign-off pending`; `/api/readiness/blockers` → solo `a9_go_no_go_formal_pending`; breakers: 6 PASS + 3 con datos reales; panel `/live-readiness` renderiza todo sin secciones loading eternas ni regresión de verdes. **Ningún flip a live ocurre en este plan.**

## Self-review

- Cobertura: los 12 ítems del inventario tienen fase/tarea (1→F2, 2→F0.3/F3.4, 3→F0.2, 4→F4.3, 5→F3.1, 6→F2, 7/8→F3.2, 9→F3.3, 10→F3.4, 11→F4.1/4.2, 12→F0.1). ✓
- Placeholders: las tareas de FASE 1 remiten a BOARDs ya existentes con file-claims exactos (PLAN-9FRONTERAS §3) — no duplicados, referencia verificada en disco. Las tareas VPS/CI citan comandos y workflows exactos. ✓
- Consistencia: endpoints y item_keys copiados verbatim de las respuestas reales del 2026-09-21T11:10Z. ✓
