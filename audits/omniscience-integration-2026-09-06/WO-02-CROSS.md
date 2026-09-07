# WO-02 — CROSS-EXAMINATION (peer adversarial, post-apply+verify)

- **Work-order:** WO-02 · **Tipo:** CROSS (par que intenta REFUTAR el entregable APPLY+VERIFY)
- **Charter del cross:** ¿cumple el charter? ¿la verificación es real o de humo? ¿regresiones fuera del claim? ¿RULE 00/R8?
- **Reglas respetadas:** 0 requests HTTP al dominio público (0/5), 2 SSH read-only a `arbx` (docker ps/inspect/logs grep, redis-cli XLEN, git log/merge-base/grep sobre el checkout — PROHIBIDO mutar, nada fue mutado), 0 git local, 0 escrituras.
- **Fecha:** 2026-09-07 (ventana local 00:3x).

---

## VEREDICTO: **GAPS** — la ingeniería local es REAL y reproducida; el OUTCOME del charter (N3#2 "streams hot XLEN=0 perpetuos") NO está cerrado en producción: el wiring deployado es inalcanzable bajo el modo orquestador vigente.

---

## 1. Lo que sobrevivió al intento de refutación (evidencia propia, no heredada)

| Aserción del entregable | Mi verificación independiente | Resultado |
|---|---|---|
| `cargo check -p searcher-rs` EXIT=0 | re-ejecutado (target caliente, árbol actual con residual WO-04) | **EXIT=0 confirmado** |
| `cargo clippy -p searcher-rs -- -D warnings` EXIT=0 | re-ejecutado | **EXIT=0 confirmado** |
| `cargo fmt -p searcher-rs -- --check` EXIT=0 | re-ejecutado | **EXIT=0 confirmado** |
| Test `hot_sim_record_maps_outcome_verbatim` PASS | re-ejecutado: `1 passed; 1144 filtered out` | **PASS confirmado** |
| Hunks §5.1/§5.2 aplicados según diseño | re-leídos completos: firma `emit_simulated(&opp,&result)` (`hot_path_emitter.rs:139-143`), XADD con los 10 campos en el orden del diseño (`:153-180`), 5-tupla (`scanner.rs:2413-2436`), emisión post-publish fail-soft (`:2648-2669`), captura única post-spawn_blocking (`:3061-3064`), 6 returns con 5º elemento correcto (`:2983-2989` None/None, `:3051-3057` None/None, `:3098-3104`, `:3157-3163`, `:3179-3185`, `:3212-3218`), helper puro (`:3225-3235`), test (`:3334-3363`) | **CONFORME al diseño** (desviación `searcher_rs::` vs `crate::` justificada: bin vs lib) |
| `emit_detected`/`emit_gate_commit_from_state` sin call-sites | grep propio backend `*.rs`: único call-site de `HotPathEmitter`/`emit_simulated` = `scanner.rs:2660-2661`; `orchestrator.rs:1220` es comentario | **Confirmado** |
| RULE 00 / R8 | cada campo XADD traza a `Opportunity` publicado o `SimulationOutcome` real (`hot_path_emitter.rs:159-178`, `scanner.rs:3229-3234`); pre-REVM→None jamás emitido como failed; `"0"` wei = zeroing real de `SimulationOutcome::failed` (probado por el test que re-ejecuté); consumer PaperExecutor usa `BigInt(...)` (`paper/executor.ts:105`) → la preservación U256-String llega BIEN al consumidor dormido (sin pérdida Number) | **Sin violación** |
| Marcadores `// WO-02 (2026-09-06)` | presentes en cada hunk propio (`hot_path_emitter.rs:18,29,119` · `scanner.rs:2411,2651,2916,2940,3061,3221,3334`) | **Cumple** |
| Regresión fuera del claim | `git diff` residual de `scanner.rs` = 7 líneas **WO-04** (liquidation gas-cost knob, `scanner.rs:443-453`), no WO-02; `git diff HEAD --stat` no muestra otros archivos del WO | **Sin regresión propia** |

Los gates del APPLY/VERIFY **no eran de humo**: los 4 reproducen verde hoy. El APPLY además documentó fail-honest sus 2 gates parciales (clippy/fmt por huérfano WO-10) que el VERIFY posterior ya encontró cerrados — coherente con la línea de tiempo (WO-10 commit `734496ed` 22:29 local).

## 2. La refutación que SÍ prende — el outcome en producción

**Hechos (SSH read-only, 2 llamadas, VPS `/opt/arbitragex-v2`):**

