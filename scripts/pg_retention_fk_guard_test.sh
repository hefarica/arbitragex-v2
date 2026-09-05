#!/usr/bin/env bash
# RETENTION-FK-01 — test de repro del guard de FKs en la fase opportunities.
#
# Bug (evidencia 2026-09-05, cron 04:17): opportunities es padre de 3 FKs
# ON DELETE SET NULL sobre columnas NOT NULL → borrar un padre con hijos
# vivos aborta SIEMPRE y hace rollback del lote (deleted=3900000:error,
# MIN(detected_at) congelado 2026-07-04). Este test demuestra:
#   Fase A (sin guard, SQL pre-fix)  → el DELETE aborta con el error exacto
#                                      del incidente y NO borra nada.
#   Fase B (con guard, SQL post-fix) → padres viejos SIN hijos se borran,
#                                      los referenciados sobreviven, hijos
#                                      intactos, idempotente en 2ª corrida.
#
# El SQL víctima es el MISMO que genera scripts/pg_retention.sh (fase purge,
# tbl=opportunities). Si el script cambia el predicado, este test debe
# actualizarse en tándem.
#
# Uso (VPS, docker disponible; NO toca el postgres de prod — container
# efímero aislado, sin puerto publicado):
#   bash scripts/pg_retention_fk_guard_test.sh
set -euo pipefail

TEST_C=arbx-fkguard-test
PG_IMAGE=postgres:15

log() { printf '[fkguard-test] %s\n' "$*"; }

cleanup() {
  docker rm -f "$TEST_C" >/dev/null 2>&1 || true
}
trap cleanup EXIT

psql_t() {  # psql_t <sql> → stdout; falla con rc!=0 en error SQL.
  # SIN -q: quiet suprime los command tags ("DELETE n") que este test usa.
  docker exec -i "$TEST_C" psql -U postgres -d fkguard -X -v ON_ERROR_STOP=1 -At -c "$1"
}

cleanup
docker run --rm -d --name "$TEST_C" -e POSTGRES_HOST_AUTH_METHOD=trust "$PG_IMAGE" >/dev/null
for i in $(seq 1 30); do
  docker exec "$TEST_C" pg_isready -U postgres >/dev/null 2>&1 && break
  sleep 1
done
docker exec "$TEST_C" pg_isready -U postgres >/dev/null

docker exec -i "$TEST_C" psql -U postgres -d postgres -X -v ON_ERROR_STOP=1 -qAt \
  -c "CREATE DATABASE fkguard" >/dev/null

