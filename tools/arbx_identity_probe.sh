#!/usr/bin/env bash
# arbx_identity_probe.sh — RUNTIME-IDENTITY-01
# Owner: Integration (1379d506-d159-59a8-9f7b-7e017d2f464d)
#
# READ-ONLY. Measures the runtime identity of every service across three
# independent axes and refuses to promote a declaration into a verification.
#
# Runs ON the deploy host (no nested quoting, no remote script injection):
#     ssh arbx 'bash -s' < tools/arbx_identity_probe.sh
#
# Optional env:
#   ARBX_DEPLOY_START   ISO-8601 of the deploy run start. When set, a container
#                       whose StartedAt precedes it is reported `stale`
#                       (i.e. "this service was NOT recreated by that deploy").
#   ARBX_IDENTITY_SERVICES  override the service list.
#
# Exit codes:  0 = every service measured and none stale
#              1 = at least one service `undeclared`/`mismatch` (integrity gap)
#              2 = at least one service `stale` (deploy did not recreate it)
#              3 = probe could not run (no docker)
#
# DESIGN INVARIANT — the verdict vocabulary is closed on purpose:
#   `verified` is only reachable when a build-baked value exists AND equals the
#   declared value. Because no Dockerfile currently bakes
#   org.opencontainers.image.revision, `verified` is UNREACHABLE today and this
#   probe reports `declared_only` instead of guessing. A declared env var is an
#   INTENTION; it is never evidence that the binary corresponds to it.

set -uo pipefail

command -v docker >/dev/null 2>&1 || { echo '{"ok":false,"reason":"docker_absent"}'; exit 3; }

DEPLOY_START="${ARBX_DEPLOY_START:-}"
SERVICES="${ARBX_IDENTITY_SERVICES:-api-server frontend edge searcher-rs selector-api sim-ctl recon token-enricher relays-client math-engine}"

worst=0
first=1
printf '['
for c in $SERVICES; do
  n="arbitragex-v2-${c}-1"

  if [ "$(docker inspect -f '{{.State.Running}}' "$n" 2>/dev/null)" != "true" ]; then
    obj=$(printf '{"service":"%s","verdict":"unmeasurable","reason":"container_absent_or_not_running"}' "$c")
    [ $worst -lt 1 ] && worst=1
  else
    envv=$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$n" 2>/dev/null)
    declared=$(printf '%s\n' "$envv" | grep '^ARBX_DEPLOY_SHA=' | cut -d= -f2-)
    declared_at=$(printf '%s\n' "$envv" | grep '^ARBX_DEPLOYED_AT=' | cut -d= -f2-)
    img=$(docker inspect -f '{{.Image}}' "$n" 2>/dev/null)
    started=$(docker inspect -f '{{.State.StartedAt}}' "$n" 2>/dev/null)
    created=$(docker inspect -f '{{.Created}}' "$n" 2>/dev/null)
    built=$(docker image inspect -f '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$img" 2>/dev/null)
    [ -z "$built" ] && built="ABSENT"

    if [ "$built" != "ABSENT" ]; then
      if [ -n "$declared" ] && [ "$built" = "$declared" ]; then v="verified"; else v="mismatch"; fi
    elif [ -n "$declared" ]; then
      v="declared_only"
    else
      v="undeclared"
    fi

    recreated="unknown"
    if [ -n "$DEPLOY_START" ] && [ -n "$started" ]; then
      if [[ "$started" > "$DEPLOY_START" ]]; then recreated="true"; else recreated="false"; v="stale"; fi
    fi

    [ "$v" = "undeclared" ] && [ $worst -lt 1 ] && worst=1
    [ "$v" = "mismatch" ]   && [ $worst -lt 1 ] && worst=1
    [ "$v" = "stale" ]      && worst=2

    obj=$(printf '{"service":"%s","verdict":"%s","declared_sha":"%s","declared_at":"%s","image_digest":"%s","image_created":"%s","container_started_at":"%s","built_from_sha":"%s","recreated_by_deploy":%s}' \
      "$c" "$v" "$declared" "$declared_at" "$img" "$created" "$started" "$built" \
      "$([ "$recreated" = "unknown" ] && echo 'null' || echo "$recreated")")
  fi

  [ $first -eq 1 ] || printf ','
  first=0
  printf '%s' "$obj"
done
printf ']'
echo
exit $worst
