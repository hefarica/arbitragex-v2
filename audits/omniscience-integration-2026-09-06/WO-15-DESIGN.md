# WO-15 — Higiene de consumer-groups de Redis Streams (D-9) + NOTA trim-antes-de-consumo (D-10)

- **Agente:** Gang Omniscience · ecc:database-reviewer (rubric: ecc-backend-patterns + ecc-redis)
- **Fecha:** 2026-09-06 · **Kind:** design + apply (local) — el board no tenía WO-15-DESIGN previo; este documento ES el diseño, y la parte de código se aplica localmente en el archivo bajo claim (`backend/api-server/src/websocket.ts`).
- **Reglas respetadas:** ssh `arbx` SOLO lectura (XLEN/XINFO/XPENDING — 0 writes en VPS, §32/§33), 0 requests a dominio público, 0 git (protocolo operador 2026-08-23), diffs marcados `// WO-15 (2026-09-06)`.
- **Upstream:** board `GOAL-WORKORDERS.md` fila WO-15 · D-9 y D-10 del `00-PREDATOR-ROADMAP.md` · hallazgos 03-api-ws.md §5.6/§7-#4 · 03-api-ws-CROSS C4 · 06-exec-terminus-CROSS N-5/P6 · 07-data-layer-CROSS §2.2/§2.3.

---

## 0. Evidencia viva (censo read-only, 2026-09-06 vía `ssh arbx`)

Contenedor Redis real: `arbitragex-v2-redis-1` (redis:7.2) — **no** `redis` (el nombre de R7 ya no existe; gotcha registrado).

| Stream | Grupo | Consumers | pending>0 | MAX-IDLE | lag | Dueño del grupo |
|---|---|---|---|---|---|---|
| `arbx:opps:detected` | `enricher` | 1 | 0 | 1.7 s (**vivo**) | 10,484 | searcher/enricher |
| `arbx:opps:detected` | `paper-archiver-g0` | 63 | 0 | 199.9 h | 10,548 | api-server (paperArchiver) |
| `arbx:opps:detected` | `selector-g0` | 59 | **1** | 199.9 h | 10,548 | selector-api |
| `arbx:opps:validated` | `sim-ctl-g0` | 51 | 0 | 199.9 h | 3,047 | sim-ctl |
| `arbx:opps:simulated` | `relays-client-g0` | 58 | 0 | 200.1 h | 0 (`last-delivered-id 0-0` = **jamás consumió**) | relays-client |
| `arbx:hot:detected` | `ws-emitter-g0` | 62 | 0 | 200.1 h | 0 (ldi 0-0) | **api-server (OpportunityHotStreamer — ESTA WO)** |
| `arbx:hot:simulated` | `ws-emitter-g0` | 62 | 0 | 200.1 h | 0 (ldi 0-0) | api-server (ídem) |

**Total: 356 consumers en 7 grupos; ~351 huérfanos** (todos menos el vivo de cada grupo). La única entrada pendiente de todo el sistema: `XPENDING arbx:opps:detected selector-g0` → id `1788512726578-0`, consumer `bb833b11d8c7` (huérfano, ~200 h idle), 1 entrega.

Mecanismo de la fuga (código local): `websocket.ts` construye `consumerName = ws-emitter-${process.pid}-${Date.now()}` (único por boot) y NUNCA llama `XGROUP DELCONSUMER` — verificado por 4 verificadores independientes (03-CROSS C4: "nada en el repo llama DELCONSUMER/XAUTOCLAIM"). Los archivers (paper/scored/bridge) y los servicios Rust repiten el patrón con otros nombres (p.ej. container-id corto `0508aae73fe7` en paper-archiver-g0): +1 consumer por recreate de contenedor, ~200 h de stock acumulado.

---

## 1. XGROUP DELCONSUMER en el shutdown graceful del api-server

**El hook de shutdown real (cita):** `backend/api-server/src/index.ts`
- `index.ts:2037-2038` — `process.on("SIGINT"|"SIGTERM", () => shutdown(...))`.
- `index.ts:2000-2003` — guard idempotente `shutdownStarted` (H6).
- `index.ts:2013` — **`await hotOpportunityStreamer.stop().catch(() => {});`** — el streamer YA se detiene en la cadena graceful, antes de cerrar los subscribers Redis (L2016-2018) y el cliente principal (L2019).

Conclusión de diseño: NO se necesita tocar `index.ts`. La desregistración vive DENTRO de `stop()` del propio `OpportunityHotStreamer` (archivo bajo claim), que el hook existente ya invoca.

