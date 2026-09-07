# N7 — Data-Layer (PostgreSQL + Redis): salud, frescura y crecimiento

- **Agente:** verificador N7 "data-layer" (round-table integración omniscience)
- **Superficie:** PostgreSQL + Redis — salud, frescura, crecimiento de datos y retención
- **Estado:** COMPLETADO
- **Ventana de verificación:** 2026-09-06 23:20 → 23:35 UTC (todo read-only; 0 requests al dominio público, 2 curl internos al VPS)
- **Veredicto: DEGRADED** — la capa de datos está VIVA y FRESCA (lag 51 s), pero el disco empeoró de la semilla 76% → **80%**, y hay una **bomba estructural de crecimiento**: la retención sigue corriendo, pero su tope diario de purge (20 M filas) es MENOR que la tasa de inserción de `route_discovery_outcomes` (~30 M filas/día). Desde la corrida de mañana el backlog crece ~+10 M filas/día (~+5 GB/día) → **ENOSPC proyectado ≈ 2026-09-12/13** (precedente: crash-loop PG del 2026-09-04).

---

## 1. Respuesta directa a la pregunta del charter

> **¿La retención ARBX-RETENTION-01 sigue vigente o regresó el crecimiento?**

**Ambas cosas.** El cron de retención está VIVO y ejecutándose a diario (hoy 04:17 UTC purgó 19.04 M filas en 137 s, `complete`), y el error del 09-05 (FK `paper_trade_runs` NOT NULL) ya está remediado por el guard `RETENTION-FK-01` presente en el script desplegado. PERO el volumen de equilibrio del sistema creció: `route_discovery_outcomes` inserta hoy **~30 M filas/día (~350/s)** contra una ventana de 2 días, y el purge tiene un **tope de 20 M filas/corrida/día** (`MAX_ROWS_PER_TABLE`). Hoy apenas cupo (expiró 18.64 M < 20 M); **mañana expira ~29-30 M > 20 M → el purge se queda atrás para siempre** (+~10 M filas/día netas ≈ +5 GB/día). Es crecimiento estructural, no un backlog viejo.

Además el `VACUUM (ANALYZE)` post-purge **falla en TODAS las corridas desde el 09-04** (bug de invocación, ver §4.3) y el `CHECKPOINT` de WAL-pacing usa el mismo patrón roto (inferencia, §4.4).

---

## 2. Evidencia por capa

### 2.1 Capa LOCAL (repo `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)`) — MATCH (con drift menor)

- `scripts/pg_retention.sh` (385 líneas, v2 2026-09-04) existe y su contenido coincide con lo desplegado en el VPS (`/opt/arbitragex-v2/scripts/pg_retention.sh`, HEAD `d4d3ff63`). Contiene:
  - Ventanas: `route_discovery_outcomes|ts_ms|ms|2|100000|rdo`, `opportunities|detected_at|ts|60|20000`, `pool_reserves|...|30|25000|reserves`, `risk_events|...|90|25000`, `scored_opportunities|60`, `simulations|90`, `opportunity_observations|60`, `paper_trade_runs|90|paper`.
  - `MAX_ROWS_PER_TABLE="${ARBX_RETENTION_MAX_ROWS:-20000000}"` con stop `daily_row_cap_${MAX_ROWS_PER_TABLE}_resume_tomorrow` (script línea ~359).
  - Bug VACUUM (línea ~381): `psql ... -c "SET statement_timeout='600s'; VACUUM (ANALYZE) $tbl"` → múltiples statements en un solo `-c` = transacción implícita → `VACUUM cannot run inside a transaction block` → falla siempre (confirmado por log, §4.3).
  - Mismo patrón en CHECKPOINT de pacing: `-c "SET statement_timeout='600s'; CHECKPOINT"` con `|| true` (silenciado).
- `docs/RETENTION_POLICY.md` documenta la política completa (rollup-first, purge-after) y el resultado de la compactación 09-04: PG 92.5→32 GB, RDO 75→15 GB (30.9 M filas), disco 94%→48%. La política decía "RDO 29 M filas/día, ~15 GB/día" — hoy la tasa real es 30 M/día: la política sigue siendo exacta.
- Drift menor (git local): rama de trabajo `a6-cbprom-01` (HEAD `f46a0522` = `d4d3ff63` + 1 chore). El ref local `main` = `28d48cdd` está STALE respecto a `origin/main` = `d4d3ff63`. Este clone NO tiene remote `github` (solo `origin` = GitHub HTTPS) — la nota de memoria "origin = VPS bare repo" está desactualizada para este clone.
- Nota honesta: un `Glob` de `scripts/*retention*` agotó 20 s de timeout (repo grande con `contracts/lib`); el archivo se verificó por ruta directa.

