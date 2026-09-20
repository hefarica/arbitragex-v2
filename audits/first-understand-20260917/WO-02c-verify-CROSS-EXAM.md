# WO-02c-verify — CROSS-EXAMINATION (par refutador) · 2026-09-17

> Objeto: `02c-SIM-STACK-SELECTOR.md` (diseñador ecc:rust-reviewer + adendum §10 del
> verificador cs-validator) y `WO-02c-DESIGN.md` / `WO-02c-verify-VERIFY.md`.
> Mandato: REFUTAR. Método propio: Read/Grep SOLAMENTE (restricción board :17-19),
> 0 cargo/npm/build, 0 git-mutación, 0 VPS, 0 HTTP (0/5 presupuesto dominio).
> Evidencia propia re-abierta independiente: ~25 marcadores file:line en
> selector-api, sim-ctl, sim-core, simulator-v2, compose, .env.example, audits/.

## 1. Intento de refutación — resultado por claim duro

| Claim del entregable | Mi evidencia independiente | Veredicto |
|---|---|---|
| SEL-GATE-01 presente local: `producerRejected` engine.ts:26-39, wire :56-58, publish accept-only, test 4 casos + candado de orden | Re-abierto: fn exacta (:26-39), wire en prefilter antes de `pairAllowed`, publish `if (decision.kind === "accept")` (consumer.ts), sel-gate01.test.ts:59-101 con `readFileSync(consumer.ts)` + índices `prefilter < checkToken < scoreOpportunity` (:93-98) | **CONFIRMADO** |
| C2: "87% de sims quemadas" NO reproducible en 00-SYNTHESIS.md | Grep propio: 00-SYNTHESIS.md no contiene 87% (contiene 76 opps/9h, 74 sims/9h, v3_quote_unavailable 60/76, 100% rejected). Grep en TODO audits/: "87%" solo en cerebro-2026-09-07 (77.87% stage S5 y 86.96% prior κ=20 — contextos distintos) y CPU idle 87%. El número vive SOLO en engine.ts:28-30 (comentario del código), sel-gate01.test.ts:5-9 y la ficha 02c §5.3/§6 (circular) | **CONFIRMADO — C2 del verificador es correcta y la suscribo** |
| bellman_ford HUEHUFO (0 consumidores externos) | Grep propio en simulator-v2/sim-core/sim-ctl/searcher: solo lib.rs:13,20,61 (`pub mod` + capabilities + doc). searcher usa SU spanning_tree_engine.rs | **CONFIRMADO** |
| bayesian.ts HUEHUFO (solo su test lo importa) | Grep -ri "bayesian" selector-api/src: únicamente bayesian.test.ts | **CONFIRMADO** |
| objetivo_usd = GAP (0 hits) | Grep propio `objetivo_usd|target_yield|objective_usd` en las 4 piezas: 0 hits | **CONFIRMADO** |
| SIM_BACKEND ausente en compose → default anvil; flip operador-only | Grep propio: 0 hits de SIM_BACKEND en compose.prod.yml/compose.dev.yml; sim-ctl service en compose.prod.yml:145; .env.example:319 "P1-3 (SIM_BACKEND=revm flip, OPERATOR-ONLY §34.3)"; main.rs:661-688 default "anvil" | **CONFIRMADO** |
| Invariante aislamiento: paper_mode=true + storage_cheats=true fijos en sim_runner | Re-abierto sim_runner.rs (bloque exec_config): `paper_mode: true, enable_storage_cheats: true` literales. §34.3 intacto | **CONFIRMADO** |
| C1 (matiz): paper_mode=false NO retorna error — delega a `verified_simulation::execute` | Re-abierto sim_multistep.rs:554-556: `if !config.paper_mode { return crate::verified_simulation::execute(ctx, simulator, config); }` ANTES de build; `validate()` (PaperModeRequired/StorageCheatsDisabled) vive en `build_multistep_plan`:355, solo alcanzable en path paper. La frase original del diseñador §3.2 era imprecisa; C1 correcta | **CONFIRMADO — C1 suscrita** |
| Doc-drift route_lookup.rs:15 ("None on PG error" vs impl `Err`) | Re-abierto: header módulo :15 miente; doc de `fetch_candidate_inputs` (:105-108) e impl (`fetch_optional(...).await?` propaga Err) dicen la verdad. Comportamiento = Err→PEL (correcto) | **CONFIRMADO** |
| Grafo inverso (mcp-sim-engine:main.rs consume SimulatorV2; relays execution_admission:108 consume verified_simulation; searcher candidate_simulation consume sim_encoder) | Grep propio: mcp-sim-engine/src/main.rs:703, relays-client/src/execution_admission.rs:90+108, searcher-rs candidate_simulation.rs:175-189 | **CONFIRMADO** |
| Cero mutación de archivos bajo claim | `git status --porcelain`: NINGÚN archivo bajo simulator-v2/sim-core/sim-ctl/selector-api modificado (solo pre-existentes: route_intent.rs = 02a/pre-gang, settings/CLAUDE/skill = fuera de claim) | **CONFIRMADO — 0 regresiones** |
| Sincronía de mesa | Cita 02-VPS-REMAP:4-5 (VPS=a06a968d), difiere shared-rs a WO-02d sin duplicar, handoff explícito a 02a/02b, C1 alinea con el drift doctrina↔código que 02d documentó | **CONFIRMADO — sin duplicación ni contradicción no nombrada** |

