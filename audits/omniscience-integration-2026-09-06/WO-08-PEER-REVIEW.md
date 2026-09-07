# WO-08 — PEER REVIEW (verify, adversarial read-only)

- **Reviewer:** ecc:react-reviewer (Gang Omniscience, WO-08 verify / re-despacho 429)
- **Fecha:** 2026-09-06
- **Objeto:** `frontend/components/RuntimePostureBar.tsx` (+44/−4 sin commit) · `frontend/components/__tests__/RuntimePostureBar.test.tsx` (+89) · claims de `WO-08-APPLY.md`
- **Modo:** READ-ONLY sobre código (cero ediciones, cero git); escritura solo de este reporte.
- **Presupuesto HTTP dominio público:** 1/5 usado.

## VEREDICTO: **APPROVE-WITH-NOTES**

El fix es correcto, fail-closed, reproduce el caso exacto del informe §5 bajo test (unitario + regresión a nivel de barra), y las verificaciones independientes pasan (vitest 25/25, tsc exit 0). Ningún defecto CRITICAL. Las notas son (a) un default fail-open PRE-EXISTENTE en `projectChannel` (fuera del diff), (b) la brecha de certificación del provider — declarada por el applier y confirmada aquí con evidencia — que deja el chip socket en CONNECTING de estado estacionario (dirección pesimista correcta), y (c) un vestigio trivial en un test.

---

## 1. Semántica — ¿algún camino a LIVE con subsistemas degradados? (charter §1)

**PASS — el agregado es fail-closed hacia el estado MENOS optimista.**

- `wsConnected=false` → `DISCONNECTED` incondicional (`RuntimePostureBar.tsx:107-112`). Sin caminos a LIVE sin transporte.
- LIVE exige que TODOS los canales de `REALTIME_CHANNELS` proyecten a `LIVE` o `POLLING` (`RuntimePostureBar.tsx:122-131`). `CONNECTING`, `STALE`, `DISCONNECTED`, `DEGRADED`, `ERROR` demuestran el agregado; el peor-token gana (`precedence.indexOf(token) < precedence.indexOf(worst)`, línea 127 — índice menor = peor). El caso "socket arriba + pairs/quote_anchor CONNECTING" produce `CONNECTING` con detail `pairs=CONNECTING, quote_anchor=CONNECTING`.
- La exención `POLLING` está correctamente acotada: `projectChannel` mapea `polling` en canal WS-nativo a `DEGRADED` (`RuntimePostureBar.tsx:86-88`), que demuestra. Un room WS caído no puede esconderse bajo POLLING.
- **Caso exacto del informe §5 cubierto por test, dos veces:** unitario `RuntimePostureBar.test.tsx:151-159` (socket up + pairs/quote_anchor connecting ⇒ `state: "CONNECTING"` nombrándolos — nunca LIVE) y regresión de barra `RuntimePostureBar.test.tsx:318-338` (reproduce el estado de prod: `wsConnected=true`, routes/runtime_ack live ⇒ exige el detail degradado en el HTML; con todos conectados exige `>LIVE<` × canales+1, línea 333). El detail no-null transitoriamente excluye LIVE (LIVE ⇒ `detail: null`, línea 131).
- **No fail-open por clave faltante:** `channels` es `Record<RealtimeChannelId, RealtimeChannelState>` inicializada con los 4 canales en `realtime-slices.ts:96-97`; `setChannel` solo mergea patches (`realtime-slices.ts:101-104`). Si una clave faltara, `ch.status` lanzaría TypeError (fail-loud), jamás defaultearía a conectado.

**NOTA-1 (MINOR, pre-existente, fuera del diff de WO-08):** `projectChannel` termina en `return "LIVE"` por defecto (`RuntimePostureBar.tsx:89`) — cualquier `status` no reconocido en runtime se asumiría conectado (fail-open). Hoy el vocabulario `ChannelStatus` es una unión cerrada de 5 valores (`realtime-slices.ts:63-68`) y el único escritor es `ArbxRealtimeProvider`, así que el sistema de tipos + drift-alarm de 7 tokens (`test:206-208`) lo contienen; pero un switch exhaustivo o guard explícito haría la proyección estructuralmente fail-closed. Follow-up sugerido, no bloqueante: no fue introducido ni agravado por WO-08 (el diff solo añade `socketChipProps` y cambia el render del chip — verificado en `git diff`).

