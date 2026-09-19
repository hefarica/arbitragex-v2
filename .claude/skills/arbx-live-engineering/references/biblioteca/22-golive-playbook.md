# 22. GO-LIVE: PLAYBOOK DE PRODUCCIÓN EXTREMO A EXTREMO

CUÁNDO CARGAR ESTA REFERENCIA: el operador pide "salir a producción", "deploy final", "go-live" o "canary"; se planifica transición de modo (PAPER_SHADOW → TESTNET → LIVE_MAINNET); se diseña o endurece CI/CD, staged deploy, rollback o freeze window; se definen SLOs, SLIs, alertas u on-call; ocurre (o se ensaya) un incidente, rollback o post-mortem; se agenda un drill de restore, kill-switch o dependencia caída; se hace capacity planning o coste por transacción.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Secuencia cero→prod | Pipeline de 12 pasos (§22.1) | Ningún paso opcional; freeze window T-48h |
| Pre-flight gates | `cargo clippy`/`fmt`/`audit`, `cargo deny`, `forge snapshot`, `npm audit` | Un gate que no corre en CI es un deseo, no un gate |
| Backups PG | `pg_basebackup --wal-method=stream` + WAL + PITR | Backup sin RESTORE DRILL probado = no backup |
| Artefacto inmutable | Image digest (`@sha256:…`), nunca `:latest` | Rollback = retag del digest previo conservado N deploys |
| Deploy veraz | SHA despachado por CI == `git rev-parse HEAD` en el host (L4) | Deploy no verificado en el entorno real no terminó |
| Rollout progresivo | shadow → canary → gradual → full | Criterios de éxito y aborto ESCRITOS antes del canary |
| SLO multi-burn | PromQL multiwindow-multiburn (14.4×/6×/1×) | Se pagina por burn del error budget, no por CPU suelta |
| Incidentes | SEV1-3 operativos + roles IC/Ops/Comms/Scribe | Mitigar primero (rollback), entender después |
| Evidencia forense | Exportar logs/coredumps/métricas ANTES de reiniciar | Reiniciar sin preservar destruye la escena del crimen |
| Post-mortem | Blameless, 5-whys, acciones con dueño y fecha | Todo incidente cierra con revert + gate nuevo (§37) |
| Drills | restore PITR, kill-switch, RPC/builder/DB caídos | Se prueban con cadencia fija, ANTES de necesitarlos |
| Continuidad | RTO/RPO por componente + coste por trade | El failover se ensaya en un drill, no se dibuja |

## 22.1 LA SECUENCIA CANÓNICA CERO → PRODUCCIÓN

El go-live no es un evento: es una cadena de estados con criterio de salida explícito cada
uno. Saltarse un paso no acelera: traslada el coste al momento más caro posible.

```
[1]  build limpio ────────▶ artefacto reproducible identificado por digest
[2]  gates de código ─────▶ unit + fuzz + fork + lint + audit + gas + capacity
[3]  infra ───────────────▶ sizing + headroom + backups con restore drill YA hecho
[4]  secrets ─────────────▶ rotados, escoped, crash-on-boot verificado
[5]  deploy staging ──────▶ por servicio, --env-file, imagen por digest
[6]  smoke staging ───────▶ suite automática (§22.6), cero intervención manual
[7]  gate manual ─────────▶ environment protegido (required reviewers)
[8]  deploy prod staged ──▶ shadow/paper primero; NUNCA todo el stack de golpe
[9]  smoke prod + L4 ─────▶ SHA servido == git rev-parse HEAD
[10] hypercare 72h ───────▶ observación intensiva, on-call reforzado
[11] SLOs activos ────────▶ multi-burn armado, budgets definidos
[12] steady state ────────▶ on-call normal, drills agendados
```

Reglas duras: la secuencia es lineal (no se entra a [8] sin [6] verde) y ningún paso es
opcional ("es un cambio chico" acorta cada paso, nunca elimina pasos). El orden
shadow→canary→live lo fija CLAUDE.md §34 (diferenciación solo en el terminus
`relays-client`); este playbook lo opera, no lo re-decide. Tras [8] el terminus sigue
default-deny (`live_exec_policy.rs`: `MainnetRefused` salvo `ARBX_LIVE_EXEC_ENABLED=true`):
un go-live de infraestructura no es un flip de modo.

