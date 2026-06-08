#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# KIMI GITHUB CHANNEL BOOTSTRAP
# ============================================================
# Purpose: Establish Kimi's GitHub CLI channel to hefarica/arbitragex-v2
# This script is READ-ONLY. It does NOT:
#   - Create secrets
#   - Push code
#   - Merge PRs
#   - Deploy anything
#   - Modify workflows
#   - Print or log tokens
# ============================================================

REPO="hefarica/arbitragex-v2"
BRANCH="omega/recovery-20260516"
PR_NUMBER=90

echo "============================================================"
echo "🜂 KIMI GITHUB CHANNEL BOOTSTRAP"
echo "============================================================"
echo "Repo:   $REPO"
echo "Branch: $BRANCH"
echo "PR:     #$PR_NUMBER"
echo "============================================================"

# ─── Step 1: Ensure gh CLI is installed ───────────────────────

if ! command -v gh >/dev/null 2>&1; then
  echo ""
  echo "[1/6] gh CLI not found. Installing..."
  
  if command -v apt-get >/dev/null 2>&1; then
    curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
      | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg 2>/dev/null
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
      | sudo tee /etc/apt/sources.list.d/github-cli.list >/dev/null
    sudo apt-get update -qq
    sudo apt-get install gh -y -qq
  elif command -v brew >/dev/null 2>&1; then
    brew install gh
  else
    echo "ERROR: Cannot auto-install gh. Install manually: https://cli.github.com/"
    exit 1
  fi
else
  echo "[1/6] gh CLI found."
fi

echo "      $(gh --version | head -1)"

# ─── Step 2: Authenticate ────────────────────────────────────

echo ""
echo "[2/6] Checking GitHub authentication..."

if gh auth status >/dev/null 2>&1; then
  echo "      Already authenticated."
elif [ -n "${GH_TOKEN:-}" ]; then
  echo "      Using GH_TOKEN from environment."
  # gh auto-consumes GH_TOKEN; just verify
  if ! gh auth status >/dev/null 2>&1; then
    echo "ERROR: GH_TOKEN present but gh auth failed."
    echo "GH_AUTH_NOT_AVAILABLE"
    exit 1
  fi
elif [ -n "${GITHUB_PAT:-}" ]; then
  echo "      Authenticating with GITHUB_PAT..."
  printf '%s' "$GITHUB_PAT" | gh auth login --with-token
  if ! gh auth status >/dev/null 2>&1; then
    echo "ERROR: GITHUB_PAT login failed."
    echo "GH_AUTH_NOT_AVAILABLE"
    exit 1
  fi
else
  echo ""
  echo "ERROR: No authentication token found."
  echo "Set GITHUB_PAT or GH_TOKEN as a secure environment variable, then rerun."
  echo ""
  echo "GH_AUTH_NOT_AVAILABLE"
  exit 1
fi

# ─── Step 3: Verify identity and permissions ──────────────────

echo ""
echo "[3/6] Verifying identity and repo access..."

GH_USER=$(gh api user --jq '.login' 2>/dev/null || echo "UNKNOWN")
echo "      GitHub user: $GH_USER"

REPO_INFO=$(gh repo view "$REPO" --json nameWithOwner,viewerPermission,isPrivate 2>/dev/null || echo "FAILED")
echo "      Repo info: $REPO_INFO"

if echo "$REPO_INFO" | grep -q "FAILED"; then
  echo "ERROR: Cannot access repository $REPO"
  echo "GH_REPO_NOT_ACCESSIBLE"
  exit 1
fi

# ─── Step 4: List workflows ──────────────────────────────────

echo ""
echo "[4/6] Listing active workflows..."
gh workflow list --repo "$REPO" 2>/dev/null || echo "WARNING: Could not list workflows"

# ─── Step 5: Read PR #90 ─────────────────────────────────────

echo ""
echo "[5/6] Reading PR #$PR_NUMBER..."

echo ""
echo "--- PR metadata ---"
gh pr view "$PR_NUMBER" --repo "$REPO" \
  --json headRefOid,headRefName,baseRefName,mergeStateStatus,mergeable,reviewDecision,state \
  2>/dev/null || echo "WARNING: Could not read PR"

echo ""
echo "--- PR checks ---"
gh pr checks "$PR_NUMBER" --repo "$REPO" 2>/dev/null || echo "WARNING: Could not read checks (may have failures)"

# ─── Step 6: Summary ─────────────────────────────────────────

echo ""
echo "============================================================"
echo "[6/6] CHANNEL STATUS"
echo "============================================================"
echo ""
echo "  GitHub User:  $GH_USER"
echo "  Repository:   $REPO"
echo "  Branch:       $BRANCH"
echo "  PR:           #$PR_NUMBER"
echo ""
echo "============================================================"
echo ""
echo "  KIMI_GITHUB_CHANNEL_READY"
echo ""
echo "============================================================"
echo ""
echo "NEXT STEPS (manual, not automated by this script):"
echo ""
echo "  1. If all 13 checks are SUCCESS and reviewDecision=APPROVED:"
echo "     gh pr merge $PR_NUMBER --repo $REPO --squash"
echo ""
echo "  2. After merge, deploy from main:"
echo "     gh workflow run deploy-vps.yml --repo $REPO --ref main"
echo ""
echo "  3. If checks fail, download logs:"
echo "     RUN_ID=\$(gh run list --repo $REPO --branch $BRANCH --workflow e2e --limit 1 --json databaseId --jq '.[0].databaseId')"
echo "     gh run view \$RUN_ID --repo $REPO --log-failed > ./ci-artifacts/failed.log"
echo ""
echo "  4. If BLOCKED by review: report BLOCKED_BY_REVIEW_REQUIRED"
echo "     Do NOT use --admin. Do NOT bypass."
echo ""
echo "============================================================"
