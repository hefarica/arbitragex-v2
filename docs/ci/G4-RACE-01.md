# G4-RACE-01 — un deploy que aterriza bien cierra en `failure`: el gate que espera 3 servicios y el que exige 7

**Alcance:** `.github/workflows/`, `scripts/ci/`, `docs/ci/`. Todo lo demás se leyó **read-only**.
**Sin merge, sin `--force`, sin `--admin`, sin saltarse un gate, sin tocar el VPS/runtime/contenedores, sin reiniciar nada.**

---

## 0. Resumen (cada número con su artefacto)

1. **La carrera está reproducida con reloj**, no argumentada (§2): `[8/9]` espera **3** condiciones y pasó en **1 s**;
   `[9/9b]` exige **7** servicios y corrió a los **25,0 s** del arranque del stack, cuando `recon` y `relays-client`
   seguían `health: starting`.
2. **El deploy aterrizó** y el gate **pasa hoy** con los mismos argumentos: el validador devuelve `rc=0` y los 7
   `verified_services` (§5, control A).
3. **El arreglo elige la vía (a) implementada sin duplicar la lista**: la espera reutiliza **el propio predicado de G4**
   (mismo script, mismos argumentos, mismo endpoint), con presupuesto **acotado** y reintento **sólo** para la familia de
   readiness (§4).
4. **Control de vacuidad obligatorio, ejecutado**: con el arreglo puesto, un SHA incorrecto **sigue fallando** (`rc=1`,
   `G4_CONDITION=served_deployment_identity_mismatch`) y **sin reintentar** (§5, control B + test dedicado).

---

## 1. El hecho medido, verbatim

Run **`37783694993`** (`workflow_dispatch` no: `event=push`), `headSha=77b42b3dccc001455fda3e8d4437d973c9e98c4b`,
`conclusion=failure`, `2026-10-08T13:21:23Z → 14:16:36Z`.

Salida literal del gate (`gh api repos/hefarica/arbitragex-v2/actions/jobs/113344193804/logs`, L11202-11204):

```
14:16:32.1083646Z [9/9b] G4 DEPLOY-VERAZ assertion (post-deploy truth)...
14:16:32.6308776Z ##[error]Served API identity/health evidence missing, stale or invalid
14:16:32.6348425Z ##[warning]Deploy FAILED (rc=1) after docker compose down. Auto-restoring stack...
14:16:32.7266183Z Stack is UP (api-server running) — no restore needed.
```

**Ese `##[error]` es el mensaje genérico**, y es el defecto de observabilidad del §3: nombra el *hecho*
("missing, stale or invalid") pero **no la condición** que lo disparó.

Estado del runtime en ese mismo instante (dump `docker ps` del propio run, log L11179-11201):

```
arbitragex-v2-recon-1          … Up 24 seconds (health: starting)   127.0.0.1:3004->3004/tcp
arbitragex-v2-relays-client-1  … Up 24 seconds (health: starting)   127.0.0.1:3005->3005/tcp
arbitragex-v2-searcher-rs-1    … Up 24 seconds (healthy)
arbitragex-v2-selector-api-1   … Up 24 seconds (healthy)
arbitragex-v2-sim-ctl-1        … Up 24 seconds (healthy)
arbitragex-v2-token-enricher-1 … Up 24 seconds (healthy)
arbitragex-v2-math-engine-1    … Up 30 seconds (healthy)
```

---

## 2. ★ LA REPRODUCCIÓN (con los tiempos, antes de tocar nada)

`started_at` / `completed_at` de cada step del run (`gh api .../actions/runs/37783694993/jobs`):

| job | step | conclusion | started_at | completed_at |
|---|---|---|---|---|
| Wait for all deployment gates | 3 · Verify same-SHA main-push gates | success | 13:22:58Z | 13:46:10Z |
| Deploy to VPS | 5 · **Deploy to VPS via SSH** | **failure** | 13:47:07Z | 14:16:32Z |
| Deploy to VPS | 6-10 · evidencia | skipped | 14:16:32Z | 14:16:32Z |

Los `[n/9]` viven **dentro** del step 5; su reloj está en el log del job:

