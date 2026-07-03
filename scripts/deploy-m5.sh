#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# M5 — Sepolia SIMULATION wrapper (CI-keyless). Reuses contracts/script/DeployTestnet.s.sol.
#
# DOCTRINE (INVIOLABLE): CI validates; the operator signs. CI never custodies or transmits keys.
#   * SEPOLIA TESTNET ONLY. There is NO mainnet path here. A hard chainId guard refuses to proceed
#     unless the RPC reports chainId 11155111.
#   * SIMULATION ONLY. `forge script` runs without any broadcast flag — nothing is signed or sent.
#   * NO LIVE PATH IN CI. A real Sepolia deploy (signing + on-chain send) runs from the operator/KMS
#     plane, never from GitHub Actions. If a live mode is requested here, this script FAILS CLOSED.
#   * NO SECRET KEY. The simulation derives a sender from a PUBLIC, well-known test key (Anvil
#     account #0 — zero value on any real network). No deployer secret is read or accepted.
#
# Env:
#   M5_MODE               dry_run (default). Any other value is refused (operator-plane only).
#   SEPOLIA_RPC_URL       Sepolia RPC endpoint (required)
#   AAVE_POOL_ADDRESS     Aave V3 Sepolia pool (required by DeployTestnet)
# ---------------------------------------------------------------------------
set -euo pipefail

MODE="${M5_MODE:-dry_run}"

# Fail closed on any non-simulation mode. Live signing/sending is operator/KMS plane only.
if [ "${MODE}" != "dry_run" ]; then
  echo "LIVE_DEPLOY_OPERATOR_PLANE_ONLY: mode='${MODE}' is refused. A live Sepolia deploy signs and" >&2
  echo "  sends on-chain and therefore runs from the operator/KMS plane, never from CI. This CI job is" >&2
  echo "  simulation-only and keyless. See docs/m5-sepolia-runbook.md (operator-plane deploy)." >&2
  exit 1
fi

: "${SEPOLIA_RPC_URL:?SEPOLIA_RPC_URL not set}"
: "${AAVE_POOL_ADDRESS:?AAVE_POOL_ADDRESS not set}"

# Public simulation sender — Anvil account #0. Zero value on any real network; NEVER a real deployer.
# DeployTestnet.s.sol reads this via vm.envUint for the simulated sender only (no signing key custody).
export DEPLOYER_PRIVATE_KEY="${DEPLOYER_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACTS_DIR="$(cd "${SCRIPT_DIR}/../contracts" && pwd)"

# --- Hard Sepolia guard: refuse any chain that is not Sepolia (11155111) -----
CHAIN_ID="$(cast chain-id --rpc-url "${SEPOLIA_RPC_URL}" 2>/dev/null || echo "unknown")"
if [ "${CHAIN_ID}" != "11155111" ]; then
  echo "M5 ABORT: RPC chainId='${CHAIN_ID}' is not Sepolia (11155111). This workflow is Sepolia-only; refusing." >&2
  exit 1
fi
echo "M5: chainId 11155111 (Sepolia) confirmed."

echo "M5: SIMULATION only — no on-chain send, no signing key custody (CI is keyless)."
cd "${CONTRACTS_DIR}"
forge script script/DeployTestnet.s.sol --rpc-url "${SEPOLIA_RPC_URL}" -vvvv
echo "M5 simulation DONE (mode=dry_run)."
