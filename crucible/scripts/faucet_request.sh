#!/usr/bin/env bash
# =============================================================================
# OMEGA Crucible — Faucet Request Helper
# =============================================================================
# Prints the canonical faucet URLs for the 3 Crucible networks and verifies
# the GAS_SPONSOR wallets are >= the minimum operational threshold.
#
# Usage:  bash crucible/scripts/faucet_request.sh
# =============================================================================
set -euo pipefail

if [[ -f .env.crucible ]]; then
  # shellcheck disable=SC1091
  source .env.crucible
else
  echo "ERROR: .env.crucible not present. Copy from .env.crucible.template first." >&2
  exit 1
fi

declare -A FAUCETS=(
  ["Holesky"]="https://holesky-faucet.pk910.de  |  https://faucet.quicknode.com/ethereum/holesky"
  ["ArbSepolia"]="https://faucet.quicknode.com/arbitrum/sepolia  |  https://www.alchemy.com/faucets/arbitrum-sepolia"
  ["PolygonAmoy"]="https://faucet.polygon.technology/  |  https://www.alchemy.com/faucets/polygon-amoy"
)

declare -A SPONSORS=(
  ["Holesky"]="$GAS_SPONSOR_HOLESKY|$HOLESKY_RPC_URL"
  ["ArbSepolia"]="$GAS_SPONSOR_ARB_SEPOLIA|$ARB_SEPOLIA_RPC_URL"
  ["PolygonAmoy"]="$GAS_SPONSOR_POLYGON_AMOY|$POLYGON_AMOY_RPC_URL"
)

MIN_ETH="0.05"

echo "========================================================================"
echo "OMEGA CRUCIBLE — Faucet Request Manifest"
echo "========================================================================"
for net in "Holesky" "ArbSepolia" "PolygonAmoy"; do
  IFS='|' read -r addr rpc <<< "${SPONSORS[$net]}"
  echo
  echo "[$net]"
  echo "  Faucets:   ${FAUCETS[$net]}"
  echo "  Sponsor:   ${addr:-<unset>}"
  if [[ -n "${addr:-}" ]]; then
    bal_hex=$(curl -sS -X POST "$rpc" \
      -H 'Content-Type: application/json' \
      -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_getBalance\",\"params\":[\"$addr\",\"latest\"],\"id\":1}" \
      | jq -r '.result // "0x0"')
    bal_eth=$(python3 -c "print(int('$bal_hex',16) / 1e18)")
    echo "  Balance:   ${bal_eth} ETH"
    if python3 -c "import sys; sys.exit(0 if float('$bal_eth') >= float('$MIN_ETH') else 1)"; then
      echo "  Status:    READY (≥ ${MIN_ETH} ETH)"
    else
      echo "  Status:    NEEDS_FUNDING — request from faucets above"
    fi
  fi
done
echo
echo "========================================================================"
echo "When all 3 networks report READY, proceed to:  bash crucible/scripts/deploy_crucible.sh"
echo "========================================================================"