**Freeze window**: solo entran fixes de SEV activo y reverts. Triggers: T-48h pre-launch;
auditoría en curso (lección del repo: NUNCA deployar con audit en curso — invalida
evidencia y contamina la ventana de logs); error budget agotado (§22.7); SEV1 abierto.

## 22.2 PRE-FLIGHT GATES: CHECKLIST COMPLETO

Versión go-live del checklist base (núcleo §10): cada ítem es un gate que CI ejecuta — lo
que un humano "revisa a mano" se pudre en dos sprints.

```text
CÓDIGO
[ ] cargo test --workspace                          unit + integration verde
[ ] cargo fuzz run <target> -- -max_total_time=300  sin crashes (targets del hot-path)
[ ] forge test --fork-url "$MAINNET_RPC"            fork tests contra estado real
[ ] cargo clippy --workspace --all-targets -- -D warnings
[ ] cargo fmt --all -- --check
[ ] npm audit --audit-level=high (o pnpm audit)     frontend sin vulns high/critical
DEPENDENCIAS
[ ] cargo audit                                     sin advisories abiertas sin excepción documentada
[ ] cargo deny check advisories bans licenses       licencias + crates baneadas (FUSILE_SOURCE_POLICY)
GAS / ECONOMÍA
[ ] forge snapshot                                  diff vs .gas-snapshot revisado:
                                                    regresión de gas = regresión de P&L (bloquea)
CAPACIDAD
[ ] capacity test al pico proyectado ×2             (RPS detección, CU de RPC, disco/día)
OPERACIÓN
[ ] plan de rollback ESCRITO con comandos exactos   (digest previo + pasos + smoke mínimo)
[ ] runbook mínimo por servicio actualizado (§22.10)
[ ] cero secretos en el diff: gitleaks detect + revisión de git log -p
[ ] version bump + CHANGELOG
```

Notas de campo: **gas snapshot** — en CI, `forge snapshot --diff` contra el snapshot del
commit base convierte la optimización en gate económico; con el net profit decidido por gas
(núcleo §1.2), +5% de gas en el executor es un P&L regression, no un style nit. **Plan de
rollback ESCRITO** — "redeploy de la versión anterior" no es plan: exige digest exacto,
comando de retag/redeploy, verificación y qué pasa con datos escritos por la versión nueva
(schema expand/contract, §22.5). **Capacity test** — reproducir el pico (burst de
oportunidades, bloque congestionado) y medir p99 del ciclo, CU de RPC, disco; pico×2 es el
floor, el techo lo pone el guard de disco (§22.3.1).

## 22.3 INFRAESTRUCTURA: SIZING, BACKUPS, SECRETS

### 22.3.1 Sizing por servicio con headroom explícito

| Servicio | CPU | RAM | Disco | Nota |
|---|---|---|---|---|
| searcher/collector (hot-path) | 2-4 cores | 4-8 GiB | 40 GiB SSD | aislado del ruido del resto |
| sim-ctl (simulación EVM) | 4+ cores | 8 GiB | 40 GiB | CPU-bound: el pico define cores |
| postgres | 2-4 cores | 8-16 GiB | NVMe + guard | el dato gordo del sistema |
| redis (streams) | 1-2 cores | 2-4 GiB | 10 GiB | ephemeral por diseño (re-detectable) |
| edge/frontend/api-server | 1-2 cores | 1-2 GiB | 10 GiB | sin estado: redeployable en minutos |

**Guard de disco obligatorio** (lección DISK-GUARD-01): el disco se dimensiona por estado +
costo por deploy, no por estado estacionario. Un deploy consume ~20 GB en cache/build (y el
builder-cache puede duplicarse 2× en 24h). El guard `--min-free-space` bloquea cualquier
deploy que dejaría el disco bajo 30 GB (advertencia) / 15 GB (parada dura): sin él, el
deploy que "arregla" el sistema llena el disco y anida el segundo incidente dentro del
primero. **Regiones**: la ubicación se decide midiendo p99 a RPC y relés/builders desde cada
región candidata, no por precio; con VPS único (realidad del repo, Hetzner) la medición
obligatoria es p99 VPS→cada proveedor RPC y la story real de continuidad es el failover
(§22.11).

### 22.3.2 Backups de PostgreSQL: basebackup + WAL + RESTORE DRILL

