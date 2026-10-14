# T19 — Gates-V6: F2 tendencia + liberación auditada + dump F4 + F1 + 4a · reporte (α9 VIGIL)

> attempt 609754a2 · 2026-10-13 17:4x–18:0x UTC · Git ssh.exe pineado · dump EN VUELO (job durable pwsh-11).
> Producción congelada; Δ periódico pasada 4: **deriva=0** (vmap_delta_p4.txt, ℛ).

## F2 — TENDENCIA DE DISCO: IDENTIFICADA CON EVIDENCIA

- df: 93% / 10.6-10.7GB libres (t15 08:2xZ: 88% ⇒ **+~7.5GB en ~9.5h**).
- Por zona (`f2_tendencia_disco.txt`): /var/lib/docker=101GB (de los cuales volúmenes 99.4GB: PG 82GB);
  /var/backups=**15GB** (full 27-sep + catálogos diarios — único backup PG, INTOCABLE); /var/log=518MB;
  journal=96MB; /tmp=226MB; imágenes=8.43GB estables; **Build Cache 0B**.
- PG 78→82GB (t15→ahora). Motor del crecimiento: **route_discovery_outcomes** — p20261012 (AYER)
  = **10.4GB** con índices; p20261013 (HOY, ~18h) = **7.8GB**; p20261014/15 pre-creadas vacías.
  Ritmo ≈ 10-12GB/día ⇒ **la ventana se cierra sola en ~24h** (DECLARADO; el guard abortará con razón).
- **CAUSA RAÍZ (retention_diagnostico.txt)**: el cron `pg_retention.sh` SÍ corrió hoy 04:17Z y purgó
  **9,753,188 filas** de 7 tablas (opportunities 4.8M, scored 4.8M, pool_reserves 114K, observations 888)
  — PERO **`route_discovery_outcomes` NO APARECE en el log**: el script (01-oct) no ejecuta la política
  ARBX-RETENTION-01 (orden del operador 2026-09-19: crudo de esa tabla = 1 día). **GAP script-vs-política
  en la tabla MÁS voluminosa.** Agravante: `retention.vacuum ... failed (non-fatal)` ×3 ⇒ lo purgado
  queda como bloat interno (espacio no devuelto al SO).

## PASO 2 — LIBERAR AUDITADO ≥12GB: higiene ejecutada; ≥12GB solo vía política (encolada tras dump)

- Ejecutado con evidencia antes/después (`p2_auditoria_imagenes.txt`): `docker builder prune` → **0B**
  (cache ya vacío); `journalctl --vacuum-time=7d` → **0B** (journal ya ≤7d, 96MB).
- Auditoría de las 24 imágenes (uso por contenedor, tabla pegada): 23 en uso; única sin contenedor =
  `rust:1.91` (2.32GB) — **NO se borra: es la base de build del Dockerfile** (borrarla = re-pull con red
  en el próximo build, rompe la propiedad sin-red). ⇒ **espacio borrable hoy sin romper nada: 0GB**.
- **La vía sancionada de ≥12GB es la PROPIA POLÍTICA del operador**: purge de
  `route_discovery_outcomes_p20261012` (10.4GB + WAL asociado). Rollup-first **VERIFICADO**
  (`rollup_cobertura.txt`): `route_discovery_outcome_rollup_5m` cubre hasta bucket_ms=1791864600000
  (2026-10-13T17:30Z) con 2,434,138 filas ⇒ el día-12 está materializado. Procedimiento ENCOLADO:
  `ALTER TABLE route_discovery_outcomes DETACH/DROP PARTITION p20261012` + re-verificar df —
  **GUARD: NO se ejecuta mientras el pg_dump esté en vuelo** (dropear una partición que el dump aún no
  leyó rompería el dump: route_discovery va DESPUÉS de opportunities/pool_reserves en el orden del dump,
  que va ~25% del total). Tras purge: ~21GB libres.

## PASO 3 — DUMP DURABLE F4: job 1 MUERTO (truncado preservado); RELANZADO con disciplina completa

- **Job 1 (pwsh-11, t18) MURIÓ** a los ~3.87GB (última escritura 16:34Z; 0 procesos ssh vivos;
  registry lo retiró como unknown). Archivo preservado como
  `arbitragex-20261013T1636Z.dump.TRUNCADO-3.87GB` (evidencia del fallo, no se borra).
- **RELANZADO (job durable, 18:15Z)** con disciplina reforzada: `ssh -o ServerAliveInterval=30
  -o ServerAliveCountMax=10` (keepalive), escritura a `.part` y **rename atómico solo si exit=0**;
  al finalizar: `sha256` → ℛ. Si vuelve a morir, diagnóstico de causa antes del tercer intento (§D: 2
  intentos y escalar).
- F4 al completar: (1) sha256 → ℛ; (2) **pg_restore --list** vía head-streaming (primeros ~2MB locales
  → `docker exec -i postgres pg_restore --list`: TOC del custom format vive al INICIO ⇒ 0 bytes VPS);
  (3) **punto de consistencia**: garantía MVCC del formato + watermark `max(detected_at)` inicio vs
  restaurado + `txid_current()` anotado al verificar.

## F1 — RESTORE-TEST: instrumento local declarado AUSENTE; vía sancionable diseñada

- La estación NO tiene PostgreSQL/pg_restore/docker (medido: docker=False; binarios pg ausentes).
  Restore-test LITERAL en la estación = INSTRUMENTO NO DISPONIBLE — declarado, no simulado.
