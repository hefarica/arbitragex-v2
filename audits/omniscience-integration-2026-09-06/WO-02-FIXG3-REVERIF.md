# WO-02 — RE-VERIFICACIÓN ADVERSARIAL del FIX G3 (Gang Omniscience, ronda 1)

- **Work-order:** WO-02 · **Objeto:** fix G3 reportado en `WO-02-FIXG3.md` (diff §5.3 + bullet liveness)
- **Verificador:** re-verificador adversarial independiente — NINGÚN claim heredado; cada ancla re-leída en el árbol + VPS read-only
- **Fecha:** 2026-09-07 · **Presupuesto:** 0 requests HTTP dominio público (0/5) · 1 SSH read-only · 0 git (NO-GIT respetado) · 0 código tocado

---

## 0. VEREDICTO: **G3 CLOSED — CONFIRMADO · SIN REGRESIONES**

El fix es real, quirúrgico y fiel al código. `grep sim_result|trace_hash` sobre
`docs/redis-schema/hot-path-v2.md` = **0** (re-ejecutado, exit 1). Diff aislado
**+22/−6 en 1 archivo** (docs-only), aplicado **verbatim** al §5.3 del diseño
(re-consultado: el hunk aplicado coincide línea a línea con el diff espec de
`WO-02-DESIGN.md §5.3`, más el marcador HTML línea 29 y el bullet aditivo
liveness líneas 47-52, hunks independientes y revertibles). Los otros 26 archivos
del working tree sin commitear pertenecen a WO-04/05/10 (rondas previas) — el
fixer G3 no tocó código (verificado con `git diff --stat` completo).

## 1. Matriz de re-verificación — doc vs código vs VPS (RULE 00)

| Claim del doc (hot-path-v2.md) | Evidencia re-leída por ESTE verificador |
|---|---|
| Producer `decode_and_score_tx` post-publish | `scanner.rs:1488` (fn) · `scanner.rs:2648` `publisher::publish` → `publisher.rs:41` `STREAM_KEY="arbx:opps:detected"` · emit en `scanner.rs:2659-2669` DESPUÉS del publish |
| Fail-soft | `scanner.rs:2661-2668` — `warn!` "fail-soft: ... canonical publish already succeeded", pipeline continúa |
| "sólo si la sim corrió, nunca pre-REVM" | `scanner.rs:2411-2436` (5º elemento `hot_sim`, `None` en rama `SIM_DISABLED_FAIL_CLOSED`) · `scanner.rs:3048-3058` (spawn_blocking err ⇒ None) · `scanner.rs:3061-3064` ("capture ONCE, verbatim; pre-REVM returns stay None") |
| Verdict verbatim passed\|failed | `scanner.rs:3226-3235` `hot_sim_record` (`passed: outcome.passed` sin reclasificar) · `hot_path_emitter.rs:149` |
| 10 campos del XADD | `hot_path_emitter.rs:153-178`: `id, status, net_profit_wei, gas_used, gas_price_wei, opportunity_id, chain_id, strategy_kind, token_pair(←pair_symbol :175-176), timestamp_ms` — doc lista los MISMOS 10 |
| `net_profit_wei` decimal string, GROSS token_in | `scanner.rs:3231` `simulated_profit_token_in.to_string()` · comment `hot_path_emitter.rs:29-35` |
| `gas_price_wei` decimal string / `gas_used` | `scanner.rs:3232-3233` · `hot_path_emitter.rs:165-168` |
| MAXLEN ~5000 | `hot_path_emitter.rs:153-157` (`MAXLEN ~ 5000`) |
| Consumer ws-emitter-g0 → room `opportunities`, event `opportunity:validated` | `websocket.ts:801,803,849,893,906-907,976` · `io.to('opportunities').emit` `websocket.ts:952` |
| Consumer paper-executor-g0 dormante salvo `ARBX_PAPER_EXECUTOR_MODE=on` | `paper/executor.ts:20,22` (STREAM_IN/GROUP) · `index.ts:1904-1915` (gate off⇒null + log "paper executor dormant") |
| Liveness: early-return v2 antes del sim gate | `scanner.rs:1589-1592` leído verbatim: `if orch_mode == OrchestratorMode::V2 { return Ok(()); }` — el wiring vive en el cuerpo legacy, DESPUÉS de ese return |
| Liveness prod: XLEN=0 con wiring deployado, mode=v2 | **VPS read-only (fresh, este verificador, 2026-09-07)**: `XLEN arbx:hot:simulated`=0 · `XLEN arbx:hot:detected`=0 · env `ARBX_ORCHESTRATOR_MODE=v2` + boot log `scanner.orchestrator_mode "mode":"v2"` a las **06:18:40Z** (tercer boot — 05:14:30Z CROSS y 05:48:09Z POSTDEPLOY fueron los dos primeros) · `XINFO GROUPS arbx:hot:simulated` = grupo `ws-emitter-g0` (3 consumers, pending 0, **last-delivered-id 0-0** = jamás entregó) y **SIN grupo paper-executor-g0** — consistente exacto con "dormante" (el grupo se crea sólo al arrancar PaperExecutor, `executor.ts:166`) |

