#!/usr/bin/env bash
# ArbitrageX v2 — rollback.sh
# Stops the application layer; keeps data plane + observability running.
# For data rollback, restore from pg_dump backup (procedure documented in docs/SOP_ENTERPRISE.md).
set -Eeuo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO"

echo "Stopping application services (data + monitoring remain up)..."
docker compose --env-file .env -f docker/compose.prod.yml stop \
  searcher-rs selector-api sim-ctl relays-client recon api-server edge frontend

echo "Done. To fully tear down (including data): docker compose --env-file .env -f docker/compose.prod.yml down -v"
echo "To restore DB from backup: see docs/SOP_ENTERPRISE.md §Backup/Restore."
