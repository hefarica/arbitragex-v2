# WO-02 — FIX G3B (Gang Omniscience, ronda 2): drift residual §4 de WO-02-FIXG3

- **Work-order:** WO-02 · **Tipo:** FIX (docs-only) — adjudicación: drift residual declarado-no-corregido en `WO-02-FIXG3.md` §4 (items 1-3) + R-1 de `WO-02-FIX.md` §6.
- **Archivo:** `docs/redis-schema/hot-path-v2.md` — sections hermanas de la sección ya corregida por G3 ronda 1 (`arbx:hot:simulated`, líneas 29-52 pre-fix — **intactas**, cero caracteres tocados).
- **Reglas respetadas:** 0 commit/push/PR/deploy (NO-GIT), 0 SSH/VPS, 0 requests HTTP (0/5 presupuesto), 0 código tocado (`git status` verificado: único archivo M = el doc). Marcadores `WO-02 (2026-09-06)` en cada hunk (ID del WO, fecha del programa; ejecución real 2026-09-07 — fail-honest, misma convención que WO-02-CROSSFIX-G5).
- **Fecha:** 2026-09-07 (ronda 2 del gang).

---

## VEREDICTO: **FIXED_VERIFIED** — los 3 hallazgos §4 + R-1 corregidos; cada línea nueva traza a código leído por este fixer (matriz §1); grep de claims falsos = 0.

## 1. Matriz de verificación — doc nuevo vs código REAL (RULE 00)

### 1.1 `arbx:hot:detected` (hallazgos §4.1 + §4.3)

| Claim en el doc (nuevo) | Evidencia de código (leída por este fixer) |
|---|---|
| Producer `HotPathEmitter::emit_detected` SIN call-sites ⇒ stream honestamente vacío | grep `emit_detected` en `backend/searcher-rs/src` = SOLO la definición `hot_path_emitter.rs:71` (0 invocaciones; coincide con WO-02-CROSS §1) |
| Fields = `id`, `chain_id`, `strategy_kind`, `detected_at_ms` — NADA más | XADD `hot_path_emitter.rs:79-94` (`.arg("id")` :85, `chain_id` :87, `strategy_kind` :89, `detected_at_ms` :91). `token_path[]`/`amounts[]` ELIMINADOS del doc (grep = 0) |
| `strategy_kind` = snake_case (`StrategyKind::as_str`) | `hot_path_emitter.rs:90` (`opp.strategy_kind.as_str()`) + doc-comment del emitter `:65` ("Strategy variant (snake_case)"); el viejo "e.g. `HolonomicLoopResolution`" era casing inexistente en el wire |
| MAXLEN ~10000 | `hot_path_emitter.rs:81-83` (`MAXLEN ~ 10000`) |
| Side effect HSET `arbx:hot:opp:{id}` + TTL 300s | `hot_path_emitter.rs:97-112` (HSET `data` :100-105, EXPIRE 300 :108-112) |
| Único consumer = `ws-emitter-g0` → WS room `opportunities`, event `opportunity:detected` | `websocket.ts:801-802` (grupo + stream), `:885` (XGROUP CREATE), `:906` (`pollLoop(HOT_DETECTED_STREAM, 'opportunity:detected')`), `:952` (`io.to('opportunities').emit`) |
| `paper-executor-g0` NO consume este stream | `paper/executor.ts:20` (`STREAM_IN = "arbx:hot:simulated"`) — grep repo `arbx:hot:detected`: 0 referencias en executor |

### 1.2 `arbx:hot:paper_executed` (hallazgo §4.2)

| Claim en el doc (nuevo) | Evidencia de código |
|---|---|
| Producer = api-server `PaperExecutor` (NO "paper archiver") | `executor.ts:21` (`STREAM_OUT`), `:303-319` (`emitResult`); el componente "paper archiver" es OTRO subsistema (archiver de `paper_trade_runs`, `index.ts:1895`) que NO escribe este stream |
| Dormant salvo `ARBX_PAPER_EXECUTOR_MODE=on` | `index.ts:1904-1917` (default off, log `paper_executor.dormant`) |
| Emite 1 entrada por sim passed; `failed` se ack sin emitir | `executor.ts:209-216` (`skip_failed` → xack → return, sin emitResult) |
| Fields: `id`, `status` ACCEPTED\|REJECTED, `net_yield_wei`, `executed_at_ms`, `execution_time_ms`, `rejection_reason` opcional | XADD `executor.ts:306-318`; status enum `:54` + `:250`; rejection_reason `:251` (`net_yield_non_positive`) y `:239` (`calculation_failed`). `paper_pnl_usd` ELIMINADO (grep = 0 — la conversión USD vive en PG `persistRun` `:340-371`, `weiToUsd` `:131-134`) |
| MAXLEN ~5000 | `executor.ts:318` (`xadd(STREAM_OUT, "MAXLEN", "~", 5000, ...)`) — doc decía ~1000 (grep `~1000` como valor = 0; único hit = substring de `~10000` de hot:detected, línea 21) |
| Side effect PG + métricas | `persistRun` `:340-371` (INSERT paper_trade_runs); `updateMetrics` `:321-328` (HINCRBY `arbx:metrics:throughput:paper_executed`, `arbx:metrics:latency:paper_execution`) |

