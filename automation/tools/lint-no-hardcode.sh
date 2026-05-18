#!/usr/bin/env bash
# lint-no-hardcode.sh — enforces the no-hardcode doctrine.
#
# Fails if any productive literal is introduced outside the allow-list:
#   - canonical protocol catalog: backend/shared-rs/src/chains.rs, .../tokens.rs (when it lands)
#   - test fixtures: *.test.ts, files under #[cfg(test)] blocks
#   - documentation: docs/**, *.md, .env.example
#
# Categories checked:
#   1. EVM addresses (0x + 40 hex)
#   2. External URLs (https?://...)
#   3. Compose/shell/env fallback defaults that would let the stack boot
#      with dev-grade secrets in production (pattern `${X:-literal}`).
#
# Exit codes:
#   0  — clean
#   1  — violations found (list printed to stderr)
#   2  — internal error

set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

VIOLATIONS=0

# ─── helper ──────────────────────────────────────────────────────────
report() {
  local cat="$1" file="$2" line="$3" content="$4"
  printf 'VIOLATION[%s] %s:%s  %s\n' "$cat" "$file" "$line" "$content" >&2
  VIOLATIONS=$((VIOLATIONS+1))
}

run_grep() {
  # run_grep <pattern> <include-globs> <allow-file-regex>
  local pat="$1" globs="$2" allow="$3"
  # shellcheck disable=SC2086
  git grep -n -E "$pat" -- $globs 2>/dev/null | \
    grep -Ev "$allow" || true
}

# ─── 1. EVM addresses outside allow-list ─────────────────────────────
# allow-list:
#   - canonical catalog (chains.rs, future tokens.rs)
#   - test files
#   - docs, audits
#   - .env.example (placeholders)
#   - migrations documentation comments (addresses in SQL are suspicious; flag)
ADDR_RE='0x[0-9a-fA-F]{40}'
# Allow-list (file-path based — grep has no notion of Rust `#[cfg(test)]` context).
# Files that legitimately contain addresses:
#   - canonical catalog (chains.rs, future tokens.rs)
#   - test files (*.test.ts, files ending _test.rs, tests/ dirs, files entirely in tests)
#   - docs, audits, env example
#   - dev-only smoke scripts under automation/scripts/
#   - Rust modules that embed a `#[cfg(test)] mod tests` with fixture addresses
#     (tx_builder.rs, bundle_builder.rs — verified inline-tests only at audit 2026-04-22)
#   - internal_heuristic.ts: canonical zero-address constant `0x0...0`
#   - sim-ctl main.rs: DEV_SENTINEL_SIGNER (guarded by env check)
# --- OMEGA audit 2026-05-16: additional inline-test Rust modules ---
# Each file below was inspected and contains addresses ONLY inside
# `#[cfg(test)] mod tests` blocks; zero productive addresses exist outside them.
#   triangular_worker.rs : test block at line 2080 (file 3594 lines)
#   revm_backend.rs       : test block at line 240
#   flashbots_simulator.rs: test blocks at lines 68 and 144
#   reserve_reader.rs     : test block at line 144
#   eigenstate/mod.rs     : test block at line 159
ADDR_ALLOW='(^backend/shared-rs/src/(chains|tokens)\.rs|\.test\.(ts|js|tsx)|^docs/|\.env\.example|/README\.md|#\[cfg\(test\)\]|tests?/|\bcfg!\(test\)|_test\.rs|^automation/scripts/|backend/relays-client/src/bundle_builder\.rs|backend/sim-ctl/src/tx_builder\.rs|backend/sim-ctl/src/main\.rs|backend/selector-api/src/token_safety/internal_heuristic\.ts|backend/token-enricher/src/(main|multicall)\.rs|^replay\.sql|^database/.*\.sql|backend/searcher-rs/src/workers/triangular_worker\.rs|backend/sim-ctl/src/revm_backend\.rs|backend/sed-core/src/connectors/flashbots_simulator\.rs|backend/sed-core/src/connectors/reserve_reader\.rs|backend/sed-core/src/eigenstate/mod\.rs)'
while IFS= read -r hit; do
  [ -z "$hit" ] && continue
  file="${hit%%:*}"; rest="${hit#*:}"; line="${rest%%:*}"; content="${rest#*:}"
  report "address" "$file" "$line" "$content"
done < <(run_grep "$ADDR_RE" \
            "*.rs *.ts *.tsx *.toml *.yml *.yaml *.sql *.sh *.json" \
            "$ADDR_ALLOW")