**Diseño aplicado en `websocket.ts` (`stop()`, L1072-1108 — ref re-verificado post-apply, ver WO-15-APPLY.md):**
1. `running = false` + wake inmediato del maintenance-loop (sleep interrumpible, L856-871) — no deja timers colgantes.
2. **Esperar los XREADGROUP en vuelo** (`Promise.allSettled(this.loopPromises)` acotado a 3 s, L1085-1090). Motivo fino: `XREADGROUP` **auto-crea** el consumer si no existe; un DELCONSUMER ejecutado mientras un XREADGROUP del propio consumer está en vuelo puede llegar ANTES que la lectura → la lectura re-crearía el consumer huérfano y la fuga sobreviviría al shutdown graceful. El XREADGROUP usa `BLOCK 1000` (L912-917), así que el drenaje toma ≤1 s normalmente; el tope de 3 s cabe dentro de la ventana de drain de 5 s del shutdown global (index.ts:2024-2033).
3. `XGROUP DELCONSUMER <stream> ws-emitter-g0 <consumerName>` para AMBOS streams (`HOT_STREAMS`, L798-801) — el grupo existe en `arbx:hot:detected` Y `arbx:hot:simulated` (el censo muestra 62+62; el bug original era pensar en un solo stream).
4. `quit()`. Todo best-effort con catch: un fallo de DELCONSUMER NO puede bloquear ni romper el shutdown del api-server (la purga periódica del §2 recoge lo que este camino no logre).

**Por qué esto solo no basta (SIGKILL):** `docker kill -9`, OOM-kill y un crash duro de Node jamás pasan por `process.on(SIGTERM)`. Docker Compose `stop` envía SIGTERM con grace period (pasa por shutdown), pero `restart: on-failure` tras un crash no. → mecanismo (2).

## 2. Purga periódica de consumers idle > X min — dónde vive y por qué

**Vive DENTRO de `OpportunityHotStreamer`** (`maintenanceLoop`, L973-987, primera pasada a los 60 s post-boot y luego cada `ARBX_WS_CONSUMER_PURGE_INTERVAL_MS`, default 15 min; `purgeIdleConsumers`, L991-1037; `claimPendingEntries`, L1039-1066).

Justificación de ubicación (charter: "NO inventar un servicio nuevo si hay un loop periódico existente donde colgarlo"):
- Los poll-loops del streamer SON el loop periódico ya existente del api-server sobre estos streams; el maintenance-loop es un tercer loop hermano dentro de la MISMA clase (`OpportunityHotStreamer`, L843) — cero servicios, cero archivos nuevos de runtime.
- **Ownership:** el dueño del grupo es el único que conoce al consumer vivo con certeza. El watchdog del edge limpiaría grupos ajenos sin poder distinguir vivo/huérfano mejor que por idle; y los archivers del api-server (paper-archiver-g0, selector-g0 en `arbx:opps:detected`) son superficies de otros WOs — replicar el patrón ahí es follow-up (§6).
- **Replica-safe por construcción:** el criterio es `idle ≥ ARBX_WS_CONSUMER_PURGE_IDLE_MS` (default 30 min, L805-816). Un consumer de otra instancia VIVA resetea su idle en cada XREADGROUP (<1 s) y jamás se toca; el propio (`this.consumerName`) se excluye explícitamente. En rolling-deploy el predecesor queda <30 min y respira.

R9 (log-disciplina): UN summary `info` agregado por sweep (`purged/reclaimed/discarded_pending`), nada per-consumer; `warn` solo en fallo o si un DELCONSUMER descartó pendings (condición que no debería ocurrir — fail-honest audible).

## 3. RUNBOOK del operador — cleanup one-shot del stock (~351 huérfanos)

Agentes NO mutamos VPS (§32/§33). Comandos exactos, todos contra `arbitragex-v2-redis-1` en el VPS (`ssh arbx`). **Ejecutar en una ventana sin deploy** (un recreate durante el loop solo añade 1 consumer que quedará para la siguiente pasada; no rompe nada).

### 3.0 BEFORE (leer y guardar)