### 1.3 `arbx:hot:sim:{id}` (hallazgo §4.3 / R-2 de WO-02-FIX)

| Claim en el doc (nuevo) | Evidencia de código |
|---|---|
| Content = ÚNICO campo `result` = JSON `SimulationResult` (`passed`, `net_profit_wei`, `gas_used`, `gas_price_wei`) | HSET `hot_path_emitter.rs:187-192` (`.arg("result").arg(result_json)`); struct `SimulationResult` `:36-42`; serialización `:185` |
| Escrito SOLO para sims passed | `hot_path_emitter.rs:183` (`if result.passed`) |
| TTL 300s (pre-existente, correcto) | `hot_path_emitter.rs:194-198` |
| "Full REVM trace summary / State diffs / Error logs" ELIMINADOS | grep = 0 como claims (único hit restante = mi cláusula de negación línea 82: "no ... are stored") |

### 1.4 Purpose line de `arbx:hot:simulated` (R-1 de WO-02-FIX)

| Claim en el doc (nuevo) | Evidencia de código |
|---|---|
| "every simulation that actually ran (passed \| failed, REVM verdict verbatim)" — no solo passed | `hot_path_emitter.rs:149` (`status = if result.passed {"passed"} else {"failed"}` — ambos verdicts al XADD `:161-162`) — la línea Purpose vieja ("for opportunities that passed validation") contradecía al Producer corregido 2 líneas abajo; grep `passed validation` = 0 |

## 2. Scope quirúrgico (no tocar la sección G3 ronda 1)

- 4 hunks propios: `arbx:hot:detected` (sección completa) · línea Purpose de `arbx:hot:simulated` (1 línea, marcador inline — la sección reescrita por ronda 1 NO se tocó: su bloque Producer 10-campos + bullet Liveness están byte-idénticos, visibles como contexto intacto en `git diff HEAD`) · `arbx:hot:paper_executed` (sección completa) · `arbx:hot:sim:{id}` (bloque Content).
- Cero archivos de código: `git status` sobre `docs/`, `backend/api-server/src/paper/`, `backend/searcher-rs/src/hot_path_emitter.rs`, `backend/api-server/src/websocket.ts`, `backend/api-server/src/index.ts` = único M = el doc.
- Gates: docs-only markdown — no aplica tsc/vitest/cargo (ningún archivo de código tocado; misma declaración fail-honest que ronda 1). Verificación = matriz §1 + greps §3.

## 3. Greps de verificación (ejecutados post-edit)

- `token_path` = 0 · `amounts[` = 0 · `paper_pnl_usd` = 0 · `paper archiver` = 0 · `passed validation` = 0 · `State diffs` = 0 · `Error logs` = 0 · ``success`, `failed`` = 0.
- `REVM trace summary` = 1 → línea 82, cláusula NEGATIVA propia ("no ... are stored") — no es un claim de existencia.
- `~1000` = 1 → línea 21, substring de `~10000` (valor REAL del código para hot:detected) — no es el viejo MAXLEN de paper_executed.

## 4. File-claim discipline

- `docs/redis-schema/hot-path-v2.md` = claim exclusivo de WO-02 (G3 ronda 1, mismo archivo) — continuación del mismo WO, sin conflicto con otros WO.
- Board NO editado (precedente ronda 1: archivo en working tree con edits concurrentes de fixers paralelos; documentado en vez de pisado). **Pendiente orquestador**: marcar "G3 residual §4 CLOSED (WO-02-FIXG3B)" en la fila WO-02.

## 5. Residuales declarados, NO tocados (fuera del charter §4)

- **R-B1**: sección `arbx:metrics:throughput:detected` (doc) documenta "INCR on each detection event" — grep `throughput:detected` en `backend/` = **0 referencias**: ningún código escribe ni lee esa clave (las métricas reales son `arbx:metrics:throughput:paper_executed` y `arbx:metrics:latency:paper_execution`, `executor.ts:324,327`). Mismo patrón de drift; requiere decidir si el key es roadmap o cable muerto del design doc original.
- **R-B2**: sección `arbx:hot:opp:{id}` — bullets ("Raw detection payload / Decoded token symbols / Source manifold identifiers / Priority score") son especulativos; el HSET real es un único campo `data` = `Opportunity` serializada (`hot_path_emitter.rs:97-105`). El header "Complete opportunity data" sí es cierto; se dejó intacta por fuera de mandato.
- **R-B3**: tabla Latency Budgets mapea "Edge Response <10ms" a `arbx:hot:paper_executed` — mapping aspiracional sin verificación de código; no es un claim de wire-contract, se dejó.

## 6. Presupuesto

0 requests HTTP al dominio público (0/5). 0 SSH. 0 git de escritura (solo `git diff`/`git status` de lectura). Escrituras: `docs/redis-schema/hot-path-v2.md` (4 hunks) + este reporte.

---

*WO-02 FIXG3B — 2026-09-07, gang Omniscience ronda 2. Fail-honest: los 3 hallazgos §4 + R-1 cerrados con evidencia propia re-leída (ningún claim heredado); residuales R-B1/R-B2/R-B3 declarados sin tocar; sección G3 ronda 1 intacta.*