### 2.2 Capa REMOTE MAIN — MATCH

```
$ git ls-remote origin main
d4d3ff634537a8b3626ae0fcdaabac70ef3a89f0    refs/heads/main
$ git remote -v
origin  https://github.com/hefarica/arbitragex-v2.git
```
`origin/main` (GitHub) = `d4d3ff63` = "feat(relays): A7-RELAYSIM-CALLSITE-01 (#543)". Coincide exactamente con el HEAD desplegado en el VPS (§2.3) → **deploy veraz** en la capa de datos (los scripts de retención leídos en el VPS son los de main).

### 2.3 Capa VPS (núcleo del charter) — DRIFT

#### a) Inventario y salud de contenedores
```
23/23 contenedores Up 22 minutes (healthy) al momento del muestreo (23:20 UTC)
arbitragex-v2-postgres-1  postgres:15   — mem 359.4MiB / 15.25GiB, cpu 5.97%
arbitragex-v2-redis-1     redis:7.2     — mem 726.9MiB / 15.25GiB, cpu 0.31%
(docker stats --no-stream; resto de la flota healthy, redeploy ~22:58 UTC)
VPS HEAD: git -C /opt/arbitragex-v2 rev-parse HEAD → d4d3ff634537a8b3626ae0fcdaabac70ef3a89f0
```

#### b) Frescura de `opportunities` (FAIL-HONEST: medido, no asumido)
```
db_now       = 2026-09-06 23:20:57.812569+00
max_detected = 2026-09-06 23:20:06.700791+00   → LAG 51 s  ✓ VIVO
oldest_row   = 2026-07-05 12:07:00+00          → FK-guard (ver §4.5), no drift abierto
c_1h  = 1,747
c_24h = 58,007
total = 12,804,229 filas
Inserción por día: 09-02: 2,317 · 09-03: 53,518 · 09-04: 43,241 · 09-05: 51,071 · 09-06: 56,731 (23.3h)
→ ~50 K/día ESTABLE (no es el driver del disco)
```

#### c) Tamaño y QUÉ crece (pregunta semilla "disco 76%")
```
df -h /  → /dev/sda1 150G  115G used  30G avail  80%   (semilla 76% · post-retención 09-04: 48%)
pg_database_size('arbitragex') = 63,667,871,079 bytes = 59 GB   (post-retención 09-04: 32 GB)
Volumen docker postgres_data = 66 GB (pg_wal = 273 MB — WAL reciclado HOY)

Top relaciones (pg_stat_user_tables, ORDER BY pg_total_relation_size DESC):
 route_discovery_outcomes          86.1M filas  42 GB (heap 33 GB + idx 9.3 GB)   ← EL DRIVER
 opportunities                     12.8M        7,903 MB (heap 4,834 + idx 3,068)
 pool_reserves                     15.9M        5,118 MB
 risk_events                        6.4M        3,156 MB
 scored_opportunities                520K         744 MB
 paper_trade_runs                    670K         323 MB
 simulations                        1.02M        314 MB
 route_discovery_outcome_rollup_5m  705K         169 MB   ← rollup persistente VIVO

Distribución del disco / (docker system df):
 Images 35.28 GB (reclaimable 117 MB) · Volumes 73.55 GB (reclaimable 998 MB)
 Build cache 21.32 GB (reclaimable 17.11 GB) · Containers 458 MB
 Otras: loki 1,000 MB · minio 957 MB · qb-cargo-registry 888 MB · /opt/arbitragex-v2 3.5 GB
```

#### d) Tasa real de crecimiento de RDO (histograma medido, query completó exit 0)
```
rdo_oldest = 2026-09-04 04:18:54 UTC (límite de la purga de HOY 04:17, ventana 2d)
rdo_newest = 2026-09-06 23:24:06 UTC
filas por día: 09-04: 24,922,712 · 09-05: 30,896,451 · 09-06: 30,197,194 (23.4h)
→ ~30 M filas/día ≈ 350 filas/s sostenido
Composición 24h: is_opportunity=f → 30,893,124 (99.82%) · t → 56,998 (0.18%)
→ el 99.8% son telemetría de rechazo del discovery/re-eval (R8 honesto, pero 42 GB por 2 días)
```

