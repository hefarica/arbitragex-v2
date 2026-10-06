# POSTGRES-SHM-01 — `/dev/shm` de 64 MB mata la agregación paralela (503 `query_failed`)

- **Tarea:** t65 · **Intento:** 1 · **attempt_id:** `a3348b3a-fecb-4972-87b1-277fe46d467a`
- **Rol:** SRE · **Rama:** `sre/postgres-shm-01` · **Base:** `origin/main` = `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`
- **In-scope tocado:** `docker/compose.prod.yml` + `docs/sre/` (este documento). **Cero archivos fuera de scope.**
- **Identidad ANTES del primer commit:** `GET /status` → `deploy.sha=c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`, `deploy.id=37405962576`, `ts=2026-10-06T04:26:35.937Z`. `origin/main` = el mismo SHA.
- **Modo:** medición + fix de configuración. Sin firma, sin broadcast, sin capital.

---

## §0 — RESUMEN, CON LA FRONTERA ENTRE LO MEDIDO Y LO HEREDADO

| pieza | estado | quién la midió |
|---|---|---|
| **Síntoma** `503 {"error":"query_failed"}` en `/api/rejections/breakdown` | ✅ **reproducido por mí** (2 veces, cuerpo crudo) | yo, vía HTTP |
| Control de canal (`/api/v1/health` → 200) | ✅ **medido por mí** | yo, vía HTTP |
| **Error crudo de PostgreSQL** (`could not resize shared memory segment … 50438144 bytes … No space left on device`) | ⚠️ **NO COMPUTADO por mí** — heredado y etiquetado como tal | capitán, por `docker exec psql` |
| **`df -h /dev/shm` del CONTENEDOR = 64 MB** | ⚠️ **NO COMPUTADO por mí** — heredado | capitán |
| **El fix** (`shm_size: 1gb` en `docker/compose.prod.yml`) | ✅ **hecho y validado** | yo |
| **Run real posterior → 200** | ⛔ **BLOQUEADO** — requiere merge + deploy, que no son míos | — |

**Por qué no pude correr psql/df yo mismo, con las tres razones verificadas en esta sesión:**
1. **El canal shell al VPS no existe en esta estación** (establecido en `t7` y no revertido): los 6 binarios OpenSSH mueren con exit 255 y 0 bytes; el TCP:22 responde.
2. **El canal SQL del tooling está mudo**, re-verificado ahora mismo en esta tarea:
   `sql_query: SELECT 1 AS ping;` → `{"ok": true, "data": "", "rows_affected": 0}` — una query que **siempre** devuelve exactamente una fila. **No es un cero: es ausencia de transporte.**
3. **Ningún workflow ofrece input libre** de SQL/command (revisado en `t54`: los 24 dispatchables sólo aceptan SHAs, run-ids, flags de confirmación o modos acotados). Y `.github/workflows/` está **fuera del in-scope** de t65.

**En consecuencia:** los criterios «reproducir el error crudo por psql» y «medir `df -h /dev/shm` antes y después» **no los puedo acreditar yo**. Se citan como **evidencia heredada del capitán, marcada como heredada** — exactamente el estándar que este equipo aplica (`§34.5.3`: artefactos reproducibles; sin artefacto propio, se declara).

---

## §1 — El síntoma, reproducido por mí (canal alcanzable)

```
$ curl -s -o /dev/null -w '%{http_code}' 'https://edge-arbx.ape-tv.net/api/rejections/breakdown?hours=24'
503

$ (cuerpo crudo, SkipHttpErrorCheck)
HTTP 503  content-type=application/json; charset=utf-8  bytes=24
body=[{"error":"query_failed"}]
  idem con &chain_id=1 → HTTP 503  body=[{"error":"query_failed"}]
  /api/v1/rejections/breakdown?hours=24 → HTTP 404   (la ruta vive en /api/…, no en /api/v1/…)
```

**Control de canal — la ruta no está "caída", el api-server contesta:**
```
$ curl -s .../api/v1/health
HTTP 200  {"system_status":"healthy","math_guardian":"passed","entropy":0.5837,...}
```

**El 503 es el camino honesto declarado, no un crash:** `backend/api-server/src/routes/rejection-breakdown.ts` **L301-302**:
```ts
} catch (e) {
  deps.logger.error({ err: (e as Error).message, route: "rejection_breakdown" }, "query_failed");
  res.status(503).json({ error: "query_failed" });
```

