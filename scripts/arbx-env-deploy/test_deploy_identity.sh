#!/usr/bin/env bash
# Contract tests for deploy_identity.sh + its call site in arbx_remote.sh —
# runnable anywhere, no VPS, no docker (a fake `docker` on PATH).
#
# WHY THIS FILE EXISTS (2026-10-19, card #4): a manual recreate
# (`arbx_remote.sh apply`, and its rollback) rebuilt the api-server container
# without ARBX_DEPLOY_SHA/_ID/_DEPLOYED_AT, so `/api/status.deploy.sha` fell to
# `unknown` while the binary served was unchanged. The fix reads the identity
# from what is running. These are its controls:
#   1. the image label (the binary's build sha) WINS when valid;
#   2. a pre-label image falls back to the container env sha;
#   3. a valid label never borrows id/at from a DIFFERENT deploy's env;
#   4. `unknown`/garbage is never emitted as a sha (fail-closed to empty);
#   5. the recreate path calls the reader BEFORE `up --force-recreate`;
#   6. shell-quoting/syntax of both scripts.
#
# Usage: bash scripts/arbx-env-deploy/test_deploy_identity.sh
# Exit 0 = all pass; 1 = at least one failed (the failing check is named).
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ID="$HERE/deploy_identity.sh"
REMOTE="$HERE/arbx_remote.sh"
[ -f "$ID" ]     || { echo "FATAL: $ID not found" >&2; exit 2; }
[ -f "$REMOTE" ] || { echo "FATAL: $REMOTE not found" >&2; exit 2; }

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

# ── fake docker: serves the two fixtures below ────────────────────────────────
mkdir -p "$WORK/bin"
cat > "$WORK/bin/docker" <<'FAKE'
#!/usr/bin/env bash
fmt=""; ref=""
while [ $# -gt 0 ]; do
  case "$1" in
    -f|--format) fmt="$2"; shift 2 ;;
    inspect) shift ;;
    *) ref="$1"; shift ;;
  esac
done
case "$fmt" in
  *".Config.Env"*) cat "${FAKE_ENV:-/dev/null}" 2>/dev/null || true ;;
  *".Image"*)      printf 'sha256:feedface\n' ;;
  *".Config.Labels"*) cat "${FAKE_LABEL:-/dev/null}" 2>/dev/null || true ;;
  *) : ;;
esac
FAKE
chmod +x "$WORK/bin/docker"
export PATH="$WORK/bin:$PATH"

ENVF="$WORK/env"; LABELF="$WORK/label"
: > "$ENVF"; : > "$LABELF"
export FAKE_ENV="$ENVF" FAKE_LABEL="$LABELF"

run() { # run => "sha|id|at" (stderr dropped; the three output lines joined)
  bash "$ID" fake-container 2>/dev/null | paste -sd'|' -
}

# 1. label wins, triple is complete
printf '%s\n' 'ARBX_DEPLOY_SHA=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' 'ARBX_DEPLOY_ID=4242' 'ARBX_DEPLOYED_AT=2026-10-19T12:44:51Z' > "$ENVF"
printf '%s\n' 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' > "$LABELF"
check "image label wins over container env" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|4242|2026-10-19T12:44:51Z" "$(run)"

# 2. no label (pre-label image) => env sha is used
: > "$LABELF"
printf '%s\n' 'ARBX_DEPLOY_SHA=bbbbbbb' 'ARBX_DEPLOY_ID=4242' 'ARBX_DEPLOYED_AT=2026-10-19T12:44:51Z' > "$ENVF"
check "pre-label image falls back to container env" \
  "bbbbbbb|4242|2026-10-19T12:44:51Z" "$(run)"

# 3. label present but env belongs to another deploy => sha only, no mixed triple
printf '%s\n' 'ccccccc' > "$LABELF"
check "valid label never borrows id/at from another sha" \
  "ccccccc||" "$(run)"

# 4. honest unknown: nothing valid => empty, and literally never 'unknown'
: > "$LABELF"
printf '%s\n' 'ARBX_DEPLOY_SHA=unknown' 'ARBX_DEPLOY_ID=unknown' 'ARBX_DEPLOYED_AT=unknown' > "$ENVF"
out="$(run)"
check "unknown is not emitted as a sha" "||" "$out"

# 5. no container reference => empty triple, exit 0
total=$((total + 1))
if bash "$ID" '' 2>/dev/null | grep -qx 'deploy_identity: sha source=none' ; then
  : # stderr line, never on stdout
fi
if [ "$(bash "$ID" '' 2>/dev/null | wc -l)" -eq 3 ]; then
  pass=$((pass + 1)); printf '  ok    no container reference yields an empty triple\n'
else
  fail=$((fail + 1)); printf '  FAIL  no container reference yields an empty triple (got %q)\n' "$out"
fi

# 6. call site: the stamp runs BEFORE each --force-recreate in arbx_remote.sh
total=$((total + 1))
missing=0
while IFS= read -r upline; do
  found=0
  for back in $(seq 1 12); do
    ln=$((upline - back))
    [ "$ln" -ge 1 ] || continue
    if sed -n "${ln}p" "$REMOTE" | grep -q 'stamp_deploy_identity'; then found=1; break; fi
  done
  [ "$found" = 1 ] || missing=$((missing + 1))
done < <(grep -n 'up -d --force-recreate' "$REMOTE" | grep -v ':[[:space:]]*#' | cut -d: -f1)
ups_seen="$(grep -n 'up -d --force-recreate' "$REMOTE" | grep -v ':[[:space:]]*#' | wc -l)"
if [ "$missing" = 0 ] && [ "$ups_seen" -ge 2 ]; then
  pass=$((pass + 1)); printf '  ok    identity is stamped before every recreate (%s sites)\n' "$ups_seen"
else
  fail=$((fail + 1)); printf '  FAIL  %s of %s recreate site(s) lack a preceding stamp call\n' "$missing" "$ups_seen"
fi

# 7. syntax of both scripts
total=$((total + 1))
if bash -n "$ID" && bash -n "$REMOTE"; then
  pass=$((pass + 1)); printf '  ok    bash -n deploy_identity.sh + arbx_remote.sh\n'
else
  fail=$((fail + 1)); printf '  FAIL  bash -n\n'
fi

printf '\n%s/%s checks passed\n' "$pass" "$total"
[ "$fail" -eq 0 ] || exit 1
