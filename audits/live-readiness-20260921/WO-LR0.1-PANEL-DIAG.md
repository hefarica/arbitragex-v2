# WO-LR0.1 — Diagnóstico panel /live-readiness (READ-ONLY)

Programa: LIVE-READINESS 100% · Board: `audits/live-readiness-20260921/GOAL-WORKORDERS.md`
Run: Hermes `run_be00f935bb8841c1a6b70068a278acde` (re-despacho) · Fecha inspección: 2026-09-21 11:58–12:10Z
Modo: READ-ONLY total. Cero edits de código. Verificación pasiva del dominio (navegador headless propio, solo GET) + curls directos al VPS.

> Árbol auditado: workspace local `Desktop/arbitragex-v2-main (17)`. Los archivos citados
> fueron verificados idénticos al SHA desplegado en prod (`f0e94d91`, verificado con
> `git diff --stat f0e94d91 -- <archivos>`: sin diferencias) EXCEPTO donde se indica
> explícitamente que se cita la versión del SHA desplegado (`frontend/lib/api-client.ts`,
> `frontend/next.config.js` — el árbol local está en rama de trabajo `fix/ws-metrics-dead-room`
> de otro worker y AÚN NO incluye FE-EDGE-DIRECT-01; prod sí).

---

## 0. Resumen ejecutivo

| Elemento reportado | Veredicto |
|---|---|
| Header "kill-switch socket DISCONNECTED" | **Estado inicial honesto por diseño (R1/§34), NO bug de transporte**. `wsConnected:false` es el estado SSR/inicial del store; persiste solo hasta el primer `connect` (~1-2 s). Aparece "eterno" solo si la página se inspecciona en render inicial o con captura estática. |
| rooms routes/runtime_ack/pairs/quote_anchor "CONNECTING eterno" | **Estado inicial honesto** (`status:"connecting"` inicial, hint literal "mounted, awaiting the first accepted payload"). HOY, tras estabilizar: routes LIVE, pairs LIVE, runtime_ack ERROR (causa real A), quote_anchor ERROR (causa real B). |
| Ignición 0/4 | **NO reproducible hoy**: el endpoint server-side devuelve 4/4 PASS y el panel renderiza 4/4. El 0/4 solo puede provenir del placeholder fail-honest (fetch/validación fallando en ese momento) — mecanismos candidatos documentados en §4. |

**Dos defectos REALES (nuevos, no reportados en el charter) encontrados al inspeccionar hoy:**

- **[DEFECT-A] quote_anchor ERROR "Failed to fetch" — CORS**: el build desplegado (FE-EDGE-DIRECT-01, en `f0e94d91` y aún no en el árbol local) manda `GET /api/quote/anchor` y `GET /api/opportunities/live` DIRECTO a `https://edge-arbx.ape-tv.net` (cross-origin), pero el edge NO emite `Access-Control-Allow-Origin` para el origen `https://arbx.ape-tv.net`: `wrangler.toml:16` tiene `ALLOWED_ORIGINS = "http://localhost:3000"` y la línea 59 con los orígenes de prod está COMENTADA. El navegador bloquea la lectura → TypeError "Failed to fetch" → chip ERROR (fail-honest, el error es real).
- **[DEFECT-B] runtime_ack ERROR "join rejected: unauthorized"**: room admin-gated; un navegador sin token admin es rechazado por diseño (ROOM-AUTH-01). Es fail-honest correcto, pero hace que CUALQUIER visitante sin sesión admin vea ERROR permanente en esa room y el chip socket degradado.

---

## 1. Elemento: header "kill-switch socket DISCONNECTED"

### 1.1 Endpoint/código que lo alimenta

- Chip "socket": `frontend/components/RuntimePostureBar.tsx:423` → `socketChipProps(wsConnected, channels)` (:103-133). **Si `wsConnected===false` → state DISCONNECTED** (:107-111), detail "the single socket.io connection is down".
- `wsConnected` inicial: `frontend/lib/store/realtime-slices.ts:101` (`wsConnected: false` en la creación del slice). Es el estado SSR + primer render cliente (R1, comment :19-24 de RuntimePostureBar).
- Único escritor de `true`: `socket.on("connect")` en `frontend/components/providers/ArbxRealtimeProvider.tsx:157-160` (montado una sola vez en el root layout, `frontend/app/layout.tsx:145`; el bar en :125).
- URL del socket: `getWsBaseUrl()` (`frontend/lib/api-client.ts:67-74`) — en navegador SIEMPRE same-origin (`wss://arbx.ape-tv.net`), servido por el rewrite `/socket.io/:path* → http://api-server:8080` (`frontend/next.config.js:128-137`; proxy HTTP-only, upgrade websocket por el mismo path — RULE 02, WS nunca via Edge).
- El "kill-switch" del mismo header es otro chip (posture): `getStatus()` → `GET /api/status` (`api-client.ts:273-279`) → edge `index.ts:591-594` → api-server `backend/api-server/src/index.ts:228-251` (`killswitch: ks`, fuente `killSwitch.state()`).

