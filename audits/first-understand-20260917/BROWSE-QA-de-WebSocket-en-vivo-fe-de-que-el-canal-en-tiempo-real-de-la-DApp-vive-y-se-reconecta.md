# BROWSE-QA — WebSocket en vivo: fe de que el canal en tiempo real de la DApp vive y se reconecta

> Persona: usuario HUMANO de la DApp (Gang Omniscience, 2026-09-17). Journey con DevTools
> (Playwright MCP instrumentando frames WS reales). Dominio: https://arbx.ape-tv.net.
> Presupuesto HTTP manual usado: 4 navegaciones (límite 5). VPS: SOLO lectura vía `ssh arbx`.
> Cero commit/push/PR/deploy. Cero mutación VPS.

## VEREDICTO: FE (con 6 observaciones documentadas abajo)

El canal en tiempo real VIVE, transporta frames reales, se reconecta con backoff creciente
y NO duplica suscripciones en estado estacionario. Cero mixed-content. La ruta WS termina
en api-server :8080 y NUNCA en el Edge (RULE 02 verificada en producción).

## 1. Conexión y ruta (RULE 02) — VERIFICADO

- Cliente: `wss://arbx.ape-tv.net/socket.io/?EIO=4&transport=websocket` (socket.io EIO=4,
  engine.io). Handshake server: `{"pingInterval":25000,"pingTimeout":20000,"maxPayload":1000000}`.
- Ruta viva en VPS (nginx ACTIVO, `nginx -T` línea 222-223):
  `location /socket.io/ { proxy_pass http://127.0.0.1:8080/socket.io/; # RULE 02: WS directo a api-server, nunca via edge (fix POLLING 2026-08-20) }`
- Contraste (clasificación: evidencia dura):
  - `curl 127.0.0.1:8787/socket.io/?EIO=4&transport=polling` (Edge Hono) → **404** — el Edge
    NO tiene ruta socket.io. Si el WS fuera via Edge, moriría aquí. NO lo es.
  - `curl 127.0.0.1:8080/socket.io/...` (api-server) → **200** + handshake válido.
  - `curl 127.0.0.1:5173/socket.io/...` (frontend Next) → **200** con `upgrades:["websocket"]`.
- `reverse_proxy.conf:3-16` documenta el fix WS-POLL-1 (2026-08-20). `nginx_patch.conf:25-26`
  (proxy a :8787) está STALE — NO está en el nginx activo (verificado `nginx -T`).
- Matiz honesto: el browser NO se conecta "directo" a :8080 (no hay `NEXT_PUBLIC_WS_URL`
  horneado en el contenedor frontend — `docker exec ... env` solo muestra `NEXT_PUBLIC_EDGE_URL`);
  la conexión es same-origin y el proxy la baja a api-server. RULE 02 cumplida en sustancia
  (cero tránsito Edge); en la letra, la env `NEXT_PUBLIC_WS_URL` (compose.dev.yml:341,348 +
  .env:129) NO llegó al runtime del contenedor frontend. Clasificación: INFERRED (cadena
  tunnel→nginx→8080; el ingress del tunnel es remotely-managed y no es legible localmente).

## 2. Frames reales + ritmo 60s — VERIFICADO (ni silencio, ni flood letal)

Ventana instrumentada 60s (reload + contadores `page.on('websocket')`):

| Métrica | Valor |
|---|---|
| Conexiones WS abiertas en la ventana | 4 (2 sesiones engine.io × probe/sid, ver §5) |
| Frames RECIBIDOS | **5.068** (~84/s incl. handshake/boot) |
| Frames ENVIADOS por el cliente | 14 (subscripciones + pongs) |
| Estado estacionario 30s (sin reload) | 1.421 frames de datos = **~47/s**, todos con payload |
| Evento dominante | `route_discovery_telemetry` 1.418/1.421 (99.8%) |
| Pings server (25s interval) | 1-2 por ventana — salud del keepalive OK |
| Badge header al final | **LIVE** (transicionó IDLE → LIVE al conectar) |