```bash
ssh arbx
R="docker exec arbitragex-v2-redis-1 redis-cli --raw"
# Invariante §33.1: el stream NO cambia con esta higiene (XPENDING/XAUTOCLAIM/XACK/DELCONSUMER no hacen XADD/XDEL/XTRIM)
for s in arbx:opps:detected arbx:opps:validated arbx:opps:simulated arbx:hot:detected arbx:hot:simulated; do
  echo "$s XLEN=$($R XLEN $s)"
done
# Conteo de consumers por grupo (before)
for gc in "arbx:opps:detected paper-archiver-g0" "arbx:opps:detected selector-g0" \
         "arbx:opps:validated sim-ctl-g0" "arbx:opps:simulated relays-client-g0" \
         "arbx:hot:detected ws-emitter-g0" "arbx:hot:simulated ws-emitter-g0"; do
  set -- $gc
  echo "$1/$2 consumers=$($R XINFO CONSUMERS $1 $2 | grep -c '^name') pending_total=$($R XPENDING $1 $2 | head -1)"
done
```

(El grep funciona porque `--raw` emite cada clave en línea propia: `name`/`pending`/`idle`/`inactive`.)

### 3.1 FASE A — grupos con `pending_total = 0` (hoy: TODOS menos `selector-g0`)

Umbral conservador 24 h (los vivos tienen idle < 1 s; sobra margen):

```bash
THRESH_MS=86400000
for gc in "arbx:opps:detected paper-archiver-g0" "arbx:opps:validated sim-ctl-g0" \
         "arbx:opps:simulated relays-client-g0" \
         "arbx:hot:detected ws-emitter-g0" "arbx:hot:simulated ws-emitter-g0"; do
  set -- $gc; S=$1; G=$2
  $R XINFO CONSUMERS "$S" "$G" | awk -v s="$S" -v g="$G" -v th="$THRESH_MS" '
    /^name$/    { getline n }
    /^pending$/ { getline p }
    /^idle$/    { getline i; if (i+0 >= th+0) print s, g, n, p }' \
  | while read -r S G NAME P; do
      if [ "${P:-0}" -gt 0 ]; then echo "SKIP (pending>0, usar FASE B): $S $G $NAME pending=$P"; continue; fi
      echo "DELCONSUMER $S $G $NAME"
      $R XGROUP DELCONSUMER "$S" "$G" "$NAME"
    done
done
```

`ws-emitter-g0` es OPCIONAL si se va a deployar este WO pronto: el nuevo maintenance-loop limpia su stock solo (primera pasada a los 60 s del boot). El runbook lo deja listo para hoy.

### 3.2 FASE B — grupo con pendings (hoy: `selector-g0`, 1 entrada) — orden sin pérdida

```bash
S=arbx:opps:detected; G=selector-g0
# 1) Ver QUÉ está pendiente y de quién
$R XPENDING "$S" "$G"                       # summary
$R XPENDING "$S" "$G" - + 10                # detalle: id, consumer, idle, entregas
#    (hoy: 1788512726578-0, consumer bb833b11d8c7, ~200h idle, 1 entrega)
# 2) XAUTOCLAIM PRIMERO: transfiere la entrada a un claimer de limpieza
#    (min-idle 60s: el dueño lleva ~200h muerto, no puede ser tocado por error)
$R XAUTOCLAIM "$S" "$G" wo15-cleanup 60000 0-0 COUNT 100
#    → [cursor, [[id, [json, ...]]], []]  — la entrada pasa a PENDING de wo15-cleanup
#    Opcional: inspeccionar el payload antes de cerrarlo:  $R XRANGE "$S" 1788512726578-0 1788512726578-0
# 3) Cerrar honesto: el archiver ya procesó esa generación (la entrada fue
#    entregada 1x al consumer que murió); XACK la reconoce sin borrarla del stream.
$R XACK "$S" "$G" 1788512726578-0
# 4) AHORA sí, eliminar los huérfanos idle>24h del grupo (mismo loop de FASE A
#    con S/G de selector-g0; el claimer wo15-cleanup también se lleva su DELCONSUMER)
```

Decisión declarada (no implementada por el agente): si el operador prefiere re-procesar esa entrada en vez de XACK (re-entregarla al archiver vivo), el paso 3 se sustituye por consumirla — el orden XAUTOCLAIM→(procesar|XACK)→DELCONSUMER se mantiene invariable.

### 3.3 AFTER (verificación)

```bash
# 1) Conteo after: cada grupo debe quedar en consumers=1 (el vivo) — o cerca,
#    si un deploy a media pasada dejó uno nuevo (<24h idle, se irá en la próxima).
#    Repetir el bloque BEFORE.
# 2) Invariante de stream: XLEN de los 5 streams IDÉNTICO al BEFORE.
# 3) El grupo relays-client-g0 conserva last-delivered-id 0-0 por diseño del
#    servicio (jamás consumió — D-9/06-CROSS N-1); este WO NO cambia ese hecho,
#    solo su higiene de consumers.
```

## 4. Invariante pérdida-cero (orden declarado)

