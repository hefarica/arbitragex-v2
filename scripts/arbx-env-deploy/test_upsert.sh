#!/usr/bin/env bash
# contract tests for arbx_env_upsert.sh — runnable anywhere, no VPS and no docker.
#
# WHY THIS FILE EXISTS (2026-10-19): README.md claimed "proven by
# `scripts/arbx-env-deploy/test_upsert.sh`, 15/15" and that file was NOT in the
# tree. The guarantee was an assertion without an artifact — exactly the class
# of green this project forbids. This restores the test AND makes the number in
# the README come from a run, not from memory.
#
# The five controls LEVI's card asks for, plus the structural promises the
# script's own header makes:
#   1. re-running is byte-identical (idempotence)
#   2. a CRLF fragment never injects \r (Excel/VBA writes CRLF)
#   3. a COMMENTED key (#KEY=old) is not clobbered
#   4. a value containing '=' (multi-provider RPC) survives literally
#   5. a duplicated key in the fragment resolves once, last value wins
#
# Usage: bash scripts/arbx-env-deploy/test_upsert.sh
# Exit 0 = all checks pass; 1 = at least one failed (the failing check is named).
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
UPSERT="$HERE/arbx_env_upsert.sh"
[ -f "$UPSERT" ] || { echo "FATAL: $UPSERT not found" >&2; exit 2; }

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
pass=0; fail=0; total=0

check() { # check <name> <expected> <actual>
  total=$((total + 1))
  if [ "$2" = "$3" ]; then
    pass=$((pass + 1)); printf '  ok    %s\n' "$1"
  else
    fail=$((fail + 1)); printf '  FAIL  %s\n        expected: %q\n        actual:   %q\n' "$1" "$2" "$3"
  fi
}

# --- fixture: a realistic .env with comments, blanks, a commented key ---------
TARGET="$WORK/target.env"
cat > "$TARGET" <<'ENV'
# managed by arbx-env-deploy — do not edit by hand
POSTGRES_PASSWORD=old-pass
GITHUB_TOKEN=keepme

# a comment between keys
AAVE_POOL_1=0xold
#RPC_HTTP_1=https://commented.example
KEEP_ORDER=1
ENV
chmod 600 "$TARGET"

FRAG="$WORK/fragment.env"
printf '%s\n' \
  'AAVE_POOL_1=0xnew' \
  'RPC_HTTP_1=alchemy=https://eth.example/v2?key=abc&x=1' \
  'RPC_HTTP_1=drpc=https://eth.drpc.org' \
  'BRAND_NEW_KEY=hello' > "$FRAG"

before_sha="$(sha256sum "$TARGET" | cut -d' ' -f1)"
before_ino="$(stat -c %i "$TARGET")"
before_mode="$(stat -c %a "$TARGET")"

# --- run 1 --------------------------------------------------------------------
if bash "$UPSERT" "$TARGET" "$FRAG" > "$WORK/run1.log" 2>&1; then
  check "run 1 exits 0" "0" "0"
else
  check "run 1 exits 0" "0" "1"
fi

line_of() { grep -n "^$1=" "$TARGET" | head -n1 | cut -d: -f1; }

check "managed key replaced in its ORIGINAL position (line 6)" "6" "$(line_of AAVE_POOL_1)"
check "replaced value is the fragment's" "AAVE_POOL_1=0xnew" "$(sed -n "$(line_of AAVE_POOL_1)p" "$TARGET")"
check "header comment preserved byte-identical" "# managed by arbx-env-deploy — do not edit by hand" "$(sed -n 1p "$TARGET")"
check "unmanaged key untouched" "POSTGRES_PASSWORD=old-pass" "$(grep '^POSTGRES_PASSWORD=' "$TARGET")"
check "blank line preserved (line 4)" "" "$(sed -n 4p "$TARGET")"
check "commented key NOT clobbered" "#RPC_HTTP_1=https://commented.example" "$(grep '^#RPC_HTTP_1=' "$TARGET")"
check "commented key got an ACTIVE duplicate appended, once" "1" "$(grep -c '^RPC_HTTP_1=' "$TARGET")"
check "duplicate fragment key: last value wins" "RPC_HTTP_1=drpc=https://eth.drpc.org" "$(grep '^RPC_HTTP_1=' "$TARGET")"
check "value with '=' and '&' survives literally" "RPC_HTTP_1=drpc=https://eth.drpc.org" "$(grep '^RPC_HTTP_1=' "$TARGET")"
check "new key appended at the end" "BRAND_NEW_KEY=hello" "$(tail -n1 "$TARGET")"
check "no CR anywhere in the output" "0" "$(tr -cd '\r' < "$TARGET" | wc -c | tr -d ' ')"
check "file mode preserved" "$before_mode" "$(stat -c %a "$TARGET")"
check "inode preserved (in-place write)" "$before_ino" "$(stat -c %i "$TARGET")"

# --- run 2: idempotence -------------------------------------------------------
after1_sha="$(sha256sum "$TARGET" | cut -d' ' -f1)"
bash "$UPSERT" "$TARGET" "$FRAG" > "$WORK/run2.log" 2>&1
after2_sha="$(sha256sum "$TARGET" | cut -d' ' -f1)"
check "re-run is byte-identical (idempotent)" "$after1_sha" "$after2_sha"
check "re-run changed something (sanity: run 1 did)" "different" \
  "$([ "$before_sha" = "$after1_sha" ] && echo same || echo different)"

# --- CRLF fragment ------------------------------------------------------------
CRLF_TARGET="$WORK/crlf.env"
printf 'A=1\nB=2\n' > "$CRLF_TARGET"
printf 'B=two\r\nC=three\r\n' > "$WORK/crlf.fragment"
bash "$UPSERT" "$CRLF_TARGET" "$WORK/crlf.fragment" > /dev/null 2>&1
check "CRLF fragment: replaced+appended, 3 lines" "3" "$(grep -c . "$CRLF_TARGET")"

printf '\n%d/%d checks passed' "$pass" "$total"
if [ "$fail" -ne 0 ]; then printf ' — %d FAILED\n' "$fail"; exit 1; fi
printf '\n'