1. El wiring WO-02 **está deployado**: VPS `HEAD=a60de001` con `git merge-base --is-ancestor 734496ed HEAD` → **YES** (merge PR #547 `a0bcf29d`); grep del checkout VPS: `emit_simulated` presente en `backend/searcher-rs/src/scanner.rs:2655`.
2. `XLEN arbx:hot:simulated` = **0** · `XLEN arbx:hot:detected` = 0 · `XLEN arbx:opps:detected` = **10001** (publicación canónica VIVA).
3. Boot log del searcher (2026-09-07T05:14:30Z): `"event":"scanner.orchestrator_mode" … "mode":"v2"` — **producción corre modo v2**.
4. Código local: `scanner.rs:1589-1592` — `if orch_mode == OrchestratorMode::V2 { return Ok(()); }` ("In V2 mode the orchestrator is the sole emit path — skip legacy"). El wiring WO-02 vive en el cuerpo legacy (`:2648-2669`), **después** de ese return.

**Consecuencia:** con el modo vigente, TODO el flujo de detección va por la pierna V2 (`opportunity_emitter.rs`), que jamás pasa por el `dispatch_orchestrator_and_classify` donde se produce `hot_sim`. El wiring está vivo-pero-inalcanzable: **`arbx:hot:simulated` seguirá XLEN=0 perpetuo en producción** — exactamente el síntoma N3#2 que el WO-02 decía cerrar. El diseño lo predijo (riesgo §8.3: "Modo v2 en prod dejaría el stream honestamente vacío — la Oleada 4 DEBE verificar `scanner.orchestrator_mode` antes de reclamar el invariante") pero **nadie lo verificó**: ni el APPLY (sin mandato VPS), ni el VERIFY (declaró INV-1..6 "post-deploy = operador"), ni existe reporte post-deploy en este directorio tras los deploys #547/#548. El board quedó con la impresión "N3#2 cerrado" cuando el síntoma de producción persiste por configuración de modo.

**Nota fail-honest a favor del entregable:** ni APPLY ni VERIFY CLAIMAN cierre en producción — ambos lo derivan al operador. El gap es del programa (verificación del invariante prometida por el diseño §6/§8.3, nunca ejecutada por nadie), no una mentira del reporte. Pero un verify adversarial que hubiera leído el boot-log del modo (SSH read-only permitido) habría degradado el veredicto a "no cierra N3#2 en prod".

## 3. Gaps (agente-corregible vs operador-gated)

| # | Gap | blocked_by |
|---|---|---|
| G1 | **Outcome N3#2 no cerrado en prod**: wiring inalcanzable bajo `ARBX_ORCHESTRATOR_MODE=v2`; INV-1..INV-6 (diseño §6) jamás ejecutados post-deploy; sin reporte post-deploy. Resolver = o cablear la pierna V2 (nuevo WO sobre `opportunity_emitter.rs`, donde corren las sims en v2) o flip de modo (blast radius enorme — NO recomendado como acción liviana) | **operator-gated** (flip de modo §34-adjacente + deploy; el diseño de la pierna V2 sí sería un WO agent-fixable) |
| G2 | El gang DEBE emitir el reporte post-deploy fail-honest que hoy falta: "XLEN arbx:hot:simulated=0 con wiring deployado + mode=v2 (log 05:14:30Z) + counter deltas" para que el board no lea "cerrado" | **agent-fixable** (este CROSS ya lo registra; falta volcarlo al board row de WO-02) |
| G3 | §5.3 `docs/redis-schema/hot-path-v2.md` sigue documentando campos que NUNCA existieron (`sim_result`, `trace_hash`) y sin el productor nuevo — drift doc activo aunque pre-existente | **agent-fixable** (diff listo en WO-02-DESIGN §5.3; docs-only) |
| G4 | §5.4 companion (`paper/executor.ts` skip_failed info→debug + `ExecutorLogger.debug`) pendiente por charter | **operator-gated** (decisión explícita reservada; hoy inerte porque el executor está dormant) |
| G5 | Test-nit: el caso passed del test nunca ejercita un `gas_price_wei` NO-cero stringificado (solo el caso failed zeroed aserta `"0"`) | **agent-fixable** (1 línea de test) |

## 4. Proceso (para el operador, no defecto de los agentes WO-02)

- El promote-batch committeó WO-02+WO-10 juntos (`734496ed`) y todo el gang-branch via PR #547 — desviación de §37 P-∅ "un PR = un ID" cometida por la sesión de promote, no por los appliers.
- Los anclados file:line de APPLY/VERIFY derivaron ~6 líneas tras el commit de WO-10 (publish `2642→2648`); citar con desplazamiento al re-auditar.

## 5. Presupuesto

0 requests HTTP al dominio público (0/5). 2 SSH read-only (ninguna mutación: `git rev-parse/merge-base/log/grep`, `docker ps/inspect/logs`, `redis-cli XLEN` ×3). 0 git local. 0 escrituras fuera de este reporte.

---

*WO-02 CROSS — 2026-09-07. Fail-honest: los 4 gates re-ejecutados por este cross están verde y los hunks son conformes al diseño; la refutación es de OUTCOME (producción en modo v2 deja el wiring inalcanzable y XLEN perpetuo en 0), documentada con evidencia VPS propia.*
