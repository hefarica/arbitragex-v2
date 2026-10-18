#!/usr/bin/env bash
# G-EDGE-CSP-SINGLE-01 — exactly ONE Content-Security-Policy header at the edge,
# and no loopback literal inside it.
#
# WHY: the edge path (Cloudflare → host nginx → edge worker) emitted TWO
# contradictory CSP headers that INTERSECT (the browser enforces the most
# restrictive union — they do not override). Header #1 was the edge worker's
# JSON policy `default-src 'none'; frame-ancestors 'none'`. Header #2 was a
# stale static nginx policy carrying `connect-src 'self' ws: wss:
# http://127.0.0.1:8080 http://127.0.0.1:8787` — loopback endpoints leaked into
# a production response (observed on https://edge-arbx.ape-tv.net/, 2026-10-18).
#
# WHAT THIS GATE PINS
#   1. LIVE   — `<edge>/` returns EXACTLY ONE `content-security-policy` header.
#   2. LIVE   — 0 occurrences of `127.0.0.1` in the response headers (loopback
#               must never appear in a public response), unless --allow-localhost
#               is passed WITH a justification string (recorded in the output).
#   3. REPO   — no nginx config tracked in the repo declares an `add_header
#               Content-Security-Policy`: CSP is owned by the app layer (edge
#               worker for the JSON API, Next headers() for HTML), never nginx.
#
# Usage:
#   scripts/gates/g-edge-csp-single-01.sh [--url URL] [--repo-only]
#                                         [--allow-localhost "reason"]
# Exit 0 = PASS, exit 1 = FAIL, exit 2 = usage error.
set -u

EDGE_URL="https://edge-arbx.ape-tv.net/"
REPO_ONLY=0
ALLOW_LOCALHOST=""
while [ $# -gt 0 ]; do
  case "$1" in
    --url) EDGE_URL="${2:-}"; shift 2;;
    --repo-only) REPO_ONLY=1; shift;;
    --allow-localhost) ALLOW_LOCALHOST="${2:-}"; shift 2;;
    -h|--help) sed -n '2,30p' "$0"; exit 0;;
    *) echo "unknown arg: $1" >&2; exit 2;;
  esac
done

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
say() { printf '%s\n' "$*"; }
chk() { if [ "$2" = "0" ]; then say "  PASS  $1"; else say "  FAIL  $1"; fail=1; fi; }

say "G-EDGE-CSP-SINGLE-01 — single CSP header at the edge"
say "  info  url=${EDGE_URL}"

# --- 3. repo layer: nginx must not declare a CSP ---------------------------
# Anchored to the start of a directive (leading whitespace, then `add_header`)
# so the contract comments in the .conf files do not self-trigger.
repo_csp="$(git grep -n -i -E '^[[:space:]]*add_header[[:space:]]+Content-Security-Policy' -- '*.conf' 2>/dev/null || true)"
if [ -z "$repo_csp" ]; then
  say "  PASS  no nginx config in the repo emits Content-Security-Policy"
else
  say "  FAIL  nginx config still emits Content-Security-Policy (single-owner violation):"
  printf '%s\n' "$repo_csp" | sed 's/^/        /'
  fail=1
fi

if [ "$REPO_ONLY" = "1" ]; then
  say "  info  --repo-only: skipping live probe"
  [ "$fail" = "0" ] && say "PASS — G-EDGE-CSP-SINGLE-01 (repo)" || say "FAIL — G-EDGE-CSP-SINGLE-01 (repo)"
  exit "$fail"
fi

# --- 1 + 2. live layer -----------------------------------------------------
hdrs="$(curl -sD - -o /dev/null --max-time 20 "$EDGE_URL" 2>/dev/null || true)"
if [ -z "$hdrs" ]; then
  say "  FAIL  could not reach ${EDGE_URL} — control NOT TESTED (never a clean pass)"
  exit 1
fi

csp_count="$(printf '%s\n' "$hdrs" | grep -ci '^content-security-policy:')"
chk "exactly one content-security-policy header (got ${csp_count})" \
    "$([ "$csp_count" = "1" ] && echo 0 || echo 1)"

lo_count="$(printf '%s\n' "$hdrs" | grep -o '127\.0\.0\.1' | wc -l | tr -d ' ')"
if [ "$lo_count" = "0" ]; then
  say "  PASS  0 occurrences of 127.0.0.1 in response headers"
elif [ -n "$ALLOW_LOCALHOST" ]; then
  say "  WAIVED  ${lo_count} occurrence(s) of 127.0.0.1 — justification: ${ALLOW_LOCALHOST}"
else
  say "  FAIL  ${lo_count} occurrence(s) of 127.0.0.1 in response headers (loopback in prod)"
  fail=1
fi

say "  info  CSP headers observed:"
printf '%s\n' "$hdrs" | grep -i '^content-security-policy:' | sed 's/^/        /'
if [ "$fail" = "0" ]; then say "PASS — G-EDGE-CSP-SINGLE-01"; else say "FAIL — G-EDGE-CSP-SINGLE-01"; fi
exit "$fail"