```conf
# postgresql.conf — WAL archiving continuo
archive_mode = on
archive_command = 'test ! -f /archive/%f && cp %p /archive/%f'
wal_level = replica
```

```bash
# Base backup físico con WAL en streaming (RPO → minutos) + verificación de integridad
pg_basebackup -D /backup/base --wal-method=stream --checkpoint=fast --create-slot --slot=arbx_base
pg_verifybackup /backup/base
```

**RESTORE DRILL obligatorio** (trimestral y T-7d antes de todo go-live). Un backup no
probado no es un backup: es una esperanza con fecha de expiración.

```bash
cp -r /backup/base /restore/drill
touch /restore/drill/recovery.signal
# postgresql.auto.conf del drill:
#   restore_command = 'cp /archive/%f %p'
#   recovery_target_time = '<ts objetivo>'
#   recovery_target_action = 'promote'
pg_ctl start -D /restore/drill -o "-p 5433"
psql -p 5433 -d arbitragex -c 'SELECT MAX(detected_at) FROM opportunities;'
```

El drill mide el RTO real (reloj en mano) y verifica el RPO (WAL perdido); ambos se anotan
en el runbook. Lecciones del repo: DELETE masivos con pacing de CHECKPOINT y cap por run
(un WAL-burst llena el disco con una sola purga); `VACUUM FULL` standalone, jamás en pico;
retención rollup-first (15 GB/día sin rollup convierte el restore en horas).

### 22.3.3 Secret manager: rotación y escoping

| Regla | Implementación |
|---|---|
| Escoping por servicio | cada servicio solo ve sus claves; observadores read-only (Redis ACL `+@read -@write`, PG `SELECT`-only, §33) |
| Cero secretos versionados | tracked solo placeholders `${VAR}`; valores reales en `.env`/secret manager gitignored |
| Detección | `gitleaks detect` en CI por PR + revisión del diff |
| Crash-on-boot | falta un secret crítico (ej. `SIM_SIGNER_ADDRESS`) → crash ruidoso en arranque (RULE 02: es seguridad) |
| Rotación | RPC keys 90d o inmediato ante sospecha; key material del terminus antes de cada cambio de modo |
| Nunca en logs | redaction en tracing; `#[debug(skip)]` en config (núcleo §6.1) |

## 22.4 CI/CD: STAGES, ARTEFACTOS INMUTABLES, DEPLOY VERAZ

```
lint ─▶ test(unit) ─▶ integration(fork) ─▶ build(digest) ─▶ deploy:staging
        ─▶ smoke:staging ─▶ [GATE MANUAL: environment "production" con
        required reviewers] ─▶ deploy:prod (por servicio)
        ─▶ smoke:prod ─▶ L4 veritas ─▶ observación 24h
```

- Cada stage falla el pipeline: no existe "warning" en el camino a prod.
- El gate manual es un GitHub environment protegido (`environment: production` + required
  reviewers) sobre branch protection con checks requeridos (el repo corre 14).
- Deploy a prod **por servicio** (lección del repo): nunca `up -d` de todo el stack de
  golpe; cada servicio sale, se fuma y se observa antes del siguiente.

**Artefactos inmutables**: el deploy referencia el digest, no el tag (los tags son
mutables; el digest es contenido):

```text
MAL:  image: ghcr.io/hefarica/searcher-rs:latest
BIEN: image: ghcr.io/hefarica/searcher-rs@sha256:<digest>   (alias legible: v1.2.3)
```

`docker inspect <contenedor> --format '{{.Image}}'` devuelve el digest real en ejecución —
ese número se compara en L4. Se conservan N=3 digests previos por servicio: rollback
instantáneo es retag + redeploy de un digest que YA existe, no un rebuild.

**Deploy veraz y env** (RULE 01-04 del repo):

```bash
# Env SIEMPRE explícito; rebuild --no-cache cuando el env se hornea en build (NEXT_PUBLIC_*)
docker compose --env-file .env -f docker/compose.dev.yml build --no-cache frontend
docker compose --env-file .env -f docker/compose.dev.yml up -d frontend

# L4: SHA en el host vs SHA despachado (el auto-deploy puede fallar EN SILENCIO).
# El repo NO expone endpoint de versión: el SHA se verifica en el host, igual que
# auto-deploy-vps.yml tras cada deploy.
[ "$(git -C /opt/arbitragex-v2 rev-parse HEAD)" = "$DEPLOYED_SHA" ]

# NEXT_PUBLIC_* se hornea en `next build`: un `restart` NO aplica cambios de .env.
curl -I http://127.0.0.1:5173/opportunities   # CSP con localhost ⇒ REGLA VIOLADA
```

