// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-12: Mainnet deploy script - Ethereum L1
//
// Deploys all three contracts as UUPS proxies (ERC1967Proxy) with the deployer
// as the initial admin.  Includes multiple safety guards to prevent accidental
// mainnet deploys during testing.
//
// PRE-DEPLOY CHECKLIST (run before broadcasting):
//   [ ] Confirm `block.chainid == 1` (Ethereum mainnet).
//   [ ] Verify deployer balance >= 0.5 ETH for gas.
//   [ ] Set CONFIRM_MAINNET_DEPLOY=true in environment (explicit opt-in).
//   [ ] Set DEPLOYER_PRIVATE_KEY to the operator hot key (NOT the multisig).
//   [ ] Verify AAVE_V3_MAINNET_POOL via Aave docs (hardcoded below as fallback).
//
// Usage:
//   export DEPLOYER_PRIVATE_KEY=0x...
//   export CONFIRM_MAINNET_DEPLOY=true
//   export ETHERSCAN_API_KEY=...
//   export MAINNET_RPC_URL=https://...
//
//   forge script script/DeployMainnet.s.sol \
//     --rpc-url $MAINNET_RPC_URL \
//     --broadcast \
//     --verify \
//     -vvvv
//
// Outputs (logged via console2):
//   ArbitrageExecutor proxy address
//   AllowanceManager proxy address
//   FlashLoanExecutor proxy address
//   Next-steps checklist
//
// Post-deploy: see contracts/DEPLOY.md for the full operations runbook.
// =============================================================================

import "forge-std/Script.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/AllowanceManager.sol";
import "../src/FlashLoanExecutor.sol";
import "../src/AdminTimelock.sol";

