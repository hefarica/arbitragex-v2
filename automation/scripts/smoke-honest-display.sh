#!/usr/bin/env bash
# Sub-Proyecto A — Honest Display smoke test.
#
# Run on VPS via:
#   ssh arbx 'cd /opt/arbitragex-v2 && bash automation/scripts/smoke-honest-display.sh'
#
# OR locally with an SSH tunnel (token-enricher metrics bind to 127.0.0.1 on the host):
#   ssh -L 9004:127.0.0.1:9004 arbx
#   METRICS_URL=http://localhost:9004/metrics bash automation/scripts/smoke-honest-display.sh
#
# Environment overrides (all optional):
#   EDGE_URL      — base URL of the edge API  (default: http://${VPS_IP}:8787)
#   FRONT_URL     — base URL of the frontend  (default: http://${VPS_IP}:5173)
#   METRICS_URL   — Prometheus /metrics URL   (default: http://localhost:9004/metrics)
#                   Port 9004 is the token-enricher metrics port (Task 8).
#                   Port 9100 is Node Exporter and is NOT the enricher.

set -euo pipefail

EDGE_URL="${EDGE_URL:-http://${VPS_IP}:8787}"
FRONT_URL="${FRONT_URL:-http://${VPS_IP}:5173}"
METRICS_URL="${METRICS_URL:-http://localhost:9004/metrics}"

echo "=== Honest Display Smoke Test ==="
echo "  EDGE_URL:    $EDGE_URL"
echo "  FRONT_URL:   $FRONT_URL"
echo "  METRICS_URL: $METRICS_URL"
echo ""

# ── Check 1: Response shape includes token_in_info and chain_id_out ───────────
echo "1. Verify /api/opportunities/live response shape"
# Empty items array → vacuously true (no shape to check on a cold deploy).
# Non-empty items array → real key-presence check on the first element.
curl -fsS "$EDGE_URL/api/opportunities/live" \
  | jq -e '.items | if length > 0 then (.[0] | has("token_in_info") and has("chain_id_out")) else true end' >/dev/null
echo "   OK — token_in_info and chain_id_out fields present (or items empty)"

# ── Check 2: All SP-A items have chain_id_out == null (single-chain scope) ───
echo "2. Verify cross-chain fields are all NULL (SP-A is single-chain)"
curl -fsS "$EDGE_URL/api/opportunities/live" \
  | jq -e '.items | all(.chain_id_out == null)' >/dev/null
echo "   OK — chain_id_out is null for all items"

# ── Check 3: Enricher has resolved at least 1 token ──────────────────────────
# Metric: arbx_enricher_tokens_resolved_total{chain_id, resolved_via}
# (confirmed from backend/token-enricher/src/metrics.rs — Lazy opts! name)
echo "3. Verify token-enricher has resolved >= 1 token"
RAW=$(curl -fsS "$METRICS_URL" 2>&1) || {
  echo "   FAIL: could not reach $METRICS_URL"
  echo "         If running locally, open an SSH tunnel first:"
  echo "           ssh -L 9004:127.0.0.1:9004 arbx"
  exit 1
}
RESOLVED=$(echo "$RAW" \
  | grep '^arbx_enricher_tokens_resolved_total' \
  | awk '{s+=$NF} END {print s+0}')
if [ "$RESOLVED" -lt 1 ]; then
  echo "   FAIL: arbx_enricher_tokens_resolved_total sum = $RESOLVED (expected >= 1)"
  echo "         The enricher may still be warming up. Wait ~60s and retry."
  exit 1
fi
echo "   OK — $RESOLVED token(s) resolved"

# ── Check 4: Frontend page renders a table ────────────────────────────────────
# We grep for '<table' rather than 'Live MEV Feed' because the h1 is rendered
# by a Client Component — while Next.js does SSR the initial HTML, '<table' is
# more structurally stable and not subject to text changes.
echo "4. Verify frontend SSR delivers a <table"
curl -fsS "$FRONT_URL/opportunities" | grep -q "<table" || {
  echo "   FAIL: $FRONT_URL/opportunities did not return a <table in the HTML"
  exit 1
}
echo "   OK — frontend SSR contains <table"

echo ""
echo "=== All smoke checks passed. ==="
