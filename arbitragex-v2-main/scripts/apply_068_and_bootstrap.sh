#!/usr/bin/env bash
# ============================================================================
#  apply_068_and_bootstrap.sh
#  --------------------------------------------------------------------------
#  Aplica la migración 068 (operator_parametrization) e inicializa operadores
#  de forma idempotente. Verifica L8/L9 + feature_manifest.
#
#  Uso:
#    DATABASE_URL=postgres://... ./apply_068_and_bootstrap.sh
#
#  Requisitos:
#    - psql en PATH.
#    - Variables: DATABASE_URL.
#    - Opcional: SOVEREIGN_PUBKEY (si se omite, sólo se aplica la migración).
# ============================================================================
set -euo pipefail

: "${DATABASE_URL:?Set DATABASE_URL}"
MIGRATION_FILE="${MIGRATION_FILE:-database/migrations/068_operator_parametrization_sovereignty.sql}"

if [[ ! -f "$MIGRATION_FILE" ]]; then
  echo "BLOCKED: Migration file not found at $MIGRATION_FILE" >&2
  exit 1
fi

echo "==> Aplicando migración 068 (idempotente)…"
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$MIGRATION_FILE"

echo "==> Verificando tabla operator_parametrization…"
COUNT=$(psql "$DATABASE_URL" -tA -c "SELECT count(*) FROM operator_parametrization;")
echo "    operator_parametrization rows: $COUNT"

echo "==> Verificando feature_manifest C9…"
psql "$DATABASE_URL" -c "SELECT feature_key, ui_path, enabled
                          FROM feature_manifest
                          WHERE feature_key LIKE 'operator_%'
                          ORDER BY feature_key;"

if [[ -n "${SOVEREIGN_PUBKEY:-}" ]]; then
  : "${SOVEREIGN_DISPLAY_NAME:=Operator Sovereign}"
  : "${SOVEREIGN_OPERATOR_ID:=op_sovereign_primary}"
  echo "==> Registrando operador sovereign primario (idempotente)…"
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<SQL
INSERT INTO operator_parametrization
  (operator_id, display_name, signing_pubkey, role,
   cap_usd_ceiling, allowed_chains, allowed_registries,
   ui_preferences, feature_overrides, config_hash)
VALUES
  ('${SOVEREIGN_OPERATOR_ID}',
   '${SOVEREIGN_DISPLAY_NAME}',
   '${SOVEREIGN_PUBKEY}',
   'sovereign',
   0.00,
   '[]'::jsonb,
   '["chain","rpc","dex","token","pool","wallet","strategy","contract","risk_gate","capital_gate","relay","agent"]'::jsonb,
   '{"locale":"es","density":"comfortable"}'::jsonb,
   '{}'::jsonb,
   'sha256:bootstrap_sovereign_primary')
ON CONFLICT (operator_id) DO UPDATE
  SET display_name=EXCLUDED.display_name,
      signing_pubkey=EXCLUDED.signing_pubkey,
      role=EXCLUDED.role,
      updated_at=NOW();
SQL
  echo "    Sovereign registrado: ${SOVEREIGN_OPERATOR_ID}"
fi

echo "==> Verificando audit_event tiene columnas L9…"
psql "$DATABASE_URL" -c "SELECT column_name FROM information_schema.columns
                          WHERE table_name='audit_event'
                            AND column_name IN ('operator_id','operator_pubkey','operator_role')
                          ORDER BY column_name;"

echo "==> Done. 9-Layer Coherence operativo (L8 + L9 cableados)."