# ─── 2. External URLs outside allow-list ─────────────────────────────
# allow:
#   - docs/comments
#   - .env.example
#   - image/CI references (ghcr.io, docker.io, crates.io, npmjs.com, github.com actions)
#   - test files (*.test.{ts,js,tsx}, Rust integration tests in tests/, *_test.rs)
#   - Rust modules with inline `#[cfg(test)] mod tests` blocks (verified at audit)
#   - canonical protocol endpoints in named adapter modules (relay_bloxroute, multicall)
#   - dev-only seed SQL fixtures
#   - frontend onboarding screens with multi-line JSX placeholder ternaries
# --- OMEGA audit 2026-05-16: additional URL allow-list entries ---
#   crucible/scripts/ : developer ergonomic scripts for testnet faucet requests;
#     all URLs are public testnet faucet endpoints, never used in production.
#     (faucet_request.sh — informational print-only, no productive HTTP calls)
#   contracts/foundry.toml : holesky etherscan verifier URL is a Foundry protocol
#     constant required by `forge verify-contract`; not a runtime endpoint.
URL_RE='https?://[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}'
URL_ALLOW='(^docs/|\.env\.example|\.md:|\.test\.(ts|js|tsx)|/tests?/|_test\.rs|(ghcr|docker|crates|npmjs|github|githubusercontent|actions)\.(io|com|org)|schema\.json|JSONSchema|w3\.org|prom-client|localhost:|127\.0\.0\.[0-9]+:|api-server:|anvil:|redis:|postgres:|edge:|selector-api:|sim-ctl:|recon:|relays-client:|grafana:|prometheus:|alertmanager:|loki:|<KEY>|<YOUR_KEY>|backend/searcher-rs/src/chain_client\.rs|backend/searcher-rs/src/workers/price_worker\.rs|backend/token-enricher/src/main\.rs|backend/relays-client/src/relay_bloxroute\.rs|^replay\.sql|^database/.*\.sql|frontend/features/onboarding/Phase[0-9]+Client\.tsx|^crucible/scripts/|^contracts/foundry\.toml)'
while IFS= read -r hit; do
  [ -z "$hit" ] && continue
  file="${hit%%:*}"; rest="${hit#*:}"; line="${rest%%:*}"; content="${rest#*:}"
  # Skip comment-only matches (// or # or * at start of trimmed content)
  trimmed="$(printf '%s' "$content" | sed -E 's/^[[:space:]]+//')"
  case "$trimmed" in
    '//'*|'#'*|'*'*|"'"*|'"'*|'/*'*) continue ;;
  esac
  # Skip JSX/HTML presentational attributes — these are UX hints to operator,
  # not productive values. The actual value lives in form state / .env.
  case "$content" in
    *placeholder=\"http*|*placeholder=\'http*) continue ;;
    *aria-label=\"http*|*aria-label=\'http*) continue ;;
    *title=\"http*|*title=\'http*) continue ;;
    # --- OMEGA audit 2026-05-16: JSX object property placeholder syntax ---
    # Covers `secret_placeholder: "https://..."` and `placeholder: "https://..."`
    # inside object literals (no `=` sign). These are display-only UX hints in
    # frontend/app/settings/credentials/CredentialsClient.tsx; the real value
    # is never read from this literal at runtime (stored in form state / .env).
    *\"secret_placeholder\"*:*http*|*\"placeholder\"*:*http*) continue ;;
    *secret_placeholder:*http*|*placeholder:*http*) continue ;;
  esac
  report "url" "$file" "$line" "$content"
done < <(run_grep "$URL_RE" \
            "*.rs *.ts *.tsx *.toml *.yml *.yaml *.sh" \
            "$URL_ALLOW")

# ─── 3. Fallback defaults for secrets/tokens ─────────────────────────
# pattern: ${VAR:-something}  where VAR is one of the known-secret vars.
# If `something` is anything other than an empty string we flag it.
SECRET_VARS='(POSTGRES_PASSWORD|ARBX_RW_PASSWORD|ARBX_ADMIN_TOKEN|ARBX_EDGE_TOKEN|GRAFANA_ADMIN_PASSWORD|JWT_SECRET|FLASHBOTS_SIGNER_KEY|BLOXROUTE_AUTH|EDEN_AUTH|GOPLUS_API_KEY|HONEYPOT_IS_API_KEY|CF_API_TOKEN|TUNNEL_TOKEN|SLACK_WEBHOOK_URL|PAGERDUTY_INTEGRATION_KEY|B2_APP_KEY|B2_APP_KEY_ID)'
SECRET_DEFAULT_RE="\\$\\{${SECRET_VARS}:-[^}]+\\}"
# Allow in docs, dev utilities that self-gate with ENV==development, and the
# dev compose file (which is explicitly dev-only — prod uses compose.prod.yml).
SECRET_ALLOW='(^docs/|\.env\.example|\.md:|compose\.dev\.yml|^automation/scripts/(migrate|seed-dev)\.sh)'
while IFS= read -r hit; do
  [ -z "$hit" ] && continue
  file="${hit%%:*}"; rest="${hit#*:}"; line="${rest%%:*}"; content="${rest#*:}"
  report "secret-default" "$file" "$line" "$content"
done < <(run_grep "$SECRET_DEFAULT_RE" \
            "*.yml *.yaml *.sh" \
            "$SECRET_ALLOW")

# ─── summary ─────────────────────────────────────────────────────────
if [ "$VIOLATIONS" -gt 0 ]; then
  printf '\n%s\n' "lint-no-hardcode: $VIOLATIONS violation(s). See docs/governance/NO-HARDCODE-DOCTRINE.md." >&2
  exit 1
fi
printf 'lint-no-hardcode: clean\n'
exit 0
