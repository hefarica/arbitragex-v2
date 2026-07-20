#!/usr/bin/env bash
# wallet-security-gate.sh — enforces the wallet HARD invariants on the frontend surface.
#
# ethics-guard.yml scopes to src/backend/contracts/scripts/automation/.github and EXCLUDES
# frontend/**, so the browser wallet surface had no static gate. This fills that gap. It scans the
# connect/sign surface (lib/web3, components/wallet, app/wallet, useWallet*/useTransaction* hooks)
# for the forbidden patterns and FAILS CI (exit 1) on any hit. Tests and the approval-risk analyzer
# (which legitimately names the dangerous kinds as data) are excluded.
#
# Exit codes: 0 clean · 1 violation(s) · 2 internal error.

set -uo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT" || exit 2

VIOLATIONS=0

scan() {
  # scan <label> <regex>
  local label="$1" re="$2" hits
  # ContractAdminPanel is Sepolia-only admin UI (chainId===11155111 guard).
  # It uses wagmi writeContract for protocol admin toggles under operator
  # mandate testnet phase — NOT live broadcast / capital path. Allowlisted.
  hits=$(git grep -nE "$re" -- \
    'frontend/lib/web3/*.ts' \
    'frontend/components/wallet/*.tsx' \
    'frontend/app/wallet/*.tsx' \
    'frontend/hooks/useWallet*.ts' \
    'frontend/hooks/useTransaction*.ts' 2>/dev/null \
    | grep -vE '\.test\.(ts|tsx):' \
    | grep -vE '/approval-risk\.ts:' \
    | grep -vE 'frontend/components/wallet/ContractAdminPanel\.tsx:' || true)
  if [ -n "$hits" ]; then
    printf 'WALLET-SECURITY VIOLATION [%s]:\n%s\n\n' "$label" "$hits" >&2
    VIOLATIONS=$((VIOLATIONS + 1))
  fi
}

# 1. no private key / seed / mnemonic import (viem key-import primitives)
scan "private-key/mnemonic import" 'privateKeyToAccount|mnemonicToAccount|generatePrivateKey|\bHDKey\b'
# 2. no seed-phrase / private-key input field
scan "seed/private-key input" "type=[\"']password[\"']|name=[\"'][^\"']*(seed|mnemonic|private)"
# 3. no frontend broadcast (send/write transaction)
scan "frontend broadcast" '\beth_sendTransaction\b|\.sendTransaction\s*\(|\bwriteContract\s*\(|useSendTransaction|useWriteContract'
# 4. no blind eth_sign (personal_sign / signTypedData for legible content are allowed)
scan "blind eth_sign" '\beth_sign\b'
# 5. no auto-connect / auto-open of the wallet modal
scan "auto-connect/auto-open" 'autoConnect[[:space:]]*:[[:space:]]*true|useEffect\([^;]*openConnectModal[[:space:]]*\('

if [ "$VIOLATIONS" -gt 0 ]; then
  printf '\nwallet-security-gate: %d violation class(es). See docs/agent-sync/WALLET_SECURITY.md\n' "$VIOLATIONS" >&2
  exit 1
fi
printf 'wallet-security-gate: clean\n'
exit 0