## 2. R1 hydration (charter §2)

**PASS.**

- Grep de evidencia sobre el archivo: cero hits de `suppressHydrationWarning` / `Date.now` / `new Date` / `window.` / `navigator.` / `document.` / `Math.random` / `localStorage` en scope de render (único hit de "Date.now" es el comentario doc línea 23). Único `useEffect` en `RuntimePostureBar.tsx:383-408` — fetch de posture + `setInterval` viven ahí, con cleanup (`alive`, `clearInterval`).
- Estado inicial determinista: `POSTURE_INITIAL` (`:371-376`) y el slice inicial (canales `connecting`, `wsConnected:false`). El test de doble render byte-idéntico (`test:340-344`) lo pinnea.
- Confirmado contra el dominio público (1 request, `GET /`): el SSR sirve exactamente el snapshot inicial honesto — `aria-label="Runtime posture"`, `KILL SWITCH —`, `>socket<`, 4× `>CONNECTING<`. Sin mismatch.
- `suppressHydrationWarning` jamás aparece — ni en contenedores ni en spans. Regla R1 satisfecha sin excepciones.

## 3. Contrato con la fuente de subsistemas (charter §3)

**PASS — los 4 canales consumidos por el badge existen y son honestos en el servidor.**

- `routes`: `subscribe:route_discovery` hace join al room `ROUTE_DISCOVERY_TELEMETRY_ROOM = 'route_discovery'` (`backend/api-server/src/websocket.ts:297-300`, const en `:51`); el puente Redis→WS re-emite `route_discovery_telemetry` con los 5 tipos de evento incluido `route_discovery.tick` (`websocket.ts:44-48` doc, emisión en `:811`). El gate de aceptación client-side (`acceptTickPayload`, `realtime-slices.ts:123-138`) rechaza schema-drift sin setTick (RG-1 fail-closed).
- `runtime_ack`: room real con re-chequeo de capability admin por socket en `websocket.ts:313-324` (exactamente las líneas citadas por el provider), broadcast en `:428`; rechazo no-autorizado emite `error` estructurado (`:319`) — el cliente nunca espera en silencio. Pasivo/event-driven: "sin REST fallback by design" es fiel al servidor.
- `pairs` / `quote_anchor`: superficies REST reales — `fetchPairs` (`frontend/lib/store/catalog-slices.ts:162-176`) y `fetchQuoteAnchor` (`frontend/lib/store/quote-slices.ts:44-58`) contra api-client, con estados `error` explícitos (RULE 00: falla honesta, jamás fabrican snapshot).
- El conjunto de 4 canales del badge = exactamente las superficies con wire + writer (regla del slice, `realtime-slices.ts:11-29`). Nada inventado.

**NOTA-2 (brecha del provider, declarada por el applier en WO-08-APPLY.md §6 — CONFIRMADA con evidencia):**
- `markFresh` estampa `lastMessageAt`/`lastError` pero nunca escribe `status` (`ArbxRealtimeProvider.tsx:72-76`) ⇒ `pairs`/`quote_anchor` permanecen `connecting` en estado estacionario aunque REST entregue cada 30s. El único escritor de `status` para ellos es el sweep `stale` (`:104-109`) o el teardown (`:159-168`).
- El handler `connect` fuerza `routes`/`runtime_ack` a `status:"live"` sin payload aceptado (`:123-124`), contradiciendo la definición del vocabulario (`realtime-slices.ts:55-57`: live = "transport delivering accepted payloads") y el propio comentario del provider (`:26` afirma que un fetch exitoso marca `live` — el código no lo hace; drift doc↔código).
- **Consecuencia post-fix (correcta en dirección):** en producción el chip socket leerá `CONNECTING` persistente con detail `pairs=CONNECTING, quote_anchor=CONNECTING` — pesimista w.r.t. la realidad del transporte, pero HONESTO w.r.t. el store, y jamás un LIVE verde sobre canales grises. El charter exige fallar hacia el estado MENOS optimista: cumplido. Cerrar la certificación del provider (badge + provider juntos) es el follow-up natural ya declarado; queda registrado como NOTA, no defecto de WO-08.

## 4. Re-ejecución independiente (charter §4)

