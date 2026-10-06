# PIPELINE-INTEGRITY-SRE-INCREMENT-01 — lo que todavía faltaba tras el arreglo mergeado

**Rama:** `ci/pipeline-integrity-sre-increment-01` (base `origin/main` = `b1b2e600b24e046e07e68b036e3996817853f93b`) · **head** `5c5548917687f1cf0add0955d0b72610815e3c86`
**Alcance:** `.github/workflows/`, `docs/release/` · **1 archivo de workflow**. **NO merge.**
**Relación con `PR #839`:** aquel PR quedó **superado y en conflicto** (ver §1). Este informe y su rama lo reemplazan.

---

## 1. Colisión: el arreglo del instrumento ya estaba mergeado (medido)

Mientras corría esta tarea, otro agente mergeó **el mismo arreglo** a `main`. Cronología medida
(`git log --format='%h %ci %s' c89d21a..origin/main -- .github/workflows/pipeline-integrity.yml`):

| Commit | Fecha | Qué hizo |
|---|---|---|
| `b5e3037` | 2026-10-05 23:02:55 -0500 (= 04:02:55Z) | `fix(PIPELINE-INTEGRITY-REPAIR-01): el vigia vuelve a poder medir (materializa ssh + clasifica la causa)` |
| `1ceb037` | 2026-10-05 23:03:16 -0500 | `docs(...): saca el literal del alias del comentario` |
| `b295e16` | 2026-10-05 23:29:56 -0500 | `fix+docs(...): RETIRA F-CAPA3-EDGE-01 (R82) y corrige la sonda a la forma del edge` |
| `65903cc` | 2026-10-06 **07:36:20 -0500 (= 12:36:20Z)** | `Merge pull request #831 from hefarica/ci/pipeline-integrity-connect-01` |

Mi rama partía de `c89d21a3`, que quedó **39 commits atrás** de `main` (`b1b2e60`). Por eso:

- `GET /pulls/839` → `mergeable=false`, `mergeable_state=dirty`, `changed_files=2`, `+533/-63`.
- `git merge-tree --write-tree origin/main origin/ci/pipeline-integrity-repair-01` →
  `CONFLICT (content): .github/workflows/pipeline-integrity.yml` y
  `CONFLICT (add/add): docs/release/PIPELINE-INTEGRITY-REPAIR-01.md`.

Es decir: el mismo ID de tarea fue trabajado en paralelo, y el otro esfuerzo **llegó primero a
`main`**. Este informe **no re-clama** nada de eso: lo cita como ajeno y construye **encima**.

## 2. Lo que el arreglo mergeado ya hace bien (y este PR NO toca)

Leído verbatim del archivo en `main` (14050 bytes, 279 líneas):

- `Setup SSH (same mechanism as auto-deploy-vps.yml:58-71)` — misma materialización
  (`printf … > ~/.ssh/deploy_key`, `ssh-keyscan -T 10`, `chmod 600`).
- `Connectivity preflight — clasifica la causa, no la esconde` (`id: conn`) que publica
  `reachable=true|false` y corta con
  `::error::VIGIA SIN CONEXION (ssh rc=$rc) … Esto NO es 'el pipeline esta roto'.`
- Vocabulario de dos modos: `VIGIA=SIN_CONEXION` vs `PIPELINE ROJO capa N`.
- 5 capas (Redis, PG, API, WS, Docker) y sus umbrales.
- Resumen en `$GITHUB_STEP_SUMMARY` que declara cuál de los dos modos ocurrió.
- Su propio comentario mide dos hechos que **confirmo de forma independiente**: el VPS **no tiene
  `jq`** (`el VPS **no tiene jq** (command -v jq -> NO_JQ)`) y la tabla 2×2 de superficies
  (`8788 /api/… -> 200 · 6913 B`, `8788 /api/v1/… -> 404 · 21 B`, `8080 /api/… -> 404 · 161 B`,
  `8080 /api/v1/… -> 200 · 6218 B`).

**Ninguno de esos elementos se modifica aquí.** Verificado por diff: solo se agregan líneas.

## 3. Los dos huecos que quedaban (ambos medidos, no inferidos)

### Hueco 1 — un rojo ocultaba las capas siguientes

