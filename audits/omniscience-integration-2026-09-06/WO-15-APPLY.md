# WO-15 — APPLY (mitad A: ítems 1-3) · verificación del estado real del árbol

- **Agente:** Gang Omniscience · ecc:database-reviewer — **REEMPLAZO A (RESPAWN-2)**, doble conocimiento sobre el agente caído.
- **Fecha:** 2026-09-06 · WO-15, kind apply · mitad A = ítems 1-3 del charter (mitad B = ítems 4-5, en paralelo).
- **Reglas respetadas:** ssh `arbx` SOLO lectura (XINFO CONSUMERS/GROUPS/XLEN/XPENDING — 0 writes, §32/§33), 0 requests a dominio público, 0 git (protocolo operador 2026-08-23), diffs marcados `// WO-15 (2026-09-06)`.

---

## 1. Situación al llegar (aserciones ≠ facts — verificado, no confiado)

El agente original murió DESPUÉS de aterrizar diseño + código + tests, pero ANTES de escribir este
reporte. Evidencia forense del árbol al llegar:

- `backend/api-server/src/websocket.ts` — MODIFICADO (330 insertions): cambios WO-15 completos y
  coexistiendo con instrumentación WO-10 (emitEntry/hot-window) en el mismo archivo.
- `backend/api-server/src/websocket-hot-streamer.test.ts` — NUEVO, 3 tests de los invariantes.
- `backend/searcher-rs/src/publisher.rs` — MODIFICADO **por WO-10** (familia de latencia
  `arbx_pipeline_latency_seconds`; todo marcado `WO-10 (2026-09-06)`). **WO-15 NO lo toca**: la
  evidencia D-10 sigue intacta (`STREAM_MAXLEN: usize = 10_000` y `XADD ... MAXLEN ~ 10000`).
- `WO-15-DESIGN.md` ya existía con las 7 secciones completas (incluido el RUNBOOK §3); sus
  line-refs de §1/§2/§4/§6 estaban desfasados porque WO-10 desplazó líneas — corregidos con errata
  en §6 (ref único autoritativo: §2 de este reporte).

## 2. Ítem 1 — XGROUP DELCONSUMER en el shutdown graceful: APLICADO y VERIFICADO

Hook de shutdown real (citas verificadas contra el árbol, no copiadas del diseño):

- `backend/api-server/src/index.ts:2037-2038` — `process.on("SIGINT"|"SIGTERM", () => shutdown(...))`.
- `backend/api-server/src/index.ts:2000-2003` — guard idempotente `shutdownStarted`.
- `backend/api-server/src/index.ts:2013` — `await hotOpportunityStreamer.stop().catch(() => {});`
  (el streamer se detiene ANTES de cerrar subscribers Redis y cliente principal).

Diseño materializado en `backend/api-server/src/websocket.ts`:

- **L1072-1108 `stop()`**: `running=false` → wake del maintenance-loop (L1080) → **drenaje de
  XREADGROUP en vuelo** con `Promise.race([Promise.allSettled(loopPromises), sleepInterruptible(3s)])`
  (L1086-1090) — crítico porque XREADGROUP auto-crea consumers: un DELCONSUMER que llegue antes que
  la lectura re-crearía el huérfano — → **`XGROUP DELCONSUMER` para AMBOS hot streams** con el
  `consumerName` propio (L1097-1106) → `quit()` (L1107). Best-effort con catch: un fallo de
  DELCONSUMER no rompe el shutdown; el sweep del ítem 2 recoge lo caído.
- **L801 `HOT_STREAMS`** = `[arbx:hot:detected, arbx:hot:simulated]` — el bug original era pensar
  en un solo stream; el censo muestra 62+62 huérfanos.

**Test:** `websocket-hot-streamer.test.ts` "stop() XGROUP DELCONSUMERs this boot's consumer from
BOTH streams before quit" — PASA (ver §5).

SIGKILL/OOM no pasan por aquí → cubiertos por el ítem 2 (sweep periódico), como declara el diseño §1.

