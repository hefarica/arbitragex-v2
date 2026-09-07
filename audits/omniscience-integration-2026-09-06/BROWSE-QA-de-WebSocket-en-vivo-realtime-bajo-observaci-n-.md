# BROWSE QA — WebSocket en vivo (realtime bajo observación) · 2026-09-07

**Persona:** QA humano de la DApp ArbitrageX (Gang Omniscience).  
**Página bajo observación:** `https://arbx.ape-tv.net/opportunities` ("Live Network value Feed").  
**Ventana de observación:** ciclo 1 = 05:39:34Z → 05:48:29Z (~9 min sin recargar); ciclo 2 post-F5 = 05:48:57Z → 05:51:07Z (~2.2 min).  
**Método:** página dedicada en navegador aislado (instancia chrome-devtools, pageId 2) tras detectar que la pestaña Playwright inicial era compartida con otro agente del gang (fue navegada a `/operations` y `/live-readiness` a mitad del journey; su consola preservada se usa SOLO como evidencia del handshake wss, no del timeline).  
**Presupuesto HTTP manual:** 1/5 usado (`curl /api/status` → 200, 0.43 s). Cero barridos. Todo lo demás es evidencia del navegador (Network/Resource Timing/consola/DOM).

---

## VEREDICTO (pregunta canónica del journey)

> **¿WS real, polling degradado o congelado? → CONGELADO.**

La DApp viva por el dominio público **no entrega WebSocket real ni un polling funcional para el feed**: entrega un snapshot SSR fresco al cargar que **se congela inmediatamente**. El transporte socket.io muere en silencio tras 5 peticiones de polling en los primeros ~1.6 s (0 peticiones nuevas en 9 min, pese a `pingInterval=25000` declarado por el propio handshake). El upgrade `wss://` por el dominio falla con **502** (evidencia de consola, repetida). El único "realtime" que sigue vivo es el REST poll de postura (~30 s) que **no alimenta el feed de oportunidades**: la API devuelve filas frescas (`detected_at` 05:43:44/05:44:12) que el DOM nunca aplica. Solo F5 trae el dato vivo (delta staleness ≈ 8–9 min). Degradación **D-11 confirmada con evidencia de navegador**.

---

## Timeline minuto a minuto (UTC; hora local del UI = UTC-5)

### Fase 0 — instancia Playwright compartida (solo evidencia de red/consola)

| t (UTC) | Observación |
|---|---|
| 05:36:07 | Navegación a `/opportunities`. Banner: "Live opportunity stream status: **LIVE**". Postura: socket **CONNECTING** ("mounted, awaiting the first accepted payload — pairs=CONNECTING, quote_anchor=CONNECTING"), routes LIVE, runtime_ack LIVE, pairs CONNECTING, quote_anchor CONNECTING. Feed: "Live stream via WebSocket · Last refresh: 12:36:16 a. m." · 0 viable / 50 total. Primera tarjeta 00:35:07 (66s, stale). |
| 05:36–05:38 | Network: socket.io **solo `transport=polling` contra el dominio** (reqs 41/50/51/59/60, sid `wTSIB228RzpnyYmYAAAF`). Cero `transport=websocket`, cero tráfico a `:8080`. Consola (preservada, mismo bundle SPA): **8+ veces** `WebSocket connection to 'wss://arbx.ape-tv.net/socket.io/?EIO=4&transport=websocket' failed: Error during WebSocket handshake: Unexpected response code: 502`. Además POST polling con sid antiguo → 400, y tormenta 500/502 puntual (~05:38) sobre APIs en ESA instancia (no reproducida en la dedicada — atribuible a carga concurrente del gang). |
| 05:38:11 | La pestaña compartida se recarga sola (uptime reset a 2.2 s; sid nuevo). Poco después otro agente la navega a `/operations` → la abandono y abro página dedicada. |

### Ciclo 1 — página dedicada, sin recargar (05:39:34 → 05:48:29)

