# RETENTION-02 — Disco VPS: capacidad por deploy + backlog RDO

> Board creado 2026-09-10 por el orquestador (IA OMEGA), diagnóstico aprobado por
> el operador ("go"). Evidencia: sesión 2026-09-10 — `df`, `docker system df`,
> `docker buildx du`, `psql` (sizes/pg_stat), `/var/log/arbx-pg-retention.log`
> y `/var/log/arbx-builder-prune.log` sobre el VPS (arbx, /opt/arbitragex-v2).

## 0. Diagnóstico (cerrado, con números)

- Disco `/` = 150GB: 132GB usados / **13GB libres (92%)** tras quick wins.
- **NO es por tiempo ni por filas de referencias**: cada deploy completo deposita
  ~20GB de buildkit cache en pocos layers gigantes (evidencia: una corrida del
  prune reclamó 20.82GB, un solo record de 1.104GB; hoy quedan 25 records /
  119.9MB). El cron semanal (RETENTION-01) no contiene 2-4 deploys/día.
- **Ocupante estructural**: volume `arbitragex-v2_postgres_data` = 113.3GB
  (db 99GB). `route_discovery_outcomes` ≈ 82.5GB (68 heap + 14.5 índices),
  132M filas vivas.
- Retención diaria en **equilibrio**: borra 20M filas/día (cap
  `daily_row_cap_20000000_resume_tomorrow` en el log, todas las corridas) ==
  inserta ~20M/día → el backlog de **~6.6 días** persiste indefinidamente vs la
  ventana de política de 2 días. `n_dead_tup=0` (autovacuum al día) → el archivo
  está en equilibrio, no crece, pero tampoco devuelve disco.
- **DELETE+VACUUM no devuelve disco**: liberar GBs reales exige reparticionado
  con DROP PARTITION (O(1), sin ráfaga WAL, sin VACUUM FULL) o `pg_repack`.
- `retention.vacuum ... failed (non-fatal)` todos los días (log 09-08 y 09-09);
  `opportunities` acumula 12M dead tuples sin vacuum (last_autovacuum NULL).
- WAL saludable: 929MB (pacing CHECKPOINT_EVERY funcionando).

## 1. Umbral de liberación (respuesta al operador)

| Libre | % usado | Acción |
|---|---|---|
| ≥ 30GB | ≤ 80% | Zona segura: 1 deploy completo cabe (≈8GB temp + 20GB cache + 1GB WAL) |
| < 30GB | > 80% | **Liberar** (guard WARN: builder prune `--min-free-space 20GB` + image prune) |
| < 15GB | > 90% | **Crítico** (guard CRIT: además volume prune de anónimos) |

## 2. Work-orders

| WO | kind | Descripción | Gate |
|---|---|---|---|
| RT-01 | apply | Particionar `route_discovery_outcomes` por día (particionado declarativo nativo o pg_partman) y purgar por **DROP PARTITION**: devuelve disco en O(1), sin ráfaga WAL ni VACUUM FULL. Migración online de 132M filas sin downtime del stack. | df muestra ≥20GB liberados netos + ventana 2 días real + suite backend verde |
| RT-02 | apply | Drenar el backlog: subir `ARBX_RETENTION_MAX_ROWS` a 40M (env del cron, 3-4 días) hasta vivir ≤2 días de datos crudos | backlog <2.1 días por 3 corridas consecutivas (query del §0) |
| RT-03 | fix | Diagnosticar `retention.vacuum failed (non-fatal)` (¿lock_timeout aplicado a VACUUM?) — `opportunities` tiene 12M dead tuples sin vacuum | log sin "failed" 3 corridas + n_dead_tup(opportunities) < 1M |
| RT-04 | apply | Caps de retención de loki (volume 1.99GB) y journald (474MB) | loki ≤ 1GB sostenido |
| RT-05 | docs | **Este PR**: `scripts/disk_guard.sh` al repo + purge post-deploy en `deploy.sh` + este board | CI verde + merge |

## 3. Ya aplicado en VPS (2026-09-10, vivo antes que este PR)

- `scripts/disk_guard.sh` instalado (archivo no-tracked + `chmod +x`), cron
  `13 * * * *` → `/var/log/arbx-disk-guard.log`. Verificado con corrida manual
  (disparó CRIT a 12,834MB y ejecutó las tres purgas). El cron semanal de
  builder prune se retiene como backstop.
- Quick wins ejecutados: 5 volúmenes huérfanos removidos (~1GB:
  qb-cargo-registry 814MB + 4 anónimos), `docker image prune -af` (29.84MB —
  el resto eran layers compartidos). 12GB → 13GB libres.

## 4. Restricción operativa vigente hasta RT-01

Con ~13GB libres **no cabe un rebuild completo del stack** (~20GB de cache):
deploys SOLO por-servicio hasta que RT-01 libere espacio en PG. El guard previene
la recurrencia del llenado pero no crea headroom.

## 5. Reglas para los WOs

- ORDEN SAGRADO: local (tests) → repo (PR + CI verde + merge) → VPS (deploy
  veraz KNOWN_GOOD_REVISION) → verificación con números post-cambio.
- Postgres/Redis por MCP: read-only (§33). Los applies de RT-01/02 son scripts
  reviewed por PR, ejecutados en ventanas acordadas con el operador.
- FAIL-HONEST (R8): cada WO reporta `df -h /` antes/después y conteos absolutos,
  nunca promedios de eras mezcladas.
