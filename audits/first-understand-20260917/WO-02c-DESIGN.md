# WO-02c — Reporte de diseño (design, read-only) · 2026-09-17

> ecc:rust-reviewer · Gang Omniscience oleada 1. Deliverable principal:
> `audits/first-understand-20260917/02c-SIM-STACK-SELECTOR.md` (fichas completas con file:line).
> Este archivo es el cierre del WO: alcance, invariantes, gates y handoff a la mesa.

## Alcance ejecutado

- Fichas de las 4 piezas bajo claim: simulator-v2 (6 módulos), sim-core (4), sim-ctl (13), selector-api (9). Cada una con ruta | firma entrada→transformación→salida | misión/fase | op_origen (multi-dex vía catálogo `shared_rs::chains::routers_for_chain`) | grafo inverso grep | muta/puro | prueba file:line | destino.
- Verificación post-fix SEL-GATE-01 (misión explícita del charter): presente en código + test de regresión + orden del call-site candado por test que lee el fuente; NO desplegado en VPS (cite 02-VPS-REMAP-20260917.md).
- Flujo Opportunity validar→simular→seleccionar documentado con tipos canónicos; ficha de shared-rs NO duplicada (deferida a WO-02d, citada).
- Restricciones honradas: 0 cargo/npm/build, 0 git, 0 VPS-mutación, 0 HTTP requests, 0 ANVIL/Foundry MCP.

## Hallazgos duros (evidencia clasificada)