`main` gatea las 5 capas con `if: steps.conn.outputs.reachable == 'true'` (sin `always()`).
Medición, run **37411885810** (head `1ceb0378`, el mismo instrumento):

```
4 success  :: Layer 1 — Redis stream has recent detections
5 success  :: Layer 2 — PG has opportunities in last 5 minutes
6 failure  :: Layer 3 — API serves opportunities with correct types
7 skipped  :: Layer 4 — WS endpoint is alive
8 skipped  :: Layer 5 — Docker containers are healthy
```

Un rojo en la capa 3 dejó **2 de 5 capas sin medir**. Un vigía que deja de medir tras el primer
rojo entrega un dato, no un diagnóstico — y es la misma clase de defecto que esta tarea vino a
corregir (instrumento que no mide).

**Incremento:** `if: always() && steps.conn.outputs.reachable == 'true'` en las 5 capas. La puerta
de conectividad se conserva (sin conexión no se mide, correcto); lo que se elimina es el corte en
cascada. El job **sigue fallando** si una capa falla.

### Hueco 2 — el rojo de la capa 3 no declaraba su causa

La capa 3 pedía el cuerpo con `curl -sf … || echo '{}'`, de modo que **un HTTP 404/500 y un parser
que falla colapsaban en el mismo `COUNT=0`** y el log acusaba al pipeline:

```
::error::PIPELINE ROJO capa 3: API returns ZERO opportunities — pipeline broken between PG and API
```

Evidencia de que ese rojo puede ser falso, del run **37463450394** de esta misma sesión (sonda con
múltiples superficies, instrumento previo):

```
probe_url=http://127.0.0.1:8080/api/v1/opportunities/live?limit=1 http=200 bytes=8644
body_head={"count":1,"window_total":12,"window":"latest","viable_only":false,…
API /opportunities/live returns 0 items
amount_in_wei type = null
```

La API **respondió 200 con 8644 bytes** y aun así el paso reportó `0 items`: el `0` lo produjo el
parser (jq no está del lado del host, como documenta el propio `main`), no el pipeline. Un cero sin
causa atribuible **no es un hecho del pipeline**: es el modo de fallo (d), leer una etiqueta como
si fuera otra cosa.

**Incremento:** la capa 3 ahora

1. captura el **código HTTP** (`curl -s -m 10 -w '\n%{http_code}'`, mismo URL, misma superficie) y
   un `HTTP != 200` se reporta como *"la API respondio HTTP <code>, no 200 — NO es 'cero
   oportunidades'"*;
2. declara **el parser**: si `jq` falla (`JQ_RC != 0`) o devuelve vacío, el veredicto es
   `NO COMPUTADO capa 3 … Un 0 aca NO es un hecho del pipeline` en vez de un rojo del pipeline;
3. imprime `evidence: http=… bytes=… keys=…` y `evidence: amount_in_wei_as_string=…`, de modo que
   cualquier rojo de esta capa sea reproducible desde su propio log;
4. lee `amount_in_wei` también de `.items` (la forma que sirve el edge), **sin cambiar el umbral**
   (sigue aceptando `string` o `null`).

## 4. Verificación del parche

Validador sobre el YAML parseado (salida real):

```
yaml_ok=1 steps=8        gates_always=5
ssh_arbx=0   localhost=0   ssh_i=6   mutantes=0
captura_http_code=True   no_computado_parser=True
umbral_cero_intacto=True vocabulario_peer_intacto=True
bash_n_ok=8  bash_n_fail=0
```

`bash -n` se ejecutó sobre los **8 bloques `run:`** extraídos del archivo (no sobre el YAML entero).
`mutantes=0` = ninguna de las sondas escribe: `docker exec … redis-cli XLEN`, `psql -c "SELECT …"`,
`curl` GET, `docker ps`.

## 5. Medición de producción (independiente, de esta sesión)

Del run **37463450394** (12:37:33-12:37:47Z), capas medidas con su cifra en el log:

| Capa | Salida literal | Veredicto |
|---|---|---|
| connection | `VERDICT=PASS layer=connection` | PASS |
| redis_stream | `Redis XLEN arbx:opps:detected = 10000` | PASS |
| pg_flow | `PG opportunities in last 5 min = 508` | PASS |
| ws_alive | `VERDICT=PASS layer=ws_alive` | PASS |
| docker_health | `VERDICT=PASS layer=docker_health` | PASS |

