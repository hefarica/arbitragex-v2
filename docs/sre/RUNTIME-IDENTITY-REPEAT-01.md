# RUNTIME-IDENTITY-REPEAT-01 — identidad horneada del despliegue NUEVO (`3f00b359`)

- **Tarea:** t46 · **Intento:** 1 · **attempt_id:** `92427190-2215-4e8a-adfe-8b027633d5ec`
- **Rol:** SRE · **Rama:** `sre/runtime-identity-repeat-01` · **Base:** `origin/main` = `3f00b359beca82685280c5d8d30f099d8bd7d921`
- **In-scope tocado:** `docs/sre/` (este documento). **Cero archivos fuera de scope.**
- **Sonda y workflow:** ambos ya en `main` (`tools/arbx_identity_probe.sh` y `.github/workflows/runtime-identity-probe.yml`, 230 líneas, 6 steps). **No se modificó ninguno de los dos.**
- **Ventana:** `2026-10-06T02:18:24Z` → `02:34:30Z`.

---

## §0 — RESULTADO: los tres ejes coinciden en el despliegue NUEVO

**`3f00b359beca82685280c5d8d30f099d8bd7d921`** en los tres, medido sobre el runtime que ya servía ese SHA.

```
$ gh run view 37404762561 --log | (tabla de identidad, 10/10 imágenes)
service          verdict   built_from_sha                            declared_sha                              image_digest                                                        image_created                container_started_at          recreated
api-server       verified  3f00b359beca82685280c5d8d30f099d8bd7d921  3f00b359beca82685280c5d8d30f099d8bd7d921  sha256:bfe03a9bf7ba8e68b2148f9fdb2c3941fff218a56d88e9f61b570af525155ae0 2026-10-06T02:33:03.469288849Z 2026-10-06T02:33:11.742208231Z true
frontend         mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:720e3cb45f93c6b83477dd22c184d59cefb53fbd252884ede43bf069f9f58962 2026-10-06T02:33:04.861566857Z 2026-10-06T02:33:12.265535366Z true
edge             mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:9e0182ca2047bfec32e13ad2b426a04d1956a1f9eb788b4575b08e5fcc7faae7 2026-10-06T02:33:04.097095356Z 2026-10-06T02:33:12.05923645Z  true
searcher-rs      mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:0f9444ab544078869b2c1373364354ab58a5e46739788ebd45f59af3dc67f036 2026-10-06T02:32:55.32495473Z  2026-10-06T02:33:11.764113227Z true
selector-api     mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:8b620330fbca15fa786c78b8b03ac36a773fcb20ffdfa15ad61964a6fe65fd29 2026-10-06T02:32:55.304010248Z 2026-10-06T02:33:11.75118124Z  true
sim-ctl          mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:6f192bae8bc0497f7248df8e125bb4a4f15e9293f3941afd189338d0012130f9 2026-10-06T02:32:55.321152607Z 2026-10-06T02:33:11.75293205Z  true
recon            mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:82e8b5338690eed4407b0b27e146c572056a234c5331a7ca5bfe0164e3f48f10 2026-10-06T02:32:55.321249879Z 2026-10-06T02:33:11.71807076Z  true
token-enricher   mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:27e4c40fc94d1c62bc9303b2efeb7d2b724a339d443200e821892d3b00ba6fd1 2026-10-06T02:32:55.295959362Z 2026-10-06T02:33:11.739622222Z true
relays-client    mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:8f06b28cef5dc55437cb8a850952e9c2f595289133974f24f8b5be83b5d8c6f4 2026-10-06T02:32:53.831820087Z 2026-10-06T02:33:11.762736501Z true
math-engine      mismatch  3f00b359beca82685280c5d8d30f099d8bd7d921  (vacío)                                   sha256:ac760969143af6ed5ab3227c972a060c17ec6f3d5a753585fe1c69c0b7da6bcc 2026-10-06T02:32:52.857973291Z 2026-10-06T02:33:05.903550623Z true

verdict tally: verified: 1 | mismatch: 9
probe_exit_code=1   (1 = undeclared/mismatch)
```

---

## §1 — El SHA realmente desplegado, y la espera que lo respalda

El deploy nuevo **todavía estaba corriendo** cuando llegué. No medí contra el runtime viejo: lo esperé.

```
$ gh run list --workflow=auto-deploy-vps.yml --limit 3 --json databaseId,headSha,status,conclusion
37399887059  3f00b359beca  in_progress        # ← el run del SHA nuevo, encolado/en curso
37399866155  a296b2b21af8  completed cancelled
37399845470  4f370d8abb8c  completed failure

$ gh api repos/.../actions/runs/37399887059
sha=3f00b359beca82685280c5d8d30f099d8bd7d921 status=completed conclusion=success
run_started_at=2026-10-06T01:34:51Z updated=2026-10-06T02:33:41Z
  112064515610  Wait for all deployment gates  completed success  01:40:11Z → 02:03:14Z
  112071790075  Deploy to VPS                 completed success  02:03:16Z → 02:33:40Z
```

