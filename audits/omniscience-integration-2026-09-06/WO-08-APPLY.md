# WO-08 — APPLY: fix badge socket honesto (R8)

- **Work-order:** WO-08 (GOAL-WORKORDERS.md, informe /goal §5)
- **Hallazgo:** el badge de estado del socket leyó "LIVE" con subsistemas en CONNECTING — viola R8 (estados honestos).
- **Estado:** APPLIED_VERIFIED (tests + tsc locales; sin deploy — oleada 3 es edición local, cero git/VPS).
- **Hora (UTC):** 2026-09-06 ~23:50Z

---

## 1. Localización (a)

- **Badge afectado:** chip `socket` del `RuntimePostureBar` (FE-0009, barra siempre visible en el root layout) —
  `frontend/components/RuntimePostureBar.tsx`.
- **Subsistemas:** los 4 canales del `RealtimeSlice` (`frontend/lib/store/realtime-slices.ts`):
  `routes`, `runtime_ack` (WS-nativos) y `pairs`, `quote_anchor` (REST-nativos), escritos SOLO por
  `frontend/components/providers/ArbxRealtimeProvider.tsx`.
- Descartado `WebSocketIndicator` (`frontend/components/`): es estado de UN socket (stream de
  oportunidades, `wsStatus` del OpportunitySlice) sin agregación de subsistemas — no es la superficie del hallazgo.

## 2. Semántica actual — por qué leía LIVE con subsistemas CONNECTING (b)

El chip renderizaba `state={wsConnected ? "LIVE" : "DISCONNECTED"}` — agregaba SOLO el booleano de
transporte del socket.io compartido e IGNORABA por completo el estado de los subsistemas que ese socket
alimenta. Esa desincronización era real y observable en producción:

1. `ArbxRealtimeProvider.socket.on("connect")` hace `setWsConnected(true)` de inmediato → chip socket = LIVE.
2. En ese mismo instante `pairs`/`quote_anchor` siguen `connecting`: su único escritor es `markFresh()`,
   que estampa `lastMessageAt`/`lastError` pero **jamás promueve `status`** — ningún writer les escribe
   `live`/`polling`, así que su chip queda en CONNECTING de forma perpetua mientras REST entrega con normalidad.
3. Además `routes`/`runtime_ack` se fuerzan a `status:"live"` en el mero `connect`, sin ningún payload
   aceptado (`lastMessageAt === null`), pese a que el vocabulario define `live` = "transport delivering
   accepted payloads".

Resultado: barra con `socket LIVE` + `pairs CONNECTING` + `quote_anchor CONNECTING` — exactamente lo que
capturó el informe /goal §5.

## 3. Corrección aplicada (c) — cambio quirúrgico en el componente del badge

`frontend/components/RuntimePostureBar.tsx` (+44/−4):

- Nueva proyección pura y exportada `socketChipProps(wsConnected, channels)`:
  - `wsConnected=false` → `DISCONNECTED` (con el detail previo "the single socket.io connection is down").
  - `LIVE` **solo si** el socket está arriba Y TODOS los subsistemas están conectados — donde "conectado"
    = token `LIVE`, o `POLLING` en superficie REST-nativa (pairs/anchor; su cadencia normal de snapshot).
  - Si algún subsistema está `CONNECTING`/`DISCONNECTED`/`STALE`/`DEGRADED`/`ERROR`, el chip muestra ese
    estado degradado REAL (peor-token, precedencia espejo de `projectChannel`: disconnected > error >
    connecting > stale > degraded) y nombra a los canales caídos en el `detail` (ej.
    `pairs=CONNECTING, quote_anchor=CONNECTING`).
- El render del chip pasa de la expresión binaria a `<ConnectionStateChip {...socketChipProps(wsConnected, channels)} />`.
- **Decisión declarada:** NO se agrega un token nuevo "PARTIAL" — el vocabulario §34 es un conjunto
  cerrado de 7 tokens con test de alarma de drift; reutilizar `CONNECTING`/`DEGRADED`/etc. es más
  quirúrgico y transmite el mismo estado degradado real que pide la WO.

## 4. Tests (d)

`frontend/components/__tests__/RuntimePostureBar.test.tsx` (+89): 7 tests nuevos —

- 6 de unidad sobre `socketChipProps`: socket caído ⇒ DISCONNECTED aunque los canales digan live;
  todos conectados ⇒ LIVE sin detail; POLLING REST-nativo NO demuestra; socket arriba + pairs/quote_anchor
  CONNECTING ⇒ CONNECTING nombrándolos (nunca LIVE); DISCONNECTED le gana a CONNECTING (precedencia
  peor-token); DEGRADED/STALE/ERROR demuestran el agregado a su propio token.
- 1 de regresión a nivel de barra (renderToStaticMarkup): reproduce el estado del informe (socket up,
  pairs/quote_anchor connecting) y exige el detail degradado en el title del chip; con todos conectados
  exige socket LIVE y ningún "not connected"; restaura el mock compartido.

## 5. Verificación (e) — comandos exactos y salida

```
$ cd frontend && npx vitest run components/__tests__/RuntimePostureBar.test.tsx
  ✓ components/__tests__/RuntimePostureBar.test.tsx (25 tests) 17ms
  Test Files  1 passed (1) · Tests  25 passed (25)      # 18 preexistentes + 7 nuevos

$ cd frontend && npx tsc --noEmit
  (sin salida) EXIT=0
```

## 6. Límites y follow-ups (fail-honest R8)

- **No deploy / cero git** (reglas del squad): el cambio vive solo en el working tree local.
- **Defecto raíz adyacente NO tocado (fuera de charter):** `ArbxRealtimeProvider.tsx` (a) nunca certifica
  `pairs`/`quote_anchor` como conectados (`markFresh` no escribe `status`) y (b) fuerza
  `routes`/`runtime_ack` a `live` en el mero `connect` sin payload aceptado. Con este fix, el chip socket
  ahora muestra honestamente CONNECTING en estado estacionario (porque ESE es el estado real del store)
  en vez de ocultarlo bajo un LIVE verde; cerrar la brecha de certificación del provider es el follow-up
  natural (badge + provider deben moverse juntos para volver a un LIVE legítimo).
- `WebSocketIndicator`/`socket-lifecycle.ts` marcan LIVE en el mero `connect` del stream de oportunidades
  (conectado ≠ "recibiendo"); fuera del alcance de esta WO, se reporta como observación.