| t (UTC) | uptime | Observación |
|---|---|---|
| 05:39:34 | 0.0s | Carga. Handshake socket.io → **200**, body: `0{"sid":"OFujUTXTnEVdDKECAAAK","upgrades":["websocket"],"pingInterval":25000,"pingTimeout":20000,"maxPayload":1000000}` — el origen **ofrece** upgrade websocket. Le siguen 4 requests polling más (última a t=1608 ms). **Después: cero actividad socket.io en lo que queda del ciclo.** |
| 05:39:59 | +25s | Last refresh **12:39:36** (≈ −23 s, fresco). Total **107** (107 rejected). Primera tarjeta 00:39:33 local = 05:39:33Z, "1s VIGENTE" (WFC→WETH, dex_arb, UniswapV3→UniswapV3). Chips: socket CONNECTING · routes **DEGRADED** · runtime_ack LIVE · pairs CONNECTING · quote_anchor CONNECTING. Banner: LIVE. |
| 05:41:41 | +127s | **Last refresh CONGELADO en 12:39:36** (2+ min). Total 200 (cambio único temprano). socketIoCount sigue 5. |
| 05:43:01 | +207s | Igual congelado. Edades de tarjetas estáticas (112 s ×4, 71 s, 1 s — re-render puntual, sin datos nuevos). |
| 05:44:30 | +296s | **Tarjeta más nueva SIGUE 00:39:33** (misma que en t=0, ~5 min vieja). Edades 213 s ×4. Total 200. |
| 05:45:37 | — | (wire) REST `GET /api/opportunities/live?limit=20` → **200** con items `detected_at: 2026-09-07T05:44:12.615Z` y `05:43:44.055Z` (¡server con datos frescos!), `x-arbx-latency-ms: 18`, `x-arbx-cache: MISS`. El DOM no aplica nada de esto. |
| 05:39–05:48 | — | (wire) REST poll de postura ininterrumpido cada ~30 s, todo 200: `paper-mode/state`, `strategies/runtime-status`, `readiness`, `readiness/decision`, `scanner/heartbeat`, `opportunities/live?limit=20`, `pairs`, `quote/anchor`. Total 384 requests del ciclo: **5 de socket.io (1.3%)** vs 379 REST (98.7%). |
| 05:48:29 | +535s | El subtítulo del feed **CAMBIA** a "**Fallback: polling edge every 4s**" — pero Last refresh **SIGUE 12:39:36** (9 min congelado), la tarjeta más nueva **SIGUE 00:39:33**, y en el wire **NO existe cadencia de 4 s** (`opportunities/live` va cada ~30 s). Anuncio no respaldado por el tráfico. Total 170. |

### F5 único (05:48:57) y ciclo 2

| t (UTC) | uptime | Observación |
|---|---|---|
| 05:48:57 | — | Reload. Navigation timeout del harness a los 10 s (SSR lento bajo carga concurrente); la página termina cargando. |
| 05:49:46 | +52s | **Datos VIVOS**: Last refresh **12:48:57**; tarjeta más nueva **00:47:20** (05:47:20Z); banner **LIVE** de nuevo (WO-08); total 50 (límite SSR); socketIoCount **5 otra vez** (handshake nuevo, mismo patrón). |
| 05:51:07 | +133s | **Se congela otra vez**: Last refresh sigue 12:48:57 (2.2 min), tarjeta 00:47:20 fija, total 106 (cambio único temprano), socket.io 5. **Patrón 100% reproducible entre ciclos.** |

### Diferencia stale vs vivo (pregunta 4 del journey)

| Campo | Pantalla antes del F5 | Tras el F5 | Delta |
|---|---|---|---|
| Tarjeta más nueva visible | 00:39:33 local (05:39:33Z) | 00:47:20 local (05:47:20Z) | **7 m 47 s de atraso** |
| "Last refresh" | 12:39:36 (05:39:36Z) | 12:48:57 (05:48:57Z) | **9 m 21 s de atraso** |

La pantalla era **stale**; solo el F5 trajo el dato vivo. El dato existía en el backend desde minutos antes (REST 200 con `detected_at` 05:43:44/05:44:12 observado a las 05:45:37).

---

## Respuestas a las 4 preguntas del journey

1. **¿El feed avanza solo?** NO. El feed principal ("Live Network value Feed") muestra el snapshot SSR del momento de carga + un único refetch temprano (contador total cambia una vez: 107→200 y 50→106) y después queda congelado: ni "Last refresh" ni la tarjeta más nueva ni las filas avanzan (9 min observados en ciclo 1; congelamiento re-confirmado en ciclo 2). **No existe intervalo de actualización del feed** — el único ritmo real es el REST de postura cada ~30 s, que no toca el feed.
2. **Chip/badge socket:** Banner superior: **"LIVE" TODO el tiempo** (ambos ciclos, desde t=0 con subsistemas en CONNECTING) — **bug conocido WO-08, fix local no desplegado; documentado como evidencia, no hallazgo nuevo**. Chips de postura (sí cambian en el tiempo): `socket=CONNECTING` permanente ("mounted, awaiting the first accepted payload"), `pairs=CONNECTING` y `quote_anchor=CONNECTING` permanentes, `routes` LIVE→**DEGRADED** (cambió durante la sesión), `runtime_ack=LIVE`. Además el subtítulo del feed muta de "Live stream via WebSocket" → "Fallback: polling edge every 4s" a los ~9 min — cambio de claim sin cambio de wire.
3. **DevTools Network/WS:** **No hay conexión socket.io websocket activa.** Solo 5 XHR `transport=polling` same-origin contra `arbx.ape-tv.net` en los primeros 1.6 s de cada ciclo, y luego silencio absoluto (ni ping/poll ciclando cada 25 s como declara el handshake, ni reintentos de reconexión en 9 min). El upgrade falla explícitamente: `wss://arbx.ape-tv.net/socket.io/?EIO=4&transport=websocket` → **"Error during WebSocket handshake: Unexpected response code: 502"** (consola, repetido). Cero tráfico a `:8080`. **Evidencia de navegador de D-11**: la ruta por el dominio (CF tunnel → Next) sirve `/socket.io/*` vía rewrite HTTP-only que no hace upgrade WS; el handshake dice `"upgrades":["websocket"]` pero la cadena del dominio lo mata con 502 — y de paso el transporte polling queda colgado sin ciclar.
4. **F5 único:** el dato tras recargar **difiere radicalmente** del que quedó en pantalla (tabla arriba): la pantalla congelada estaba 8–9 min por detrás del dato vivo. Solo F5 avanza el feed.

