#!/usr/bin/env bash
# =============================================================================
# ARBITRAGEX-V2 — GATE 0 BASELINE CAPTURE (VPS-SIDE)
# =============================================================================
# Purpose: Capture a decision-grade baseline of the deployed system for drift,
#          rollback, and audit reference. This script is designed to be run on
#          the VPS by the operator, NOT by automation.
#
# Usage:
#   bash scripts/vps/capture-baseline.sh --dry-run
#   bash scripts/vps/capture-baseline.sh --tag baseline/YYYY-MM-DD-gate0 --yes
#   bash scripts/vps/capture-baseline.sh --verify-rollback baseline/YYYY-MM-DD-gate0 --dry-run
#   bash scripts/vps/capture-baseline.sh --compare baseline/YYYY-MM-DD-gate0
#
# Safety:
#   * Defaults to live capture; use --dry-run to preview. Tag creation still
#     requires operator confirmation unless --yes is explicitly passed.
#   * No secrets are written to the manifest (only key names and hashes).
#   * No automatic git push; the operator must push the tag manually.
#   * Untracked files are classified but NEVER deleted.
#
# Doctrine:
#   * paper/shadow only, no executor activation, no live flips.
#   * Fail-honest (R8): every failure is reported with exact reason.
#   * Verify against reality: read live docker/git/db/redis state.
# =============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
DEPLOY_PATH="${DEPLOY_PATH:-/opt/arbitragex-v2}"
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.prod.yml}"
BASELINE_DIR="${BASELINE_DIR:-${DEPLOY_PATH}/.baseline}"
CANONICAL_REMOTE="${CANONICAL_REMOTE:-github}"
COMPOSE_PROJECT="${COMPOSE_PROJECT:-arbitragex-v2}"
GPG_KEY="${GPG_KEY:-$(git config --global user.signingkey || true)}"

DRY_RUN=false
YES=false
TAG=""
VERIFY_ROLLBACK=""
COMPARE_TAG=""

# ---------------------------------------------------------------------------
# Utilities
# ---------------------------------------------------------------------------
log() { echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"; }
ok()  { printf '  [ \033[32mOK\033[0m ]  %s\n' "$*"; }
warn() { printf '  [ \033[33mWARN\033[0m ] %s\n' "$*"; }
fail() { printf '  [ \033[31mFAIL\033[0m ] %s\n' "$*"; }
die() { log "FATAL: $*"; exit 1; }

observations=()
add_observation() {
  local category="$1" severity="$2" detail="$3"
  observations+=("$(jq -n \
    --arg category "$category" \
    --arg severity "$severity" \
    --arg detail "$detail" \
    '{category:$category,severity:$severity,detail:$detail}')")
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "required command not found: $cmd"
}

usage() {
  cat <<'EOF'
Usage: capture-baseline.sh [OPTIONS]

OPTIONS:
  --dry-run                       Print actions without executing state changes.
  --tag NAME                      Create annotated signed tag NAME after capture.
  --yes                           Skip interactive confirmation (operator explicit).
  --verify-rollback NAME          Verify rollback reproducibility against tag NAME.
  --compare NAME                  Compare current state to baseline tag NAME.
  --help                          Show this help.

EXAMPLES:
  bash scripts/vps/capture-baseline.sh --dry-run
  bash scripts/vps/capture-baseline.sh --tag baseline/2026-07-18-gate0 --yes
  bash scripts/vps/capture-baseline.sh --verify-rollback baseline/2026-07-18-gate0 --dry-run
EOF
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=true ;;
    --tag) TAG="${2:-}"; shift ;;
    --yes) YES=true ;;
    --verify-rollback) VERIFY_ROLLBACK="${2:-}"; shift ;;
    --compare) COMPARE_TAG="${2:-}"; shift ;;
    --help|-h) usage; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done

if $DRY_RUN; then
  log "=== DRY-RUN MODE: no state changes will be made ==="
fi

