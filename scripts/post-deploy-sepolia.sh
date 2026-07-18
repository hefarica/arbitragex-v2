#!/usr/bin/env bash
# =============================================================================
# Post-Deploy Sepolia — Comandos cast para configurar contratos
# =============================================================================
# Ejecutar DESPUÉS de DeploySepolia.s.sol. Configura tokens, routers,
# allowances, y roles necesarios para que el smoke test funcione.
#
# Pre-requisitos:
#   - forge/cast instalado
#   - Variables de entorno seteadas (ver abajo)
#   - Deployer wallet tiene SepoliaETH para gas
#
# Uso:
#   export ARBITRAGE_EXECUTOR=0x...
#   export ALLOWANCE_MANAGER=0x...
#   export FLASHLOAN_EXECUTOR=0x...
#   export ADMIN_PROXY=0x...            # Timelock proxy (tiene admin roles)
#   export DEPLOYER_PRIVATE_KEY=0x...   # O usa --ledger/--trezor
#   export SEPOLIA_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com
#   ./scripts/post-deploy-sepolia.sh
# =============================================================================

set -euo pipefail

# ── Validación de env vars ─────────────────────────────────────────
: "${ARBITRAGE_EXECUTOR:?Requerido: dirección proxy ArbitrageExecutor}"
: "${ALLOWANCE_MANAGER:?Requerido: dirección proxy AllowanceManager}"
: "${FLASHLOAN_EXECUTOR:?Requerido: dirección proxy FlashLoanExecutor}"
: "${ADMIN_PROXY:?Requerido: dirección proxy AdminTimelock (tiene admin roles)}"
: "${DEPLOYER_PRIVATE_KEY:?Requerido: private key del deployer (tiene admin en Sepolia)}"
: "${SEPOLIA_RPC_URL:?Requerido: RPC endpoint de Sepolia}"

CAST="cast"
RPC="--rpc-url $SEPOLIA_RPC_URL"
# Si usas Ledger/Trezor en lugar de private key, reemplaza --private-key por --ledger
AUTH="--private-key $DEPLOYER_PRIVATE_KEY"

echo "=== Post-Deploy Sepolia Configuration ==="
echo "ArbitrageExecutor : $ARBITRAGE_EXECUTOR"
echo "AllowanceManager  : $ALLOWANCE_MANAGER"
echo "FlashLoanExecutor : $FLASHLOAN_EXECUTOR"
echo "AdminTimelock     : $ADMIN_PROXY"
echo ""

# ── Direcciones de tokens y routers en Sepolia ─────────────────────
WETH="0xfff9976782d46cc05630d1f6ebab18b2324d6b14"
USDC="0x1c7d4b196cb0c7b01d743fbc6116a902379c7238"
# Uniswap V2 Router Sepolia
UNI_V2_ROUTER="0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3"
# Uniswap V3 Universal Router Sepolia
UNI_V3_ROUTER="0x3bFA4769FB12cAbC4f333E0015D74eBd78D58861"
# Balancer Vault (flash loan fallback)
BALANCER_VAULT="0xBA12222222228d8Ba445958a75a0704d566BF2C8"

echo "=== 1. Wire AllowanceManager en ArbitrageExecutor ==="
$CAST send $ARBITRAGE_EXECUTOR \
  "setAllowanceManager(address)" $ALLOWANCE_MANAGER \
  $RPC $AUTH

echo "=== 2. Aprobar tokens en ArbitrageExecutor ==="
$CAST send $ARBITRAGE_EXECUTOR \
  "setTokenApproval(address,bool)" $WETH true \
  $RPC $AUTH
$CAST send $ARBITRAGE_EXECUTOR \
  "setTokenApproval(address,bool)" $USDC true \
  $RPC $AUTH

echo "=== 3. Aprobar routers en ArbitrageExecutor ==="
$CAST send $ARBITRAGE_EXECUTOR \
  "setRouterApproval(address,bool)" $UNI_V2_ROUTER true \
  $RPC $AUTH
$CAST send $ARBITRAGE_EXECUTOR \
  "setRouterApproval(address,bool)" $UNI_V3_ROUTER true \
  $RPC $AUTH

echo "=== 4. Grant allowances en AllowanceManager ==="
# Aprobar routers para gastar tokens en nombre del ArbitrageExecutor
# MAX_SAFE_ALLOWANCE = 1e30 (definido en el contrato)
MAX_ALLOWANCE="1000000000000000000000000000000"
$CAST send $ALLOWANCE_MANAGER \
  "batchGrantAllowance(address[],address[],uint256[])" \
  "[$WETH,$USDC]" \
  "[$UNI_V2_ROUTER,$UNI_V2_ROUTER]" \
  "[$MAX_ALLOWANCE,$MAX_ALLOWANCE]" \
  $RPC $AUTH

echo "=== 5. Configurar FlashLoanExecutor ==="
# Set Balancer Vault como fallback
$CAST send $FLASHLOAN_EXECUTOR \
  "setBalancerVault(address)" $BALANCER_VAULT \
  $RPC $AUTH
# Referral code = 0 (deshabilitado en testnet)
$CAST send $FLASHLOAN_EXECUTOR \
  "setReferralCode(uint16)" 0 \
  $RPC $AUTH

echo "=== 6. Grant EXECUTOR_ROLE a la EOA de prueba (opcional) ==="
# Si TEST_CALLER es una EOA que ejecutará el smoke test directamente contra el simulador,
# necesita EXECUTOR_ROLE en FlashLoanExecutor para que requestFlashLoan no revierta.
# En producción esto NO se hace; el rol va al relays-client signer.
if [[ -n "${TEST_CALLER:-}" ]]; then
  echo "Granting EXECUTOR_ROLE on FlashLoanExecutor to TEST_CALLER=$TEST_CALLER"
  # EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE") = 0xd8aa0f3194971a2a116679f7c2090f6939c8d4e01a2a8d7e41d55e5351469e63
  EXECUTOR_ROLE="0xd8aa0f3194971a2a116679f7c2090f6939c8d4e01a2a8d7e41d55e5351469e63"
  $CAST send $FLASHLOAN_EXECUTOR \
    "grantRole(bytes32,address)" $EXECUTOR_ROLE $TEST_CALLER \
    $RPC $AUTH
fi

echo ""
echo "=== Post-Deploy Completo ==="
echo ""
echo "Variables para .env del sim-ctl:"
echo "  ARBITRAGE_EXECUTOR=$ARBITRAGE_EXECUTOR"
echo "  FLASHLOAN_EXECUTOR=$FLASHLOAN_EXECUTOR"
echo "  FLASHLOAN_EXECUTOR_11155111=$FLASHLOAN_EXECUTOR"
echo ""
echo "Verificación rápida:"
echo "  cast call $ARBITRAGE_EXECUTOR 'allowanceManager()(address)' --rpc-url $SEPOLIA_RPC_URL"
echo "  cast call $ARBITRAGE_EXECUTOR 'approvedTokens(address)(bool)' $WETH --rpc-url $SEPOLIA_RPC_URL"
echo "  cast call $FLASHLOAN_EXECUTOR 'balancerVault()(address)' --rpc-url $SEPOLIA_RPC_URL"
