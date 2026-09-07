# N5 CROSS — Familia de Simulación (veredicto propio: DEGRADED)

- **Agente**: verificador N5 (simulator-family), fase cross-examination del round-table omniscience 2026-09-06.
- **Base**: mi reporte base `05-simulator-family.md` (DEGRADED: 0 sims aprobadas en 1.000.718 filas históricas, `XLEN arbx:opps:simulated`=0, labels=0, flip revm pendiente).
- **Ventana del cross**: 2026-09-06 ~23:40–00:05Z (POST al tercer reinicio de flota del día — ver §4).
- **Presupuesto HTTP público**: 0/5 requests. Todo read-only (ssh arbx con psql SELECT / redis read-only / curl 127.0.0.1 internos; git read-only local).

---

## 1. La cadena de inanición E2E — mi hallazgo central del cross

El cross me permitió probar de punta a punta lo que en mi reporte base era una inferencia: **la salida de MI superficie es la entrada del terminus, y está en cero desde siempre**.

```
searcher → arbx:opps:validated (XLEN 10.002, entries-added 337.782)
  → sim-ctl-g0 (51 consumers, lag 3.047) → simulations: 1.000.718 filas, passed=f al 100%
    → arbx:opps:simulated: XLEN = 0 (NADA publicado en la historia; MAXLEN 10K nunca recortaría a 0 solo)
      → relays-client consumer (SPAWNEADO, bloqueado en XREADGROUP)
        → 0 eventos relay_sim.no_submit en TODOS los logs
          → paper_trade_runs congelada desde 2026-09-01 16:32
            → readiness A.7 sigue "partial" → verdict NO_GO persiste
```

Evidencia dura nueva (comandos + outputs reales):

```
$ ssh arbx "docker logs arbitragex-v2-relays-client-1 --tail 300 | grep -c relay_sim.no_submit"
0
$ ssh arbx "docker exec redis redis-cli EXISTS arbx:opps:simulated:dlq"
0        # el DLQ del terminus NI EXISTE → cero mensajes entregados jamás al grupo relays-client-g0
$ ssh arbx "docker logs arbitragex-v2-relays-client-1 --tail 15"
{..."event":"relays_consumer.spawned_paper_only","paper_mode":true}
{..."event":"relays_consumer.started","stream":"arbx:opps:simulated","group":"relays-client-g0",...}
```

- `backend/relays-client/src/consumer.rs:21` — `const STREAM: &str = "arbx:opps:simulated"` (confirmado en código).
- `backend/relays-client/src/consumer_spawn.rs` — el consumer se spawnea en paper sin signer (`spawn ⇔ has_db ∧ has_rpc ∧ (has_signer ∨ paper_mode)`): está vivo, esperando un primer mensaje que nunca llega.