Deploy veraz = dos números iguales: el SHA que CI despachó y `git rev-parse HEAD` en el
host. Divergencia = el pipeline mintió y se detecta ANTES de que el incidente lo exponga.

## 22.5 ROLLOUT PROGRESIVO: SHADOW → CANARY → GRADUAL → FULL

```
PAPER_SHADOW ──▶ CANARY (fracción mínima) ──▶ GRADUAL ──▶ FULL
capital 0         capital acotado pre-escrito    ↑ steps     ↑
                  abort automático armado        └── cada step exige el criterio previo
```

**Canary**: fracción mínima de capital/tráfico con límites ESCRITOS ANTES de arrancar (el
repo fija el techo del operador en §34.5: capital en riesgo ≤ $350, principal TLS 5 WETH).
Por escrito y antes: **criterios de éxito** — duración mínima 48-72h cubriendo ambos
regímenes de volatilidad, cero invariantes rotos (§22.6), cero pérdidas netas no
revertidas, p99 del ciclo en SLO, rate de inclusión ≥ umbral; **criterios de aborto
automáticos** (sin humano en el loop) — pérdida acumulada > X, drawdown > Y, divergencia
RPC > 500ms (§9), circuit breaker abierto → el aborto dispara el kill-switch del feature
flag, no una discusión.

| Dimensión | Blue-green | Rolling |
|---|---|---|
| Cambio de versión | instantáneo (switch de tráfico) | gradual por réplica |
| Coste | 2× infra | ~1× |
| Versiones mixtas | no | sí (nueva + vieja coexisten) |
| Encaje en este repo | edge/frontend/api (sin estado) | hot-path con réplicas |

Regla transversal: **schema expand/contract** — durante el rollout las migraciones solo
agregan (columnas nullable, tablas nuevas); la contracción (DROP) llega releases después,
cuando ninguna imagen vieja lee el schema. Así el rollback de la app JAMÁS requiere
rollback de datos: se hace rollback de imagen y la base queda compatible.

**Kill-switch por feature flag**: cada feature nueva viaja tras flag independiente del
deploy (deploy ≠ release): el flag se prende días después del deploy y se apaga en segundos
sin redeploy. El kill-switch global (API/File/Edge, <10 ms, §9) es el último eslabón; el
feature flag es el primero. **Rollback instantáneo** = retag del digest previo (§22.4) +
`up -d` del servicio + smoke mínimo; minutos, medidos en drill, no estimados.

## 22.6 SMOKE SUITE POST-DEPLOY AUTOMATIZADA

Corre en staging (paso 6) y prod (paso 9) sin intervención humana: sale 0 o falla el
deploy. No es un checklist que un humano "va mirando".

```bash
#!/usr/bin/env bash
set -euo pipefail
# 1. Health/readiness por servicio (núcleo §4.2: /health, /ready)
for ep in 8787/health 8080/health; do curl -fsS --max-time 5 "http://127.0.0.1:$ep" >/dev/null; done
# 2. Endpoint clave con contrato {success,data}
curl -fsS http://127.0.0.1:8787/api/opportunities/live | jq -e '.success == true' >/dev/null
# 3. WS vivo: el handshake de upgrade debe responder 101 (head -1 cierra el pipe y curl
#    muere por SIGPIPE bajo pipefail: se captura con || true y se asserta aparte)
ws_head=$(curl -sS --include --no-buffer --max-time 2 -H "Connection: Upgrade" \
  -H "Upgrade: websocket" -H "Sec-WebSocket-Version: 13" \
  -H "Sec-WebSocket-Key: $(openssl rand -base64 16)" \
  "http://127.0.0.1:8080/socket.io/?EIO=4&transport=websocket" 2>/dev/null | head -1 || true)
case "$ws_head" in *101*) : ;; *) echo "FATAL: WS handshake sin 101" >&2; exit 1 ;; esac
# 4. Invariantes de negocio (trazabilidad R7): los contadores AVANZAN
A=$(docker exec redis redis-cli XLEN arbx:opps:detected); sleep 120
[ "$(docker exec redis redis-cli XLEN arbx:opps:detected)" -gt "$A" ]
[ "$(docker exec postgres psql -U postgres -d arbitragex -tAc \
  'SELECT EXTRACT(EPOCH FROM (now() - MAX(detected_at))) < 600 FROM opportunities;')" = "t" ]
# 5. L4 veritas (SHA del host) + CSP sin localhost (RULE 04)
[ "$(git -C /opt/arbitragex-v2 rev-parse HEAD)" = "$EXPECTED_SHA" ]
! curl -sI http://127.0.0.1:5173/opportunities | grep -qi localhost
```

