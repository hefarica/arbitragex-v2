# N3 — api-server WS 8080 (local vs VPS) — REPORTE FINAL

- **Agente:** verificador N3 "api-ws" (round-table integración DApp ArbitrageX)
- **Superficie:** api-server WebSocket puerto 8080 — código local vs contenedor `arbitragex-v2-api-server-1`
- **Estado:** COMPLETADO
- **Veredicto global:** **DEGRADED** — la capa de conexión WS está íntegra y desplegada con SHA veraz, pero el broadcast insignia de oportunidades está en silencio por DOS gaps de wiring, y el transporte WS público del dominio corre degradado a HTTP-polling.
- **Reglas respetadas:** solo lectura (ssh read-only, docker ps/logs/inspect, psql SELECT, redis XLEN/XINFO, curl interno). NO abrí WebSockets (charter). Escritura únicamente en este archivo.

---

## 0. Respuesta directa al charter: ¿hay clientes WS vivos ahora?

**SÍ — exactamente 1 sesión socket.io viva**, y 1 conexión TCP adicional que NO es WS (Prometheus).

Evidencia (VPS, 2026-09-06 ~23:21–23:28Z):

```
$ docker logs -t arbitragex-v2-api-server-1 2>&1 | grep -E '\[WebSocket\]'
2026-09-06T23:02:29.841Z [WebSocket] Nuevo cliente conectado: kiobOpV7CGD1IvtxAAAA
2026-09-06T23:02:30.041Z [WebSocket] Cliente kiobOpV7CGD1IvtxAAAA se suscribió a Telemetría de Route Discovery
2026-09-06T23:02:30.041Z [WebSocket] Cliente kiobOpV7CGD1IvtxAAAA INTENTÓ unirse a runtime_ack sin autorización — rechazado
```

- Solo **3 líneas WS en TODA la vida del contenedor** (1 cliente; count total = 3). **Sin log de desconexión** → la sesión sigue abierta.
- El intento de unirse a `runtime_ack` sin token fue **rechazado por el gate admin** (`websocket.ts:313-324`): la defensa P0-2 funciona en producción. Ese triple (conectar → suscribir route_discovery → rechazo runtime_ack) es el patrón del dashboard del operador en browser o del L4 socket.io-client; ambas hipótesis son consistentes con la evidencia, no distinguibles read-only.
- Conexiones TCP **establecidas** ahora mismo dentro del netns del contenedor (`/proc/<pid>/net/tcp6`, listener `[::]:1F90`):

```
local ::ffff:172.18.0.21:8080  ← peer ::ffff:172.18.0.23:49738   (= arbitragex-v2-frontend-1, proxy same-origin)
local ::ffff:172.18.0.21:8080  ← peer ::ffff:172.18.0.2:33976    (= arbitragex-v2-prometheus-1, keepalive /metrics)
```

- Re-verificado minutos después: siguen exactamente **2** estables. **Inferencia (declarada, no medida):** la conexión del frontend es el long-poll colgante de la sesión socket.io `kiobOpV7CGD1IvtxAAAA` enrutada por el proxy del Next (alternativa: pool keepalive del rewrite; ver §4). Ningún cliente desde internet llega directo a 8080: el bind es `127.0.0.1:8080->8080` (loopback) y `ss` del host muestra 0 conexiones externas.

---

## 1. Capa LOCAL — MATCH (drift declarado de feature branch en curso)

**Evidencia:**

```
$ git rev-parse HEAD && git branch --show-current && git remote -v
f46a05229c4b958b63dc5795a5c6801d19257174
a6-cbprom-01
origin  https://github.com/hefarica/arbitragex-v2.git   (fetch/push)
```

- `merge-base HEAD origin/main` = `d4d3ff63` = `origin/main` → local = main + exactamente 1 commit chore (`f46a0522 "chore: update branch with main (post-#543 merge)"`).
- Nota de config: en ESTE clone `origin` apunta a GitHub (CLAUDE.md §RULE 01 dice origin=VPS bare; el VPS mismo corre branch `main` en `/opt/arbitragex-v2` — verificado en §3).
- Diff `HEAD` vs `origin/main` limitado a la feature A6-CBPROM-01 (PR en curso, merge esperado):

```
backend/api-server/src/routes/risk-circuit-breakers.ts      | 73 ±
backend/api-server/src/routes/risk-circuit-breakers.test.ts | 97 −
monitoring/alerts.rules.yml                                 | 79 −
shared-ts/src/metrics/index.ts                              | 34 −
```