| instante (UTC) | marca | delta |
|---|---|---|
| 14:16:01.143 | arrancan los contenedores del stack | — |
| 14:16:06.885 / 14:16:07.139 | arrancan `relays-client-1` / `recon-1` | — |
| 14:16:07.512 | `[7b/9]` minio | — |
| 14:16:08.157 | `[7c/9]` thanos | — |
| 14:16:31.545 | `[8/9]` **inicia** la espera de salud | — |
| 14:16:31.703 | **`healthy after 1s (api-server + edge via localhost; searcher-rs="Health":"healthy")`** | la espera tardó **1 s** |
| 14:16:31.900 | `[9/9]` dump de `docker ps` (el de arriba) | — |
| **14:16:32.108** | **`[9/9b]` G4** | **+25,0 s** desde el arranque del stack |
| 14:16:32.630 | falla G4 | +0,5 s |

**Lo que espera y lo que exige no son lo mismo**, con archivo y línea:

- **Lo que espera (`[8/9]`)**: `auto-deploy-vps.yml:470-491` → tres condiciones: `curl 127.0.0.1:8080/api/health`
  (L472), `curl 127.0.0.1:8787/health` (L473) y `searcher-rs` healthy por compose (L477-478). Al agotar 90 intentos
  **hace `exit 1`** (L482-488) — o sea, la espera ya sabe fallar.
- **Lo que exige (`[9/9b]`)**: `scripts/ci/verify_deploy_identity.py:15`
  `SERVICES = ("selector-api", "sim-ctl", "recon", "relays-client", "searcher-rs", "math-engine", "token-enricher")`
  — **siete**, exigidos `ok is True` y `status == 200` en L60-63.

⇒ `recon` y `relays-client` están en la lista de G4 y **no** en la de `[8/9]`. Esa asimetría es la carrera, y se
reprodujo: a los 25,0 s los dos estaban `health: starting` (snapshot del propio run) y el gate los exigió igual.

---

## 3. ★ EL RELOJ: por qué 25 s era poco *por construcción*, y el defecto de observabilidad

### 3.1 Cuánto tarda cada servicio (medido read-only sobre el runtime vivo, sin reiniciar nada)

Configuración de healthcheck **idéntica en los 7**: `interval=30s timeout=5s retries=5 start_period=30s`,
`start_interval=0s` (`docker inspect --format '{{.Config.Healthcheck.*}}'`, los 7 contenedores).

| servicio | `StartedAt` | probe (Start/End, exit) | test del healthcheck |
|---|---|---|---|
| `recon` | 2026-10-08T14:16:07.116Z | 14:24:33.492 / 14:24:33.540 exit=0 | `CMD bash -c 'exec 3<>/dev/tcp/127.0.0.1/3004'` |
| `relays-client` | 2026-10-08T14:16:06.841Z | 14:24:38.266 / 14:24:38.318 exit=0 | `wget -q -O- http://localhost:3005/health` |
| `searcher-rs` | 2026-10-08T14:16:07.102Z | 14:24:13.416 / 14:24:13.552 exit=0 | `wget -q -O- http://localhost:9001/health` |
| `selector-api` | 2026-10-08T14:16:07.114Z | 14:24:13.950 / 14:24:14.079 exit=0 | `node -e … get('http://localhost:3002/health')` |
| `sim-ctl` | 2026-10-08T14:16:07.119Z | 14:24:13.364 / 14:24:13.453 exit=0 | `wget -q -O- http://localhost:3003/health` |
| `math-engine` | 2026-10-08T14:16:01.129Z | 14:24:37.469 / 14:24:37.527 exit=0 | `wget -q -O- http://localhost:3006/health` |
| `token-enricher` | 2026-10-08T14:16:07.113Z | 14:24:23.523 / 14:24:23.589 exit=0 | `metrics` + `arbx_enricher_up 1` + frescura < 180 s |

**Los probes de cada contenedor están a 30 s exactos** (5 entradas consecutivas por contenedor, p. ej. `recon`:
`14:23:03.318 → 14:23:33.378 → 14:24:03.432 → 14:24:33.492 → 14:25:03.541`).

Consecuencia medida: los probes de `recon` caen en el offset `:03/:33` y los de `relays-client` en `:08/:38`.
**G4 corrió a las `14:16:32.108`; el siguiente probe de `recon` fue a las `14:16:33.49`** — la aserción adelantó
**1,4 s** al propio reloj del healthcheck. Y el de `relays-client`, 6,2 s.

