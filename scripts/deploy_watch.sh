#!/bin/bash
# DEPLOY-WATCH-02 (2026-10-13): comprobación horaria + RECUPERACIÓN ACTIVA.
# Upgraded from DEPLOY-WATCH-01 (read-only) after the 2026-10-13 incident:
# api-server was left in "Created" state by force-recreate and NOTHING
# restarted it for 3+ hours (the read-only watchdog just logged the problem).
#
# This version DETECTS containers in non-running states and STARTS them.
# If a container is in a crash loop (starts and immediately exits 3+ times),
# it logs the error and alerts via the log (no further restart attempts —
# that would be a mask, not a fix).
cd /opt/arbitragex-v2 || exit 1

# ── Observability (same as v1) ─────────────────────────────────────────────
SHA=$(git rev-parse --short HEAD)
HEALTHY=$(docker ps --format "{{.Status}}" | grep -c healthy)
TOTAL=$(docker ps -q | wc -l)
DISK=$(df -BG --output=avail / | tail -1 | tr -dc "0-9")
printf "%s sha=%s healthy=%s/%s disk=%sG" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$SHA" "$HEALTHY" "$TOTAL" "$DISK"

# ── ACTIVE RECOVERY (NEW in v2) ────────────────────────────────────────────
# Detect containers that should be running but aren't (Created, Exited, Paused)
# and start them. Docker's restart policy only applies to containers that
# HAVE STARTED and then crashed — it ignores containers in "Created" state.
RECOVERED=0
for cid in $(docker ps -aq --filter label=com.docker.compose.project=arbitragex-v2 2>/dev/null); do
  cname=$(docker inspect "$cid" --format '{{.Name}}' 2>/dev/null | tr -d '/')
  cstate=$(docker inspect "$cid" --format '{{.State.Status}}' 2>/dev/null)
  
  if [ "$cstate" = "created" ] || [ "$cstate" = "paused" ]; then
    # Never started or paused — start it
    echo " RECOVERING:$cname(was:$cstate)"
    docker start "$cid" >/dev/null 2>&1
    RECOVERED=$((RECOVERED+1))
  elif [ "$cstate" = "exited" ]; then
    # Check if it's supposed to be running (has a restart policy)
    rpolicy=$(docker inspect "$cid" --format '{{.HostConfig.RestartPolicy.Name}}' 2>/dev/null)
    if [ "$rpolicy" != "no" ] && [ "$rpolicy" != "" ]; then
      # Check restart count to avoid crash-loop masks
      rcount=$(docker inspect "$cid" --format '{{.RestartCount}}' 2>/dev/null)
      if [ "$rcount" -lt 3 ]; then
        echo " RESTARTING:$cname(exited,restart_count=$rcount)"
        docker start "$cid" >/dev/null 2>&1
        RECOVERED=$((RECOVERED+1))
      else
        echo " CRASHLOOP:$cname(restart_count=$rcount,NOT restarting — manual intervention needed)"
      fi
    fi
  fi
done

if [ "$RECOVERED" -gt 0 ]; then
  echo " recovered=$RECOVERED"
else
  echo ""
fi