#### e) La matemática del purge (por qué el crecimiento REGRESA mañana)
```
MAX_ROWS_PER_TABLE = 20,000,000 filas/día/corrida (default del script, sin env override visible)
Tasa de inserción   = ~30,000,000 filas/día
→ expiry diario (≈30M) > cap (20M) ⇒ backlog neto +10 M filas/día ≈ +4-5 GB/día (a 488 B/fila total)
→ 30 GB libres ÷ ~5 GB/día ⇒ ENOSPC ≈ 2026-09-12/13 (precedente 09-04 13:36Z: PG crash-loop)
HOY 04:18Z aún cupo: RDO deleted=18,640,893 complete (186 batches × 100K)
```
El volumen ACTUAL (59 GB DB) es ~el estado de equilibrio diseñado (2-3 días × 30 M/día = 60-90 M filas ≈ 30-45 GB solo RDO); la bomba es que **a partir de mañana el cap impide sostener la ventana de 2 días**.

#### f) Cron de retención VIVO (evidencia de las últimas 2 corridas)
```
crontab -l (root):
 17 4 * * * bash /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
 23 5 * * 0 docker builder prune -f >> /var/log/arbx-builder-prune.log 2>&1   (semanal domingo; legacy hourly DESACTIVADO ✓)

/var/log/arbx-pg-retention.log (tail real):
 2026-09-06T04:17:03Z rdo.backfill chunk inserted rows=1078 elapsed=1s
 2026-09-06T04:17:04Z rdo.backfill done missing_remaining=0 elapsed=2s budget=900s
 2026-09-06T04:18:47Z retention.table table=route_discovery_outcomes deleted=18640893 elapsed=96s complete
 2026-09-06T04:19:25Z retention.table table=opportunities deleted=217461 elapsed=38s complete
 2026-09-06T04:19:27Z retention.table table=pool_reserves deleted=183300 elapsed=2s complete
 2026-09-06T04:19:28Z retention.summary dry_run=0 deleted_total=19041654 elapsed_total=137s ... :complete ×7
 2026-09-05T04:20:16Z retention.table table=opportunities deleted=3900000 elapsed=186s
        stop='error: ... null value in column "opportunity_id" of relation "paper_trade_runs"
        violates not-null constraint ...'   ← YA REMEDIADO (guard RETENTION-FK-01 en el script; corrida 09-06 complete)
 retention.vacuum table=<todas> failed (non-fatal)   ← TODAS las corridas, TODAS las tablas (bug §4.3)

retention_settings (SELECT): archive_auto = {"enabled": false}  → sin consumo de disco por archives .zst
```

#### g) Redis (salud + drift de consumidores)
```
XLEN arbx:opps:detected = 10,001–10,004 (estable ~10K — política: "XLEN estable ~10K, no crece" ✓)
DBSIZE = 3,801 keys · mem contenedor 726.9 MiB / 15.25 GiB · cpu 0.31%
XINFO STREAM:
 entries-added=518,089 · length=10,004 · first-entry=1788720315367 (17:25:16Z) · last=1788737116977 (23:25:16Z)
 → el stream retiene ~6 h de oportunidades (trim por maxlen ~10K)
XINFO GROUPS:
 enricher          1 consumer   lag=10,428  pending=0
 paper-archiver-g0 61 consumers lag=10,492  pending=0
 selector-g0       57 consumers lag=10,492  pending=1
```
**Drift medido:** el lag de los 3 grupos (~10.4-10.5 K) es MAYOR que el largo del stream (10,004) → al menos ~488 entradas fueron **recortadas por el trim antes de ser consumidas** (lag − length). `paper-archiver-g0` es el writer del ledger paper → posible hueco de ~cientos de oportunidades en el archivo/ledger. No afirmo magnitud exacta del hueco (los contadores entries-read/lag pueden incluir saltos del puntero tras el restart de la flota hace 22 min); es un hallazgo a verificar con conteos de ledger vs stream.

#### h) Trazabilidad R7 (datos → API) — edge interno
```
curl http://127.0.0.1:8787/api/opportunities/live (dentro del VPS):
 {"count":50,"window":"latest","viable_only":false,"max_age_seconds":300,
  "items":[{"id":"bf8bd946-...","chain_id":1,"strategy_kind":"dex_arb","dex_a":"UniswapV3",
  "pair_symbol":"7adbfd…/a0b869…","token_in_info":{"symbol":"ALBRH","decimals":18,...}}...]}
→ PG→edge SIRVIENDO datos reales y frescos ✓ (fila más nueva del stream: detected_at 23:25:16Z)
```