El paso 4 verifica la **invariante de negocio** (el pipeline produce eventos), no solo que
los procesos respiren: un contenedor "healthy" con el pipeline muerto por dentro pasa
`/health` y fracasa la invariante. El paso 5 es el L4 del repo — el SHA consultado al host
real; documentos y dashboards no son evidencia (precedente 2026-09-15:
afirmaciones sin artefacto reproducible no valen).

## 22.7 SLOs Y ALERTAS: SLIs, MULTI-BURN, ERROR BUDGETS

**Selección de SLIs**: un SLI válido es (a) lo que la estrategia percibe, no lo que el ops
percibe, y (b) un ratio buenos/total, no un número absoluto.

| SLI | Definición (métricas del núcleo §5.1) | SLO inicial |
|---|---|---|
| Disponibilidad del pipeline | `rate(arbx_simulations_passed)/rate(arbx_simulations_total)` | 99% / 30d |
| Detección viva | `rate(arbx_opportunities_detected_total) > floor` en 30m | 99% de ventanas 30m |
| Ciclo detect→decide | p99 de `arbx_simulation_duration_seconds` | < 250 ms |

**Multiwindow-multiburn** (patrón canónico del Google SRE Workbook, "Alerting on SLOs"):
se pagina por rate de consumo del error budget, no por umbrales sueltos de CPU/RAM.

| Burn rate | Ventana corta | Ventana larga | Budget consumido | Acción |
|---|---|---|---|---|
| 14.4 | 5m | 1h | 2% | PAGE (fast burn) |
| 6 | 30m | 6h | 5% | PAGE en hypercare / ticket |
| 1 | 6h | 3d | 10% | ticket (slow burn) |

```promql
# FAST BURN con budget 0.01 (SLO 99%): AMBAS ventanas deben quemar
(
  1 - sum(rate(arbx_simulations_passed[5m])) / sum(rate(arbx_simulations_total[5m])) > 14.4 * 0.01
  and
  1 - sum(rate(arbx_simulations_passed[1h])) / sum(rate(arbx_simulations_total[1h])) > 14.4 * 0.01
)
```

La ventana corta da velocidad; la larga suprime el falso positivo. **Qué pagina vs qué
tiquea**: página =
burn 14.4×/6×, `up == 0` por 2m, `arbx_circuit_breaker_open == 1` (núcleo §5.3), invariante
R7 muerta; ticket = burn 1×, deuda de observabilidad, rechazo alto pero estable. **Error
budget**: política escrita — al 100% consumido, **freeze automático** de features; solo
entran fixes de fiabilidad y reverts hasta recuperar. Budget agotado no es mala suerte: es
el sistema diciendo que la velocidad excedió la calidad.

## 22.8 GESTIÓN DE INCIDENTES

**Severidades con definición operativa** (no adjetivos). Se declara al abrir y se
re-clasifica explícitamente en el canal; un SEV2 "que al final era SEV1" se manejó mal
dos veces.

| SEV | Definición operativa | Ejemplos | Respuesta |
|---|---|---|---|
| SEV1 | capital en riesgo, pérdida de datos, o pipeline detect→execute muerto | breaker abierto con posiciones vivas; PG caído con escrituras perdidas; divergencia RPC > 500ms sostenida | page 24/7, mitigación < 15 min |
| SEV2 | degradación con workaround; SLI en burn sin budget crítico | 1 RPC caído con quorum 2/3 intacto; rechazo > 95% sostenido | page horario laboral, < 4 h |
| SEV3 | sin impacto de capital/usuario | dashboard degradado; alerta ruidosa; métrica ausente | ticket, próxima ventana |