## 3. Ítem 2 — Purga periódica idle (XAUTOCLAIM→DELCONSUMER): APLICADO y VERIFICADO

Vive DENTRO de `OpportunityHotStreamer` — cero servicios nuevos (charter: "no inventar un servicio
si hay un loop periódico existente"):

- **L977-989 `maintenanceLoop()`** — primera pasada a los 60 s del boot (limpia el stock del
  predecesor sin esperar el intervalo completo), luego cada `ARBX_WS_CONSUMER_PURGE_INTERVAL_MS`
  (default 15 min, L815). Sleep interrumpible (L857-862) para que `stop()` no deje timers colgantes.
- **L991-1037 `purgeIdleConsumers()`** — por cada hot stream: `XINFO CONSUMERS` → skip self y
  vivos/recientes (`idle < ARBX_WS_CONSUMER_PURGE_IDLE_MS`, default 30 min, L816) → si el huérfano
  tiene pendings: `claimPendingEntries` PRIMERO → `XGROUP DELCONSUMER`. Si un DELCONSUMER reporta
  `removed > 0` (carrera): `warn` audible + contador (fail-honest, L1019-1024). R9: UN summary
  `info` por sweep (`purged/reclaimed/discarded_pending`, L1031-1035), nada per-consumer.
- **L1039-1062 `claimPendingEntries()`** — `XAUTOCLAIM` iterado (min-idle = umbral de purga,
  COUNT 100, cap 100 iteraciones/sweep, cursor abierto → avisa y retoma el próximo sweep),
  re-broadcast `at-least-once` + `XACK` por entrada — pérdida cero declarada en §4 del diseño.
- Soporte del invariante: **L929-935 XACK batch post-emit** en `pollLoop` — sin esto el PEL del
  propio consumer crecería para siempre y DELCONSUMER descartaría entradas.

Replica-safe por construcción: un consumer de otra instancia VIVA resetea su idle en cada
XREADGROUP (<1 s) y jamás se toca; en rolling-deploy el predecesor queda <30 min y respira.

**Tests:** "purge removes only idle orphans and XAUTOCLAIMs pending entries BEFORE their
DELCONSUMER" y "pollLoop XACKs every entry it broadcast" — PASAN (ver §5).

## 4. Ítem 3 — RUNBOOK del operador: listo (WO-15-DESIGN.md §3) + censo BEFORE fresco

El RUNBOOK completo (BEFORE / FASE A pending=0 / FASE B con pendings / AFTER + invariante XLEN)
vive en `WO-15-DESIGN.md` §3.0-§3.3 — agentes NO mutamos VPS (§32/§33). Censo read-only fresco
corroborado hoy por el reemplazo A (`ssh arbx`, ~21:33-21:36 local, contenedor
`arbitragex-v2-redis-1`):

| Stream | Grupo | Consumers | pend>0 | MAX-IDLE | lag | XLEN |
|---|---|---|---|---|---|---|
| arbx:opps:detected | enricher | 1 (vivo, idle 19 ms) | 0 | 19 ms | 10,484 | 10,001 |
| arbx:opps:detected | paper-archiver-g0 | 63 | 0 | 200.6 h | 10,548 | 10,001 |
| arbx:opps:detected | selector-g0 | 59 | 1 | 200.6 h | 10,548 | 10,001 |
| arbx:opps:validated | sim-ctl-g0 | 51 | 1→0 (ver nota) | 200.6 h | 3,047 | 10,000 |
| arbx:opps:simulated | relays-client-g0 | 58 | 0 | 200.7 h | 0 (ldi 0-0) | **0** |
| arbx:hot:detected | ws-emitter-g0 | 62 | 0 | 200.7 h | 0 (ldi 0-0) | **0** |
| arbx:hot:simulated | ws-emitter-g0 | 62 | 0 | 200.7 h | 0 (ldi 0-0) | **0** |

**TOTAL = 356 consumers; ~351 huérfanos** — sin deriva vs el censo del diseño (§0). Notas delta,
fail-honest (ambas lecturas reportadas, ninguna interpretada como ley):

1. **`sim-ctl-g0` mostró 1 consumer con pending en el censo T1 (21:33) y `XPENDING ... - + 5`
   devolvió vacío + `XINFO GROUPS` pending=0 en T2 (21:36).** No fabrico explicación: el RUNBOOK
   es robusto a esto por diseño — FASE A hace skip per-consumer si `pending>0` (va a FASE B) y
   FASE B se aplica a CUALQUIER grupo con pendings, no solo a los del snapshot. El operador debe
   leer el BEFORE en su ventana antes de ejecutar, no confiar en esta tabla como estado congelado.
2. **XLEN=0 en `arbx:hot:*` y `arbx:opps:simulated`**: hoy NO hay publisher vivo en esos streams
   (flujo 100% rejected, D-7). Los 124 huérfanos de `ws-emitter-g0` tienen pending=0 confirmado →
   su limpieza es hoy de riesgo cero; y el nuevo maintenance-loop del api-server (ítems 1-2) los
   purga solo tras deploy (primera pasada a los 60 s del boot) — el RUNBOOK los marca OPCIONALES
   por exactamente esto.
3. El único pending estable del sistema sigue siendo `selector-g0` → id `1788512726578-0`,
   consumer `bb833b11d8c7` (~200 h idle, 1 entrega) — FASE B del RUNBOOK lo cubre con el orden
   sin pérdida XAUTOCLAIM→(procesar|XACK)→DELCONSUMER.

Lag fresco para D-10 (mitad B): lag 10,484-10,548 > XLEN 10,001 en `arbx:opps:detected`
(483-547 entradas estructuralmente recortadas); `sim-ctl-g0` lag 3,047 < XLEN 10,000 (sin pérdida).

## 5. Verificación toolchain (ejecutada por el reemplazo A, NO copiada del caído)

| Check | Comando | Resultado |
|---|---|---|
| Typecheck api-server | `npx tsc --noEmit -p tsconfig.json` (desde `backend/api-server`) | **exit 0** |
| Tests superficie websocket | `npx vitest run src/websocket-hot-streamer.test.ts src/websocket.test.ts src/websocket-rooms.test.ts src/websocket-carnot.test.ts src/websocket-wo10.test.ts` | **5 archivos / 17 tests PASAN** (3 de WO-15 hot-streamer incluidos) |
| Runtime vs Redis vivo | — | NO verificado por diseño (§32/§33: el agente no deploya); verificación post-deploy = operador, §7 del diseño |

Advertencia de espejo paralelo: la suite completa de api-server NO se corrió en esta pasada porque
otros agentes del gang editan concurrently el mismo árbol (`credentials/`, `simulation/`,
`routes/trading-config.ts` visibles modified en `git status`) — un fallo ajeno contaminaría la
lectura. La superficie de ESTA WO (websocket + su test nuevo) está íntegramente verde y tsc
compila el workspace completo con los cambios WO-15 dentro.

## 6. Estado de mi mitad y pendientes del orquestador

- Ítem 1 (shutdown DELCONSUMER): **DONE** — código + test + hook verificados.
- Ítem 2 (sweep periódico XAUTOCLAIM→DELCONSUMER): **DONE** — código + 2 tests verificados.
- Ítem 3 (RUNBOOK one-shot ~351 huérfanos): **DONE** — WO-15-DESIGN.md §3 + censo fresco §4 aquí.
  Ejecución = OPERADOR (0 writes por agentes).
- Ítems 4-5 (invariante formal §4 + nota D-10): mitad B — el diseño ya los documenta (§4/§5) y el
  test de ordenamiento los cubre; B fusiona su verificación.
- Follow-ups declarados (fuera de claim, diseño §7): archivers paper/scored/selector, sim-ctl,
  relays-client (Rust, `shared-rs`) — mismo patrón, otros PRs; decisión D-10 = operador.

— EOF WO-15-APPLY · database-reviewer · reemplazo A (RESPAWN-2) · 2026-09-06
