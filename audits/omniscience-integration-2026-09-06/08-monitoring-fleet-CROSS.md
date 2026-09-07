# N8 monitoring-fleet — CROSS-EXAMINATION (round 2)

- **Agente:** N8 monitoring-fleet · **Superficie:** flota + Prometheus/Alertmanager/Grafana/Loki/Thanos + reglas
- **Ventana del cross:** 2026-09-06 23:46Z → 23:55Z (verificación viva post-redeploy, read-only)
- **Presupuesto público:** 1 request nuevo en esta ronda (**3 de 5** acumulado: / , /readiness en ronda 1, /live-readiness ahora). 0 mutaciones.
- **Veredicto del cross:** DEGRADO se **refuerza**: la cadena de notificación está rota por CONFIG (no por misterio), 8 de 24 reglas están muertas estructuralmente por fuentes de métrica inexistentes, y hubo un **TERCER recreate de flota (23:45:26Z)** que invalida parte de la evidencia de otros verificadores.

---

## 0. ACTUALIZACIÓN DE MI SUPERFICIE (estado a 23:48Z — post-ciclo #544)

| Ítem | Ronda 1 (23:31-23:40Z) | AHORA (23:46-23:55Z) | Evidencia |
|---|---|---|---|
| VPS HEAD | d4d3ff63 (#543) | **9ac06d2d (#544) — drift D3 RESUELTO** | `git -C /opt/arbitragex-v2 rev-parse HEAD` |
| Flota | 24/24 (gen 23:33) | **24/24, TERCER recreate 23:45:26Z** ("Up 2 minutes") | `docker ps -q \| wc -l` = 24; AM `StartedAt=23:45:26.610Z` |
| Dominio público | no re-testeado | **sirve #544**: `data-slot="go-no-go-signoff-card"` ×1, HTTP 200 | curl público `/live-readiness` (request 3/5) |
| Targets Prometheus | 8 up / 1 down (edge 404) | idéntico: 8 up / 1 down (`http://edge:8787/metrics` 404) | `/api/v1/targets` |
| Reglas | 4 grupos / 24 reglas | idéntico; health 24×"ok", 0 errores de evaluación | `/api/v1/rules` |
| Alertas activas | 5× RpcCircuitBreakerOpen | idéntico (5× en Prometheus Y en AM, 0 silences) | `/api/v1/alerts`, `/api/v2/alerts` |
| Breakers RPC | 5 open (searcher) | idéntico y PERSISTENTE a través de 3 generaciones: blockpi, publicnode, drpc, mevblocker, flashbots | `arbx_rpc_provider_state == 2` |
| Pipeline searcher | — | **VIVO**: gauge de insert fresco a 38.5s | `time() - max(arbx_pipeline_last_opportunity_insert_unixtime)` |
| Disco / | 78% (33G libres) | **76% (36G libres)** — la limpieza post-deploy liberó ~6G | `df -h /` |

**Cadena de config de Alertmanager CERRADA (md5):** `git show origin/main:monitoring/alertmanager/alertmanager.yml` (md5 `7a0529d4…`) == archivo en VPS (`7a0529d4…`) == montado en el contenedor (`/etc/alertmanager/alertmanager.yml`, mount verificado por `docker inspect`). El receiver enmascarado `<secret>` de mi ronda 1 queda **des-enmascarado por el repositorio**: `http://api-server:8080/admin/alertmanager/webhook`, **sin header de auth alguno** (`alertmanager.yml:44-48`).

---

## 1. CONFIRMACIONES (acuerdos con los otros verificadores)

1. **api-ws (N3) — webhook 401 cada 5:00 min: CONFIRMADO y con causa raíz.** Sus 6×401 (23:00→23:25, gen-1) los reproduje en la gen-3: primer 401 a las **23:46:46Z** (contenedor de 2 min de vida) y seguirá. Causa: `monitoring/alertmanager/alertmanager.yml:47` define el webhook **sin `http_config`/token**, el gate admin de api-server exige `x-arbx-admin-token` → 401 estructural desde el "Hardened 2026-08-03". La cadencia exacta de 5:00 = `group_interval: 5m` (línea 26) sobre alertas que flapbean (el breaker half-open cada 30s genera nuevas transiciones de estado). **Nadie recibe NADA: ni firing ni resolved (`send_resolved: true` también muere en el 401).** Mi fail-honest #1 de ronda 1 queda cerrado por evidencia cruzada.
2. **data-layer (N7) — disco y bomba de crecimiento: CONFIRMADO en tendencia.** Mis mediciones 78%→76% enmarcan su 80%; la mejora es limpieza transitoria del ciclo de deploy, NO cancela la proyección estructural (+10M filas/día netas ⇒ ENOSPC ~09-12/13). Su hallazgo del VACUUM roto y del cron vivo es consistente con lo que yo vi del ciclo de migraciones.
3. **frontend-web (N1) — drift #544: era real, y YA SE CERRÓ SOLO.** Su hallazgo (dominio sirviendo d4d3ff63, 0× `go-no-go-signoff-card`) era verdadero en su ventana; el ciclo de deploy en cola que yo documenté en ronda 1 completó a las 23:45:26Z y **verifiqué el dominio público sirviendo el card nuevo** (§0). Su comando de verificación (`data-slot=go-no-go-signoff-card` presente) es exactamente el gate correcto — ya pasa.
4. **edge-gateway (N2) — evento de redeploy en vivo: CONFIRMADO y EXTENDIDO** (ver desafío E-1: fueron 3 recreates, no 2). Su método de fingerprint runtime (`x-arbx-audit-token` en headers públicos) es sólido y yo re-apliqué el mismo principio md5 sobre alertmanager.yml.
5. **simulator-family (N5) — 100% passed=false: CONFIRMADO a nivel métrica.** `arbx_simulation_total` existe con labels `{passed="false", simulator="not_implemented"|"anvil"}` — el label `simulator="not_implemented"` ES la cara métrica de `strategy_not_simulatable_in_s4`. Su lectura de PG (0 aprobaciones históricas) y la métrica viva cuentan la misma historia.
6. **exec-terminus (N6) — ausencia de emission A6 confirmada desde otra punta:** `count(arbx_risk_cb_state)` = **vacío** en Prometheus → la familia `arbx_risk_cb_*` del branch A6-CBPROM-01 no existe en prod (consistente con mi D2: grupo `circuit_breakers` solo en branch local, c498773c no-ancestro de main). Su propuesta de gauge `arbx_live_exec_enabled` no colisiona con nada existente.
7. **searcher-pipeline (N4):** su reporte está EN_CURSO (solo plan). Le regalo verificación (§3-Q1): el pipeline está VIVO (gauge 38.5s, heartbeat 60s con `redis_stream_delta=1`), pero con `scanner.rpc_timeout` en cadena (timeout 50ms, txs descartadas) y `price_worker.alchemy_failed` 429 en ráfaga — coherente con 5/6 proveedores http mainnet del searcher en breaker OPEN.

---

## 2. DESAFÍOS (contra-evidencia propia)

**E-1 → edge-gateway: "flota re-creada 23:29-23:33Z" — incompleto: fue un TERCER recreate a las 23:45:26Z.**
Su advertencia ("otros verificadores con evidencia pre-23:29Z auditaron el deploy anterior") aplica ahora contra SU PROPIA evidencia (probes 23:37Z = generación 2, muerta 8 min después). Total: **3 recreates en 47 min** (22:58, 23:33, 23:45:26) por la cascada #545→#543→#544. Contra-evidencia: AM `StartedAt=2026-09-06T23:45:26.610884205Z`, flota entera "Up 2 minutes" a las 23:48Z, VPS HEAD ya 9ac06d2d. Mi ronda 1 (reglas/targets muestreados en gen-2) también quedó superada — la re-verifiqué (§0: los resultados de targets/reglas/alertas son estables entre generaciones; lo que se pierde es el estado `for:`/pending y la continuidad de rate()).

**E-2 → data-layer: "ls-remote origin main → d4d3ff63… deploy veraz EXACTO" — cierto solo en su ventana de muestreo (pre-23:27Z).**
#544 se mergeó 23:27:05Z; a 23:48Z main = 9ac06d2d y el VPS YA está en 9ac06d2d. Su capa "remote_main MATCH" no era errónea, pero presentada sin timestamp indujo a leer "main == VPS == d4d3ff63" como estado estable — duró 18 minutos. Menor: conté **24** contenedores (`docker ps -q | wc -l`, 23:48Z) vs su "23/23" — probable artefacto del recreate en vuelo durante su muestreo; pido su comando exacto (Q-3).

**E-3 → data-layer, propuesta #4 ("añadir alerta de disco node_filesystem; el PR debe empezar por comprobar si existe"): la regla YA EXISTE y está MUERTA — el fix no es una regla, es la fuente de la métrica.**
`monitoring/alerts.rules.yml` (main, md5 verificado == runtime) contiene `PostgresDiskSpaceLow` y `GrafanaDiskSpaceLow` sobre `node_filesystem_avail_bytes/size_bytes`. Sonda viva: `count(node_filesystem_avail_bytes)` = **vector VACÍO** — no hay node-exporter en los 9 targets (api-server, prometheus, recon, relays-client, searcher-rs, selector-api, sim-ctl, token-enricher, edge). Las reglas llevan meses sin poder dispararse JAMÁS; el ENOSPC del 09-04 sin aviso está plenamente explicado. Además, aunque existiera el exporter, el regex `mountpoint=~"/var/lib/postgresql.*"` no matchea un volumen compose (host ve `/var/lib/docker/volumes/…` y todo cuelga de `/`). **Esto transforma su P1 en: desplegar node-exporter + reescribir regexes a mounts reales + umbrales 75/85.**

**E-4 → exec-terminus: "RPC pool chain-1 degradado a 5/9 (alchemy 429 monthly-capacity, llama CF-challenge, 0xrpc 404, 1rpc 403)" — mezcla dos planos distintos y el conjunto de proveedores no coincide con el estado runtime.**
Estado métrico real (23:49Z, `arbx_rpc_provider_state`): **searcher-rs** mainnet-http tiene 5/6 breaker OPEN (blockpi, publicnode, drpc, mevblocker, flashbots) y **alchemy state=0 (Healthy)**; **relays-client y recon reportan TODOS sus proveedores state=0**. Los proveedores llama/0xrpc/1rpc **no existen como serie** (fueron removidos del pool — MC-RPC-1). El "alchemy 429" que usted vio es la **API de PRECIOS** (`api.g.alchemy.com/prices/v1/...` 429 Too Many Requests, `price_worker.alchemy_failed` con fallback Coingecko — log searcher 23:24:18Z), no el pool RPC. Pido su línea de log fuente (Q-4): si su 5/9 venía de boot-logs de un pool viejo, el dato operativo correcto para el operador es el del métrico (5/6 open en searcher; pool vivo para relays/recon).

**E-5 → frontend-web, propuesta P1 ("sincronizar VPS a main + rebuild"): YA EJECUTADA por el ciclo de deploy automático a las 23:45:26Z.**
Verificado: VPS HEAD=9ac06d2d y el dominio público sirve `go-no-go-signoff-card` (1 hit, HTTP 200). Reclasificar de "acción del operador" a "resuelto — solo falta su re-verificación formal". El fondo de su propuesta (build sin identidad verificable, P0 build-SHA) sigue 100% vigente — esta noche lo demostró dos veces (su hallazgo y el deploy silencioso de 23:45).

**E-6 → mesa completa — hallazgo NUEVO de mi superficie que nadie reportó: 8 de 24 reglas muertas estructuralmente + 1 sospechosa.**
Sondas vivas (todas `count(<metrica>)` = vacío):
- `node_filesystem_avail_bytes` → **PostgresDiskSpaceLow, GrafanaDiskSpaceLow muertas** (E-3).
- `kube_pod_container_status_restarts_total` → **ContainerRestartLoop, ContainerExcessiveRestarts muertas**: son reglas de KUBERNETES en un deployment docker-compose (no hay kube-state-metrics ni k8s).
- `container_start_time_seconds` → **DockerContainerRestartLoop muerta también**: el "fallback para Docker Compose" usa métrica de cAdvisor y NO hay target cAdvisor. Un crash-loop real de cualquier contenedor sería invisible hoy.
- `arbx_revert_gas_wasted_usd`, `arbx_realized_pnl_usd`, `arbx_sim_predicted_profit_usd`, `arbx_actual_profit_usd` → **GasLossOnReverts, NetPnLNegative1h, SimVsActualVarianceHigh muertas** (TODO auto-declarado en el propio archivo: "metrics marked TODO are not yet emitted").
- Sospechosa: **SimulationFailureRateHigh `state: inactive`** pese a un régimen de ~100% `passed="false"` (37.9K/24h) — la expresión evaluada en vivo devuelve vector vacío a 23:49Z y en la generación 22:58-23:33 (33 min) tampoco disparó (mi ronda 1: solo RpcCircuitBreakerOpen activas). No pude fijar la causa (¿matching de labels `passed`/`simulator` entre numerador y denominador? ¿ventana rate() post-restart?) — fail-honest: requiere unit-test promtool (confiable post-#545) con el shape real de labels `{simulator,passed}`. Extiende el hallazgo del simulador (N5): no solo G-SIM-1 es un green engañoso — **la única alerta de tasa de fallo de simulación también está muda**.
- Observación menor: `RpcAllProvidersUnhealthy` hace `count(arbx_rpc_provider_state == 0) == 0` **sin agregación por servicio** — mientras relays-client/recon tengan un proveedor sano, la alerta no dispara aunque el pool del searcher muera entero.
Balance: de 24 reglas, hoy pueden señalizar ~15. La familia FREEZE-01 (PIPELINE_SILENCE/GAUGE_ABSENT/SCRAPE_DOWN, gauge fresco a 38.5s) y RpcCircuitBreakerOpen son la parte sana.

---

## 3. PREGUNTAS DIRECTAS

- **Q-1 → searcher-pipeline (N4):** su reporte quedó EN_CURSO. Datos que le faltan, ya medidos: pipeline VIVO (gauge insert 38.5s fresco; heartbeat 60s `redis_stream_delta=1`), `scanner.rpc_timeout` en cadena (timeout_ms=50) y `price_worker.alchemy_failed` 429 en ráfaga. ¿Completa el reporte y explica la **serie doble de publicnode en searcher-rs (state 0 Y state 2 simultáneos — ¿pool por cadena distinto)?** Es el único proveedor con dualidad y afecta cómo se lee el breaker.
- **Q-2 → api-ws (N3):** el gate de `/admin/alertmanager/webhook` — ¿exime alguna condición interna (IP docker/red privada) o exige `x-arbx-admin-token` incondicionalmente? Define la forma del fix (header en `webhook_configs.http_config` vs exención interna AM→api-server). Es su superficie de código; yo solo tengo el 401 del lado servidor.
- **Q-3 → data-layer (N7):** (a) ¿qué comando produjo "23/23" (yo cuento 24 con `docker ps -q | wc -l` a 23:48Z)? (b) ¿su proyección ENOSPC incluye el crecimiento del stack de monitoring en el mismo FS (Loki 1.0 GB, MinIO 0.96 GB, bloques Thanos — hoy chico pero presente)? (c) si el operador aplica su fix #1 (cron 2×/día = hasta 40M filas/día), el CHECKPOINT de pacing roto (su #3) pasa a ser crítico — ¿lo marca como bloqueante del #1 o como paralelo?
- **Q-4 → exec-terminus (N6):** ¿línea exacta de log/comando fuente del "alchemy 429 monthly-capacity / llama / 0xrpc / 1rpc"? Mi métrico muestra alchemy Healthy(0) en el pool del searcher y esos 3 proveedores sin serie (removidos). Si era el price-API 429 (api.g.alchemy.com/prices), conviene separarlo en su reporte: son dos riesgos distintos (pool de nodos vs feed de precios con fallback Coingecko).
- **Q-5 → simulator-family (N5):** ¿existe en sim-ctl alguna serie `arbx_simulation_total{passed="true"}` (aunque sea valor 0 desde boot)? Lo necesito para cerrar por qué `SimulationFailureRateHigh` está inactive (E-6) — el promtool test con el shape real de labels lo fijaría.
- **Q-6 → frontend-web (N1):** su hallazgo CSP report-only fue medido sobre la imagen de las 22:57Z; la imagen actual es de las 23:45Z (rebuild de #544). ¿re-verifica CSP + paridad en la generación nueva cuando toque? Y para su P0 build-SHA: propongo además exponer el SHA como **métrica info de Prometheus** (label del stack), no solo header/meta — así el drift deploy↔main se alerta desde MI superficie (ver P-6' abajo).

---

## 4. PROPUESTAS REFINADAS (con dependencias aprendidas del round-table)

| # | What (refinado) | Why / dependencia | Pri | Effort | Gate |
|---|---|---|---|---|---|
| P-1' | **Fix del webhook 401 (absorbe api-ws #5)**: header `x-arbx-admin-token` en `webhook_configs.http_config` de alertmanager.yml (token vía Vault/envsubst — el propio archivo ya documenta el mecanismo), O exención interna documentada AM→api-server. Post-fix: alerta sintética end-to-end y ausencia de 401 en logs | **P0 absoluto del round-table**: hoy 5 alertas activas + 24 reglas entregan a un 401 perpetuo (desde 2026-08-03). Sin esto, TODA mejora de alertas (data-layer, terminus, A6) muere en el mismo hoyo | **P0** | XS-S | `safe-production-observability` + evidencia de recibo + 0×401 en `docker logs api-server` |
| P-2' | **Anti dead-rules**: desplegar **node-exporter** (FS del host) + reescribir las 2 storage con mounts reales y umbrales 75/85 (absorbe data-layer #4); reemplazar las 3 de containers por fuente docker-native (cAdvisor target o `docker_stats` exporter); comentar/marcar las 3 TODO de mev-signals hasta que exista emisión | 8/24 reglas muertas por fuente inexistente (E-3/E-6) — el ENOSPC del 09-04 pasó inadvertido POR ESTO. Dependencia: P-1' primero (si no, las nuevas reglas tampoco llegan a nadie) | **P0** | M | PR con ID (P-∅) + promtool unit-tests (CI confiable post-#545) + alerta sintética de disco |
| P-3' | **Deploy coalescing + serialización** (1 ciclo por ventana, lock, verificación SHA anclado pre-siguiente-ciclo) | 3 recreates en 47 min (22:58/23:33/23:45:26) — confirma y agrava mi R2: alert-state y rate() se resetean por cada merge; #544 se deployó SOLO y silenciosamente (validó el hallazgo de frontend) | **P0** | M | G4 deploy-veraz §37 + e2e |
| P-4' | **Unit-test promtool de `SimulationFailureRateHigh`** con shape real `{simulator,passed}`; si es bug de expr → fix; si es semántica → documentar umbral | Regla muda durante régimen de ~100% fallo (E-6) — complementa el "G-SIM-1 green engañoso" del simulador: tampoco el warning-layer avisa | **P1** | S | CI promtool (#545) + P-∅ |
| P-5' | **Merge de A6-CBPROM-01** (grupo `circuit_breakers`, 24→29 reglas) **+ setear ARBX_CB_*/ARBX_RISK_* en el mismo ciclo** | `arbx_risk_cb_state` no existe en prod (count vacío). OJO aprendido del review-fix del propio branch: con env unset el steady-state es NOT_AVAILABLE(5) → `RiskCircuitBreakerNotConfigured` (warning, for:10m) va a disparar por diseño fail-honest — no es regresión, es la señal de "falta config del operador". Dependencia: P-1' (si no, las 5 alertas nuevas caen en el 401) | **P1** | S | CI Monitoring Config + promtool + operador setea thresholds |
| P-6' | **Identidad de build como métrica**: `arbx_build_info{sha}` (o similar) expuesta por cada servicio + alerta si SHA runtime ≠ main | Une mi superficie con el P0 de frontend: el drift deploy↔main se vuelve alertable; esta noche hubo 2 drifts invisibles (#544 ausente 18+ min… y el deploy POSTERIOR que nadie vio llegar) | **P1** | S | G5 + contract-test |
| P-7' | **Edge /metrics** (o quitar el target) + alerta de bucket 429 del edge | Mi D4 + edge-gateway D-1 (EDGE_AUDIT_TOKEN ausente, riesgo NR-0000): hoy el edge es el único target DOWN y su rate-limit es invisible a Prometheus | **P1** | S-M | contract defi intacto + `arbx-rpc-failover-discipline` N/A |
| P-8' | **Triage de los 5 RpcCircuitBreakerOpen persistentes** (3 generaciones) + decisión silences documentados; separar el riesgo "price-API 429" (feed con fallback) del riesgo "pool RPC" | R3 se agrava: persistencia multi-generación sugiere providers muertos de verdad, no half-open transitorio. Insumo para la pregunta E-4 al terminus | **P1** | S | `alchemy-rpc-robust-integration` |
| P-9' | Higiene: token cloudflared a EnvironmentFile (hallazgo compartido ×2: api-ws y yo); key-only SSH + fail2ban (brute-force 22:31-22:45Z); nota ops "thanos 2-weeks = diseño"; **redactar la API key de Alchemy que aparece en URLs de logs del price_worker** (visible en `docker logs searcher-rs` — no la reproduzco aquí) | Superficie de exposición activa, no teórica | **P2** | S | `security-auditor` |

**Orden propuesto al operador:** P-1' → P-2'/P-3' (paralelos) → P-5' → resto. P-1' desbloquea el valor de todas las demás alertas que este round-table propuso.

---

## 5. DECLARACIONES FAIL-HONEST

1. La causa exacta de `SimulationFailureRateHigh` inactive NO quedó determinada (E-6): expresión evaluada en vivo = vector vacío a las 23:49Z, inactiva también en la generación 22:58-23:33; hipótesis (matching de labels / ventana rate post-restart) sin probar — requiere el promtool test de P-4'.
2. El conteo "24 vs 23" contenedores (E-2) no lo pude reconciliar retroactivamente read-only; mi 24 es de 23:48Z con `docker ps -q | wc -l`.
3. No verifiqué la entrega POST-fix del webhook (el fix no existe aún); la afirmación "nadie recibe nada" cubre solo la cadena observada (AM → 401 api-server), no otros consumidores hipotéticos del AM (no hay otros receivers activos en la config md5-verificada).
4. Mi ronda 1 muestreó reglas/targets en la generación 23:33; los re-verifiqué a 23:48Z (idénticos), pero la continuidad de `rate()`/`for:` entre generaciones es irreconstruible — parte del precio del churn de P-3'.
5. Requests públicos: 3 de 5 acumulados (2 en ronda 1, 1 en este cross — `/live-readiness`). Internos 127.0.0.1 por SSH: fuera de presupuesto, ~20.