# ---------------------------------------------------------------------------
# 0. Prerequisites
# ---------------------------------------------------------------------------
require_cmd git
require_cmd docker
require_cmd jq
require_cmd curl
require_cmd sha256sum

[[ -d "$DEPLOY_PATH" ]] || die "DEPLOY_PATH does not exist: $DEPLOY_PATH"
cd "$DEPLOY_PATH"

mkdir -p "$BASELINE_DIR"
exec 200>"${BASELINE_DIR}/.capture.lock"
if ! flock -n 200; then
  die "another baseline capture is already running"
fi

# ---------------------------------------------------------------------------
# Helper: resolve manifest filename from a git tag
# ---------------------------------------------------------------------------
manifest_from_tag() {
  local tag="$1"
  local file_in_tree
  file_in_tree="$(git ls-tree --name-only "$tag" .baseline/ 2>/dev/null | grep -E '^\.baseline/BASELINE-[0-9T]+\.json$' | head -n1)"
  if [[ -n "$file_in_tree" ]]; then
    git show "${tag}:${file_in_tree}" 2>/dev/null
    return 0
  fi
  return 1
}

# ---------------------------------------------------------------------------
# 1. Resolve canonical remote
# ---------------------------------------------------------------------------
CANONICAL_URL="$(git remote get-url "$CANONICAL_REMOTE" 2>/dev/null || echo "NOT_CONFIGURED")"
if [[ "$CANONICAL_URL" == "NOT_CONFIGURED" ]]; then
  die "canonical remote '$CANONICAL_REMOTE' is not configured; baseline requires github remote"
fi

# ---------------------------------------------------------------------------
# 2. Git state capture
# ---------------------------------------------------------------------------
log "=== Capturing Git state ==="
GIT_COMMIT="$(git rev-parse HEAD)"
GIT_BRANCH="$(git branch --show-current 2>/dev/null || echo "DETACHED")"
GIT_PORCELAIN="$(git status --porcelain=v1 -uall 2>/dev/null || echo "ERROR")"
GIT_CLEAN=false
[[ -z "$GIT_PORCELAIN" ]] && GIT_CLEAN=true

GIT_BEHIND=0
GIT_AHEAD=0
if git fetch "$CANONICAL_REMOTE" "$GIT_BRANCH" >/dev/null 2>&1; then
  GIT_BEHIND="$(git rev-list --count HEAD.."$CANONICAL_REMOTE/$GIT_BRANCH" 2>/dev/null || echo 0)"
  GIT_AHEAD="$(git rev-list --count "$CANONICAL_REMOTE/$GIT_BRANCH"..HEAD 2>/dev/null || echo 0)"
else
  warn "could not fetch from $CANONICAL_REMOTE; behind/ahead counts are local-only"
  add_observation "git" "warn" "fetch from $CANONICAL_REMOTE failed"
fi

# ---------------------------------------------------------------------------
# 3. Docker / Compose state capture
# ---------------------------------------------------------------------------
log "=== Capturing Docker state ==="
COMPOSE_REAL_PATH="${DEPLOY_PATH}/${COMPOSE_FILE}"
if [[ ! -f "$COMPOSE_REAL_PATH" ]]; then
  die "compose file not found: $COMPOSE_REAL_PATH"
fi
COMPOSE_SHA256="$(sha256sum "$COMPOSE_REAL_PATH" | awk '{print $1}')"

mapfile -t RUNNING_CONTAINERS < <(docker ps --filter "label=com.docker.compose.project=${COMPOSE_PROJECT}" --format '{{.Names}}' 2>/dev/null || true)

