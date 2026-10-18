#!/usr/bin/env bash
# G-IMG-ROLLBACK-01 — image-level rollback + deploy-environment gate.
#
# WHY: on 2026-10-18 the read-only VPS census (audit-vps-wiring, run
# 39143097918) measured **0 image tags matching pre-/backup/rollback** on
# production. The deploy rebuilt every service with `--no-cache` (re-pointing
# `:latest` and dangling the previous release, which the disk guard then
# pruned), so the only way back was a git reset + a ~30-minute rebuild —
# exactly when the stack may be down. This gate pins the three rules that
# close that hole, so a later edit cannot silently drop them:
#
#   G-IMG-ROLLBACK-01  the deploy captures the running images as rollback
#                      points, in a namespace the disk guard cannot prune.
#   G-DEPLOY-ENV-01    the deploy job runs in the GitHub `production`
#                      environment (deployment record + documented brake).
#   G-ROLLBACK-EXEC-01 the one-step restore exists and never builds.
#
# It checks ORDER in line order (never in a comment) and EXECUTES the capture
# block under `set -euo pipefail` against a mocked docker, because the FIRST
# production run of this mechanism is the EMPTY state (no generations exist):
# a static grep cannot prove that the empty state does not abort the deploy.
#
# Exit 0 = PASS, exit 1 = FAIL. Every check prints an explicit line.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

WF=".github/workflows/auto-deploy-vps.yml"
RB="scripts/deploy/rollback-images.sh"

fail=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; fail=1; fi; }
has() { if grep -qF -- "$2" "$1" 2>/dev/null; then echo 0; else echo 1; fi; }
# line number of the first NON-COMMENT line matching a regex, or empty.
lineno() { sed 's/#.*//' "$1" | grep -nE -- "$2" | head -1 | cut -d: -f1; }

say "G-IMG-ROLLBACK-01 — image rollback + deploy-environment gate"
say "repo root: $ROOT"
say ""

say "1) the files exist"
chk "$WF present" "$([ -f "$WF" ] && echo 0 || echo 1)"
chk "$RB present" "$([ -f "$RB" ] && echo 0 || echo 1)"
chk "$RB is executable" "$([ -x "$RB" ] && echo 0 || echo 1)"

say ""
say "2) G-IMG-ROLLBACK-01 — capture, namespace, keepers, bounded retention"
chk "capture step present" "$(has "$WF" 'Capturing image rollback points')"
chk "tags the arbx-rollback namespace" "$(has "$WF" 'arbx-rollback/${svc}:${ROLLBACK_GEN}')"
chk "keeper created by plain 'docker create' (not compose)" "$(has "$WF" 'docker create \')"
chk "keeper carries the cleanup label arbx.rollback=1" "$(has "$WF" '--label arbx.rollback=1')"
chk "keeper never carries a compose project label" \
  "$(grep -q 'com.docker.compose.project' "$WF" && echo 1 || echo 0)"
