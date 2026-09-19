# WO-E4 · AG4 transport — polling-400: ¿dato o ruido? (VERIFY, solo diagnóstico)

Fecha: 2026-09-17 · Agente: ecc:performance-optimizer (Gang Omniscience) · CERO fix (demo congelada).
Claim files: `frontend/components/providers/ArbxRealtimeProvider.tsx`, `backend/api-server/src/websocket.ts` (solo lectura).
Presupuesto dominio: 1 navegación manual usada de 5 (https://arbx.ape-tv.net/opportunities, 17:16Z).

## VEREDICTO: **PASS con GAPS** (transporte sano; el "polling-400" del orquestador no se reprodujo y el ruido console real de la ventana es otro: 503 quote/anchor)

1. **El fallback polling NO degrada la frescura del dato — añade ~1 RTT de transporte, no pérdida.** PASS.
2. **Latencia evento→card: NO COMPUTADO (fail-honest R8)** — la página /opportunities renderizó 0 filas ("0 viable / 0 total") durante toda la ventana de medición; no hubo card que medir. Ver GAP-2.
3. **El 400 es ruido de establecimiento de transporte (upgrade WS rechazado en el tunnel), NO pérdida de frames.** INFERRED — ver clasificación y GAP-3.

---

## 1. Lectura de código (file:line, CANONICAL_REPO)

### Frontend — `ArbxRealtimeProvider.tsx`
- **:152-155** — única conexión socket.io: `io(getWsBaseUrl(), { transports: ["websocket", "polling"], auth: {...} })`. WebSocket primero, polling como fallback de engine.io.
- **:157-187** — `socket.on("connect")`: el canal `routes` se marca `transport:"ws", status:"live"` **incondicional al transporte real** (GAP-4: mislabel cosmético cuando engine.io quedó en polling). `connect` se dispara igual sobre polling — el flujo de datos no depende del transporte.
- **:200-210** — `route_discovery_telemetry` → `acceptTickPayload` (fail-closed RG-1): payload aceptado ⇒ `setTick` + `markFresh`. El reject de schema NUNCA llega al store (R8 honesto).
- **:122-138** — fallback REST de `routes` (30s, `REST_POLL_MS` :66) solo cuando `wsConnectedRef.current === false` (socket CAÍDO, no socket en polling). Re-chequea liveness tras los awaits (WO-GAP3) — sin flap LIVE↔DEGRADED.
- **:222-230** — `disconnect` real: routes→`polling` (REST), runtime_ack→`disconnected` (recorder pasivo sin fallback, honesto).

**Conclusión (1):** si el socket conecta por polling, el store recibe push con idéntico handler que por WS. La única degradación es la del transporte HTTP long-poll vs WS: **~1 RTT extra por batch de eventos**. El REST de 30s solo entra en desconexión total.

### Backend — `websocket.ts`
- **:351-363** — `new Server(server, {cors…})` montado **directamente sobre el HttpServer** (no `http-proxy-middleware`): el patrón R4 (upgrade binding de proxy Express) **NO APLICA** en api-server.
- **:400-408** (modificación local no desplegada, WO-NO-WS-LOGS-01) — loguearía `transport` verbatim. El desplegado `ba596c4a` NO lo tiene: por eso 0 líneas `ws.*` en 2h de logs (explicación, no hueco).
- **:13-118 (WO10)** — instrumentación de latencia detección→broadcast: origen `detected_at` (searcher) → término emit socket.io. Es la pierna server-side medida abajo.

### Ruta de red real (checklist R4 en el path del dominio)
- Host nginx (`docker/nginx-proxy.conf:16-24`) tiene el bloque /socket.io/ R4-CONFORME (`proxy_http_version 1.1` + `Upgrade`/`Connection` + `read_timeout 86400`).
- **PERO el host nginx NO está en el path del dominio**: `cloudflared` corre token-run (ingress remoto en CF dashboard); `access.log` del host registra solo ruido de internet (bots al IP pelado, 7 hits socket.io históricos). El dominio entra por el tunnel → ingress remoto. El upgrade WS muere ahí (ver §3).

## 2. Medición en vivo (17:15–17:18Z)

### Pierna server-side: detección → emit socket.io (PRIMARY_SOURCE, logs VPS read-only)
Ventanas WO10 de 60s (últimas 8, ~18.4K eventos, `skipped=0` — R8 limpio):

| p50 | p95 | p99 | max | n/min |
|---|---|---|---|---|
| 164–239 ms | 345–642 ms | 373–721 ms | 389–775 ms | 1959–2786 |

### Pierna browser: transporte y cadencia (PRIMARY_SOURCE, performance API + network trace)
- **Socket en polling puro vía tunnel**: 5 requests engine.io, todas `transport=polling` **200**, `sid=MBZryrhGESBSlBKJAABV` **estable** en toda la sesión. **Cero requests `transport=websocket`** — el upgrade WS nunca se completa a través del tunnel.
- **Long-poll quasi-push**: turnarounds de 212–927 ms; el poll siguiente arranca 3–7 ms después del anterior (8521→8528, 8983→8986). Con ~33–47 eventos/s, el ciclo se re-arma continuamente: la entrega es continua, no amortiguada por el `pingInterval`.
- **Coste extra vs WS ≈ 1 RTT de tunnel (orden 100–400 ms) por batch** — consistente con p95 server 350–640 ms + RTT browser.
- **Runtime Posture (el propio panel, honesto)**: `routes=LIVE`, `runtime_ack=LIVE` (canales entregando payloads aceptados sobre este socket); `pairs=CONNECTING`, `quote_anchor=CONNECTING`.

### Pierna card (evento→DOM): **NO COMPUTADO**
MutationObserver 12s sobre /opportunities: **0 mutaciones, 0 deltas de texto**. Motivo honesto: la página mostró "0 viable / 0 total — waiting for market topology" toda la ventana. El DOM-mutating del orquestador (74.6→83.2MB) fue en OTRA superficie (no /opportunities) — la memoria JS siguió creciendo por otros renders, no por filas de oportunidades. No inventé una cifra (RULE 00).

## 3. Clasificación del 400

**NO OBSERVADO en mi ventana**: 0 errores socket.io en 6 min de consola (36 errores, todos `/api/quote/anchor 503`). Clasificación del mecanismo (INFERRED con evidencia de estado):

- **Estado actual probado**: engine.io nunca upgradea a WS por el tunnel (0 requests websocket) y la sesión es estable y entregando. Es decir, el socket YA vive en el fallback final y los canales están LIVE.
- **400 del orquestador = rechazo del upgrade/handshake WS en el tunnel** (engine.io reintenta upgrade, tunnel responde 400/HTTP en vez de 101): ruido de establecimiento esperado, **sin pérdida de frames** — la prueba es la continuidad de `sid` y los payloads aceptados en la misma sesión que reporta LIVE.
- **Hipótesis alternativa descartada**: 400 por `sid` invalidado tras restart de api-server — `docker inspect`: `StartedAt=13:38:42Z, RestartCount=0`; no hubo restart en la ventana de la demo.
- **Hallazgo colateral (para quien lo reclame)**: el ruido console REAL y reproducible es `GET /api/quote/anchor?chain_id=1 → 503` en loop (~1–3 por ciclo de 30s = `REST_POLL_MS`), que deja `quote_anchor` en CONNECTING forever. Es un 503 honesto de upstream, NO transporte WS. Si el orquestador contó "10 errores console" en una ventana corta, lo más plausible es que fueran ESTOS 503, no polling-400 (misma firma visual en consola).

## GAPS

- **GAP-1 (medio)**: dominio sin upgrade WS por CF tunnel — todo el RT va por long-poll HTTP. No rompe frescura pero añade RTT y coste de conexión por poll. La corrección es config del ingress del tunnel (fuera de esta demo; §32 read-only).
- **GAP-2 (fuerte, derivar a WO-E2/E7)**: server broadcastea **~2.3K oportunidades/min** (WO10, room `opportunities`) mientras la página muestra **"0 viable / 0 total"** y 0 mutaciones DOM. ¿El hook page-local (`useOpportunitiesStream`, WS-POLL-1) está suscrito a la room correcta / su filtro traga todo? Esta es la desconexión que la demo debería explicar — el transporte NO es.
- **GAP-3 (medio)**: el 400 original no quedó capturado con URL/status exactos; mi clasificación es INFERRED del estado (sin upgrade + sid estable + LIVE). Si se reproduce, capturar la línea completa del network trace para cerrar como PRIMARY_SOURCE.
- **GAP-4 (cosmético)**: `ArbxRealtimeProvider.tsx:186` marca `transport:"ws"` aunque engine.io esté en polling — el panel Runtime Posture no distingue transporte real. Fix futuro: leer `socket.io.engine.transport.name` (NO tocado: demo congelada).
- **GAP-5 (info)**: `ws.connected` logging (WO-NO-WS-LOGS-01) existe local pero el desplegado `ba596c4a` no lo incluye → imposibilidad actual de correlacionar sesiones server-side.

## Pregunta de ataque a WO-E7

> Tu medición de latencia: ¿controló por el transporte real del socket? En esta ventana engine.io corre **100% polling vía tunnel** (0 upgrades WS), así que cualquier p50/p95 browser-side que hayas medido incluye ~1 RTT de tunnel que un WS nativo eliminaría. Y si mediste sobre /opportunities con "0 viable / 0 total": ¿qué timestamp comparaste — `detected_at` de un payload WS o el `last refresh` del snapshot REST (30s)? Ambas cosas cambian tu número por cientos de ms.

## Evidencia citable
- VPS (read-only): `docker logs arbitragex-v2-api-server-1` (WO10 x56 en 2h), `docker inspect` StartedAt/RestartCount, `docker/nginx-proxy.conf`, `cloudflared` token-run, `/var/log/nginx/access.log`.
- Browser: network trace engine.io (sid estable, polling 200, 0 websocket), console log 36×503 quote/anchor, Runtime Posture snapshot, performance entries.
- Deploy en VPS: `ba596c4a` (≠ HEAD local `6d034e54` —_gap esperado, cambios locales no desplegados por NO-GIT).
