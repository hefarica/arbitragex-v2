#!/usr/bin/env bash
# G-SEC-HEADERS-01 — the security-header set is decided once, and it is what the
# host ACTUALLY SERVES.
#
# Origin (2026-10-18): EDGE-CSP-SINGLE-01 fixed ONE header (nginx stopped emitting
# the CSP) but left nginx emitting the rest of the set, so the same defect
# reappeared on the NEXT header: the edge response carries `x-xss-protection`
# TWICE and contradictory (`0` from Cloudflare + `1; mode=block` from nginx).
#
# A gate that only reads the repo's .conf files is BORN FALSE-GREEN: it is green
# while the host serves something else — the exact state today (the live
# `referrer-policy: no-referrer` matches no .conf in the repo). So this gate:
#
#   A. reads the live response of each host and COUNTS duplicates per header
#      (presence is not enough: the defect is a second, contradicting emitter);
#   B. compares the live value of each header to the set the repo declares, and
#      flags host headers the repo does not own (the repo must be a SUPERSET);
#   C. keeps the static repo consistency check (one value per header,
#      X-XSS-Protection == 0, no nginx-owned CSP).
#
# Exit 0 = PASS, 1 = FAIL, 2 = SKIP (no live host reachable / curl absent).
# Live probing is mandatory: a gate that cannot measure is never reported green.
# Set SEC_HEADERS_HOSTS="host1 host2" to override; SEC_HEADERS_PATH to change the
# probed path.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

command -v curl >/dev/null 2>&1 || { echo "G-SEC-HEADERS-01 — SKIP: curl ausente"; exit 2; }

HOSTS="${SEC_HEADERS_HOSTS:-https://arbx.ape-tv.net https://edge-arbx.ape-tv.net}"
PROBE_PATH="${SEC_HEADERS_PATH:-/api/status}"
HEADERS=(x-frame-options x-content-type-options x-xss-protection strict-transport-security
         referrer-policy permissions-policy content-security-policy content-security-policy-report-only)

rc=0
tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT

# ---- C. static repo set -----------------------------------------------------
mapfile -t FILES < <(find infra/nginx frontend config/nginx -type f \( -name '*.conf' -o -name 'nginx.conf' \) 2>/dev/null | sort -u)
for f in "${FILES[@]}"; do
  grep -hEo '^[[:space:]]*add_header[[:space:]]+[A-Za-z0-9-]+[[:space:]]+"[^"]*"' "$f" 2>/dev/null | \
    sed -E 's/^[[:space:]]*add_header[[:space:]]+([A-Za-z0-9-]+)[[:space:]]+"([^"]*)"/\1\t\2/' | \
    while IFS=$'\t' read -r h v; do printf '%s\t%s\t%s\n' "$(echo "$h" | tr 'A-Z' 'a-z')" "$v" "$f"; done >> "$tmp"
done
declare -A EXPECT
for h in "${HEADERS[@]}"; do
  vals=$(awk -F'\t' -v h="$h" '$1==h{print $2}' "$tmp" | sort -u)
  n=$(printf '%s\n' "$vals" | grep -c . || true)
  [ "$n" -eq 0 ] && continue
  if [ "$n" -gt 1 ]; then
    echo "  FAIL [$h] the repo declares CONTRADICTING values:"; awk -F'\t' -v h="$h" '$1==h{printf "        %s  (%s)\n",$2,$3}' "$tmp" | sort -u; rc=1
  fi
  EXPECT["$h"]="$(printf '%s\n' "$vals" | head -1)"
done
[ -n "${EXPECT[content-security-policy]:-}" ] && { echo "  FAIL nginx declares a CSP (G-EDGE-CSP-SINGLE-01: app-owned)"; rc=1; }
if [ -n "${EXPECT[x-xss-protection]:-}" ] && [ "${EXPECT[x-xss-protection]}" != "0" ]; then
  echo "  FAIL [x-xss-protection] the repo must declare \"0\" (legacy, superseded by CSP); got ${EXPECT[x-xss-protection]}"; rc=1
fi

# ---- A/B. live probe, per host ---------------------------------------------
probed=0
for host in $HOSTS; do
  hdr="$(mktemp)"
  if ! curl -sD - -o /dev/null -m 20 -A "curl/8" -H 'Accept: application/json' "$host$PROBE_PATH" > "$hdr" 2>/dev/null || [ ! -s "$hdr" ]; then
    echo "  FAIL [$host] live probe failed (no response) — cannot verify the served set"; rc=1; rm -f "$hdr"; continue
  fi
  probed=$((probed+1))
  echo "  -- $host$PROBE_PATH"
  for h in "${HEADERS[@]}"; do
    mapfile -t live < <(awk -v h="$h" 'BEGIN{IGNORECASE=1} index(tolower($0), h":")==1 { sub(/\r$/,""); sub(/^[^:]*:[[:space:]]*/,""); print }' "$hdr")
    n="${#live[@]}"
    [ "$n" -eq 0 ] && continue
    if [ "$n" -gt 1 ]; then
      echo "     FAIL [$h] served $n TIMES (duplicate emitters):"
      for v in "${live[@]}"; do echo "           $v"; done
      rc=1
    fi
    # The CSP is APP-OWNED by doctrine (G-EDGE-CSP-SINGLE-01): the repo must NOT
    # declare it, so neither its absence from the repo set nor its value is drift.
    if [ "$h" = "content-security-policy" ] || [ "$h" = "content-security-policy-report-only" ]; then
      continue
    fi
    exp="${EXPECT[$h]:-}"
    if [ -z "$exp" ]; then
      echo "     FAIL [$h] served by the host but NOT declared in the repo (repo must be a superset): ${live[0]}"
      rc=1
    elif [ "${live[0]}" != "$exp" ]; then
      echo "     FAIL [$h] served value differs from the repo: host=\"${live[0]}\" repo=\"$exp\""
      rc=1
    fi
  done
  rm -f "$hdr"
done
[ "$probed" -eq 0 ] && { echo "G-SEC-HEADERS-01 — SKIP: ningún host respondió (no se puede medir)"; exit 2; }

if [ "$rc" = "0" ]; then
  echo "G-SEC-HEADERS-01 — PASS (single emitter per header, values match the repo, on every host)"
  exit 0
fi
echo "G-SEC-HEADERS-01 — FAIL (ver arriba)"
exit 1