chk "retention is bounded (ROLLBACK_KEEP)" "$(has "$WF" 'ROLLBACK_KEEP="${ROLLBACK_KEEP:-')"

say ""
say "3) ORDER, in line order and not in a comment"
CAP_LN="$(lineno "$WF" 'Capturing image rollback points')"
PRUNE_LN="$(lineno "$WF" 'docker image prune -af')"
BUILD_LN="$(lineno "$WF" '\[6/9\] Building core')"
say "     capture line=${CAP_LN:-?}  first 'docker image prune -af' line=${PRUNE_LN:-?}  build line=${BUILD_LN:-?}"
chk "capture found / prune found / build found" \
  "$([ -n "$CAP_LN" ] && [ -n "$PRUNE_LN" ] && [ -n "$BUILD_LN" ] && echo 0 || echo 1)"
chk "capture is STRICTLY before the first 'docker image prune -af'" \
  "$([ -n "$CAP_LN" ] && [ -n "$PRUNE_LN" ] && [ "$CAP_LN" -lt "$PRUNE_LN" ] && echo 0 || echo 1)"
chk "capture is STRICTLY before the build" \
  "$([ -n "$CAP_LN" ] && [ -n "$BUILD_LN" ] && [ "$CAP_LN" -lt "$BUILD_LN" ] && echo 0 || echo 1)"

say ""
say "4) the disk guard was NOT weakened (prune still present)"
chk "builder prune -af still present" "$(has "$WF" 'docker builder prune -af')"
chk "image prune -af still present" "$(has "$WF" 'docker image prune -af')"

say ""
say "5) G-DEPLOY-ENV-01 — production environment on the deploy job"
chk "environment declared" "$(has "$WF" 'environment:')"
chk "  its name is production" "$(has "$WF" 'name: production')"

say ""
say "6) G-ROLLBACK-EXEC-01 — the one-step restore exists and never builds"
chk "restore uses --no-build (a build would overwrite the rollback image)" "$(has "$RB" 'up -d --no-build --force-recreate')"
chk "restore retags back to :latest" "$(has "$RB" 'arbitragex-v2-${svc}:latest')"
chk "restore is fail-closed on a missing generation" "$(has "$RB" 'refusing (nothing to restore)')"
chk "restore verifies /api/health" "$(has "$RB" 'api/health')"
chk "restore offers a documented cleanup path (--clean)" "$(has "$RB" '--clean) CLEAN=1')"
chk "restore refuses to touch git/build" \
  "$(sed 's/#.*//' "$RB" | grep -qE '(^|[^-a-z])git (reset|checkout|pull|fetch)|docker compose [^|]*[^-a-zA-Z]build([^-a-zA-Z]|$)' && echo 1 || echo 0)"

say ""
say "7) the EMPTY state does not abort the deploy (executed, not grepped)"
# Extract the capture block verbatim and run it under the SAME shell options the
# remote deploy uses (set -euo pipefail) with a mocked docker. The first
# production run is this exact state: no generations yet.
BLOCK="/tmp/g-img-rollback-01.block.sh"
awk '
  /# ═══ G-IMG-ROLLBACK-01/ {p=1}
  p { print }
  p && /^ *# ═+/ && !/# ═══ G-IMG-ROLLBACK-01/ { exit }
' "$WF" | sed 's/^          //' > "$BLOCK"
chk "capture block extracted ($(grep -c . "$BLOCK" || true) lines)" \
  "$([ "$(grep -c . "$BLOCK" || true)" -gt 20 ] && echo 0 || echo 1)"

run_block() { # $1 = mock docker function body file
  local out="$1" h="/tmp/g-img-rollback-01.harness.sh"
  { echo '#!/usr/bin/env bash'; echo 'set -euo pipefail'
    echo 'docker() {'
    echo '  case "$*" in'
    cat "$out"
    echo '  esac; return 0; }'
    cat "$BLOCK"
    echo 'echo GATE_BLOCK_OK'
  } > "$h"
  bash "$h" 2>&1
}

MOCK_EMPTY="/tmp/g-img-rollback-01.mock.empty"
cat > "$MOCK_EMPTY" <<'MOCK'
    "ps -q"*) : ;;
    "inspect -f"*) : ;;
    "tag"*) : ;;
    "create --name"*) : ;;
    "ps -a --filter"*) : ;;
    "image ls"*) : ;;
    "rm -f"*) : ;;
    "image rm"*) : ;;
MOCK
OUT_EMPTY="$(run_block "$MOCK_EMPTY" || true)"
chk "empty state (first prod run) exits 0" \
  "$(printf '%s' "$OUT_EMPTY" | grep -q 'GATE_BLOCK_OK' && echo 0 || echo 1)"
chk "empty state reports 0 captured and 0 kept" \
  "$(printf '%s' "$OUT_EMPTY" | grep -q 'captured: 0' && printf '%s' "$OUT_EMPTY" | grep -q 'generations kept: 0' && echo 0 || echo 1)"

MOCK_GENS="/tmp/g-img-rollback-01.mock.gens"
cat > "$MOCK_GENS" <<'MOCK'
    "ps -q"*) echo "abc123" ;;
    "inspect -f"*) echo "sha256:deadbeef" ;;
    "tag"*) : ;;
    "create --name"*) : ;;
    "ps -a --filter"*) printf 'arbx-rollback-api-server-20261018T090000Z\narbx-rollback-edge-20261018T090000Z\narbx-rollback-api-server-20261018T100000Z\narbx-rollback-edge-20261018T100000Z\narbx-rollback-api-server-20261018T110000Z\narbx-rollback-edge-20261018T110000Z\narbx-rollback-api-server-20261018T120000Z\narbx-rollback-edge-20261018T120000Z\n' ;;
    "image ls"*) printf 'arbx-rollback/api-server:20261018T090000Z\narbx-rollback/edge:20261018T090000Z\n' ;;
    "rm -f"*) echo "RELEASED $*" ;;
    "image rm"*) echo "IMAGE_RM $*" ;;
MOCK
OUT_GENS="$(run_block "$MOCK_GENS" || true)"
chk "4 generations + keep=3 exits 0" \
  "$(printf '%s' "$OUT_GENS" | grep -q 'GATE_BLOCK_OK' && echo 0 || echo 1)"
chk "4 generations + keep=3 releases the OLDEST only" \
  "$(printf '%s' "$OUT_GENS" | grep -q 'releasing rollback gen 20261018T090000Z' \
     && ! printf '%s' "$OUT_GENS" | grep -qE 'releasing rollback gen 20261018T1[012]0000Z' && echo 0 || echo 1)"
chk "the newest 3 generations are kept" \
  "$(printf '%s' "$OUT_GENS" | grep -q 'generations kept: 3' && echo 0 || echo 1)"

say ""
if [ "$fail" -eq 0 ]; then
  say "G-IMG-ROLLBACK-01: PASS"
else
  say "G-IMG-ROLLBACK-01: FAIL"
fi
exit "$fail"
