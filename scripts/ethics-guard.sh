#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# ethics-guard — block re-introduction of prohibited fail-honest / safety
# anti-patterns into code, scripts, and CI workflows.
#
# Doctrine: OMEGA fail-honest. This is a REAL gate — it fails the build (exit 1)
# on any match, and fails CLOSED (exit 2) if the grep tool is unavailable.
#
# v1 rules (all currently clean on main — see the PR body for the audit):
#   E1  Fabricated live-readiness certificate strings.
#   E2  Fake-validation tells (a check that always passes) — the C1 anti-pattern.
#   E3  Transaction signing inside CI (--private-key / --broadcast in any
#       .github/workflows file). CI must be keyless; broadcasting belongs to the
#       operator/KMS plane.
#
# DEFERRED (documented, NOT enforced in v1): continue-on-error:true / "|| true"
# on security jobs — pervasive on current main (6 workflows + 20 files). Enforcing
# it requires first cleaning security.yml/ci.yml (a separate honest-CI PR).
#
# The guard's own files are excluded by path so its pattern definitions do not
# self-trigger.  Usage: bash scripts/ethics-guard.sh [repo-root]  (default: .)
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

command -v grep >/dev/null 2>&1 || { echo "::error::ethics-guard FAIL-CLOSED: grep unavailable"; exit 2; }

ROOT="${1:-.}"
cd "$ROOT" || { echo "::error::ethics-guard FAIL-CLOSED: cannot cd to '$ROOT'"; exit 2; }

# Scan only the dirs that exist.
CODE_DIRS=()
for d in src backend contracts scripts automation; do [ -d "$d" ] && CODE_DIRS+=("$d"); done
WF_DIR=".github/workflows"

INC=(--include='*.sh' --include='*.ts' --include='*.js' --include='*.rs' --include='*.sol' --include='*.yml' --include='*.yaml' --include='*.py')

FAIL=0

# check LABEL MESSAGE GREP_ARGS... — runs in the current shell so FAIL propagates.
# The guard's own files (path contains "ethics-guard") are dropped by path so its
# pattern definitions never self-trigger.
check() {
  local label="$1" msg="$2"; shift 2
  local out
  out="$(grep -rEn "$@" 2>/dev/null | awk -F: 'index($1, "ethics-guard")==0' || true)"
  [ -z "$out" ] && return 0
  FAIL=1
  local m file tail line
  while IFS= read -r m; do
    [ -z "$m" ] && continue
    file="${m%%:*}"; tail="${m#*:}"; line="${tail%%:*}"
    echo "::error file=${file},line=${line}::[ethics-guard:${label}] ${msg}"
    echo "  ✗ [${label}] ${file}:${line}"
  done <<< "$out"
}

echo "== ethics-guard: scanning ${CODE_DIRS[*]:-<none>} + ${WF_DIR} =="

if [ "${#CODE_DIRS[@]}" -gt 0 ]; then
  # E1 — fabricated live-readiness certificate (narrowed; not the generic "all checks passed").
  check "E1-fabricated-cert" "Fabricated live-readiness certificate string (no real validation behind it)" \
    "${INC[@]}" 'OMEGA-LIVE-DECLARATION|certified for live|READY FOR M0|Mock Live Deploy' "${CODE_DIRS[@]}"

  # E2 — fake-validation tells.
  check "E2-fake-validation" "Fake-validation marker (a check that always passes — the C1 anti-pattern)" \
    -i "${INC[@]}" 'mock the logic|always true for structure|placeholder.{0,24}always true' "${CODE_DIRS[@]}"
fi

# E3 — signing/broadcast inside CI workflows (must be keyless).
if [ -d "$WF_DIR" ]; then
  check "E3-ci-signing" "Transaction signing/broadcast inside CI (--private-key/--broadcast) — CI must be keyless; use the operator/KMS plane" \
    --include='*.yml' --include='*.yaml' '(^|[^A-Za-z0-9_-])--(private-key|broadcast)([^A-Za-z0-9_-]|$)' "$WF_DIR"
fi

if [ "$FAIL" -ne 0 ]; then
  echo ""
  echo "::error::ethics-guard FAILED — prohibited pattern(s) above. See scripts/ethics-guard.sh header."
  exit 1
fi
echo "✓ ethics-guard: no prohibited patterns found."
exit 0
