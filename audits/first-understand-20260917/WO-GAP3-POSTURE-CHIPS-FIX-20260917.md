# WO-GAP3 — FIX: header mini-channels socket/pairs/quote_anchor stuck CONNECTING + routes flap LIVE↔DEGRADED

> Fixer gang ronda 1 (2026-09-17). Charter: GAP-3 del BROWSE de mesa — "header
> mini-channels socket/pairs/quote_anchor stuck at CONNECTING permanently (all
> pages, all sessions) while FEED is LIVE and refresh ticks; routes flaps
> LIVE↔DEGRADED — mislabeled indicator trains operator to ignore badges".
> Reglas: RULE 00 / R8, §32/§33 read-only, NO-GIT (edición local + verificación).
> HTTP manual 0/5 · VPS 0 · git 0.

## 1. Síntoma (PRIMARY_SOURCE, citado de pares)

- QA-WS (`BROWSE-QA-de-WebSocket-en-vivo-...md` §6): DOM establece
  `socket: CONNECTING, pairs: CONNECTING, quote_anchor: CONNECTING` mientras el
  canal WS entrega ~47 frames/s reales (§2) y el badge del feed termina LIVE.
- Operador de mesa (`BROWSE-Operador-de-mesa-...md` §1): postura
  `socket CONNECTING · routes DEGRADED · runtime_ack LIVE · pairs CONNECTING ·
  quote_anchor CONNECTING` con feed fresco al segundo vía REST tick.
- La combinación observada (runtime_ack LIVE + socket CONNECTING) ya acota la
  causa: `wsConnected === true` (runtime_ack solo va a LIVE en el handler de
  connect) y el agregado del socket chip degradado por el PEOR subsistema
  (`socketChipProps`, RuntimePostureBar.tsx:103-133, WO-08).

## 2. Causa raíz (CANONICAL_REPO, re-derivada de primera mano)

**Defecto A — `markFresh` jamás escribía `status`.**
`ArbxRealtimeProvider.tsx` (pre-fix :72-76): el helper del REST loop hacía
`setChannel(id, { lastMessageAt, lastError: null })` — SIN `status`. El estado
inicial de todo canal es `status: "connecting"` (`blank()`,
realtime-slices.ts:90-95). Para los canales REST-native (pairs/quote_anchor)
NO existía NINGÚN otro write de status en todo el repo (grep `setChannel\(`:
provider + 1 test). Resultado: pairs/quote_anchor quedaban `connecting` PARA
SIEMPRE aunque su snapshot llegara aceptado cada 30s. El propio doc-comment
del provider (:25-27 pre-fix) declara la intención — "a successful fetch marks
the channel `live` on transport `rest`" — la implementación la omitió.
Cascada: `projectChannel` (RuntimePostureBar.tsx:84) mapea `connecting` →
CONNECTING, y `socketChipProps` (WO-08) degrada el chip socket al peor
subsistema → **socket CONNECTING perpetuo con la conexión arriba**. El propio
test WO-08 (RuntimePostureBar.test.tsx:318-322) documenta este estado de
producción como fixture — hizo honesto el agregado, pero nunca cerró la
transición que lo causaba.

**Defecto B — carrera del REST pass (routes flapea LIVE↔DEGRADED con socket up).**
El REST pass corre cada 30s; si arranca durante una desconexión
(`wsConnectedRef.current === false`), tras los `await` de
fetchPairs/fetchQuoteAnchor/fetchTick el socket puede RECONECTARSE (el handler
de connect pone routes `live`), y el pass entonces estampa
`routes → polling` SOBRE el `live` fresco (pre-fix :87-98). Como markFresh no
escribía status (Defecto A), SOLO el próximo evento de connect podía restaurar
`live` → flap LIVE↔DEGRADED visible con el socket conectado y runtime_ack LIVE
(la firma exacta que observó el Operador de mesa). Nota honesta: los ciclos
genuinos de disconnect/reconnect del transporte (backoff 1.3s→10.3s medido por
QA-WS §3; fallos wss del journey del Operador §4.4) también producen
DEGRADED transitorio real — eso NO es mislabel y NO se enmascara (RULE 00).

**Clasificación**: Defecto A = CANONICAL_REPO (código leído, cadena completa
state→proyección→DOM). Defecto B = CANONICAL_REPO para la carrera (orden de
writes demostrable en el código); la proporción flap-carrera vs
flap-transporte-real en producción = INFERRED (no medible sin deploy).

## 3. Fix aplicado (marcado `// WO-GAP3 (2026-09-17)`)

Archivos (ninguno reclamado por otro WO — git status pre-fix los tenía limpios):

