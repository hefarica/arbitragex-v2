#!/usr/bin/env bash
# ARBX-RETENTION-01 — RDO emergency compact (one-time, 2026-09-04)
# Contexto: 2º ENOSPC — el table-swap a 2d necesitaba ~45GB libres (imposible
# en disco de 150GB con la tabla vieja viva). Estrategia nativa:
#   1) purge batched con cutoff 1d (rollup <1d ya materializado: subconjunto
#      del missing_remaining=0 verificado 13:25Z) — CHECKPOINT cada 20 batches
#   2) VACUUM FULL: reescritura in-place 65GB→~16GB → ~49GB devueltos al SO
#      (AEL ~10-15min: writers de RDO en pausa documentada durante la ventana)
set -uo pipefail
PSQL() { docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -X \
  -At -v ON_ERROR_STOP=1 "$@"; }
log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "rdo.compact $*"; }

log "start free=$(df -h / | tail -1 | awk '{print $4}')"
rows0=$(PSQL -c "SELECT count(*) FROM route_discovery_outcomes")
log "rows_before=$rows0"

CUT=$(PSQL -c "SELECT (extract(epoch FROM now() - interval '1 day')*1000)::bigint")
# Guard: buckets <cutoff TODOS en rollup (sin esto NO se borra nada)
missing=$(PSQL -c "
  WITH oldest AS (SELECT min(ts_ms)/300000::bigint*300000 AS b FROM route_discovery_outcomes)
  SELECT count(*) FROM generate_series((SELECT b FROM oldest), $CUT - $CUT % 300000, 300000) g(bucket)
  WHERE NOT EXISTS (SELECT 1 FROM route_discovery_outcome_rollup_5m rr
                    WHERE rr.dim='__totals__' AND rr.bucket_ms=g.bucket)")
log "rollup_missing_below_cutoff=$missing"
if [ "$missing" != "0" ]; then log "ABORT: rollup incomplete — nothing deleted"; exit 1; fi

deleted=0; n=1; i=0
while [ "$n" -gt 0 ]; do
  out=$(PSQL -c "SET lock_timeout='5s'; SET statement_timeout='300s';
    WITH victim AS (SELECT ctid FROM route_discovery_outcomes WHERE ts_ms < $CUT LIMIT 100000)
    DELETE FROM route_discovery_outcomes t USING victim v WHERE t.ctid = v.ctid" 2>&1)
  if [ $? -ne 0 ]; then log "PURGE_ERROR: $(printf '%s' "$out" | head -c 200)"; exit 1; fi
  n=$(printf '%s' "$out" | grep -oE 'DELETE [0-9]+' | grep -oE '[0-9]+$') || n=0
  deleted=$((deleted + n)); i=$((i + 1))
  if [ $((i % 20)) -eq 0 ]; then
    PSQL -c "SET statement_timeout='600s'; CHECKPOINT" >/dev/null 2>&1 || true
    log "purged=$deleted batches=$i free=$(df -h / | tail -1 | awk '{print $4}')"
  fi
done
log "purge_done deleted=$deleted free=$(df -h / | tail -1 | awk '{print $4}')"

log "vacuum_full_start (AEL window ~10-15min: RDO writers paused)"
vac=$(PSQL -c "SET lock_timeout='30s'; SET statement_timeout='1800s'; VACUUM (FULL, ANALYZE) route_discovery_outcomes" 2>&1)
if [ $? -ne 0 ]; then log "VACUUM_ERROR: $(printf '%s' "$vac" | head -c 200)"; exit 1; fi
log "vacuum_done free=$(df -h / | tail -1 | awk '{print $4}')"

rows1=$(PSQL -c "SELECT count(*) FROM route_discovery_outcomes")
sz=$(PSQL -c "SELECT pg_size_pretty(pg_total_relation_size('route_discovery_outcomes'))")
log "COMPLETE rows_after=$rows1 size=$sz free=$(df -h / | tail -1 | awk '{print $4}')"