**Corrección de cita (el archivo cambió desde el diagnóstico):** el task cita «el SQL de `rejection-breakdown.ts:25-44`». En el archivo **actual (290 líneas)** ese rango es el comentario de cabecera y los imports. El SQL que muere está en:
- **L173-188** — el GROUP BY sobre `opportunities` (`SELECT split_part(rejection_reason, ':', 1) AS family_raw, … FROM opportunities … GROUP BY …`) ← **la agregación paralela**;
- **L189-192** — los totales (`SELECT COUNT(*)::int AS total, … FROM opportunities …`);
- **L132-139** — `timedQuery`, que envuelve cada agregado en su propia transacción con `SET LOCAL statement_timeout = 15 s` (**`BREAKDOWN_STATEMENT_TIMEOUT_MS`, L56**).

Ésa es la pieza de código que explica el 503: el agregado es *elegible para paralelismo* y su timeout lo hace **fallar honesto** en vez de colgar el pool. **No lo medí yo** que el paralelismo sea lo que falla; la correlación causal viene del error crudo del capitán (§0). Lo que sí verifiqué es que la ruta consultada es esa y que el 503 sale de ese `catch`.

---

## §2 — Cuál compose usa la producción de VERDAD (verificado, y la cita estaba desactualizada)

El task decía `auto-deploy-vps.yml:211`. **En el `main` actual la línea es la 153**, con el mismo valor:

```
$ Select-String .github/workflows/auto-deploy-vps.yml -Pattern 'COMPOSE_FILE'
  L153: COMPOSE_FILE="docker/compose.prod.yml"     ← producción
  L219: if [ ! -f "$COMPOSE_FILE" ]; then
  L220:   echo "ERROR: $COMPOSE_FILE missing after git reset at $(pwd)"
  L252:   docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d --remove-orphans postgres redis
  L276:   docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" logs --tail=40 postgres
  L303:   docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d …
  L322:   docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" build --no-cache "$service"
  L329:   docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d --force-recreate --remove-orphans …
  L389:   docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" ps
  L397:   docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" exec -T --interactive=false redis …
```

**Confirmado: `docker/compose.prod.yml`.** Y no es una suposición sobre el archivo, es el archivo que el pipeline usa en **cada** operación de compose.

**Chequeo negativo (no había fix previo en ninguna parte):**
```
docker/compose.prod.yml -> 0 coincidencias de shm_size
docker/compose.dev.yml  -> 0 coincidencias de shm_size
```

**Decisión de alcance, declarada:** el fix va **sólo** en `docker/compose.prod.yml`. `docker/compose.dev.yml` **no lo usa la producción** y tocarlo sería scope creep sin efecto sobre el defecto. Si algún día se levanta el stack de dev con cargas comparables, heredará el mismo límite de 64 MB — y eso es un hallazgo aparte, no este fix.

**Y el efecto es automático en el próximo deploy:** `auto-deploy-vps.yml:252` y `:329` recrean `postgres` explícitamente (`up -d … postgres redis` / `--force-recreate`). `shm_size` sólo se aplica al **crear** el contenedor, así que el deploy siguiente lo materializa sin ningún paso manual extra.

---

## §3 — El fix

`docker/compose.prod.yml`, servicio `postgres` (L23-73), inserción después de `image: postgres:15`:

```diff
@@ -23,6 +23,18 @@ services:
   postgres:
     logging: *id001
     image: postgres:15
+    # POSTGRES-SHM-01 (2026-10-06) — Docker's default /dev/shm is 64 MB. PostgreSQL
+    # parallel workers allocate DSM segments in /dev/shm; the observed request was
+    # 50,438,144 bytes (~48 MiB) and died with:
+    #   could not resize shared memory segment "/PostgreSQL.<pid>" to 50438144 bytes:
+    #   No space left on device
+    # surfacing as 503 query_failed on /api/rejections/breakdown. The limit is the
+    # CONTAINER's, not the host's (host /dev/shm had 7.7 GB free; disk had 31 GB).
+    # 1 GB = ~20x the observed segment and covers concurrent parallel aggregates.
+    # /dev/shm is tmpfs: this budget is RAM (16 GB host), never disk. Raising shm is
+    # the canonical fix — disabling parallel query instead would degrade EVERY
+    # aggregate to hide a container misconfiguration.
+    shm_size: 1gb
     # PERF-STACK WO-4 (2026-09-20): tuned for the 8-core/16GB VPS running the
```