⇒ **25 s no era "un poco lento": era imposible.** Dos servicios exigidos por el gate tenían su primer veredicto
de salud *después* de que el gate los juzgara. No es una carrera de milisegundos: es una aserción que se adelanta
al muestreo.

### 3.2 El defecto de observabilidad (declarado, y **corregido** en vez de conservado)

`scripts/ci/verify_deploy_identity.py:79-82` (antes): **diez** condiciones distintas — `invalid_utc_timestamp`,
`duplicate_json_key`, `non_finite_json`, `status_payload_size_invalid`, `expected_identity_invalid`,
`api_status_not_healthy`, `served_deployment_identity_mismatch`, `status_timestamp_not_current`,
`services_evidence_missing`, `upstream_not_healthy:<servicio>` — colapsaban en **un** mensaje genérico.
Por eso hubo que reproducir el validador a mano: **la condición no era nombrable desde el log**.

Se elige **hacerla nombrable**, con el mínimo cambio posible:

- se añade `CONDITION_PATTERN` + `condition_code(exc)` (vocabulario **cerrado**: `[a-z][a-z0-9_]*(:[a-z0-9-]+)?`);
- el `except` imprime además `G4_CONDITION=<código>` **en stdout**;
- **NO cambia**: el mensaje genérico de stderr (sigue literal), el **exit code** (1), `validate()`, `SERVICES`,
  ni un solo criterio de aceptación;
- el servicio que aparece en `upstream_not_healthy:<servicio>` sale de `SERVICES` (constante del script), **nunca**
  del payload ⇒ sigue sin poder filtrar contenido de upstream; si el texto no encaja en el vocabulario se reporta
  `unclassified`. El test `test_wrong_served_identity_fails_without_body_leak` (que mete `DO_NOT_PRINT` en el
  payload) sigue pasando.

---

## 4. ★ EL ARREGLO: vía elegida, por qué, y qué pasa si un servicio no levanta nunca

**Vía (a) — que la espera cubra lo mismo que el gate — implementada reutilizando el predicado del gate**,
con presupuesto acotado (que es lo único bueno de (b)). Se descarta escribir una **segunda** lista de 7 servicios:
sería una lista que puede divergir de la que G4 exige, que es exactamente la clase de defecto de esta tarea.

Bloque `[9/9a]` insertado en `.github/workflows/auto-deploy-vps.yml` entre `[9/9]` y `[9/9b]`:

- hace `curl ... /status` y lo pasa por **el mismo** `verify_deploy_identity.py` con **los mismos** argumentos
  (`$TARGET_SHA`, `$DEPLOY_RUN_ID`, `$ARBX_DEPLOYED_AT` — los tres ya exportados en L223/L104, sin plumbing nuevo);
- **reintenta sólo** la familia de readiness: `status_unavailable` | `api_status_not_healthy` |
  `services_evidence_missing` | `upstream_not_healthy:*`;
- **cualquier otra condición falla en el primer intento** (identidad, timeline, cuerpo malformado/gigante) con
  `::error:: G4 readiness wait: '<condición>' is not a readiness condition — failing now, no retry`;
- presupuesto **`G4_WAIT_ATTEMPTS=45 × G4_WAIT_SLEEP=2s = 90 s`**, alineado con el tope de 90 s que `[8/9]` ya usa;
- **si un servicio no levanta nunca**: se agota el presupuesto, imprime
  `::error::G4 readiness not reached in 90s (last condition=<condición>)` y **`exit 1`** ⇒ el deploy **FALLA**,
  nombrando el servicio. Un servicio muerto sigue siendo un servicio muerto; el arreglo **no puede** convertirlo
  en `success`.
- **No puede fabricar un verde**: la espera **no decide nada**. `[9/9b]` corre inmediatamente después, con los
  mismos argumentos, y es lo único que puede aprobar. Si la espera se equivocara en cualquiera de sus ramas, el
  gate la desmiente.

**Camino crítico:** en un deploy sano la espera cuesta **una** llamada al validador —medida en vivo: `curl` a
`/status` = **7,6 ms** (`time_total=0.007599s`) + un `python` sobre 583 B— y **cero** `sleep`. No alarga nada
cuando no hay nada que esperar. Solo gasta presupuesto cuando el plano operativo está genuinamente arrancando, que
es el caso que hoy cierra en rojo. En el peor caso (servicio muerto) un run que **ya iba a fallar** tarda hasta
**+90 s** sobre un deploy de **29 min** (13:47:07 → 14:16:32), y a cambio gana una razón nombrable.