### 1.2 Estado server-side REAL (2026-09-21 ~11:58Z)

- Handshake socket.io 200 con SID en los tres saltos:
  - `curl http://127.0.0.1:5173/socket.io/?EIO=4&transport=polling` → `0{"sid":"-pXR_5I9UnBJRxIbAAAt","upgrades":["websocket"],...}` [HTTP 200]
  - `curl http://127.0.0.1:8080/socket.io/...` → `0{"sid":"H3DFBJzgQIXhzSE6AAAs",...}` [HTTP 200]
  - por el edge (:8787) → `{"error":"not_found"}` [HTTP 404] — correcto: RULE 02, no hay WS por Edge.
- `GET /api/status` (edge 8787 y api 8080) → HTTP 200: `"killswitch":{"enabled":false,"reason":"VER","updated_at":"2026-09-20T23:56:33.726Z"}`, todos los servicios ok, deploy `f0e94d91…` (2026-09-21T05:42:57Z).
- `docker logs arbitragex-v2-api-server-1 --since 1h`: conexiones WS entrantes continuas (`ws.connected` transport websocket/polling) y mi sesión headless (`n-oPpQ-EfVHRF-NYAAAp`, 12:01:38Z) sigue viva >10 min sin disconnect.
- Contenedores: `docker ps` — frontend/edge/api-server healthy, Up 6 hours.

### 1.3 Causa raíz del DISCONNECTED

**Estado inicial honesto, no bug.** El vocabulario §34 reserva DISCONNECTED para "pre-connect, teardown, or a passive channel whose only transport is down" (`RuntimePostureBar.tsx:201-204`). Antes del primer `connect` (~1-2 s tras hydrate) el chip ES DISCONNECTED con razón. Un navegador real estabilizado no lo muestra: verificado en headless (load 12:01Z, lectura 12:04Z): chip socket = **ERROR** (no DISCONNECTED) porque wsConnected=true y la degradación viene de los canales (§2). NOTA: sesiones previas del dashboard muestran `disconnect reason="ping timeout"` recurrentes (11:46Z: tres sockets) — el socket.io client auto-reconecta, y durante esas ventanas (~20 s) el chip vuelve a DISCONNECTED legítimamente.

### 1.4 Accionable

- Nada que arreglar en el transporte del socket para DISCONNECTED per se. WO-LR4.1 no debe "parchar" el estado inicial (es doctrina R1/§34).
- [WO-LR4.1, opcional UX] El título del chip socket dice "the single socket.io connection is down" — cuando el socket está UP pero un canal falla, el chip hereda ERROR/CONNECTING del peor canal (:113-132). Hoy el mensaje es correcto. Sin acción.

---

## 2. Elemento: rooms CONNECTING (routes/runtime_ack/pairs/quote_anchor)

### 2.1 Código

- Proyección: `projectChannel` (`RuntimePostureBar.tsx:78-90`): `status==="connecting"` → CONNECTING (:84).
- Estado inicial: `blank()` = `{transport:"rest", status:"connecting"}` para las 4 rooms (`realtime-slices.ts:91-98`).
- Transiciones (todas en `ArbxRealtimeProvider.tsx`): REST pass inmediato (:140) → `applyRestOutcome`/`markFresh` marcan `live` (:81-99, fix WO-GAP3 2026-09-17 "refreshing lastMessageAt alone left REST-native channels connecting forever"); `socket.on("connect")` marca routes `ws/live` (:186); fallos escriben `lastError` → ERROR.

### 2.2 Estado REAL tras estabilizar (headless, ~40 s y ~8 min tras load)

Bar (texto literal): `KILL SWITCH OFF | PAPER MODE ON | socket ERROR | routes LIVE | runtime_ack ERROR | pairs LIVE | quote_anchor ERROR`

Titles (lastError literales):
- routes: "transport delivering accepted payloads" (WS room entregando).
- pairs: LIVE (REST `/api/pairs?chain_id=1` → 200, entries reales).
- **runtime_ack: "last delivery attempt failed … — runtime_ack join rejected: unauthorized (room is admin-gated)"**.
- **quote_anchor: "last delivery attempt failed … — Failed to fetch"**.

