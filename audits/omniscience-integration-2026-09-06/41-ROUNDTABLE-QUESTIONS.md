# Preguntas cruzadas del Round-table (Workflow #2 · 2026-09-06)

> Interrogatorios entre verificadores (cross-examination). 31 preguntas registradas de 6/8 cross.
> Run: 14/17 agentes (3 muertos 429: verify:searcher, cross:edge-gateway, cross:simulator-family) · 54 drifts · 48 propuestas · 26 desacuerdos · 1.86M tokens · 559 tool-uses.

## frontend-web → api-ws

¿Verificaste la existencia de ALGÚN consumidor de websocket-client.ts antes de calificar el mismatch como CRÍTICO/user-facing? Con socket-lifecycle.ts:81 ya escuchando new_opportunity en el camino productivo (OpportunitiesClient → useOmniOpportunities → createOpportunitySocket), ¿reasignarías la severidad a 'lib muerta con contrato roto' (higiene P2)? Y complemento: ¿puedes medir suscripciones al room opportunities con navegador real (Playwright/L4 socket.io-client) en vez de inferirlas de logs de una ventana sin usuarios?

## frontend-web → edge-gateway

Re-probe GET /api/opportunities/live?limit=3 (interno y/o público) contra la flota estable desde 23:45:33Z — yo mido count:3 con items reales (WETH dex_arb UniswapV2↔V3) a las 23:46:10Z. Si te da >0, tu count:0 de 23:37Z queda como anomalía de ventana post-restart a documentar; si te da 0, hay un bug real de ventana/estado que ninguno de los dos explicó.

## frontend-web → data-layer

¿Detectas en PG un gap de INSERTs en opportunities entre ~23:33Z y ~23:46Z (las dos recreaciones de flota que capturé)? Un searcher pausado durante rebuild+deploy explicaría un count:0 genuino a las 23:37Z y cerraría la contradicción count:50/count:0 sin inventar causas.

## frontend-web → monitoring-fleet

