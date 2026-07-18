// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-12-Sepolia: Sepolia testnet deploy script — Ethereum L1 testnet
//
// Adapted from DeployMainnet.s.sol for Sepolia testnet (chainid 11155111).
// Used for E2E smoke-testing the simulator-v2 REVM path before mainnet.
//
// Differences from mainnet deploy:
//   - Chain ID: 11155111 (Sepolia)
//   - Aave V3 Pool: Sepolia testnet address
//   - Timelock delay: 1h (3600s) instead of 24h
//   - Multisig can be EOA on testnet (no contract check)
//   - Balance requirement: 0.05 SepoliaETH (testnet is cheap)
//   - Environment variable: CONFIRM_SEPOLIA_DEPLOY
//
// PRE-DEPLOY CHECKLIST:
//   [ ] SepoliaETH in deployer wallet (get from faucet: sepolia-faucet.pk910.de)
//   [ ] Set CONFIRM_SEPOLIA_DEPLOY=true
//   [ ] Set DEPLOYER_PRIVATE_KEY (test key, not mainnet)
//   [ ] Set MULTISIG_ADDRESS (can be same as deployer on testnet)
//   [ ] Set SEPOLIA_RPC_URL
//
// Usage:
//   export DEPLOYER_PRIVATE_KEY=0x...
//   export CONFIRM_SEPOLIA_DEPLOY=true
//   export SEPOLIA_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com
//
//   forge script script/DeploySepolia.s.sol \
//     --rpc-url $SEPOLIA_RPC_URL \
//     --broadcast \
//     -vvvv
//
// Post-deploy: run scripts/smoke-test-sepolia.sh to validate simulator-v2.
// =============================================================================

import "forge-std/Script.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/AllowanceManager.sol";
import "../src/FlashLoanExecutor.sol";
import "../src/AdminTimelock.sol";

