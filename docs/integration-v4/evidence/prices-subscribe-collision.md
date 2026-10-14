# EVIDENCIA — Colisión `subscribe:prices` (doble handler) · Track 𝒯

> Detectada durante prep t3 (backend-dev, 2026-10-13, mensaje al capitán `2dfd51b3`).
> Propietario de la resolución: **frontend-dev (t5, "frontend/bus")** — fallo de frontera
> del capitán: el dispatcher unificado es diseño y propiedad de t5; t3 NO toca websocket.ts
> ni priceBus.ts. Registrada también como insumo para t17-FALSATOR (ataques de contrato).
> Fail-honest: toda afirmación cita ruta:línea verificada en esta sesión, o declara su fuente.

## 1. Los dos handlers del MISMO evento

### Handler EXISTENTE (legacy, en producción) — `prices-stream.ts`

| Hecho | Ubicación |
| --- | --- |
| Registro: listener `io.on('connection')` aditivo, separado del gateway | `backend/api-server/src/index.ts:1815` (`attachPriceRooms(io, redis)`) |
| Additividad documentada (semántica EventEmitter: múltiples listeners disparan todos) | `backend/api-server/src/prices-stream.ts:94-97` ("multiple `io.on('connection')` listeners are additive by design") |
| Handler del evento | `backend/api-server/src/prices-stream.ts:104` — `socket.on("subscribe:prices", async (payload: unknown) => {...})` |
| Payload esperado | `{ chain_id: number }` válido (`backend/api-server/src/prices-stream.ts:105-109`; entero 1..2³²−1, `:89-91`) |
| Rooms: una por cadena | `prices:<chain_id>` (`:32` `PRICES_ROOM_PREFIX`, `:36` `roomFor`, `:110` `socket.join`) |
| Respuesta inmediata | `prices:snapshot` con shape `{ chain_id, prices, count, ttl_secs, ts, seq }` (`:113`, contrato en `:21`; tipo `PricesSnapshot` `:46-56`) |
| Rechazo de payload inválido | `prices:error` `{ code: "invalid_chain_id", ... }` (`:107`) |
| Broadcast continuo | `prices:update` mismo shape, vía puente Redis pub/sub `arbx:prices:updated:*` (`:134`, `:166-200`) |

### Handler NUEVO (spec v4, NO aplicado) — `PATCHES/websocket.ts.diff`

| Hecho | Ubicación |
| --- | --- |
| Inserción DENTRO de `setupWebSocketGateway` (mismo `io.on('connection')` del gateway) | diff `@@` sobre `backend/api-server/src/websocket.ts` — función real en `:347`, `io.on('connection')` en `:400` |
| Handler del evento | `.tmp-inbox/v4-diffs/PATCHES/websocket.ts.diff:15` — `socket.on('subscribe:prices', () => {...})` **SIN payload** |
| Room global única | `PRICES_ROOM = 'prices'` (diff `:5`, `:16` `socket.join`) — DISTINTA de `prices:<chain_id>` |
| Respuesta inmediata | `prices:ready` `{ts}` + `prices:snapshot` **`{ ticks: priceBus.snapshot() }`** (diff `:17-18`) — shape DISTINTO al legacy |
| Nombres endurecidos en contratos shared | `.tmp-inbox/v4-diffs/shared-ts/src/prices-contracts.ts:17-21` (`PRICES_ROOM='prices'`, `PRICES_SUBSCRIBE_EVENT='subscribe:prices'`, `PRICES_SNAPSHOT_EVENT='prices:snapshot'`) |
| Los contratos v4 declaran alinear nombres "al patrón subscribe:* del gateway y al comentario G-PRICE-1" | `.tmp-inbox/v4-diffs/shared-ts/src/prices-contracts.ts:15-16` (comentario) — el patrón real ya estaba tomado por prices-stream.ts |

**NOTA t2**: el mapa F0 (`docs/integration-v4/integration-manifest.md:35`) ya registraba que
websocket.ts está "SIN PRICES_ROOM/subscribe:prices hoy" — cierto para el gateway, pero NO
cubre que `subscribe:prices` ya existe en el listener aditivo de prices-stream.ts: la colisión
es específica del EVENTO, no de la room. El parche v4 fue escrito contra main real (base
5f8ea3d, manifest §c:52) donde prices-stream.ts YA existía ⇒ omisión del generador del parche,
no drift del repo.

## 2. Mecánica de la colisión (si se aplicara el parche tal cual)

Socket.IO/EventEmitter: los handlers de DOS listeners `io.on('connection')` distintos se
acumulan sobre el MISMO socket — un `socket.on('subscribe:prices')` por listener. Un solo
emit del cliente dispara AMBOS:

1. **Cliente legacy** (hoy en producción; consumidor mapeado en
   `docs/integration-v4/integration-manifest.md:38` → `frontend/lib/hooks/usePricesStream.ts`,
   NO leído en esta sesión — mapeo citado de t2):
   emite `subscribe:prices {chain_id:1}` →
   (a) handler legacy: join `prices:1` + `prices:snapshot {chain_id, prices, ...}` (correcto) +
   (b) handler v4: join room global `prices` + `prices:ready` + `prices:snapshot {ticks}`
   (**evento extra con shape distinto sobre el mismo nombre** — el cliente recibe DOS
   `prices:snapshot` de formas incompatibles por subscripción) + queda suscrito a TODO el
   fan-out `prices:tick` global (ruido cross-chain no solicitado).

2. **Cliente v4** (parches `frontend/lib/prices/*`): emite `subscribe:prices` sin payload →
   (a) handler legacy: `isValidChainId(undefined)` = false → **`prices:error
   invalid_chain_id` (`:107`) sobre una subscripción legítima del contrato v4** +
   (b) handler v4: ready + snapshot `{ticks}` (correcto).

Resultado: contrato ambiguo para ambos clientes — un error emitido por camino feliz y shapes
incompatibles bajo el mismo nombre de evento. Exactamente la clase de defecto que R10/WO-D5
(documentado en `backend/api-server/src/websocket.ts:415-418`: room `metrics` removida por
handler-sin-productor) prohíbe dejar pasar.

## 3. Restricción vinculante para t5 (fallo de frontera del capitán)

- El dispatcher debe discriminar por payload ANTES de enrutar: payload con `chain_id` válido
  → camino legacy intacto (RULE 00/02, no se rompe consumidor en producción); payload ausente
  → camino v4 (room global + ready + `{ticks}`). Alternativa válida: split explícito de
  evento, pero exige renombrar en `shared-ts/src/prices-contracts.ts:18` ANTES de congelar
  contratos.
- Un solo punto de registro para `subscribe:prices` (o discriminación total): dos handlers
  del mismo evento en listeners distintos queda PROHIBIDO por esta evidencia.
- Owner: frontend-dev (t5). t3 (backend-dev) no modifica `websocket.ts`, `prices-stream.ts`
  ni crea `backend/api-server/src/prices/priceBus.ts`.

## 4. Cadena de custodia del hallazgo

| Paso | Artefacto |
| --- | --- |
| Detección | prep t3, lectura `prices-stream.ts` completa + grep `subscribe:prices` |
| Reporte capitán | mensaje AgentTeams `2dfd51b3-5aaa-4383-b26c-4c86a745c802` (2026-10-13) |
| Fallo de frontera + relanzamiento a frontend-dev como restricción vinculante | respuesta del capitán (mismo hilo, 2026-10-13) |
| Destino auditoría | t17-FALSATOR (quórum §7) — caso de ataque de contrato dup-event |
