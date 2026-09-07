# N8 — Verificador de Flota + Monitoring ("monitoring-fleet")

- **Agente:** N8 monitoring-fleet (round-table integración DApp ArbitrageX)
- **Superficie:** Flota + monitoring — qué corre, qué alerta, qué drift
- **Estado:** COMPLETADO
- **Ventana de auditoría:** 2026-09-06 23:31Z → 23:40Z (inicio local 18:31 -0500)
- **Veredicto:** **DEGRADED** — el stack de monitoring funciona de punta a punta, pero la flota se reinició 2 veces en 35 min DURANTE esta auditoría (cascada de auto-deploy), hay 5 circuit breakers RPC activos sin mecanismo de notificación verificable, y el hallazgo semilla del orquestador estaba invertido.

---

## 1. RESUMEN EJECUTIVO

| Pregunta del charter | Respuesta (con evidencia en §3-§6) |
|---|---|
| ¿Qué reinició la flota (24 contenedores "Up 17 min")? | **Auto-deploy post-merge a main**, no crash ni operator: merge #545 22:30:49Z → ciclo completo (pull+107 migraciones SQL+build ~27min+`up -d`) → recreate 22:58:05-11Z. Luego merge #543 23:01:22Z → migraciones 23:16:48, builds 23:20:30-23:31:39, **segundo recreate 23:33:07-29Z** (presenciado en vivo). #544 mergeado 23:27:05Z quedó en cola. |
| ¿Por qué thanos "Up 2 weeks"? | **Por diseño de compose, no drift**: thanos usa imagen pinned `quay.io/thanos:v0.36.1` sin cambios de config desde 2026-08-19 (`git log -S thanos -- docker/compose.prod.yml` = vacío desde Aug 19) → `up -d` no lo recrea. Los demás servicios usan imágenes locales rebuildadas cada deploy → sí se recrean. Daemon Docker NO se reinició (up 39 días). |
| Prometheus targets UP/DOWN | **9 targets: 8 UP, 1 DOWN crónico** (`edge` → `http://edge:8787/metrics` = HTTP 404; el edge no expone /metrics). |
| Reglas cargadas | **4 grupos / 24 reglas** desde `/etc/prometheus/alerts.rules.yml` (arbitragex-core 12, mev-signals 7, containers 3, storage 2). MD5 repo-VPS == contenedor == commit d4d3ff63 == GitHub main (diff monitoring/ VACÍO entre ambos). |
| ¿"VPS corre #545 pero origin/main = #543, el fix existe SOLO en el VPS"? | **FALSO — hallazgo semilla corregido**: VPS HEAD = `d4d3ff63` (#543) que CONTIENE `4cb807d2` (#545) como ancestro; GitHub main = `9ac06d2d` (#544, ADELANTADO al VPS). El fix #545 está en main, en VPS y es contenido idéntico en bytes. En este clone `origin` = GitHub (no bare VPS). |
| Alertmanager silences activos | **0 silences en total** (lista limpia). **5 alertas `RpcCircuitBreakerOpen` ACTIVE** (providers blockpi, publicnode, drpc, mevblocker, flashbots — searcher-rs:9001, kind=http, severity=warning). Receiver = webhook con URL enmascarada `<secret>` → destino de notificación NO verificable read-only. |
| Grafana/Loki/Thanos estado | Grafana 11.4.0 healthy (`database: ok`); Loki `ready` (tras ventana transitoria post-restart); thanos-query `OK` en 127.0.0.1:10904; thanos-sidecar recuperó de heartbeat-fail 22:58:01→ready 22:58:31 y subió bloque a objstore 23:00:33. |

---

## 2. TIMELINE FORENSE DEL REINICIO DE FLOTA (UTC, todo 2026-09-06)

| Hora | Evento | Evidencia |
|---|---|---|
| 2026-07-28 23:38 | Boot del host (uptime 39 días — NO hubo reboot) | `uptime -s` |
| 2026-08-19 06:25:31 | Última recreación de los 3 thanos | `docker inspect StartedAt` |
| 22:30:49 | Merge **#545** (promtool fix) a main | `git log --format='%cI'` = `2026-09-06T17:30:49-05:00` |
| **22:58:05-11** | **Recreate #1**: 21 servicios StartedAt juntos, RestartCount=0, policy unless-stopped | `docker inspect` de frontend/edge/api-server/searcher/prometheus/alertmanager |
| ~23:0x-23:10 | Pull de #543 al VPS: HEAD pasa a `d4d3ff63` | `git -C /opt/arbitragex-v2 rev-parse HEAD` (verificado 23:32) |
| 23:01:22 | Merge **#543** (A7 relays) a main | `git log` committer date |
| 23:16:48 | 107 logs de migración SQL escritos (`/tmp/mig_*.log`, ej. `mig_104_readiness_evidence.sql.log` mtime 23:16:48) | `find /tmp -name 'mig_*.log' -newermt '2026-09-06 22:30' \| wc -l` = 107 |
| 23:20:30→23:31:39 | Builds en curso (`exporting to image` ×4 en journal dockerd, mismo traceID c7a68deb…) | `journalctl -u docker.service` |
| 23:27:05 | Merge **#544** (A9 panel) a main — queda tras la ciclo #543 | `git log` committer date |
| **23:33:07-29** | **Recreate #2 presenciado en vivo**: task-delete storm + prom/AM StartedAt 23:33:28.87 | journal + `docker inspect` post-hoc; procesos prom startTime 23:33:29.428 |
| 23:35-23:38 | Estado: 24/24 Up (healthy), load 1.27 (1min) tras pico 35.54/41.17 | `docker ps`, `uptime` |

**Qué NO fue:** no reboot (uptime 39d), no restart del daemon (thanos intactos + dockerd PID 3490150 constante), no crash-loop (RestartCount=0 en todos los inspeccionados), no cron (crontab solo pg_retention 04:17 + builder-prune domingos), no timer de deploy, no runner self-hosted en VPS, no login interactivo (`last` = solo reboots Jul 28 y root May 14). La cadena es merge→CI/automation→deploy.sh (migraciones+build+up). Nota: los `/tmp/deploy*.log` clásicos están stale (más nuevo = Aug 29) — el ciclo de hoy no dejó log en /tmp con ese patrón.

---

## 3. CAPA LOCAL (clone Windows, branch a6-cbprom-01)

**Estado: DRIFT (trabajo no mergeado)**

```
$ git remote -v          → origin = https://github.com/hefarica/arbitragex-v2.git  (¡GITHUB, no bare VPS!)
$ git branch --show-current → a6-cbprom-01
$ git log --oneline -6   → f7db6867 (update post-#544), 9ac06d2d (#544), f46a0522, d4d3ff63 (#543), 086347f7 (post-#545), 4cb807d2 (#545)
$ git ls-remote origin main → 9ac06d2dc70594dd8eac904aea027613a22a1940
$ git merge-base --is-ancestor 4cb807d2 9ac06d2d → SÍ (#545 está en main)
$ git merge-base --is-ancestor c498773c 9ac06d2d → NO (A6-CBPROM-01 NO está en main)
$ git merge-base --is-ancestor c498773c f7db6867 → SÍ (está en la branch local)
$ grep -c 'alert:' monitoring/alerts.rules.yml → 29   (main: 24)
$ grupos local:  arbitragex-core, storage, containers, mev-signals, circuit_breakers
$ grupos main:    arbitragex-core, storage, containers, mev-signals
```

- **Drift local:** el grupo `circuit_breakers` (+5 alertas, los 10 breakers doctrinales de A6-CBPROM-01, commit `c498773c`) existe SOLO en la branch local. Ni main ni VPS lo tienen.
- `monitoring/tests/` existe (fix #545 aplicado: `promtool test rules` resuelve `rule_files` contra el dir del TEST, no el CWD — bug de 42 corridas CI rojas 08-17→09-06, ahora arreglado EN main).
- `git diff --stat d4d3ff63 9ac06d2d -- monitoring/ docker/` = **VACÍO**: monitoring idéntico entre VPS-HEAD y main.

## 4. CAPA REMOTE_MAIN (GitHub origin/main = 9ac06d2d #544)

**Estado: MATCH (en monitoring; VPS va 1 merge detrás en app)**

- main contiene #545 (4cb807d2) y #543 (d4d3ff63) como ancestros — verificado con `merge-base --is-ancestor` (§3).
- `git show d4d3ff63:monitoring/alerts.rules.yml | md5sum` = `e804aa003696c637282ba3e6d04dcaf2` == md5 del archivo en repo VPS == md5 dentro del contenedor Prometheus. **Cadena de reglas byte-idéntica local↔main↔VPS↔runtime.**
- **Corrección del hallazgo semilla:** "el fix existe SOLO en el VPS" es incorrecto en ambos sentidos: (a) #545 SÍ está en GitHub main; (b) el VPS no corre "más nuevo" que main — corre d4d3ff63, y main ya tiene #544 (panel A9-GONOGO) que el VPS aún no despliega (o desplegaba en el ciclo en cola al cierre de esta auditoría).

## 5. CAPA VPS (ssh arbx, read-only)

**Estado: DRIFT (transitoria — deploy en vuelo durante la auditoría)**

### 5.1 Flota (24 contenedores, proyecto `arbitragex-v2`, compose.prod.yml, workdir /opt/arbitragex-v2/docker)
- 21 servicios recreados 22:58 y de nuevo 23:33 (ver §2); al cierre 24/24 Up (healthy).
- 3 thanos (query/sidecar/store) Up desde 2026-08-19 06:25:31 — explicación en §1; **no hay cambios de config thanos desde Aug 19** (`git log -S 'thanos' --since=2026-08-19 -- docker/compose.prod.yml` = vacío) → no-drift, comportamiento esperado de compose.
- Thanos-sidecar: warn heartbeat-fail 22:58:01 (prometheus reiniciándose) → ready 22:58:31 → `upload new block id=01M1WF6H...` 23:00:33. Ciclo de shipping SANO.

### 5.2 Recursos del host
- 8 vCPU, 15.6 GB RAM (5.4 usados), disco `/` 150G con **112G usados = 78% (33G libres)** — tendencia al alza post-retention (48% el 2026-09-04).
- **Load pico 35.54/41.17/28.06 durante los builds** (23:31) → normalizado a 1.27/11.05/18.41 al cierre (23:38). El build --no-cache de la flota completa compite por CPU con el hot-path en el mismo host.

### 5.3 Prometheus (127.0.0.1:9090)
- `/-/ready` OK. Proceso actual startTime 23:33:29 (recreate #2); el snapshot de targets/reglas se tomó en la generación 22:58 (mismo archivo de reglas, md5 verificado).
- **Targets: total=9, up=8, down=1.** DOWN: `job=edge url=http://edge:8787/metrics err=server returned HTTP 404 Not Found` (crónico: el edge no implementa /metrics).
- UP jobs: api-server, prometheus, recon, relays-client, searcher-rs, selector-api, sim-ctl, token-enricher (1 c/u).
- **Reglas: 4 grupos, 24 reglas alerting** (arbitragex-core 12 / mev-signals 7 / containers 3 / storage 2), file=alerts.rules.yml; `rule_files: [/etc/prometheus/alerts.rules.yml]`, scrape/evaluation 15s.
- **Alertas activas en el momento de muestreo: 8 series `RpcCircuitBreakerOpen`** (evolucionaron 5 pending → 6 firing/2 pending → 5 active en AM tras el restart): instance=searcher-rs:9001, kind=http, providers = **blockpi, publicnode, drpc, mevblocker, flashbots**, severity=warning.
- Métricas `arbx_*` vivas: 83 nombres (incl. `arbx_cb_state`, `arbx_cb_trips_total`, `arbx_killswitch_enabled`, `arbx_candidates_total`, enriquecer/engine/...) — emisión del plano de datos confirmada.

### 5.4 Alertmanager (127.0.0.1:9093, v0.27.0)
- **Silences: total=0, activos=0** (estado limpio, sin fatiga de silencios… y sin gestión activa).
- **Alertas recibidas: 5 `RpcCircuitBreakerOpen` active** (una por provider) — el pipeline Prometheus→Alertmanager FUNCIONA.
- Route: `receiver: default`, group_by presente, `receivers: - name: default, webhook_configs:` con **`url: <secret>`** (AM enmascara la URL). **Destino real de la notificación NO verificable en modo read-only** — fail-honest: no puedo afirmar que alguien reciba estas alertas.

### 5.5 Grafana / Loki / Thanos-query
- Grafana 11.4.0: `{"database":"ok","version":"11.4.0"}` (127.0.0.1:3000).
- Loki: `ready` (primera sonda devolvió el transitorio "Ingester not ready: waiting for 15s after being ready" — post-restart; segunda sonda OK).
- Thanos-query: `OK` vía 127.0.0.1:10904 (puerto publicado; 10902 no está publicado en host).

### 5.6 Git en VPS
- `/opt/arbitragex-v2`: branch=main, HEAD=`d4d3ff63` (#543), dirty solo `?? archives/` (untracked). Remote origin = `git@github.com:hefarica/arbitragex-v2.git`.

## 6. CAPA LIVE_DOMAIN (https://arbx.ape-tv.net)

**Estado: MATCH (UP, con observación de latencia)** — 2 requests públicos gastados de 5.

```
$ curl -o /dev/null -w '%{http_code} %{time_total}s' https://arbx.ape-tv.net/          → 200, 5.90s
$ curl -o /dev/null -w '%{http_code} %{time_total}s' https://arbx.ape-tv.net/readiness → 200, 0.48s
```

- Dominio hallado en docs del repo (7 referencias a `arbx.ape-tv.net`, 3 a `edge-arbx.ape-tv.net`); el túnel cloudflared corre como servicio systemd **token-managed** (config remota en CF, sin config local — por eso no había hostname en /etc/cloudflared). Túnel activo (`systemctl is-active cloudflared` = active).
- El TTFB de 5.9s en `/` corresponde al cold-start post-recreate 23:33 (readiness posterior 0.48s). Consistente con churn de deploy, no con rotura.

---

## 7. DRIFTS MEDIDOS (consolidado)

- **D1 — CORRECCIÓN del hallazgo semilla:** "VPS corre #545 pero origin/main = #543; el fix existe SOLO en el VPS" es FALSO. Realidad: VPS HEAD = #543 (contiene #545); GitHub main = #544 (ADELANTADO). Fix #545 presente en ambas puntas. (Evidencia §3-§4.)
- **D2:** Reglas A6 `circuit_breakers` (5 alertas nuevas; archivo local 29 vs main/VPS 24) existen SOLO en branch local `a6-cbprom-01` (c498773c no-ancestro de main). Los 10 breakers doctrinales NO alertan en producción.
- **D3:** VPS HEAD 1 merge detrás de main (#544 panel A9) al cierre de la auditoría; ciclo de deploy de #544 en cola/in-course.
- **D4:** Target Prometheus `edge` DOWN permanente (404 en /metrics) — ceguera de monitoreo del Edge Worker (hot-path REST).
- **D5:** 5 `RpcCircuitBreakerOpen` active en Alertmanager (5 providers RPC http del searcher-rs) — 0 silences, webhook destino enmascarado.
- **D6:** Thanos "2 weeks" ≠ drift — pinned image + config sin cambios desde Aug 19; compose no recrea. Documentar para no re-diagnosticar.
- **D7:** Flota recreada 2× en 35 min durante la auditoría (22:58, 23:33) por cascada de 3 merges en <1h — alert-state de Prometheus se resetea en cada recreate (los `for:` vuelven a pending).

## 8. RIESGOS

- **R1 (P0):** Cadena de notificación de alertas INVERIFICABLE: receiver único = webhook `<secret>`; nadie confirma recepción; 5 breakers RPC activos ahora mismo podrían no llegar a ningún humano. En mainnet esto es ceguera operativa total.
- **R2 (P0):** Churn de deploy: cada merge a main = recreate de 21 servicios (~30 min ciclo: migraciones+build+up). 3 merges/hora = flota en recreate casi continuo, pérdida de estado de alertas, TTFB 5.9s, y riesgo de merges pisándose (ciclo #543 y #544 podrían solaparse — no verifiqué serialización; los traceID de build sugieren una sesión a la vez).
- **R3 (P1):** 5 proveedores RPC http con breaker ABIERTO en searcher-rs (blockpi/publicnode/drpc/mevblocker/flashbots) — salud del pool degradada sin triage visible (0 silences, 0 acks).
- **R4 (P1):** Build --no-cache de flota completa en el mismo host de producción: load 35-41 sobre 8 vCPU contiende contra el hot-path (sin resource limits observables en la ventana).
- **R5 (P1):** Disco 78% (33G libres), tendencia creciente post-retention (48% hace 2 días) — las reglas `storage` (2) existen pero no dispararon: revisar umbral vs 78%.
- **R6 (P2):** Branch local con A6 sin merge (§36 anti-caos): el fix de promtool #545 vuelve confiable el check de reglas — pero las reglas nuevas aún no pasan por él en main.
- **R7 (P2):** Token del túnel cloudflared embebido en `ExecStart` del unit systemd (visible ante `systemctl cat` por root/lectores del unit). NO se reproduce su valor aquí. Mover a EnvironmentFile restringido.
- **R8 (P2):** `docker events --since 22:57 --until 22:59:30` devolvió VACÍO pese a StartedAt verificados en esa ventana — la forense por events API no fue confiable (tuve que reconstruir por journal+inspect+mig-logs). Causa no determinada.
- **R9 (P2):** journal muestra brute-force SSH continuo contra usuarios `deploy`/`deployer` desde IPs externas (22:31-22:45Z). Higiene: key-only + fail2ban.

## 9. PROPUESTAS → PRODUCTION-MAINNET

| # | What | Why | Priority | Effort | Gate |
|---|---|---|---|---|---|
| P-1 | **Deploy coalescing + serialización**: 1 deploy por ventana (ej. 10 min debounce de merges), lock de deploy, y verificar post-up que `git rev-parse HEAD` == SHA anclado (regla existente de deploy veraz) ANTES de iniciar el siguiente ciclo | Elimina churn 2-recreates/35min, protege alert-state y TTFB; hoy 3 merges/hora colapsan la flota en recreate perpetuo | **P0** | M (script deploy.sh + workflow) | G-deploy veraz (§37 G4) + e2e |
| P-2 | **Verificar y documentar la entrega del webhook de Alertmanager** (alerta sintética end-to-end + registrar el destino en ops-runbook; considerar receiver dual webhook+telegram) | R1: 5 alertas activas con destino enmascarado = nadie demostró recibirlas; mainnet sin notificación accionable es inaceptable | **P0** | S | `safe-production-observability` + evidencia de recibo |
| P-3 | **Triage de los 5 RpcCircuitBreakerOpen** (¿providers muertos o half-open post-restart?) y silences documentados si son conocidos | R3: salud RPC real del searcher; normaliza la señal de alertas | **P1** | S | `alchemy-rpc-robust-integration` |
| P-4 | **Endpoint /metrics del edge** (o remover el target del scrape config mientras no exista) | D4: un target DOWN permanente normaliza el ruido y ciega el monitoreo del Edge | **P1** | S-M | contract defi `{success,data}` N2 intacto |
| P-5 | **Aislar el build del host de prod** (builder remoto, o `--load` con límites cpuset/cpus en build; ideal blue-green) | R4: load 35-41/8 cores durante build contiende hot-path | **P1** | M-L | `cloud-low-latency-infrastructure` |
| P-6 | **Merge de A6-CBPROM-01 por PR** (reglas circuit_breakers 24→29) aprovechando que #545 ya arregló promtool | D2/R6: los 10 breakers doctrinales deben alertar en prod; el check de reglas ahora es confiable | **P2** | S | CI Monitoring Config verde + promtool unit-test |
| P-7 | **Umbral de disco**: revisar reglas `storage` vs 78% real + alerta de tendencia | R5: 33G libres cayendo; antes del próximo 90% | **P2** | S | regla storage en alerts.rules.yml |
| P-8 | **Higiene**: token cloudflared a EnvironmentFile; key-only SSH + fail2ban; nota en ops-doc "thanos 2-weeks = diseño" | R7/R9/D6 — eliminar falsos positivos de futuras auditorías y superficie de exposición | **P2** | S | `security-auditor` |

## 10. DECLARACIONES FAIL-HONEST (R8 de doctrina — huecos declarados)

1. **NO verifiqué** la entrega real del webhook de Alertmanager (URL enmascarada por el propio AM; ninguna acción de escritura permitida para testear).
2. **NO pude** leer el contenido de la alerta RpcCircuitBreakerOpen histórica (prometheus se reinició 2× durante la muestreo; los `for:` y estados son de las generaciones 22:58/23:33).
3. `docker events` devolvió vacío para la ventana 22:57-22:59 (causa indeterminada); la forense se reconstruyó por StartedAt + journalctl + mtime de mig-logs.
4. **NO leí** `.env` ni valores de secretos; el token de cloudflared apareció en `systemctl cat` y NO se reproduce.
5. Requests públicos gastados: **2 de 5** (arbx.ape-tv.net/ y /readiness). `edge-arbx.ape-tv.net` NO probado (fuera de charter de esta superficie).
6. La serialización exacta de los ciclos #543/#544 (¿se pisaron?) no fue determinada — los traceID de build sugieren sesiones separadas pero no hay log de deploy de hoy en /tmp para confirmarlo.