Frame entrante real (muestra, t=11.16s tras reload):
`42["route_discovery_telemetry",{"algorithm":"multihop_negcycle","block_number":25995384,"chain_id":1,"event":"route_scanner.cycle","hops":4,...}]`
— datos on-chain reales (block_number mainnet actual, no mock). RULE 00 OK en el canal WS.

- Cero frames con payload vacío detectados en 90s de muestreo (todo frame ≥6 bytes, mínimo
  = `3probe`). Cero campos null no declarados en los frames muestreados (los nulls R8 legítimos
  viven en el DOM, ver §6).

## 3. Reconexión forzada (offline/online) — VERIFICADO

Toggle `context.setOffline(true)` 15s → `setOffline(false)`, midiendo:

| t (ms desde offline) | Evento |
|---|---|
| 1299 / 3027 / 5256 / 10272 | intentos WS fallidos `ERR_INTERNET_DISCONNECTED` — **backoff creciente** (1.3s→3.0s→5.3s→10.3s) |
| 15039 | online restaurado |
| 15462 | **nueva conexión WS abierta (~420ms tras online)** |
| 15851 / 16040 | handshake engine.io (`0{sid...}`) + namespace connect (`40{sid...}`) |
| +15s | 1 socket exactamente abierto (`openSocketsNow=1`) → **SIN duplicación de suscripciones** |
| +15s | **712 frames recibidos** (~47/s) — flujo de datos restaurado al ritmo nominal |

- 4 intents durante offline = reconnect loop con backoff; cero tormenta de intents.
- La consola registró los fallos `ERR_INTEGRNET_DISCONNECTED` esperados y luego recuperó
  sin intervención. El DOM de "Runtime posture" refleja degradación honesta durante la caída.

## 4. Mixed-content — CERO

`browser_console_messages(all)` en 3 momentos del journey: **0 errores mixed-content,
0 warnings**. Todos los errores de consola (25) son: (a) los `ERR_INTERNET_DISCONNECTED`
inducidos por MI prueba offline, (b) `401` en `/api/admin/archive/status` (endpoint admin
sin token — correcto), (c) un `503` en `/api/route-discovery-outcomes/summary` (fail honesto).
Todo el tráfico es https/wss same-origin.

## 5. Observaciones R8 / defects documentados (nada inventado, nada maquillado)

1. **FEED-SCHEMA-01 (R8, bug real)** — "Live opportunity feed: Opportunity feed
   unavailable — **edge response shape invalid: items.0.block_number: Expected number,
   received string** (retrying every 30s)". El edge serializa `block_number` como string y
   el Zod del frontend espera number → el feed de oportunidades por REST está CAÍDO con
   estado honesto + retry 30s. La tabla igualmente muestra 239 filas vía otra vía.
   Screenshot: `ws-qa-home-feed-schema-error-block-number-string.png`.
2. **ROOM-AUTH-01** — frame entrante `42["error",{"code":"unauthorized","room":"runtime_ack"}]`
   (x2 en boot). El cliente intenta un room admin sin autorización; el server responde
   honesto. El DOM luego muestra `runtime_ack: LIVE` — discrepancia menor transporte↔UI.
3. **NO-WS-LOGS-01 (VPS)** — `docker logs arbitragex-v2-api-server-1 --tail 400 | grep -i 'ws|socket'`
   = **0 líneas**: api-server NO registra conexiones socket.io → imposible confirmar
   server-side la conexión desde logs (la conexión se confirmó por handshake 200 en :8080
   y frames reales). Gap de observabilidad.