1. `frontend/components/providers/ArbxRealtimeProvider.tsx`
   - `markFresh` ahora también escribe `status: "live"` (un payload aceptado
     DEBE sacar al canal de `connecting`). Para routes-vía-REST el estado
     `polling` se estampa después como antes (WS-native en fallback → DEGRADED
     correcto); para routes-vía-WS y runtime_ack no cambia nada visible.
   - REST failures ahora surfacen el error REAL (R8): `pairsError` /
     `quoteAnchorError` / `tickError` del store viajan verbatim a
     `channel.lastError` → chip ERROR rojo en vez de `connecting` mudo.
   - Defecto B: re-check de `wsConnectedRef.current` DESPUÉS de los awaits —
     un pass que arrancó mid-disconnect no pisa el `live` de la reconexión
     (`if (wsConnectedRef.current) return;` — el WS vuelve a ser dueño de
     routes). Además, con el fix A, cada tick aceptado re-sella `live`
     (self-healing ≤1 cadencia de tick).
2. `frontend/lib/store/realtime-slices.ts`
   - Helper puro `restFetchOutcome(status, error)` → `accepted | failed |
     inflight` (seam testeable headless, patrón "Pure policy helpers" del
     archivo). Import type-only de `FetchStatus` (runtime-slices.ts:36; sin
     ciclo: runtime-slices no importa realtime-slices).
3. Tests:
   - `frontend/lib/store/__tests__/realtime-slices.test.ts`: describe nuevo
     `restFetchOutcome` (4 casos: ready→accepted; error+msg→failed verbatim;
     idle/loading/error-sin-msg→inflight nada certificado).
   - `frontend/components/__tests__/RuntimePostureBar.test.tsx`: regresión de
     proyección — canal REST-native con el estado que el provider ahora
     escribe (`transport:"rest", status:"live"`) proyecta LIVE, no CONNECTING.

Estado steady del header tras el fix (con datos fluyendo): `socket LIVE ·
routes LIVE · runtime_ack LIVE · pairs LIVE · quote_anchor LIVE`. Durante una
desconexión real: socket DISCONNECTED · routes DEGRADED (fallback REST) —
degradación honesta, ahora creíble porque el resto de la barra ya no miente.

## 4. Verificación (todo local, cero git/VPS/HTTP)

- `npx vitest run` (suite COMPLETA): **127 archivos / 1283 tests PASS**
  (incl. 5 nuevos de este fix; creció vs los 126/1265 de los pares por edits
  concurrentes ajenos).
- `npx tsc --noEmit`: 3 errores, TODOS pre-existentes y ajenos —
  `components/OpportunityTicker.tsx` (1) + `lib/schemas.test.ts` (2), zona del
  par que está aterrizando FEED-SCHEMA-01 (block_number string; working tree
  muestra schemas.ts/opportunities-live.ts modified en vuelo). PROBADO con
  stash-roundtrip de MIS 4 archivos: los mismos 3 errores existen sin mi diff
  (grep idéntico pre/post). Cero errores en superficie propia.
- `npx eslint` sobre los 4 archivos tocados: exit 0.
- Limitación declarada (fail-honest): el efecto del provider no es testeable
  bajo renderToStaticMarkup (env node, sin jsdom — patrón del repo); la
  cobertura del wiring es indirecta (contrato puro + proyección). La
  verificación en vivo del header queda post-deploy (operator-gated; recordar
  RULE 03: rebuild --no-cache del frontend para que aplique).

## 5. Residuales documentados (NO tocados, fuera de charter)

- **ROOM-AUTH-01** (QA-WS §5.2): el server responde
  `42["error",{"code":"unauthorized","room":"runtime_ack"}]` al join sin token
  admin, pero el handler de connect marca runtime_ack `live` sin esperar ack
  de suscripción → chip LIVE sobre un room rechazado. Mismo género de mislabel
  (RULE 00), pero requiere conocer el protocolo de error/ack del server antes
  de tocar (el evento "error" del socket también lo consumen otras vías).
  Dejado para su propio fixer.
- **NO-WS-LOGS-01 / LOGFLOOD server-side** (QA-WS §5.3-5.4): sin observabilidad
  server-side de conexiones socket.io. Infra/VPS, operator-gated.
- Flap por churn REAL del transporte (túnel/red local): sigue visible y debe
  seguir siéndolo — enmascararlo con histéresis fabricaría estado (violaría
  RULE 00 / R8).

## 6. Sincronía de mesa redonda

- Construye sobre: QA-WS §2/§5.6/§6 (frames vivos + chips CONNECTING),
  Operador de mesa §1/§4.4 (postura exacta + polling), y el test WO-08 de
  RuntimePostureBar.test.tsx:318-338 (que ya aislaba el síntoma como fixture).
- No contradice a ningún par: el "socket CONNECTING" del Operador §4.4 no era
  (solo) transporte sin upgrade — era este defecto de estado; su observación
  de frescura vía REST tick es consistente con el REST loop que ya funcionaba.
- Sin colisión de archivos: mis 4 archivos estaban limpios en git status
  pre-fix; los edits concurrentes de pares (layout.tsx, schemas.ts,
  opportunities-live.*, StatusPill, NavigationSentinel) son disjuntos.
- NO deployado (NO-GIT). Post-deploy, el criterio de verificación del header
  es el estado steady del §3.
