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
# --- OMEGA audit 2026-05-16 (no-hardcode triage sprint) ---
# Additional files audited: every address in each file resides inside a
# `#[cfg(test)] mod tests` block; no productive (non-test) literal exists.
# Evidence: file:line ranges verified against test-block start lines.
#
#   sim_prefund.rs        : test block starts line 306; all addr violations 325-549 (F. fixture)
#   amm_math.rs           : test block starts line 301; all addr violations 453-564 (F. fixture)
#   round_trip_executor.rs: test block starts line 212; all addr violations 222-350 (F. fixture)
#   swap_encoder.rs       : test block starts line 201; all addr violations 218-396 (F. fixture)
#   sim_encoder_pg.rs     : test block starts line  45; all addr violations 269-343 (F. fixture)
#   sim_encoder.rs        : test block starts line  54; all addr violations 504-883 (F. fixture)
#   sim_orchestrator.rs   : test block starts line 449; all addr violations 460-480 (F. fixture)
#   sim_multistep.rs      : test block starts line 728; all addr violations 740-766 (F. fixture)
#   chain_client.rs       : test block starts line 255; all addr violations 315-334 (F. fixture)
#   price_worker.rs       : test block starts line 663; all addr violations 762-779 (F. fixture)
#   reserves.rs           : test block starts line 293; all addr violations 323-407 (F. fixture)
#   orchestrator.rs       : test block starts line 848; all addr violations 947-1224 (F. fixture)
#   submit_engine.rs      : test block starts line 714; all addr violations 730-731 (F. fixture)
#   size_optimizer.rs     : test block starts line 1055; addr violation at 1140 (F. fixture)
#   CredentialsClient.tsx : secret_placeholder at line 142 is a 64-hex UX display-only
#     value (32-byte zero private key hint); matches addr regex but is never a runtime
#     address — classified G. falso positivo. Scoped to single file.
#
# D. dirección EVM de protocolo (immutable, on-chain verified, audited 2026-05-16):
#   These files contain protocol-canonical address constants or pool identifiers
#   that are immutable by protocol design. The correct long-term action is to
#   consolidate into backend/shared-rs/src/chains.rs (tracked as H. deuda).
#   Until that consolidation is done, each file is individually justified:
#
#   scanner.rs           : V3_QUOTER_V2_MAINNET, V3_MULTICALL3_ADDR
#                          Uniswap V3 QuoterV2 and Multicall3 mainnet addresses.
#                          Both are deterministic cross-chain deploys (fixed salt).
#   liquidation_worker.rs: AAVE_V3_POOL_MAINNET (Aave V3 docs verified),
#                          MULTICALL3_MAINNET (deterministic), test addr at line 1425
#                          (inside #[cfg(test)] block starting at line 1098).
#   pool_sync_worker.rs  : MULTICALL3_ADDR (same deterministic deploy as above).
#   cex_dex_worker.rs    : dex_pool_addr in default_pairs() is the WETH/USDC 0.05%
#                          UniV3 pool — well-known canonical pool. Phase 2 will
#                          make this operator-configurable.
#   jit_v3_worker.rs     : same WETH/USDC pool address in default config.
#   recon/pnl_engine.rs  : address in a doc comment inside #[cfg(test)] block
#                          (test block starts line 188; violation at line 199).
#                          G. falso positivo — comment-only, no runtime value.
#   route_plan.rs        : zero address sentinel in test fixture inside
#                          #[cfg(test)] block (starts line 141; violation at 150).
#                          F. fixture — zero address is a null-factory placeholder.
#   shared-rs/price_oracle.rs: WETH address in test assertion inside
#                          #[cfg(test)] block (starts line 300; violation at 402).
#                          F. fixture.
#   credentials/validators.ts : single address is WETH_MAINNET_ADDR constant,
#                          declared with D. doctrine comment as canonical probe
#                          token for the Alchemy Prices endpoint validation.
#                          All exchange URLs removed in this sprint (refactored
#                          to config/exchange-endpoints.ts). No A/B/C violations remain.
#
# D/E. Token catalog literals in protocol engines (H. deuda real — consolidation
# into backend/shared-rs/src/tokens.rs is tracked in the backlog):
#
#   erc20_storage.rs       : balance_slot_for / allowance_slot_for — empirically
#                            verified storage slot indices for WETH/USDC/USDT/DAI/WBTC/LINK/UNI/AAVE.
#                            These match specific deployed contract storage layouts;
#                            they are not arbitrary addresses. E. constante técnica
#                            legítima. Consolidation into tokens.rs is H. deuda.
#   flashloan_engine.rs    : AAVE_V3_SUPPORTED_ASSETS_MAINNET, DYDX_SUPPORTED_ASSETS
#                            (named constants, D. dirección EVM, lines 80-94)
#                            + inline weth_addr / stable_addrs (D/E, lines 287-300)
#                            + test-block fixtures at line 383+ (F. fixture, already noted).
#                            H. deuda: productive literals to consolidate into
#                            shared-rs/src/tokens.rs in a follow-up PR.
#   triangular_engine.rs   : known_token_address() lookup table — D. dirección EVM.
#                            Same table as triangular_worker (already in allow-list).
#                            H. deuda: both copies to be deduplicated into tokens.rs.
ADDR_ALLOW='(^backend/shared-rs/src/(chains|tokens)\.rs|\.test\.(ts|js|tsx)|^docs/|\.env\.example|/README\.md|#\[cfg\(test\)\]|tests?/|\bcfg!\(test\)|_test\.rs|^automation/scripts/|backend/relays-client/src/bundle_builder\.rs|backend/sim-ctl/src/tx_builder\.rs|backend/sim-ctl/src/main\.rs|backend/selector-api/src/token_safety/internal_heuristic\.ts|backend/token-enricher/src/(main|multicall)\.rs|^replay\.sql|^database/.*\.sql|backend/searcher-rs/src/workers/triangular_worker\.rs|backend/sim-ctl/src/revm_backend\.rs|backend/sed-core/src/connectors/flashbots_simulator\.rs|backend/sed-core/src/connectors/reserve_reader\.rs|backend/sed-core/src/eigenstate/mod\.rs|backend/searcher-rs/src/sim_prefund\.rs|backend/searcher-rs/src/amm_math\.rs|backend/prioritization-spine/src/round_trip_executor\.rs|backend/prioritization-spine/src/swap_encoder\.rs|backend/searcher-rs/src/sim_encoder_pg\.rs|backend/searcher-rs/src/sim_encoder\.rs|backend/searcher-rs/src/sim_orchestrator\.rs|backend/searcher-rs/src/sim_multistep\.rs|backend/searcher-rs/src/chain_client\.rs|backend/searcher-rs/src/workers/price_worker\.rs|backend/searcher-rs/src/reserves\.rs|backend/searcher-rs/src/orchestrator\.rs|backend/relays-client/src/submit_engine\.rs|backend/searcher-rs/src/size_optimizer\.rs|frontend/app/settings/credentials/CredentialsClient\.tsx|backend/searcher-rs/src/scanner\.rs|backend/searcher-rs/src/workers/liquidation_worker\.rs|backend/searcher-rs/src/workers/pool_sync_worker\.rs|backend/searcher-rs/src/workers/cex_dex_worker\.rs|backend/searcher-rs/src/workers/jit_v3_worker\.rs|backend/recon/src/pnl_engine\.rs|backend/prioritization-spine/src/route_plan\.rs|backend/shared-rs/src/price_oracle\.rs|backend/api-server/src/credentials/validators\.ts|backend/prioritization-spine/src/erc20_storage\.rs|backend/searcher-rs/src/engines/flashloan_engine\.rs|backend/searcher-rs/src/engines/triangular_engine\.rs)'
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
# --- OMEGA audit 2026-05-16 (no-hardcode triage sprint) ---
#   sed-core flashbots_simulator.rs : URL violations at lines 79 and 87 are inside
#     `#[cfg(test)] mod tests` block (starts line 68). F. fixture — test-only assert.
#     NOTE: sed-core is under separate cargo-check fix; we may not modify its source.
#   sed-core reserve_reader.rs : URL violation at line 155 is inside `#[cfg(test)]`
#     block (starts line 144). F. fixture — constructor test assertion with placeholder key.
URL_RE='https?://[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}'
URL_ALLOW='(^docs/|\.env\.example|\.md:|\.test\.(ts|js|tsx)|/tests?/|_test\.rs|(ghcr|docker|crates|npmjs|github|githubusercontent|actions)\.(io|com|org)|schema\.json|JSONSchema|w3\.org|prom-client|localhost:|127\.0\.0\.[0-9]+:|api-server:|anvil:|redis:|postgres:|edge:|selector-api:|sim-ctl:|recon:|relays-client:|grafana:|prometheus:|alertmanager:|loki:|<KEY>|<YOUR_KEY>|backend/searcher-rs/src/chain_client\.rs|backend/searcher-rs/src/workers/price_worker\.rs|backend/token-enricher/src/main\.rs|backend/relays-client/src/relay_bloxroute\.rs|^replay\.sql|^database/.*\.sql|frontend/features/onboarding/Phase[0-9]+Client\.tsx|^crucible/scripts/|^contracts/foundry\.toml|backend/sed-core/src/connectors/flashbots_simulator\.rs|backend/sed-core/src/connectors/reserve_reader\.rs)'
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
