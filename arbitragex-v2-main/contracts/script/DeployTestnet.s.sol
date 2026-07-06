// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-13: Testnet deploy script — Sepolia / Holesky
//
// Deploys all four contracts as UUPS proxies (ERC1967Proxy) with the deployer
// as the initial admin. Reads configuration from environment variables.
//
// Usage (Sepolia):
//   export DEPLOYER_PRIVATE_KEY=0x...
//   export AAVE_POOL_ADDRESS=0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951  # Aave V3 Sepolia
//   export SEPOLIA_RPC_URL=https://...
//   export ETHERSCAN_API_KEY=...
//
//   forge script script/DeployTestnet.s.sol \
//     --rpc-url $SEPOLIA_RPC_URL \
//     --broadcast \
//     --verify \
//     -vvvv
//
// Usage (Holesky — no Aave V3 yet, pass address(0) or a mock):
//   export AAVE_POOL_ADDRESS=0x0000000000000000000000000000000000000001
//   forge script script/DeployTestnet.s.sol \
//     --rpc-url $HOLESKY_RPC_URL \
//     --broadcast \
//     -vvvv
//
// Outputs (logged via console2):
//   ArbitrageExecutor proxy address
//   AllowanceManager proxy address
//   FlashLoanExecutor proxy address
//   AdminTimelock proxy address
//
// Post-deploy checklist:
//   1. Grant EXECUTOR_ROLE on ArbitrageExecutor to your off-chain signer.
//   2. Grant EXECUTOR_ROLE on FlashLoanExecutor to your off-chain signer.
//   3. Approve tokens via ArbitrageExecutor.setTokenApproval().
//   4. Approve routers via ArbitrageExecutor.setRouterApproval().
//   5. Grant allowances via AllowanceManager.grantAllowance() or batchGrantAllowance().
//   6. (SC-10) Transfer ADMIN_ROLE on all three contracts to AdminTimelock proxy.
//      See contracts/DEPLOY.md §9 for the role-transfer procedure.
// =============================================================================

import "forge-std/Script.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/AllowanceManager.sol";
import "../src/FlashLoanExecutor.sol";
import "../src/AdminTimelock.sol";

contract DeployTestnet is Script {
    function run() external {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);
        address aavePool = vm.envAddress("AAVE_POOL_ADDRESS");

        console2.log("=== ArbitrageX v2 Testnet Deploy ===");
        console2.log("Deployer:", deployer);
        console2.log("Aave Pool:", aavePool);
        console2.log("Chain ID:", block.chainid);

        vm.startBroadcast(deployerKey);

        // ----------------------------------------------------------------
        // 1. ArbitrageExecutor — UUPS proxy
        // ----------------------------------------------------------------
        ArbitrageExecutor implAE = new ArbitrageExecutor();
        ERC1967Proxy proxyAE = new ERC1967Proxy(
            address(implAE),
            abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, deployer)
        );
        address arbitrageExecutorProxy = address(proxyAE);

        // ----------------------------------------------------------------
        // 2. AllowanceManager — UUPS proxy
        // ----------------------------------------------------------------
        AllowanceManager implAM = new AllowanceManager();
        ERC1967Proxy proxyAM = new ERC1967Proxy(
            address(implAM),
            abi.encodeWithSelector(AllowanceManager.initialize.selector, deployer)
        );
        address allowanceManagerProxy = address(proxyAM);

        // ----------------------------------------------------------------
        // 3. FlashLoanExecutor — UUPS proxy
        //    Points to: aavePool + arbitrageExecutorProxy
        // ----------------------------------------------------------------
        FlashLoanExecutor implFL = new FlashLoanExecutor();
        ERC1967Proxy proxyFL = new ERC1967Proxy(
            address(implFL),
            abi.encodeWithSelector(
                FlashLoanExecutor.initialize.selector,
                deployer,
                aavePool,
                arbitrageExecutorProxy
            )
        );
        address flashLoanExecutorProxy = address(proxyFL);

        // ----------------------------------------------------------------
        // 4. AdminTimelock — SC-10
        //    Testnet: 60s minDelay so the team can iterate quickly.
        //    Mainnet: use DeployMainnet.s.sol which sets 86400 (24h).
        //
        //    proposers  = [deployer]  (replace with multisig post-config)
        //    executors  = [deployer]  (replace with multisig post-config)
        //    admin      = deployer    (renounce after transferring roles)
        // ----------------------------------------------------------------
        address[] memory proposers = new address[](1);
        proposers[0] = deployer;
        address[] memory executors = new address[](1);
        executors[0] = deployer;

        AdminTimelock implTL = new AdminTimelock();
        ERC1967Proxy proxyTL = new ERC1967Proxy(
            address(implTL),
            abi.encodeWithSelector(
                AdminTimelock.initialize.selector,
                uint256(60),  // 60s — testnet iteration speed
                proposers,
                executors,
                deployer
            )
        );
        address adminTimelockProxy = address(proxyTL);

        vm.stopBroadcast();

        // ----------------------------------------------------------------
        // Output — copy these addresses to your .env / ops runbook
        // ----------------------------------------------------------------
        console2.log("");
        console2.log("=== Deployed Proxies ===");
        console2.log("ArbitrageExecutor proxy :", arbitrageExecutorProxy);
        console2.log("AllowanceManager proxy  :", allowanceManagerProxy);
        console2.log("FlashLoanExecutor proxy :", flashLoanExecutorProxy);
        console2.log("AdminTimelock proxy     :", adminTimelockProxy);
        console2.log("");
        console2.log("=== Implementation Addresses (for verify --watch) ===");
        console2.log("ArbitrageExecutor impl  :", address(implAE));
        console2.log("AllowanceManager impl   :", address(implAM));
        console2.log("FlashLoanExecutor impl  :", address(implFL));
        console2.log("AdminTimelock impl      :", address(implTL));
        console2.log("");
        console2.log("=== Post-Deploy Checklist ===");
        console2.log("[ ] grantRole(EXECUTOR_ROLE, <signer>) on ArbitrageExecutor");
        console2.log("[ ] grantRole(EXECUTOR_ROLE, <signer>) on FlashLoanExecutor");
        console2.log("[ ] setTokenApproval(<tokenIn>, true) on ArbitrageExecutor");
        console2.log("[ ] setRouterApproval(<router>, true) on ArbitrageExecutor");
        console2.log("[ ] batchGrantAllowance(...) on AllowanceManager if needed");
        console2.log("[ ] (SC-10) Transfer ADMIN_ROLE to AdminTimelock -- see DEPLOY.md S9");
    }
}