**Cambio en el orden de fallo, declarado:** una identidad servida incorrecta ahora falla en `[9/9a]` (antes que en
`[9/9b]`). Sigue siendo un fallo, con el mismo predicado y el mismo exit code; lo que cambia es que falla **antes**
y con la condición **nombrada**. La cobertura del rechazo por identidad del gate se conserva a nivel unitario
(`IdentityTests.test_healthy_http_payload_with_wrong_sha`, etc., que llaman `validate()` directo).

---

## 5. ★★ CONTROL DE VACUIDAD (obligatorio): con el arreglo puesto, un SHA incorrecto SIGUE FALLANDO

### 5.1 Contra el runtime vivo, con el validador **ya modificado**

Payload real: `curl -s http://195.201.235.70/api/status` (583 B, `ok=true`,
`deploy={"sha":"77b42b3d…","id":"37783694993","at":"2026-10-08T13:47:10Z"}`); ejecución
`python scripts/ci/verify_deploy_identity.py --sha … --run-id … --deployed-at …` con el payload por stdin.

| # | caso | rc | stdout |
|---|---|---|---|
| **A** | camino real (sha despachado) | **0** | recibo JSON: `"sha":"77b42b3d…"`, `"run_id":"37783694993"`, `"verified_services":[los 7]` |
| **B** | **sha incorrecto** (`000…0`) | **1** | `G4_CONDITION=served_deployment_identity_mismatch` |
| **C** | run-id incorrecto (`1`) | **1** | `G4_CONDITION=served_deployment_identity_mismatch` |
| **D** | `deployed-at` incorrecto | **1** | `G4_CONDITION=served_deployment_identity_mismatch` |
| **E** | control negativo: `9090` público | **7** | `external_9090_http_code=000` (Prometheus no es público; la vía es loopback por ssh) |

⇒ **A prueba que el gate estaba bien y el deploy aterrizó**; **B/C/D prueban que no se volvió permisivo**.

### 5.2 Contra el **texto real del workflow**, ejecutado (tests en repo)

`scripts/ci/test_deploy_completion.py` ejecuta el **tail real** del workflow con adaptadores (`docker`, `curl`,
`git`, `python3`) — sin SSH, sin Docker de producción, sin RPC, sin firma. Tres tests nuevos:

| test | qué prueba | aserción |
|---|---|---|
| `test_g4_readiness_wait_cannot_swallow_a_wrong_identity` | **CONTROL DE VACUIDAD** | rc≠0 · **sin** `G4 PASS:` · `served_deployment_identity_mismatch` presente · `no retry` presente · **`readiness attempt` AUSENTE** (el bucle no se activó) |
| `test_g4_readiness_wait_retries_a_readiness_condition_then_gate_asserts` | el arreglo **sí** cierra la carrera | secuencia `[recon no-listo, listo]` ⇒ `upstream_not_healthy:recon` → `readiness OK after 2 attempt(s)` → `G4 PASS:` → **rc=0** |
| `test_g4_readiness_wait_exhausts_and_fails_when_a_service_never_comes_up` | servicio muerto **no** se vuelve success | `upstream_not_healthy:relays-client` ⇒ `readiness not reached in 0s` ⇒ **rc≠0**, sin `G4 PASS:` |

**Suite completa: `Ran 31 tests … OK`, rc=0** (`python scripts/ci/test_deploy_completion.py`) — 28 previos sin
regresión + los 3 nuevos, incluido `test_workflow_shell_syntax` (`bash -n` sobre el workflow real, que valida el
bloque insertado).

---

## 6. ★ EL DIFF: aditivo, acotado, y qué NO se tocó

```
$ git diff --cached --stat origin/main
 .github/workflows/auto-deploy-vps.yml |  57 +
 docs/ci/G4-RACE-01.md                 | 284 +
 scripts/ci/test_deploy_completion.py  |  68 +-
 scripts/ci/verify_deploy_identity.py  |  19 +-
 4 files changed, 428 insertions(+), 3 deletions(-)
```

