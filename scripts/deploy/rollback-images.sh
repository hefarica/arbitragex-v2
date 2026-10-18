#!/usr/bin/env bash
# G-IMG-ROLLBACK-01 — one-step IMAGE rollback for the production stack.
#
# WHY: before deploy-hardening-01 (2026-10-18) the auto-deploy workflow built a
# new image for every service with `docker compose build --no-cache`, which
# re-pointed `arbitragex-v2-<svc>:latest` and left the previous release dangling
# — and the disk guard prunes dangling images. A read-only census of the VPS
# (audit-vps-wiring, run 39143097918) measured 0 tags matching pre-/backup/
# rollback: there was NO image-level way back. The only path was a git reset +
# a full ~30-minute rebuild, exactly when the stack may be broken.
#
# WHAT THE DEPLOY NOW DOES ([5b/9] of auto-deploy-vps.yml): before it builds, it
# retags each running image BY ID as `arbx-rollback/<svc>:<gen>` and creates a
# keeper container so `docker image prune -af` cannot delete it. This script is
# the other half: it puts one of those generations back and force-recreates the
# services — no rebuild, no git, no SSH from here.
#
# RUN IT ON THE VPS, from the deploy path:
#   bash scripts/deploy/rollback-images.sh --list
#   bash scripts/deploy/rollback-images.sh --gen 20261018T153156Z --dry-run
#   bash scripts/deploy/rollback-images.sh --gen 20261018T153156Z
#
# EXIT CODES: 0 = rolled back (or listed), 1 = refused / failed.
# This script never touches git, never builds, and never removes a container
# that is not part of the rollback set.
set -euo pipefail

COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.prod.yml}"
ENV_FILE="${ENV_FILE:-.env}"
# The services the deploy captures and can therefore restore.
ROLLBACK_SERVICES="${ROLLBACK_SERVICES:-api-server edge frontend searcher-rs sim-ctl relays-client recon selector-api anvil math-engine token-enricher socket-proxy}"

GEN=""
LIST=0
DRY_RUN=0

usage() {
  sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-1}"
}

while [ $# -gt 0 ]; do
  case "$1" in
    --gen) GEN="${2:-}"; shift 2 ;;
    --list) LIST=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage 0 ;;
    *) echo "unknown argument: $1" >&2; usage 1 ;;
  esac
done

say() { printf '%s\n' "$*"; }

command -v docker >/dev/null 2>&1 || { echo "::error::docker not found — run this ON the VPS" >&2; exit 1; }

generations() {
  docker ps -a --filter 'name=arbx-rollback-' --format '{{.Names}}' 2>/dev/null \
    | sed -nE 's/^arbx-rollback-.*-([0-9]{8}T[0-9]{6}Z)$/\1/p' | sort -u
}

if [ "$LIST" -eq 1 ]; then
  gens="$(generations)"
  if [ -z "$gens" ]; then
    say "no rollback generations captured yet (nothing to roll back to)"
    exit 0
  fi
  say "rollback generations available (oldest first):"
  printf '%s\n' "$gens" | while read -r g; do
    n="$(docker image ls --format '{{.Repository}}' 2>/dev/null | grep -c '^arbx-rollback/' || true)"
    say "  ${g}   (${n} repo tag(s) present)"
  done
  exit 0
fi

if [ -z "$GEN" ]; then
  say "::error::--gen is required (use --list to see what exists)"
  exit 1
fi

# Fail-closed: the generation must have been actually captured.
present=0
for svc in $ROLLBACK_SERVICES; do
  if docker image inspect "arbx-rollback/${svc}:${GEN}" >/dev/null 2>&1; then
    present=$((present+1))
  fi
done
if [ "$present" -eq 0 ]; then
  say "::error::generation ${GEN} has no captured images — refusing (nothing to restore)"
  exit 1
fi
say "generation ${GEN}: ${present}/${#ROLLBACK_SERVICES} service images present"

targets=""
for svc in $ROLLBACK_SERVICES; do
  if docker image inspect "arbx-rollback/${svc}:${GEN}" >/dev/null 2>&1; then
    targets="${targets} ${svc}"
  fi
done
say "services to restore:${targets}"

if [ "$DRY_RUN" -eq 1 ]; then
  say "[dry-run] would retag each arbx-rollback/<svc>:${GEN} -> arbitragex-v2-<svc>:latest"
  say "[dry-run] would run: docker compose --env-file ${ENV_FILE} -f ${COMPOSE_FILE} up -d --no-build --force-recreate${targets}"
  exit 0
fi

# 1) Put the captured image back under the tag compose expects.
for svc in $targets; do
  docker tag "arbx-rollback/${svc}:${GEN}" "arbitragex-v2-${svc}:latest"
done
say "retagged ${present} image(s) back to :latest"

# 2) Recreate ONLY the restored services, never building (--no-build keeps the
#    rolled-back image; a build would immediately overwrite it).
# shellcheck disable=SC2086
docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d --no-build --force-recreate $targets

# 3) Verify the control plane is answering.
for i in $(seq 1 30); do
  code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 http://localhost/api/health || true)"
  if [ "$code" = "200" ]; then
    say "rollback verified: /api/health -> 200 (after ${i}s)"
    say "NOTE: this restores the APPLICATION IMAGES only. Database migrations are"
    say "      forward-only; if the bad release shipped a schema change, restore the DB"
    say "      from backup separately."
    exit 0
  fi
  sleep 1
done

echo "::error::rollback retagged the images but /api/health did not return 200 in 30s" >&2
exit 1
