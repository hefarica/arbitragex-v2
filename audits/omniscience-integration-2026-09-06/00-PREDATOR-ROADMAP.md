# 00 — PREDATOR ROADMAP · Síntesis final del round-table de integración

- **Autor:** agente sintetizador del round-table omniscience (read-only total; escritura solo en este directorio).
- **Fecha de cierre:** 2026-09-06T23:57:34Z (estampado git local: branch `a6-cbprom-01` @ `f7db6867`; `origin/main` = `9ac06d2d`; ref `main` local = `28d48cdd` stale).
- **Fuentes:** los 8 reportes de verificación + 7 reportes CROSS + `GOAL-WORKORDERS.md` + `WO-01-APPLY.md` + `WO-08-APPLY.md` + `n4-searcher-logs-tail200.txt` (leído vía destilación del CROSS de N4), todos en este directorio. Toda afirmación cita su reporte fuente; nada fue inventado (RULE 00 / R8).
- **Advertencia de ventana (afecta a TODO lo abajo):** la flota fue recreada **3 veces en 47 min durante la auditoría** (22:58:05Z, 23:33:07Z, 23:45:26Z — cascada de merges #545→#543→#544) y hubo una **4ª recreación ~00:1xZ SIN merge nuevo** (trigger no identificado read-only; 03-api-ws-CROSS §0.4). Cada recreación = feed público en 0 por ~5-7 min y data-plane caído decenas de segundos (07-data-layer-CROSS §0). Toda medición de esta mesa lleva timestamp implícito por ello.

---

## 1. ESTADO REAL DE INTEGRACIÓN

### 1.1 Veredictos por superficie × capa

| Superficie | LOCAL | REMOTE_MAIN | VPS | DOMINIO PÚBLICO | Veredicto global |
|---|---|---|---|---|---|
| **N1 frontend-web** | DRIFT (main local stale 28d48cdd; árbol compartido mutando por agentes) | MATCH 9ac06d2d | DRIFT→**RESUELTO 23:45Z** (d4d3ff63→9ac06d2d) | DRIFT (CSP 100% report-only; sin buildId/SHA en HTML) | **DEGRADED** |
| **N2 edge-gateway** | MATCH (edge/ byte-idéntico en f7db6867/9ac06d2d/d4d3ff63) | MATCH | MATCH (corre worker Hono canónico; B-02 resuelto) | MATCH (F-13 completo, RL vivo 594→579, latencia interna 5-8 ms) | **INTEGRATED** |
| **N3 api-ws** | MATCH (núcleo WS idéntico a main) | MATCH | MATCH (SHA veraz; boot limpio; 0 errores WS) | DRIFT (WS público degradado a HTTP-polling; viola RULE 02) | **DEGRADED** |
| **N4 searcher-pipeline** | MATCH (searcher-rs sin delta vs main) | MATCH | MATCH (+3 recreates) | N-A | **VIVO-DEGRADADO** (veredicto cerrado en CROSS: 15 días con 100% rechazo) |
| **N5 simulator-family** | MATCH (familia idéntica a main d4d3ff63) | MATCH (muestra pre-#544; stale sin efecto en la familia) | MATCH (SHA veraz; sim-ctl booteado, fork anvil vivo, era 501 TERMINÓ) | N-A (loopback-only) | **DEGRADED** (0 sims aprobadas en TODO el historial) |
| **N6 exec-terminus** | MATCH (relays-client sin diff vs main; default-deny en código L61-62/L84-86) | DRIFT menor (1 commit atrás, frontend-only, terminus idéntico) | MATCH (default-deny VIVO en runtime ×3 generaciones; capital=0) | N-A (loopback-only) | **INTEGRATED** (con salvedad: terminus *inerte*, jamás procesó un mensaje) |
| **N7 data-layer** | MATCH (scripts de retención == desplegados) | MATCH | **DRIFT** (disco 80%→76%, ENOSPC proyectado ~09-12/13) | N-A (0/5 requests) | **DEGRADED** |
| **N8 monitoring-fleet** | DRIFT (reglas circuit_breakers 24→29 SOLO en branch local) | MATCH | DRIFT (churn de deploy; 5 breakers RPC; 8/24 reglas muertas) | MATCH (200 OK; TTFB 5.9 s cold-start) | **DEGRADED** |

**Balance: 2 INTEGRATED, 5 DEGRADED, 1 VIVO-DEGRADADO.** Ninguna superficie ROTA: la flota 23/24-contenedores (discrepancia 23 vs 24 no reconciliada, ver §1.4) está healthy y sirviendo datos reales y frescos (lag PG 51 s; edge interno count:50 con items reales).

### 1.2 Los SHAs, reconciliados (estado al cierre 23:57Z)

| Referencia | SHA | Estado |
|---|---|---|
| GitHub `origin/main` | `9ac06d2d` (#544 A9-GONOGO) | Punta canónica |
| VPS `/opt/arbitragex-v2` | `9ac06d2d` (desde 23:45:32Z) | **DEPLOY VERAZ RESTAURADO** — el drift "dominio 1 PR detrás" se auto-cerró por la cola de auto-deploy; el dominio público sirve `data-slot="go-no-go-signoff-card"` (1 hit, HTTP 200; 06-exec-terminus-CROSS §0, 08-CROSS §0) |
| Árbol local compartido | `f7db6867` (branch `a6-cbprom-01`) | = main + feature **A6-CBPROM-01 no mergeada** (`c498773c` NO es ancestro de main; reglas circuit_breakers 24→29 solo aquí) + chore merges |
| Ref `main` local | `28d48cdd` (#531) | **STALE ~14+ commits** — "build desde main local" daría una app más vieja que la live (01-frontend-web §1) |
| Diffs sin commit en el árbol | `M frontend/lib/websocket-client.ts` (+63, WO-01) · `M frontend/components/RuntimePostureBar.tsx` + test (+44/−4 y +89, WO-08) · `?? websocket-client.test.ts` | Oleada 3 de la Remediation Squad, viva en el árbol compartido — se pierde en el próximo checkout si no aterriza por PR (§36) |

**Corrección del hallazgo semilla (fail-honest):** "#545 perdido del bare / el fix existe SOLO en el VPS" es **FALSO** — `merge-base --is-ancestor` en ambos sentidos prueba que `4cb807d2` (#545) es ancestro de `d4d3ff63` (VPS) y de `9ac06d2d` (main); md5 de `alerts.rules.yml` idéntico local↔main↔VPS↔contenedor Prometheus (08-monitoring §1, §4). Y "remote github roto" no es un remote roto: es **deriva documental de RULE 01** — la regla documenta `origin`=VPS bare + `github`=GitHub, pero tanto el clone Windows como el repo VPS usan `origin`=GitHub como único remote (07-data-layer §2.1, 08 §5.6). El flujo real es GitHub→VPS-pull; el documento describe una topología que ya no existe.

### 1.3 Drifts medidos que importan (cifras, no adjetivos)

| # | Drift | Medida | Fuente |
|---|---|---|---|
| D-1 | **CSP 100% report-only en producción pública** | Única política = `content-security-policy-report-only` con `unsafe-inline`/`unsafe-eval` baked-in (`next.config.js:149`); byte-idéntica origen↔dominio; **re-verificada en el build 9ac06d2d** post-#544 | 01 §1/§3, 01-CROSS A-5, 06-CROSS §0.3 |
| D-2 | **Disco estructural**: `/` 80% (23:20Z) → 78% → 76% (23:45Z, cleanup de imágenes); `pg_database_size` 59-60 GB; driver = `route_discovery_outcomes` 42 GB / 86.1M filas a ~30M/día vs cap de purge **20M/día** → desde la corrida del 09-07 backlog **+10M filas/día (~+5 GB/día) → ENOSPC ≈ 09-12/13** | 07 §2.3, 07-CROSS §2.4 |
| D-3 | **Churn de deploy sin registro**: 3 recreates/47 min; el ciclo de hoy NO dejó `/tmp/deploy*.log` (los clásicos stale desde 08-29); 4º recreate ~00:1xZ sin merge (trigger sin identificar); load pico 35.54 sobre 8 vCPU durante builds en el mismo host del hot-path | 08 §2, 03-CROSS §0.4, 06-CROSS §0.2 |
| D-4 | **Canal de alertas MUERTO**: Alertmanager rutea TODO a `http://api-server:8080/admin/alertmanager/webhook` SIN auth (`alertmanager.yml:44-48`) y el gate exige `x-arbx-admin-token` → **401 cada 5:00 min exactos** (6 en 27 min; reproducido en la generación nueva 23:46:46Z). 0 alertas entregadas por cualquier canal mientras 5 `RpcCircuitBreakerOpen` están activas | 03 §3, 03-CROSS D1, 08-CROSS §1.1 |
| D-5 | **8/24 reglas de Prometheus muertas estructuralmente** (fuente de métrica inexistente: sin node-exporter ni cAdvisor; 3 reglas TODO sin emisión) + `SimulationFailureRateHigh` inactiva pese a ~100% `passed="false"` (37.9K/24h) | 08-CROSS E-3, E-6 |
| D-6 | **0 sims aprobadas en TODO el historial**: `simulations` 1.000.718 filas, `passed=f` al 100%; `XLEN arbx:opps:simulated`=0; `paper_trade_runs` 598.878 filas congeladas desde 09-01 16:32, labels=0 | 05 §1.3, 07-CROSS §1.1 |
| D-7 | **El embudo detecta solo ruido**: 57.981/24h detecciones, **100% rejected 15 días consecutivos** (última no-rechazada 2026-08-22 16:52Z); flood AGLD 37.2% + XEN 34.0% + PEPE 25.8% = **97.0%** en self-pairs (token_in==token_out, no-op matemático) amplificados ×28 por la matriz de cartuchos | 04-CROSS §"Evidencia propia" |
| D-8 | **Cables muertos**: `HotPathEmitter` 0 call-sites (streams `arbx:hot:*` XLEN=0 perpetuos, confirmado por 3 verificadores); mismatch de eventos WS `new_opportunity`↔`opportunity:detected` en `websocket-client.ts` (lib con **0 consumidores**; el feed productivo `socket-lifecycle.ts:81` SÍ escucha `new_opportunity` — severidad de usuario refutada) | 03 §5, 01-CROSS C-1/C-2, 05-CROSS §2 |
| D-9 | **Fuga sistémica de consumer-groups**: 5 grupos, ~235-285 consumers huérfanos (relays-client-g0 58, ws-emitter-g0 62, sim-ctl-g0 51, paper-archiver-g0 63, selector-g0 59), +1/boot; `relays-client-g0` con `last-delivered-id 0-0` = **jamás consumió en su historia** | 03-CROSS C4, 06-CROSS N-5, 07-CROSS §2.2 |
| D-10 | **Trim-antes-de-consumo**: lag de grupos (10,428-10,548) > length del stream (10,001-10,004) en `arbx:opps:detected` → ~488→547 entradas recortadas sin consumir (déficit ~14% del flujo); hoy inerte (100% rejected), crítico post-flip | 07 §2.3-g, 07-CROSS §2.3/§2.5 |
| D-11 | **WS público degradado a HTTP-polling** (rewrite de Next es HTTP-only; nginx :80 con upgrade nativo está FUERA de la ruta dominio) — viola RULE 02 | 03 §4, 01-CROSS A-2 |
| D-12 | **Terminus de broadcast vacío**: relay catalog=0, `FLASHBOTS_SIGNER_KEY` ausente, Vault SEALED (shamir 3/2) con healthcheck `vault status \|\| true` que lo reporta "healthy" — observabilidad mentirosa en la frontera capital | 06 §3, 06-CROSS §1 |
| D-13 | **Builds sin identidad**: HTML sin buildId/SHA (grep 0 hits), `api-server /health` version 0.1.0 sin SHA, `sim-ctl /capabilities build.sha=null` — 4 verificadores tropezaron con el mismo hueco | 01 §1/§4, 03-CROSS C6, 05 D-10, 06-CROSS P-11 |
| D-14 | **RPC degradado**: searcher-rs mainnet-http con 5/6 breakers OPEN persistentes en 3 generaciones (blockpi, publicnode, drpc, mevblocker, flashbots); `scanner.rpc_timeout` timeout_ms=50 en cadena; API de **precios** de Alchemy en 429 persistente (135/12 min) con fallback Chainlink/Coingecko vivo | 08 §5.3, 08-CROSS E-4, 04-CROSS §"Evidencia propia" |
| D-15 | **Sin build-SHA ni registro, los drifts son invisibles**: el dominio sirvió 18+ min sin #544 sin señal alguna; luego #544 se deployó solo y silenciosamente a 23:45Z y nadie lo vio llegar | 01 §4, 08-CROSS E-5 |

### 1.4 Discrepancias adjudicadas y huecos declarados (R8)

- **count:50 (23:25Z interno) vs count:0 (23:37Z público) vs `internal_error` (23:44:56Z) vs count:3 (23:46:10Z)** — ADJUDICADO: gaps de INSERT en PG exactamente en los minutos 23:31-23:37 y 23:44-23:45 calzan con los redeploys (2 únicos gaps >300 s en 3 h, máx 7m30s; m5=117 a 23:46:36Z). Cada recreate = feed público en 0 por ~5-7 min. El `internal_error` fue el edge respondiendo con su upstream muerto (01-CROSS C-3, 04-CROSS desafío 1).
- **"23/23" (N7) vs "24/24" (N8) contenedores** — NO reconciliado read-only; irrelevante para conclusiones, declarado para que el operador no cite ninguna como canónica (06-CROSS §3.5, 08-CROSS E-2).
- **"Alchemy 429 monthly-capacity, 5/9 providers"** — REFINADO: el 429 es la API de PRECIOS (`api.g.alchemy.com/prices/v1`), no el pool de nodos; el métrico muestra alchemy Healthy en el pool y llama/0xrpc/1rpc REMOVIDOS del pool (sin serie). El estado operativo correcto: 5/6 breakers OPEN en el searcher; relays/recon reportan sus providers state=0 (08-CROSS E-4).
- **`n4-searcher-logs-tail200.txt`**: 200 líneas = 5.6 s de reloj del contenedor (violación R9 viva); su contenido se consumió vía la destilación del CROSS de N4 (heartbeats 60 s con `pending_received=491/570`, `redis_stream_delta=1-2`, `pg_period_inserted=57`).
- **No verificado por nadie**: entrega real de notificaciones post-fix (el fix no existe aún), ingress exacto de Cloudflare (token-managed), código on-chain de las direcciones de executor, comportamiento del navegador real (nadie navegó con Playwright esta ronda).

---

## 2. P0 — CERRAR DRIFTS DE INTEGRACIÓN

Ordenado por dependencia. **Owner** = quien ejecuta (los agentes NO mutan VPS: §32/§33; todo lo del VPS es acción del operador o PR con deploy estándar).

### P0-1 · Identidad de build en TODA la flota (el meta-fix)
- **Acción:** UN PR con build-arg/ENV `GIT_SHA` horneado y expuesto en 4 superficies: frontend (`NEXT_PUBLIC_GIT_SHA` + header `x-arbx-build-sha`), sim-ctl (`/capabilities build.sha` vía `option_env!`), api-server y edge (`/health {commit}`); idealmente también como métrica `arbx_build_info{sha}` para que el drift deploy↔main sea ALERTABLE (08-CROSS P-6').
- **Gate:** G4/G5 deploy-veraz — post-deploy `curl` del header/endpoint == `git rev-parse HEAD` en VPS, automatizado.
- **Riesgo si no se hace:** esta noche hubo DOS drifts invisibles (#544 ausente 18+ min sin señal; y el deploy posterior que nadie vio llegar). Todo gate "deploy veraz" es hoy una forense manual de 30 min. **Es el habilitante de P0-5.**
- Owner: PR (P-∅ §37). Effort S. Fuentes: 01 propuesta 1, 03-CROSS #6, 05-CROSS #3, 06-CROSS P-11.

### P0-2 · Higiene git del clone compartido + aterrizaje de la Oleada 3
- **Acción:** (a) actualizar ref `main` local a `origin/main` (9ac06d2d) y congelar/anunciar checkouts del árbol compartido durante verificaciones (§36); (b) aterrizar WO-01 + WO-08 por PR con ID **junto con la decisión promote-or-retire de `websocket-client.ts`** (WO-01 parchea una lib con 0 consumidores: sin decisión deja DOS clientes WS con contratos divergentes — la trampa que ya desvió a un verificador); (c) corregir RULE 01 (origin=GitHub; el flujo VPS-bare documentado no existe) o restaurar el remote bare si se prefiere esa topología.
- **Gate:** PR con ID (P-∅); CI verde; disciplina §36 anunciada.
- **Riesgo si no se hace:** el diff WO-01/WO-08 se pierde en el próximo checkout de un agente (ya pasó 3 veces hoy según reflog); "código local" deja de significar una sola cosa; el próximo agente construye sobre `main` local stale (28d48cdd) creyendo que es main.
- Owner: agentes (PRs) + operador (habilitar commit). Effort XS. Fuentes: 01 §1, 03-CROSS D2, 01-CROSS C-2/propuesta 5, 07 §2.1.

### P0-3 · Disco: purge ≥ inserción + reclamar build-cache ANTES de la corrida del 09-07 04:17Z
- **Acción (operador, minutos):** (a) `ARBX_RETENTION_MAX_ROWS=40000000` (o cron `17 4,16 * * *`) — el cap 20M/día < inserción 30M/día hace que la corrida del 09-07 sea la PRIMERA deficitaria (+10M filas/día ≈ +5 GB/día netos); (b) `docker builder prune -af --keep-storage 5GB` (20.65 GB reclaimable medidos con ACTIVE=0 — +60% de margen inmediato) y fijarlo en el cron semanal con `-a`.
- **Acción (PR):** fix del bug VACUUM/CHECKPOINT en `scripts/pg_retention.sh` (multi-statement en un solo `psql -c` = transacción implícita → `retention.vacuum` falla TODAS las corridas desde 09-04; el CHECKPOINT de pacing WAL usa el mismo patrón roto silenciado con `|| true`). **Bloqueante si se sube el cap a 40M**: sin pacing, se repite el WAL-burst→ENOSPC del 09-04.
- **Gate:** operador (cron/env) + PR con ID para el script; verificación: corrida de retención `complete` con deleted ≥ 30M y `pg_wal` acotado.
- **Riesgo si no se hace:** ENOSPC ≈ 09-12/13 → crash-loop de PG (precedente 09-04) → cae el stack MONOLÍTICO entero (23-24 contenedores en el mismo FS). Es el único P0 con fecha de vencimiento.
- Fuentes: 07 propuestas 1-3, 07-CROSS §2.4/§5, 08-CROSS Q-3c.

### P0-4 · Reparar el canal de alertas (webhook 401) — la precondición de TODA la observabilidad
- **Acción:** agregar `http_config.authorization` (token `x-arbx-admin-token`) al receiver `default` de `monitoring/alertmanager/alertmanager.yml` — token por el canal vault-agent→envsubst (el propio archivo documenta el mecanismo en L6-17; mientras Vault no esté operativo, env interino documentado). Post-fix: alerta sintética end-to-end con recibo evidenciado + 0× 401 en logs. Alternativa a decidir: exención interna documentada AM→api-server (pregunta abierta de N8 a N3).
- **Gate:** `safe-production-observability` + evidencia de recibo.
- **Riesgo si no se hace:** 5 `RpcCircuitBreakerOpen` activas + 24 reglas entregan a un 401 perpetuo desde 2026-08-03. **Nadie recibe NADA** (ni firing ni resolved). Sin esto, cualquier alerta nueva (disco, A6 breakers, flip de live-exec) muere en el mismo hoyo: por eso va ANTES que las demás mejoras de alertas.
- Owner: PR config + operador (secrets T1 §33). Effort XS-S. Fuentes: 03-CROSS D1/propuesta 2, 08-CROSS P-1'.

### P0-5 · Deploy coalescing + serialización + registro del ciclo
- **Acción:** debounce de merges a main (1 deploy por ventana, ej. 10-15 min), lock de deploy, verificación SHA-anclado (consume P0-1) antes de iniciar el siguiente ciclo, y log persistente del ciclo (el de hoy NO dejó `/tmp/deploy*.log`). Incluir investigación del **4º recreate sin merge** (~00:1xZ, origin/main congelado en 9ac06d2d) — un recreate sin causa identificada es inaceptable en la frontera de capital. Deseable: edge responde `503 Retry-After` honesto durante restarts en vez de `internal_error` opaco.
- **Gate:** G4 deploy-veraz + e2e post-deploy.
- **Riesgo si no se hace:** cada merge = ~30 min de ciclo + recreate de 21 servicios + feed público en 0 por 5-7 min + data-plane caído decenas de segundos + reset del estado `for:`/pending de TODAS las alertas + load 35 sobre 8 vCPU contra el hot-path + regeneración de build-cache que alimenta el ENOSPC de P0-3. 3 merges/hora = flota en recreate perpetuo.
- Owner: PR (deploy.sh/workflow) + operador. Effort M. Fuentes: 08 P-1, 08-CROSS P-3', 06-CROSS §0.2, 01-CROSS propuesta 6, 03-CROSS #9.

### P0-6 · CSP enforcing en 2 fases (re-verificado vigente en el build 9ac06d2d)
- **Acción:** (a) duplicar header `content-security-policy` idéntico junto al report-only 48-72 h monitoreando NEL de Cloudflare; (b) retirar report-only; en paralelo migrar a nonce para eliminar `unsafe-inline`/`unsafe-eval` (WO-09). NO aplicar enforcing a ciegas (rompería prod: templates inline).
- **Gate:** security-auditor + smoke e2e (Playwright) post-deploy + 0 regresiones en el feed de reportes NEL.
- **Riesgo si no se hace:** protección XSS por CSP = 0 en el dominio público; el propio comentario del código ("switch once report stream clean ≥7 días") lleva meses vencido.
- Owner: PR + operador (ventana de deploy quieta — depende de P0-5). Effort M. Fuentes: 01 propuesta 3, 01-CROSS A-5, 06-CROSS §0.3.

---

## 3. P1 — REMEDIOS HG: de certificación 0/10 a un sistema que DETECTA y EVALÚA de verdad

**Punto de partida (certificación HG 2026-09-06: 0/10 gates PASS, 8 FAIL + 2 de doctrina):** 992K sims 0 passed (hoy medido con más precisión: **1.000.718 filas, passed=f al 100%**) · relays 0/2 (Eden/Beaver inexistentes; hoy relay catalog=0 confirmado en runtime) · `executor/` ~350 líneas jamás compiladas (WO-05) · Vault SEALED decorativo · G7 dirty 4/5 con knob-OFF como el más cercano. **Causa raíz nueva, medida esta noche:** el sistema lleva **15 días con 100% de rechazo** y el 97% del volumen es un no-op estructural (self-pairs de 3 tokens spam ×28 por la matriz de cartuchos). Ningún remedio aguas abajo converge mientras el embudo sea ruido generado por el propio sistema.

**Secuencia con dependencias probadas** (invertirla produce streams muertos o alertas que nadie recibe — 05-CROSS propuesta 6):

1. **Fix canal de alertas (P0-4).** Prerrequisito: sin él, la alerta de fallo de simulación y los 10 breakers doctrinales no llegan a nadie.
2. **Filtro anti-flood estructural en el searcher, ANTES de la emisión** (sube de prioridad: era WO-06/Oleada 4). Un self-pair (`token_in==token_out`) es un no-op matemático: debe morir en el loop de evaluación, junto con dedup por (token, ventana) y tiering por liquidez — **criterio DINÁMICO** (el flood rota: PEPE 25.8% es nuevo; hardcodear XEN+AGLD nace obsoleto; RULE 00). Efecto cascada: desahoga el purge (parte de los 30M/día de RDO es eco del flood ×28), reduce WAL/ENOSPC, mejora señal del flip revm y el ruido de alertas.
   **Gate:** PR con ID; invariante XLEN delta=0 en entradas legítimas; fixtures AGLD/XEN/PEPE; unit tests. **Riesgo si no:** el flip revm procesa ~40× ruido de más (agrava drain-guard y cobertura).
3. **Flip operador a simulación route-aware real:** `SIM_BACKEND=revm` + `REVM_RPC_URL` en `.env` del VPS + cablear ambas en `docker/compose.prod.yml` (hoy NO están cableadas en ningún docker/*.yml) + redeploy con RULE 03. **RPC DEDICADO de pago obligatorio** — prohibido apuntar al pool actual (5/6 breakers open; drpc pending sostiene el fork anvil). El binario YA contiene simulator-v2/revm 42.0.1; migraciones 112/113 confirmadas live; drain-guard protege el boot (fail-loud, consume nothing). Es el único path que aprueba sin inventario pre-fondeado (TLS step 0).
   **Gate:** §34.3 flips = operador-only con autorización explícita; `arbx-simulation-mandatory` + `arbx-paper-trade-first` PASS previos; **gate data-layer bloqueante** (ver 5); post-flip: primer `passed=t` en `simulations` + `XLEN arbx:opps:simulated` > 0 + primer evento `relay_sim.no_submit` en relays-client (cierra el lazo A.7).
   **Riesgo si no:** 0 labels perpetuos → calibración imposible → paper-shadow corre a ciegas de feedback (R-1 de N5).
4. **Blindaje del flip (data-layer + terminus):** (a) consumer verificado sobre `arbx:opps:simulated` (el grupo `relays-client-g0` tiene `last-delivered-id 0-0` — jamás consumió; el flujo post-flip ejerce POR PRIMERA VEZ el camino consumer→persistence→paper_trade_runs, un path runtime nunca probado); (b) alerta `lag > length` en TODOS los streams `arbx:opps:*` (el hermano `detected` ya pierde ~14% del flujo por trim-antes-de-consumo); (c) ventana de observación del terminus en la primera hora (persist_err, DLQ `dlq_max_retries=3`, rate de paper_trade_runs); (d) aclarar quién escribió las 598.878 filas históricas del ledger (este grupo no fue — comparabilidad post-flip no garantizada).
5. **Cobertura de estrategias del path b2c:** auditar qué `strategy_kind` emite el searcher (`mev_01_*/mev_04_*` ~100% del mix actual) vs las que simulator-v2 resuelve — `strategy_not_simulatable_in_s4` = 37.3K/24h (~98% del fallo reciente) no entra a simulación. Patrón generated-table+probe del repo.
6. **Señales honestas (anti-green-tramposo):** G-SIM-1 (o gate hermano G-SIM-2) debe exigir passed-rate con ventana 24 h (`increase(...[24h])` con `passed>0`) — hoy green mide FLUJO con 0 aprobaciones históricas; reescribir `SimulationFailureRateHigh` igual (la ratio `rate[5m]` flapea con ráfagas y jamás dispara; window 24 h es inmune al burst). Misma familia: `pg_period_profit_pos` del heartbeat cuenta polvo 7e-12 USD como "positivo" — corregir o eliminar. **Gate:** P-∅; unit-test promtool con shape real `{simulator,passed}` (CI confiable post-#545).
7. **Aterrizar A6-CBPROM-01 por PR** (grupo `circuit_breakers` 24→29 reglas; los 10 breakers doctrinales de riesgo NO alertan en producción hoy) **+ setear `ARBX_CB_*/ARBX_RISK_*` en el mismo ciclo** (con env unset, `RiskCircuitBreakerNotConfigured` disparará por diseño fail-honest — es señal de "falta config del operador", no regresión). **Dependencia:** P0-4 primero.
8. **Anti dead-rules:** desplegar node-exporter (FS del host) + reescribir las 2 storage con mounts reales y umbrales 75/85; reemplazar las 3 de containers por fuente docker-native (cAdvisor); comentar las 3 TODO de mev-signals. Hoy 8/24 reglas son estructuralmente incapaces de disparar — el ENOSPC del 09-04 pasó inadvertido POR ESTO.
9. **Wiring/limpieza del hot-path (WO-02, DESPUÉS del flip):** `HotPathEmitter` (0 call-sites, streams XLEN=0, ~120 consumers huérfanos entre ws-emitter-g0 y los grupos de streams vacíos) — conectarlo en `opportunity_emitter.rs` (publisher central, NO cartridge_boot) y emitir POST-validación (emisión en detección cruda solo traslada el flood al browser); o eliminar el cable muerto. Junto con WO-01+WO-08 ya aplicados localmente (ver P0-2) y la decisión promote-or-retire de `websocket-client.ts`.
10. **Higiene sistémica de consumer-groups:** UN PR en `shared-rs` (DELCONSUMER en shutdown + GC idle>24h) para los 5 grupos (~235-285 huérfanos) — no 4 parches por servicio. Sin deploy-coalescing (P0-5) la fuga crece +N por deploy.
11. **Onboarding de relays (catálogo 0 → ≥1, p.ej. Flashbots protect)** vía `POST /admin/relays` — requisito previo a testnet-live; junto a la reconfiguración de cadena del terminus (bootea chain-1-configurado SIN pool por-cadena; el flip Sepolia exige cfg.chain_id o pool por-cadena, no solo flags — N-4 de 06-CROSS).
12. **Signer vía Vault + hardening del terminus** (P0 de N6): provisionar el signer con unseal controlado por operador + vault-agent → `/run/secrets/arbx.env` (el plan T0 del compose existe solo como comentario); healthcheck honesto de vault (sealed≠ready); gauge `arbx_live_exec_enabled{chain}` + alerta si ≠0 y si vault pasa a unsealed fuera de ventana + audit de cambios de `arbx:papermode:*`; probe E2E runtime de `MainnetRefused` en staging; normalizar `ARBX_LIVE_EXEC_ENABLED=False`→`"false"` (contrato `"true"` exacto documentado); fijar `arbx:papermode:11155111` EXPLÍCITO en el checklist pre-testnet (hoy el freno Sepolia es config-default).
13. **R9/LOGFLOOD (dos frentes):** `paper_archiver.skip_rejected` → debug + histograma (895 líneas/27 min; --tail 150 cubre 48 s) y `active_eval_enter` del searcher → debug (141 líneas/5.6 s ≈ 25/s a INFO; 200 líneas de log = 5.6 s de reloj — forense imposible).
14. **WS nativo en la ruta pública (RULE 02):** ingress CF para `/socket.io` → nginx :80 (ya activo con la ruta) o hostname separado → 127.0.0.1:8080. Verify con socket.io-client real (curl muere en ping/pong). Desplegar en ventana quieta (depende de P0-5). En paralelo: `ALLOWED_ORIGINS=https://arbx.ape-tv.net` + `EDGE_AUDIT_TOKEN` en env del edge (completa #539: bucket auditor hoy inerte, fail-closed).
15. **Los literales económicos vivos (WO-04) y el código fantasma (WO-05):** tip 2 gwei (`bundle_builder.rs:171`), `GAS_COST_USD=30`, 30bps — parametrizar con config declarativa validada / lectura on-chain; decidir integrar-vs-eliminar `mod executor` (~350 líneas sin compilar), `NonceManager::refresh`, Eden/Beaver (0 líneas). **Sin fees reales, el sizing Kelly calcula contra un mundo imaginario.**

---

## 4. P2 — VENTAJA PREDATORIA: qué tiene este sistema vs los searchers profesionales de mainnet

**Framing honesto (sin promesas de ingresos — el mercado captura el top-1% y el flujo 90%+ es privado; margen sub-dólar/evento):** hoy NO compite en los ejes donde compiten los profesionales. La brecha medible:

| Eje | Searcher profesional mainnet | Este sistema HOY (medido) | Qué cierra la brecha |
|---|---|---|---|
| **Latencia** | Nodo propio/colocado, suscripción bloques/txs directa, envío a relay en ms | WS público **degradado a HTTP-polling** (D-11); latencia edge interna 5-8 ms pero latencia detección→broadcast **NO instrumentada** (WO-10/MN-006); scanner con timeout_ms=50 descartando txs; kill-switch "<10 ms" autodeclarado sin benchmark (WO-11) | Instrumentar percentiles E2E (WO-10) + benchmark reproducible del kill-switch (WO-11) + upgrade WS nativo + RPC dedicado. Solo entonces la cifra existe y se puede optimizar |
| **Routing** | Motores CEX-DEX/multi-venue optimizados con feedback real | **Matriz 264 cartuchos × 31 operadores matemáticos** (8.184 relaciones, mode-invariant §34.1), route discovery multi-path, SizeOptimizer como motor económico, Kelly caps landed — pero **0 labels en la historia = todo el aparato corre sin feedback**; calibración bayesiana inerte (flat prior, 0 claves `*calib*`; el writer κ=20/backoff vive en un branch sin merge — WO-07) | El funnel de P1 (flip revm → labels → calibración). La matriz es potencial diferencial REAL; sin labels es un piano sin partitura |
| **Relays privados / stealth** | Flashbots/bloxroute/Titan/Eden/Beaver directos, orderflow privado, ofuscación | **Relay catalog = 0** (NotSubmitted/501), cero mempool privado cableado, feed por RPCs públicos con 5/6 breakers open; "Ghost Protocol" es doctrina, no runtime | Onboarding de relays (P1-11) + `arbx-mev-ethics-gate`/skills MEV del arsenal. El dark-routing doctrinal §4.4 no tiene cable vivo |
| **Sizing con fees on-chain** | Fees leídas del bloque/gas real, sizing por simulación continua | Kelly + SizeOptimizer EXISTEN, pero los literales económicos están **hardcodeados** (tip 2 gwei, GAS_COST_USD=30, 30bps — WO-04) y el "exact gas" nunca se midió (0 ejecuciones) | WO-04 parametrización + fees on-chain post-flip. El sizing honesto exige simular contra el bloque real (el fork mainnet de anvil ya vive para eso) |
| **Infraestructura** | Bare metal colocado, redundante, builds fuera del hot-path | 1 VPS (8 vCPU, 15.6 GB); builds `--no-cache` de flota completa EN el mismo host (load 35 vs hot-path); túnel CF como único ingreso | Aislar builds (builder remoto/límites/blue-green — N8 P-5); el hot-path nunca debe compartir CPU con el build |
| **Capital/frontera de ejecución** | — | **AQUÍ está la ventaja real y medida:** default-deny vivo en runtime ×3 generaciones; `MainnetRefused` INCONDICIONAL para chain 1 (incluso si se lista explícitamente; tests de regresión del "triple peligro"); doble cerradura estructural (terminus bootea chain-1-configurado vs política Sepolia-only → broadcast imposible sin reconfiguración deliberada); barrera runtime re-leída POR llamada; sin signer en contenedor; Vault sellado; `assert_broadcast_allowed` como PRIMERA sentencia de `build_and_sign` | Ya está construida. Es el activo que el propio informe externo destacó como la mayor fortaleza real del sistema |

**Ventajas diferenciales reales ya en posesión** (medidas, no aspiracionales): (1) la frontera de capital fail-closed descansa arriba; (2) **telemetría honesta radical** — RULE 00/R8: 100% del rechazo registrado con razón exacta, `state:"no_ledger"` sin fingir ledger, vacío honesto en el feed — la mayoría de los sistemas de retail no tienen esto ni en paper; (3) simulación-first obligatoria (revm 42 in-process ya en el binario, solo falta el flip) con drain-guard fail-loud; (4) matriz de estrategias 264×31 mode-invariant lista para calibrar; (5) stack de observabilidad completo (Prometheus/Alertmanager/Grafana/Loki/Thanos) — hoy sordo en parte (D-4/D-5), pero existe; (6) disciplina de retención/rollup y ledger paper auditora­ble.

**Traducción predatoria:** cuando P0+P1 cierren, este sistema no gana por velocidad (llega tarde a ese juego) sino por **cobertura matemática calibrada + honestidad de costos + frontera de capital institucional**: detecta familias de asimetría topológica que los searchers monotarea no miran (264 cartuchos), las evalúa contra el bloque real (fork+revm), las dimensiona con Kelly sobre fees medidos, y puede operar en paper/testnet/mainnet SIN cambiar la matemática (§34.1). Ese es el plan; hoy es un paper-platform con rieles institucionales, y la brecha hasta readiness es la lista de §5 — no un número de ingresos.

---

## 5. CRITERIOS DE ÉXITO PARA EL OPERADOR — checklist del flip §34.3 a LIVE_MAINNET

**Regla innegociable primero:** el flip es decisión del **OPERADOR con autorización explícita** (§34.3: "no inferida de flags ni de chat"). Los agentes **JAMÁS** lo ejecutan, ni lo recomiendan ejecutar sin gates PASS, y el default-deny + `MainnetRefused` **NO se remueven** sin los tres puntos de §34.3 satisfechos. Precedente: live-flip-via-chat RECHAZADO (2026-08-29). El camino canónico es **gradual**: paper → Sepolia (11155111) → mainnet.

La lista exacta que debe estar VERDE (todo medible con evidencia, no auto-declarado):

**A. Doctrina y autorización**
- [ ] §32/§33 satisfechos (política permanent audit/scaffold → promoción explícita).
- [ ] Autorización operativa explícita y documentada del operador (no inferida de flags ni de chat).

**B. Gates de skill obligatorios PASS**
- [ ] `arbx-paper-trade-first` — PASS con historial de paper runs REALES (hoy: 0 filas/24h, congelado desde 09-01).
- [ ] `arbx-simulation-mandatory` — PASS (hoy: 0 aprobadas en 1.000.718 sims).
- [ ] `arbx-risk-limits-enforcement` — PASS con los 10 breakers doctrinales EMITIENDO y ALERTANDO (hoy: rama A6 sin merge, `arbx_risk_cb_state` sin serie en prod).
- [ ] `arbx-pre-execute-checklist` — PASS íntegro.
- [ ] (Dependientes de ruta TLS: `arbx-flash-loan-discipline`, `arbx-net-profit-gate`, `arbx-contract-atomicity-rules`.)

**C. Función económica demostrada (la que hoy no existe)**
- [ ] Filtro anti-flood vivo: self-pairs no llegan a emisión; flood-share observable en métricas del searcher (hoy 97%).
- [ ] Flip revm operado por el operador; primer `passed=t` en `simulations`; `XLEN arbx:opps:simulated` > 0 con `lag < length` sostenido 24 h (stream de salida con lector verificado).
- [ ] Primer evento `relay_sim.no_submit` procesado en relays-client (el grupo jamás consumió: `last-delivered-id 0-0`).
- [ ] `paper_trade_runs` escribiendo de nuevo con labels (`actual_timestamp` NOT NULL) y `sim_attempts` > 0 (drift_tracker ON).
- [ ] G-SIM-1/2 green por passed-rate (24 h), no por flujo; `SimulationFailureRateHigh` unit-testeada y capaz de disparar.
- [ ] Literales económicos parametrizados (tip/gas/bps de fuente declarativa u on-chain — WO-04 cerrado).

**D. Readiness y sign-off**
- [ ] `/api/v1/readiness` verdict **GO** (hoy NO_GO, blockers A.6 partial / A.7 partial / A.9 critical).
- [ ] Panel A.9 `GoNoGoSignOffCard` (ya desplegado en el dominio): `/api/go-no-go/status` con `ledger_hash` ≠ null, `sign_offs` presentes del operador, `go_live_eligible: true` (hoy `state:"no_ledger"` — honesto y rojo, como corresponde).

**E. Infraestructura estable y observable**
- [ ] Disco < 75% sostenido; purge ≥ inserción; VACUUM/CHECKPOINT funcionales; ENOSPC imposible en el horizonte de operación.
- [ ] Canal de alertas PROBADO end-to-end (alerta sintética recibida por un humano; 0× 401); node-exporter + reglas de disco vivas; 0 reglas muertas en `alerts.rules.yml`.
- [ ] Deploy coalescing + SHA-anclado operativos; 0 recreates sin causa identificada; builds fuera del hot-path.
- [ ] Identidad de build visible en cada servicio y == `git rev-parse HEAD` (G4/G5 automáticos).
- [ ] CSP enforcing activo (fase b completada); token cloudflared en EnvironmentFile; SSH key-only + fail2ban; API key de Alchemy redactada de logs.
- [ ] WS nativo en la ruta pública (RULE 02 cumplida; verificado con socket.io-client real).
- [ ] RPC pool sano para la cadena objetivo (0 breakers open persistentes; RPC dedicado para simulación).

**F. Terminus de ejecución (frontera capital)**
- [ ] Relay catálogo ≥ 1 acreditado para la cadena objetivo (hoy 0 — terminus NotSubmitted/501); reconfiguración de cadena del terminus resuelta (pool/catálogo/signer por cadena).
- [ ] Signer provisionado VÍA VAULT (unseal solo en ventana de operación controlada por el operador; NUNCA auto-unseal); `FLASHBOTS_SIGNER_KEY` fuera del `.env` plano; healthcheck de vault honesto (sealed ≠ ready).
- [ ] `SIM_SIGNER_ADDRESS` placeholder reemplazado por signer real del operador; direcciones de executor verificadas on-chain (hoy "no verificado").
- [ ] Probe E2E runtime de `MainnetRefused` en STAGING ejercitado (hoy: unit tests sí, runtime nunca); probe del path Sepolia completo.
- [ ] `ARBX_LIVE_EXEC_ENABLED` normalizado (`"false"`; contrato `"true"` exacto documentado); `arbx:papermode:<chain>` fijado EXPLÍCITO; gauge/alerta de flip `arbx_live_exec_enabled` activo con ruta dual (AM + panel A.9).
- [ ] Flags de migración §34.2 retirados del runtime (`ARBX_ORCHESTRATOR_MODE`, `ARBX_CARTRIDGE_MODE`) o documentados como inertes.

**G. Seguridad externa (decisión/contratación del operador — no de los agentes)**
- [ ] Auditoría externa de contratos (Trail of Bits/OpenZeppelin/Spearbit u otra) PASS o riesgo aceptado por escrito.

**H. Post-flip (pre-acordado ANTES de flip)**
- [ ] Kill-switch benchmarked (<10 ms probado, no autodeclarado — WO-11) y plan de rollback ensayado; primera semana de mainnet con `max_value_eth` acotado y monitoreo de `arbx_live_exec_enabled` continuo.

**Nota final R8:** nada de esta lista está VERDE hoy. Lo más cercano: la frontera fail-closed (F parcialmente verde por diseño) y G7 (knob-OFF vivo como el gate más cercano de la certificación). La brecha hasta readiness es esencialmente: alertas que lleguen (P0-4) + embudo sin ruido (P1-2) + un solo flip de simulación del operador (P1-3) + labels que alimenten calibración (P1-3/4) + discos que no exploten (P0-3). Todo lo demás es hardening del camino. **El sistema está a ~5 trabajos estructurales de poder EVALUAR de verdad por primera vez — y a cero promesas de lo que encontrará cuando lo haga.**

---

*Fin del roadmap. Evidencia completa en los reportes 01-08 (+CROSS) de este directorio. Fail-honest R8 en todo: los huecos declarados en §1.4 son huecos, no ausencias.*
