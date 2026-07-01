#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# M5 — Sepolia deploy wrapper (gated). Reuses contracts/script/DeployTestnet.s.sol.
#
# DOCTRINE (INVIOLABLE):
#   * SEPOLIA TESTNET ONLY. There is NO mainnet path here. A hard chainId guard
#     refuses to proceed unless the RPC reports chainId 11155111.
#   * DRY-RUN BY DEFAULT (M5_MODE=dry_run): `forge script` WITHOUT --broadcast =
#     pure simulation, nothing is signed or sent on-chain.
#   * A LIVE broadcast (M5_MODE=live) ADDITIONALLY requires the explicit sentinel
#     M5_CONFIRM_LIVE=DEPLOY-SEPOLIA. Absent/wrong sentinel → abort, no broadcast.
#   * Secrets (DEPLOYER_PRIVATE_KEY/ETHERSCAN) come from the environment only;
#     never hard-coded. The dry-run path uses a public dummy key for simulation.
#
# Env:
#   M5_MODE                dry_run (default) | live
#   M5_CONFIRM_LIVE        must equal "DEPLOY-SEPOLIA" for a live broadcast
#   M5_VERIFY              true|false — pass --verify on a live deploy (default true)
#   SEPOLIA_RPC_URL        Sepolia RPC endpoint (required)
#   DEPLOYER_PRIVATE_KEY   deployer key (required; dummy is fine for dry_run)
#   AAVE_POOL_ADDRESS      Aave V3 Sepolia pool (required by DeployTestnet)
# ---------------------------------------------------------------------------
set -euo pipefail

MODE="${M5_MODE:-dry_run}"
: "${SEPOLIA_RPC_URL:?SEPOLIA_RPC_URL not set}"
: "${DEPLOYER_PRIVATE_KEY:?DEPLOYER_PRIVATE_KEY not set}"
: "${AAVE_POOL_ADDRESS:?AAVE_POOL_ADDRESS not set}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACTS_DIR="$(cd "${SCRIPT_DIR}/../contracts" && pwd)"

# --- Hard Sepolia guard: refuse any chain that is not Sepolia (11155111) -----
CHAIN_ID="$(cast chain-id --rpc-url "${SEPOLIA_RPC_URL}" 2>/dev/null || echo "unknown")"
if [ "${CHAIN_ID}" != "11155111" ]; then
  echo "M5 ABORT: RPC chainId='${CHAIN_ID}' is not Sepolia (11155111). This workflow is Sepolia-only; refusing." >&2
  exit 1
fi
echo "M5: chainId 11155111 (Sepolia) confirmed."

ARGS=(script script/DeployTestnet.s.sol --rpc-url "${SEPOLIA_RPC_URL}" -vvvv)

if [ "${MODE}" = "live" ]; then
  if [ "${M5_CONFIRM_LIVE:-}" != "DEPLOY-SEPOLIA" ]; then
    echo "M5 ABORT: live mode requires M5_CONFIRM_LIVE=DEPLOY-SEPOLIA (got '${M5_CONFIRM_LIVE:-<empty>}'). No broadcast." >&2
    exit 1
  fi
  echo "M5: LIVE Sepolia broadcast AUTHORIZED (sentinel + environment passed)."
  ARGS+=(--broadcast)
  if [ "${M5_VERIFY:-true}" = "true" ]; then
    ARGS+=(--verify)
  fi
else
  echo "M5: DRY-RUN — simulation only, NO --broadcast (nothing signed/sent)."
fi

cd "${CONTRACTS_DIR}"
echo "M5: forge ${ARGS[*]}"
forge "${ARGS[@]}"
echo "M5 deploy step DONE (mode=${MODE})."
