# WO-S6 — Root-cause lock-timeout INSERT vs TRUNCATE/retención (2026-09-20)

> Pregunta: los `db_errors` intermitentes del searcher (pre-auditoría:
> 10-22/15m clusterizados ~:02-:05 de hora; PG `canceling statement due to
> lock timeout` + `terminating connection due to administrator command`)
> ¿siguen vivos? ¿quién bloquea a quién?

## Evidencia (VPS read-only, comandos + salidas)

1. **Tasa ACTUAL colapsada**: logs de PG (contenedor reiniciado 09:58Z hoy)
   contienen exactamente **1 lock timeout desde el reinicio** (ventana ~5.5h
   hasta el censo; `docker logs arbitragex-v2-postgres-1 | grep -c 'lock timeout'` = 1).
   Cero `administrator command` / `deadlock` en la misma ventana.
2. **El único evento es DEPLOY-TRANSITORIO**: `2026-09-20 14:51:53 UTC ERROR:
   canceling statement due to lock timeout` sobre
   `INSERT INTO pool_reserves (...) SELECT id,... FROM pools WHERE...`.
   Coincide exactamente con el deploy de #612 en el VPS:
   - `/opt/arbitragex-v2/database/run_migrations.sh` mtime `Sep 20 14:51`.
   - `/var/log/arbx-watchdog.log`: `deploy lock held` 14:52→14:59 (deploy en curso).
   - VPS HEAD = d6f8d893 (#612). El INSERT choca con locks de migración —
     fail-fast a los 5s (`lock_timeout=5s`, verificado; `statement_timeout=0`).
3. **El clustering histórico :02-:05 horario YA NO EXISTE** y sus causales
   fueron retirados:
   - Cron horario `/etc/cron.d/arbx-retention` **desactivado 2026-09-04**
     (comentado en el propio archivo; reemplazado por `scripts/pg_retention.sh`
     v3 con batching, diario 04:17 UTC — crontab root verificado).
   - FLIPPER `arbx-pg-retention.sh` (cron HORARIO que reseteaba credenciales)
     removido 2026-09-17 (incidente documentado).
   - Hoy la retención corrió 04:17→04:18:20 (`/var/log/arbx-pg-retention.log`):
     661.501 deletes en pool_reserves, sin errores.
4. **Límite R9 (honesto)**: los logs del searcher (5×10m) están rotados —
   arrancó 09:58Z pero retiene desde ~14:58Z; los de PG sólo desde 09:58Z.
   La ventana 04:17 de HOY no es auditable desde logs de contenedor. El
   veredicto "retención diaria sin contención" queda como pendiente de
   confirmar mañana tras la corrida 04:17 (heartbeat `db_errors` del searcher).

## Veredicto

- **Causa raíz de los :02-:05 horarios (histórico)**: cron de retención
  HORARIO legacy (single-statement DELETE sin batching) + FLIPPER — ambos
  retirados. No es drift de esquema; era contención administrativa.
- **Causa del evento residual actual**: locks de `run_migrations.sh` durante
  deploys contra PG vivo. Fail-fast 5s funciona como diseño (el INSERT se
  cancela, no se cuelga; el searcher reintenta en el siguiente batch).
- **Remediación**: NINGUNA requerida ahora (1 evento por deploy ≈ ruido
  aceptable; el deploy ya coordina con watchdog vía flock). Si creciera,
  la mejora con ID sería `SET LOCAL lock_timeout` + retry en
  run_migrations.sh, no en el hot-path.

## Seguimiento

- Mañana 2026-09-21 ~04:20Z: verificar `db_errors` en heartbeats del searcher
  tras la retención 04:17 (confirma o refuta contención residual de la
  retención diaria).