**Roles**: IC (Incident Commander) decide y desempata, NO teclea — si el on-call está solo,
IC y Ops son la misma persona y lo primero que hace es pedir refuerzo; Ops ejecuta
mitigación (rollback, kill-switch, failover); Comms actualiza a stakeholders cada 30 min
(SEV1) / 2 h (SEV2) aunque el update sea "sin cambios"; Scribe construye el timeline con
fuente (alerta|log|decisión humana). Canal dedicado `#inc-YYYYMMDD-<slug>` desde el minuto
2: las decisiones van al canal, no al DM — el post-mortem se escribe del canal, no de la
memoria.

**Doctrina** (§37 Parte 5): mitigar primero (`git revert` + redeploy), entender después.
Pero ANTES de reiniciar cualquier proceso afectado, la evidencia se preserva:

```bash
# 1. Ventana de logs retenida (R9) y export del contenedor afectado
docker inspect searcher-rs --format '{{.HostConfig.LogConfig.Config}}'
docker logs searcher-rs --since "${INCIDENT_TS}" > evidence/searcher-rs.log 2>&1
# 2. Coredumps (docker cp ANTES de destruir el contenedor)
docker cp searcher-rs:/tmp evidence/coredumps/ 2>/dev/null || true
# 3. Snapshot de métricas (Prometheus con --web.enable-admin-api)
curl -fsS -XPOST http://prometheus:9090/api/v1/admin/tsdb/snapshot | jq -r .data.name
# 4. SOLO entonces: reiniciar / rollback
```

Un reinicio apurado sin 1-3 convierte "causa raíz desconocida" en "causa raíz recuperable
por nadie". **Timeline canónico**: T+0 detección (registrar FUENTE) → T+2m ack + severidad
+ canal → T+5m IC asignado + objetivo de mitigación declarado → T+15m SEV1: mitigación
aplicada O justificación ESCRITA de por qué no → T+30m comms update (y cada 30m) → all-clear
(síntoma eliminado, SLO recuperando, evidencia preservada) → post-mortem < 72h (SEV1/2).

## 22.9 POST-MORTEM BLAMELESS

Blameless es ingeniería, no amabilidad: si el 5-whys termina en "alguien se equivocó", el
análisis falló — la correctiva sería "humanos que no se equivocan", que no existe. Se busca
el proceso/herramienta que permitió el error o no lo atrapó.

```text
# PM-<fecha>-<slug>
Estado: [BORRADOR | REVISADO | ACCIONES CERRADAS]
Severidad / Duración: SEV_ / <detección → all-clear>
Impacto: cuantificado (capital expuesto, datos, minutos de SLI violado)
Timeline (UTC): una línea por evento con fuente (alerta|log|decisión)
Causa raíz: el mecanismo técnico (no "alguien tocó X")
5-Whys: cadena hasta proceso/sistema/gate ausente
Qué funcionó / qué falló: detección, mitigación, comms, runbook
Acciones (cada una con dueño, fecha y tipo):
  - [ ] A-1 <acción> — @dueño — YYYY-MM-DD — tipo: gate|tooling|doc|monitor
Regla de cierre (§37): revert aplicado + GATE NUEVO identificado (CI check,
alerta o guard). Sin gate nuevo = regresión con fecha de cita.
```

## 22.10 ON-CALL Y RUNBOOKS

**Runbook mínimo por servicio** (síntoma → diagnóstico → acción; una página máximo):

```text
## searcher-rs
SÍNTOMA: alerta NoDetections (rate(detected)=0 por 10m) con servicios up
DIAGNÓSTICO:
  1. docker logs searcher-rs --tail 200 | grep -iE 'error|reconnect'   (¿RPC vivo?)
  2. redis-cli XLEN arbx:opps:detected                                 (¿avanza? → aguas abajo)
  3. curl -fsS $RPC_PRIMARY -X POST -d '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'
ACCIÓN:
  - RPC caído → failover al secundario (quorum) + ticket a proveedor
  - Contenedor crasheado → preservar evidencia (§22.8) → redeploy digest previo
ESCALA: > 30m sin detecciones → declarar SEV2 y llamar IC
```

