# RUNTIME-PROBE-ACTIONS-01 — Identidad HORNEADA leída desde Actions

- **Tarea:** t37 · **Intento:** 1 · **attempt_id:** `197dfffb-52dc-43d7-94f2-41b916339f59`
- **Rol:** SRE · **Rama:** `sre/runtime-identity-probe-01` · **Commit del workflow:** `a9114be7960bd0e4371acf649d84b259878ca6f9`
- **Clon aislado:** sparse checkout (`--filter=blob:none --no-checkout --depth 1`) fuera del checkout compartido; base `origin/main`.
- **Workflow añadido:** `.github/workflows/runtime-identity-probe.yml` (solo `workflow_dispatch`, read-only).
- **Ventana:** `2026-10-06T01:03Z` → `01:08Z`.

---

## §0 — RESULTADO: la identidad HORNEADA se obtuvo, y `verified` es alcanzable

El objetivo de t7 se cierra. La sonda corrió **desde GitHub Actions contra el VPS** por el mismo canal SSH que usa el deploy, y devolvió el label `org.opencontainers.image.revision` **leído de las imágenes en el host**.

```
$ gh run view 37397569735 --log | (filas de la tabla de identidad)
service          verdict   built_from_sha                            declared_sha                              image_digest                                                        image_created                container_started_at          recreated_by_deploy
api-server       verified  a38e677923ad76e0179547ed662bb904a374f7af  a38e677923ad76e0179547ed662bb904a374f7af  sha256:db80238028b194f5c7fefeae9b6482e86a6516ec92662e6b85045f54b39fa50e 2026-10-06T00:29:11.418761232Z 2026-10-06T00:29:19.583439286Z true
frontend         mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:4414ef50d71ba05c2d0bd0174048a169b8931061db813d03685ddd07e1cbcc16 2026-10-06T00:29:12.793218462Z 2026-10-06T00:29:20.092832543Z true
edge             mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:4b2c063b34eccd884c5e4d6512ee3d7e93392fc78565863e4df6ac9c30bff554 2026-10-06T00:29:11.990593327Z 2026-10-06T00:29:19.902345987Z true
searcher-rs      mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:516ed70321e5aa88dac6fcf437c730fcbc5f0888571279a67f84125f35381722 2026-10-06T00:29:02.826923069Z 2026-10-06T00:29:19.595057797Z true
selector-api     mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:5b5546a7ccd7ce5e51ac9f0a7a2baf1b7ac0be14a2b25202a38c0619cda82447 2026-10-06T00:29:02.807819053Z 2026-10-06T00:29:19.587413514Z true
sim-ctl          mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:0ddf9b17cf88b985dc86119a0179f511c4bad5215d8a94e28ede6072d478f1b9 2026-10-06T00:29:02.82301697Z  2026-10-06T00:29:19.578461534Z true
recon            mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:7f48a921fe8055cc8125df613de6bc1440da436c8cf106f13c5a5168919a377c 2026-10-06T00:29:02.823583985Z 2026-10-06T00:29:19.514781092Z true
token-enricher   mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:6f3511e3ebac5595673e80e67e9a90158f333966de4996e528028d785af29667 2026-10-06T00:29:02.800553762Z 2026-10-06T00:29:19.590850231Z true
relays-client    mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:b547f910da43a7ed9438592762b3b39ba295d28b3c8e7c0e5b8fe4069bdd59d5 2026-10-06T00:29:01.91117954Z  2026-10-06T00:29:19.586297116Z true
math-engine      mismatch  a38e677923ad76e0179547ed662bb904a374f7af  (vacío)                                   sha256:c31b5e9e1a5aa432289a4bd0528aaa82d3e04d32b4c13e3f1bc6b59030499c78 2026-10-06T00:29:00.696504802Z 2026-10-06T00:29:13.704040453Z true

verdict tally: verified: 1 | mismatch: 9
probe_exit_code=1   (0=all measured, none stale | 1=undeclared/mismatch | 2=stale | 3=docker absent | 255=ssh failure)
```

**`built_from_sha` es el mismo en las 10 imágenes** y **NO es `ABSENT`**: la expansión `ARG ARBX_BUILD_SHA` → `LABEL` ocurrió en un `docker build` real. Eso convierte en obsoleta la frase del encabezado de la sonda (L24-27) y de `implementation-state/CHECKPOINT.md:246` — *"no hay evidencia de que la expansión `$ARBX_BUILD_SHA` funcione en un build real"*. Hoy hay evidencia, medida en el host.

