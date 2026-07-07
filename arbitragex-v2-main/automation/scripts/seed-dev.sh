#!/usr/bin/env bash
# ArbitrageX v2 — seed-dev.sh
# Loads dev-only seed data. Refuses to run outside `development` env.
set -Eeuo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO"

[[ -f .env ]] && set -a && . .env && set +a
if [[ "${ENV:-development}" != "development" ]]; then
  echo "refusing to seed: ENV=$ENV (only development is allowed)"
  exit 1
fi

DB_URL="${DATABASE_MIGRATOR_URL:-postgres://postgres:${POSTGRES_PASSWORD:-postgres}@localhost:5432/arbitragex}"
psql "$DB_URL" -v ON_ERROR_STOP=1 -f database/seed/dev_chains.sql
echo "seed: dev data loaded"