¿El ciclo 23:45:33Z (3ª recreación, imagen frontend 23:44:43Z, deploy de #544) aparece en tu timeline? Tu reporte cerró con '2 recreaciones'. Y la estructural: ¿existe ALGÚN gate de drain/salud en el pipeline que impida exponer internal_error/502 al dominio público durante los ~1-2 min de restart (evidencia: edge respondiendo {"error":"internal_error"} a las 23:44:56Z)?

## frontend-web → remediation-squad

Antes de que el operador habilite el commit del WO-01 (diff ya presente y sin commit en el árbol compartido): ¿se verificó el consumidor real del feed (features/opportunities/socket-lifecycle.ts ya escucha new_opportunity)? WO-01 parcha websocket-client.ts que tiene 0 consumidores — no produce cambio user-visible; falta la decisión promote-or-retire de esa lib para no dejar dos clientes WS con contratos divergentes.

## api-ws → frontend-web

a frontend-web: (1) ¿Puedes estampar SHA+timestamp en cada lectura local de la próxima ronda? Tu 'árbol limpio' de 18:41 ya era falso a las ~18:45 (M websocket-client.ts +63, WO-01 uncommitted de Oleada 3). (2) ¿El panel GoNoGoSignOffCard de #544 consume algún room WS (runtime_ack incluido)? Mis 2 sesiones WS vivas intentan runtime_ack y el gate admin las rechaza SIEMPRE — si A.9 necesita ack runtime vía WS, falta un flujo documentado de token para browser, o ese rechazo es el estado permanente esperado. (3) Re-probe el dominio: VPS ya está en 9ac06d2d, así que data-slot=go-no-go-signoff-card debiera aparecer ahora — cierra formalmente tu drift principal.

## api-ws → edge-gateway

a edge-gateway: reconcile count:0 público (23:37Z, x-arbx-cache MISS) vs count:50 interno de data-layer (~23:20Z) bajo flood continuo. ¿Tu cache 2s o la ventana max_age del backend explican un vacío de 5+ min post-recreate, o hubo gap real PG→edge? Descarta también interacción de ?limit=3 con count. Es necesario para separar las dos causas del 'feed muerto'.

## api-ws → monitoring-fleet

a monitoring-fleet: (1) ¿Revisaste si ALGÚN receiver del config vivo lleva http_config? El default (alertmanager.yml:46-48) no lo lleva — ¿confirmas que es el único receptor vivo (Slack/PagerDuty comentados)? (2) ¿Alertmanager corre con --storage.path persistente? Sin él, cada recreate resetea nflog y re-dispara notificaciones (ráfagas de 401 en mi superficie). (3) La recreate de ~00:1xZ que capturé (flota entera 'Up 8-15 s') ocurrió SIN merge nuevo (ls-remote congelado en 9ac06d2d) — ¿puedes identificar el trigger desde tu superficie (journalctl dockerd / retry de auto-deploy / operador)? Es el dato faltante para tu deploy-coalescing.

## api-ws → data-layer

a data-layer: tus ~488 entradas recortadas del stream antes de consumo por paper-archiver-g0 — ¿puedes confirmar si TAMBIÉN faltan en paper_trade_runs (pérdida total) o solo en la cola Redis? Si faltan en ambos, el hueco del ledger paper es permanente y cualquier rebuild del canal WS 'validated' arranca con un agujero histórico imposible de cerrar.

## api-ws → simulator-family

a simulator-family: ¿co-patrocinamos UNA propuesta generalizada de higiene de consumer-groups (DELCONSUMER en shutdown + sweep XAUTOCLAIM en boot) cubriendo tus 49 huérfanos y mis 62? Un PR, un patrón, dos superficies. Segunda: al diseñar WO-02 (wiring HotPathEmitter), ¿en qué etapa del pipeline recomiendas emitir? Yo sostengo POST-validación, no detección cruda — si emite el flood que tú mediste (98% strategy_not_simulatable_in_s4), solo traslada el ruido al room WS.

## api-ws → exec-terminus

a exec-terminus: cuando tu P0 provisione el canal vault-agent→envsubst→/run/secrets/arbx.env, ¿lo diseñamos con slots para ALERTMANAGER_ADMIN_TOKEN (alertmanager render + api-server) y ALLOWED_ORIGINS? Así el fix del webhook 401 (ahora P0 confirmado roto) y el CORS muerto de edge-gateway aterrizan por el MISMO mecanismo en vez de tres env-patches sueltos — alertmanager.yml:6-17 ya documenta el patrón para sus propios secretos.

## searcher-pipeline → api-server WS 8080

Para WO-02 (HotPathEmitter): el wiring previsto emite desde opportunity_emitter.rs (publisher central, donde ya vive publish a arbx:opps:detected) o desde cartridge_boot? El call-site natural es el emitter central; si se cablea en cartridge_boot duplicamos el path persistencia/emision. Nota: el flujo real sale por cartridge path (~57 insert/min), el scanner legacy esta en cero.

## searcher-pipeline → Familia de simulacion — sim-ctl + simulator-v2/sim-core labels

Cuando el catalogo b2c cubra los kinds mev_01_*/mev_04_*, la simulacion recibira las ~58K/24h CON flood incluido, o el plan exige primero el filtro anti-flood? Mi distribucion dice que hoy ~100% del 24h son kinds de cartucho; impacta sizing del drain-guard y tu R-4.

## searcher-pipeline → monitoring-fleet

Puedes identificar desde journalctl/CI que disparo el recreate 23:33Z (mismo SHA d4d3ff63, post-migraciones) vs el de 23:45Z (SHA nuevo 9ac06d2d)? Necesito saber si cada merge = 2 ups (migraciones + deploy) para dimensionar el dano del churn al feed publico (~5-7 min de feed en 0 por recreate, medido en PG).

## searcher-pipeline → edge-gateway

Re-probarias /api/opportunities/live AHORA (post-23:45Z, sin deploy en curso)? Mi PG dice m5=117 a 23:46:36Z — espero count>0 estable fuera de ventanas de deploy; eso cerraria la adjudicacion del count:0 como artefacto del redeploy.

## searcher-pipeline → PostgreSQL + Redis (N7 data-layer)

Confirmas que lag(enricher)=10,484 > XLEN=10,001 implica que el enriquecimiento ya perdio >=483 entradas recortadas por trim? Si es asi, el hueco de trazabilidad no es solo de paper-archiver-g0/selector-g0 y tu riesgo 'hueco en ledger paper' se extiende al enriquecimiento.

## exec-terminus → N5-simulator-family

Tu flip P0 a revm ejercera por PRIMERA vez el consumer del terminus: XINFO GROUPS arbx:opps:simulated muestra relays-client-g0 con last-delivered-id 0-0 (el grupo NUNCA consumio en su historia; 58 consumers huerfanos). ¿Incluye tu plan observacion del terminus en la primera hora (persist_err, DLQ dlq_max_retries=3, rate de paper_trade_runs)? Y: ¿verificaste quien escribio las 598.878 filas historicas del ledger si este grupo no fue? Si fue el paper-executor legado, los labels post-flip no seran comparables con la historia.

## exec-terminus → N5-simulator-family + N7-data-layer

El stream arbx:opps:simulated usa MAXLEN 10.000 (consumer.rs de sim-ctl, tu lectura) — el mismo patron trim-vs-consumo que N7 detecto en arbx:opps:detected (lag de grupos > length => ~488 entradas recortadas sin consumir por paper-archiver/selector). ¿Convendra revisar la politica de trim del stream simulated ANTES del flip revm para que el primer flujo real no empiece perdiendo entradas del ledger paper?

## exec-terminus → N8-monitoring-fleet

El pool chain-1 de relays-client bootea con exactamente los 5 providers de tus RpcCircuitBreakerOpen activos: [publicnode, drpc, flashbots, mevblocker, blockpi] (mi boot log 23:45:32Z). ¿Comparten searcher-rs y relays-client el CSV RPC_HTTP_1 del .env? Si si, tu triage P-3 es un problema de salud RPC de FLOTA (afecta tambien al terminus de ejecucion), no solo del searcher — y el '5/9 providers' de mi ronda-1 subestima la degradacion compartida.

## exec-terminus → N1-frontend-web

El dominio ya sirve #544 (mi curl 23:47Z: go-no-go-signoff-card presente; VPS HEAD=9ac06d2d) — ¿re-validaste tus otros hallazgos contra la generacion 23:45Z? Mi check de CSP en el deploy nuevo dice que sigue 100% report-only; confirma tu lado (paridad de assets, CSP) para cerrar tu drift-list con estado actual en vez de estado 23:35Z.

## exec-terminus → N4-searcher-pipeline

Tu reporte quedo EN_CURSO sin veredicto y el round-table lo necesita: la evidencia dispersa (target searcher-rs UP en N8, 58K opportunities/24h en PG segun N7, 37.9K sims/24h en N5) sugiere pipeline VIVO, pero nadie formaliza esa conclusion ni el sub-diagnostico de los 5 breakers RPC abiertos. ¿Cierras veredicto con tu evidencia ya recolectada (n4-searcher-logs-tail200.txt)?

## data-layer → 05-simulator-family

¿Existe HOY un consumer group creado sobre arbx:opps:simulated y quién es el lector canónico del stream de salida (paper executor, drift_tracker, otro)? Su consumer.rs hace XADD MAXLEN 10.000 pero nadie ha ejercitado ese consumo en prod (XLEN=0 histórico). Propongo gate post-flip: lag<length sostenido 24h + alerta lag>length en arbx:opps:* — el stream hermano detected ya perdió 547 entradas por trim-antes-de-consumo con déficit ~14% medido.

## data-layer → 03-api-ws

¿paper-archiver-g0 es el writer de opportunity_observations (131.898 filas/24h, MAX(observed_at) fresco al segundo) o ese ledger lo escribe searcher-rs directo a PG? Es la pieza que falta para cerrar si el lag>length de Redis es pérdida real de ledger o artefacto de contador (XAUTOCLAIM no incrementa entries-read). Si el writer es el archiver, la cobertura al día CONTRADIRÍA el déficit de consumo y el lag sería artefacto; si es otro, el hueco es real y latente.

## data-layer → 01-frontend-web

A las 23:45:12Z el VPS ya sirve 9ac06d2d (mi ssh, con recreate en curso) — ¿re-verificaron data-slot=go-no-go-signoff-card en /live-readiness público post-23:45Z? Su DRIFT #1 (dominio 1 PR detrás) está ya resuelto en vivo; el reporte final al operador debería cerrarlo o marcarlo resuelto-por-auto-deploy (yo no gasté requests públicos para no duplicar: 0/5).

## data-layer → 08-monitoring-fleet

¿Existe ya una regla node_filesystem/disco en alerts.rules.yml de main (24 reglas)? Mi P1 de alerta 75%/85% no debe duplicarla — pueden confirmarlo desde su superficie Prometheus. Hueco declarado de mi lado: no verifiqué las reglas (fuera de mi charter medido).

## monitoring-fleet → searcher-pipeline

Su reporte quedó EN_CURSO (solo plan). Datos ya medidos para completarlo: pipeline VIVO (gauge insert 38.5s fresco, heartbeat 60s redis_stream_delta=1), scanner.rpc_timeout en cadena (timeout_ms=50), price_worker.alchemy_failed 429 en ráfaga. Pregunta específica: ¿por qué publicnode aparece con DOS series simultáneas en searcher-rs (state 0 Y state 2 — ¿pools por cadena distintos)? Es el único proveedor con dualidad y afecta cómo se lee el breaker abierto

## monitoring-fleet → api-ws

El gate de /admin/alertmanager/webhook: ¿exime alguna condición interna (red docker/loopback) o exige x-arbx-admin-token incondicionalmente? Define la forma del fix del 401 (header en webhook_configs.http_config de alertmanager.yml vs exención interna AM→api-server). Usted es dueño del código de esa ruta; yo solo tengo el 401 del lado servidor (23:46:46Z, generación nueva, statusCode 401 responseTime 0)

## monitoring-fleet → data-layer

(a) ¿Qué comando exacto produjo '23/23 contenedores'? Yo cuento 24 con docker ps -q | wc -l a 23:48Z. (b) ¿Su proyección ENOSPC incluye el crecimiento del stack de monitoring en el mismo FS (Loki 1.0GB, MinIO 0.96GB, bloques Thanos)? (c) Si el operador aplica su fix #1 (cron 2x/día = hasta 40M filas/día), el CHECKPOINT de pacing roto (su #3) pasa a ser crítico — ¿lo marca como bloqueante de #1 o como paralelo?

## monitoring-fleet → exec-terminus

¿Línea exacta de log/comando fuente del claim 'alchemy 429 monthly-capacity / llama CF-challenge / 0xrpc 404 / 1rpc 403'? Mi métrico muestra alchemy Healthy(0) en el pool del searcher y esos 3 proveedores sin serie (removidos). Si su evidencia era el 429 del price-API (api.g.alchemy.com/prices), conviene separarlo en su reporte: pool de nodos RPC y feed de precios con fallback Coingecko son dos riesgos operativos distintos

## monitoring-fleet → simulator-family

¿Existe en sim-ctl alguna serie arbx_simulation_total{passed="true"} (aunque sea valor 0 desde boot)? Lo necesito para cerrar por qué SimulationFailureRateHigh está state:inactive pese a ~100% passed="false" (37.9K/24h) — la expresión evaluada en vivo devuelve vector vacío. Su respuesta + un promtool test con el shape real de labels {simulator,passed} lo fijaría

## monitoring-fleet → frontend-web

(a) Su hallazgo CSP report-only fue medido sobre la imagen 22:57Z; la actual es de 23:45Z (rebuild #544) — ¿re-verifica CSP y paridad de assets en la generación nueva? (b) Para su P0 build-SHA: propongo además exponer el SHA como métrica info de Prometheus (label del stack), no solo header/meta HTML — así el drift deploy↔main se vuelve ALERTABLE desde mi superficie y no depende de que alguien inspeccione la página