**Dashboards por servicio**: las 4 señales (latencia, tráfico, errores, saturación) + la
métrica de negocio propia; el dashboard del incidente debe existir ANTES del incidente
(construirlo durante el SEV1 añade minutos al T+15). **Dead-man switches** — la ausencia
de señal ES señal, dos capas:

```promql
absent(rate(arbx_opportunities_detected_total[10m]))          # métrica desaparecida → page
time() - max(arbx_service_last_success_unixtime{service="sim-ctl"}) > 300   # heartbeat explícito
```

(cada servicio expone el gauge unix-time de su último éxito; es convención a implementar,
no una métrica preexistente).

**Drills** (probar ANTES de necesitarlo; un kill-switch nunca disparado en drill es una
hipótesis — la primera vez que se necesite no puede ser la primera vez que se prueba):

| Drill | Cadencia | Éxito medido |
|---|---|---|
| Restore PITR (§22.3.2) | trimestral + T-7d pre-launch | RTO real y RPO real anotados |
| Kill-switch | mensual (staging) | trigger → 0 submissions nuevas en 1 bloque |
| Rollback de imagen | mensual | minutos desde decisión hasta smoke verde |
| RPC caído | mensual | quorum sostiene el pipeline; alerta correcta dispara |
| PG caído | trimestral | servicios a fail-honest (RULE 00/R8), cero datos fabricados |

## 22.11 CONTINUIDAD: RTO/RPO, FAILOVER, COSTE

| Componente | RPO objetivo | RTO objetivo | Mecanismo | Verificado en |
|---|---|---|---|---|
| PostgreSQL | ≤ 5 min (WAL) | 30 min | basebackup + PITR | restore drill trimestral |
| Redis streams | 0 aceptado (re-detectable) | < 1 min | reinicio; eventos regenerables | kill/restart drill |
| edge/frontend | n/a (sin estado) | 5 min | redeploy digest previo | rollback drill |
| relays-client (terminus) | n/a | inmediato | halt = estado seguro (default-deny) | kill-switch drill |
| RPC providers | n/a | < 1 min | failover quorum 2/3 | proveedor caído drill |

La asimetría es deliberada: perder el stream de Redis es tolerable (el universo observado
lo regenera); perder PG no. RTO/RPO se miden en drill, se publican en el runbook y se
re-miden cuando cambia la infra.

**Drills de dependencia caída**: *RPC down* — bloquear el primario (regla firewall/DNS de
prueba) y verificar failover al quorum, contador de failover incrementando, cero halt si el
quorum sobrevive, alerta SEV2 correcta. *Builder/relay down* — el terminus degrada a los
relés fallback configurados y, sin canal confiable, deja de submitir (estado seguro); jamás
"mempool público como última opción" sin decisión explícita (stealth routing, núcleo §4; la
postura frente a contrapartes ofensivas se rige por `arbx-mev-ethics-gate`). *DB down* — los
servicios de lectura caen a fail-honest con `reason` explícito (`discovery_failed`, etc.,
RULE 00/R8): el sistema degrada SIN fabricar datos.

**Coste por transacción y capacity planning**:

```text
coste_marginal_por_trade = gas_used × effective_gas_price + builder_tip + flash_loan_fee
$/detection  = coste_infra_diario / detecciones_día
$/simulation = coste_infra_diario / simulaciones_día
$/RPC        = CU consumidas / cuota mensual   (alerta al 80% de cuota — lección del repo)
```

El coste por trade se mide desde `arbx_gas_cost_wei` y los eventos de ejecución, y se
reporta junto al net profit por trade (núcleo §1.2): un canary "rentable" que ignora el
coste fijo de infra no es rentable. Capacity planning post-launch: pico observado ×2 como
headroom; crecimiento de disco medido en GB/día (con rollup activo) + ~20 GB por deploy
reservados al guard (§22.3.1); consumo de CU con alerta al 80% de cuota antes del overage.

## GOBERNANZA

Todo este playbook está subordinado a los gates `arbx-*` (paper-trade-first,
simulation-mandatory, risk-limits-enforcement, pre-execute-checklist) y a CLAUDE.md §34:
LIVE_MAINNET es default-deny en el terminus `relays-client`; su habilitación exige gates
PASS con evidencia reproducible + autorización del operador. Nada aquí autoriza flip a live
ni broadcast con capital real: el playbook prepara, opera y recupera — nunca aprueba.
