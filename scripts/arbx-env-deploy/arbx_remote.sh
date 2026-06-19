#!/usr/bin/env bash
# Remote runner (executes ON the VPS). Keeps all bash control-flow (&&/||/pipes)
# on the Linux side so the Windows PowerShell orchestrator never has to parse them.
#   Usage: arbx_remote.sh <dryrun|apply> <env_path>
# Expects /tmp/arbx_fragment.env and /tmp/arbx_env_upsert.sh already scp'd.
# Self-cleans the secret-bearing fragment on exit (trap).
set -euo pipefail

mode="${1:?mode (dryrun|apply) required}"
envp="${2:?env path required}"
frag="/tmp/arbx_env_fragment.env"
ups="/tmp/arbx_env_upsert.sh"

cleanup() { shred -u "$frag" 2>/dev/null || rm -f "$frag"; rm -f "$ups" /tmp/arbx_remote.sh; }
trap cleanup EXIT

[ -f "$envp" ] || { echo "ENV not found: $envp" >&2; exit 3; }
[ -f "$frag" ] || { echo "fragment not uploaded: $frag" >&2; exit 3; }
[ -f "$ups" ]  || { echo "upsert not uploaded: $ups" >&2; exit 3; }

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
    bash "$ups" "$envp" "$frag" --backup
    chmod 600 "$envp"
    echo ".env updated in place (timestamped .bak_* written on the VPS)"
    cd "$(dirname "$envp")"
    # compose.prod.yml interpolates ${GITHUB_TOKEN}; compose's default project env is docker/.env
    # which lacks it -> a harmless 'not set' warning on stderr. Export it (from the real .env if
    # present, else blank) so compose finds it and stays silent. Runtime identical (was blank anyway).
    export GITHUB_TOKEN="$(grep -m1 '^GITHUB_TOKEN=' "$envp" 2>/dev/null | cut -d= -f2- || true)"
    # up -d --force-recreate (NOT restart): `docker compose restart` keeps the container's
    # original create-time environment, so .env changes never reach the process. Recreate
    # reloads env_file/.env into the new container. --no-deps avoids touching dependencies.
    docker compose -f docker/compose.prod.yml up -d --force-recreate --no-deps searcher-rs api-server
    sleep 15
    dc="docker compose -f docker/compose.prod.yml"
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