```
$ cd frontend && npx vitest run components/__tests__/RuntimePostureBar.test.tsx
  ✓ components/__tests__/RuntimePostureBar.test.tsx (25 tests) 15ms
  Test Files  1 passed (1) · Tests  25 passed (25)          EXIT=0

$ cd frontend && npx tsc --noEmit
  (sin salida)                                            EXIT=0
```

- **25/25 = 18 intactos + 7 nuevos, verificado estructuralmente:** `git diff --stat` del test = `89 insertions(+)`, 0 deletions (aditivo puro ⇒ los 18 preexistentes no fueron tocados). Los 7 nuevos: 6 del bloque `socketChipProps` + 1 regresión `WO-08:` a nivel de barra (grep del diff). Coincide con APPLY §4.
- Diff del componente: `44 insertions(+), 4 deletions(-)` — coincide con APPLY §3. Los −4 son exactamente la expresión binaria antigua `state={wsConnected ? "LIVE" : "DISCONNECTED"}` reemplazada por `<ConnectionStateChip {...socketChipProps(...)} />`.

## 5. Radix / accessibility (charter §5)

**PASS.**

- Sin primitivas Radix en el componente ⇒ la lección §HG de proxy-wrappers (refs a través de wrappers) no aplica. No hay `forwardRef` ni consumidores de ref.
- Contenedor `role="status"` + `aria-label="Runtime posture"` (`:411-414`, testeado en `test:346-350`); iconos `aria-hidden` (`:249`, `:279`); etiquetas son texto visible real (no solo title). Cero iconos unlabeled.
- Los hints viajan en `title` (disciplina §40: sin apóstrofes, " — " reservado al detail) — testeado (`test:196-234`).
- **NOTA-3 (TRIVIAL, pre-existente):** `role="status"` hace de toda la barra una región aria-live (implícita polite); cada tick del store/poll re-anuncia. Ruido potencial para lectores de pantalla. Fuera del diff de WO-08; registrar para FE-MASTER.
- **NOTA-4 (TRIVIAL):** `expect(healthy).not.toContain("not connected")` (`test:332`) es vestigial — ninguna vía de código produce esa cadena (el hint de DISCONNECTED es "no transport: ...", el detail del socket caído es "the single socket.io connection is down"). Inofensivo; la aserción con carga es el conteo `>LIVE<` de la línea 333.

## 6. Claims de WO-08-APPLY.md — verificación cruzada

| Claim | Veredicto | Evidencia |
|---|---|---|
| Diff +44/−4 en componente | ✔ exacto | `git diff --stat` |
| Test +89, 7 nuevos, 18 intactos | ✔ exacto | diff aditivo puro; grep its nuevos |
| Diagnóstico §2 (wsConnected solo, markFresh no promueve, connect fuerza live) | ✔ confirmado | diff (líneas removidas); `ArbxRealtimeProvider.tsx:72-76`, `:117-125` |
| "LIVE solo si TODOS los subsistemas conectados" | ✔ | `RuntimePostureBar.tsx:122-131` |
| Sin deploy / cero git (oleada 3 = edición local) | ✔ consistente | archivos siguen `M` sin commit; dominio público sirve snapshot inicial idéntico pre/post (indistinguible en SSR) |
| vitest 25/25 + tsc exit 0 | ✔ reproducido | §4 arriba, ejecutado por este reviewer |

## 7. Estado de reglas duras (self-audit del reviewer)

- RULE 00 / R8: este reporte solo declara lo observado; el fix auditado tampoco fabrica estados (detalle null = sin degradación; error viaja verbatim).
- §32/§33: solo lectura; cero executor/wallets/broadcast; VPS no tocado.
- NO-GIT: cero commit/push/PR/deploy; el working tree quedó exactamente como se encontró (solo se añadieron estos dos reportes en `audits/`).
- Presupuesto dominio: 1/5 request (`GET /`, grep del snapshot inicial).

## 8. Conclusión

**APPROVE-WITH-NOTES.** El defecto del informe §5 (badge LIVE con subsistemas CONNECTING) está cerrado con semántica fail-closed, doble cobertura de test del caso exacto, y verificación independiente reproducida. Las 4 notas son no-bloqueantes: dos pre-existentes y fuera del diff (default LIVE de `projectChannel`; aria-live de la barra), una brecha declarada y confirmada del provider (follow-up badge+provider juntos para volver a un LIVE legítimo), y un vestigio trivial de test.