## 2. Sin regresiones

- Docs-only: ningún archivo de código, gate, §34.3, ni config tocado por este fix.
- Secciones vecinas del mismo doc (`arbx:hot:detected`, `arbx:hot:paper_executed`,
  keys TTL, latencia) quedaron byte-idénticas (confirmado en el hunk diff: sólo cambió
  la sección `arbx:hot:simulated`).
- Estructura markdown válida (comentarios HTML invisibles al render).
- NO-GIT respetado: el fix vive como edición local sin commit/push/PR.
- Evento de concurrencia del fixer (otro agente aplicó §5.3 a las 00:59:51) —
  forense del §3 del reporte es plausible y el resultado en disco es un único
  estado coherente con marcadores separados; no hay líneas pisadas.

## 3. Gaps RESTANTES (los que persisten tras este fix)

| Gap | Estado | blocked_by |
|---|---|---|
| **G1 — N3#2 abierto en prod**: wiring `emit_simulated` inalcanzable bajo `ARBX_ORCHESTRATOR_MODE=v2` (early-return `scanner.rs:1589-1592`); re-medido HOY: XLEN=0 en tercer boot 06:18:40Z. Remedio = WO cableando la pierna V2 viva (`opportunity_emitter.rs`) o flip de modo | PERSISTE (agente puede diseñar/aplicar el WO de la pierna V2; flip y deploy = operador) | agent-fixable (diseño/apply V2-leg) · operator-gated (flip/deploy) |
| **G4 — §5.4 companion** (`executor.ts` skip_failed info→debug + ExecutorLogger.debug) | PERSISTE, inerte (executor dormante) | operator-gated |
| **G5 — test-nit 1 línea**: caso passed del test de `hot_sim_record` nunca ejercita `gas_price_wei` no-cero stringificado | PERSISTE (próximo pase Rust) | agent-fixable |
| **Drift residual MISMO archivo (declarado §4 del fixer, verificado CIERTO por este re-verificador, fuera de mandato G3)**: (a) `hot:detected` lista `paper-executor-g0` como consumer (`hot-path-v2.md:14,24`) — falso: consume `simulated` (`executor.ts:20`); único consumer real de detected = ws-emitter-g0 (`websocket.ts:906`); (b) `emit_detected` **0 call-sites** (grep backend/ = 0) ⇒ el stream hot:detected mismo carece de productor cableado; (c) campos `token_path[]`/`amounts[]` (`:19-20`) no existen en el XADD real (`hot_path_emitter.rs:79-92`: id, chain_id, strategy_kind, detected_at_ms); (d) `paper_executed` MAXLEN doc ~1000 (`:62`) vs código ~5000 (`executor.ts:318`) + producer "paper archiver" vs PaperExecutor (`executor.ts:21`) + drift de campos (code: executed_at_ms/execution_time_ms/rejection_option) | PERSISTE — micro-FIX docs-only de 1 pase | agent-fixable |
| **Ghost-fields en doc histórico ajeno**: `docs/superpowers/plans/2025-07-10-omega-pipeline-sub100ms.md:49` aún lista el contrato fantasma de 4 campos (sim_result/trace_hash). Es un plan histórico (no schema vivo); decisión del operador si se anota o se deja como registro | PERSISTE (menor, informativo) | operator-gated (anotación) o agent-fixable con mandato |

Notas de descarte (NO gaps): `docs/m5-sepolia-runbook.md:167` (`trace_hash != 0`) y
`docs/superpowers/specs/2026-04-21-sprint3-selector-design.md` (`sim_result`) se
refieren a OTROS contratos (evidence `simulation_trace_hash` `evidence.rs:162`,
input del selector) — no al stream `arbx:hot:simulated`; fuera de alcance G3.
G2 (reporte post-deploy fail-honest) queda CUBIERTO: WO-02-POSTDEPLOY.md + fila del
board actualizada con la causa estructural + bullet de liveness en el doc.

## 4. Nit informativo (no gap)

Doc `:44-46` "Consumers: ... paper-executor-g0 (dormant unless ...)" — en prod el
grupo ni siquiera existe hasta activar el modo (XINFO GROUPS hoy: sólo ws-emitter-g0).
El paréntesis "dormant" ya lo declara honestamente; precisión opcional para el micro-FIX §3.

---

*WO-02-FIXG3-REVERIF — 2026-09-07. Fail-honest: toda la evidencia de esta matriz fue
re-leída directamente (árbol local + 1 SSH read-only al VPS); nada heredado del fixer.
0 requests dominio público. VPS no mutado.*