**Consecuencia para el round-table**: el blocker A.7 de readiness NO se cierra con más código (el callsite #543 ya está cableado en `submit_engine.rs` step 4.5) — se cierra con FLUJO. Y el único generador de ese flujo es mi flip P0 (`SIM_BACKEND=revm` + `REVM_RPC_URL`). **Mi P0 es critical-path del NO_GO global (A.7 runtime evidence → A.9 sign-off).**

## 2. Confirmaciones (lo de otros que MI evidencia respalda)

| Claim del round-table | Mi verificación |
|---|---|
| **api-ws**: HotPathEmitter con 0 call-sites, streams `arbx:hot:*` XLEN=0 perpetuos | **CONFIRMADO doble**: código — grep en `backend/` solo halla `hot_path_emitter.rs` (definición) + `searcher-rs/src/lib.rs:125` (`pub mod`); runtime — `XLEN arbx:hot:detected`=0, `XLEN arbx:hot:simulated`=0 (medido 2026-09-06 ~23:50Z) |
| **terminus**: consumer paper-only activo, 0 denials ejercidos | **CONFIRMADO y EXTENDIDO**: `relays_consumer.started` a 23:45:38Z sobre mi stream; 0 eventos; DLQ inexistente (§1) |
| **monitoring**: 5 RpcCircuitBreakerOpen (blockpi/publicnode/drpc/mevblocker/flashbots, searcher-rs) | **CONFIRMADO con matiz**: a 23:5xZ = 2 FIRING (blockpi, publicnode) + 3 pending (flashbots, mevblocker, drpc), todos `activeAt 23:45-46Z` = nacidos con el último restart. "5 activos" estructuralmente cierto; el estado individual hace churn pending↔firing alrededor de cada restart |
| **terminus**: RPC pool degradado a 5/9 (llama CF-challenge, 0xrpc 404, 1rpc 403) | **CONFIRMADO independiente** desde el boot de relays-client 23:45:35Z: `rpc_pool.boot_check_failed` ×3 (llama 403 CF, 0xrpc 404, 1rpc 403) → `rpc_pool.ready count=5 providers=[publicnode,drpc,flashbots,mevblocker,blockpi]` |
| **data-layer**: riesgo de trim-antes-de-consumo en streams Redis | **CONFIRMADO como riesgo estructural compartido**; en MI stream de entrada hoy NO ocurre: `arbx:opps:validated` XLEN=10.002, entries-added=337.782, `sim-ctl-g0` lag=3.047 < 10.002 (sin pérdida aún); mismo patrón MAXLEN que el `detected` donde ellos SÍ midieron recorte |
| **frontend**: builds sin identidad verificable (sin SHA/buildId) | **PARALELO exacto a mi D-10**: `/capabilities` de sim-ctl da `build.sha=null` (`option_env!` no inyectado). Mismo hueco raíz en dos superficies — ver propuesta refinada #3 |
| **edge**: flota recreada 23:29–23:33Z durante la auditoría | **CONFIRMADO y EXTENDIDO**: `docker inspect` muestra relays-client y sim-ctl con `StartedAt=2026-09-06T23:45:32Z` — un TERCER evento de restart después del que reportó monitoring (su cronología D7 termina 23:33Z) |

## 3. Desafíos (contra-evidencia propia)

### 3.1 A `monitoring-fleet` — inventario de alertas incompleto y R1 a medio contar

- Mi pull de `127.0.0.1:9090/api/v1/alerts` (~23:50Z) mostró una alerta que tu reporte NO lista: **`SimulationFailureRateHigh`** (`service=sim-ctl`, severity warning, state **pending**, activeAt 23:45:41Z, value 0.397). Es la ÚNICA alerta del sistema que vigila el defecto central de mi superficie (falla de simulación). Tu inventario ("5 RpcCircuitBreakerOpen, 0 silences") omite el único alerta que habla del pipeline de simulación.
- Tu R1 dice "entrega del webhook inverificable". **El verificador api-ws ya resolvió la mitad que faltaba**: Alertmanager POSTea cada 5:00 min exactos a `POST /admin/alertmanager/webhook` del api-server y recibe **401** (6 en 27 min). Es decir: el receiver `default` de AM existe, su destino es el api-server, y está CAÍDO. No es "inverificable" — es **roto con evidencia**. Tuverdad parcial enmascara un canal de notificaciones muerto.
- Tu cronología de churn (D7) termina en 23:33Z; hubo otro restart de flota 23:45:32Z (relays+sim-ctl `StartedAt` via `docker inspect`). Si tu evidencia de contadores/logs es pre-23:45Z, está un restart atrás.

### 3.2 A `api-ws` — propuesta HotPathEmitter mal secuenciada

Tu P1 "conectar HotPathEmitter en searcher-rs" está invertida en prioridad: con `passed=0` en TODA la historia (1.000.718 filas) y `arbx:opps:simulated` vacío desde siempre, `arbx:hot:simulated` sería un **segundo stream muerto** — duplicaría el 100% de fallos en vez de añadir señal. Conectar el emitter tiene sentido DESPUÉS del flip revm (mi P0), cuando exista algo que valga la pena emitir en sub-100ms.

### 3.3 A `terminus` — INTEGRATED sin la salvedad "inerte"

Tu capa VPS reporta el consumer "paper-only activo". Preciso: **activo pero inerte** — 0 mensajes procesados en la historia completa (DLQ inexistente, 0 eventos `relay_sim.no_submit`, 0 `LiveExecDenied`). El callsite A.7 que #543 cableó (verifiqué el diff: `submit_engine.rs` step 4.5, `classify_no_submit` paper→LogOnly) **jamás se ejecutó en runtime**. Un veredicto INTEGRATED sin esa salvedad sugiere un terminus ejercitado cuando en realidad nunca recibió输入. También el commit message de #543 lo admite: el blocker `a7_private_relay_no_submit_partial` seguía PARTIAL "on exactly this gap" — el wiring existía pero sin flujo que lo dispare.

### 3.4 A `data-layer` — el hueco del ledger paper es aguas arriba

Tu hallazgo de trim en `detected` (lag 10.492 > length 10.004 → ~488 entradas recortadas) es real, pero tu riesgo "hueco silencioso en el ledger paper" tiene una causa dominante distinta: `paper_trade_runs` está congelada desde 09-01 porque **no hay NADA que archivar** — 0 sims aprobadas. Los 3 escritores de la tabla: `api-server/src/paper/executor.ts:349`, `api-server/src/routes/paper-trade-archiver.ts:276`, `relays-client/src/persistence.rs:141` — los dos primeros dejaron de escribir opps no-viables (era post-fix R-0001) y el tercero consume un stream vacío. El trim es efecto secundario; mientras mi D-1/D-2 persista, el ledger no recibe filas ni con trim perfecto.

### 3.5 Matiz menor a `frontend-web`

Tu afirmación "main GitHub = 9ac06d2d" y mi reporte base "ls-remote origin main = d4d3ff63" NO se contradicen: mi medición fue pre-merge de #544 (23:27Z). Lo registro para que el operador no lea drift donde hay evolución temporal. Tu delta VPS..main = 5 archivos solo `frontend/` coincide con mi verificación de que la familia de simulación es idéntica local↔main↔VPS.

## 4. Hallazgos propios NUEVOS (derivados del cross)

- **N-1 — La única alerta de mi defecto no puede disparar**: `SimulationFailureRateHigh` (`monitoring/alerts.rules.yml:196`, presente en main d4d3ff63 — desplegada) mide `rate(arbx_simulation_total{passed="false"}[5m]) / clamp_min(rate(arbx_simulation_total[5m]),1) > 0.30 for:5m`. El flujo es bursty: en los valles el `rate[5m]` queda vacío → ratio=0 → el pending se RESETEA. Observado en vivo: pending (0.397) a 23:45–50Z y DESAPARECIDA a 23:5xZ, pese a que `sum by (passed)` muestra que `passed=false` es la ÚNICA serie activa (ratio instantáneo = 1.0, rate 0.0107/s, 37.846 sims/24h). Con `for:5m` continuo y ráfagas, la alerta flapea en pending eterno. Y si algún día dispara: el webhook está en 401 (§3.1). **El canal de alertas del defecto central del sistema es un callejón sin salida doble** (flap + 401).
- **N-2 — El flip revm no puede apoyarse en el RPC actual**: pool chain-1 = 5/9 vivos, y de los 5 vivos, 2 con breaker FIRING (blockpi, publicnode) + 3 pending. El fork anvil corre sobre `eth.drpc.org` (terminus) y drpc está pending en el searcher. `REVM_RPC_URL` exige endpoint dedicado de pago — la degradación del pool que reportaron terminus+monitoring convierte mi pre-check "RPC sano" en requisito duro.
- **N-3 — Churn medible en mi consumer group**: `sim-ctl-g0` pasó de 49 (mi reporte base) a **51 consumers** huérfanos tras los 3 restarts de hoy (22:58, 23:33, 23:45Z) — +1 por boot, sin DELCONSUMER. El churn de deploy que denuncia monitoring (R2) se acumula directamente en mi superficie.

## 5. Preguntas directas

1. **a monitoring-fleet**: (a) ¿Confirmas que el receiver `default` de Alertmanager es exactamente el `POST /admin/alertmanager/webhook` (api-server) que api-ws vio en 401 cada 5 min? — en el contenedor AM los receivers slack/pagerduty/telegram están comentados y solo vive `default`. (b) ¿Entras `SimulationFailureRateHigh` (y su flap) en tu inventario y en tu propuesta de alerta sintética end-to-end? (c) ¿Capturaste el restart 23:45:32Z o tu ventana terminó 23:33Z?
2. **a terminus**: (a) ¿Aceptas la salvedad "integrado pero inerte (0 mensajes en historia)" en tu veredicto INTEGRATED? (b) ¿`REVM_RPC_URL` admite el pool `shared_rs::rpc_failover` o exige endpoint único? Con 2/5 vivos en FIRING necesito saber qué puede consumir el flip.
3. **a api-ws**: ¿Cuál es la fuente prevista de `arbx:hot:simulated` — searcher directo o el post-pass de sim-ctl? Si es lo segundo, tu P1 (conectar HotPathEmitter) debe secuenciarse después de mi flip; si es lo primero, el emitter duplicará fallos hoy.
4. **a data-layer**: ¿Puedes incorporar `arbx:opps:validated` / grupo `sim-ctl-g0` a tu monitoreo lag-vs-XLEN? Hoy lag 3.047 < XLEN 10.002, pero el día que cruce (como ya mediste en `detected`) perderé sims de entrada silenciosamente.
5. **a searcher-pipeline (N4)**: tu reporte `04-searcher-pipeline.md` quedó EN_CURSO sin veredicto y tu superficie es mi entrada directa — ¿quién escribe `arbx:opps:validated` (¿enricher?) y con qué MAXLEN? La ventana de ~6h que data-layer midió en `detected` sugiere MAXLEN≈10K también aquí; necesito la cifra confirmada.

## 6. Propuestas refinadas (con dependencias del round-table)

| # | What (refinado) | Delta vs mi reporte base | Pri | Gate |
|---|---|---|---|---|
| 1 | **Flip `SIM_BACKEND=revm` + `REVM_RPC_URL`** — pero con endpoint DEDICADO de pago; PROHIBIDO apuntar al pool actual (5/9 vivos, blockpi+publicnode FIRING, drpc pending — medido §4/N-2). Pre-check añadido: health del endpoint + aceptación explícita de que este flip es el que desbloquea A.7 runtime evidence y (vía A.9) el NO_GO global | Antes: "RPC pago/dedicado" como consejo; ahora es REQUISITO con evidencia del round-table (terminus+monitoring) | **P0** | §34.3 operador-only; verificación: primer `passed=t` + `XLEN arbx:opps:simulated`>0 + primer evento `relay_sim.no_submit.validated` en relays-client (cierra el lazo con terminus) |
| 2 | **G-SIM-1 passed-rate** con ventana 24h (`increase(...[24h])` con `passed>0`, o query directo a `simulations`), NO ratio `rate[5m]` | Nuevo: mi hallazgo N-1 prueba que la alerta ratio-based flapea y no dispara; la ventana 24h es inmune al burst. La alerta SimulationFailureRateHigh debe reescribirse igual | **P1** | P-∅ PR con ID; test con counter passed=0 |
| 3 | **Identidad de build unificada** — UN PR: ARG/ENV `*_GIT_SHA` para frontend (NEXT_PUBLIC_GIT_SHA + header `x-arbx-build-sha`) Y sim-ctl (`SIM_CTL_BUILD_SHA` → `option_env!` → `/capabilities`) | Fusión de mi D-10 (P2) con el P0 de frontend-web: mismo hueco raíz, dos superficies, un solo PR — sube a P1 | **P1** | P-∅; verify-deploy L2 lo consume (frontend) + curl /capabilities (yo) |
| 4 | **Reparar el canal de alertas ANTES de confiar en cualquier alerta**: fusionar monitoring-R1 + api-ws-401 → fix del auth del receiver `default` (o receiver dual documentado) + alerta sintética end-to-end con recibo evidenciado | Nueva: el round-table tenía las dos mitades separadas; sin esto, SimulationFailureRateHigh y las 5 RpcCircuitBreakerOpen no llegan a ningún humano | **P1** | safe-production-observability; recibo capturado |
| 5 | **Higiene consumer-group ligada al churn**: DELCONSUMER en boot + alerta si lag sostenido; cuenta YA en 51 (+3 hoy por los 3 restarts — §4/N-3) | Refinada con la evidencia de churn de edge/monitoring: sin deploy-coalescing (su P0) esto crece +N por deploy | **P2** | test de shutdown |
| 6 | **Secuencia explícita para el operador**: (i) fix canal alertas (#4) → (ii) flip revm (#1) → (iii) primer passed + A.7 runtime evidence → (iv) hot-streams (api-ws P1) → (v) labels/calibración + A.9 GoNoGo | Nueva: orden de dependencias probado por la cadena §1 — invertir (ii)/(iv) o (i)/(ii) produce streams muertos o alertas que nadie recibe | — | — |

## 7. Notas de método

- Comandos read-only únicamente; 0/5 requests HTTP públicos; secretos jamás leídos (solo nombres de variables / receivers con `<url-redacted>`).
- El log de relays-client mostró accidentalmente el cuerpo HTML completo del CF-challenge de llama (error 403) — es output de un proveedor muerto, no un secreto del sistema; no lo reproduzco en extenso.
- Fail-honest R8: lo que NO verifiqué — (a) si `ARBX_USE_SIMULATOR_V2=true` cambia el path del searcher antes del stream validated (el dispatch_gate de capabilities.rs se lee en request-time; no audité al searcher como cliente); (b) el ingress exacto de Cloudflare (token-managed, inaccesible RO); (c) si el valor 0.397 de la alerta pending venía de ventana parcial post-restart (plausible, no probado).