Endpoints server-side (curl VPS, todo 200 con datos reales): `/api/pairs?chain_id=1` ✓, `/api/quote/anchor?chain_id=1` ✓ (quote_version 696, scores reales), `/api/route-discovery/tick?chain_id=1` ✓ (tick vivo, 100000 edge visits). Logs api-server: `ws.subscribe_denied room=runtime_ack code=unauthorized "socket lacks admin capability flag"` (11:02, 11:11, 11:14, 12:01 — mi sesión incluida).

Resource timing del navegador (performance API): TODAS las llamadas a `https://edge-arbx.ape-tv.net/api/opportunities/live` y `/api/quote/anchor` → **status 0** (falladas), repetidas cada ciclo de 30 s; TODAS las same-origin a `arbx.ape-tv.net/api/*` → 200. Fetch manual same-origin de `/api/quote/anchor?chain_id=1` DESDE la página: 200 con payload fresco — el endpoint está sano; falla solo la variante cross-origin.

### 2.3 Causa raíz

- **quote_anchor**: FE-EDGE-DIRECT-01 (presente SOLO en prod, `f0e94d91:frontend/lib/api-client.ts` — `getPublicEdgeBaseUrl()` + `getQuoteAnchor(..., {baseUrl: getPublicEdgeBaseUrl()})`; también `getOpportunitiesLive` y `getTradingConfig`). El edge público responde 200 (verificado por curl desde el VPS y desde fuera: HTTP/2 200, `x-arbx-latency-ms: 4`) PERO sin cabecera ACAO: el middleware CORS (`edge/worker/src/index.ts:313-328`) hace `allowed = ALLOWED_ORIGINS.split(",").includes(origin)` y en el VPS `/opt/arbitragex-v2/edge/worker/wrangler.toml:16` = `"http://localhost:3000"`; la línea 59 `# ALLOWED_ORIGINS = "https://arbx.ape-tv.net,https://www.arbx.ape-tv.net"` está comentada. El comentario de FE-EDGE-DIRECT-01 decía "full CORS for the app origin (verified live)" — ya no es cierto en la config desplegada.
- **runtime_ack**: room admin-gated (ROOM-AUTH-01); navegador sin `arbx_admin_session`/token → join rechazado → lastError → ERROR. Es fail-honest BY DESIGN ("passive recorder with no REST fallback by design", `RuntimePostureBar.tsx:219`).
- El "CONNECTING eterno" reportado = estado inicial + (probablemente) estos dos errores vistos sin esperar el primer ciclo REST/WS, o captura antes de los ~30 s del primer pass.

### 2.4 Accionable

- **[OPERADOR/VPS-OPS, fix mínimo sin rebuild]** descomentar/ajustar `ALLOWED_ORIGINS` en `wrangler.toml` para incluir `https://arbx.ape-tv.net` y redeploy del worker (config Cloudflare, aprobación operador). 
- **[WO-LR4.1, alternativa código]** revertir las 3 llamadas edge-direct a same-origin (quitar `baseUrl: getPublicEdgeBaseUrl()`), requiere rebuild frontend (RULE 03). Ambas opciones cierran quote_anchor; elegir una (recomendada la config: no toca código).
- **runtime_ack**: no es defecto. [WO-LR4.1 opcional] chip distintivo "requires admin session" para no leerse como fallo de infra. El operador con sesión admin lo ve LIVE.
- [Monitor] `disconnect reason="ping timeout"` se observa en sesiones ~15 min: socket.io reconecta solo; si el operador reporta flapping, investigar keepalive/idle de cloudflared. No urgente.

---

## 3. Elemento: Secuencia de ignición 0/4

### 3.1 Endpoint/código que lo alimenta

