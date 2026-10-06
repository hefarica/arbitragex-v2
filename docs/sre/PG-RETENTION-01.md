# PG-RETENTION-01 — la mitad VIVA de #732: VACUUM acotado y error observable

**Rama:** `sre/pg-retention-01` (base `origin/main` = `b1b2e600b24e046e07e68b036e3996817853f93b`)
**Alcance:** `scripts/pg_retention.sh` + este documento. **Sin merge.**
**Cierra:** t75 · **Origen:** la parte viva de #732 (`OPS-PGSHM-01`, 2026-09-30), sin su `shm_size`.

---

## 1. Estado real de #732 (criterio 1)

```
$ gh pr view 732 --json state,mergeable,files,headRefOid
{"files":[{"path":"docker/compose.prod.yml","additions":19,"deletions":0,"changeType":"MODIFIED"},
          {"path":"scripts/pg_retention.sh","additions":19,"deletions":3,"changeType":"MODIFIED"}],
 "headRefOid":"585955c7589a764d78d31128d139c73dee6b92b1",
 "mergeable":"CONFLICTING",
 "state":"OPEN"}
```

**#732 está OPEN, `CONFLICTING`, head `585955c7`, 2 archivos.** El conflicto es real y tiene una
causa concreta: `main` ya movió `docker/compose.prod.yml` por su cuenta (#832).

## 2. Su parte de `shm` quedó OBSOLETA (criterio 2)

```
$ git show origin/main:docker/compose.prod.yml | grep -n shm_size
37:shm_size: 1gb
```

`main` ya tiene **`shm_size: 1gb`** (línea 37, por #832). **NO se reintroduce `2gb`**: subirlo más
exige una medición que lo justifique, y `1gb` es 16× el default de Docker que causaba el fallo.
Por eso la mitad `docker/` de #732 **no se re-materializa** — y `docker/` está fuera del alcance
de esta orden.

## 3. Qué se re-materializa y qué se descarta (criterio 3)

| Parte de #732 | Decisión | Motivo |
|---|---|---|
| `VACUUM (ANALYZE, PARALLEL 0)` | **SE RE-MATERIALIZA** | Es la mitad viva: quita el segmento DSM en `/dev/shm`. Medido por #732 en el VPS: `VACUUM (ANALYZE, PARALLEL 0) pool_reserves` → OK. |
| Error observable en lugar de `>/dev/null 2>&1` | **SE RE-MATERIALIZA** | Es el defecto que convirtió un límite de contenedor en 24 h de deploys congelados (ver §4). |
| `shm_size: 2gb` en `docker/compose.prod.yml` | **DESCARTADO** | Obsoleto: `main` ya tiene `1gb` por #832 (§2). Además `docker/` está fuera de alcance. |
| `TABLES`: `opportunities`, `scored_opportunities`, `opportunity_observations` de 3 → **60 días** | **DESCARTADO** | Es **política de datos**, no instrumento: cambia cuánto se borra y por tanto el disco y el ritmo de bloat. Merece su propia medición y su propia decisión; no entra en una orden de "restaurar el instrumento". Queda nombrado, no colado. |

Diff contra `main`: **1 archivo, +31/−4** (`scripts/pg_retention.sh`), 0 cambios en `docker/`.

## 4. Por qué sigue valiendo aunque el `shm` esté arreglado (criterio 4)

**La línea donde el error se descartaba** (`main`, `scripts/pg_retention.sh` L543-544):

```bash
      -c "VACUUM (ANALYZE) $tbl" >/dev/null 2>&1 \
      || log "retention.vacuum table=$tbl failed (non-fatal)"
```

Dos defectos en dos líneas: `2>&1 >/dev/null` **tira el mensaje de error** (el `log` no dice la
causa, ni rc, ni SQLSTATE) y el resumen final no cuenta el fallo, así que el único rastro de un
VACUUM roto es una línea idéntica todos los días.

La cadena que #732 midió el 2026-09-30, eslabón por eslabón — **y cuál eslabón se corta acá**:

```
/dev/shm = 64 MB (default Docker)   vs   maintenance_work_mem = 512 MB
   ↓  (1) TODO VACUUM de retención falla
   ↓  (2) y el error se traga en /dev/null          ← ESTE eslabón: lo corta este cambio
   ↓  (3) los DELETE nunca devuelven espacio → bloat permanente (1.7 GB de índices sobre 3 filas)
   ↓  (4) disco bajo el umbral de 15 GB del disk_guard
   ↓  (5) TODOS los auto-deploys fallan → producción congelada >24 h
   ↓  (6) los parches de seguridad no llegan a runtime
```

Con `shm_size: 1gb` el eslabón (1) puede dejar de ocurrir — **pero (2) seguía intacto**: mientras
el script no sepa *por qué* falló, la próxima vez que el VACUUM falle por cualquier otra causa
(bloat distinto, `maintenance_work_mem` subido a futuro, un `lock_timeout`, un DSM de otro
tamaño) el mismo silencio vuelve a esconderlo durante días. **`PARALLEL 0` además saca la
dependencia del `shm` del camino común: sin paralelismo no hay segmento DSM que reservar.**

El cambio (`scripts/pg_retention.sh` L534-572):

```bash
    VAC_RC=0
    VAC_OUT=$(docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X -qAt \
      -v ON_ERROR_STOP=1 \
      -c "SET lock_timeout='$BATCH_LOCK_TIMEOUT'" \
      -c "SET statement_timeout='600s'" \
      -c "VACUUM (ANALYZE, PARALLEL 0) $tbl" 2>&1) || VAC_RC=$?
    if [ "$VAC_RC" -ne 0 ]; then
      vacuum_failures=$((vacuum_failures + 1))
      summary+=("$tbl:vacuum=FAILED:rc=${VAC_RC}")
      log "retention.vacuum table=$tbl FAILED rc=$VAC_RC err=$(printf '%s' "$VAC_OUT" | tr '\n' ' ' | tr -s ' ' | cut -c1-300)"
    else
      log "retention.vacuum table=$tbl ok"
    fi
```

más `vacuum_failures=0` junto a los otros contadores (L391) y `vacuum_failures=$vacuum_failures`
en la línea `retention.summary` final (L575), para que el fallo sea **contable sin parsear prosa**.

**Sigue siendo non-fatal a propósito**: el purge de datos no depende de que el VACUUM pueda
devolver espacio. R9 en los dos sentidos: un fallo de mantenimiento se **registra**, no se
silencia; y no se convierte en un fallo del purge que no es.

## 5. Verificación (salida real, no descripción)

**a) Sintaxis del archivo real:**
```
$ bash -n scripts/pg_retention.sh
rc=0
```

**b) Prueba funcional del bloque REAL** — se extraen las 40 líneas del bloque desde el propio
archivo (`awk`-equivalente en el arnés), se les da un `docker` de mentira y se ejecutan los dos
caminos. No es una descripción del cambio: son los bytes del archivo los que corren.

