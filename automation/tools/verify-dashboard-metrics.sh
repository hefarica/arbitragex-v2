#!/usr/bin/env bash
# verify-dashboard-metrics.sh — guardrail for monitoring/grafana/dashboards/*.json.
#
# Every Grafana panel `expr` must reference only arbx_* metrics that actually
# exist in the running services. Uses node for JSON parsing to avoid a hard
# jq dependency (node is already a repo-wide dev requirement).

set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

DASH_DIR="monitoring/grafana/dashboards"
SOURCES=("backend/shared-rs/src/metrics.rs" "shared-ts/src/metrics/index.ts")

if ! command -v node >/dev/null 2>&1; then
  echo "verify-dashboard-metrics: node is required" >&2
  exit 2
fi

# Build set of known arbx_* metric bases from Rust + TS sources.
KNOWN="$(mktemp)"
trap 'rm -f "$KNOWN"' EXIT

for src in "${SOURCES[@]}"; do
  [ -f "$src" ] || continue
  grep -oE '"arbx_[a-z0-9_]+"' "$src" | tr -d '"' >> "$KNOWN"
done
sort -u -o "$KNOWN" "$KNOWN"

echo "verify-dashboard-metrics: known arbx_* metrics: $(wc -l < "$KNOWN")"

VIOLATIONS=0
for dash in "$DASH_DIR"/*.json; do
  [ -f "$dash" ] || continue
  dash_name="$(basename "$dash")"

  # Extract (panel_id \t expr) lines via node.
  while IFS=$'\t' read -r panel_id expr_text; do
    [ -z "$expr_text" ] && continue
    [ "$expr_text" = "null" ] && continue

    while read -r metric; do
      [ -z "$metric" ] && continue
      base="$metric"
      case "$base" in
        *_bucket) base="${base%_bucket}" ;;
        *_sum)    base="${base%_sum}"    ;;
        *_count)  base="${base%_count}"  ;;
      esac
      case "$base" in
        arbx_nodejs_*) continue ;;
      esac
      if ! grep -qxF "$base" "$KNOWN"; then
        echo "VIOLATION: $dash_name panel=$panel_id metric='$metric' (base='$base') not registered in source metrics" >&2
        VIOLATIONS=$((VIOLATIONS+1))
      fi
    done < <(printf '%s\n' "$expr_text" | grep -oE 'arbx_[a-z0-9_]+' | sort -u)
  done < <(
    node -e '
      const fs = require("fs");
      const d = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
      for (const p of d.panels || []) {
        for (const t of p.targets || []) {
          if (typeof t.expr === "string" && t.expr.length) {
            process.stdout.write(p.id + "\t" + t.expr.replace(/\r?\n/g, " ") + "\n");
          }
        }
      }
    ' "$dash"
  )
done

if [ "$VIOLATIONS" -gt 0 ]; then
  echo "" >&2
  echo "verify-dashboard-metrics: $VIOLATIONS violation(s). Dashboards reference metrics that aren't registered." >&2
  echo "Fix: register the metric in shared-rs/src/metrics.rs or shared-ts/src/metrics/index.ts, or remove the panel." >&2
  exit 1
fi

echo "verify-dashboard-metrics: clean ($(ls "$DASH_DIR"/*.json 2>/dev/null | wc -l) dashboards)"
exit 0
