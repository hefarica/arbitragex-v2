# N3 — api-ws CROSS-EXAMINATION (round-table integración, ronda 2)

- **Agente:** verificador N3 "api-ws" · **Superficie:** api-server WS 8080
- **Base:** mi reporte `03-api-ws.md` (veredicto DEGRADED) + RE-VERIFICACIÓN post-recreate hecha para esta ronda
- **Reglas:** read-only (0/5 requests públicos usados; ssh + docker/psql/redis-cli RO + curl interno VPS + git local RO). Escritura solo en este archivo.
- **Fecha/hora de re-verificación:** 2026-09-06 ~23:50Z–00:20Z (VPS UTC)

---

## 0. Lo que cambió bajo mis pies desde mi reporte (obligatorio leer primero)

Mi evidencia original (contenedor booteado 22:58:12Z, "Up 23 min") fue tomada **antes** de que la flota se recreara. Re-verifiqué TODO contra el stack nuevo y encontré tres movimientos:

1. **VPS ya NO está en d4d3ff63: está en `9ac06d2d`** (GitHub main tip, #544 desplegado). El drift "dominio 1 PR detrás" que reportó frontend-web ERA real y YA fue cerrado por el pipeline/operador.
2. **El mismatch de eventos P0 sigue VIVO en main**: verifiqué `frontend/lib/websocket-client.ts` en los TRES SHA relevantes (`git show <sha>:...`) — en `f46a0522`, `d4d3ff63` (VPS) y `9ac06d2d` (main) el cliente escucha SOLO `opportunity:detected`/`opportunity:validated`; el listener de `new_opportunity` NO está en ningún commit. #544 no lo toca (diff 5 archivos, ninguno WS).
3. **Hay un fix WO-01 SIN COMMIT en el árbol compartido AHORA**: `git status` muestra `M frontend/lib/websocket-client.ts` (+63 líneas, listener `new_opportunity` con comentario "WO-01 (2026-09-06)"). Es la Remediation Squad "Oleada 3" (ver `GOAL-WORKORDERS.md` en este directorio) implementando MI propuesta #1 en plena mesa redonda. El árbol local mutó durante la auditoría (§36 en vivo).

Re-verificación del stack nuevo (evidencia fresca, comandos en §5):

```
$ ssh arbx docker ps --filter name=api-server
arbitragex-v2-api-server-1 | Up 11 minutes (healthy) | 127.0.0.1:8080->8080/tcp
$ git -C /opt/arbitragex-v2 rev-parse HEAD → 9ac06d2d  (== GitHub main)
$ curl 127.0.0.1:8080/health → {"ok":true,"service":"api-server","version":"0.1.0","uptime_s":679}
$ redis-cli XLEN arbx:hot:detected → 0 ; XLEN arbx:hot:simulated → 0 ; XLEN arbx:opps:detected → 10001
```

- **WS del contenedor nuevo**: 6 líneas `[WebSocket]` = **2 clientes**, cada uno con el MISMO triple (conectar → suscribir `route_discovery` → rechazado en `runtime_ack`). Mi hallazgo del gate admin P0-2 no era una anomalía de una sesión: es el patrón de CADA sesión de dashboard, y el gate lo rechaza siempre.
- **Consumers huérfanos `ws-emitter-g0`: 60 → 62** (medido en los dos sondeos de esta ronda). La fuga creció +2 mientras auditaba — una por boot de api-server, exactamente el mecanismo que reporté.
- **`paper_archiver.skip_rejected`: 278 líneas en ~11 min (~25/min)** en el contenedor nuevo — la violación R9 sigue viva.
- **4ª recreación de flota EN VIVO durante mi sondeo**: mi `docker exec` contra redis falló con "container 60c878… is not running"; el `docker ps` inmediato posterior muestra TODA la flota "Up 8-15 seconds". Y `git ls-remote origin main` sigue en `9ac06d2d` → **esta recreación NO fue disparada por un merge nuevo** (trigger no identificado read-only; ver pregunta a monitoring-fleet).

**Correcciones a mi propio reporte (fail-honest):**
- §0 decía "exactamente 1 sesión socket.io viva" — era cierto para el contenedor de las 22:58Z; el patrón correcto es "1 triple por sesión de dashboard; 2 sesiones en el contenedor nuevo". La conclusión (gate admin funciona, nadie llega a 8080 desde internet) no cambia.
- §1 decía "local = main + 1 commit chore (f46a0522)" — el HEAD volvió a moverse (f7db6867, agentes concurrentes); mis hallazgos de núcleo WS son SHA-independientes (verificados en d4d3ff63 y 9ac06d2d).

---

## 1. CONFIRMACIONES a los demás (su evidencia coincide con la mía)

| # | A quién | Qué confirmo | Mi evidencia |
|---|---------|--------------|--------------|
| C1 | **edge-gateway (D-2)** | `ALLOWED_ORIGINS` ausente → allowlist CORS vacía en prod. Lo confirmo DESDE MI superficie: `websocket.ts:146-149` (api-server) también lee allowlist vacía; fail-closed same-origin only. No es solo un problema del worker REST: el handshake WS del dominio vive de que el proxy Next NO reenvíe `Origin` — frágil por diseño | Código local (websocket.ts L146-149) + printenv del contenedor en mi reporte original §4 |
| C2 | **edge-gateway (evento flota)** | "Flota recreada 23:29–23:33Z; evidencias pre-23:29Z auditaron el deploy anterior". Confirmo y EXTIENDO: capturé una 3ª/4ª recreación más tarde (~00:1xZ) SIN merge nuevo; mis dos sondeos consecutivos vieron api-server "Up 11 min" → flota entera "Up 8-15 s". La invalidación de ventanas de evidencia ocurrió DOS veces solo en mi ronda | `docker ps` ×2 consecutivos + error "container not running" en docker exec intermedio |
| C3 | **monitoring-fleet (R1)** | Su riesgo "cadena de notificación inverificable (webhook enmascarado)" es PEOR de lo que reportaron — ver §2.D1: el receiver está confirmado ROTO (401), no solo inverificable | Logs del contenedor 22:58Z (6× 401 cada 5:00 exactos) + config + código (§2.D1) |
| C4 | **simulator-family (D-9)** | 49 consumers huérfanos en SU grupo = MISMA clase de fuga que mis 62 en `ws-emitter-g0`. Dos verificados independientes, dos grupos, un patrón sistémico: nada en el repo llama `XGROUP DELCONSUMER`/`XAUTOCLAIM` en shutdown | XINFO GROUPS: 60 (reporte original) → 62 (esta ronda) |
| C5 | **data-layer** | `XLEN arbx:opps:detected` ~10K estable (su 10,001-10,004; mi 10004→10001). Además: su hallazgo de trim-antes-de-consumo NO afecta mi canal WS vivo (mi broadcast es PostgreSQL LISTEN/NOTIFY, no Redis) — lo declaro para que el operador no los mezcle | XLEN en ambos sondeos + `websocket.ts:339` + `index.ts:1849` (path PG) |
| C6 | **frontend-web** | Su hallazgo "build sin identidad verificable" es fleet-wide, no solo del HTML: mi `/health` devuelve `version:"0.1.0"` sin SHA en ambos contenedores (22:58Z y nuevo). Tres superficies (frontend HTML, sim-family `/capabilities build.sha=null`, api-server `/health`) = UNA sola convención faltante | `curl 127.0.0.1:8080/health` ×2 contenedores |
| C7 | **exec-terminus** | Su propuesta P0 "provisionar secretos vía vault-agent → /run/secrets/arbx.env (envsubst)" es EXACTAMENTE el canal que necesita mi fix del webhook 401 — ver propuesta refinada #5. El comentario de `alertmanager.yml:6-17` ya documenta ese mecanismo para sus secretos | `monitoring/alertmanager/alertmanager.yml:6-17` |

## 2. DESAFÍOS (dónde contradigo o corrijo con contra-evidencia)

### D1 → monitoring-fleet: su webhook era DIAGNOSTICABLE, no "enmascarado e inverificable"
Su R1 dice "receiver único con URL enmascarada `<secret>`… nadie ha demostrado recibir notificación alguna". La afirmación de incognoscibilidad es incorrecta: la evidencia de que ese canal está MUERTO estaba disponible desde tres fuentes que su superficie no cruzó:

1. **Config** (`monitoring/alertmanager/alertmanager.yml:38-48`, en el repo): TODO (`severity: critical|warning|info`) rutea al receiver `default` → `webhook_configs: url: "http://api-server:8080/admin/alertmanager/webhook"` **SIN `http_config`/header de autorización alguno**. Slack/PagerDuty están COMENTADOS (L50-58). O sea: ese webhook interno es el ÚNICO receptor vivo del sistema.
2. **Código** (`backend/api-server/src/routes/alertmanager-webhook.ts:50`): la ruta exige `requireAdminToken` (`x-arbx-admin-token`); contrato probado por unit test (`stubs.test.ts:102`: "POST /admin/alertmanager/webhook → 401 without token").
3. **Runtime** (mi reporte §3): 6× `401 POST /admin/alertmanager/webhook` en 27 min, cada 5:00 exactos (23:00, 23:05, … 23:25Z).

Conclusión forzada: **0 alertas entregadas por cualquier canal** mientras 5 `RpcCircuitBreakerOpen` están activas. Su propuesta "alerta sintética end-to-end" está bien pero es innecesaria para diagnosticar — el fix es determinista (darle el header al receiver). Subo la prioridad a P0 (ver propuestas).

### D2 → frontend-web: su "árbol limpio" ya era falso minutos después — y afecta la reproducibilidad de TODOS
Su evidencia local (18:41) dice "frontend/ limpio sin cambios no commiteados". En mi lectura (~18:45+) `git status` muestra `M frontend/lib/websocket-client.ts` (+63). No es error suyo (el tiempo avanzó), pero demuestra que **ninguna afirmación "local" de esta mesa es estable**: la Oleada 3 muta el árbol mientras verificamos. Toda evidencia local de esta ronda necesita timestamp; y el WO-01 uncommitted debe aterrizar por PR con ID (P-∅ §37) o se pierde en el próximo checkout de un agente (§36 — ya pasó 3 veces hoy según su propio reflog).

### D3 → edge-gateway + data-layer: `count:0` público (23:37Z) vs `count:50` interno (23:20Z) — incompatible con inserción continua
Con ~58K detecciones/24h (~0.67/s) y `max_age_seconds:300`, la ventana REST nunca debería estar vacía en régimen permanente. Edge-gateway midió `count:0, items:[]` en el dominio público a 23:37Z; data-layer midió `count:50` interno a ~23:20Z. La recreate de 23:33Z (cache fría + searcher re-arrancando) podría explicarlo, pero NINGUNO de los dos lo cerró. Esto importa a MI superficie: la percepción "feed muerto" del usuario tiene (al menos) DOS causas independientes — mi mismatch de eventos WS Y estas ventanas REST vacías post-deploy. Si el operador solo arregla una, el síntoma persiste. (Pregunta directa en §3.)

### D4 → simulator-family (matiz, no contradicción): su G-SIM-1 "green engañoso" tiene un paralelo exacto en mi superficie
Su R-3 ("green mide que las sims CORREN, no que APRUEBEN") es la misma clase de señal tramposa que mi badge WS (WO-08 ya lo caza): "conectado" ≠ "recibiendo eventos del canal correcto". Lo aporto como patrón transversal para el operador: 3 gates verdes (G-SIM-1, badge WS LIVE, readiness) miden flujo/conexión, no función económica. No corrige su evidencia — la generaliza.

## 3. PREGUNTAS DIRECTAS

1. **a frontend-web**: (a) El listener WO-01 (+63 líneas uncommitted) aterrizó en el árbol compartido minutos después de tu "árbol limpio" — ¿tu metodología de drift puede estampar SHA+timestamp en CADA lectura local para la próxima ronda? (b) ¿El nuevo panel GoNoGoSignOffCard de #544 (ahora YA desplegado en el dominio: VPS=9ac06d2d) consume algún room WS (`runtime_ack` incluido)? Mis 2 sesiones WS vivas intentan `runtime_ack` y el gate las rechaza SIEMPRE — si el panel A.9 necesita ack runtime vía WS, necesita un flujo documentado de token para el browser, o ese rechazo es el estado permanente esperado. (c) Re-probe el dominio: `data-slot=go-no-go-signoff-card` debiera aparecer ahora — cierra formalmente tu drift #1.
2. **a edge-gateway**: reconcile `count:0` público (23:37Z, `x-arbx-cache MISS`) vs `count:50` interno (23:20Z) bajo flood continuo (§2.D3). ¿Tu cache 2s o la ventana `max_age` del backend explican un vacío de 5+ minutos post-recreate, o hubo gap real de datos PG→edge? Descarta también que `?limit=3` interactúe con `count`.
3. **a monitoring-fleet**: (a) ¿Revisaste si ALGÚN receiver del config vivo lleva `http_config`? El mío no (alertmanager.yml:46-48) — ¿confirmas que el `default` es el único vivo? (b) ¿Alertmanager corre con `--storage.path` persistente? Sin él, cada una de las 4 recreates de esta noche resetea el nflog y re-dispara notificaciones (→ ráfagas de 401 en mi superficie). (c) La 4ª recreate (~00:1xZ) ocurrió SIN merge nuevo (origin/main congelado en 9ac06d2d, verificado por mí) — ¿identificas el trigger desde tu superficie (retry del auto-deploy? operador)? Es el dato que falta para tu propuesta de deploy-coalescing.
4. **a data-layer**: tus ~488 entradas recortadas antes de consumo por `paper-archiver-g0` — ¿puedes confirmar que esas entradas TAMBIÉN faltan en `paper_trade_runs` (pérdida total) y no solo en el stream (pérdida de cola)? Si faltan en ambos, el hueco del ledger paper es permanente y mi propuesta #3 (WS `opportunity:validated` vía HotStreamer) arrancaría con un agujero histórico ya imposible de cerrar.
5. **a simulator-family**: ¿co-patrocinamos UNA propuesta generalizada de higiene de consumer-groups (DELCONSUMER en shutdown + sweep XAUTOCLAIM en boot) cubriendo tus 49 huérfanos + mis 62? Un solo PR, un solo patrón, dos superficies.
6. **a exec-terminus**: cuando tu P0 provea el canal vault-agent→envsubst→`/run/secrets/arbx.env`, ¿lo diseñamos con slots para `ALERTMANAGER_ADMIN_TOKEN` (alertmanager render + api-server) y `ALLOWED_ORIGINS`? Así MI fix del 401 y el CORS muerto de edge-gateway aterrizan por el MISMO mecanismo en vez de tres env-patches sueltos.

## 4. PROPUESTAS REFINADAS (las mías, actualizadas con dependencias cruzadas)

| # | What (delta vs mi reporte) | Why cambia | Prio | Effort | Gate + dependencias |
|---|---|---|---|---|---|
| 1 | **Contrato de eventos WS**: mi propuesta #1 está SIENDO implementada (WO-01, +63 líneas uncommitted, cliente escucha `new_opportunity` mapeado al flujo `opportunity:detected`). Refino: debe aterrizar como PR con ID (P-∅), con contract-test de AMBOS nombres, y dejando claro que el path `HotStreamer` (`opportunity:validated`) sigue muerto hasta #3 | El fix es correcto pero frágil: uncommitted en un árbol que muta cada ~30 min (§36); sin PR se pierde | P0 | XS (ya escrito) + test | `arbx-pre-edit-audit` + G2 paridad + L4 socket.io-client real. DEP: ninguna bloqueante |
| 2 | **401 webhook Alertmanager → api-server** — SUBO a P0 con causa raíz cerrada: agregar `http_config.authorization` (token) al receiver `default` de `alertmanager.yml`, token por el canal vault-agent/envsubst propuesto por exec-terminus (el archivo ya documenta el mecanismo en L6-17) | Es el ÚNICO canal de alertas vivo del sistema y está 100% muerto (401) mientras 5 breakers RPC arden; monitoring-fleet lo tenía como "inverificable" — ya es "confirmado roto" | **P0**↑ | XS (config + secret) | Operador (secrets T1 §33) + verificar 0× 401 en logs post-deploy. DEP: canal Vault de exec-terminus (o env interino documentado) |
| 3 | **Wiring HotPathEmitter (WO-02)** — refino con lo aprendido de simulator-family: el wiring debe emitir en etapa POST-validación, no en detección — si emite el flood crudo (98% `strategy_not_simulatable_in_s4`, D-7 de sim-family) solo traslada el ruido al room WS | El diseño "conectar el cable" sin semántica de etapa reproduciría el flood en el browser | P1 | M | `arbx-simulation-mandatory`, invariante XLEN delta §33.1. DEP: decisión de etapa vs sim-family |
| 4 | **Higiene consumer-groups GENERALIZADA** (antes era solo ws-emitter-g0): DELCONSUMER en shutdown + sweep XAUTOCLAIM en boot, patrón único para `ws-emitter-g0` (62 y contando) + el grupo de sim-family (49) | Dos superficies, misma fuga, un PR | P1 | XS-S | Test de shutdown + verificación RO redis. DEP: co-patrocinio sim-family (pregunta 5) |
| 5 | **Ruta WS nativa pública** (polling→upgrade): sin cambios de fondo, PERO añado requisito operativo: desplegar en ventana quieta — esta noche hubo 4 recreates y cualquiera de mis verificaciones WS habría sido inválida a mitad | La verification de un cambio de transporte WS exige estabilidad mínima del stack | P0 | M | verify socket.io-client real + R7. DEP: deploy-coalescing de monitoring-fleet (su P0) es PREREQUISITO operativo |
| 6 | **Identidad de build fleet-wide** — fusiono la P0 de frontend-web (NEXT_PUBLIC_GIT_SHA + header) con sim-family D-10 (`build.sha=null`) y mi `/health` (version "0.1.0" sin SHA): UNA convención (`<service> expone build SHA`), tres superficies, un PR | Tres verificados tropezaron con el mismo hueco por separado; unificado es un solo gate deploy-veraz G5 | P1 | S | G5 deploy-veraz (`git rev-parse HEAD` == SHA reportado por CADA servicio). DEP: ninguna |
| 7 | **ALLOWED_ORIGINS**: sin cambio de sustancia, pero coalesco con edge-gateway D-2 y WO-14 en UNA entrada de runbook (un env, dos superficies: edge REST + api-server WS) | Evita dos PRs para el mismo env | P1 | XS | Operador (runbook WO-14) + test cross-origin socket.io-client |
| 8 | **R9 `paper_archiver.skip_rejected`** → debug! + histograma: re-confirmado en contenedor nuevo (278 líneas/11 min ≈ 25/min). Añado: el churn de deploys AMPLIFICA el daño (cada recreate resetea la ventana y los greps dirigidos que hice son la ÚNICA forma confiable) | Igual | P2 | XS | Doc LOGFLOOD-01 |
| 9 | *(nueva)* **Post-mortem del deploy-churn de esta noche**: 3 recreates por merges legítimos (22:58Z, 23:33Z) + 1 SIN merge identificado (~00:1xZ) — alimentar el deploy-coalescing de monitoring-fleet con este dato: la 4ª recreate es hoy inexplicada | Un recreate sin causa identificada en la frontera de capital es inaceptable para mainnet | P1 | S (investigación) | monitoring-fleet (pregunta 3c) + journalctl dockerd |

## 5. Inventario de evidencia de ESTA ronda (read-only, todos ejecutados por mí)

- Local git: `git show f46a0522|d4d3ff63|9ac06d2d:frontend/lib/websocket-client.ts | grep socket.on` (3 SHA sin listener `new_opportunity`) · `git status --porcelain` (M websocket-client.ts) · `git log -S "WO-01"` (0 commits → fix inédito) · `git ls-remote origin main` (9ac06d2d sin cambio pese a la 4ª recreate) · `grep alertmanager-webhook.ts:50 / stubs.test.ts:102 / alertmanager.yml:38-58`
- SSH arbx: `docker ps --filter name=api-server` (Up 11 min → luego flota entera Up 8-15 s) · `git -C /opt rev-parse HEAD` (9ac06d2d) · `curl 127.0.0.1:8080/health` (uptime_s 679) · `docker logs` grep `\[WebSocket\]` (6 líneas / 2 clientes / 2 rechazos runtime_ack) · grep `401.*alertmanager/webhook` (0 en 11 min — vida demasiado corta + nflog reseteado; la evidencia de 401 es del contenedor 22:58Z, 6× cada 5:00) · grep `paper_archiver.skip_rejected` (278) · `redis-cli XLEN arbx:hot:detected|simulated|opps:detected` (0/0/10001) · `XINFO GROUPS arbx:hot:detected` (62 consumers) · docker exec fallido "container 60c878… not running" (recreate intermedia capturada)

*Cross-examination N3 api-ws — 2026-09-06. Fail-honest: 3 inferencias declaradas en el reporte base se mantienen; esta ronda añade 0 inferencias nuevas no marcadas. 0/5 requests públicos usados.*
