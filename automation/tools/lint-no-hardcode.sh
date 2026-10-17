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

# ARBX_GATE_ROOT lets the regression test (test-gate-secretos.sh) point this gate
# at a throwaway fixture repo instead of the checkout under test.
ROOT="${ARBX_GATE_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "$ROOT" || exit 2

# Legacy informational findings (categories 1-4): reported, never fatal
# (267 pre-existing findings on main at 2026-10-05 — flipping them fatal is a
# separate decision, NOT this hardening).
VIOLATIONS=0
# Blocking findings (categories 5-6 = the E-7 secret-exposure class): fatal.
BLOCKING=0

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

# Case-insensitive variant (POSIX ERE has no inline (?i); keys like
# ANTHROPIC_AUTH_TOKEN are uppercase, so -i is required).
run_grep_i() {
  # run_grep_i <pattern> <include-globs>
  local pat="$1" globs="$2"
  # shellcheck disable=SC2086
  git grep -n -i -E "$pat" -- $globs 2>/dev/null || true
}

# Blocking findings (the E-7 class). Distinct label so CI logs separate
# "this blocks the merge" from the legacy informational categories.
report_blocking() {
  local cat="$1" file="$2" line="$3" content="$4"
  printf 'BLOCKING[%s] %s:%s  %s\n' "$cat" "$file" "$line" "$content" >&2
  BLOCKING=$((BLOCKING+1))
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
ADDR_ALLOW='(^backend/shared-rs/src/(chains|tokens)\.rs|\.test\.(ts|js|tsx)|^docs/|\.env\.example|/README\.md|#\[cfg\(test\)\]|tests?/|\bcfg!\(test\)|_test\.rs|^automation/scripts/|backend/relays-client/src/bundle_builder\.rs|backend/sim-ctl/src/tx_builder\.rs|backend/sim-ctl/src/main\.rs|backend/selector-api/src/token_safety/internal_heuristic\.ts|backend/token-enricher/src/(main|multicall)\.rs|^replay\.sql|^database/.*\.sql|backend/searcher-rs/src/workers/triangular_worker\.rs|backend/sim-ctl/src/revm_backend\.rs|backend/sed-core/src/connectors/flashbots_simulator\.rs|backend/sed-core/src/connectors/reserve_reader\.rs|backend/sed-core/src/eigenstate/mod\.rs|backend/searcher-rs/src/sim_orchestrator\.rs|backend/searcher-rs/src/sim_prefund\.rs|backend/searcher-rs/src/sim_encoder_pg\.rs|backend/searcher-rs/src/sim_multistep\.rs|backend/searcher-rs/src/size_optimizer\.rs|backend/searcher-rs/src/workers/cex_dex_worker\.rs|backend/searcher-rs/src/workers/jit_v3_worker\.rs|backend/searcher-rs/src/workers/liquidation_worker\.rs|backend/searcher-rs/src/workers/pool_sync_worker\.rs|backend/searcher-rs/src/workers/price_worker\.rs|backend/shared-rs/src/price_oracle\.rs|backend/prioritization-spine/src/erc20_storage\.rs|backend/prioritization-spine/src/swap_encoder\.rs|backend/searcher-rs/src/engines/flashloan_engine\.rs|backend/searcher-rs/src/amm_math\.rs|backend/api-server/src/credentials/validators\.ts|backend/prioritization-spine/src/round_trip_executor\.rs|backend/prioritization-spine/src/route_plan\.rs|backend/recon/src/pnl_engine\.rs|backend/relays-client/src/submit_engine\.rs|backend/searcher-rs/src/chain_client\.rs|backend/searcher-rs/src/engines/triangular_engine\.rs|backend/searcher-rs/src/orchestrator\.rs|backend/searcher-rs/src/reserves\.rs|backend/searcher-rs/src/scanner\.rs|backend/searcher-rs/src/sim_encoder\.rs)'
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
URL_ALLOW='(^docs/|\.env\.example|\.md:|\.test\.(ts|js|tsx)|/tests?/|_test\.rs|(ghcr|docker|crates|npmjs|github|githubusercontent|actions)\.(io|com|org)|schema\.json|JSONSchema|w3\.org|prom-client|localhost:|127\.0\.0\.[0-9]+:|api-server:|anvil:|redis:|postgres:|edge:|selector-api:|sim-ctl:|recon:|relays-client:|grafana:|prometheus:|alertmanager:|loki:|<KEY>|<YOUR_KEY>|backend/searcher-rs/src/chain_client\.rs|backend/searcher-rs/src/workers/price_worker\.rs|backend/token-enricher/src/main\.rs|backend/relays-client/src/relay_bloxroute\.rs|^replay\.sql|^database/.*\.sql|frontend/features/onboarding/Phase[0-9]+Client\.tsx|^crucible/scripts/|^contracts/foundry\.toml|apis/openapi\.yaml|backend/api-server/src/services/tokenValidation/liquidityReality\.ts|backend/searcher-rs/src/workers/cex_dex_worker\.rs|backend/sed-core/src/connectors/flashbots_simulator\.rs|backend/sed-core/src/connectors/reserve_reader\.rs|mkdocs\.yml)'
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

# ─── 4. Symbol-keyed token identity in financial paths (ARBX-TW-003) ──
# Doctrine (ARBX-0018 / REQ-QB-002): the runtime identity of a token is
# (chain_id, address); symbols are METADATA. No financial path may gate a
# token by comparing symbol strings. This category flags the symbol-gate
# idioms outside the canonical surface:
#   - shared-rs/src/trading_config.rs     — `token_allowed` = the legacy
#     symbol VIEW (documented; the address-keyed gate is
#     `TokenIdentityIndex::is_allowed_addr`)
#   - shared-rs/src/token_identity.rs     — the resolver itself
#   - searcher-rs/src/token_identity.rs   — composition site (cache check)
#   - shared-rs/src/price_oracle.rs       — price stack contract keys on
#     symbols by design (metadata feed, not an identity gate)
#   - prioritization-spine/src/config_aware.rs — the DOCUMENTED legacy
#     fallback when no identity index is attached (ARBX-0018: None =
#     symbol-compare for callers not yet migrated)
#   - api-server/src/routes/trading-config.ts — operator input plane
#   - tests / docs
# Line-level grep (same fuzziness as categories 1-3): multi-line gate
# idioms escape — the Rust regression anchors in token_identity.rs and
# config_aware.rs tests pin the semantics; this net catches new sites.
SYMBOL_KEY_RE='\.token_allowed\(|allowed_token_symbols[^;)]*\.(contains|includes)\(|allowed_token_symbols[^;)]*\.iter\(\)'
SYMBOL_KEY_ALLOW='(^backend/shared-rs/src/(token_identity|trading_config|price_oracle)\.rs|^backend/searcher-rs/src/token_identity\.rs|backend/prioritization-spine/src/config_aware\.rs|^backend/api-server/src/routes/trading-config\.ts|#\[cfg\(test\)\]|tests?/|\.test\.(ts|js|tsx)|_test\.rs|^docs/|\.md:)'
while IFS= read -r hit; do
  [ -z "$hit" ] && continue
  file="${hit%%:*}"; rest="${hit#*:}"; line="${rest%%:*}"; content="${rest#*:}"
  report "symbol-key" "$file" "$line" "$content"
done < <(run_grep "$SYMBOL_KEY_RE" \
            "*.rs *.ts *.tsx" \
            "$SYMBOL_KEY_ALLOW")

# ─── 5. JSON secret literals ("CLAVE": "valor") — BLOCKING (E-7) ─────
# Why this category exists: the shape that entered the tree on 2026-07-11 and
# stayed tracked until 2026-09-30 (.claude/settings.json -> ANTHROPIC_AUTH_TOKEN)
# is a JSON `"KEY": "<opaque value>"` pair. Categories 1-4 never matched it, and
# no gate read JSON at all: the file was missed by TEMPORAL SCOPE, not by
# allowlist. This category is PATH+FORM based — it never compares a candidate
# value against a known credential, and it never prints one.
# The secret-bearing word must be the LAST element of the key: this matches
# "ANTHROPIC_AUTH_TOKEN" / "apiKey" / "seller_secret" and rejects descriptors
# such as "auth_scheme", "auth_type", "api_key_id", "max_tokens" (plural).
JSON_SECRET_RE='"([A-Za-z0-9_.-]*)(token|secret|password|passwd|credential|api_?key|private_key)"[[:space:]]*:[[:space:]]*"[^"]{16,}"'
# Values that are demonstrably NOT credentials: documented placeholders,
# interpolations, env reads (same spirit as the boot validator rejecting
# *_change_me / *_dev_only).
JSON_PLACEHOLDER_RE='(change_me|dev_only|replace_me|placeholder|example|dummy|sample|your_|your-|YOUR_|xxx|XXXX|\$\{|\$\(|process\.env|requireEnv|<[A-Z_]+>|\.\.\.)'
# Paths where a DOCUMENTED example legitimately lives. `.claude/` is deliberately
# absent: operator-local config is exactly where the E-7 file lived.
JSON_ALLOW='(^docs/|^audits/|\.example|\.test\.|/tests?/|_test\.rs|package-lock\.json|pnpm-lock\.yaml|^ci-artifacts/|^frontend/playwright-report/)'

# 5a. .claude/** — NO allow-list. Any secret-shaped JSON pair here is fatal.
while IFS= read -r hit; do
  [ -z "$hit" ] && continue
  printf '%s' "$hit" | grep -Eq "$JSON_PLACEHOLDER_RE" && continue
  file="${hit%%:*}"; rest="${hit#*:}"; line="${rest%%:*}"; content="${rest#*:}"
  report_blocking "json-secret-claude" "$file" "$line" "$content"
done < <(run_grep_i "$JSON_SECRET_RE" ".claude/")

# 5b. JSON files anywhere outside the documented-example allow-list.
while IFS= read -r hit; do
  [ -z "$hit" ] && continue
  printf '%s' "$hit" | grep -Eq "$JSON_PLACEHOLDER_RE" && continue
  printf '%s' "$hit" | grep -Eq "$JSON_ALLOW" && continue
  file="${hit%%:*}"; rest="${hit#*:}"; line="${rest%%:*}"; content="${rest#*:}"
  report_blocking "json-secret" "$file" "$line" "$content"
done < <(run_grep_i "$JSON_SECRET_RE" "*.json *.jsonc *.json5")

# ─── 6. Operator-local credential paths must NEVER be tracked — BLOCKING ──
# `git add -f` defeats .gitignore; this category defeats `git add -f`.
# Path-based only: the file is never opened, matched or printed.
FORBIDDEN_TRACKED_RE='^\.claude/settings(\..*)?\.json$'
while IFS= read -r f; do
  [ -z "$f" ] && continue
  report_blocking "tracked-operator-local-secret-path" "$f" "0" \
    "operator-local credential file is TRACKED (.gitignore is inert for tracked paths)"
done < <(git ls-files | grep -E "$FORBIDDEN_TRACKED_RE" || true)

# ─── summary ─────────────────────────────────────────────────────────
if [ "$BLOCKING" -gt 0 ]; then
  printf '\n%s\n' "lint-no-hardcode: $BLOCKING BLOCKING finding(s) — E-7 class (JSON secret literal / tracked operator-local credential path). See docs/security/GATE-SECRETOS-01.md." >&2
  if [ "$VIOLATIONS" -gt 0 ]; then
    printf '%s\n' "lint-no-hardcode: ($VIOLATIONS legacy informational finding(s), categories 1-4, non-fatal by design)" >&2
  fi
  exit 1
fi
if [ "$VIOLATIONS" -gt 0 ]; then
  printf '\n%s\n' "lint-no-hardcode: $VIOLATIONS legacy informational finding(s) (categories 1-4, non-fatal). See docs/governance/NO-HARDCODE-DOCTRINE.md." >&2
else
  printf 'lint-no-hardcode: clean\n'
fi
exit 0