**La espera, con su conteo** (sondeo de `/status` cada ~45 s, más el estado del run):

| instante (UTC) | run de deploy | `/status.deploy.sha` |
|---|---|---|
| 02:18:24 → 02:32:26 (12 lecturas) | `in_progress` | `a38e6779` ← **runtime VIEJO** |
| **02:33:12** (13ª lectura) | `success` | **`3f00b359`** ← flip |

**Ninguna medición de identidad se hizo antes del flip.** Las dos corridas de la sonda (02:33:16Z y 02:34:08Z) ocurrieron **después** de que el runtime pasara a servir `3f00b359`.

**¿Coincide con `origin/main`?** SÍ: `git rev-parse origin/main` → `3f00b359beca82685280c5d8d30f099d8bd7d921`.

---

## §2 — Los tres ejes (más `origin/main`): COINCIDEN

| eje | valor | de dónde salió |
|---|---|---|
| **HORNEADO** (`org.opencontainers.image.revision`) | `3f00b359beca82685280c5d8d30f099d8bd7d921` | `docker image inspect -f '{{index .Config.Labels "org.opencontainers.image.revision"}}'` **en el host**, 10/10 imágenes (runs `37404691956` y `37404762561`) |
| **DECLARADO** (`deploy.sha` de `/status`) | `3f00b359beca82685280c5d8d30f099d8bd7d921` · `deploy.id=37399887059` · `deploy.at=2026-10-06T02:03:30Z` | `GET https://edge-arbx.ape-tv.net/status`, ts `2026-10-06T02:33:52.974Z`, `7` servicios `ok:200` |
| **RUN DE DEPLOY** | `head_sha=3f00b359beca82685280c5d8d30f099d8bd7d921` · `conclusion=success` · `run_started_at=2026-10-06T01:34:51Z` | `gh api repos/.../actions/runs/37399887059` |
| `origin/main` | `3f00b359beca82685280c5d8d30f099d8bd7d921` | `git rev-parse origin/main` |

**Sin discrepancias.** Los cuatro valores son el mismo SHA.

---

## §3 — La medición anterior NO se hereda (y hay prueba directa, no una promesa)

La ronda de t37 midió, con el mismo procedimiento y sobre el mismo host, **`a38e6779…`** en 10/10 imágenes. Esta ronda mide **`3f00b359…`** en 10/10. **El valor medido cambió con el despliegue**, que es exactamente la demostración de que aquella conclusión estaba atada a *ese* despliegue y no era un hecho permanente.

Prueba adicional e independiente: **los 10 digests de imagen son distintos** de los de la ronda anterior (p. ej. `api-server`: `sha256:db802380…` → `sha256:bfe03a9b…`), y `deploy.id` pasó de `37388790182` a `37399887059`.

**Si hubiera heredado la conclusión, este documento diría `a38e6779`.** Dice `3f00b359` porque se midió.

---

## §4 — Los 9 `mismatch` son del vocabulario cerrado, NO un hueco de integridad

Se conserva y se declara la misma limitación de la ronda anterior, sin maquillarla:

```
{"service":"frontend","verdict":"mismatch","declared_sha":"", ... "built_from_sha":"3f00b359beca82685280c5d8d30f099d8bd7d921","recreated_by_deploy":true}
{"service":"api-server","verdict":"verified","declared_sha":"3f00b359beca82685280c5d8d30f099d8bd7d921", ... "built_from_sha":"3f00b359beca82685280c5d8d30f099d8bd7d921","recreated_by_deploy":true}
```

`declared_sha` está **vacío** en esos 9 contenedores porque sólo `api-server` recibe `ARBX_DEPLOY_SHA` (`docker/compose.prod.yml:391-393`; ya documentado en `docs/integration/RUNTIME-IDENTITY-01.md:65`). Con `built` presente y `declared` vacío, el vocabulario cerrado de la sonda (`tools/arbx_identity_probe.sh:55-61`) cae en `mismatch`. **El `built_from_sha` es idéntico en las 10 imágenes** y `probe_exit_code=1` se explica por esos 9, no por una discrepancia entre valores.

**No se cambió la sonda para que dé verde**, y en esta tarea tampoco habría sido posible: `tools/` y `.github/workflows/` están fuera del in-scope de t46 (`docs/sre/`). La limitación se declara; el vocabulario lo arregla su dueño o se acepta como está.

Un dato que sí agrega la ronda nueva: los 9 `mismatch` **no ocultan** el eje horneado — el label se leyó igual en los 10, y los 10 coinciden con el SHA desplegado. `mismatch` describe *la ausencia de una segunda parte con la que comparar*, no un desacuerdo.