contract DeployMainnet is Script {
    // M3 (audit 2026-05-10): AAVE_V3_POOL is read from env at run-time so operators
    // can override without touching this file if Aave migrates the Pool contract.
    // Default fallback is the verified Ethereum mainnet address (2026-05-08).
    // Source: https://docs.aave.com/developers/deployed-contracts/v3-mainnet/ethereum
    address constant AAVE_V3_POOL_MAINNET_DEFAULT = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;

    function run() external {
        // ----------------------------------------------------------------
        // Safety gate 1: explicit opt-in environment variable.
        // Prevents accidental execution in CI, local fork runs, or staging.
        // ----------------------------------------------------------------
        require(vm.envBool("CONFIRM_MAINNET_DEPLOY"), "DeployMainnet: set CONFIRM_MAINNET_DEPLOY=true to proceed");

        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);

        // ----------------------------------------------------------------
        // Safety gate 2: chain ID must be Ethereum mainnet (1).
        // Rejects Sepolia (11155111), Holesky (17000), Anvil (31337), etc.
        // ----------------------------------------------------------------
        require(block.chainid == 1, "DeployMainnet: not on Ethereum mainnet (chainid != 1)");

        // ----------------------------------------------------------------
        // M2 (audit 2026-05-10): MULTISIG_ADDRESS guard.
        // The multisig becomes proposer/executor of the timelock.
        // Must differ from deployer (defeats separation of duties otherwise)
        // and must be a contract (EOA multisig is not a multisig).
        // ----------------------------------------------------------------
        address multisig = vm.envAddress("MULTISIG_ADDRESS");
        require(multisig != address(0), "DeployMainnet: MULTISIG_ADDRESS not set");
        require(
            multisig != deployer,
            "DeployMainnet: MULTISIG_ADDRESS must differ from deployer (timelock requires separation of duties)"
        );
        require(
            multisig.code.length > 0,
            "DeployMainnet: MULTISIG_ADDRESS must be a contract (Gnosis Safe or equivalent multisig wallet)"
        );

        // ----------------------------------------------------------------
        // M3 (audit 2026-05-10): Aave V3 Pool from env (override) or default.
        // Override: export AAVE_V3_POOL=0x... before running forge script.
        // ----------------------------------------------------------------
        address aavePool = vm.envOr("AAVE_V3_POOL", AAVE_V3_POOL_MAINNET_DEFAULT);
        require(
            aavePool.code.length > 0,
            "DeployMainnet: AAVE_V3_POOL must be a deployed contract (check env or default address)"
        );

        // ----------------------------------------------------------------
        // Safety gate 3: deployer must have at least 0.5 ETH.
        // Deploying 3 proxies + 3 implementations ≈ 0.05-0.15 ETH at 50 gwei.
        // 0.5 ETH gives 3-10x headroom.
        // ----------------------------------------------------------------
        require(deployer.balance >= 0.5 ether, "DeployMainnet: deployer balance < 0.5 ETH -- top up before deploying");

        console2.log("=== ArbitrageX v2 Mainnet Deploy ===");
        console2.log("Deployer        :", deployer);
        console2.log("Deployer balance:", deployer.balance);
        console2.log("Multisig        :", multisig);
        console2.log("Aave V3 Pool    :", aavePool);
        console2.log("Chain ID        :", block.chainid);

        vm.startBroadcast(deployerKey);

        // ----------------------------------------------------------------
        // 1. ArbitrageExecutor - UUPS proxy
        //    Admin = deployer. Grant EXECUTOR_ROLE post-deploy to signer.
        // ----------------------------------------------------------------
        // Each implementation local is scoped in its own block so it is freed
        // before the next deploy - this keeps run() under the EVM stack limit
        // (it previously hit "stack too deep") without enabling project-wide
        // via_ir, which would change src/ codegen and gas. Behavior is identical.
        ERC1967Proxy proxyAE;
        {
            ArbitrageExecutor implAE = new ArbitrageExecutor();
            console2.log("ArbitrageExecutor impl  :", address(implAE));
            proxyAE = new ERC1967Proxy(
                address(implAE), abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, deployer)
            );
        }

        // ----------------------------------------------------------------
        // 2. AllowanceManager - UUPS proxy
        //    Admin = deployer. Wire to ArbitrageExecutor post-deploy.
        // ----------------------------------------------------------------
        ERC1967Proxy proxyAM;
        {
            AllowanceManager implAM = new AllowanceManager();
            console2.log("AllowanceManager impl   :", address(implAM));
            proxyAM = new ERC1967Proxy(
                address(implAM), abi.encodeWithSelector(AllowanceManager.initialize.selector, deployer)
            );
        }

        // ----------------------------------------------------------------
        // 3. FlashLoanExecutor - UUPS proxy
        //    Points to: Aave V3 mainnet pool + ArbitrageExecutor proxy.
        // ----------------------------------------------------------------
        ERC1967Proxy proxyFL;
        {
            FlashLoanExecutor implFL = new FlashLoanExecutor();
            console2.log("FlashLoanExecutor impl  :", address(implFL));
            proxyFL = new ERC1967Proxy(
                address(implFL),
                abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, deployer, aavePool, address(proxyAE))
            );
        }

        // ----------------------------------------------------------------
        // 4. AdminTimelock - SC-10
        //    24h minDelay on mainnet.
        //    M2 (audit 2026-05-10): proposer/executor = multisig (validated above).
        //    The deployer EOA is the initial TimelockController admin only so it
        //    can renounce after the timelock is wired. OZ TimelockController grants
        //    DEFAULT_ADMIN_ROLE to BOTH address(this) (self-administration) and the
        //    `admin` arg (deployer); the deployer's copy is renounced below (M10),
        //    leaving only the self-admin so governance runs through schedule+execute.
        // ----------------------------------------------------------------
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
                    uint256(86_400), // 24h - mainnet standard
                    proposers,
                    executors,
                    deployer
                )
            );
        }

        // ----------------------------------------------------------------
        // M10 (audit 2026-05-10) + P0 fix (mainnet-readiness audit 2026-07):
        // Atomic role-custody transfer to timelock.
        //
        // BOTH roles move: DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
        // _authorizeUpgrade on all three UUPS contracts is gated ONLY by
        // UPGRADER_ROLE, and initialize() grants it to the deployer - so
        // transferring admin alone (the pre-fix behavior) left the deployer
        // EOA with an instant upgradeToAndCall that bypassed multisig +
        // 24h timelock entirely.
        //
        // All grantRole+revokeRole calls are INSIDE the same broadcast block
        // as the proxy deployments, so there is no window between script
        // completion and a separate manual transfer where the deployer EOA
        // holds either role.
        //
        // Order per contract (CRITICAL - do not reorder):
        //   1. grant  UPGRADER_ROLE      -> timelock  \ both need the caller to
        //   2. revoke UPGRADER_ROLE      <- deployer  / hold DEFAULT_ADMIN_ROLE
        //   3. grant  DEFAULT_ADMIN_ROLE -> timelock    (UPGRADER's role-admin)
        //   4. revoke DEFAULT_ADMIN_ROLE <- deployer    (LAST: after this the
        //      deployer can no longer grant/revoke anything on the contract)
        // Granting to the timelock before revoking the deployer ensures the
        // contract is never left without an admin or an upgrader (plain
        // AccessControl allows revoking the last holder - that would brick it).
        // ----------------------------------------------------------------
        address timelockProxy = address(proxyTL);

        // Scoped so the executor/manager casts are freed before the output
        // section below - keeps run() under the EVM stack limit.
        {
            // --- ArbitrageExecutor ---
            ArbitrageExecutor ae = ArbitrageExecutor(payable(address(proxyAE)));
            // SC-13 (flash-loan fund-handoff): the flash path calls
            // ArbitrageExecutor.executeArbitrageFlashFunded (onlyExecutor) with
            // FlashLoanExecutor as msg.sender, so the wrapper itself MUST hold
            // EXECUTOR_ROLE. Grant it now, while the deployer still holds
            // DEFAULT_ADMIN_ROLE - after the atomic handoff just below, this grant
            // could only be made via a multisig + 24h-timelock action.
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
            // OZ TimelockController grants DEFAULT_ADMIN_ROLE to the `admin` arg
            // (deployer) at init as a bootstrap admin, IN ADDITION to the timelock's
            // own self-administration (address(this)). Without this renounce the
            // deployer EOA would keep DEFAULT_ADMIN_ROLE over the timelock itself and
            // could grantRole(PROPOSER/EXECUTOR, deployer) instantly (a direct
            // AccessControl call - it does NOT pass through the delay), defeating the
            // multisig separation of duties. renounceRole drops the caller's own role;
            // during broadcast msg.sender == deployer. The timelock's self-admin
            // (address(this)) is untouched, so the multisig can still manage roles via
            // schedule+execute targeting the timelock - it does not brick.
            AdminTimelock tl = AdminTimelock(payable(timelockProxy));
            tl.renounceRole(tl.DEFAULT_ADMIN_ROLE(), deployer);
        }

        console2.log("Admin + upgrader transferred to timelock:", timelockProxy);
        console2.log("Deployer admin + upgrader revoked from all contracts (incl. timelock):", deployer);

        vm.stopBroadcast();

        // ----------------------------------------------------------------
        // Output - copy these to your .env / ops runbook immediately.
        // ----------------------------------------------------------------
        console2.log("");
        console2.log("=== Deployed Proxies ===");
        console2.log("ArbitrageExecutor proxy :", address(proxyAE));
        console2.log("AllowanceManager proxy  :", address(proxyAM));
        console2.log("FlashLoanExecutor proxy :", address(proxyFL));
        console2.log("AdminTimelock proxy     :", address(proxyTL));
        console2.log("");
        console2.log("=== Implementation Addresses (for --verify --watch) ===");
        console2.log("(logged above next to each proxy deploy)");
        console2.log("");
        console2.log("=== MANDATORY Post-Deploy Checklist ===");
        console2.log("[ ] 1. Wire AllowanceManager:");
        console2.log("       ArbitrageExecutor.setAllowanceManager(", address(proxyAM), ")");
        console2.log("[ ] 2. Grant EXECUTOR_ROLE on ArbitrageExecutor to off-chain signer:");
        console2.log("       ArbitrageExecutor.grantRole(EXECUTOR_ROLE, <signer>)");
        console2.log("[ ] 3. Grant EXECUTOR_ROLE on FlashLoanExecutor to off-chain signer:");
        console2.log("       FlashLoanExecutor.grantRole(EXECUTOR_ROLE, <signer>)");
        console2.log("[x] 3b.(SC-13, DONE IN-SCRIPT) FlashLoanExecutor proxy holds EXECUTOR_ROLE on");
        console2.log("       ArbitrageExecutor (required by executeArbitrageFlashFunded). Granted");
        console2.log("       atomically pre-handoff; any change now needs multisig + 24h timelock.");
        console2.log("[ ] 4. Approve tokenIn tokens:");
        console2.log("       ArbitrageExecutor.setTokenApproval(<WETH|USDC|...>, true)");
        console2.log("[ ] 5. Approve routers:");
        console2.log("       ArbitrageExecutor.setRouterApproval(<UniV3Router|...>, true)");
        console2.log("[ ] 6. Batch-grant allowances in AllowanceManager:");
        console2.log("       AllowanceManager.batchGrantAllowance([tokens], [routers], [amounts])");
        console2.log("[ ] 7. (Optional) Set Aave referral code if enrolled:");
        console2.log("       FlashLoanExecutor.setReferralCode(<code>)");
        console2.log("[ ] 8. (Optional) Set Balancer Vault address if using Balancer flash loans:");
        console2.log("       FlashLoanExecutor.setBalancerVault(0xBA12222222228d8Ba445958a75a0704d566BF2C8)");
        console2.log("NOTE: Admin + upgrader atomically transferred to timelock (M10 + P0 fix 2026-07).");
        console2.log("      Deployer EOA no longer holds DEFAULT_ADMIN_ROLE or UPGRADER_ROLE on any contract,");
        console2.log("      including the AdminTimelock itself (bootstrap admin renounced in-script).");
        console2.log("      The timelock retains self-administration (address(this)); future admin actions");
        console2.log("      AND upgrades require multisig + 24h timelock (schedule+execute).");
        console2.log("[ ] 9. (SC-10) Verify admin + upgrader roles via Etherscan or cast:");
        console2.log("       cast call <proxyAE> 'hasRole(bytes32,address)' DEFAULT_ADMIN_ROLE <timelock>");
        console2.log("       cast call <proxyAE> 'hasRole(bytes32,address)' UPGRADER_ROLE <timelock>");
        console2.log("");
        console2.log("See contracts/DEPLOY.md for the full operations runbook.");
    }
}
