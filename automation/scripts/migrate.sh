#!/usr/bin/env bash
# ArbitrageX v2 — migrate.sh
# Applies SQL migrations from database/migrations/NNN_*.sql in lexical order.
# Idempotent via schema_migrations table.
set -Eeuo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# shellcheck disable=SC1091
[[ -f .env ]] && set -a && . .env && set +a

DB_URL="${DATABASE_MIGRATOR_URL:-postgres://postgres:${POSTGRES_PASSWORD:-postgres}@localhost:5432/arbitragex}"
MIG_DIR="database/migrations"

command -v psql >/dev/null || { echo "psql not installed; trying docker..." ; USE_DOCKER=1; }

psql_cmd() {
  if [[ "${USE_DOCKER:-0}" == "1" ]]; then
    docker run --rm -v "$REPO_ROOT:$REPO_ROOT" -w "$REPO_ROOT" \
      --network host postgres:15 psql "$DB_URL" "$@"
  else
    psql "$DB_URL" "$@"
  fi
}

# Ensure control table exists
psql_cmd -v ON_ERROR_STOP=1 -c "CREATE TABLE IF NOT EXISTS schema_migrations (version TEXT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), checksum TEXT, applied_by TEXT NOT NULL DEFAULT CURRENT_USER);" >/dev/null

applied=()
while IFS= read -r -d '' f; do
  version="$(basename "$f" .sql)"
  exists=$(psql_cmd -tA -c "SELECT 1 FROM schema_migrations WHERE version='$version' LIMIT 1;" 2>/dev/null || echo "")
  if [[ "$exists" == "1" ]]; then
    echo "  SKIP  $version (already applied)"
    continue
  fi
  echo "  APPLY $version"
  # 001b_role_passwords needs psql -v variables for env-injected passwords.
  if [[ "$version" == "001b_role_passwords" ]]; then
    _migrator_pw="${ARBX_MIGRATOR_PASSWORD:-arbx_migrator_dev_only}"
    _rw_pw="${ARBX_RW_PASSWORD:-arbx_rw_dev_only}"
    _ro_pw="${ARBX_RO_PASSWORD:-arbx_ro_dev_only}"
    psql_cmd -v ON_ERROR_STOP=1 \
      -v "arbx_migrator_pw=$_migrator_pw" \
      -v "arbx_rw_pw=$_rw_pw" \
      -v "arbx_ro_pw=$_ro_pw" \
      -f "$f" >/dev/null
  else
    psql_cmd -v ON_ERROR_STOP=1 -f "$f" >/dev/null
  fi
  psql_cmd -v ON_ERROR_STOP=1 -c "INSERT INTO schema_migrations(version) VALUES ('$version');" >/dev/null
  applied+=("$version")
# LC_ALL=C forces deterministic byte-order sort across locales. Under an
# en_US.utf8 collation, sort is punctuation-insensitive and places 001b_* BEFORE
# 001_roles (compares 'b' vs 'r'), applying role-passwords before the roles exist.
# Byte order keeps NNN_* before NNNb_* since '_'(0x5F) < 'b'(0x62).
done < <(find "$MIG_DIR" -maxdepth 1 -type f -name '*.sql' -print0 | LC_ALL=C sort -z)

if [[ ${#applied[@]} -eq 0 ]]; then
  echo "migrations: no new migrations"
else
  echo "migrations: applied ${#applied[@]} — ${applied[*]}"
fi
