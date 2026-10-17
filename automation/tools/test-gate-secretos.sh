#!/usr/bin/env bash
# test-gate-secretos.sh — regression test for the E-7 secret-exposure class.
#
# Incident this test pins (GATE-SECRETOS-01 / E-7): `.claude/settings.json`
# carried a live authentication token as a JSON `"KEY": "<value>"` pair. It was
# tracked from 2026-07-11 to 2026-09-30 and NO gate saw it — not because it was
# allowlisted, but because the scanners had the wrong TEMPORAL SCOPE and none of
# them understood the JSON pair shape.
#
# What this test asserts, on throwaway fixture repos (never on the real tree):
#   A1  a TRACKED `.claude/settings.json` with a JSON secret pair  -> gate FAILS
#   A2  a TRACKED `.claude/<other>.json` with the same shape       -> gate FAILS
#       (A2 is the discriminator for the JSON-FORM rule alone: `.claude/other-*`
#        is not covered by the tracked-path rule, so only category 5 can catch it)
#   A3  the corrected state (untracked + ignored + placeholder)     -> gate PASSES
#   A4  MUTATION SELF-CHECK: with the JSON-form rule neutered, A2 must stop
#       failing. If the mutant survives, this suite exits non-zero.
#
# Nothing here reads, compares, prints or commits a real credential: the fixture
# value is a synthetic string generated in a temp dir at runtime.
#
# Usage:
#   bash automation/tools/test-gate-secretos.sh
#   GATE_BIN=/path/to/mutant.sh bash automation/tools/test-gate-secretos.sh
#     -> with a mutant (JSON rule removed) the suite MUST fail. That is the
#        mutation-sensitivity evidence.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE_BIN="${GATE_BIN:-$HERE/lint-no-hardcode.sh}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

FAILURES=0
pass() { printf 'PASS: %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; FAILURES=$((FAILURES+1)); }

# Fixture value is DERIVED at runtime, never written as a literal.
#
# Why (measured, not stylistic): the first push of this test carried a hardcoded
# 32-char fixture constant. gitleaks flagged it — correctly — with
# `RuleID: generic-api-key, Entropy: 4.954, File: automation/tools/test-gate-secretos.sh:41`.
# The fix-forward is to remove the secret-shaped constant, NOT to allowlist the
# test: a repo must not need a scanner exemption to test its own scanner.
# The generated value keeps the SHAPE the gate must catch (>=16 chars, no
# placeholder marker), so the assertion is unchanged.
FIXTURE_SECRET="$(printf '%s' 'arbx gate secretos fixture seed 01' | sha256sum | cut -c1-32)"

run_gate_in() { # run_gate_in <dir> <gate> ; prints output, returns gate exit code
  local dir="$1" gate="$2"
  ( cd "$dir" && ARBX_GATE_ROOT="$dir" bash "$gate" ) 2>&1
}

new_fixture() { # new_fixture <dir>
  local dir="$1"
  mkdir -p "$dir/.claude"
  git -C "$dir" init -q
  git -C "$dir" config user.email fixture@example.invalid
  git -C "$dir" config user.name fixture
}

write_secret_json() { # write_secret_json <path>
  cat > "$1" <<JSON
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "$FIXTURE_SECRET"
  }
}
JSON
}

# ─── A1: tracked operator-local settings.json with a JSON secret pair ──────
F1="$WORK/f1"
new_fixture "$F1"
write_secret_json "$F1/.claude/settings.json"
git -C "$F1" add -f .claude/settings.json
OUT1="$(run_gate_in "$F1" "$GATE_BIN")"; RC1=$?
if [ "$RC1" -ne 0 ]; then pass "A1 gate rejects a tracked .claude/settings.json with a JSON secret pair (exit $RC1)"; else fail "A1 gate ACCEPTED a tracked .claude/settings.json with a JSON secret pair (exit 0)"; fi
if printf '%s' "$OUT1" | grep -q 'BLOCKING\[tracked-operator-local-secret-path\]'; then pass "A1 reports the tracked-path rule"; else fail "A1 did not report BLOCKING[tracked-operator-local-secret-path]"; fi
if printf '%s' "$OUT1" | grep -q 'BLOCKING\[json-secret-claude\]'; then pass "A1 reports the JSON-form rule"; else fail "A1 did not report BLOCKING[json-secret-claude]"; fi

# ─── A2: JSON-FORM discriminator (tracked, but NOT a settings* path) ───────
F2="$WORK/f2"
new_fixture "$F2"
write_secret_json "$F2/.claude/other-credentials.json"
git -C "$F2" add -f .claude/other-credentials.json
OUT2="$(run_gate_in "$F2" "$GATE_BIN")"; RC2=$?
if [ "$RC2" -ne 0 ]; then pass "A2 gate rejects a tracked .claude/other-credentials.json with a JSON secret pair (exit $RC2)"; else fail "A2 gate ACCEPTED the JSON secret pair shape (exit 0) — the E-7 hole is open"; fi
if printf '%s' "$OUT2" | grep -q 'BLOCKING\[json-secret-claude\]'; then pass "A2 JSON-form rule is the catcher"; else fail "A2 BLOCKING[json-secret-claude] absent"; fi

# ─── A3: corrected state must PASS ─────────────────────────────────────────
F3="$WORK/f3"
new_fixture "$F3"
echo '.claude/settings.json' > "$F3/.gitignore"
cat > "$F3/.claude/settings.json" <<'JSON'
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "your-token-here"
  }
}
JSON
OUT3="$(run_gate_in "$F3" "$GATE_BIN")"; RC3=$?
if [ "$RC3" -eq 0 ]; then pass "A3 corrected state (untracked + ignored + placeholder) passes (exit 0)"; else fail "A3 corrected state was rejected (exit $RC3)"; fi

# ─── A4: mutation self-check — neuter the JSON-form rule and re-run A2 ─────
MUTANT="$WORK/mutant.sh"
sed 's|^JSON_SECRET_RE=.*|JSON_SECRET_RE="THIS_PATTERN_NEVER_MATCHES"|' "$GATE_BIN" > "$MUTANT"
if grep -q 'THIS_PATTERN_NEVER_MATCHES' "$MUTANT"; then
  OUT2M="$(run_gate_in "$F2" "$MUTANT")"; RC2M=$?
  if [ "$RC2M" -eq 0 ]; then
    pass "A4 MUTATION-DETECTED: neutering the JSON-form rule makes A2 pass again (rule is the cause, not the fixture)"
  else
    fail "A4 MUTATION-SURVIVED: A2 still fails with the JSON-form rule neutered (something else catches it — A2 is not a discriminator)"
  fi
  if printf '%s' "$OUT2M" | grep -q 'BLOCKING\[json-secret-claude\]'; then
    fail "A4 mutant still emitted BLOCKING[json-secret-claude] — mutation was not effective"
  else
    pass "A4 mutant emitted no json-secret-claude finding (mutation effective)"
  fi
else
  fail "A4 could not build the mutant (json-secret rule assignment not found in $GATE_BIN)"
fi

printf '\n'
if [ "$FAILURES" -gt 0 ]; then
  printf 'test-gate-secretos: %s assertion(s) FAILED\n' "$FAILURES" >&2
  exit 1
fi
printf 'test-gate-secretos: all assertions passed (gate=%s)\n' "$GATE_BIN"
exit 0