**La verificación es REAL, no de humo**: 25 marcadores re-abiertos por mí con 0
desviaciones. El muestreo de 18 fichas del verificador es representativo del árbol.

## 2. GAPS que sobreviven al cross-exam (ninguno invalida el dictamen PASS)

### GAP-1 (agent-fixable) — Aritmética de completitud del adendum §10.5 es INCORRECTA
- "`selector-api: COMPLETO (12/12)`": el árbol tiene **13** archivos src no-test
  (consumer, index, persistence, policy/blacklist, policy/engine, score,
  scoring/{bayesian,engine,factors}, token_safety/{cache,client,goplus,internal_heuristic}).
  La cobertura sí es 13/13 (fichas §5.1-5.9 los cubren todos) — el CONTEO está mal.
- "`sim-ctl: 15/16`": src tiene 15 .rs + build.rs = 16. Con route_lookup.rs Y build.rs
  ambos declarados sin ficha, lo cubierto es **14/16** (14/15 si solo se cuenta src).
  "15/16" es aritméticamente indefendible con sus propias palabras de la línea anterior.
- Bajo FAIL-HONEST del board (:70), los tallies de evidencia deben ser exactos.
  Fix: corregir §10.5 de 02c y §6 de WO-02c-verify-VERIFY.md (edición de reporte
  propio del gang — NO requiere gate: no es código de producción).

### GAP-2 (agent-fixable) — El entregable sigue AFIRMANDO el "87%" en su propio cuerpo
§5.3 (:171) y §6 citan "87% de sims quemadas... (00-SYNTHESIS)" mientras C2 (§10.4)
declara esa misma cifra no reproducible. El "corrección gated" confla dos ediciones:
(a) test header sel-gate01.test.ts:5-9 y comentario engine.ts:28-30 = código, gated;
(b) el TEXTO de la ficha 02c §5.3/§6 y de WO-02c-DESIGN = deliverable del gang,
agent-fixable sin gate. Un pares que lea §5.3 sin llegar a §10.4 propagará el número
(escenario real: la mesa lee por secciones). Fix: re-bajar a "mayoría no cuantificada
(canónicamente soportado por 00-SYNTHESIS: 74 sims/9h con detección 100% rejected)"
en §5.3/§6 con remisión a C2.

### GAP-3 (agent-fixable, matiz) — §10.2 sobrestima la falta de trazabilidad del pin
"la simulación corre contra latest SIN log ni observación que lo declare": el
doc-comment de `simulator_for_candidate` (consumer.rs:776-783) DOCUMENTA
explícitamente el caso None → simulador compartido sin pin ("the shared simulator is
reused untouched when no pin applies"). Lo que falta es observabilidad RUNTIME (log
/metrica), no documentación. El hallazgo sigue válido como degradación de calidad
silenciosa en runtime, pero la frase debe acotarse. Fix: una línea en §10.2.

### GAP-4 (operator-gated) — Todo lo sustantivo correctamente cercado, sin hallazgo de fuga
DEPLOY-DEBT-01 (deploy fdb40401+125b1e0b+dcfe890c con verificación SHA post-deploy),
WIRE-BAYES-01, DEPRECATE-BF-01, SEL-GATE-01bis, doc-fix route_lookup.rs:15 y la
re-cuantificación del embudo (query archivado o re-baja) = todos gated. El cross-exam
NO encontró ninguna acción prohibida ejecutada (0 git, 0 VPS-mutación, 0 build, 0
broadcast, default-deny intacto — §32/§33/§34.3 verificados vía C1 + sim_runner).

## 3. Nota para pares posteriores

- Si citan el incidente SEL-GATE-01: usen "fenómeno canónico, cifra sin artefacto"
  (C2 + mi §1). No propaguen "87%" hasta que exista el query del embudo versionado.
- Si fichan route_lookup.rs (deuda de GAP del diseñador): citen §10.2/§10.5 de 02c;
  el doc-drift :15 y la semántica Err/None ya están evidenciados dos veces (verificador
  + este cross-exam).
- lines-per-file.txt contiene DOS árboles (principal + worktree
  `.claude/worktrees/wf_257a24e1-859-2/` con line-numbers distintos, p.ej. main.rs
  :504 vs :659): cualquier conteo de completitud debe filtrar por `./backend/...`
  — probable fuente de los tallies erróneos de GAP-1.

## 4. Dictamen del cross-examiner

**PASS CON 3 CORRECCIONES MENORES (GAP-1/2/3, todas agent-fixable).** El charter se
cumplió, la verificación fue real (re-auditada independientemente: 25/25 marcadores
exactos), no hubo regresiones fuera del claim, RULE 00/R8/§32-34/NO-GIT respetados en
acción, y la sincronía de mesa se honró con citas nombradas. Los gaps son de estándar
de evidencia en los PROPIOS reportes del gang (tallies y una cifra no reproducible que
el propio adendum ya flags pero no corrige en el cuerpo) — no del código auditado ni
del veredicto SEL-GATE-01, que queda en pie: **local presente (fdb40401), VPS sin
deploy (a06a968d), efecto runtime = INFERRED**.