1. **[CANONICAL_REPO]** Orden runtime: `detected → selector-api(selector-g0) → validated → sim-ctl(sim-ctl-g0) → simulated → relays-client` (consumer.ts:23-25; sim-ctl consumer.rs:48-50; relays consumer.rs:21).
2. **[CANONICAL_REPO]** HUEHUFO confirmados: `simulator-v2/src/bellman_ford.rs` (0 consumidores externos) y `selector-api/src/scoring/bayesian.ts` (solo su test lo importa).
3. **[CANONICAL_REPO]** Invariante de aislamiento del stack sim: `paper_mode=true && enable_storage_cheats=true` obligatorios en `MultiStepExecutionConfig` (sim_multistep.rs:70-74) y fijados por sim_runner.rs:204-205 — cumple §32/§33/§34.3 (sin signer, sin broadcast).
4. **[CANONICAL_REPO — scope ACOTADO por // WO-02c-FIX-G3 (2026-09-17)]** Idempotencia de redelivery: `insert_simulation` ON CONFLICT DO NOTHING + skip de XADD duplicado (persistence.rs:28-36,53-61; consumer.rs:443-461) — **válido SOLO para `simulator='revm'` (B2c)**: el índice árbitro es PARCIAL a propósito (`WHERE simulator = 'revm'`, migrations/113_simulations_revm_idempotency.sql:27-29, comment :12-14 "legacy anvil history"; diseño original multi-attempt por oportunidad en 004_simulations.sql:2). El path legacy anvil (`SimulatorKind::Anvil`, sim_engine.rs:163,205 — el default de compose según hallazgo 7/ficha 4.6) NO deduplica: para filas anvil el ON CONFLICT nunca arbitra → `inserted_fresh` siempre true → un PEL-redelivery de una sim anvil passed re-publicaría duplicado a `arbx:opps:simulated` (impacto HOY nulo: passed=0 histórico, 00-SYNTHESIS). Extender la dedup a anvil, si se desea, = WO propio con diff, operator-gated (ver G6/G7 del cross-exam).
5. **[CANONICAL_REPO + PRIMARY_SOURCE]** SEL-GATE-01: código local fdb40401 presente; VPS en a06a968d sin él (02-VPS-REMAP). Efecto runtime VPS: INFERRED — selector sigue re-decidiendo rows producer-rejected.
6. **[CANONICAL_REPO]** objetivo_usd = **GAP** en las 4 piezas (solo gates: min_accept_score, SIM_MIN_PROFIT_WEI default 0, min_acceptable_score). RULE 00: no se inventa blanco.
7. **[CANONICAL_REPO]** SIM_BACKEND no definido en compose → default `anvil` (compose.prod.yml:144-158); el path B2c revm (real multi-step) requiere flip operador-only (.env.example:319). // WO-02c-FIX-G5 (2026-09-17, cross-exam G5): el sello CANONICAL_REPO acota SOLO al plano compose. El valor RUNTIME del VPS es **UNKNOWN** — el bloque sim-ctl monta `env_file: ../.env` (compose.prod.yml:150-151) que puede definir SIM_BACKEND=revm y sobreescribir el default (main.rs:661-664; B2c lo exige :734); 02-VPS-REMAP no leyó el .env (solo stat, :12). Reclasificación: compose=CANONICAL_REPO, runtime=INFERRED/UNKNOWN.
8. **[CANONICAL_REPO]** Gate latente: `decide()` gates 2-3 (simulation_failed, revert_risk) corren con `sim:null` en el stream path (consumer.ts:213) — solo activos vía /score.

## Diseño propuesto (4 ítems, ver §9 de la ficha)

Sin diffs de producción (orden expresa: NO editar código de producción). Resumen:

| ID | diseño | invariante | gate |
|---|---|---|---|
| DEPLOY-DEBT-01 | deploy main local (operador) | SHA deploy == SHA despachado | `git rev-parse HEAD` post-deploy + G-PIPE-1 verde |
| WIRE-BAYES-01 | cablear BayesianToxicityEngine como factor/gate VPIN | posterior solo de reverts reales (RULE 00) | vitest integración consumer→decide; tsc --noEmit |
| DEPRECATE-BF-01 | marcar/consolidar bellman_ford.rs HUEHUFO | no-borrado (§3 surgical) | grep 0 consumidores re-verificado |
| SEL-GATE-01bis | // WO-02c-FIX-G1B (2026-09-17): re-bajado — "87%" NO reproducible (C2 §10.4 de 02c) → endurecer prefilter cruzando PG si persiste el FENÓMENO post-deploy: fracción mayoritaria NO cuantificada de sims quemadas en rows producer-rejected (soporte canónico 00-SYNTHESIS:24-25: 74 sims/9h S4, detección 100% rejected, v3_quote_unavailable 60/76) | clasificación conservadora inalterada | test con producer viejo sin status/reason |

Todos requieren WO propio con diff exacto + aprobación operador (NO-GIT vigente).

## Handoff a la mesa

- **WO-02d** (relays-client/shared-rs): tu terminus consume `arbx:opps:simulated` (consumer.rs:21) y el MISMO carrier `arbx:validated_plan:{id}` (submit_engine.rs:161,442-445) que sim-ctl re-simula (canonical_plan_consumer.rs:41) — la coherencia carrier producer/consumer/terminus es un invariante transversal que ya quedó documentado de mi lado. `sim_core::verified_simulation` es consumido por TU crate (execution_admission.rs:108) — ficha del módulo en mi 02c §3.4, cíta­la, no la dupliques.
- **WO-02a** (searcher-rs): el productor construye `Arc<SimulatorV2>` per-chain en main.rs:696-731 con el flag `ARBX_USE_SIMULATOR_V2` (main.rs:491-494 — "no runtime flip yet": construido pero sin dispatch hot-path completo); candidate_simulation.rs:453-469 conduce execute_multistep_revm. El carrier M2 carrier-B se escribe en scanner.
- **WO-02b** (operators/math-engine): sim stack es agnóstico de operador (multi-dex); tu matriz 264×31 vive aguas arriba del candidate.
- Board actualizado: WO-02c → DONE en GOAL-WORKORDERS.md.

## Errata append-only

- **// WO-02c-FIX-G1B (2026-09-17, fixer gang ronda 1)** — fila SEL-GATE-01bis (tabla §
  "Diseño propuesto", :34) decía "si persiste el 87% post-deploy". La cifra "87%" es NO
  reproducible (C2 del verify, §10.4 de 02c; grep del fixer: 0 hits de "87%" en
  `audits/real-cycles-audit-20260916/`; existe SOLO en sel-gate01.test.ts:5-9 y
  engine.ts:28-30 — código gated, y en las citas circulares ya corregidas). Re-bajada
  in situ a "fracción mayoritaria NO cuantificada" con soporte canónico
  00-SYNTHESIS.md:24-25 (74 sims/9h S4, detección 100% rejected, v3_quote_unavailable
  60/76). Cierra el residuo del GAP-2 del cross-exam (WO-02c-verify-CROSS-EXAM.md:44-52):
  el paralelo WO-02c-FIX-G1 ya había re-bajado 02c §5.3 (:171) y §9.4 (:235) y documentado
  que §6 (:197-207) NO contenía la cifra — este fix cubre la mitad DESIGN que quedó viva.
  El test header sel-gate01.test.ts:5-9 y el comentario engine.ts:28-30 siguen conteniendo
  "87%" = operator-gated, NO tocados. Reporte: `WO-02c-FIX-G1B-20260917.md`.