---

## §1 — Comparación contra `deploy.sha` y contra `origin/main`: COINCIDEN

```
$ Invoke-WebRequest https://edge-arbx.ape-tv.net/status   (raw, ts=2026-10-06T01:08:29.369Z)
deploy.sha = a38e677923ad76e0179547ed662bb904a374f7af
deploy.id  = 37388790182
deploy.at  = 2026-10-05T23:59:40Z
env        = production-like      services ok:200 = 7

$ git -C <clon aislado> rev-parse origin/main
a38e677923ad76e0179547ed662bb904a374f7af

$ (identidad horneada, 10/10 imágenes) built_from_sha
a38e677923ad76e0179547ed662bb904a374f7af
```

| eje | valor | fuente |
|---|---|---|
| HORNEADO (`org.opencontainers.image.revision`) | `a38e677923ad76e0179547ed662bb904a374f7af` | `docker image inspect` en el host (10/10) |
| DECLARADO (`deploy.sha` de `/status`) | `a38e677923ad76e0179547ed662bb904a374f7af` | `GET /status`, ts `2026-10-06T01:08:29.369Z` |
| `origin/main` | `a38e677923ad76e0179547ed662bb904a374f7af` | `git rev-parse origin/main` |

**Los tres coinciden.** No es una inferencia desde el SHA de `main`: el valor horneado se leyó con `docker image inspect -f '{{index .Config.Labels "org.opencontainers.image.revision"}}'` sobre las imágenes que sirven los contenedores en ejecución. La coincidencia con `origin/main` es un hallazgo *a posteriori*, no el método.

---

## §2 — Qué acredita esto y qué NO (el límite, declarado)

**Acredita — y es nuevo respecto de t7:**

1. **La cadena `build ↔ SHA`, extremo a extremo:** el label está en la imagen (no en el env del contenedor), con el mismo SHA en 10/10. El eslabón que faltaba —`ARBX_DEPLOY_SHA` → `build.args` de compose → `ARG` del Dockerfile → `LABEL` de la imagen— quedó **medido en el host**.
2. **La recreación de los contenedores por ese deploy.** Con `ARBX_DEPLOY_START=2026-10-05T23:29:38Z` (el `run_started_at` del run de deploy `37388790182`, no el `deploy.at` declarado), el resultado es `recreated_by_deploy = true` en **10/10**. Además `container_started_at` (00:29:19,58Z) es **8 s POSTERIOR** a `image_created` (00:29:11,41Z) para `api-server`. Es decir: **no es el caso de "contenedor viejo con imagen nueva"**.
3. **`verified` es alcanzable.** Para `api-server`, horneado == declarado y las dos partes son no vacías.

**NO acredita — declarado explícitamente:**

1. **Que el proceso dentro del contenedor corresponda a ese commit.** El label es un valor que el **build** escribió a partir de un `ARG`; no hay attestation de contenido (no hay `cosign`/SLSA sobre estas imágenes, y `org.opencontainers.image.revision` no es una firma). Lo que se midió es la **procedencia declarada por el build**, ahora sí inmutable dentro de la imagen.
2. **El origen del valor horneado sigue siendo una inyección:** `docker/compose.prod.yml:125,196,238,273,312,355,394,473,516,683,771` → `ARBX_BUILD_SHA: ${ARBX_DEPLOY_SHA:-unknown}` y `auto-deploy-vps.yml:163` → `export ARBX_DEPLOY_SHA="$TARGET_SHA"`. Lo que cambia con este run es que el `ARG` **ya no es una promesa estática**: se expandió en 10 imágenes reales.
3. **El límite que el contrato pedía declarar, en su forma general:** leer un label acredita `build ↔ SHA` de la **imagen**; **NO** acredita que el proceso en ejecución sea ese binario si el contenedor no se recreó. **En este caso concreto esa condición NO se da** (`recreated_by_deploy=true` y `StartedAt > image_created`), y por eso se dice: la frontera existe pero aquí no muerde. Si en una medición futura `recreated_by_deploy=false` o `stale`, la conclusión debe degradarse a `declared_only` para ese servicio.
4. **Nada sobre comportamiento, capital o P&L.** Ver §6.

