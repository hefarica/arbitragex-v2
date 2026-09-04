#!/usr/bin/env bash
# ARBX-RETENTION-01 — RDO table-swap one-time (runbook de docs/RETENTION_POLICY.md)
# Devuelve ~34GB al SO: la tabla nueva conserva SOLO el rango vivo (2d) y la
# vieja (con el espacio de los 53.8M DELETEs no devuelto) se dropea.
# Anti-incidente-13:36Z: INSERT por trozos de 3h con CHECKPOINT tras cada uno
# (pico WAL ~1.5GB, no 16GB de una vez). AEL final: solo delta + DDL metadata.
set -uo pipefail

PSQL() { docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -X \
  -At -v ON_ERROR_STOP=1 "$@"; }
log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "rdo.swap $*"; }

CUT=$(PSQL -c "SELECT (extract(epoch FROM now() - interval '2 days')*1000)::bigint")
NOW=$(PSQL -c "SELECT (extract(epoch FROM now())*1000)::bigint")
log "cutoff=$CUT now=$NOW"

v0=$(PSQL -c "SELECT count(*) FROM route_discovery_outcomes WHERE ts_ms >= $CUT")
log "rango_vivo_esperado=$v0"

PSQL -c "CREATE TABLE IF NOT EXISTS route_discovery_outcomes_new (LIKE route_discovery_outcomes INCLUDING ALL)" || { log FATAL_create; exit 1; }
PSQL -c "ALTER SEQUENCE route_discovery_outcomes_id_seq OWNED BY NONE" || { log FATAL_seq; exit 1; }
log "new_table_ready seq_detached"

lo=$CUT
while [ "$lo" -lt "$NOW" ]; do
  hi=$((lo + 10800000)); [ "$hi" -gt "$NOW" ] && hi=$NOW
  out=$(PSQL -c "INSERT INTO route_discovery_outcomes_new SELECT * FROM route_discovery_outcomes WHERE ts_ms >= $lo AND ts_ms < $hi" 2>&1)
  rc=$?
  if [ $rc -ne 0 ]; then log "FATAL_chunk lo=$lo rc=$rc out=$(printf '%s' "$out" | head -c 200)"; exit 1; fi
  n=$(printf '%s' "$out" | grep -oE 'INSERT [0-9]+ [0-9]+' | awk '{print $3}') || n=0
  PSQL -c "SET statement_timeout='600s'; CHECKPOINT" >/dev/null 2>&1 || true
  log "chunk lo=$lo inserted=$n"
  lo=$hi
done
log "chunks_done"

# Multi-statement en un solo -c corre en UNA transacción implícita atómica:
# si el LOCK no llega en 5s (writers saturando), aborta y la vieja queda intacta.
PSQL -c "SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='120s';
LOCK TABLE route_discovery_outcomes IN ACCESS EXCLUSIVE MODE;
INSERT INTO route_discovery_outcomes_new SELECT * FROM route_discovery_outcomes WHERE ts_ms >= $NOW;
DROP TABLE route_discovery_outcomes;
ALTER TABLE route_discovery_outcomes_new RENAME TO route_discovery_outcomes;" || { log "FATAL_swap (transaction rolled back — old table intact)"; exit 1; }
log "swapped"

# Los índices de la nueva tabla nacieron como *_new_* (INCLUDING ALL autonombra):
# renombrar a los canónicos para que IF NOT EXISTS futuros no dupliquen.
renames=$(PSQL -c "SELECT 'ALTER INDEX '||indexname||' RENAME TO '||replace(indexname,'_new_','_')||';' FROM pg_indexes WHERE tablename='route_discovery_outcomes' AND indexname LIKE '%\_new\_%'")
if [ -n "$renames" ]; then
  PSQL -c "$renames" >/dev/null 2>&1 || log "index_renames parcial (no fatal)"
  log "indexes_renamed"
fi

PSQL -c "ANALYZE route_discovery_outcomes" >/dev/null 2>&1 || true
v1=$(PSQL -c "SELECT count(*) FROM route_discovery_outcomes")
mn=$(PSQL -c "SELECT min(ts_ms) FROM route_discovery_outcomes")
log "final_count=$v1 (expected >= $v0) min_ts=$mn (must be >= $CUT)"
df_out=$(df -h / | tail -1 | awk '{print $4}')
log "disk_free=$df_out"
if [ "$v1" -lt "$v0" ]; then log "VERIFY_FAIL count menor al rango vivo"; exit 1; fi
if [ "$mn" -lt "$CUT" ]; then log "VERIFY_FAIL min_ts bajo cutoff"; exit 1; fi
log "COMPLETE"