Semántica Redis: `XGROUP DELCONSUMER` **no borra entradas del stream** — elimina el consumer y su PEL (las entradas entregadas-no-reconocidas dejan de poder ser re-entregadas a nadie: eso ES la pérdida evitable). Orden obligatorio en TODOS los caminos de esta WO:

```
(1) XPENDING / XINFO CONSUMERS          → identificar huérfano y sus pendings
(2) XAUTOCLAIM → consumer VIVO          → transfiere ownership de cada pending
(3) emit + XACK (o XACK si ya emitido)  → at-least-once deliberado: re-broadcast
                                          posible tras crash, pérdida cero
(4) XGROUP DELCONSUMER huérfano         → su PEL ya está vacío → descarta 0
```

Soportes en código:
- **XACK post-emit en el camino normal** (`websocket.ts:810-820`): cada entrada broadcasteada se reconoce en batch — el PEL del consumer vivo queda en 0, condición que hace que el DELCONSUMER del §1 (propio) y del §2 (ajeno) nunca descarte nada. Sin esto, cada entrada leída quedaba en el PEL para siempre (el código original jamás llamaba XACK).
- **Purga:** XAUTOCLAIM con `min-idle = CONSUMER_PURGE_IDLE_MS` ANTES del DELCONSUMER (`websocket.ts:879-903`); si un DELCONSUMER aún reporta `removed > 0` (carrera), `warn` audible + contador en el summary (fail-honest).
- **Tests (nuevo `websocket-hot-streamer.test.ts`, 3 tests):** stop() hace DELCONSUMER en ambos streams antes de `quit()` y con el consumerName propio; la purga elimina solo huérfanos idle>30 min (conserva self, par vivo y predecesor reciente) y ordena XAUTOCLAIM ANTES del DELCONSUMER del dueño de los pendings; pollLoop XACKea todo lo emitido.

## 5. NOTA D-10 — trim-antes-de-consumo (MAXLEN del publisher). Documentado, NO implementado

Hecho medido hoy: `lag(paper-archiver-g0) = 10,548` y `lag(selector-g0) = 10,548` > `XLEN(arbx:opps:detected) = 10,000-10,001` → **547 entradas recortadas sin consumir** (lag − length; coincide con el rango 488→547 del roadmap; `entries-added = 525,978` desde el arranque del stream).

Causa raíz (código): `backend/searcher-rs/src/publisher.rs:7` `STREAM_MAXLEN: usize = 10_000` y `publisher.rs:16-26` `XADD ... MAXLEN ~ 10000` (trim APROXIMADO `~` — recorta en nodos del radix tree, por eso XLEN oscila 10,000-10,004). El publisher recorta por longitud del stream SIN coordinar con la posición (`last-delivered-id`) de los consumer-groups: cuando un grupo lleva >10,000 entradas de retraso, el frente de trim ya pasó su posición y esas entradas son irrecuperables — el lag queda estructuralmente > XLEN para siempre (el contador nunca baja aunque el grupo avance, porque las entradas contadas ya no existen).

Contraste: `arbx:opps:validated` XLEN=10,002 con `sim-ctl-g0` lag=3,047 < XLEN → sin pérdida AÚN (05-CROSS §2 confirmado).

Recomendación (decisión del operador; esta WO no la implementa):
1. **Hoy es inerte** (100% del flujo `arbx:opps:detected` es rejected; D-7) — no hay pérdida operativa real, solo deuda estructural. Crítico post-flip.
2. Regla de diseño: `MAXLEN` es una política de retención de hot-path — válida SOLO si TODOS los grupos del stream consumen en vivo (lag≈0). Cualquier consumidor duradero (archiver/ledger) debe persistir en PG desde un consumidor vivo o leer un stream con retención mayor; nunca "alcanzará más tarde" a un stream con MAXLEN.
3. Palancas concretas, en orden de preferencia: (a) mantener MAXLEN 10_000 y FIXEAR a los consumidores dormidos (paper-archiver-g0/selector-g0 arrastran ~10.5K de lag = no están consumiendo: su EMBUDO es el problema, no el trim); (b) si un grupo está legítimamente dormido, avanzarlo explícitamente con `XGROUP SETID <stream> <group> $` (declarar el salto, no fingir consumo) — decisión operador; (c) subir MAXLEN o mover a `XTRIM MINID` anclado al `last-delivered-id` del grupo más lento VIVO (más caro, solo si (a) es imposible).
4. Observabilidad recomendada: alerta cuando `lag > 5% × XLEN` en cualquier grupo (XINFO GROUPS ya expone `lag` en Redis 7.2) — detecta el cruce ANTES de que el trim coma entradas. Cae natural en WO-10 (instrumentación de latencia) o en el watchdog del edge.