---

## §5 — Recreación de contenedores: `true` en 10/10

Con `ARBX_DEPLOY_START=2026-10-06T01:34:51Z` (el `run_started_at` del run de deploy `37399887059`, **no** el `deploy.at` declarado `02:03:30Z`):

`recreated_by_deploy = true` en **10/10** (columna de la tabla de §0). Coherente con el orden observado:

- `image_created` de las 10 imágenes: `02:32:52Z` – `02:33:04Z` (construidas por ese deploy)
- `container_started_at`: `02:33:05Z` – `02:33:12Z` — **posterior** a la creación de sus imágenes (8-10 s)
- el runtime sirve `3f00b359` desde `02:33:12Z` (poll 13)

Es decir: los contenedores en ejecución son los de las imágenes medidas. La frontera "label acredita la imagen, no que el proceso sea ese binario si el contenedor no se recreó" **no muerde aquí**, y se dice.

---

## §6 — Disciplina de salida, guardas y no-mutación

Las dos corridas cerraron **`completed/success`** con los **8/8 steps en verde** (`Set up job`, provenance, read-only guard, Setup SSH, Run probe, secret-leak guard, Publish identity, Complete job). Guardas, con su salida cruda:

```
probe_sha256=0fa9e0411d847b25fdd5b311005294e76dea1bf3326ca992d05d31676084876b
probe_bytes=3703
probe provenance OK (matches tools/arbx_identity_probe.sh)
docker verbs found in the probe: image inspect          (guard leído en el step 3; conclusión success)
read-only guard OK: verb inventory is a subset of {inspect,image}
leak guard = step 6 -> success   (el guard FALLA si encuentra un patrón de env/secreto/token)
```

- **Sólo identidad publicada:** `service`, `verdict`, `built_from_sha`, `declared_sha`, `image_digest`, `image_created`, `container_started_at`, `recreated_by_deploy`. Nada más.
- **PROHIBIDO y no hecho:** volcar env, secretos, tokens o `.env`. GitHub redacta además `VPS_HOST`/`VPS_USER`/`VPS_KEY` como `***` en el log.
- **Sin firmar, sin broadcast, sin mutar el VPS:** el inventario de verbos docker de la sonda es `{image, inspect}` — sólo lecturas. No se ejecutó ningún comando mutante, no se reinició nada, no se purgó nada.
- **SSH por el mismo mecanismo:** `VPS_SSH_KEY`/`VPS_SSH_HOST`/`VPS_SSH_USER`/`VPS_PORT`, clave a `~/.ssh/deploy_key` con `umask 077`, `ssh-keyscan -H`, `BatchMode=yes`, `StrictHostKeyChecking=yes` — idéntico a `auto-deploy-vps.yml:58-71`.
- **Ningún guard se disparó.** Si alguno hubiera saltado, el step habría fallado y se declararía aquí.

---

## §7 — Alcance: esto NO mueve P/N y no acredita nada más

**P/N sigue en 0/115.** Esta medición **no mueve P/N**, no cierra ningún criterio de negocio y **no convierte en verificado ningún otro criterio**. Acredita una sola cosa, y la acredita para *este* despliegue: que las imágenes que corren en el host llevan horneado el SHA `3f00b359…`, que coincide con lo declarado y con el run que las desplegó. Nada sobre comportamiento, economía o estrategia.

**Y no es permanente:** el valor dejará de ser válido en el próximo deploy. Esta es una medición con fecha, no un invariante.

---

## §8 — Reproducción

```
# el SHA realmente desplegado (antes de medir)
gh api repos/hefarica/arbitragex-v2/actions/runs/37399887059 --jq '.head_sha,.status,.conclusion'
curl -s https://edge-arbx.ape-tv.net/status   # -> deploy.sha / deploy.id / deploy.at

# la medición (idempotente, read-only, no muta el VPS)
gh workflow run runtime-identity-probe.yml --ref main \
  -f deploy_start="$(gh api repos/hefarica/arbitragex-v2/actions/runs/37399887059 --jq .run_started_at)"
gh run view <id> --log 2>&1 | Select-String 'revision|LABEL|digest|verified'

# runs de esta evidencia
#   37404691956  workflow_dispatch  success  (identidad, sin baseline)
#   37404762561  workflow_dispatch  success  (identidad + recreated_by_deploy=true)

# verify del contrato
gh run list --workflow=auto-deploy-vps.yml --limit 3 --json databaseId,headSha,status,conclusion
gh run view 37399887059 --json headSha,status,conclusion
```

**Comandos crudos y salidas:** todos los valores provienen de los runs `37404691956` / `37404762561`, de `gh api .../actions/runs/37399887059` y de `GET /status` a `ts=2026-10-06T02:33:52.974Z`. Ninguno se copió de la ronda de t37.
