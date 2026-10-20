#!/usr/bin/env bash
# Remote runner (executes ON the VPS). Keeps all bash control-flow (&&/||/pipes)
# on the Linux side so the Windows PowerShell orchestrator never has to parse them.
#   Usage: arbx_remote.sh <dryrun|apply> <env_path>
# Expects /tmp/arbx_fragment.env, /tmp/arbx_env_upsert.sh and /tmp/deploy_identity.sh
# already shipped (scp or base64); see README.
# Self-cleans the secret-bearing fragment on exit (trap).
set -euo pipefail
# TRANSPORT-HYGIENE-01 (2026-10-19): the fragment carries live secrets and lands
# in a world-readable /tmp. Everything this process creates is 0600 from here on,
# and the fragment is tightened BEFORE it is ever read by the upsert.
umask 077

mode="${1:?mode (dryrun|apply) required}"
envp="${2:?env path required}"
frag="/tmp/arbx_env_fragment.env"
ups="/tmp/arbx_env_upsert.sh"
# DEPLOY-IDENTITY-RECREATE-01: the reader that resolves the identity of the
# binary a container serves. Uploaded next to this script (see README); the
# repo-relative path is used when the script runs from a checkout (CI tests).
idcli_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || echo /tmp)"
idcli="$idcli_dir/deploy_identity.sh"
[ -f "$idcli" ] || idcli="/tmp/deploy_identity.sh"

cleanup() { shred -u "$frag" 2>/dev/null || rm -f "$frag"; rm -f "$ups" /tmp/arbx_remote.sh /tmp/deploy_identity.sh; }
trap cleanup EXIT

[ -f "$envp" ] || { echo "ENV not found: $envp" >&2; exit 3; }
[ -f "$frag" ] || { echo "fragment not uploaded: $frag" >&2; exit 3; }
chmod 600 "$frag" 2>/dev/null || true   # tighten before anything reads it
[ -f "$ups" ]  || { echo "upsert not uploaded: $ups" >&2; exit 3; }

# DEPLOY-IDENTITY-RECREATE-01 (card #4, 2026-10-19): a recreate is not a deploy.
# `docker compose up --force-recreate` keeps the same binary but rebuilds the
# container environment from .env + the SHELL env; ARBX_DEPLOY_SHA/_ID/_AT live
# only in the deploy's shell, so without this the recreate publishes
# `/api/status.deploy.sha = unknown` while serving an unchanged binary.
# The identity is READ from what is running (image label, else its env) and
# exported before the recreate; environment values injected by a workflow win.
# Nothing is fabricated: no valid source => compose keeps its honest `unknown`.
stamp_deploy_identity() {
  local dir="${1:-.}" cid out s i a
  if [ -n "${ARBX_DEPLOY_SHA:-}" ] && [ "${ARBX_DEPLOY_SHA}" != "unknown" ]; then
    echo "deploy identity: inherited from environment (sha=$ARBX_DEPLOY_SHA)"
    return 0
  fi
  if [ ! -f "$idcli" ]; then
    echo "WARN: $idcli missing — recreate leaves deploy.sha at compose default (unknown)" >&2
    return 0
  fi
  cid="$( cd "$dir" && docker compose -f docker/compose.prod.yml ps -q api-server 2>/dev/null || true )"
  cid="$(printf '%s' "$cid" | head -1)"
  if [ -z "$cid" ]; then
    # compose cannot always be asked: `ps` interpolates the same variables the
    # apply may have just broken (that was the #982 defect). The container
    # itself is the durable source, so fall back to it by name.
    cid="$(docker ps -a --filter 'name=api-server' --format '{{.ID}}' 2>/dev/null | head -1 || true)"
  fi
  if [ -z "$cid" ]; then
    echo "deploy identity: no api-server container to read; compose defaults apply" >&2
    return 0
  fi
  out="$(bash "$idcli" "$cid" 2>/dev/null || true)"
  s="$(printf '%s\n' "$out" | sed -n '1p')"
  i="$(printf '%s\n' "$out" | sed -n '2p')"
  a="$(printf '%s\n' "$out" | sed -n '3p')"
  if [ -n "$s" ]; then
    export ARBX_DEPLOY_SHA="$s"
    export ARBX_DEPLOY_ID="${i:-unknown}"
    export ARBX_DEPLOYED_AT="${a:-unknown}"
    echo "deploy identity preserved across recreate: sha=$s id=${i:-unknown} at=${a:-unknown}"
  else
    echo "WARN: no deploy identity derivable from the running api-server; /status reports unknown" >&2
  fi
}