Tabla 2×2 de superficies **medida por mí** en ese mismo run, que coincide con la del arreglo
mergeado (coincidencia independiente, no copia):

```
8080 /api/opportunities/live    -> 404 · 160 B     (main midio 404 · 161 B)
8080 /api/v1/opportunities/live -> 200 · 8644 B    (main midio 200 · 6218 B)
8787 /api/opportunities/live    -> 200 · 8644 B    (main mide 8788 /api/... -> 200 · 6913 B)
8787 /api/v1/opportunities/live -> 404 ·  21 B     (main mide 8788 /api/v1/... -> 404 · 21 B)
```

## 6. Estados de los runs de esta tarea — sin adornos

| Run | head | Estado | Lectura correcta |
|---|---|---|---|
| `37428952107` | `c89d21a3` | `completed/failure` | `ssh: Could not resolve hostname arbx` / exit 255; capas 3-6 `skipped`. El instrumento no midió. |
| `37462970330` | `9d3cbffc` | `completed/failure` | Llegó al VPS. `connection`/`redis`/`pg` PASS. Capa 6 roja, 7-8 `skipped`. |
| `37463450394` | `abb71a47` | `completed/failure` | **Las 6 capas medidas** (5 verdes + `api_types` FAIL atribuido al parser). |
| `37464780122` | `3f4eb353` | **`completed/cancelled`** | **NO midió.** Cancelado a las `13:16:19Z`, 3 s después de encolar el run `37469480262` (`13:16:16Z`): `concurrency: group: pipeline-integrity / cancel-in-progress: true` (L64-66). Un cancelado no es verde ni rojo: es **no medido**. |
| `37469480262` | `5c554891` | `pending` al cerrar este informe | Encolado; el pool de runners está saturado (12 runs ajenos `IN_PROGRESS`, un lote de 12 workflows encolado a `12:57:39Z`). **No se le atribuye resultado.** |

## 7. Alcance económico y de producto

- **P/N 0/115 intacto.** No se toca ledger, contador, oportunidad, cartucho ni tabla.
- **Sin firma, sin broadcast, sin deploy, sin capital.** `mutantes=0` en los bloques `run:`.
- **`ARBX_DRIFT_TRACKER_MODE` y demás palancas de producto: intactas.** El diff de la rama contra
  `main` es **1 archivo de workflow** (+ este documento).
- **No se modifica ningún umbral, capa ni vocabulario del arreglo ya mergeado.**

## 8. Qué NO prueba este artefacto

1. **La rama no es `main`**: el incremento no vigila todavía lo desplegado hasta que se mergee.
2. **El run de verificación (`37469480262`) seguía `pending`** al cerrarse este informe: el
   incremento está validado por YAML + `bash -n` + diff, **no** todavía por una ejecución real.
   Cuando corra, su resultado se reporta como suplemento — no se anticipa.
3. **La saturación del pool y el `cancel-in-progress`** hacen que dos ejecuciones simultáneas del
   mismo workflow se anulen entre sí: medir y no medir compiten por el mismo grupo.

## 9. Reproducción

```bash
git log --format='%h %ci %s' c89d21a..origin/main -- .github/workflows/pipeline-integrity.yml
git merge-tree --write-tree origin/main ci/pipeline-integrity-repair-01 | grep CONFLICT
gh api repos/hefarica/arbitragex-v2/actions/runs/37411885810/jobs --jq '.jobs[0].steps[] | "\(.number) \(.conclusion) :: \(.name)"'
gh api repos/hefarica/arbitragex-v2/actions/runs/37463450394/jobs --jq '.jobs[0].steps[] | "\(.number) \(.conclusion) :: \(.name)"'
gh run view 37463450394 --repo hefarica/arbitragex-v2 --log | grep -E 'VERDICT=|probe_url=|answered_by='
gh api repos/hefarica/arbitragex-v2/actions/runs/37464780122 --jq '"\(.status)/\(.conclusion) updated=\(.updated_at)"'
```