contract DeploySepolia is Script {
    // Aave V3 Pool on Sepolia testnet (verified 2026-07).
    // Source: https://docs.aave.com/developers/deployed-contracts/v3-testnet-addresses
    address constant AAVE_V3_POOL_SEPOLIA_DEFAULT = 0x6Ae43d3271d1bB2bD0B19dF999473B5Bb40eF162;

    function run() external {
        // Safety gate 1: explicit opt-in
        require(
            vm.envBool("CONFIRM_SEPOLIA_DEPLOY"),
            "DeploySepolia: set CONFIRM_SEPOLIA_DEPLOY=true to proceed"
        );

        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer    = vm.addr(deployerKey);

        // Safety gate 2: chain ID must be Sepolia (11155111)
        require(block.chainid == 11155111, "DeploySepolia: not on Sepolia (chainid != 11155111)");

        // M2: MULTISIG_ADDRESS — on testnet can be EOA (relaxed contract check)
        address multisig = vm.envAddress("MULTISIG_ADDRESS");
        require(multisig != address(0), "DeploySepolia: MULTISIG_ADDRESS not set");
        require(
            multisig != deployer,
            "DeploySepolia: MULTISIG_ADDRESS must differ from deployer (timelock requires separation of duties)"
        );
        // Relaxed for testnet: multisig does NOT need to be a contract

        // Aave V3 Pool from env or default
        address aavePool = vm.envOr("AAVE_V3_POOL", AAVE_V3_POOL_SEPOLIA_DEFAULT);
        require(
            aavePool.code.length > 0,
            "DeploySepolia: AAVE_V3_POOL must be a deployed contract (check env or default address)"
        );

        // Safety gate 3: deployer must have at least 0.05 SepoliaETH
        require(
            deployer.balance >= 0.05 ether,
            "DeploySepolia: deployer balance < 0.05 SepoliaETH -- get from faucet"
        );

        console2.log("=== ArbitrageX v2 Sepolia Deploy (Smoke Test) ===");
        console2.log("Deployer        :", deployer);
        console2.log("Deployer balance:", deployer.balance);
        console2.log("Multisig        :", multisig);
        console2.log("Aave V3 Pool    :", aavePool);
        console2.log("Chain ID        :", block.chainid);

        vm.startBroadcast(deployerKey);

        // 1. ArbitrageExecutor — UUPS proxy
        ERC1967Proxy proxyAE;
        {
            ArbitrageExecutor implAE = new ArbitrageExecutor();
            console2.log("ArbitrageExecutor impl  :", address(implAE));
            proxyAE = new ERC1967Proxy(
                address(implAE),
                abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, deployer)
            );
        }

        // 2. AllowanceManager — UUPS proxy
        ERC1967Proxy proxyAM;
        {
            AllowanceManager implAM = new AllowanceManager();
            console2.log("AllowanceManager impl   :", address(implAM));
            proxyAM = new ERC1967Proxy(
                address(implAM),
                abi.encodeWithSelector(AllowanceManager.initialize.selector, deployer)
            );
        }

        // 3. FlashLoanExecutor — UUPS proxy
        ERC1967Proxy proxyFL;
        {
            FlashLoanExecutor implFL = new FlashLoanExecutor();
            console2.log("FlashLoanExecutor impl  :", address(implFL));
            proxyFL = new ERC1967Proxy(
                address(implFL),
                abi.encodeWithSelector(
                    FlashLoanExecutor.initialize.selector,
                    deployer,
                    aavePool,
                    address(proxyAE)
                )
            );
        }

        // 4. AdminTimelock — 1h delay on Sepolia testnet
        ERC1967Proxy proxyTL;
        {
            address[] memory proposers = new address[](1);
            proposers[0] = multisig;
            address[] memory executors = new address[](1);
            executors[0] = multisig;

            AdminTimelock implTL = new AdminTimelock();
            console2.log("AdminTimelock impl      :", address(implTL));
            proxyTL = new ERC1967Proxy(
                address(implTL),
                abi.encodeWithSelector(
                    AdminTimelock.initialize.selector,
                    uint256(3_600), // 1h — Sepolia testnet
                    proposers,
                    executors,
                    deployer
                )
            );
        }

        // Atomic role-custody transfer to timelock (M10 + P0 fix)
        address timelockProxy = address(proxyTL);

        {
            // --- ArbitrageExecutor ---
            ArbitrageExecutor ae = ArbitrageExecutor(payable(address(proxyAE)));
            ae.grantRole(ae.EXECUTOR_ROLE(), address(proxyFL));
            ae.grantRole(ae.UPGRADER_ROLE(), timelockProxy);
            ae.revokeRole(ae.UPGRADER_ROLE(), deployer);
            ae.grantRole(ae.DEFAULT_ADMIN_ROLE(), timelockProxy);
            ae.revokeRole(ae.DEFAULT_ADMIN_ROLE(), deployer);

            // --- AllowanceManager ---
            AllowanceManager am = AllowanceManager(payable(address(proxyAM)));
            am.grantRole(am.UPGRADER_ROLE(), timelockProxy);
            am.revokeRole(am.UPGRADER_ROLE(), deployer);
            am.grantRole(am.DEFAULT_ADMIN_ROLE(), timelockProxy);
            am.revokeRole(am.DEFAULT_ADMIN_ROLE(), deployer);

            // --- FlashLoanExecutor ---
            FlashLoanExecutor fl = FlashLoanExecutor(payable(address(proxyFL)));
            fl.grantRole(fl.UPGRADER_ROLE(), timelockProxy);
            fl.revokeRole(fl.UPGRADER_ROLE(), deployer);
            fl.grantRole(fl.DEFAULT_ADMIN_ROLE(), timelockProxy);
            fl.revokeRole(fl.DEFAULT_ADMIN_ROLE(), deployer);

            // --- AdminTimelock (self) ---
            AdminTimelock tl = AdminTimelock(payable(timelockProxy));
            tl.renounceRole(tl.DEFAULT_ADMIN_ROLE(), deployer);
        }

        console2.log("Admin + upgrader transferred to timelock:", timelockProxy);
        console2.log("Deployer admin + upgrader revoked from all contracts (incl. timelock):", deployer);

        vm.stopBroadcast();

        // Output
        console2.log("");
        console2.log("=== Deployed Proxies (Sepolia) ===");
        console2.log("ArbitrageExecutor proxy :", address(proxyAE));
        console2.log("AllowanceManager proxy  :", address(proxyAM));
        console2.log("FlashLoanExecutor proxy :", address(proxyFL));
        console2.log("AdminTimelock proxy     :", address(proxyTL));
        console2.log("");
        console2.log("=== NEXT STEPS FOR SMOKE TEST ===");
        console2.log("1. Copy proxy addresses to your .env:");
        console2.log("   ARBITRAGE_EXECUTOR=", address(proxyAE));
        console2.log("   FLASHLOAN_EXECUTOR=", address(proxyFL));
        console2.log("2. Set FLASHLOAN_EXECUTOR_11155111=", address(proxyFL));
        console2.log("3. Approve tokens + routers + allowances (post-deploy steps 4-6 from mainnet runbook)");
        console2.log("4. Run: ./scripts/smoke-test-sepolia.sh");
        console2.log("5. On SIM_SUCCESS → set ARBX_SIMULATOR_V2_READY=true");
        console2.log("");
        console2.log("NOTE: Timelock delay = 1h (testnet). For production use DeployMainnet.s.sol (24h).");
    }
}