### 2.4 Capa LIVE DOMAIN — N-A
El charter de data-layer no exige golpear el dominio público (la capa de datos no se expone directamente; se consume vía edge→frontend). Se verificó el extremo de datos con el curl interno del §2.3-h. **Presupuesto público usado: 0 de 5 requests.** La validación del dominio público le corresponde a los verificadores de frontend/edge de este round-table.

---

## 3. Drifts medidos (resumen)

1. **Disco / 80%** (115G/150G, 30G libres) — empeoró vs semilla 76% y vs 48% post-retención del 09-04.
2. **PG 59 GB** vs 32 GB post-retención: driver = `route_discovery_outcomes` 42 GB / 86.1 M filas (ventana 2 d a ~30 M/día).
3. **Cap de purge 20 M/día < inserción 30 M/día** → backlog neto +10 M filas/día (~+5 GB/día) desde la corrida del 09-07 → **ENOSPC ~09-12/13**.
4. **`retention.vacuum` falla TODAS las corridas** desde el 09-04 (todas las tablas): `psql -c "SET ...; VACUUM (ANALYZE) tbl"` = multi-statement = transacción implícita → `VACUUM cannot run inside a transaction block`.
5. **WAL-pacing CHECKPOINT presumiblemente no-op** — mismo patrón `SET ...; CHECKPOINT` en un solo `-c`, silenciado con `|| true` (INFERENCIA del mismo anti-patrón; no medido directamente). Riesgo de repetir el WAL-burst del 09-04 si el purge corre backlog grande.
6. **Bloat latente**: `pool_reserves` n_dead_tup = 1,679,560 con autovacuum stale (últ. 09-05 04:22); `opportunities` 199,290 dead. (RDO n_dead_tup=0 ✓ — autovacuum 09-06 16:07 lo alcanzó.)
7. **Redis: lag de grupos (~10.4-10.5 K) > length (10,004)** → ≥ ~488 entradas recortadas antes de consumo por `paper-archiver-g0`/`selector-g0` (posible hueco en ledger paper); `selector-g0` pending=1.
8. **Git local menor**: ref `main` local stale (28d48cdd) vs `origin/main` (d4d3ff63); clone sin remote `github` separado (memoria desactualizada).
9. **Build cache 17.11 GB reclaimable** — el `builder prune -f` semanal (domingo 05:23, corrió hoy) no libera cache "en uso" reciente; requiere `-a`/`--keep-storage`.
10. **CERRADO (no drift abierto):** error 09-05 de purge de `opportunities` (FK ON DELETE SET NULL sobre columnas NOT NULL de `paper_trade_runs`) — remediado por guard `RETENTION-FK-01`, corrida 09-06 `complete` ✓. Y `opportunities` con oldest 2026-07-05 (>60 d) es efecto documentado del guard (padres referenciados por hijos de ventana 90 d), no fallo.

---

## 4. Riesgos

- **R1 (P0): ENOSPC en ~6 días** → crash-loop de PostgreSQL sobre `/` (precedente 09-04 13:36Z, resolución requirió VACUUM FULL standalone). El stack monolítico (23 contenedores, incl. Loki/MinIO/Thanos en el mismo FS) cae entero con él.
- **R2 (P1): WAL-burst sin pacing funcional** — si se intenta "alcanzar" el backlog (cap 40M+ o corridas manuales), el CHECKPOINT cada 20 batches probablemente no se ejecuta (mismo bug de transacción implícita) → el pico de WAL del 09-4 puede repetirse.
- **R3 (P1): Hueco silencioso en el ledger paper** — entradas del stream recortadas antes de que `paper-archiver-g0` las consuma (lag > length) rompen la promesa de trazabilidad del ledger del operador (política de retención: "el historial del operador nunca se pierde").
- **R4 (P2): Bloat progresivo** — con VACUUM manual roto, la higiene depende 100 % de autovacuum; hoy RDO está en 0 dead pero `pool_reserves` ya acumula 1.68 M dead tuples.
- **R5 (P2): Margen de proyección** — 30 GB libres incluyen el comportamiento del build cache (regenera tras cada deploy `--no-cache`) y Loki (retención 15 d auto). El día-a-ENOSPC puede acortarse con deploys nocturnos.

---

## 5. Propuestas (what / why / priority / effort / gate)