- **El núcleo WS es idéntico a main**: `websocket.ts` (gateway socket.io, rooms `opportunities`/`metrics`/`convergence`/`telemetry`/`route_discovery`/`runtime_ack`, gates de auth, puentes Redis), `index.ts` (LISTEN `opportunities_channel`, `API_PORT ?? 8080`), `websocket-carnot.ts`. Cero diff.
- Hallazgo de código local (carga para §5): `backend/searcher-rs/src/hot_path_emitter.rs` — `HotPathEmitter` está declarado (`lib.rs:125 pub mod hot_path_emitter;`) pero **NO TIENE NINGÚN call-site** en todo searcher-rs (grep: 0 usos de `emit_detected`/`emit_simulated` fuera del propio módulo).

## 2. Capa REMOTE_MAIN — MATCH (deploy veraz)

```
$ git ls-remote origin main
d4d3ff634537a8b3626ae0fcdaabac70ef3a89f0  refs/heads/main   (feat(relays): A7-RELAYSIM-CALLSITE-01 #543)
```

## 3. Capa VPS — MATCH (contenedor sano, SHA veraz; gaps funcionales NO son drift de código)

```
$ docker ps --filter name=api-server
arbitragex-v2-api-server-1 | Up 23 minutes (healthy) | 127.0.0.1:8080->8080/tcp
$ git -C /opt/arbitragex-v2 rev-parse HEAD; git -C /opt/arbitragex-v2 branch --show-current
d4d3ff634537a8b3626ae0fcdaabac70ef3a89f0   ← == GitHub main, EXACTO
main
$ curl -s -m 5 http://127.0.0.1:8080/health
{"ok":true,"service":"api-server","version":"0.1.0","uptime_s":1384}
```

**Boot limpio 22:58:12Z — todos los puentes WS subscriptos:**

```
{"event":"service.boot","port":8080,...,"msg":"api-server listening"}
[ArteriaWSS] Subscribed to Redis channel: arbx:signals:convergence
[RouteDiscoveryTelemetry] Subscribed to Redis channel: arbx:route_discovery:telemetry
[CartridgeTelemetry] Subscribed to Redis channel: arbx:cartridge:telemetry
[HotStreamer] Consumer group already exists for arbx:hot:detected / arbx:hot:simulated
[HotStreamer] Starting poll loops
{"event":"websocket.listen","msg":"Listening to PostgreSQL opportunities_channel for WebSockets"}
```

**Errores en TODA la vida del contenedor (~27 min): 7** — 6× `401 POST /admin/alertmanager/webhook` (cada 5:00 min exactos: 23:00, 23:05, 23:10, 23:15, 23:20, 23:25) + 1× `503 GET /api/v1/route-discovery-outcomes/summary?hours=24` a 23:02:37 (responseTime 8516ms, post-deploy). **Cero errores del gateway WS.**

**Ruido (R9):** `paper_archiver.skip_rejected` = **895 líneas en ~27 min** — la ventana de 150 líneas que pide el charter cubre solo **48 segundos** de reloj (23:20:44→23:21:32). Diagnóstico de "ausencias" desde `--tail 150` en este contenedor es metodológicamente inválido sin greps dirigidos sobre el histórico completo (exactamente lo que hice).

**Streams hot (broadcast de oportunidades) — VACÍOS:**

```
$ redis-cli XLEN arbx:hot:detected    → 0
$ redis-cli XLEN arbx:hot:simulated   → 0
$ redis-cli XINFO GROUPS arbx:hot:detected → group ws-emitter-g0, consumers 60, pending 0, last-delivered-id 0-0, lag 0
```

60 consumers acumulados (`ws-emitter-<pid>-<ts>`, el más viejo idle ~349,353s ≈ 4 días) = cada restart registra un consumer y jamás se hace `XGROUP DELCONSUMER`. Referencia: `arbx:opps:detected` XLEN = 10004 (pipeline principal con datos; retención activa).

## 4. Capa LIVE_DOMAIN — DRIFT (WS público degradado a polling; doctrina RULE 02 no es lo que corre)

Ruta real del WS público hoy (evidencia `frontend/lib/api-client.ts:67-74` + `frontend/next.config.js:83-131` + netns):

```
browser wss://<dominio> (same-origin, getWsBaseUrl)
  → Cloudflare tunnel (cloudflared --token, ingress remoto gestionado, sin config local)
  → frontend:5173 (Next)  →  rewrite HTTP-only  /socket.io/*  →  http://api-server:8080 (default INTERNAL_API)
  → socket.io cae a TRANSPORTE POLLING (rewrite de Next NO pasa upgrades WS)
```

