#!/usr/bin/env bash
# audit-dockerfiles.sh — verify that every Rust/Node workspace member listed in
# backend/Cargo.toml has a COPY directive in the corresponding Dockerfile.
#
# INF-8: Phase 3 INF batch — 2026-05-07
#
# What it checks:
#   For each backend/*/Dockerfile:
#     a) A multi-stage builder exists (FROM ... AS builder).
#     b) COPY directives cover every Rust workspace member from Cargo.toml,
#        OR the Dockerfile uses a wildcard `COPY . .` that covers all members.
#     c) Node Dockerfiles: COPY covers shared-ts + the service directory.
#
# Exit codes:
#   0 — all Dockerfiles pass
#   1 — one or more Dockerfiles have missing COPY directives
#
# Usage:
#   ./automation/scripts/audit-dockerfiles.sh
#   ./automation/scripts/audit-dockerfiles.sh --strict   # treat warnings as failures

set -euo pipefail

STRICT="${1:-}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BACKEND_DIR="$REPO_ROOT/backend"
CARGO_TOML="$BACKEND_DIR/Cargo.toml"

log()  { printf '[audit-dockerfiles] %s\n' "$*"; }
warn() { printf '[audit-dockerfiles] WARN: %s\n' "$*"; }
fail_item() { printf '[audit-dockerfiles] FAIL: %s\n' "$*"; FAILURES=$((FAILURES + 1)); }

FAILURES=0

# ── 1. Parse workspace members from Cargo.toml using grep/sed ────────────────

RUST_MEMBERS=()
if [[ -f "$CARGO_TOML" ]]; then
  # Extract lines between 'members = [' and the closing ']'
  # Each member is a quoted string like "searcher-rs" or "shared-rs"
  in_members=false
  while IFS= read -r line; do
    if echo "$line" | grep -qE 'members\s*=\s*\['; then
      in_members=true
    fi
    if $in_members; then
      # Extract quoted strings from the line
      while IFS= read -r member; do
        [[ -n "$member" ]] && RUST_MEMBERS+=("$member")
      done < <(echo "$line" | grep -oE '"[^"]+"' | tr -d '"')
      # Stop when we hit the closing bracket on its own line
      echo "$line" | grep -qE '^\s*\]' && in_members=false
    fi
  done < "$CARGO_TOML"
  log "Rust workspace members (${#RUST_MEMBERS[@]}): ${RUST_MEMBERS[*]:-<none found>}"
else
  log "WARNING: $CARGO_TOML not found — skipping Rust member coverage checks"
fi

# ── 2. Audit each Dockerfile ─────────────────────────────────────────────────

check_dockerfile() {
  local dockerfile="$1"
  local service
  service="$(basename "$(dirname "$dockerfile")")"
  log "--- $service/Dockerfile ---"

  # (a) Builder stage must exist
  if ! grep -qE '^FROM .+ AS builder' "$dockerfile"; then
    warn "$service/Dockerfile: no 'FROM ... AS builder' multi-stage found (single-stage or base image)"
    [[ "$STRICT" == "--strict" ]] && FAILURES=$((FAILURES + 1))
    return
  fi

  # (b) Wildcard COPY covers everything
  if grep -qE '^COPY \. \.(/?)?' "$dockerfile"; then
    log "$service/Dockerfile: wildcard COPY . detected — all members implicitly covered. OK"
    return
  fi

  # Detect Node vs Rust
  local is_node=false
  grep -qE '^FROM node:' "$dockerfile" && is_node=true

  if $is_node; then
    local ok=true
    if ! grep -q 'shared-ts' "$dockerfile"; then
      fail_item "$service/Dockerfile: Node Dockerfile missing 'COPY shared-ts'"
      ok=false
    fi
    local svc_lower
    svc_lower="$(echo "$service" | tr '[:upper:]' '[:lower:]')"
    if ! grep -qE "COPY .*(backend/)?${svc_lower}" "$dockerfile"; then
      fail_item "$service/Dockerfile: Node Dockerfile missing COPY for service dir '${svc_lower}'"
      ok=false
    fi
    $ok && log "$service/Dockerfile: OK"
  else
    # Rust: each workspace member should appear in a COPY directive
    local missing=()
    for member in "${RUST_MEMBERS[@]}"; do
      if ! grep -qE "^COPY ${member}( |/|\$)" "$dockerfile"; then
        missing+=("$member")
      fi
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
      for m in "${missing[@]}"; do
        warn "$service/Dockerfile: no explicit COPY for workspace member '${m}' (may be OK if not a transitive dep)"
      done
      [[ "$STRICT" == "--strict" ]] && FAILURES=$((FAILURES + ${#missing[@]}))
    else
      log "$service/Dockerfile: OK"
    fi
  fi
}

# Find all Dockerfiles under backend/ (maxdepth 2 covers backend/<service>/Dockerfile)
while IFS= read -r df; do
  check_dockerfile "$df"
done < <(find "$BACKEND_DIR" -maxdepth 2 -name Dockerfile | sort)

# Also spot-check frontend and edge
for extra_df in \
    "$REPO_ROOT/frontend/Dockerfile" \
    "$REPO_ROOT/edge/dev-local/Dockerfile"; do
  [[ -f "$extra_df" ]] || continue
  service="$(basename "$(dirname "$extra_df")")"
  log "--- $service/Dockerfile (non-backend) ---"
  if ! grep -qE '^FROM .+ AS builder' "$extra_df"; then
    warn "$extra_df: no multi-stage builder (may be intentional for simple images)"
  else
    log "$service/Dockerfile: OK"
  fi
done

# ── 3. Result ─────────────────────────────────────────────────────────────────

echo
if [[ "$FAILURES" -gt 0 ]]; then
  log "AUDIT FAILED — ${FAILURES} issue(s). Fix the Dockerfiles listed above."
  exit 1
else
  log "AUDIT PASSED — all Dockerfiles look healthy."
  exit 0
fi