- Componente: `frontend/components/ReadinessStepper.tsx:56-97` (progress `N/4` en :57, título "Secuencia de Ignición Operativa" :63). Montado en `frontend/app/live-readiness/page.tsx:302` (`<LiveReadinessStepper>`).
- Hook: `frontend/hooks/useSystemReadiness.ts` — poll 20 s (:62) a **`GET /api/readiness/steps`** vía `getReadinessSteps()` (`frontend/lib/api-client.ts:366`). **OJO: este 6º endpoint no estaba en la lista del charter (los 5 REST alimentan los paneles de abajo; el stepper tiene el suyo).**
- Placeholder fail-honest: `placeholderSteps()` (:131-146) = 4 pasos, status pending/blocked, evidence "—", `completedCount=0` → **UI 0/4 mientras el primer fetch está en vuelo o si falla** (refresh :160-174: en error mantiene placeholder/último bueno y expone `readiness.error`, que el stepper pinta "Verificador de readiness no disponible: …" — `ReadinessStepper.tsx:77-81`).
- Servidor: `backend/api-server/src/routes/readiness-steps.ts` — ruta `GET /api/v1/readiness/steps` (:518-533), evaluadores puros: topology (:151-179, PASS si ≥1 WSS Y ≥1 RPC activos), credentials (:187-209, PASS si signer O private_key presente; vars :115-127), markets (:219-251, PASS si chains≥1 Y dexes≥1 Y pools≥1), engines (:260-288, PASS si ≥1 estrategia habilitada), cascada BLOCKED (:296-325). Edge alias: `edge/worker/src/index.ts:1415` (cache 15 s).

### 3.2 Estado server-side REAL (curls 11:58Z y 12:04Z, edge y api directos)

`completed_steps: 4, readiness_score: "4/4", all_ready: true`:
1. topology PASS — "4 active WSS + 5 active RPC provider(s) in topology vault (version 1786916896)" (drpc, publicnode, 0xrpc, tenderly WSS; alchemy/drpc/publicnode/0xrpc/cloudflare-eth RPC).
2. credentials PASS — `signer: "present"` (redacted), private_key/relay null.
3. markets PASS — "6 chain(s), 23 dex(es), 1750 pool(s), 4803 token(s)".
4. engines PASS — "8 resolution engine(s) enabled: dex_arb, dex_arb_v2v2, dex_arb_v2v3, dex_arb_v3v2, dex_arb_v3v3, flashloan_arb, liquidation, triangular. paper=true shadow=true, live disabled."

El panel headless renderiza **4/4 (Paso 1-4 "Completado")** y cero errores de verificador.

### 3.3 Causa raíz del 0/4 reportado

El estado del backend NO puede producir 0/4 (todo PASS). El 0/4 del charter solo puede ser el **placeholder fail-honest**, es decir `getReadinessSteps()` fallando en ese momento en el navegador. Candidatos, sin poder reproducir hoy (ordenado por plausibilidad):

1. **Ventana de fallo/degradación real**: durante ~5-6 s (~12:05:35Z en mi sesión) TODAS las llamadas same-origin fallaron en bloque (status 0, duraciones ~5 s = timeout+retries del cliente) — burst de latencia/429 del stack (paper-mode 11 s, strategies 5,6 s). Si el operador cargó la página en una ventana así, stepper=0/4 con banner de error. Los `ping timeout` masivos de 11:46Z muestran que tales ventanas existen.
2. **Deriva de esquema Zod** (menos probable): `getValidated` usa Zod estricto; un drift de shape del endpoint dejaría el stepper en 0/4 permanente — hoy valida OK, descartado en el build vigente.
3. **Bundle antiguo** (si la observación fue anterior al deploy 09-21 05:42Z): no verificado, sin evidencia.

Si reaparece: capturar el texto de "Verificador de readiness no disponible" (lleva el error literal) y/o `document.querySelector('[role=...]')` + DevTools Network del fetch `/api/readiness/steps`.

### 3.4 Accionable

- **Paso 2 (signer) NO es [OPERADOR]: ya está presente** server-side (env `SIM_SIGNER_ADDRESS`/`SIGNER_ADDRESS` — `readiness-steps.ts:115`). NADA que hacer, y jamás crear keys.
- Los 4 pasos están PASS: no hay accionable de contenido para WO-LR4.1 en la ignición HOY. La acción es la de §2.4 (CORS) para que los chips del header no ensucien la lectura del panel, y vigilancia del burst de latencia (ítem 1) si se repite.

---

## 4. Hallazgos transversales

