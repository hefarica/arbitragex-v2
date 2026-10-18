#!/usr/bin/env bash
# G-SEC-HEADERS-01 — the security-header SET is decided once, and consistently.
#
# Origin (2026-10-18): EDGE-CSP-SINGLE-01 fixed ONE header (nginx stopped
# emitting CSP) but left nginx emitting the rest of the set, so the same defect
# reappeared on the NEXT header: the edge response carried `x-xss-protection`
# TWICE and contradictory (Cloudflare `0` + nginx `1; mode=block`). The lesson is
# about the CLASS, not the header: one emitter per header, and the gate must
# check the whole set.
#
# This gate is STATIC (repo configs; it cannot SSH the VPS). It fails on:
#   1. a security header declared with CONTRADICTING values by two emitters;
#   2. X-XSS-Protection != "0" (legacy, superseded by CSP — `1; mode=block` is a
#      no-op in current browsers and contradicts the value Cloudflare emits);
#   3. nginx declaring a Content-Security-Policy (G-EDGE-CSP-SINGLE-01: the CSP
#      is owned by the application layer).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

# Every nginx config fragment that can declare a response header.
mapfile -t FILES < <(find infra/nginx frontend config/nginx -type f \( -name '*.conf' -o -name 'nginx.conf' \) 2>/dev/null | sort -u)
if [ "${#FILES[@]}" -eq 0 ]; then
  echo "G-SEC-HEADERS-01 — SKIP: no nginx config files found"; exit 2
fi

HEADERS=(X-Frame-Options X-Content-Type-Options X-XSS-Protection Strict-Transport-Security Referrer-Policy Permissions-Policy Content-Security-Policy)

rc=0
tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
# Collect: header <TAB> value <TAB> file
for f in "${FILES[@]}"; do
  grep -hEo '^[[:space:]]*add_header[[:space:]]+[A-Za-z0-9-]+[[:space:]]+"[^"]*"' "$f" 2>/dev/null | \
    sed -E 's/^[[:space:]]*add_header[[:space:]]+([A-Za-z0-9-]+)[[:space:]]+"([^"]*)"/\1\t\2/' | \
    while IFS=$'\t' read -r h v; do printf '%s\t%s\t%s\n' "$h" "$v" "$f"; done >> "$tmp"
done

for H in "${HEADERS[@]}"; do
  # distinct values for this header
  vals=$(awk -F'\t' -v h="$H" '$1==h{print $2}' "$tmp" | sort -u)
  nvals=$(printf '%s\n' "$vals" | grep -c . || true)
  [ "$nvals" -eq 0 ] && continue
  if [ "$nvals" -gt 1 ]; then
    echo "  FAIL $H is declared with CONTRADICTING values:"
    awk -F'\t' -v h="$H" '$1==h{printf "        %s  (%s)\n",$2,$3}' "$tmp" | sort -u
    rc=1
  fi
  if [ "$H" = "Content-Security-Policy" ]; then
    echo "  FAIL nginx declares a CSP (G-EDGE-CSP-SINGLE-01: the CSP is app-owned):"
    awk -F'\t' '$1=="Content-Security-Policy"{printf "        %s  (%s)\n",$2,$3}' "$tmp" | sort -u
    rc=1
  fi
  if [ "$H" = "X-XSS-Protection" ]; then
    bad=$(printf '%s\n' "$vals" | grep -v '^0$' || true)
    if [ -n "$bad" ]; then
      echo "  FAIL X-XSS-Protection must be \"0\" (legacy, superseded by CSP); found: $bad"
      rc=1
    fi
  fi
done

echo "  static set: $(awk -F'\t' '{printf "%s=%s ",$1,$2}' "$tmp" | sort -u | tr '\n' ' ')"
if [ "$rc" = "0" ]; then
  echo "G-SEC-HEADERS-01 — PASS (set consistent + no nginx CSP + X-XSS-Protection: 0)"
  exit 0
else
  echo "G-SEC-HEADERS-01 — FAIL (ver arriba)"
  exit 1
fi