## 6. Diffs aplicados (local, 0 git) y verificación

| Archivo | Cambio | Marcado |
|---|---|---|
| `backend/api-server/src/websocket.ts` | knobs env + HOT_STREAMS + sleep/sleepInterruptible (L795-816); `redisClient` inyectable para tests (L821-824); start() guarda loop-promises y lanza maintenanceLoop (L899-905); XACK batch post-emit (L929-935); emitEntry extraído (L944-967, hoy con instrumentación WO-10 superpuesta); maintenanceLoop/purgeIdleConsumers/claimPendingEntries (L973-1062); stop() con drenaje+DELCONSUMER×2 (L1072-1108) | `// WO-15 (2026-09-06)` |

**Errata de refs (re-verificación RESPAWN-2 A, 2026-09-06):** los refs de línea de §1/§2/§4 quedaron anclados a un estado intermedio del archivo — WO-10 aterrizó instrumentación en el MISMO `websocket.ts` y desplazó líneas. Los refs autoritativos, verificados contra el árbol final con `Read`, viven en la tabla de arriba y en `WO-15-APPLY.md` §2. Censo read-only fresco post-apply (delta vs §0): `WO-15-APPLY.md` §4.
| `backend/api-server/src/websocket-hot-streamer.test.ts` | NUEVO — 3 tests de los invariantes (shutdown-deregistro, purga idle con orden XAUTOCLAIM→DELCONSUMER, XACK post-emit) con spy de cliente Redis inyectado | cabecera WO-15 |
| `backend/searcher-rs/src/publisher.rs` | **SIN cambios** — citado como evidencia D-10 (L7, L16-26). La recomendación del §5 NO se implementa (charter) | — |

Verificación (Windows local, target/node_modules calientes):
- `npx tsc --noEmit -p tsconfig.json` (backend/api-server) → **exit 0**.
- `npx vitest run` (backend/api-server, suite completa) → **57 archivos / 692 tests PASAN** (692 incluye los 3 nuevos; antes eran 689). Warnings `[ioredis] ETIMEDOUT` preexistentes de tests de integración que intentan Redis local (RULE 01 prohíbe levantarlo) — no bloquean.
- Runtime: NO verificado contra Redis vivo por diseño (§32/§33 — el agente no deploya; la verificación post-deploy es del operador, §7).

## 7. Verificación post-deploy (operador) y follow-ups

Post-deploy del WO (cuando el operador lo promueva por PR):
```bash
# 1) Boot log del api-server: el streamer arranca igual ("[HotStreamer] Starting poll loops").
# 2) A los ~60 s del boot, UNA línea de higiene por stream:
docker logs arbitragex-v2-api-server-1 2>&1 | grep 'group hygiene'
#    esperado: purged=62 reclaimed=0 discarded_pending=0 en arbx:hot:detected y arbx:hot:simulated
# 3) Conteo after (bloque 3.3): ws-emitter-g0 → consumers=1 en cada stream.
# 4) Tras un docker restart del api-server (SIGTERM): conteo SIGUE en 1 (DELCONSUMER del shutdown);
#    tras un kill -9 + restart: vuelve a 1 en ≤45 min (maintenance loop).
```

Follow-ups FUERA del claim de esta WO (mismo patrón, otras superficies — coherente con 06-CROSS P6 "un patrón, no cuatro PRs"):
1. `paper-archiver-g0` / `scored/opps archivers` (api-server, archivos de archiver no claimados) y `selector-g0` (selector-api): replicar stop()-DELCONSUMER + XACK post-emit + sweep idle. Ideal: helper compartido en `shared-ts`/`shared-rs`.
2. `sim-ctl-g0` (sim-ctl) y `relays-client-g0` (relays-client, Rust): mismo patrón en `shared-rs`; nota dura: `relays-client-g0` tiene `last-delivered-id 0-0` — jamás consumió; su "consumidor vivo" no consume, así que la purga idle debe ir acompañada de la decisión de wiring (06-CROSS N-1), o el sweep limpiaría también al vivo inactivo si su idle supera el umbral — en esos servicios el vivo debe excluirse por nombre, no solo por idle (diferencia documentada vs esta WO, donde el vivo consume continuamente).
3. D-10: decisión del operador sobre el §5 (SETID vs fix del embudo de archivers vs retención).

— EOF WO-15-DESIGN · database-reviewer · 2026-09-06