- Vía ejecutable equivalente (0 bytes de disco VPS para el dump): restore SELECTIVO por stdin —
  `cat dump | ssh "docker exec -i postgres pg_restore --dbname arbx_restore_test --schema-only +
  --table=<tabla crítica ≤1GB>"` en DB scratch `arbx_restore_test` (fila A0d ya registrada), con
  conteo de filas restauradas == conteo origen leído por ssh. Un dump sin restore sigue siendo CLAIM;
  este restore selectivo es el ∃ℳ ejecutable hoy.

## PASO 4a — TAMAÑO FINAL DE IMAGEN: MEDIDO — (a) ALCANZA, (b) INNECESARIO

- Imagen final searcher-rs (multi-stage) = **185MB** (medida t18: `arbitragex-v2-searcher-rs:latest`
  dfd71012e1dc 185MB; `docker images` pegado en p2_auditoria). El "15-25GB" era el **stage de build**.
- Criterio del operador: final ≤8GB con margen 1.5× sobre libres tras (a) ⇒ **185MB ≪ 8GB ⇒ (a)
  ALCANZA, (b) save/load INNECESARIO**. Salvedad declarada: el builder TRANSITORIO durante el build
  necesita ≤14-18GB (evidencia empírica: el deploy de HOY 06:16Z corrió y cupo con los libres de
  entonces); tras el purge encolado (~21GB libres) el build queda holgado; sin purge, marginal.

## HIGIENE DE REPORTES (operador) — HECHA con re-hash en cadena

- **A1 SUPERSEDE §10** declarado en A1_MANIFEST (enmienda 0): gobierna **+0.60778 MEDIDO**
  (`+60.778%` / `+60,78%` — decimal, JAMÁS separador de miles); `'+0.604'` del maestro SUPERSEDIDO
  (mismo tratamiento que la prohibición del `'+0.636'`, que SIGUE vigente). Notación revisada en los
  reportes propios: sin instancias de formato '+60,778%'.
- A1 regenerado sha `7e1612f33c8194adb61dbd086c21df3f6a5687fe7ee8d119b7fd3f1885910290`; fila A1 del ℛ
  actualizada (re-hash en cadena); ℛ **55 filas** sha `7c26f5dd…`.

## V-MAP-Δ periódico

Pasada 4 (17:5xZ): deriva=0 en todos los invariantes medidos (G2 0-failed, puertos 22/80/8787,
/opt/arbx ausente, git 5f8ea3d6 + 3 untracked conocidos; compose: campos no parseados por fallo
cosmético del tail en el pipe — G2/G4/G6/git OK). Sin deriva ⇒ sin ABORT.

## Escalados al capitán/operador (con números)

1. **Gap retention**: `pg_retention.sh` no purga `route_discovery_outcomes` (política 1 día) — el disco
   crece ~10-12GB/día por esto. Reparación del script o DROP manual sancionado TRAS el dump (procedimiento listo arriba; rollup-first ya verificado).
2. **Vacuum non-fatal ×3** (opportunities/pool_reserves/scored): bloat interno acumulándose — el
   espacio de 9.75M filas purgadas hoy NO volvió al SO. `VACUUM` (no FULL) en ventana baja, decisión del operador.
3. **Dump lento** (3.87GB en ~1.7h, fase opportunities): ETA en horas; secuencia swap se mantiene
   dump→verificar→purge→build→drill-real→swap(V7/V8) con gates intactos.

## CORRECCIÓN AL DIAGNÓSTICO (α11 v2, tras fallos del capitán — fail-honest sobre MI hallazgo)

El "gap script-vs-política" del escalado 1 fue **LECTURA INCOMPLETA mía**: el script del VPS
(main 5f8ea3d6, verificado `git show 5f8ea3d6:scripts/pg_retention.sh` →
`"route_discovery_outcomes|ts_ms|ms|1|100000|rdo"` PRESENTE) **SÍ implementa la política** — por
PARTICIONES (bloque 2b, `scripts/pg_retention.sh:275-349`), no por DELETE batcheado:

- RDO particionado NO emite `retention.table table=route_discovery…` (lo que busqué en el log):
  emite `rdo.partition…` al dropear. Hoy 04:17Z no había partición ELEGIBLE: con ventana 1d,
  `cutoff=12-oct 04:17Z` y `day_end(p20261012)=13-oct 00:00Z > cutoff` ⇒ `continue` (:330).
- **p20261012 se dropeará AUTOMÁTICAMENTE el 14-oct 04:17Z** (elegible cuando now−1d ≥ 13-oct
  00:00Z), con guard rollup-first propio (:338-349) y devolución instantánea al SO (DROP PARTITION).
  El diseño mantiene ~2 particiones conviviendo por la ventana-borde (:320-323) — el estado actual
  (hoy+ayer ≈ 18GB en RDO) es COMPORTAMIENTO DISEÑADO, no bug.
- **CARRERA RECALCULADA**: el disco NO se llena en ~24h como estimé — mañana 04:17Z el cron
  devuelve ~10.4GB por sí solo. El DROP manual sancionado **ADELANTA ese ciclo** (misma política,
  mismo guard; mi verificación rollup ya hecha) — válido y seguro; si no se ejecuta, el cron lo hace.
- VACUUM: sigue vigente para el bloat de las 7 tablas DELETE-adas (9.75M filas hoy).
- **Parche propuesto: N/A** — el script ya implementa la política; no hay diff que viajar en el
  release. Observación menor (sugerencia, no parche): loguear `rdo.partition none_eligible` cuando
  no haya partición elegible, para que la ausencia en el log no se lea como gap (me ocurrió a mí).
- Dump job 2 (lanzado ~16:51Z, nombre T1815Z): 0.22GB a ~4 min (≈3.3GB/h) — lento pero vivo;
  si muere otra vez ⇒ división por tabla obligatoria (fallo capitán 3).