---

## §3 — El `mismatch` de 9/10 NO es una discrepancia de valores

Es importante no leerlo como un gap de integridad. En esos 9 servicios, `declared_sha` está **vacío**: no hay ningún valor con el que comparar.

```
{"service":"frontend","verdict":"mismatch","declared_sha":"","declared_at":"", ... "built_from_sha":"a38e677923ad76e0179547ed662bb904a374f7af","recreated_by_deploy":true}
{"service":"api-server","verdict":"verified","declared_sha":"a38e677923ad76e0179547ed662bb904a374f7af","declared_at":"2026-10-05T23:59:40Z", ... "built_from_sha":"a38e677923ad76e0179547ed662bb904a374f7af","recreated_by_deploy":true}
```

El vocabulario cerrado de la sonda (`tools/arbx_identity_probe.sh:55-61`) hace:

```
if [ "$built" != "ABSENT" ]; then
  if [ -n "$declared" ] && [ "$built" = "$declared" ]; then v="verified"; else v="mismatch"; fi
```

Con `built` presente y `declared` vacío cae en `mismatch`. La causa del vacío es conocida y está en el propio repo: **sólo `api-server` recibe `ARBX_DEPLOY_SHA`** (`docker/compose.prod.yml:391-393`), y `docs/integration/RUNTIME-IDENTITY-01.md:65` ya lo documentaba. Por eso `probe_exit_code=1`.

**Lectura correcta:** `mismatch` aquí significa "no hay segunda parte para comparar", no "los valores difieren" — el `built_from_sha` es el mismo en las 10. Se reporta tal cual y **no se corrige**: `tools/` está fuera del in-scope de esta tarea (in-scope: `.github/workflows/`, `docs/sre/`). Queda como hallazgo para quien sea dueño de la sonda.

---

## §4 — Disciplina de salida: qué se publicó y qué se blindó

**Publicado (y nada más):** `service`, `verdict`, `built_from_sha`, `declared_sha`, `image_digest`, `image_created`, `container_started_at`, `recreated_by_deploy`. La tabla se construye con `jq` sobre el JSON de la sonda, cuyos campos son identity-only por construcción (`arbx_identity_probe.sh:72-74`).

**Tres guards, los tres en verde en el run bueno:**

| guard | qué asegura | evidencia en el log |
|---|---|---|
| Provenance | los bytes ejecutados son los previstos | `probe_sha256=0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b` · `probe_bytes=3703` · `probe provenance OK (matches tools/arbx_identity_probe.sh)` |
| Read-only | el inventario de verbos `docker` es subconjunto de `{inspect,image}` | `docker verbs found in the probe: image inspect` · `read-only guard OK: verb inventory is a subset of {inspect,image}` |
| Secret-leak | el payload no trae env/secretos/tokens | `leak guard OK: no env/secret/token pattern in the published payload` |

El guard de fuga busca y **no imprime** las líneas que casan: `_key=`, `_token=`, `password=`, `passwd=`, `secret=`, `private_key`, `database_url=`, `minio_root_`, `admin_token`, `ghp_`, `github_token`, `bearer `, `BEGIN … PRIVATE KEY`. Si dispara, falla el job sin publicar.

**Lo que NO se volcó:** variables de entorno de los contenedores, `.env`, tokens, claves. El log de Actions muestra `VPS_HOST: ***`, `VPS_USER: ***`, `VPS_KEY: ***` (redacción nativa de GitHub), y ninguna línea con contenido de `.env`. El único `docker inspect` que toca el env es interno a la sonda, que **extrae sólo** `ARBX_DEPLOY_SHA` y `ARBX_DEPLOYED_AT` por `grep '^ARBX_DEPLOY_SHA='` (`arbx_identity_probe.sh:46-48`) — es decir, identidad, no entorno.

**Mecanismo SSH: el mismo del deploy, no uno nuevo.** `VPS_SSH_KEY` / `VPS_SSH_HOST` / `VPS_SSH_USER` / `VPS_PORT`, clave a `~/.ssh/deploy_key` con `umask 077`, `ssh-keyscan -T 10 -H`, `BatchMode=yes`, `StrictHostKeyChecking=yes` — copiado de `.github/workflows/auto-deploy-vps.yml:58-71`.

