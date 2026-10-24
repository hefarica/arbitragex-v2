#!/usr/bin/env bash
# test-lint-fail-closed.sh — controls for automation/tools/lint-no-hardcode.sh
#
# Purpose: prove the linter actually gates. A gate without a negative control
# is a virgin gate (doctrine: "un gate sin artefacto es virgen y no es verde").
#
# Controls:
#   C1 positive  clean tree (baseline committed)            -> exit 0
#   C2 negative  one NEW violation, not baselined           -> exit != 0 and names it
#   C3 negative  baseline file missing                      -> exit != 0 (fail-closed)
#   C4 negative  baseline tampered (body-sha mismatch)      -> exit != 0 (fail-closed)
#   C5 negative  baseline regen without declared window     -> exit != 0
#
# Usage: bash automation/tools/test-lint-fail-closed.sh
# Exit 0 only when every control behaved as specified.

set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

LINT="automation/tools/lint-no-hardcode.sh"
BASELINE="automation/tools/lint-no-hardcode.baseline"
CONTROL_FILE="backend/searcher-rs/src/z_lint_control_negativo.rs"
CONTROL_LINE='const CONTROL_NEGATIVO: &str = "0x1111111111111111111111111111111111111111";'

FAILED=0
TMPDIR_CTL="$(mktemp -d)"
# Snapshot the working state so C6 compares against it instead of demanding an
# empty tree (the PR's own uncommitted edits are expected to be there).
PRE_STATUS="$(git status --porcelain --untracked-files=no)"
PRE_BASELINE_SHA="$(sha256sum "$BASELINE" 2>/dev/null | cut -d' ' -f1)"

remove_control() {
  git rm -q --cached --ignore-unmatch "$CONTROL_FILE" >/dev/null 2>&1 || true
  rm -f "$CONTROL_FILE"
}

cleanup() {
  remove_control
  rm -rf "$TMPDIR_CTL"
}
trap cleanup EXIT

pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1" >&2; FAILED=$((FAILED+1)); }

expect_rc() { # expect_rc <name> <expected-nonzero|zero> <rc> <output> <substring|-> 
  local name="$1" want="$2" rc="$3" out="$4" needle="$5"
  if [ "$want" = "zero" ]; then
    if [ "$rc" -ne 0 ]; then fail "$name: expected exit 0, got $rc"; return; fi
  else
    if [ "$rc" -eq 0 ]; then fail "$name: expected non-zero exit, got 0 (fail-open!)"; return; fi
  fi
  if [ "$needle" != "-" ] && ! printf '%s' "$out" | grep -qF -- "$needle"; then
    fail "$name: output does not mention '$needle'"; return
  fi
  pass "$name (exit $rc)"
}

printf '== C1 positive: clean tree, baseline committed ==\n'
out="$(bash "$LINT" 2>&1)"; rc=$?
printf '%s\n' "$out" | tail -1
expect_rc "C1 clean-tree" zero "$rc" "$out" "-"

printf '\n== C2 negative: one NEW violation, not in baseline ==\n'
printf '%s\n' "$CONTROL_LINE" > "$CONTROL_FILE"
git add "$CONTROL_FILE"
out="$(bash "$LINT" 2>&1)"; rc=$?
printf '%s\n' "$out" | grep -F "NEW findings" -A2 | head -4
expect_rc "C2 new-violation" nonzero "$rc" "$out" "$CONTROL_FILE"
remove_control

printf '\n== C3 negative: baseline file missing ==\n'
out="$(LINT_BASELINE_FILE="$TMPDIR_CTL/no-such-baseline" bash "$LINT" 2>&1)"; rc=$?
printf '%s\n' "$out" | grep -F "FAIL-CLOSED" | head -2
expect_rc "C3 missing-baseline" nonzero "$rc" "$out" "FAIL-CLOSED"

printf '\n== C4 negative: baseline tampered (body-sha mismatch) ==\n'
cp "$BASELINE" "$TMPDIR_CTL/tampered.baseline"
printf '%s\n' "address|backend/fake.rs|deadbeefdeadbeefdeadbeefdeadbeef" >> "$TMPDIR_CTL/tampered.baseline"
out="$(LINT_BASELINE_FILE="$TMPDIR_CTL/tampered.baseline" bash "$LINT" 2>&1)"; rc=$?
printf '%s\n' "$out" | grep -F "integrity" | head -2
expect_rc "C4 tampered-baseline" nonzero "$rc" "$out" "integrity"

printf '\n== C5 negative: regen without declared window ==\n'
out="$(LINT_BASELINE_REGEN=1 bash "$LINT" 2>&1)"; rc=$?
printf '%s\n' "$out" | head -2
expect_rc "C5 regen-without-window" nonzero "$rc" "$out" "LINT_BASELINE_WINDOW"

printf '\n== C6 clean-up check: state unchanged, control file gone, baseline untouched ==\n'
dirty="$(git status --porcelain --untracked-files=no)"
post_sha="$(sha256sum "$BASELINE" 2>/dev/null | cut -d' ' -f1)"
if [ "$dirty" != "$PRE_STATUS" ]; then
  fail "C6 state-unchanged: tracked-file status changed during the controls
$dirty"
elif [ "$post_sha" != "$PRE_BASELINE_SHA" ]; then
  fail "C6 state-unchanged: baseline rewritten by a refused regeneration (pre=$PRE_BASELINE_SHA post=$post_sha)"
elif git ls-files --error-unmatch "$CONTROL_FILE" >/dev/null 2>&1; then
  fail "C6 state-unchanged: $CONTROL_FILE still in the index"
elif [ -e "$CONTROL_FILE" ]; then
  fail "C6 state-unchanged: $CONTROL_FILE still on disk"
else
  pass "C6 state-unchanged"
fi

printf '\n'
if [ "$FAILED" -gt 0 ]; then
  printf 'lint-fail-closed controls: %s FAILED\n' "$FAILED" >&2
  exit 1
fi
printf 'lint-fail-closed controls: all passed\n'
exit 0
