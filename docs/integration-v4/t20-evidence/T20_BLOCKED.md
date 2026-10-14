# t20 — α2 Build imagen arbx:R-<ts> — VEREDICTO: BLOQUEADA (precondición NO satisfecha; guard de disco ABORT)

Attempt 1 7253840c-aed6-4c34-8dae-1c967725b23b · 2026-10-13 (mediciones frescas esta sesión, ssh Git-bash
pineado, SOLO LECTURA del VPS: 0 bytes escritos). **No se ejecutó ningún build: los 3 gates del propio
contrato (t18 ABORT + t19 secuencia sancionada + precondición t20) lo prohíben con los números medidos.**

## Gate 1 — GUARD DE DISCO: ABORT (idéntico al ABORT de t18 paso 4, misma regla)

- df VPS medido AHORA: `/dev/sda1 150G 134G **11G** **93%** /` (ssh arbx df -h /).
- Builder transitorio del build real: **14-18GB** (t19 4a, medido empírico del deploy de hoy 06:16Z).
- Regla (R1/orden operador): libre ≥ 1.5×est ⇒ requiere **21-27GB** > 11GB libres ⇒ **ABORT**.
- El purge sancionado que libera ~10.4GB+WAL (DROP p20261012, rollup-first verificado por t19)
  **NO se ejecutó**: df sigue 93% idéntico a la medición de t19 — está ENCOLADO tras dump (guard de t19).

## Gate 2 — PRECONDICIÓN "dump+restore-test completados": NO satisfecha — y el dump está ZOMBI

- pg_dump VIVO dentro del contenedor pero ESTANCADO: `docker top arbitragex-v2-postgres-1 | grep pg_dump`
  → `PID 659745 ... StartTime 16:51 ... TIME 00:03:02` (**3 min de CPU en ~2h** = bloqueado en write de
  un pipe cuyo lector murió; el shim docker exec mantiene el stdout abierto).
- **El "relanzo durable 18:15Z" (t19) NO existe como proceso**: no hay 2º pg_dump en el contenedor.
- En la estación: **0 procesos ssh** (Get-Process ssh), **0 archivos** `arbitragex-*.dump*`/`.part`
  en workspace (raíz+subdir), %TEMP%, Downloads, C:\ (búsqueda múltiple) — NI el .TRUNCADO-3.87GB
  preservado por t19 aparece en las rutas buscadas. ⇒ el consumidor del dump murió (3ª mortalidad,
  confirma el escalado (3) de t19 desde este asiento).
- restore-test: PENDIENTE por diseño hasta que exista dump (t19 F1: vía sancionable diseñada, no corrida).

## Gate 3 — SECUENCIA SANCIONADA DEL OPERADOR: build va DESPUÉS de dump→verificar→purge

- Orden aprobado (ruling t6): transporte → D1 → **dump+restore** → drill → swap único+V7 → V8.
- t19 escalado 3: "secuencia swap se mantiene **dump→verificar→purge→build**→drill-real→swap(V7/V8)".
- Estamos ANTES de 'dump completado' ⇒ t20 (build) no puede empezar sin pisar la secuencia.

## Insumos del build — estado medido (para el re-intento)

- Release **R-20261013T1545Z-t6 NO está transportado al VPS**: `find /tmp /opt -maxdepth 3 -name
  '*R-20261013T1545*'` → VACÍO; solo existe `/tmp/t18-transport-test` (el PROBE del patrón 37e6be2b…).
  El transporte del release REAL es paso previo del build (contrato t20 paso 1: patrón probado,
  transporte pendiente).
- Imagen de referencia presente: `arbitragex-v2-searcher-rs:latest dfd71012e1dc 185MB` (base del drill
  retag de t18); NO existe ninguna `arbx:R-<ts>`.

## Qué desbloquea t20 (decisiones del capitán/qa, no mías — P1/P5)

1. Matar el pg_dump zombi (PID 659745) — mutación VPS, propiedad de qa/capitán (yo: solo lectura).
2. Re-sincronizar dump → verificar (sha256 + pg_restore --list) → restore-test selectivo (t19 F1)
   con el patrón durable CORREGIDO (la 3ª mortalidad exige diagnóstico de causa, no un 4º intento ciego).
3. Ejecutar el purge sancionado (DROP p20261012; procedimiento listo en T19_report §PASO 2) → df ≥21GB.
4. Re-despachar t20: transporte tar-over-ssh del release REAL (patrón 37e6be2b…) → docker build
   RULE 03 → inspect Id → A1 digest + re-hash en cadena → boot probado → ℛ.

## Evidencia por comando (literal, esta sesión)

| comando | salida clave |
|---|---|
| `ssh arbx "df -h / \| tail -1"` | `/dev/sda1 150G 134G 11G 93% /` |
| `ssh arbx "docker top arbitragex-v2-postgres-1 \| grep -c pg_dump"` | `1` |
| `ssh arbx "docker top ... \| grep pg_dump"` | `root 659745 3875420 46 16:51 ? 00:03:02 /usr/lib/postgresql/15/bin/pg_dump -U postgres -Z3 -Fc arbitragex` |
| `ssh arbx "find /tmp /opt -maxdepth 3 -name '*R-20261013T1545*'"` | (vacío) + `/tmp/t18-transport-test` existe |
| `ssh arbx "docker images ... \| grep -Ei 'searcher\|arbitragex\|rust'"` | `arbitragex-v2-searcher-rs:latest dfd71012e1dc 185MB` (+7 stack) |
| `Get-Process ssh` | (vacío — 0 consumidores) |
| `Get-ChildItem <workspace/%TEMP%/Downloads/C:\> -Filter 'arbitragex-*.dump*'` | (vacío — sin .part ni dump) |
