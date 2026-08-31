#!/bin/bash
# ArbitrageX v2 — Deployment Script
#
# Audit N1 (2026-05-10 re-run): this script previously claimed "Production"
# but invoked compose.dev.yml. Catastrophic operator confusion vector.
#
# Now: explicit COMPOSE_FILE selection (default: dev). Operator must opt-in
# to prod via `COMPOSE_FILE=docker/compose.prod.yml ./scripts/deploy.sh up -d`
# OR rename to scripts/deploy-dev.sh / scripts/deploy-prod.sh.
#
# Always exports .env vars before docker compose to ensure proper interpolation.
#
# ARBX-R-0004 (REGRESSION, incident 08-22 16:22 — VPS on a foreign branch while
# deploys kept "succeeding"): `git pull origin main` MERGES INTO whatever branch
# is checked out, and the old `|| WARN; continue` fallback deployed unverified
# local HEAD. Gates now (§37 G4 "deploy veraz"):
#   1. deploy ALWAYS from verified main — refuse any other branch/detached HEAD;
#   2. checkout state documented — branch + HEAD sha before AND after pull;
#   3. KNOWN_GOOD_REVISION explicit — REQUIRED for prod (the SHA CI verified),
#      optional for dev; any drift (incl. post-pull HEAD != origin/main) aborts.
# Untracked files (.env, logs) are legitimate VPS runtime state — only TRACKED
# modifications dirty the tree (submodules ignored: not in the built images).
set -euo pipefail

cd /opt/arbitragex-v2

# Compose file selection — default DEV, operator overrides for PROD.
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.dev.yml}"

# Sanity: file must exist
if [ ! -f "$COMPOSE_FILE" ]; then
    echo "ERROR: COMPOSE_FILE='$COMPOSE_FILE' does not exist" >&2
    echo "  Available: $(ls docker/compose*.yml 2>/dev/null | tr '\n' ' ')" >&2
    exit 1
fi

# Sanity: refuse to run prod compose unless explicit confirmation
if [[ "$COMPOSE_FILE" == *"prod"* ]]; then
    if [ "${CONFIRM_PROD_DEPLOY:-}" != "true" ]; then
        echo "ERROR: prod compose detected ($COMPOSE_FILE) — set CONFIRM_PROD_DEPLOY=true to proceed" >&2
        echo "  Reason: prevent accidental prod deploys (audit N1, 2026-05-10)" >&2
        exit 1
    fi
fi

echo "=== ArbitrageX v2 Deploy ==="
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Compose file: $COMPOSE_FILE"
echo "Args: $*"

# ── DEPLOY-WATCHDOG-RACE gate (GEN-CI-FAIL attempts 1+3, 2026-08-31) ──────────
# Same flock the deploy workflow and the minute cron watchdog use. Without it
# a manual deploy racing the workflow (or the watchdog firing mid-recreate)
# produces "Error response from daemon: No such container: …". Wait up to
# 10min for a concurrent deploy, then fail loud — never race.
DEPLOY_LOCK="/tmp/arbx-deploy.lock"
touch "$DEPLOY_LOCK" && chmod 666 "$DEPLOY_LOCK" 2>/dev/null || true
exec 9>"$DEPLOY_LOCK"
if ! flock -w 600 9; then
    echo "ERROR: another deploy holds ${DEPLOY_LOCK} for >10min — refusing to race it" >&2
    exit 1
fi
echo "deploy lock acquired: ${DEPLOY_LOCK} (fd 9, held for script lifetime)"

# Export .env vars for compose ${} interpolation
# shellcheck disable=SC2046
export $(grep -v '^#' .env | grep -v '^$' | xargs)

# ── ARBX-R-0004 gate 1+2: verified main + documented checkout ─────────────────
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
HEAD_BEFORE="$(git rev-parse HEAD)"
echo "Checkout state: branch=$BRANCH HEAD=$HEAD_BEFORE"

if [ "$BRANCH" != "main" ]; then
    echo "ERROR: working tree is on branch '$BRANCH', not main — refusing to deploy (ARBX-R-0004)" >&2
    echo "  Incident 08-22: git pull would merge INTO this branch. Fix: git checkout main" >&2
    exit 1
fi

if ! git -c diff.ignoreSubmodules=all diff --quiet HEAD || \
   ! git -c diff.ignoreSubmodules=all diff --cached --quiet; then
    echo "ERROR: tracked files are modified — the deployed tree would match no revision (ARBX-R-0004)" >&2
    git -c diff.ignoreSubmodules=all status --porcelain | grep -v '^??' | head -20 >&2
    exit 1
fi

# Pull with FULL output (no 2>/dev/null — it is the audit trail). With errexit,
# a failed pull aborts: an unverifiable tree is not deployable (fail-closed).
git pull origin main

HEAD_AFTER="$(git rev-parse HEAD)"
ORIGIN_MAIN="$(git rev-parse origin/main)"
echo "Post-pull state: HEAD=$HEAD_AFTER origin/main=$ORIGIN_MAIN"

if [ "$HEAD_AFTER" != "$ORIGIN_MAIN" ]; then
    echo "ERROR: HEAD ($HEAD_AFTER) != origin/main ($ORIGIN_MAIN) after pull — refusing (ARBX-R-0004)" >&2
    exit 1
fi

# ── ARBX-R-0004 gate 3: KNOWN_GOOD_REVISION explicit ──────────────────────────
# Prod requires the SHA the operator/CI verified (e.g. the merged PR SHA); dev
# may omit it. Any mismatch aborts BEFORE a container is touched.
if [ "${CONFIRM_PROD_DEPLOY:-}" = "true" ] && [ -z "${KNOWN_GOOD_REVISION:-}" ]; then
    echo "ERROR: prod deploy without KNOWN_GOOD_REVISION — pass the verified SHA (ARBX-R-0004)" >&2
    exit 1
fi
if [ -n "${KNOWN_GOOD_REVISION:-}" ]; then
    if [ "$HEAD_AFTER" != "$KNOWN_GOOD_REVISION" ]; then
        echo "ERROR: HEAD ($HEAD_AFTER) != KNOWN_GOOD_REVISION ($KNOWN_GOOD_REVISION) — refusing (ARBX-R-0004)" >&2
        exit 1
    fi
    echo "Known-good revision verified: $KNOWN_GOOD_REVISION"
fi

# Deploy
docker compose --env-file .env -f "$COMPOSE_FILE" "$@"

echo "=== Deploy complete ($COMPOSE_FILE) ==="
