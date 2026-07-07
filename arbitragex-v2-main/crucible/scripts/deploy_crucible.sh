#!/usr/bin/env bash
# =============================================================================
# OMEGA Crucible — Deterministic CREATE2 Multi-Testnet Deploy
# =============================================================================
# Deploys DeterministicFactory + ResolutionCore + Adapters on the 3 testnets
# with identical addresses via Foundry forge script.
# =============================================================================
set -euo pipefail
source .env.crucible

run_deploy() {
  local network="$1" rpc="$2" chain_id="$3"
  echo "------------------------------------------------------------------------"
  echo "Deploying to $network (chain_id=$chain_id) ..."
  forge script contracts/script/DeployCrucible.s.sol:DeployCrucible \
    --rpc-url "$rpc" \
    --broadcast \
    --slow \
    --legacy \
    --chain-id "$chain_id" \
    --sender "$GAS_SPONSOR_HOLESKY" \
    --private-key "$(omega-cred get $GAS_SPONSOR_KEY_REF)" \
    --sig 'run()' \
    -vvv
}

run_deploy "Holesky"     "$HOLESKY_RPC_URL"     "$HOLESKY_CHAIN_ID"
run_deploy "ArbSepolia"  "$ARB_SEPOLIA_RPC_URL" "$ARB_SEPOLIA_CHAIN_ID"
run_deploy "PolygonAmoy" "$POLYGON_AMOY_RPC_URL" "$POLYGON_AMOY_CHAIN_ID"

echo
echo "========================================================================"
echo "Deployment complete. Verifying CREATE2 address symmetry..."
echo "========================================================================"
python3 crucible/scripts/verify_address_symmetry.py