**Diff: `1 file changed, 12 insertions(+), 0 deletions(-)`** — es una **inserción pura**: no se modificó ni una línea existente. (En el primer intento mi propio script pegó la línea siguiente al `shm_size`; lo detecté en el diff, lo corregí y verifiqué que el comentario `PERF-STACK WO-4` quedara intacto. Se declara porque un diff sucio se habría colado como "cambio no intencional".)

**Validación (PyYAML, sobre el archivo real):**
```
services: 25
postgres.image      = postgres:15
postgres.shm_size   = '1gb'
postgres.healthcheck= True
postgres.volumes    = ['postgres_data:/var/lib/postgresql/data', '../database/init:/docker-entrypoint-initdb.d:ro']
shm_size en otros servicios: ninguno
VALIDACION OK
```

**Sizing, declarado:**
- La petición observada fue **50.438.144 B ≈ 48,1 MiB** para **un** segmento.
- `work_mem=32MB` (L34) y `hash_mem_multiplier` por defecto (2,0) implican que un agregado paralelo puede pedir del orden de decenas de MB **por worker**, y varios agregados concurrentes se suman.
- **`1gb` ≈ 20× el segmento observado**, con holgura para concurrencia.
- **Es RAM, no disco:** `/dev/shm` es tmpfs y descuenta de la RAM del host (16 GB, con `shared_buffers=1GB`). Si el operador prefiere ser conservador, `512m` también cubriría el segmento observado; se elige `1gb` por holgura y por ser el valor estándar recomendado para contenedores PostgreSQL.

**La palanca correcta es `shm_size`, y NO se tocó `max_parallel_workers_per_gather`:** bajarlo a 0 globalmente taparía un límite de contenedor mal configurado degradando **todas** las agregaciones. No se hizo y no debe hacerse.

---

## §4 — El run real posterior al fix: **BLOQUEADO, con el paso exacto que falta**

El contrato pide «un run REAL posterior al fix devuelve 200 y cuerpo con datos». **No puedo producirlo yo**, y no lo maquillo:

1. El fix vive en `docker/compose.prod.yml`, y ese archivo llega al VPS por el checkout del deploy (`git reset --hard $TARGET_SHA`, `auto-deploy-vps.yml:196`) disparado por **push a `main`**.
2. El contrato de t65 dice **NO mergees**; y el mandato de la célula es **sin deploy**. Por lo tanto **el archivo no puede llegar a producción por mi mano**.
3. Sin contenedor recreado, `shm_size` no se aplica (`shm_size` no es actualizable en caliente: `docker update` no lo soporta) → el `/dev/shm` sigue en 64 MB y la ruta sigue en 503.

**Pasos exactos que faltan (ninguno es mío):**
```
1) mergear el PR de esta rama a main       (operador / revisor independiente)
2) esperar el auto-deploy del push a main  (recrea postgres: auto-deploy-vps.yml:252 y :329)
3) verificar, en ese orden:
   docker exec arbitragex-v2-postgres-1 df -h /dev/shm           # esperado: ~1G en /dev/shm
   curl -s -o /dev/null -w '%{http_code}' \
     'https://edge-arbx.ape-tv.net/api/rejections/breakdown?hours=24'   # esperado: 200
   docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"  # control: fila
```
El **control** (`SELECT 1`) está en la lista a propósito: distingue «el fix funcionó» de «el canal dejó de contestar». Un 200 sin el control no probaría nada.

**Registro de lo que sí queda medido hasta aquí:** el 503 con su cuerpo crudo (2 lecturas) y el 200 de `/api/v1/health`. La cadena «shm 64 MB → error de PostgreSQL → 503» está **documentada con evidencia del capitán** y su eslabón de código verificado por mí (`L173-192` + `L301-302`), pero **el eslabón causal no lo medí yo**. Un `PASS` de esta tarea no debe leerse como «ya funciona»: se lee como «el fix está aplicado en el archivo correcto y validado».