case "$mode" in
  dryrun)
    cand="$(mktemp)"
    cp "$envp" "$cand"
    bash "$ups" "$cand" "$frag" >/dev/null
    echo "----- DIFF (current .env -> after upsert) -----"
    diff -u "$envp" "$cand" || true
    echo "----- (DRY-RUN: real .env UNCHANGED) -----"
    rm -f "$cand"
    ;;
  apply)
    # CONCURRENCY (2026-10-19): two sources (Excel macro + frontend) can call
    # apply at the same time. Without a lock the two upserts interleave and the
    # resulting .env is neither one fragment nor the other. flock -n fails CLEAN
    # instead of queueing: a second apply is an operator error, not a retry.
    LOCK="${TMPDIR:-/tmp}/arbx_env_apply.lock"
    exec 9>"$LOCK"
    flock -n 9 || { echo "another apply is in progress (lock $LOCK); refusing to interleave" >&2; exit 7; }

    # ROLLBACK (2026-10-19): a backup that is never restored is not a rollback.
    # The pre-apply bytes are kept aside and put back if the recreate or the
    # service check fails, so a bad fragment can never leave the stack worse
    # than it was found.
    bak="${envp}.rollback.$(date +%Y%m%d_%H%M%S)"
    cp -p "$envp" "$bak"
    before_sha="$(sha256sum "$envp" | cut -d' ' -f1)"
    restore() {
      echo "ROLLBACK: restoring $envp from $bak and recreating" >&2
      cat "$bak" > "$envp"
      chmod 600 "$envp"
      # The rollback recreates the same binary: its identity must survive too.
      stamp_deploy_identity "$(dirname "$envp")"
      ( cd "$(dirname "$envp")" && docker compose -f docker/compose.prod.yml up -d --force-recreate --no-deps searcher-rs api-server ) >&2 || true
    }

    bash "$ups" "$envp" "$frag" --backup
    chmod 600 "$envp"
    echo ".env updated in place (timestamped .bak_* written on the VPS; rollback copy: $bak)"
    cd "$(dirname "$envp")"
    # compose.prod.yml interpolates ${GITHUB_TOKEN}; compose's default project env is docker/.env
    # which lacks it -> a harmless 'not set' warning on stderr. Export it (from the real .env if
    # present, else blank) so compose finds it and stays silent. Runtime identical (was blank anyway).
    export GITHUB_TOKEN="$(grep -m1 '^GITHUB_TOKEN=' "$envp" 2>/dev/null | cut -d= -f2- || true)"
    # Identity BEFORE the recreate: read from the live container, not invented.
    stamp_deploy_identity "$(dirname "$envp")"
    # up -d --force-recreate (NOT restart): `docker compose restart` keeps the container's
    # original create-time environment, so .env changes never reach the process. Recreate
    # reloads env_file/.env into the new container. --no-deps avoids touching dependencies.
    if ! docker compose -f docker/compose.prod.yml up -d --force-recreate --no-deps searcher-rs api-server; then
      restore; exit 1
    fi
    sleep 15
    dc="docker compose -f docker/compose.prod.yml"
    # VERIFY: both services must be RUNNING before the apply may claim success.
    running="$($dc ps --services --filter status=running 2>/dev/null | sort | tr '\n' ' ')"
    case "$running" in
      *searcher-rs*api-server* | *api-server*searcher-rs*) echo "verify ok: running = [$running]" ;;
      *) echo "VERIFY FAILED: running services = [$running]" >&2; restore; exit 1 ;;
    esac
    # APPLY RECEIPT (2026-10-19): this flow does NOT pass through git, so the
    # only durable trace of "what changed and when" is written here, next to the
    # .env: timestamps + sha256 before/after + the rollback copy. No values, no
    # secrets — key names only, so the file is safe to read and to share.
    receipt="${envp}.apply.log"
    printf '%s before=%s after=%s rollback=%s\n' \
      "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$before_sha" "$(sha256sum "$envp" | cut -d' ' -f1)" "$(basename "$bak")" \
      >> "$receipt"
    chmod 600 "$receipt" 2>/dev/null || true
    echo "receipt appended: $receipt"
    echo "--- searcher-rs ---"; $dc logs searcher-rs --tail 25 2>&1 | grep -E 'scanner.subscribed|scanner.no_rpc|ERROR' || true
    echo "--- api-server ---";  $dc logs api-server  --tail 15 2>&1 | grep -E 'SECURE_BOOT|listening|error' || true
    ;;
  safediff)
    # Transcript-safe dry-run: report each fragment key as ADD/CHANGE/SAME by NAME
    # only (compares values internally, never prints any value). Real .env unchanged.
    add=0; chg=0; same=0
    echo "----- DRY-RUN SUMMARY (key names only, NO values) -----"
    while IFS= read -r line || [ -n "$line" ]; do
      line="${line%$'\r'}"
      [ -z "${line//[[:space:]]/}" ] && continue
      [ "${line:0:1}" = "#" ] && continue
      [ "$line" = "${line#*=}" ] && continue
      k="${line%%=*}"; v="${line#*=}"
      cur="$(grep -m1 -E "^${k}=" "$envp" 2>/dev/null || true)"
      if [ -z "$cur" ]; then echo "  ADD     $k"; add=$((add+1))
      elif [ "${cur#*=}" = "$v" ]; then echo "  SAME    $k"; same=$((same+1))
      else echo "  CHANGE  $k"; chg=$((chg+1)); fi
    done < "$frag"
    echo "----- totals: ADD=$add CHANGE=$chg SAME=$same (real .env UNCHANGED) -----"
    ;;
  *) echo "unknown mode: $mode" >&2; exit 2 ;;
esac