DOCKER_IMAGES_JSON="[]"
for c in "${RUNNING_CONTAINERS[@]}"; do
  [[ -z "$c" ]] && continue
  image_name="$(docker inspect "$c" --format '{{.Config.Image}}' 2>/dev/null || echo "UNKNOWN")"
  image_digest="$(docker inspect "$image_name" --format '{{index .RepoDigests 0}}' 2>/dev/null || echo "")"
  if [[ -z "$image_digest" ]]; then
    image_digest="local_only"
    add_observation "docker" "warn" "image $image_name has no RepoDigest (locally built?)"
  fi
  entry="$(jq -n \
    --arg service "$c" \
    --arg image "$image_name" \
    --arg digest "$image_digest" \
    --arg container "$c" \
    '{service:$service,image:$image,digest:$digest,container:$container}')"
  DOCKER_IMAGES_JSON="$(printf '%s\n' "$DOCKER_IMAGES_JSON" | jq --argjson e "$entry" '. + [$e]')"
done

DOCKER_VOLUMES_JSON="$(docker volume ls --format '{{.Name}}' 2>/dev/null | grep "^${COMPOSE_PROJECT}" | jq -R . | jq -s . || echo '[]')"
DOCKER_NETWORKS_JSON="$(docker network ls --filter "label=com.docker.compose.project=${COMPOSE_PROJECT}" --format '{{.Name}}' 2>/dev/null | jq -R . | jq -s . || echo '[]')"

ENV_KEYS_HASH=""
if [[ -f "${DEPLOY_PATH}/.env" ]]; then
  ENV_KEYS_HASH="$(grep -E '^[A-Z_][A-Z0-9_]*=' "${DEPLOY_PATH}/.env" 2>/dev/null | cut -d= -f1 | sort | sha256sum | awk '{print $1}')"
fi

# ---------------------------------------------------------------------------
# 4. PostgreSQL migration level
# ---------------------------------------------------------------------------
log "=== Capturing PostgreSQL migration level ==="
SCHEMA_MIGRATION_MAX="null"
MIGRATION_FILES_COUNT=0

POSTGRES_CONTAINER="$(docker ps --filter "label=com.docker.compose.project=${COMPOSE_PROJECT}" --filter "ancestor=postgres" --format '{{.Names}}' 2>/dev/null | head -n1 || echo "")"
if [[ -n "$POSTGRES_CONTAINER" ]]; then
  SCHEMA_MIGRATION_MAX="$(docker exec "$POSTGRES_CONTAINER" psql -U postgres -d arbitragex -Atc 'SELECT MAX(version) FROM schema_migrations;' 2>/dev/null || echo "null")"
  if [[ "$SCHEMA_MIGRATION_MAX" == "null" || -z "$SCHEMA_MIGRATION_MAX" ]]; then
    warn "could not read schema_migrations from postgres"
    add_observation "database" "warn" "schema_migrations query failed or empty"
  fi
else
  warn "postgres container not running; skipping DB capture"
  add_observation "database" "warn" "postgres unavailable"
fi

if [[ -d "${DEPLOY_PATH}/database/migrations" ]]; then
  MIGRATION_FILES_COUNT="$(find "${DEPLOY_PATH}/database/migrations" -maxdepth 1 -type f | wc -l)"
fi

# ---------------------------------------------------------------------------
# 5. Redis state capture
# ---------------------------------------------------------------------------
log "=== Capturing Redis state ==="
REDIS_XLEN="null"
REDIS_LASTSAVE="null"
REDIS_PERSISTENCE="unknown"

REDIS_CONTAINER="$(docker ps --filter "label=com.docker.compose.project=${COMPOSE_PROJECT}" --filter "ancestor=redis" --format '{{.Names}}' 2>/dev/null | head -n1 || echo "")"
if [[ -n "$REDIS_CONTAINER" ]]; then
  REDIS_XLEN="$(docker exec "$REDIS_CONTAINER" redis-cli XLEN arbx:opps:detected 2>/dev/null || echo "null")"
  REDIS_LASTSAVE="$(docker exec "$REDIS_CONTAINER" redis-cli LASTSAVE 2>/dev/null || echo "null")"
  REDIS_PERSISTENCE="$(docker exec "$REDIS_CONTAINER" redis-cli INFO persistence 2>/dev/null | grep -E '^rdb_|^aof_' | head -20 | tr '\n' ';' || echo "unknown")"