---

## §5 — Alcance del defecto

**Qué quedaba afectado:** toda consulta con **agregación paralela** sobre las tablas grandes del ledger — el planner habilita paralelismo cuando el coste lo justifica y el objeto es grande:
- `opportunities` — **8.060.571** filas
- `pool_reserves` — **35.022.648** filas
- `route_discovery_outcomes` — **~19 M por día** (3 particiones vivas)

(cifras provistas por el capitán; **no las re-medí**, y por eso van sin marca de medición propia).

**Qué ruta concreta desbloquea en la superficie HTTP:** `/api/rejections/breakdown` (el GROUP BY de `L173-188`), hoy en **503** medido por mí.

**La distinción que NO hay que mezclar (`t60`):** la medición `g2_simulations_passed -> 0` de `t60` se hizo **por el canal de GitHub Actions**, que va por `docker exec psql` **y no pasa por el api-server**. Ese camino **no** atraviesa el pool de PG del api-server ni la ruta HTTP, así que **no estaba afectada por este defecto y sigue válida**. Lo que este fix desbloquea es el **canal HTTP**; no cambia, no invalida y no re-mide `g2_simulations_passed`.

**Lo que este fix NO es:** no es un defecto de producto ni de la lógica de negocio. Es **infraestructura localizada** (un límite de contenedor por defecto que nadie configuró) con fix canónico de una línea. La etiqueta `No space left on device` es de PostgreSQL hablando de su segmento de memoria compartida — **no del disco** (31 GB libres) **ni del `/dev/shm` del host** (7,7 GB libres).

---

## §6 — Alcance y límites de esta entrega

**Esto NO mueve P/N (0/115).** No se firma, no se emite, no se toca capital: `ARBX_TRADE_MODE=paper`, sin broadcast, sin deploy. El cambio es **una clave de configuración** en el compose que producción usa; su efecto se materializa **sólo** cuando un deploy recrea el contenedor, y ese acto no es de esta tarea.

**Fronteras declaradas:** no toqué `backend/`, `frontend/`, `.github/workflows/`, `implementation-state/`, `docs/governance/`, `docs/release/`, `docs/data/`. No corrí SQL contra producción. No medí `df` ni el error crudo (razones en §0). No mergeé.

---

## §7 — Procedencia y reproducción

| # | Comando (crudo) | Resultado |
|---|---|---|
| 1 | `curl -s -o /dev/null -w '%{http_code}' '.../api/rejections/breakdown?hours=24'` | **503** |
| 2 | `Invoke-WebRequest ... -SkipHttpErrorCheck` (cuerpo crudo) | `HTTP 503 · application/json · bytes=24 · body=[{"error":"query_failed"}]` (idem con `&chain_id=1`) |
| 3 | `curl -s .../api/v1/health` (control de canal) | **200** `system_status=healthy` |
| 4 | `Select-String .github/workflows/auto-deploy-vps.yml -Pattern COMPOSE_FILE` | **L153** `COMPOSE_FILE="docker/compose.prod.yml"`; recreación de postgres en L252/L329 |
| 5 | `Select-String docker/compose.{prod,dev}.yml -Pattern shm_size` | **0 coincidencias** en ambos (antes del fix) |
| 6 | `python <validador> docker/compose.prod.yml` (PyYAML) | `services: 25`, `postgres.shm_size='1gb'`, `shm_size en otros servicios: ninguno`, `VALIDACION OK` |
| 7 | `git diff --stat docker/compose.prod.yml` | `1 file changed, 12 insertions(+), 0 deletions(-)` |
| 8 | `sql_query: SELECT 1 AS ping;` | `{"ok":true,"data":"","rows_affected":0}` ← **canal mudo** (no es un cero) |
| 9 | `GET /status` (identidad antes del commit) | `deploy.sha=c89d21a3437c…`, `deploy.id=37405962576`, `ts=2026-10-06T04:26:35.937Z` |

**Evidencia heredada, marcada como tal:** el error crudo de PostgreSQL y los valores de `df -h /dev/shm` (contenedor 64 MB, host 7,7 GB libres) y los conteos de filas de las tablas grandes provienen de la medición del capitán (2026-10-06T04:2xZ). Se citan para cerrar el diagnóstico; **no los presento como mediciones propias**.