4. **LOGFLOOD server-side** — los mismos logs están dominados por `paper_archiver.skip_rejected`
   a ~30 líneas/s (info level, razones R-0001 `spot_product_le_one`/`v3_quote_unavailable`/
   `non_positive_profit`) — patrón R9: el ruido impide ver eventos de conexión (causa
   directa de NO-WS-LOGS-01 no siendo visible).
5. **BOOT-DUP-01** — en boot se abren 4 conexiones WS en ~4s (2 sesiones engine.io en
   paralelo) y se observó el MISMO evento de telemetría entregado 2× (t=11160/11161ms,
   payload idéntico 199B) → doble subscripción transitoria durante el montaje. En estado
   estacionario queda exactamente 1 socket (verificado §3). Menor, pero real.
6. **TELEMETRY-RATE** — ~47 frames/s concentrados 99.8% en `route_discovery_telemetry`
   (per-ítem del scanner). No es flood letal (~9KB/s) pero es el único contenido del canal;
   las rooms de opportunities/pairs/quote_anchor no emiten (CONNECTING en el DOM). Cliente
   tolera; server emite per-ítem donde la doctrina R9 sugeriría agregación.

## 6. Estados vacíos honestos (R8) en el DOM — OK

- `socket: CONNECTING`, `pairs: CONNECTING`, `quote_anchor: CONNECTING` (REST snapshots
  pendientes), `routes: LIVE/DEGRADED` con fallback REST declarado.
- Tabla de oportunidades: 239 filas reales; celdas vacías declaran `no emitido`, `no target`,
  "No target applied (inverse-sizing not run for this route)" — fail-honest, no placeholders.
- Postura coherente con §34: `Live: OFF · Paper: ON · EXPLICIT · Capital: $0 · GO live: NO-GO
  · LIVE lock: REVIEW · KILL SWITCH OFF · PAPER MODE ON`.

## 7. Evidencia reproducible

- Screenshots (raíz del workspace):
  - `ws-qa-opportunities-live-stream.png` — /opportunities con badge LIVE y tabla 239 filas.
  - `ws-qa-home-feed-schema-error-block-number-string.png` — error FEED-SCHEMA-01.
- Comandos VPS (todos lectura): `nginx -T | grep socket.io` (líneas 222-223),
  `curl 127.0.0.1:{8787,8080,5173}/socket.io/?EIO=4&transport=polling`, `ss -tlnp`,
  `docker logs arbitragex-v2-api-server-1`, `docker exec arbitragex-v2-frontend-1 env`.
- Metodología de frames: Playwright `page.on('websocket')` → `framereceived` con muestreo
  de payload (sin curl al feed, como ordena el journey).

## 8. Sincronía de mesa redonda

- Construye sobre `02-VPS-REMAP-20260917.md` (flota 24/24 healthy, SHA a06a968d) — mis probes
  confirman esa salud en el canal WS.
- No contradice hallazgos previos; añade: nginx_patch.conf stale (→8787) convive con el
  config activo correcto (→8080) — riesgo de regresión si alguien "aplica" el patch viejo.
- Para pares de WO-02a/02c: el rate de `route_discovery_telemetry` 47/s del cliente es
  consecuencia directa del route_scanner.cycle per-ítem (multihop_negcycle) que fichan.
- Precedente citado: fix WS-POLL-1 (2026-08-20) sigue vivo y correcto.

## Clasificación de afirmaciones

- WS termina en api-server :8080, Edge 404 socket.io, nginx activo →8080: PRIMARY_SOURCE (nginx -T + curl).
- Ritmos de frames, backoff, no-duplicación: PRIMARY_SOURCE (instrumentación in-vivo reproducible).
- Cadena tunnel→nginx→8080 del dominio público: INFERRED (ingress del tunnel remotamente gestionado).
- FEED-SCHEMA-01: PRIMARY_SOURCE (DOM + screenshot + mensaje exacto).
- Causa raíz del string block_number (edge serializador): UNKNOWN desde el browser (requiere
  mirar código edge — quedará para el dueño de esa pieza).
