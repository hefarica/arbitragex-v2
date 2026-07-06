#!/bin/bash
# ArbitrageX v2 — Run all migrations
set -e

PGUSER=postgres
PGDB=arbitragex

run_sql() {
  docker exec arbitragex-v2-postgres-1 psql -U "$PGUSER" -d "$PGDB" -c "$1"
}

run_file() {
  docker exec -i arbitragex-v2-postgres-1 psql -U "$PGUSER" -d "$PGDB" < "/opt/arbitragex-v2/database/migrations/$1"
}

echo "=== Creating roles ==="
run_sql "CREATE EXTENSION IF NOT EXISTS pgcrypto;"
run_sql "DO \$\$ BEGIN CREATE ROLE arbx_migrator WITH LOGIN CREATEDB; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$;"
run_sql "DO \$\$ BEGIN CREATE ROLE arbx_rw WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$;"
run_sql "DO \$\$ BEGIN CREATE ROLE arbx_ro WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$;"
run_sql "GRANT CONNECT ON DATABASE arbitragex TO arbx_migrator, arbx_rw, arbx_ro;"
run_sql "ALTER ROLE arbx_migrator WITH PASSWORD 'arbx_migrator_dev_only';"
run_sql "ALTER ROLE arbx_rw WITH PASSWORD 'arbx_rw_dev_only';"
run_sql "ALTER ROLE arbx_ro WITH PASSWORD 'arbx_ro_dev_only';"

echo "=== Running schema migrations ==="
for f in 002_schema_migrations.sql 003_opportunities.sql 004_simulations.sql 005_executions.sql \
         006_strategy_scores.sql 007_relay_scores.sql 008_token_safety_cache.sql 009_risk_events.sql \
         010_incident_log.sql 011_audit_log.sql 012_recon_reports.sql 013_relays.sql \
         014_scoring_weights.sql 015_execution_policy.sql 016_risk_policy.sql 017_price_oracles.sql \
         018_onboarding_progress.sql 019_audit_log_partitions.sql 020_audit_retention_func.sql \
         021_defi_registries.sql 022_defi_pools_routes.sql 023_defi_wallets.sql 024_profit_reconciliation.sql; do
  echo "  -> $f"
  run_file "$f"
done

echo "=== Granting permissions ==="
run_sql "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO arbx_rw;"
run_sql "GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO arbx_rw;"
run_sql "GRANT SELECT ON ALL TABLES IN SCHEMA public TO arbx_ro;"
run_sql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO arbx_rw;"
run_sql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO arbx_rw;"
run_sql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO arbx_ro;"

echo "=== DONE ==="