**No se tocó**: `validate()`, `SERVICES` (los 7), el exit code del validador, el mensaje genérico de stderr, el
bloque `[9/9b]` (L510-529), el bucle `[8/9]`, las condiciones de ningún otro paso, ningún otro workflow,
`docker/compose*`, ni un solo archivo fuera de `.github/workflows/`, `scripts/ci/`, `docs/ci/`.
El arreglo **no obligó a tocar nada más**; si lo hubiera exigido, esta tarea paraba y lo declaraba.

---

## 7. ★ EFECTO SOBRE LO YA HECHO (sin reescribir historia)

`gh run list --workflow auto-deploy-vps.yml --limit 8` — **tasa medida, no impresión**:

| run | conclusion | headSha | created |
|---|---|---|---|
| **37783694993** | **failure** | `77b42b3d` | 2026-10-08T13:21:23Z |
| 37749114208 | success | `901eb947` | 08:19:51Z |
| 37738680784 | success | `d1a4c3f5` | 06:38:01Z |
| 37728772398 | success | `8414e512` | 04:42:30Z |
| 37722666338 | success | `80e2f86c` | 03:25:40Z |
| 37715116308 | success | `9ccd1d04` | 01:53:01Z |
| 37708390646 | success | `fa6f5284` | 00:33:27Z |
| 37700927145 | success | `32743333` | 2026-10-07T23:12:39Z |

⇒ **7 `success` / 1 `failure` en los últimos 8 runs (12,5%)**: la carrera **NO es sistemática**, es una
**varianza de arranque** (los 7 anteriores cerraron `success`). **Todo deploy que aterrizó pudo cerrar en
`failure`** por esta misma aserción adelantada, y en esta ventana lo hizo **una vez**. Los veredictos ya emitidos
**no se reescriben**: se acotan con este conteo. La carrera solo se manifiesta cuando el arranque del plano
operativo se estira más allá de los ~25 s que el workflow concede.

---

## 8. ★★ LA PRUEBA DE FUEGO ES UN DEPLOY REAL

El arreglo vive en una rama y se publica como **PR en DRAFT**. El run que lo sirve es el **próximo ciclo real de
deploy** (un aterrizaje por ciclo, ~57 min), y **no se re-dispara el deploy para fabricar el verde**.

Estado de esta prueba a la fecha de este documento: **PENDIENTE — no puede completarse antes de que el PR se
incorpore**, porque `auto-deploy-vps.yml` corre desde `main`. Se declara así en vez de simularlo:

- **no se fuerza**: no se re-dispatcha el deploy, no se usa `--force`, no se salta un gate;
- **criterio de aceptación del próximo ciclo**: `conclusion=success` **y** `/api/status` sirviendo el SHA
  despachado (el mismo recibo del control A);
- **si vuelve a cerrar en `failure`**, el arreglo **NO funcionó** y se dirá — y el log, ahora, **nombrará la
  condición** (`G4_CONDITION=…`), que es precisamente lo que faltaba en `37783694993`.

---

## 9. Límites declarados (lo NO medido)

- **No se midió** el intervalo con que el **api-server** sondea sus 7 upstreams (no está en sus logs de la
  ventana). Lo medido es: el gate lee el mapa `services` del api-server, y su boot
  (`{"event":"service.boot","upstreams":[los 7]}` a las `14:16:08.55`, log del api-server) nombra **los mismos 7**
  que el validador ⇒ la lista de 7 no es inventada por el instrumento.
- **No se midió** el tiempo exacto a `healthy` de `recon`/`relays-client` desde el arranque: la ventana del
  health log de Docker (5 entradas) ya había rotado. Lo medido es la **cota**: seguían `health: starting` a los
  +24,8 s y su siguiente probe era a `14:16:33` / `14:16:38`, **después** de G4 (`14:16:32.108`).
- **No se tocó el VPS**: toda la evidencia de runtime es read-only (loopback por ssh, `docker inspect`,
  `docker logs`, `docker ps`). Ningún reinicio, ninguna escritura, ningún `docker exec` mutante.
- **Wart de instrumento propio, declarado:** el display de `stderr` vacío en el control A lanzó un
  `InvalidOperation` de PowerShell (`$se.Trim()` sobre vacío). Es cosmético: los `rc`, `stdout` y `stderr` de los
  cinco casos se capturaron por redirect a archivo. No afecta ningún valor reportado.
- **Cero cambios a la lógica de identidad** y **cero cambios al gate en sí**: el validador sigue devolviendo 1 y el
  mismo mensaje genérico; sólo añade el nombre de la condición en stdout.
