#!/usr/bin/env bash
# deploy_identity.sh — resolve the identity of the binary a container is SERVING.
#
# WHY THIS FILE EXISTS (2026-10-19, card #4): every recreate that is not a
# deploy (`arbx_remote.sh apply` and its rollback) ran
# `docker compose up -d --force-recreate --no-deps searcher-rs api-server`
# without ARBX_DEPLOY_SHA/_ID/_DEPLOYED_AT in the environment. compose.prod.yml
# interpolates them with `:-unknown`, so `/api/status` fell from the deployed
# sha to the literal `unknown` although the running binary had not changed.
# The deploy path stamps the identity (scripts/deploy.sh:116-126,
# auto-deploy-vps.yml); the recreate path did not. This is the missing reader.
#
# SOURCES, in order, and why:
#   1. `org.opencontainers.image.revision` on the image the container runs.
#      That label is baked with ARBX_BUILD_SHA=$ARBX_DEPLOY_SHA at build time
#      (backend/api-server/Dockerfile), so it IS the binary's build sha.
#   2. The container's own ARBX_DEPLOY_SHA env (the last deploy's stamp). Used
#      only when the image carries no valid label (an image older than the
#      label). It preserves identity on stacks built before the label existed.
#   ID/AT are taken from the container env ONLY when that env's sha equals the
#   chosen sha: the identity is a triple, and mixing a sha from one deploy with
#   the id/at of another would fabricate a deploy that never happened.
#
# Never prints `unknown` and never invents a value: no valid source => three
# empty lines, so compose keeps its own honest `unknown`.
#
# Usage: deploy_identity.sh <container-id-or-name>
# Output: three lines — sha, deploy-id, deployed-at (empty when unknown).
# Stderr: one provenance line (`deploy_identity: sha source=...`).
set -uo pipefail

ref="${1:-}"

valid_sha() { printf '%s' "$1" | grep -Eq '^[0-9a-f]{7,40}$'; }
valid_id()  { printf '%s' "$1" | grep -Eq '^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$'; }
valid_at()  { printf '%s' "$1" | grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$'; }

emit() { printf '%s\n%s\n%s\n' "$1" "$2" "$3"; }

if [ -z "$ref" ]; then
  echo "deploy_identity: sha source=none (no container reference)" >&2
  emit "" "" ""
  exit 0
fi

env_lines="$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$ref" 2>/dev/null || true)"
env_sha="$(printf '%s\n' "$env_lines" | sed -n 's/^ARBX_DEPLOY_SHA=//p' | head -1)"
env_id="$(printf '%s\n' "$env_lines"  | sed -n 's/^ARBX_DEPLOY_ID=//p'  | head -1)"
env_at="$(printf '%s\n' "$env_lines"  | sed -n 's/^ARBX_DEPLOYED_AT=//p' | head -1)"

img="$(docker inspect -f '{{.Image}}' "$ref" 2>/dev/null || true)"
label_sha=""
if [ -n "$img" ]; then
  label_sha="$(docker inspect -f '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$img" 2>/dev/null || true)"
fi

sha=""; src="none"
if valid_sha "$label_sha"; then
  sha="$label_sha"; src="image-label"
elif valid_sha "$env_sha"; then
  sha="$env_sha"; src="container-env"
fi

did=""; dat=""
if [ -n "$sha" ] && [ "$env_sha" = "$sha" ]; then
  valid_id "$env_id" && did="$env_id"
  valid_at "$env_at" && dat="$env_at"
fi

echo "deploy_identity: sha source=${src}" >&2
emit "$sha" "$did" "$dat"
