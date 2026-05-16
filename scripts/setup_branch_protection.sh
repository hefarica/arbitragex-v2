#!/usr/bin/env bash
# scripts/setup_branch_protection.sh
#
# OMEGA-102 Delta Pack · 2026-05-16
# Idempotent setup of branch protection on `main` for hefarica/arbitragex-v2.
#
# Two modes:
#   bash scripts/setup_branch_protection.sh
#       → bootstrap mode: enforces reviewers + linear history, BUT does
#         NOT yet require status checks. Use this on first run, before
#         workflows have produced any green check.
#
#   bash scripts/setup_branch_protection.sh --enforce-required-checks
#       → final mode: adds the canonical required status checks. Run this
#         only AFTER at least one PR has been merged green with all
#         workflows producing the expected contexts.
#
# Requires:
#   - GITHUB_TOKEN env var with admin:repo on hefarica/arbitragex-v2
#   - curl, jq
#   - The repo MUST already be private (Fase 0 of OMEGA-102 rollout)
#
# Safety:
#   - This script NEVER force-pushes, NEVER rewrites history.
#   - It only mutates branch protection settings via PUT /branches/main/protection.
#   - It is idempotent: running twice produces the same state.

set -euo pipefail

REPO="${REPO:-hefarica/arbitragex-v2}"
BRANCH="${BRANCH:-main}"
API="https://api.github.com/repos/${REPO}/branches/${BRANCH}/protection"

if [ -z "${GITHUB_TOKEN:-}" ]; then
  echo "::error::GITHUB_TOKEN env var is required (admin:repo scope)" >&2
  exit 1
fi

ENFORCE_CHECKS=0
for arg in "$@"; do
  case "$arg" in
    --enforce-required-checks) ENFORCE_CHECKS=1 ;;
    -h|--help)
      sed -n '2,30p' "$0"
      exit 0
      ;;
    *)
      echo "::warning::unknown arg: $arg (ignored)" >&2
      ;;
  esac
done

# Canonical required status checks. Authoring intent: every workflow that
# blocks production readiness must appear here. Order does not matter;
# names must match the `name:` field of each workflow job.
REQUIRED_CHECKS_JSON='[
  "cargo audit (Rust advisories)",
  "npm audit (Node advisories)",
  "gitleaks (secrets scanner)",
  "analyze (rust)",
  "analyze (javascript-typescript)",
  "repo policy invariants",
  "rust workspace tests",
  "typescript build + tests",
  "frontend build + tests",
  "foundry tests (Solidity)",
  "e2e tests",
  "no-hardcode lint",
  "PII gates",
  "dockerfile audit"
]'

if [ "$ENFORCE_CHECKS" -eq 1 ]; then
  CHECKS_CLAUSE=$(jq -n --argjson c "$REQUIRED_CHECKS_JSON" '{strict: true, contexts: $c}')
else
  # null means "do not require any status checks" — bootstrap-safe.
  CHECKS_CLAUSE="null"
fi

PAYLOAD=$(jq -n --argjson required_status_checks "$CHECKS_CLAUSE" '{
  required_status_checks: $required_status_checks,
  enforce_admins: true,
  required_pull_request_reviews: {
    required_approving_review_count: 1,
    dismiss_stale_reviews: true,
    require_code_owner_reviews: true,
    require_last_push_approval: true
  },
  restrictions: null,
  required_linear_history: true,
  allow_force_pushes: false,
  allow_deletions: false,
  required_conversation_resolution: true,
  lock_branch: false,
  allow_fork_syncing: false,
  block_creations: false
}')

echo "::group::PUT $API"
echo "$PAYLOAD" | jq .
echo "::endgroup::"

HTTP_CODE=$(curl -sS -o /tmp/branch_protection_resp.json -w "%{http_code}" \
  -X PUT "$API" \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -d "$PAYLOAD")

if [ "$HTTP_CODE" != "200" ]; then
  echo "::error::PUT failed with HTTP $HTTP_CODE" >&2
  cat /tmp/branch_protection_resp.json >&2 || true
  exit 1
fi

echo "ok · branch protection updated"
if [ "$ENFORCE_CHECKS" -eq 1 ]; then
  REQ=$(jq -r '.required_status_checks.contexts | length' /tmp/branch_protection_resp.json)
  echo "ok · $REQ required status checks enforced"
else
  echo "note · status checks NOT yet enforced (bootstrap mode)"
  echo "      run again with --enforce-required-checks after first green merge"
fi
