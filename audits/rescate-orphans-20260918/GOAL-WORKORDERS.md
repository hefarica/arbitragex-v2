# BOARD — RESCATE-ORPHANS-0918

> /goal: Rescatar, completar y aterrizar el trabajo huérfano de la sesión caída
> ("Capacidades de LLMs y harness", muerte por cascada 503 CCR → 401), con verificación
> por capas. Origen: decisión del operador 2026-09-18 (despacho completo confirmado).
> Protocolo: no-git-until-final-gate — CERO commit/push/PR/deploy sin gate del operador.

## Contexto duro (verificado por el orquestador, no por aserción de agentes)
- Sesión muerta: `a282329c-3f45-4566-8fce-0277209f14c7` (transcript leído).
- `87676654` = merge #590 (S1 fee dual-unit) — YA en origin/main. ✅
- `8794ece6` (E0063 fix + leg_fees_bps f1) — **CORREGIDO 09-19: SÍ pusheado** en `origin/feat/leg-econ-f1-20260918` (HEAD=8794ece6); sin PR. `feat/legfix-e0063-20260918` es local-only (mismo commit).
  Dos branches espejo: `feat/legfix-e0063-20260918` y `feat/leg-econ-f1-20260918` (mismo commit),
  worktrees `arbx-legfix-20260918` y `arbx-legs-econ-f1-20260918` con `triangular_worker.rs`
  divergente SIN commit en ambos.
- Worktree `.claude/worktrees/reject-traces` (branch `feat/reject-traces-20260918`, 0 commits
  propios): 8+ archivos modificados sin commit. Addendum E1 (endpoint `pool_b_lag`,
  columnas probe_block/oracle_block) NO existe en ninguna parte.
- Worktree `.claude/worktrees/price-exchange` (branch `feat/price-exchange-20260918`, 0 commits
  propios): cambios sin commit + archivos nuevos `price-delta.ts`, `price-exchange-wire.test.ts`.
  Builder murió corriendo vitest — estado de tests DESCONOCIDO.