---

## §5 — Read-only: la sonda no mutó el host

`docker verbs found in the probe: image inspect`. Ni `rm`, `rmi`, `stop`, `kill`, `restart`, `prune`, `down`, `up`, `exec`, `run`, `build`, `tag`, `commit`, `load`, `save`, `volume`, `network`, `system`, `swarm`, `context`. Sin firma, sin broadcast, sin escritura remota. El workflow sólo hace `docker inspect` / `docker image inspect` vía `ssh … 'bash -s'`.

---

## §6 — Alcance: esto NO mueve P/N y no verifica nada más

**P/N permanece en 0/115.** Este resultado **no** mueve P/N, no cierra ningún criterio de negocio, y **no convierte en verificado ningún otro criterio**: acredita la procedencia imagen↔SHA del build desplegado y la recreación de los contenedores por ese deploy. Nada más. En particular **no** dice nada sobre: que la estrategia opere, que las oportunidades sean rentables, la economía real de las cards, ni los criterios de aceptación viva de la matriz.

---

## §7 — Hallazgo colateral: la sonda NO está en el repositorio

`tools/arbx_identity_probe.sh` existe sólo como **archivo no trackeado** del checkout local:

```
$ gh api repos/hefarica/arbitragex-v2/contents/tools/arbx_identity_probe.sh
gh: Not Found (HTTP 404)

$ git -C <checkout compartido> ls-files --error-unmatch tools/arbx_identity_probe.sh
error: pathspec 'tools/arbx_identity_probe.sh' did not match any file(s) known to git

$ git -C <clon> ls-tree -r --name-only origin/main | grep identity_probe
(ninguna línea)
```

Consecuencia: el workflow **no puede invocarlo como archivo**; embebe su cuerpo **verbatim** (3703 B, LF, `sha256 0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876B`) y verifica el hash en runtime, de modo que cualquier divergencia entre lo ejecutado y el archivo medido aborta el job. Si se quiere que la sonda sea la fuente única, hay que **trackearla** — fuera del in-scope de esta tarea (`tools/`).

---

## §8 — Dos fallos de mi propio instrumento (declarados, no maquillados)

| run | qué pasó | causa | arreglo |
|---|---|---|---|
| `37397216835` | falló en el step de provenance | comparé el hash de `sha256sum` (minúsculas) contra una constante en MAYÚSCULAS | normalizo ambos lados con `tr 'A-F' 'a-f'`. El log prueba que el contenido era correcto: `probe_bytes=3703`, hash idéntico salvo el caso |
| `37397289654` | falló mi guard read-only | el patrón `docker inspect.*>` marcaba `2>/dev/null` (falso positivo) | lo reemplacé por un **inventario de verbos**: sólo `{inspect, image}` |

Ninguno de los dos fue un problema del VPS, de la sonda ni del canal. Los dos fueron instrumentación propia mal escrita y ambos se detectan por su propio log.

También hubo un YAML inválido en el primer push (el terminador del heredoc quedó en columna 0, cerrando el bloque escalar antes de tiempo): detectado con `yaml.safe_load` antes de confiar en él y corregido indentándolo.

---

## §9 — Reproducción

```
# 1) dispatch read-only (sin baseline de recreación)
gh workflow run runtime-identity-probe.yml --ref sre/runtime-identity-probe-01

# 2) dispatch con el baseline de recreación (run_started_at del deploy, NO deploy.at)
gh workflow run runtime-identity-probe.yml --ref sre/runtime-identity-probe-01 \
  -f deploy_start="$(gh api repos/hefarica/arbitragex-v2/actions/runs/37388790182 --jq .run_started_at)"

# 3) verificación
gh run view <id> --json status,conclusion,jobs
gh run view <id> --log 2>&1 | Select-String 'revision|LABEL|digest|verified'

# runs de esta evidencia:
#   37397425099  workflow_dispatch  success  (identidad, sin baseline)
#   37397569735  workflow_dispatch  success  (identidad + recreated_by_deploy=true)
```

**Artefactos y comandos crudos:** todos los valores de este documento provienen de esos dos runs y de `GET /status` a `ts=2026-10-06T01:08:29.369Z`; ningún número se copió de otro informe.
