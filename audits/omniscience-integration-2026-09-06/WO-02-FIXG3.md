# WO-02 — FIX G3 (Gang Omniscience, ronda 1): drift doc `hot-path-v2.md`

- **Work-order:** WO-02 · **Tipo:** FIX (gap G3 adjudicado por WO-02-CROSS §3)
- **Gap:** `docs/redis-schema/hot-path-v2.md` documentaba campos que NUNCA existieron
  (`sim_result`, `trace_hash`) y omitía el productor/contrato que WO-02 puso en el
  stream `arbx:hot:simulated` — drift doc activo (pre-existente, agravado por el wiring).
- **Remedio aplicado:** diff de WO-02-DESIGN §5.3 (docs-only, cero gates de código) + 1 bullet
  aditivo fail-honest de liveness (ver §2).
- **Fecha:** 2026-09-07 ~01:0x local. **Reglas:** 0 requests HTTP (0/5), 0 SSH, 0 git local,
  0 código tocado. NO-GIT respetado (sin commit/push/PR/deploy).

---

## 0. VEREDICTO: **G3 FIXED + VERIFICADO** (con evento de concurrencia documentado, §3)

`arbx:hot:simulated` en el doc ahora documenta EXACTAMENTE el contrato que vive en el código:
10 campos reales del XADD, productor real (`decode_and_score_tx`, post-publish, fail-soft),
consumidores reales (ambos grupos), MAXLEN real (~5000), y nota de liveness bajo modo v2.
`grep -c "sim_result|trace_hash"` sobre el doc = **0** (era 2 campos documentados como
existentes). Diff total: `docs/redis-schema/hot-path-v2.md` +22/−6, 1 archivo, sin tocar
ninguna otra sección.

## 1. Matriz de verificación — cada línea del doc vs código REAL (RULE 00)

Ninguna línea documentada fue heredada del diseño sin re-verificar contra el árbol:

| Claim en el doc (línea nueva) | Evidencia de código (leída por este fixer) |
|---|---|
| Producer `searcher-rs decode_and_score_tx`, post-publish, fail-soft, sólo si la sim corrió | `backend/searcher-rs/src/scanner.rs:1488` (fn `decode_and_score_tx`) · `:2648` (`publisher::publish`) → `:2651-2669` (emit en `if let Some(sim) = hot_sim`, `warn!` fail-soft en error, jamás falla el pipeline) |
| emitido passed\|failed VERBATIM, nunca pre-REVM | `hot_path_emitter.rs:149` (`status = if result.passed { "passed" } else { "failed" }`) · `scanner.rs:3061-3064` (`hot_sim` sólo post-`spawn_blocking` del outcome) |
| Campo `id` (UUID, correlación de mensaje) | `hot_path_emitter.rs:159-160` |
| Campo `status` | `:161-162` |
| `net_profit_wei` = decimal STRING, GROSS token_in, net-of-gas es decisión downstream | `:163-164` + `:29-35` (comment WO-02: U256 stringified; `simulated_profit_token_in` = gross) |
| `gas_used` (u64) | `:165-166` |
| `gas_price_wei` decimal string | `:167-168` |
| `opportunity_id` = mismo UUID (FK PaperExecutor→opportunities.id) | `:169-170` · consumidor: `backend/api-server/src/paper/executor.ts:69,202` (`parseSimulatedOpportunity`) |
| `chain_id` / `strategy_kind` / `token_pair` (correlación) | `:171-176` (`token_pair` ← `opp.pair_symbol`) |
| `timestamp_ms` epoch millis | `:177-178` |
| MAXLEN ~5000 | `:155-157` (`MAXLEN ~ 5000`) |
| Consumer `ws-emitter-g0` → OpportunityHotStreamer → room `opportunities`, event `opportunity:validated` | `backend/api-server/src/websocket.ts:801` (grupo `ws-emitter-g0`) · `:849` (clase) · `:907` (pollLoop HOT_SIMULATED_STREAM → `opportunity:validated`) · `:952` (`io.to('opportunities').emit`) |
| Consumer `paper-executor-g0` dormante salvo `ARBX_PAPER_EXECUTOR_MODE=on` | `paper/executor.ts:20-22` (`STREAM_IN="arbx:hot:simulated"`, `GROUP="paper-executor-g0"`) · `backend/api-server/src/index.ts:1904-1915` (dormant by default) |
| Liveness: bajo `ARBX_ORCHESTRATOR_MODE=v2` el scanner retorna antes del sim gate ⇒ stream honestamente vacío | `scanner.rs:1589-1592` (leído por este fixer: `if orch_mode == OrchestratorMode::V2 { return Ok(()); }` — el wiring WO-02 vive en el cuerpo legacy, después de ese return) + evidencia VPS de WO-02-CROSS §2 (boot log mode=v2 05:14:30Z, `XLEN arbx:hot:simulated`=0 con wiring deployado) |