```
bloque_lineas=534..572 (40)   ejecutables=15   con_>/dev/null=0
--- camino FALLA (stub devuelve rc=1 con el error medido del DSM) ---
2026-10-06T13:42:24Z retention.vacuum table=pool_reserves FAILED rc=1 err=ERROR: could not resize shared memory segment "/PostgreSQL.1234" to 536870912 bytes: No space left on device
RESULTADO vacuum_failures=1
RESULTADO summary=pool_reserves:vacuum=FAILED:rc=1
--- camino OK (stub devuelve rc=0) ---
2026-10-06T13:42:24Z retention.vacuum table=pool_reserves ok
RESULTADO vacuum_failures=0
```

El camino de fallo **nombra la causa** (`could not resize shared memory segment … No space left on
device`), la cuenta (`vacuum_failures=1`) y la deja en el summary. Antes, ese mismo fallo
producía sólo `failed (non-fatal)`.

**c) Alcance del diff:** `git diff --stat` → `scripts/pg_retention.sh | 35 ++++++----` (1 archivo,
+31/−4); `git diff --stat -- docker/` → **0 líneas**.

## 6. PR y disposición de #732 (criterio 5)

Se hizo **las dos cosas**, y por razones distintas:

1. **PR nuevo** desde la rama `sre/pg-retention-01` (base `main`), con **solo la mitad viva** —
   sin conflicto con `main` y sin la mitad obsoleta.
2. **Comentario en #732** declarando que su parte `shm_size: 2gb` **quedó obsoleta** por #832 y que
   su cambio de ventanas 3→60 días **no** viaja acá. Se comenta además que su parte `shm` está
   mergeada por otra vía, para que nadie resuelva el conflicto reintroduciendo `2gb`.

**NO se mergea nada.** Ni el PR nuevo ni #732.

## 7. Alcance económico y de producto (criterio 6)

- **No se firma, no se emite, no se toca capital.** El cambio es un script de mantenimiento de
  base de datos: `VACUUM` + logging. Cero transacciones, cero broadcast, cero deploy.
- **P/N 0/115 intacto.**
- Fronteras respetadas: no se toca `docker/`, `.github/`, `backend/`, `frontend/`, `edge/`,
  `docs/backend/`, `docs/release/`, `docs/governance/`, `implementation-state/`,
  `scripts/vps/`, `scripts/ci/`.

## 8. Límites de este artefacto (lo que NO prueba)

1. **No hay corrida real del purge.** Requiere Docker + PostgreSQL; esta estación no los tiene y
   su canal al VPS es de sólo lectura para otras vías. Lo verificado es sintaxis + comportamiento
   del bloque con un `docker` de mentira, más el diff.
2. **`PARALLEL 0` no lo re-medí yo**: se cita la medición de #732 en el VPS
   (`VACUUM (ANALYZE, PARALLEL 0) pool_reserves` → OK). Es evidencia heredada y se declara como tal.
3. **La ventana 3→60 días queda abierta**: descartarla implica que las tablas siguen purgando a 3
   días. Si el operador quiere 60, es otra unidad, con medición de disco propia.
4. **El umbral de `disk_guard` (15 GB) y el tamaño del bloat (1.7 GB de índices) son cifras de
   #732**, no remedidas acá.

## 9. Reproducción

```bash
gh pr view 732 --json state,mergeable,files,headRefOid
git show origin/main:docker/compose.prod.yml | grep -n shm_size
git show origin/main:scripts/pg_retention.sh | grep -n "2>/dev/null\|maintenance_work_mem\|VACUUM"
git diff origin/main -- scripts/pg_retention.sh
bash -n scripts/pg_retention.sh
```
