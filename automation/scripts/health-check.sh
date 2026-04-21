#!/usr/bin/env bash
# ArbitrageX v2 — health-check.sh
# Hits /health on every service. Returns non-zero exit count.
set -u

declare -A ENDPOINTS=(
  [searcher-rs]="http://localhost:9001/health"
  [selector-api]="http://localhost:3002/health"
  [sim-ctl]="http://localhost:3003/health"
  [recon]="http://localhost:3004/health"
  [relays-client]="http://localhost:3005/health"
  [api-server]="http://localhost:8080/health"
  [edge]="http://localhost:8787/health"
  [prometheus]="http://localhost:9090/-/healthy"
  [grafana]="http://localhost:3000/api/health"
  [alertmanager]="http://localhost:9093/-/healthy"
  [loki]="http://localhost:3100/ready"
)

fails=0
printf '%-18s %-10s %s\n' "service" "status" "endpoint"
printf '%s\n' "----------------------------------------------------------------------"
for svc in "${!ENDPOINTS[@]}"; do
  url="${ENDPOINTS[$svc]}"
  code=$(curl -o /dev/null -s -w '%{http_code}' --max-time 3 "$url" || echo "000")
  if [[ "$code" =~ ^2 ]]; then
    printf '%-18s \033[32m%-10s\033[0m %s\n' "$svc" "OK($code)" "$url"
  else
    printf '%-18s \033[31m%-10s\033[0m %s\n' "$svc" "FAIL($code)" "$url"
    fails=$((fails+1))
  fi
done

echo
echo "Failed: $fails"
exit $fails