## 2. Qué añadió ESTE fixer más allá del diff §5.3 (declarado)

El diff §5.3 del diseño NO incluía la nota de liveness. La añadí como bullet aditivo
(`hot-path-v2.md:47-52`) porque: (a) el propio diseño §6 ya porta el caveat v2 ("en v2 el
scanner retorna antes del sim gate — stream honestamente vacío"); (b) WO-02-CROSS §2 lo
confirmó contra producción (mode=v2, XLEN=0 con wiring vivo); (c) sin ella, el doc
documentaría un productor activo y el próximo lector diagnosticaría mal un XLEN=0 que es
ESPERADO por configuración de modo — exactamente el patrón de mala lectura que el CROSS
sancionó a nivel board (G2). Si el operador la considera fuera de mandato, es UN hunk
independiente (líneas 47-52) revertible sin tocar el resto.

## 3. Evento de concurrencia (transparencia)

A las **00:59:51** local (mtime del archivo), MIENTRAS este fixer verificaba el contrato
contra el código (antes de intentar escribir), **otro agente del gang aplicó el diff §5.3**
al mismo archivo. Mi `Edit` inicial falló por stale-read (protección correcta del harness).
Forense: releí el archivo + `git diff` — el cambio del otro agente es EXACTAMENTE el diff
§5.3 + un comentario marcador HTML (línea 29), sin tocar nada más. **No lo pisé**: verifiqué
cada línea de SU texto contra el código con la matriz §1 (mi verificación era previa e
independiente — nunca heredé su claim), y mi única escritura fue el bullet aditivo §2
(ninguna línea suya modificada). Ambos hunks coexisten con marcadores separados
(`WO-02 (2026-09-06)` línea 29 · `WO-02-CROSS G1/G3 (2026-09-07)` línea 47).

## 4. Drift residual ENCONTRADO fuera de mandato (NO tocado — quirúrgico)

En el mismo archivo, secciones vecinas tienen drift pre-existente que G3 no cubría
(quedan para un fix futuro / decisión del operador — NO los toqué):

1. **`arbx:hot:detected` lista `paper-executor-g0` como consumer** (`hot-path-v2.md:14,24`)
   — FALSO en código: PaperExecutor consume `arbx:hot:simulated` (`executor.ts:20
   STREAM_IN`), NO `arbx:hot:detected`. Único consumer real de hot:detected:
   `ws-emitter-g0` (`websocket.ts:906`). Además `emit_detected` NO tiene call-sites
   (WO-02-CROSS §1) — el stream mismo está sin productor cableado.
2. **`arbx:hot:paper_executed` MAXLEN ~1000** (`hot-path-v2.md:62`) vs código
   `MAXLEN ~ 5000` (`executor.ts:318`); además el doc dice producer "paper archiver
   component" cuando el emitter es PaperExecutor (`executor.ts:21 STREAM_OUT`).
3. Campos de `arbx:hot:detected` documentados (`token_path[]`, `amounts[]`) no coinciden
   con el XADD real de `emit_detected` (`hot_path_emitter.rs:79-94`: id, chain_id,
   strategy_kind, detected_at_ms) — misma clase de drift que G3 pero en la sección hermana.

## 5. Gates

- **Docs-only, cero gates de código** (hint del cross-examiner): cumplido — no se tocó
  ningún archivo de código; `git status` confirma que mis escrituras = 1 doc + este reporte.
- Verificación propia: `git diff` quirúrgico (+22/−6, 1 archivo) · `grep sim_result|trace_hash`
  = 0 · estructura markdown válida (comentarios HTML invisibles al render, patrón ya usado
  en el archivo) · cada claim trazado a file:line (§1).

## 6. Pendiente para el orquestador

- Marcar **G3 FIXED** en la row WO-02 del BOARD (`GOAL-WORKORDERS.md`) — este fixer NO
  editó el board porque ya está `M` en el working tree (riesgo de colisión con edits
  concurrentes de otros fixers de la ronda; documentado en vez de pisado).
- G1 (outcome en prod) sigue **operator-gated**; G2 (reporte post-deploy) ya cubierto por
  el propio CROSS §2 + esta nota de liveness en el doc; G4 operator-gated; G5 (test-nit
  1 línea) sigue abierto para el próximo pase Rust.
- Considerar micro-FIX para el drift §4 (1/2/3) — mismo patrón, docs-only.

---

*WO-02-FIXG3 — 2026-09-07. Fail-honest: el diff §5.3 en disco lo escribió otro agente
concurrente (00:59:51); este fixer lo verificó línea a línea contra el código ANTES de
saberlo (matriz §1) y añadió sólo el bullet de liveness. Drift residual §4 declarado, no
corregido (fuera de mandato).*