- `next.config.js:126-129` lo admite textualmente: *"Next rewrites are HTTP-only: socket.io will run its POLLING TRANSPORT through this proxy; the true websocket upgrade needs the nginx path"*.
- **nginx está activo en :80** (`systemctl is-active nginx` = active; `ss -tlnp` muestra 0.0.0.0:80/[::]:80) pero está **FUERA de la ruta del dominio público** (tunnel → 5173 directo). El upgrade WS nativo existe pero nadie del dominio pasa por él.
- El frontend corre sin `INTERNAL_API_URL` (printenv: solo `INTERNAL_API_URL` ausente, usa el default `http://api-server:8080` del rewrite).
- Cloudflared con `--token` (ingress remoto en dashboard CF): **no verificable read-only** desde el VPS. journalctl 3h sin hits 8080/websocket (túneles token no loguean por-request). Declarado como hueco.
- La sesión viva (`kiobOp…` via par docker `frontend-1`) es consistente con esta ruta; **paso CORS no verificado** (charter prohíbe abrir WS): `ALLOWED_ORIGINS` está **unset** en el contenedor → allowlist vacía → same-origin only (`websocket.ts:146-149`); que la sesión pasara implica que el Origin del browser no llegó intacto al handshake (inferencia: el proxy no lo reenvía).
- **Incidente de higiene en MI salida:** `systemctl cat cloudflared` imprimió el `--token` del túnel en claro (ExecStart del unit). NO lo reproduzco aquí; es un secreto expuesto a cualquier lector del unit file. Riesgo registrado en §6.

---

## 5. Drifts medidos (la cadena de broadcast de oportunidades está ROTA en 2 puntos)

1. **CRÍTICO — mismatch de nombre de evento server↔client.** El ÚNICO productor vivo de broadcast de oportunidades es el path PostgreSQL: trigger `trg_notify_opportunity` (+ `trg_notify_opportunity_update`, ambos `tgenabled='O'`) → `notify_new_opportunity()` (`pg_notify`) → LISTEN `opportunities_channel` (`index.ts:1849`) → `broadcastOpportunity` (`websocket.ts:339`) emite **`new_opportunity`** al room. Pero el frontend escucha SOLO **`opportunity:detected`** y **`opportunity:validated`** (`frontend/lib/websocket-client.ts:92,96`). Nadie consume el evento que se emite; el evento que se consume no se emite. **Feed WS de oportunidades = silencio total para el usuario** (los paneles muestran datos por REST).
2. **CRÍTICO — `HotPathEmitter` sin wiring.** `backend/searcher-rs/src/hot_path_emitter.rs` (productor de `arbx:hot:detected`/`arbx:hot:simulated`) tiene 0 call-sites. Streams XLEN=0 perpetuos; el `OpportunityHotStreamer` del api-server consume aire. Es el mismo patrón "2 pipelines desconectados" ya visto en multihop-financing.
3. **Transporte público degradado:** WS del dominio = polling HTTP (ver §4) — drift vs RULE 02 ("WebSocket → api-server DIRECTO, NUNCA via proxy") y vs la intención de diseño (§3: latencia sub-milisegundo).
4. **Branch local ≠ main:** esperado (A6-CBPROM-01 en curso: `risk-circuit-breakers.ts` ±73 + test 97 + alerts.rules 79 + metrics 34). No es defecto; drift declarado.
5. **Alertmanager → api-server webhook 401 cada 5 min** (6 en 27 min): alertas no entregadas por esa vía + warn recurrente.
6. **60 consumers huérfanos** en `ws-emitter-g0` (fuga por restarts, sin DELCONSUMER/XAUTOCLAIM).
7. **R9 violation:** `paper_archiver.skip_rejected` a nivel info per-ítem (895/27min) rota la ventana de observabilidad.

## 6. Riesgos

- El usuario del dashboard NO recibe oportunidades en vivo por WS aunque el server las emite (riesgo de percepción "sistema muerto" + pérdida del diferencial de latencia del canal live).
- Polling por el dominio: +1 RTT HTTP por evento, conexiones colgantes de ~25s, y escalado inútil de logs/conexiones a volumen mainnet (48K opps/24h hoy → cada una notifica a un room sin oyentes del evento correcto).
- Si el proxy Next cambia su manejo de headers (p.ej. pasa a reenviar `Origin`), el WS público muere **silenciosamente** (allowlist CORS vacía = fail-closed) sin ninguna alerta que lo cubra.
- Token del túnel cloudflared en claro en el unit de systemd (demostrado en esta auditoría con `systemctl cat`; no reproducido).
- Webhook de alertas en 401 perpetuo → canal de notificación api-server inservible y contaminación de logs de warn.
- Metadatos del consumer-group de Redis crecen sin límite (60 y contando).