1. **[DEFECT-A] CORS edge-direct** (detalle en §2.3): afecta `/api/quote/anchor`, `/api/opportunities/live`, `/api/trading-config` en el build f0e94d91. En el árbol local YA no existe (branch de trabajo lo revirtió) — al mergear/main siguiente hay que decidir: o se sube con el fix de config, o se revierte FE-EDGE-DIRECT-01 definitivamente.
2. **[MENOR] `GET /api/go-no-go/status` sirve `generated_at: "2026-09-07T00:20:06.186Z"`** (2 semanas) mientras `/api/readiness/blockers` en la misma página sirve 12:08Z del mismo día. Es un campo semántico congelado de la vista derivada (misma marca la contenía el fixture prod 2026-08-24), no caché (x-arbx-cache MISS/`cf DYNAMIC`). No bloquea NO_GO (que depende de A.9), pero si el operador quiere frescura visible, asignarlo en el WO de go-no-go.
3. **Latencia p95 del rewrite same-origin**: `dur ~950-1250 ms` por llamada API en el headless (doble hop CF→cloudflared→Next→edge→api). Rango normal para este despliegue; mencionado porque multiplica el riesgo de ventanas tipo ítem 1 de §3.3.
4. **Gotcha charter "quote_anchor 503 pese a Redis EXISTS=1"**: HOY devuelve 200 en ambos orígenes server-side (api y edge, con datos frescos quote_version 696). No reproducido en esta ventana; queda documentado como intermitente del endpoint (no de infra).
5. **Símbolo "—" en cards**: NO apareció en /live-readiness durante la inspección (el placeholder del stepper usa "—" por diseño). Root cause 09-20 (filas PG sin metadata) sigue pendiente de decisión A/B — no tocado, como ordena el charter.

## 5. Gate de cierre — veredicto explícito

**¿El DISCONNECTED es bug real de transporte o estado inicial honesto?** → **Estado inicial honesto** (R1/§34; `wsConnected:false` SSR + hasta el primer `connect`). Evidencia a favor: handshake socket.io 200 con SID en :5173 y :8080; `ws.connected` continuo en logs; sesión headless propia conectada por websocket (>10 min, sin disconnect); chip socket en ERROR (no DISCONNECTED) una vez hidratado. Lo que SÍ son defectos reales hoy: DEFECT-A (CORS edge-direct → quote_anchor ERROR) y DEFECT-B (runtime_ack admin-gated leído como error por visitantes sin sesión — diseño, con matriz de comunicación mejorable).

**Sin un solo archivo editado** (excepto este entregable y la fila del BOARD). Sin POST, sin toggles, sin deploy, sin broadcast/wallets/capital.

## 6. Comandos reproducibles

```bash
# Contenedores y salud
ssh arbx 'docker ps --format "{{.Names}} | {{.Status}} | {{.Ports}}" | grep -i arbitragex'
# Endpoints implicados (edge y api)
ssh arbx 'for p in "/api/readiness/steps" "/api/status" "/api/paper-mode/state" "/api/pairs?chain_id=1" "/api/quote/anchor?chain_id=1" "/api/route-discovery/tick?chain_id=1"; do curl -s -m 8 -w "\n[HTTP %{http_code}]" "http://127.0.0.1:8787$p" | head -c 400; echo; done'
ssh arbx 'curl -s -m 8 -w "\n[HTTP %{http_code}]" "http://127.0.0.1:8080/api/v1/readiness/steps" | head -c 600'
# Handshake socket.io (los tres saltos)
ssh arbx 'curl -s "http://127.0.0.1:5173/socket.io/?EIO=4&transport=polling"; curl -s "http://127.0.0.1:8080/socket.io/?EIO=4&transport=polling"; curl -s "http://127.0.0.1:8787/socket.io/?EIO=4&transport=polling"'
# Rooms WS reales
ssh arbx 'docker logs arbitragex-v2-api-server-1 --since 1h 2>&1 | grep -E "ws.connected|ws.subscribe|ws.disconnected" | tail -20'
# CORS del edge público (sin ACAO = lectura bloqueada en navegador)
curl -s -D - -o /dev/null -H "Origin: https://arbx.ape-tv.net" "https://edge-arbx.ape-tv.net/api/quote/anchor?chain_id=1" | grep -iE "^(HTTP|access-control)"
ssh arbx 'sed -n "16p;59p" /opt/arbitragex-v2/edge/worker/wrangler.toml'
# Navegador pasivo (headless propio):
#   chrome --headless=new --remote-debugging-port=9223 ... ; abrir https://arbx.ape-tv.net/live-readiness
#   document.querySelector('[role="status"][aria-label="Runtime posture"]').innerText  → chips
#   performance.getEntriesByType("resource") → edge-arbx... status 0 vs arbx... 200
```

Evidencia browser persistida: `~/.hermes/.../run_be00f935bb8841c1a6b70068a278acde/estado-panel-1210Z.json`
(ruta completa: `C:\Users\HFRC\AppData\Local\hermes\profiles\ccr-glm53\cache\browser-use\workspace\run_be00f935bb8841c1a6b70068a278acde\estado-panel-1210Z.json`).