---

## Evidencia de código (por qué el dominio se comporta así)

- `frontend/next.config.js:83-88` — WS-POLL-1: `/socket.io` debe terminar en el gateway WS del api-server (RULE 02: WS directo, nunca vía edge).
- `frontend/next.config.js:123-132` — el propio comentario lo admite: *"Next rewrites are HTTP-only: socket.io will run its POLLING TRANSPORT through this proxy (a LIVE connection); **the true websocket upgrade needs the nginx path** (nginx /socket.io/ → api-server:8080, fixed on the VPS 2026-08-20)."* Por la ruta del dominio público el upgrade NO existe (502 observado). Además, en la práctica ni siquiera el polling "LIVE" prometido cicla (5 requests y silencio).
- `frontend/next.config.js:98-105` — `skipTrailingSlashRedirect` para que el rewrite vea `/socket.io/?EIO=4...`.
- HTML servido (verificado desde el DOM, 05:52Z): **no contiene `:8080`** → el cliente socket.io deployado es same-origin contra el dominio, no directo al api-server como manda RULE 02.

## Veredicto RULE 00 / R8 (datos reales, estados honestos)

- **Datos reales (RULE 00 OK):** las tarjetas son detecciones verdaderas (XEN `TokenNotAllowed:0x0645…6fb8`, WFC→WETH `single_pool_no_spread`, WETH verificado score 75 vía uniswap), con `detected_at`/`trace_id` reales, `simulated_cost_breakdown` real (ops_overhead 0.5977) y el API respondiendo 200/18 ms con rows frescas. Nada fabricado.
- **Estados honestos (R8) MIXTOS:** lo honesto — "—" en roi_pct/in USD cuando no computado, contador "0 viable / N total (N rejected)", chip `routes=DEGRADED`, subtítulo que eventualmente anuncia fallback. Lo deshonesto — banner **"LIVE"** permanente con el transporte muerto (WO-08 conocido) y el claim "**polling edge every 4s**" que el wire desmiente (no hay tal cadencia ni el feed se refresca).

## Screenshots (en esta carpeta)

| Archivo | Momento | Qué muestra |
|---|---|---|
| `qa-ws-t0-opportunities-2026-09-07.png` | 05:36Z instancia inicial | Feed + banner LIVE + postura con socket CONNECTING |
| `qa-ws-t0-dedicated-posture-feed-2026-09-07.png` | 05:40Z t=0 dedicado | Chips socket CONNECTING / routes DEGRADED; feed fresco 12:39:36; tarjeta 1s VIGENTE |
| `qa-ws-t3min-frozen-refresh-2026-09-07.png` | 05:43-44Z (+3-5 min) | Last refresh congelado 12:39:36; total 200; edades estáticas |
| `qa-ws-post-f5-fresh-feed-2026-09-07.png` | 05:49Z post-F5 | Dato vivo: Last refresh 12:48:57, tarjeta 00:47:20, banner LIVE de nuevo (WO-08) |

## Gaps / remediación (no ejecutada — solo lectura, NO-GIT)

1. **D-11 (operador-gated):** habilitar upgrade WS por la ruta del dominio (path nginx/cloudflared con upgrade a api-server:8080, o apuntar `NEXT_PUBLIC_WS_URL` horneada al host WS correcto) + redeploy. Sin eso, todo "Live stream via WebSocket" por el dominio es falso.
2. **WO-08 (operador-gated):** el fix local del badge honesto sigue sin desplegarse — el banner "LIVE" miente en producción (evidencia arriba).
3. **Fallback "polling every 4s" (agent-fixable):** el claim no está implementado en el wire ni refresca el feed; o se implementa o se retira el texto.
4. **Motor socket.io colgado (agent-fixable):** tras las 5 peticiones iniciales no hay ciclo de ping/poll (25 s declarados) ni reconexión en 9 min — el cliente queda zombie en CONNECTING; falta timeout/reconnect efectivo del lado cliente.
5. **Feed no aplica REST fresco (agent-fixable):** `opportunities/live` responde 200 con rows nuevas cada ~30 s y el DOM jamás las aplica — si el feed tuviera fallback REST real, el usuario vería datos con ≤30 s de lag sin WS.

**Conclusión final:** la DApp viva entrega un **feed congelado** tras un snapshot inicial fresco: ni WS (502 en upgrade por rewrite HTTP-only — D-11), ni polling socket.io vivo (motor colgado tras 1.6 s), ni fallback REST aplicado al feed (anunciado a 4 s, inexistente en el wire). El badge "LIVE" del banner es la única cosa "realtime" de la página — y es el bug conocido WO-08.