## 7. Propuestas para production-mainnet

| # | What | Why | Priority | Effort | Gate |
|---|------|-----|----------|--------|------|
| 1 | Alinear el contrato de eventos: que `websocket-client.ts` escuche también `new_opportunity` (o renombrar la emisión server-side a `opportunity:detected`) | Es el broadcast insignia; hoy el único productor vivo emite a nadie | **P0** | S (líneas + contract test) | `arbx-pre-edit-audit` + contract-test G2 paridad frontend↔edge/api + L4 con socket.io-client real |
| 2 | Upgrade WS nativo en la ruta pública: ingress CF para `/socket.io` → localhost:80 (nginx) o hostname separado → 8080, eliminando el fallback polling | Polling rompe el presupuesto de latencia (§3/RULE 02) y añade conexiones colgantes | **P0** | M (config túnel + nginx + R4 upgrade binding) | verify con socket.io-client (NUNCA curl, muere en ping/pong) + R7 trazabilidad + `/mcp` §33 |
| 3 | Conectar `HotPathEmitter` en searcher-rs (call-site en el pipeline) O eliminar cable muerto (emitter + HotStreamer + room) | Cable muerto hoy: 0 call-sites, streams vacíos, consumers huérfanos | **P1** | M | `arbx-simulation-mandatory`, R8 fail-honest, invariante XLEN delta documentado (§33.1) |
| 4 | `XGROUP DELCONSUMER`/`XAUTOCLAIM` para `ws-emitter-g0` en shutdown del api-server | 60 consumers huérfanos y creciendo por restart | **P1** | XS | Verificación RO redis + test de shutdown |
| 5 | Arreglar el 401 de `/admin/alertmanager/webhook` (token en config de alertmanager o desactivar la ruta) | Alertas no entregadas + 6 warns/30min | **P1** | XS | Observar ausencia de 401 en logs (read-only) |
| 6 | `paper_archiver.skip_rejected` → debug! + histograma info (R9) | 895 líneas/27min destruyen la ventana de observabilidad | P2 | XS | Doc LOGFLOOD-01 + ventana de logs verificada |
| 7 | Mover el token cloudflared a credentials-file con permisos 600 (hoy va en `ExecStart --token`) | Secreto legible por cualquier `systemctl cat` (demostrado) | P2 | S (operador, fuera de mi alcance RO) | Política de secretos §33 |
| 8 | Poblar `ALLOWED_ORIGINS` con el dominio público y test CORS explícito del proxy | Cierra la ambigüedad fail-closed del handshake vía proxy | P2 | XS | Test con socket.io-client real cross-origin |

## 8. Inventário de evidencia (comandos clave, todos read-only)

- `git rev-parse HEAD` / `git ls-remote origin main` / `git merge-base` / `git diff --stat HEAD origin/main -- backend/api-server`
- `ssh arbx`: `docker ps` · `git -C /opt/arbitragex-v2 rev-parse HEAD` · `curl 127.0.0.1:8080/health` · `docker inspect` (PortBindings `map[8080/tcp:[{127.0.0.1 8080}]]`) · `docker logs [--tail 150 | -t | grep]` · `docker exec redis redis-cli XLEN/XINFO` · `docker exec postgres psql -t -c "SELECT ... pg_trigger/pg_proc"` · `ss -tn/-tlnp` · `/proc/<pid>/net/tcp{,6}` · `systemctl is-active/cat` · `journalctl -u cloudflared --since -3h`
- Local: `backend/api-server/src/websocket.ts` (L146-149 CORS, L313-324 gate runtime_ack, L339-341 broadcastOpportunity, L679-799 HotStreamer) · `index.ts` (L1849-1860 LISTEN, L1741 API_PORT) · `frontend/lib/api-client.ts:67-74` · `frontend/next.config.js:83-131` (WS-POLL-1) · `frontend/lib/websocket-client.ts:92-96` · `backend/searcher-rs/src/hot_path_emitter.rs` (sin call-sites) · PG: `trg_notify_opportunity`/`notify_new_opportunity` verificados.

*Reporte N3 api-ws — 2026-09-06. Fail-honest: cada afirmación lleva su comando; las 2 inferencias (long-poll del par frontend; razón por la que CORS no bloqueó el handshake) están declaradas como tales.*
