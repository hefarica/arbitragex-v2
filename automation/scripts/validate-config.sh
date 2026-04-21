#!/usr/bin/env bash
set -Eeuo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO"
python3 automation/tools/validate-config.py configs/app.toml configs/schemas/app.schema.json
