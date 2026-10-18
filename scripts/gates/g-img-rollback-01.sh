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

say "G-IMG-ROLLBACK-01 — image rollback + deploy-environment gate"
say "repo root: $ROOT"
say ""

say "1) workflow exists and is the auto-deploy one"
chk "$WF present" "$([ -f "$WF" ] && echo 0 || echo 1)"
chk "$RB present" "$([ -f "$RB" ] && echo 0 || echo 1)"
chk "$RB is executable" "$([ -x "$RB" ] && echo 0 || echo 1)"

say ""
say "2) G-IMG-ROLLBACK-01 — capture before build, in a prune-proof namespace"
chk "capture step [5b/9] present" "$(has "$WF" '[5b/9] Capturing image rollback points')"
chk "tags the arbx-rollback namespace" "$(has "$WF" 'arbx-rollback/${svc}:${ROLLBACK_GEN}')"
chk "creates a keeper container (image becomes unprunable)" "$(has "$WF" 'docker create --name "arbx-rollback-')"
chk "retention is bounded (ROLLBACK_KEEP)" "$(has "$WF" 'ROLLBACK_KEEP="${ROLLBACK_KEEP:-')"
chk "capture runs BEFORE the build step" \
  "$(awk '/\[5b\/9\] Capturing image rollback points/{c=NR} /\[6\/9\] Building core/{b=NR} END{ if (c && b && c < b) print 0; else print 1 }' "$WF")"

say ""
say "3) the disk guard was NOT weakened (prune still present)"
chk "builder prune -af still present" "$(has "$WF" 'docker builder prune -af')"
chk "image prune -af still present" "$(has "$WF" 'docker image prune -af')"

say ""
say "4) G-DEPLOY-ENV-01 — production environment on the deploy job"
chk "environment: production declared" "$(has "$WF" 'environment:')"
chk "  its name is production" "$(has "$WF" 'name: production')"

say ""
say "5) G-ROLLBACK-EXEC-01 — the one-step restore exists and never builds"
chk "restore uses --no-build (a build would overwrite the rollback image)" "$(has "$RB" 'up -d --no-build --force-recreate')"
chk "restore retags back to :latest" "$(has "$RB" 'arbitragex-v2-${svc}:latest')"
chk "restore is fail-closed on a missing generation" "$(has "$RB" 'refusing (nothing to restore)')"
chk "restore verifies /api/health" "$(has "$RB" 'api/health')"
chk "restore refuses to touch git/build" \
  "$(sed 's/#.*//' "$RB" | grep -qE '(^|[^-a-z])git (reset|checkout|pull|fetch)|docker compose [^|]*[^-a-zA-Z]build([^-a-zA-Z]|$)' && echo 1 || echo 0)"

say ""
if [ "$fail" -eq 0 ]; then
  say "G-IMG-ROLLBACK-01: PASS"
else
  say "G-IMG-ROLLBACK-01: FAIL"
fi
exit "$fail"