- `origin/feat/cex-ui-existing-20260918` (PR #589) — NO merged; el auto-merge "armado" nunca ejecutó.
- Hermes: store PARCIAL. `run_c3f28ef` (mesa fee) recuperado → respaldó S1 (ya en main).
  `run_09d4aa` (WETH live-first) PERDIDO del store.
- Run durable del ciclo abierto: `run_930a51c1243f4740b94c3a4b9a56d585`.

## Work Orders

### WO-1 — Reconciliar espejos E0063/leg_fees y preparar push+PR — 🟢 DONE (gate operador: push+PR)
- **Owner**: builder Rust + cross-examiner.
- **Claims**: `backend/searcher-rs/src/workers/triangular_worker.rs` (SERIE — dos worktrees lo
  tocan), branches `feat/legfix-e0063-20260918` / `feat/leg-econ-f1-20260918`.
- **Gate**: diff de ambos worktrees comparado línea a línea; un solo árbol final;
  `cargo check` en árbol caliente (AppControl §36.4); tests existentes pasan.
- **Entrega**: árbol reconciliado + diff final documentado + paquete PR listo (SIN abrir PR).
- **Resultado**: informe `01-wo1-espejos-e0063.md`. Ambos deltas = MISMO cambio (`leg_fees_bps: None`,
  2 líneas); gana legfix (orden canónico, rustfmt-ok, sin CRLF). Árbol final = `arbx-legfix-20260918`;
  econ-f1 congelado (descartable). `cargo check -p searcher-rs` ✅ verde (target caliente, recheck forzado);
  target frío = 4551. `cargo test` NO concluyente (E0463 artefacto target compartido) → gate en CI.

### WO-2 — Evaluar y completar reject-traces E1 — 🔴 ACTIVE
- **Owner**: verificador de trabajo parcial + builder.
- **Claims**: `.claude/worktrees/reject-traces/**` (worktree propio, sin conflicto).
- **Gate**: dictamen explícito por archivo: ¿el parcial es rescatable o basura? (RULE 00/R8);
  si rescatable → completar charter E1 + addendum (`probe_block`/`oracle_block`,
  endpoint `GET /api/forensic/pool_b_lag`, fail-honest `sin_datos`).
- **Entrega**: informe por archivo + (si aplica) trabajo completado en el worktree. SIN commit.

### WO-3 — Verificar y completar price-exchange E1+E2 — ✅ DONE (verificado, sin fixes; espera gate operador para PR)
- **Owner**: builder TS + verificador.
- **Claims**: `.claude/worktrees/price-exchange/**` (worktree propio).
- **Gate**: vitest del worktree corrido hasta el veredicto (el builder murió antes);
  tests nuevos (`price-exchange-wire.test.ts`) pasan o se documentan fallos exactos.
- **Entrega**: estado real de tests + fixes mínimos si aplican. SIN commit.
- **Resultado**: `03-wo3-price-exchange.md`. vitest api-server 69 files / 875 tests PASS (los 3 nuevos: 24/24); FE hook 11/11; tsc limpio; cargo check shared-rs+token-enricher OK (2m08s); cargo test sidecar 4/4. Inventario real = 14 archivos (BOARD decía 3). 0 líneas tocadas. Gap declarado: `price_worker` sin sidecar ⇒ source `unknown`.

### WO-4 — #589 cex-ui: estado del auto-merge fallido — 🟢 RESUELTO (esperando gate operador)
- **Owner**: orquestador (verificación directa). HECHO 2026-09-18.
- **Verificado vía GitHub API**: state=open, `mergeable: True`, `mergeable_state: clean`,
  head `576ccccb`, los 20 checks TODOS en verde (ci-gate, CodeQL, rust-check, frontend-build,
  TS integration, gitleaks, etc.). El auto-merge armado nunca ejecutó porque la sesión murió.
  **Acción pendiente = merge (decisión operador, gated).**

### WO-5 — Veredicto WETH live-first — 🟢 DONE (veredicto archivado)
- **Owner**: orquestador + Hermes. HECHO el re-disparo 2026-09-18.
- Charter original reconstruido del transcript (líneas 3548/3564): live-first con config como
  fallback de emergencia + guard de frescura; pregunta del cap de capital (¿vivo siempre o
  conservador max(live/config)?); divergencia +6,5% ($2.516,09 Chainlink vs $2.350,79 config);
  métrica `base_price_drift_pct` + fuente visible.
- Runs perdidos por wipes: run_09d4aa, run_e9f82d16. **Run final**: `run_5b408757…` → `hermes-run_weth.json`.
- **VEREDICTO SANCHO** (verificado contra clamp_to_cap_wei triangular_worker.rs:414-451 + migración 094):
  (1) live-first + config como fallback documentado: APROBADO; guard = staleness 300s + banda ±10% + fail-honest None→CapClampFailed.
  (2) **CAP DE CAPITAL = max(live, config)** (menos wei; el cap es cota de riesgo, no valorización). Live sí para gas-USD/P&L.
  (3) `base_price_drift_pct` + `base_price_source` {chainlink,config_fallback} + `base_price_age_ms` por candidato sized; alerta >2%/5min, fallback >10%.
  Corrección numérica: divergencia real **+7,03%** (2516.09/2350.79−1); el 6,5% era 1−cfg/live. Criterios A1-A6 en el JSON archivado.
  → WO derivado: WO-CAP-MAXPRICE-01 (implementación por builder + validador matemático independiente §12.1).

### WO-6 — Cierre §12.4: veredictos Hermes — 🟢 DONE (ambos archivados)
- **Owner**: orquestador. Al cierre del ciclo: `run_930a51c1...` (riesgos del rescate) y
  `run_e9f82d16...` (WETH live-first).

## Escaladas al operador (gated)
- Push de `8794ece6` reconciliado + apertura de PRs (WO-1/2/3) → decisión operador.
- Merge de #589 → decisión operador (o confirmar auto-merge con update-branch).

## Log
- 2026-09-18: BOARD creado. Inventario verificado por orquestador (git + worktrees + Hermes API).
  Veredicto fee-mesa recuperado y archivado (`hermes-run_c3f28ef.json`).
- 2026-09-18: Apertura §12.4 — run durable del ciclo `run_930a51c1243f4740b94c3a4b9a56d585`.
  Oleada 1 despachada: 3 builders en background (WO-1 espejos E0063, WO-2 reject-traces,
  WO-3 price-exchange). WO-4 verificado directo por orquestador (PR #589 clean + 20 checks
  verdes). WO-5 re-disparado (`run_e9f82d16...`).
- 2026-09-18 ~22:40: INCIDENTE ACP/gateway resuelto. Causa: restart de VS Code relanzó hermes-acp → gateway
  ccr-glm53 sin heredar `HERMES_API_SERVER_KEY` (key efímera → 401 a la key canónica) + wipe de TODOS los
  runs en memoria (run_09d4aa, 930a51c1, e9f82d16, 054e4f91 perdidos). Fix: `hermes -p ccr-glm53 gateway
  restart` con `API_SERVER_KEY` del registro User → paridad verificada (200 en /v1/models). Los 3 builders
  murieron 2× (restart + 401 CCR) y fueron REANUDADOS con contexto intacto (no re-spawn). Runs Hermes
  re-disparados: WETH `run_5b408757…`, ciclo `run_3963e652…` — monitor con archivo inmediato.
  Lección grabada en memoria: hermes-gateway-restart-wipes-runs.
- 2026-09-18 WO-3: verificador retomó tras 2 cortes (503 CCR / 401 gateway). Worktree sin node_modules raíz → junction al árbol principal (artefacto local, no git). Veredicto que el builder muerto no dio: TODO VERDE (875/875 + 24/24 + 11/11 + Rust 4/4). Sin commit. Reporte `03-wo3-price-exchange.md`.
- 2026-09-18 WO-1 (builder Rust): espejos comparados línea a línea — mismo cambio en dos formas, sin
  divergencia real (no aplica worst-wins). Reconciliado en legfix sin edición extra (delta ya era correcto;
  re-verificado en disco tras 2 interrupciones 503/401). check verde; tests → CI (AppControl 4551 + E0463
  en target compartido). Paquete PR en §6 del informe. CERO commit/push/PR. Escalada: push
  `feat/legfix-e0063-20260918` + PR → operador.

## Adjudicación del veredicto Sancho-ciclo (`hermes-run_cycle.json`) — verificado por orquestador 09-19
| Claim de Sancho | Veredicto | Evidencia |
|---|---|---|
| PR #589 "BEHIND 4 / checks verde-stale" | **CONFIRMADO** | `git rev-list --left-right`: 3 ahead / 4 behind; main avanzó a 62a9d379 (merge #588). GitHub dice `clean` pero required checks corrieron pre-S1 → **update-branch + re-CI antes del merge** (lección merge-cascade 405). |
| econ-f1 ya en origin, legfix local-only | **CONFIRMADO** (mi BOARD estaba mal) | `merge-base --is-ancestor 8794ece6 origin/feat/leg-econ-f1` = SÍ. |
| Espejos = "stub, NO f1 real" | **REFUTADO** | 8794ece6 = 312 líneas: `plan.legs.iter().map(\|leg\| leg.fee_bps).collect()` en candidates.rs + persistence 46 líneas + tests FE deriveLegs. El `None` de +2 líneas es SOLO el 6.º constructor (triangular, sin fee en esa capa) — fail-honest documentado. Dictamen WO-1 se sostiene. |
| Colisión bps/pips con S1 (familia E0004) | **REFUTADO** | 0 archivos en común (S1 = solo quote_anchor_runtime.rs). 8794ece6 persiste RAW dual-unit *verbatim* citando el MATH-BATCH; FE types.ts documenta "/1e4 vs /1e6, renderers must label the unit". Consistente con LEARNINGS "fee_bps dual-unit por diseño". **Check para cross-exam**: que el renderer FE etiquete unidad por tipo de pool. |
| price-exchange "vitest JAMÁS corrido" | **STALE** | WO-3 lo corrió tras el run: 875/875 + 24/24 nuevos + 4/4 Rust. |
| Snapshot-commit WIP inmediato + push | **VÁLIDO, GATED** | Riesgo real (36+13 archivos solo en disco). Commits WIP en ramas feature (no main) = bajo riesgo, pero viola no-git-until-final-gate → **escalado al operador como opción 0 recomendada**. |
| Orden: 589 → reject-traces rebase-primero → price-exchange → espejos | **ADOPTADO** con ajuste: econ-f1 canónica (ya en origin), legfix descartable. |