1. **[P0] Subir el throughput de purge ≥ tasa de inserción**: duplicar la frecuencia del cron (`17 4,16 * * *` → 2×20 M = 40 M/día) O setear `ARBX_RETENTION_MAX_ROWS=40000000` en el entorno del cron. Why: cap 20 M < 30 M/día ⇒ +5 GB/día hasta ENOSPC ~09-12; hoy ya se purgaron 18.6 M en 96 s sin presión de disco (pg_wal 273 MB), la dosificación ya está probada. Effort: 1 línea de crontab o 1 env (minutos). Gate: OPERADOR (mutación de cron/VPS — como auditor read-only no la ejecuto).
2. **[P0] Recuperar 17 GB YA**: `docker builder prune -af --keep-storage 5GB` (y añadirlo al cron semanal en vez de `-f` solo). Why: 17.11 GB reclaimable medidos = +55 % de margen inmediato (30→47 GB libres) ≈ +3 días de aire para implementar (1). Effort: 1 comando + 1 línea de cron. Gate: OPERADOR.
3. **[P1] Fix del bug VACUUM/CHECKPOINT en `scripts/pg_retention.sh`**: separar en dos invocaciones `psql -c` (o `PGOPTIONS="-c statement_timeout=600s"` + `-c "VACUUM (ANALYZE) tbl"`; ídem CHECKPOINT). Why: hoy la higiene post-purge es un no-op enmascarado como "non-fatal" y el pacing WAL no opera; con backlog creciente ambos dejan de ser cosméticos. Effort: ~10 líneas + test. Gate: PR con ID de anomalía (P-∅ §37) + CI + deploy estándar (la capa de datos NO se toca a mano en VPS).
4. **[P1] Alerta de disco en Prometheus (75 % warn / 85 % critical)** con ruta a Alertmanager (el stack ya existe: prometheus+alertmanager activos). Why: el 09-04 el disco se llenó sin aviso; el 80 % actual también habría pasado inadvertido sin esta auditoría. Fail-honest: NO verifiqué si ya existe una regla `node_filesystem` (fuera de mi charter medido); el PR debe empezar por comprobarlo. Effort: 1 regla + test promtool (hay CI de unit-tests de reglas desde #545). Gate: PR + CI.
5. **[P2] Verificar y cerrar el hueco del archiver**: contar en PG (ledger) vs `entries-added` del stream para `paper-archiver-g0`; si el hueco es real, subir el trim (`XTRIM MAXLEN`) o garantizar consumo antes de trim; alertar cuando `group lag > stream length`. Why: ledger del operador con entradas perdidas silenciosamente = violación de la promesa de trazabilidad. Effort: investigación (2-4 h) + posible PR searcher/api. Gate: PR con evidencia de conteo primero (RULE 00: medir antes de cambiar).
6. **[P2] Decidir la ventana RDO 2 d → 1 d** (documentado en RETENTION_POLICY.md) si la tasa de ~30 M/día es el nuevo normal: bajaría el steady state de RDO de ~42 GB a ~21 GB. Why: 99.82 % de las filas son rechazos de telemetría; la trazabilidad fina >1 d ya vive en el rollup 5 m persistente. Effort: 1 celda de la tabla TABLES + doc. Gate: OPERADOR (decisión de trazabilidad, no técnica) + PR.

---

## 6. Apendice: comandos exactos usados (todos read-only)

```
ssh arbx date -u; df -h /; df -i /
ssh arbx docker ps --format '...'; docker stats --no-stream
ssh arbx docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex [-At|-x]:
  SELECT NOW(), MAX(detected_at), MIN(detected_at) FROM opportunities;
  SELECT COUNT(*) FILTER (1h/24h), COUNT(*) FROM opportunities;
  SELECT pg_size_pretty(pg_database_size('arbitragex'));
  SELECT relname,n_live_tup,pg_total_relation_size... FROM pg_stat_user_tables ORDER BY 2 DESC LIMIT 12;
  SELECT relname,n_dead_tup,last_vacuum,last_autovacuum FROM pg_stat_user_tables WHERE relname IN (...);
  SELECT column_name FROM information_schema.columns WHERE table_name='route_discovery_outcomes';
  SELECT date_trunc('day',inserted_at),COUNT(*) FROM route_discovery_outcomes GROUP BY 1;  (+is_opportunity 24h)
  SELECT key,value FROM retention_settings;
ssh arbx docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected / DBSIZE / XINFO STREAM / XINFO GROUPS
ssh arbx crontab -l; tail /var/log/arbx-pg-retention.log; sed -n scripts/pg_retention.sh
ssh arbx du -sh /var/lib/docker/volumes/* ; docker exec postgres du -sh .../pg_wal .../data ; docker system df
ssh arbx curl -s http://127.0.0.1:8787/api/opportunities/live
git ls-remote origin main; git remote -v; git log/rev-parse (locales, read-only)
```
Cero escrituras al sistema. Único archivo escrito: este reporte.