# --- Fixture: topología FK idéntica a prod (solo las columnas del predicado) ---
psql_t "
CREATE TABLE opportunities (id uuid PRIMARY KEY, detected_at timestamptz NOT NULL);
CREATE TABLE paper_trade_runs (id uuid PRIMARY KEY,
  opportunity_id uuid NOT NULL REFERENCES opportunities(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE risk_events (id uuid PRIMARY KEY,
  opportunity_id uuid NOT NULL REFERENCES opportunities(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE opportunity_observations (id uuid PRIMARY KEY,
  opportunity_id uuid NOT NULL REFERENCES opportunities(id) ON DELETE SET NULL,
  observed_at timestamptz NOT NULL DEFAULT now());
CREATE INDEX idx_paper_trade_runs_opportunity ON paper_trade_runs (opportunity_id);
CREATE INDEX idx_risk_events_opportunity_id ON risk_events (opportunity_id) WHERE (opportunity_id IS NOT NULL);
CREATE INDEX idx_opportunity_observations_opportunity_id ON opportunity_observations (opportunity_id) WHERE (opportunity_id IS NOT NULL);
-- 3 viejos SIN hijos (70d) + 3 viejos CON hijos (uno por FK) + 2 frescos.
INSERT INTO opportunities VALUES
  ('00000000-0000-0000-0000-0000000000a1', now() - interval '70 days'),
  ('00000000-0000-0000-0000-0000000000a2', now() - interval '70 days'),
  ('00000000-0000-0000-0000-0000000000a3', now() - interval '70 days'),
  ('00000000-0000-0000-0000-0000000000b1', now() - interval '70 days'),
  ('00000000-0000-0000-0000-0000000000b2', now() - interval '70 days'),
  ('00000000-0000-0000-0000-0000000000b3', now() - interval '70 days'),
  ('00000000-0000-0000-0000-0000000000c1', now()),
  ('00000000-0000-0000-0000-0000000000c2', now());
INSERT INTO paper_trade_runs (id, opportunity_id) VALUES ('10000000-0000-0000-0000-0000000000b1', '00000000-0000-0000-0000-0000000000b1');
INSERT INTO risk_events (id, opportunity_id) VALUES ('20000000-0000-0000-0000-0000000000b2', '00000000-0000-0000-0000-0000000000b2');
INSERT INTO opportunity_observations (id, opportunity_id) VALUES ('30000000-0000-0000-0000-0000000000b3', '00000000-0000-0000-0000-0000000000b3');
" >/dev/null

CUTOFF="now() - interval '60 days'"
GUARD="AND NOT EXISTS (SELECT 1 FROM paper_trade_runs p WHERE p.opportunity_id = o.id)
       AND NOT EXISTS (SELECT 1 FROM risk_events r WHERE r.opportunity_id = o.id)
       AND NOT EXISTS (SELECT 1 FROM opportunity_observations x WHERE x.opportunity_id = o.id)"

fails=0
assert_eq() {  # assert_eq <desc> <esperado> <obtenido>
  if [ "$2" = "$3" ]; then
    log "PASS: $1 ($3)"
  else
    log "FAIL: $1 — esperado=$2 obtenido=$3"
    fails=$((fails + 1))
  fi
}

# --- Fase A: SQL pre-fix (SIN guard) debe abortar con el error del incidente ---
OLD_OUT=$(psql_t "
WITH victim AS (SELECT ctid FROM opportunities o WHERE o.detected_at < $CUTOFF LIMIT 100)
DELETE FROM opportunities t USING victim v WHERE t.ctid = v.ctid" 2>&1) && old_rc=0 || old_rc=$?
if printf '%s' "$OLD_OUT" | grep -q 'null value in column "opportunity_id"'; then
  log "PASS: fase A reproduce el abort exacto del cron (SET NULL sobre NOT NULL)"
else
  log "FAIL: fase A no reprodujo el error — rc=$old_rc out=$(printf '%s' "$OLD_OUT" | head -c 200)"
  fails=$((fails + 1))
fi
assert_eq "rollback: opportunities intactas tras abort" 8 "$(psql_t 'SELECT count(*) FROM opportunities')"

# --- Fase B: SQL post-fix (CON guard) — mismo batch del script ---
NEW_OUT=$(psql_t "
WITH victim AS (SELECT ctid FROM opportunities o WHERE o.detected_at < $CUTOFF $GUARD LIMIT 100)
DELETE FROM opportunities t USING victim v WHERE t.ctid = v.ctid")
assert_eq "viejos sin hijos borrados (DELETE n)" "DELETE 3" "$NEW_OUT"
assert_eq "viejos referenciados sobreviven" 3 "$(psql_t "SELECT count(*) FROM opportunities WHERE id::text LIKE '%b%'")"
assert_eq "frescos intactos" 2 "$(psql_t "SELECT count(*) FROM opportunities WHERE id::text LIKE '%c%'")"
assert_eq "ledger paper intacto" 1 "$(psql_t 'SELECT count(*) FROM paper_trade_runs')"
assert_eq "risk_events intacto" 1 "$(psql_t 'SELECT count(*) FROM risk_events')"
assert_eq "observations intacto" 1 "$(psql_t 'SELECT count(*) FROM opportunity_observations')"

# --- Idempotencia: 2ª corrida ya no borra nada (los b1* no tienen hijos) ---
NEW_OUT2=$(psql_t "
WITH victim AS (SELECT ctid FROM opportunities o WHERE o.detected_at < $CUTOFF $GUARD LIMIT 100)
DELETE FROM opportunities t USING victim v WHERE t.ctid = v.ctid")
assert_eq "idempotente: 2ª corrida borra 0" "DELETE 0" "$NEW_OUT2"

if [ "$fails" -eq 0 ]; then
  log "RESULTADO: 9/9 PASS — RETENTION-FK-01 verificado"
  exit 0
else
  log "RESULTADO: ${fails} FAIL"
  exit 1
fi
