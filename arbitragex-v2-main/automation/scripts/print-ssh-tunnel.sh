#!/usr/bin/env bash
# print-ssh-tunnel.sh — emit the exact ssh -L command to access the stack
# from outside when running with compose.noports.override.yml.
#
# Container IPs on the docker bridge change whenever you recreate a service.
# Re-run this script after any `docker compose up` or `restart` to refresh
# the tunnel command.
#
# Usage:
#   # on the VPS:
#   bash automation/scripts/print-ssh-tunnel.sh
#
#   # or from your laptop, all-in-one:
#   ssh arbx 'bash /opt/arbitragex-v2/automation/scripts/print-ssh-tunnel.sh'

set -euo pipefail

SERVICES=(frontend:5173 edge:8787 api-server:8080 grafana:3000 prometheus:9090 alertmanager:9093)
HOST_ALIAS="${HOST_ALIAS:-arbx}"
PROJECT="arbitragex-v2"

declare -a fwds
declare -a lines

for sp in "${SERVICES[@]}"; do
  svc="${sp%%:*}"
  port="${sp##*:}"
  cname="${PROJECT}-${svc}-1"
  ip="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$cname" 2>/dev/null || true)"
  if [ -z "$ip" ]; then
    lines+=("  $svc: NOT RUNNING (container $cname absent)")
  else
    fwds+=("-L $port:$ip:$port")
    lines+=("  $svc → http://localhost:$port  (container $ip:$port)")
  fi
done

printf '=== ArbitrageX v2 — SSH tunnel to running containers ===\n\n'

if [ ${#fwds[@]} -eq 0 ]; then
  echo 'No running containers found. Start the stack first:'
  echo '  docker compose -f docker/compose.dev.yml -f docker/compose.noports.override.yml up -d'
  exit 1
fi

printf 'Services reachable once the tunnel is up:\n'
for l in "${lines[@]}"; do printf '%s\n' "$l"; done

printf '\nRun this from your laptop (keep it open):\n\n'
printf '  ssh -N \\\n'
for f in "${fwds[@]}"; do printf '    %s \\\n' "$f"; done
printf '    %s\n\n' "$HOST_ALIAS"

printf 'Then open http://localhost:5173 in your browser.\n'
printf '\nContainer IPs change on restart. Re-run this script after any\n'
printf '`docker compose up` or service recreate to get updated IPs.\n'