else
  warn "redis container not running; skipping Redis capture"
  add_observation "redis" "warn" "redis unavailable"
fi

# ---------------------------------------------------------------------------
# 6. Untracked files classification
# ---------------------------------------------------------------------------
log "=== Classifying untracked files ==="
UNTRACKED_JSON="[]"
mapfile -t UNTRACKED_FILES < <(git ls-files --others --exclude-standard 2>/dev/null || true)
for f in "${UNTRACKED_FILES[@]}"; do
  [[ -z "$f" ]] && continue
  size=0
  checksum=""
  category="unknown"
  if [[ -L "$f" ]]; then
    category="symlink"
    checksum="$(readlink "$f" 2>/dev/null || echo "")"
  elif [[ -f "$f" ]]; then
    size="$(stat -c%s "$f" 2>/dev/null || echo 0)"
    checksum="$(sha256sum "$f" 2>/dev/null | awk '{print $1}' || echo "")"
  fi
  case "$f" in
    .env|.env.*|*secret*|*credential*|*.pem|*.key) category="secret_possible" ;;
    logs/*|.audit/*|.baseline/*|frontend/.next/*|backend/target/*|*.log) category="runtime_artifact" ;;
    tmp-vps-wip/*|scripts/cartridge-deployment/*) category="runtime_cartridge" ;;
  esac
  [[ -L "$f" ]] && category="symlink"
  entry="$(jq -n \
    --arg path "$f" \
    --argjson size "$size" \
    --arg sha256 "$checksum" \
    --arg category "$category" \
    '{path:$path,size:$size,sha256:$sha256,category:$category}')"
  UNTRACKED_JSON="$(printf '%s\n' "$UNTRACKED_JSON" | jq --argjson e "$entry" '. + [$e]')"
done
UNTRACKED_COUNT="${#UNTRACKED_FILES[@]}"

# ---------------------------------------------------------------------------
# 7. Build manifest
# ---------------------------------------------------------------------------
CAPTURED_AT="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
HOSTNAME="$(hostname 2>/dev/null || echo "unknown")"

OBSERVATIONS_JSON="$(printf '%s\n' "${observations[@]}" | jq -s .)"

MANIFEST="$(jq -n \
  --arg schema_version "gate0-v1" \
  --arg captured_at "$CAPTURED_AT" \
  --arg hostname "$HOSTNAME" \
  --arg git_commit "$GIT_COMMIT" \
  --arg git_branch "$GIT_BRANCH" \
  --arg canonical_remote "$CANONICAL_REMOTE" \
  --argjson git_clean "$GIT_CLEAN" \
  --arg git_porcelain "$GIT_PORCELAIN" \
  --argjson behind_canonical "$GIT_BEHIND" \
  --argjson ahead_canonical "$GIT_AHEAD" \
  --arg compose_file "$COMPOSE_FILE" \
  --arg compose_sha256 "$COMPOSE_SHA256" \
  --argjson docker_images "$DOCKER_IMAGES_JSON" \
  --argjson docker_volumes "$DOCKER_VOLUMES_JSON" \
  --argjson docker_networks "$DOCKER_NETWORKS_JSON" \
  --arg env_keys_hash "$ENV_KEYS_HASH" \
  --arg schema_migration_max "$SCHEMA_MIGRATION_MAX" \
  --argjson migration_files_count "$MIGRATION_FILES_COUNT" \
  --argjson redis_xlen "$REDIS_XLEN" \
  --argjson redis_lastsave "$REDIS_LASTSAVE" \
  --arg redis_persistence "$REDIS_PERSISTENCE" \
  --argjson untracked_count "$UNTRACKED_COUNT" \
  --argjson untracked_files "$UNTRACKED_JSON" \
  --argjson observations "$OBSERVATIONS_JSON" \
  '{
    schema_version: $schema_version,
    captured_at: $captured_at,
    hostname: $hostname,
    git: {
      commit: $git_commit,
      branch: $git_branch,
      canonical_remote: $canonical_remote,
      working_tree_clean: $git_clean,
      porcelain: $git_porcelain,
      behind_canonical: $behind_canonical,
      ahead_canonical: $ahead_canonical
    },
    docker: {
      compose_file: $compose_file,
      compose_sha256: $compose_sha256,
      images: $docker_images,
      volumes: $docker_volumes,
      networks: $docker_networks
    },
    environment: {
      env_keys_hash: $env_keys_hash
    },
    database: {
      schema_migration_max: $schema_migration_max,
      migration_files_count: $migration_files_count
    },
    redis: {
      xlen_arbx_opps_detected: $redis_xlen,
      lastsave: $redis_lastsave,
      persistence: $redis_persistence
    },
    untracked: {
      count: $untracked_count,
      files: $untracked_files
    },
    observations: $observations
  }')"

# ---------------------------------------------------------------------------
# 8. Persist artifacts
# ---------------------------------------------------------------------------
TS="$(date -u '+%Y%m%dT%H%M%SZ')"
MANIFEST_NAME="BASELINE-${TS}.json"
MANIFEST_PATH="${BASELINE_DIR}/${MANIFEST_NAME}"

if ! $DRY_RUN; then
  printf '%s\n' "$MANIFEST" > "$MANIFEST_PATH"
  git status --porcelain=v1 -uall > "${BASELINE_DIR}/BASELINE-${TS}.git.txt"
  printf '%s\n' "${UNTRACKED_FILES[@]}" > "${BASELINE_DIR}/BASELINE-${TS}.untracked.txt"
  ok "manifest written to ${MANIFEST_PATH}"
else
  printf '  [DRY-RUN] would write manifest to %s\n' "$MANIFEST_PATH"
  printf '%s\n' "$MANIFEST" | jq .
fi

# ---------------------------------------------------------------------------
# 9. Optional: create signed annotated tag
# ---------------------------------------------------------------------------
if [[ -n "$TAG" ]]; then
  if [[ -z "$TAG" || ! "$TAG" =~ ^[a-zA-Z0-9_/.:-]+$ ]]; then
    die "invalid tag name: $TAG (allowed: [a-zA-Z0-9_/.:-]+)"
  fi
  if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null 2>&1; then
    die "tag $TAG already exists"
  fi
  if $DRY_RUN; then
    printf '  [DRY-RUN] would create signed tag %s (no state change)\n' "$TAG"
    exit 0
  elif ! $YES; then
    die "--tag requires --yes (or --dry-run) to prevent accidental state changes"
  fi

  TAG_MESSAGE="Gate 0 baseline ${TS}
manifest: ${MANIFEST_NAME}
commit: ${GIT_COMMIT}
compose: ${COMPOSE_FILE} sha256=${COMPOSE_SHA256}
schema_migration_max: ${SCHEMA_MIGRATION_MAX}
redis_xlen: ${REDIS_XLEN}"

  if ! $DRY_RUN; then
    if [[ -z "$GPG_KEY" ]]; then
      die "GPG signing key not configured; set user.signingkey or pass GPG_KEY"
    fi
    git tag -s -- "$TAG" -m "$TAG_MESSAGE"
    ok "signed annotated tag created: $TAG"
    warn "operator must push manually: git push github $TAG"
  else
    printf '  [DRY-RUN] would create signed tag %s with message:\n%s\n' "$TAG" "$TAG_MESSAGE"
  fi
fi

# ---------------------------------------------------------------------------
# 10. Optional: compare current state to existing baseline tag
# ---------------------------------------------------------------------------
if [[ -n "$COMPARE_TAG" ]]; then
  log "=== Comparing current state to $COMPARE_TAG ==="
  BASE_TAG_COMMIT="$(git rev-list -n1 "$COMPARE_TAG" 2>/dev/null || echo "UNKNOWN")"
  if [[ "$BASE_TAG_COMMIT" == "UNKNOWN" ]]; then
    die "tag $COMPARE_TAG not found"
  fi

  BASE_MANIFEST="$(manifest_from_tag "$COMPARE_TAG" || true)"
  if [[ -z "$BASE_MANIFEST" ]]; then
    local_file="$(find "$BASELINE_DIR" -name 'BASELINE-*.json' -type f -print -quit 2>/dev/null || echo "")"
    [[ -n "$local_file" ]] && BASE_MANIFEST="$(cat "$local_file")"
  fi
  [[ -z "$BASE_MANIFEST" ]] && die "no manifest found for tag $COMPARE_TAG"

  BASE_COMMIT="$(printf '%s\n' "$BASE_MANIFEST" | jq -r '.git.commit')"
  BASE_COMPOSE_SHA="$(printf '%s\n' "$BASE_MANIFEST" | jq -r '.docker.compose_sha256')"

  if [[ "$GIT_COMMIT" == "$BASE_COMMIT" ]]; then
    ok "HEAD matches baseline commit"
  else
    fail "HEAD differs from baseline commit: ${BASE_COMMIT} -> ${GIT_COMMIT}"
  fi

  if [[ "$COMPOSE_SHA256" == "$BASE_COMPOSE_SHA" ]]; then
    ok "compose file hash matches baseline"
  else
    fail "compose file hash differs from baseline"
  fi

  while IFS='|' read -r svc base_digest; do
    [[ -z "$svc" ]] && continue
    current_digest="$(printf '%s\n' "$DOCKER_IMAGES_JSON" | jq -r --arg s "$svc" '.[] | select(.service==$s) | .digest')"
    if [[ "$current_digest" == "$base_digest" ]]; then
      ok "${svc} digest matches baseline"
    else
      fail "${svc} digest differs: baseline=${base_digest} current=${current_digest}"
    fi
  done < <(printf '%s\n' "$BASE_MANIFEST" | jq -r '.docker.images[] | "\(.service)|\(.digest)"')
fi

# ---------------------------------------------------------------------------
# 11. Optional: rollback verification
# ---------------------------------------------------------------------------
if [[ -n "$VERIFY_ROLLBACK" ]]; then
  log "=== Verifying rollback reproducibility for $VERIFY_ROLLBACK ==="
  RB_MANIFEST=""
  if git rev-parse "$VERIFY_ROLLBACK" >/dev/null 2>&1; then
    RB_MANIFEST="$(manifest_from_tag "$VERIFY_ROLLBACK" || true)"
  fi
  if [[ -z "$RB_MANIFEST" ]]; then
    rb_file="$(find "$BASELINE_DIR" -name 'BASELINE-*.json' -type f -print -quit 2>/dev/null || echo "")"
    [[ -n "$rb_file" ]] && RB_MANIFEST="$(cat "$rb_file")"
  fi
  [[ -z "$RB_MANIFEST" ]] && die "no manifest found for rollback tag $VERIFY_ROLLBACK"

  RB_COMMIT="$(printf '%s\n' "$RB_MANIFEST" | jq -r '.git.commit')"
  ok "rollback target commit: $RB_COMMIT"

  missing_images=0
  while IFS='|' read -r img; do
    [[ -z "$img" ]] && continue
    if docker inspect "$img" >/dev/null 2>&1; then
      ok "image available: $img"
    else
      fail "image NOT available locally: $img"
      missing_images=$((missing_images + 1))
    fi
  done < <(printf '%s\n' "$RB_MANIFEST" | jq -r '.docker.images[] | "\(.image)@\(.digest)"')

  if $DRY_RUN; then
    printf '  [DRY-RUN] would check image availability only (no compose override generated in this version)\n'
  else
    if [[ $missing_images -eq 0 ]]; then
      ok "all baseline images available locally; rollback is feasible"
    else
      fail "some baseline images are missing; rollback requires re-pull/build"
    fi
    warn "operator must manually run: docker compose down && docker compose up -d"
  fi
fi

log "=== Gate 0 baseline capture complete ==="
