#!/usr/bin/env bash
# MEV Ethics Gate Linter — CI enforcement
#
# Usage: ./mev-ethics-lint.sh [--fix]
#
# Scans the codebase for MEV naming red flags and architectural violations.
# Fails the build if violations are found. Use --fix to auto-comment violations.
#
# Enforces:
# 1. Naming red flags (victim*, targetUser*, frontrun*, etc.)
# 2. "Depends on specific user tx" patterns
# 3. "Extract from user" patterns
# 4. Unvalidated bundle construction

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && git rev-parse --show-toplevel)"
FIX="${1:-}"

RED_FLAGS=(
  "victim"
  "targetUser"
  "frontrun"
  "sandwich"
  "extractValueFromUser"
  "racePending"
  "unwindAfterVictim"
  "priceManipulation"
  "oracleNudge"
  "jitDisplace"
  "reorgClaim"
  "genericFrontrun"
)

VIOLATION_COUNT=0
VIOLATION_DETAILS=()

echo "[MEV Ethics Linter] Scanning for naming red flags and violations..."

# Pattern 1: Naming red flags in identifiers
for flag in "${RED_FLAGS[@]}"; do
  # Case-insensitive grep; use pathspec to filter files properly
  matches=$(git -C "$REPO_ROOT" grep -iE "(fn|struct|enum|const|let|var|class|function|def) .*${flag}" \
    -- \
    '*.rs' '*.ts' '*.tsx' '*.sol' \
    ':!*/node_modules/*' \
    ':!*/target/*' \
    ':!*/.git/*' \
    ':!*.test.ts' \
    ':!*.spec.rs' \
    ':!mev_ethics_gate.rs' \
    2>/dev/null || echo "")

  if [[ -n "$matches" ]]; then
    VIOLATION_COUNT=$((VIOLATION_COUNT + 1))
    VIOLATION_DETAILS+=("Naming red flag '${flag}' found in identifier")
  fi
done

# Pattern 2: "specific.*pending.*tx" patterns
matches2=$(git -C "$REPO_ROOT" grep -iE "specific.*pending|pending.*specific|targetUser|victim.*tx" \
  -- \
  '*.rs' '*.ts' '*.tsx' '*.sol' \
  ':!*/node_modules/*' \
  ':!*/target/*' \
  ':!*/.git/*' \
  2>/dev/null || echo "")

if [[ -n "$matches2" ]]; then
  VIOLATION_COUNT=$((VIOLATION_COUNT + 1))
  VIOLATION_DETAILS+=("Found pattern suggesting 'specific pending tx' dependency")
fi

# Pattern 3: Bundle construction without validation comment
matches3=$(git -C "$REPO_ROOT" grep -E "BundlePosition|bundle_new|Bundle\{" \
  -- \
  '*.rs' '*.ts' '*.tsx' '*.sol' \
  ':!*/node_modules/*' \
  ':!*/target/*' \
  ':!*/.git/*' \
  -A 2 2>/dev/null | grep -v "eth_sendBundle\|Flashbots\|MEV-Share\|private.*relay\|// .*non-extractive\|// .*arbitrage\|// .*liquidation" || echo "")

# Pattern 3 is informational only, not a hard violation
if [[ -n "$matches3" ]]; then
  echo "[INFO] Bundle construction found without explicit validation comment (not blocking)"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "[MEV Ethics Linter] Scan complete"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ $VIOLATION_COUNT -gt 0 ]]; then
  echo ""
  echo "❌ Found ${VIOLATION_COUNT} potential violation(s):"
  for detail in "${VIOLATION_DETAILS[@]}"; do
    echo "  • $detail"
  done
  echo ""
  echo "See arbx-mev-ethics-gate SKILL.md for PERMITTED/PROHIBITED strategies."
  echo "Violations must be justified by human code review or refactored away."
  exit 1
else
  echo "✅ No naming red flags detected."
  exit 0
fi
