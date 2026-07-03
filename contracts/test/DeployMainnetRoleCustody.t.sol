// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// DeployMainnetRoleCustody — P0 regression tests (mainnet-readiness audit 2026-07)
//
// P0 under test: the "atomic handoff" block in script/DeployMainnet.s.sol used
// to transfer ONLY DEFAULT_ADMIN_ROLE to the AdminTimelock.  UPGRADER_ROLE
// stayed with the deployer EOA on all three UUPS proxies, and _authorizeUpgrade
// is gated ONLY by UPGRADER_ROLE — so the deployer kept an instant
// upgradeToAndCall that bypassed multisig + 24h timelock (total-drain vector).
//
// These tests execute the REAL script (DeployMainnet.run()) in-test — the
// handoff sequence exercised here is the script's own code, not a replica —
// and assert, for ArbitrageExecutor, AllowanceManager and FlashLoanExecutor:
//
//   1. hasRole(UPGRADER_ROLE, deployer)      == false   (the P0: pre-fix TRUE)
//   2. hasRole(UPGRADER_ROLE, timelock)      == true    (pre-fix FALSE)
//   3. hasRole(DEFAULT_ADMIN_ROLE, deployer) == false   (regression pin)
//   4. hasRole(DEFAULT_ADMIN_ROLE, timelock) == true    (regression pin)
//   5. deployer upgradeToAndCall REVERTS post-handoff   (pre-fix it SUCCEEDED)
//   6. positive control: an upgrade is still possible via multisig + timelock
//      (guards against "closing the hole by bricking the upgrade path")
//
// The script's safety gates (chainid == 1, env opt-in, multisig-is-contract,
// pool-has-code, 0.5 ETH balance) are satisfied with cheatcodes; no gate is
// weakened in the script itself.
// =============================================================================

import "forge-std/Test.sol";
import "../script/DeployMainnet.s.sol";

contract DeployMainnetRoleCustodyTest is Test {
    // ERC-1967 standard slots/events (eip-1967; same constants OZ ERC1967Utils uses).
    bytes32 internal constant UPGRADED_TOPIC = keccak256("Upgraded(address)");
    bytes32 internal constant ERC1967_IMPL_SLOT =
        bytes32(uint256(keccak256("eip1967.proxy.implementation")) - 1);

    // OZ 5.x AccessControl revert (same pattern as AllowanceManager.t.sol).
    bytes4 internal constant ACL_UNAUTHORIZED =
        bytes4(keccak256("AccessControlUnauthorizedAccount(address,bytes32)"));

    // Must match the minDelay hardcoded in DeployMainnet.run() (86_400 = 24h).
    uint256 internal constant MIN_DELAY = 86_400;

    // Test-only key; any nonzero uint works — vm.addr() derives the deployer.
    uint256 internal constant DEPLOYER_KEY = uint256(keccak256("arbx.test.mainnet-deployer"));

    address internal deployer;
    address internal multisig;
    address internal aavePool;

    ArbitrageExecutor internal ae;
    AllowanceManager internal am;
    FlashLoanExecutor internal fl;
    AdminTimelock internal tl;

    function setUp() public {
        deployer = vm.addr(DEPLOYER_KEY);
        multisig = makeAddr("multisig");
        aavePool = makeAddr("aavePool");

        // The script requires multisig + Aave pool to be contracts (code.length > 0).
        // Any nonempty runtime bytecode satisfies the gate; neither is called in-test.
        vm.etch(multisig, hex"01");
        vm.etch(aavePool, hex"01");

        // Satisfy the script's env / chain / balance safety gates.
        vm.setEnv("CONFIRM_MAINNET_DEPLOY", "true");
        vm.setEnv("DEPLOYER_PRIVATE_KEY", vm.toString(DEPLOYER_KEY));
        vm.setEnv("MULTISIG_ADDRESS", vm.toString(multisig));
        vm.setEnv("AAVE_V3_POOL", vm.toString(aavePool));
        vm.chainId(1);
        vm.deal(deployer, 1 ether);

        // Run the REAL deploy script.  It does not expose the proxy addresses,
        // so recover them from the ERC1967 Upgraded events each proxy emits at
        // construction — in deploy order: AE, AM, FL, TL.
        vm.recordLogs();
        new DeployMainnet().run();
        Vm.Log[] memory logs = vm.getRecordedLogs();

        address[4] memory proxies;
        uint256 found;
        for (uint256 i = 0; i < logs.length; i++) {
            if (logs[i].topics[0] == UPGRADED_TOPIC) {
                require(found < 4, unicode"more than 4 proxy deployments — script drifted, re-sync test");
                proxies[found] = logs[i].emitter;
                found++;
            }
        }
        assertEq(found, 4, "expected exactly 4 ERC1967 proxies (AE, AM, FL, TL)");

        ae = ArbitrageExecutor(payable(proxies[0]));
        am = AllowanceManager(payable(proxies[1]));
        fl = FlashLoanExecutor(payable(proxies[2]));
        tl = AdminTimelock(payable(proxies[3]));

        // Sanity-pin the deploy order assumed above:
        //  - the script grants AE.EXECUTOR_ROLE to the FlashLoanExecutor proxy (SC-13)
        //  - the timelock was initialized with minDelay 24h and multisig as proposer
        assertTrue(ae.hasRole(ae.EXECUTOR_ROLE(), address(fl)), "proxy order drift: AE/FL mismatch");
        assertEq(tl.getMinDelay(), MIN_DELAY, "proxy order drift: TL mismatch");
        assertTrue(tl.hasRole(tl.PROPOSER_ROLE(), multisig), "multisig must be timelock proposer");
    }

    // -----------------------------------------------------------------------
    // P0 assertion 1: deployer must NOT hold UPGRADER_ROLE on any UUPS contract.
    // Pre-fix this failed on all three — the drain vector.
    // -----------------------------------------------------------------------
    function testP0_Deployer_HasNoUpgraderRole_AllThree() public view {
        assertFalse(ae.hasRole(ae.UPGRADER_ROLE(), deployer), "AE: deployer still holds UPGRADER_ROLE");
        assertFalse(am.hasRole(am.UPGRADER_ROLE(), deployer), "AM: deployer still holds UPGRADER_ROLE");
        assertFalse(fl.hasRole(fl.UPGRADER_ROLE(), deployer), "FL: deployer still holds UPGRADER_ROLE");
    }

    // -----------------------------------------------------------------------
    // P0 assertion 2: the timelock must hold UPGRADER_ROLE on all three, so
    // upgrades remain possible — but only via multisig + 24h delay.
    // -----------------------------------------------------------------------
    function testP0_Timelock_HoldsUpgraderRole_AllThree() public view {
        assertTrue(ae.hasRole(ae.UPGRADER_ROLE(), address(tl)), "AE: timelock lacks UPGRADER_ROLE");
        assertTrue(am.hasRole(am.UPGRADER_ROLE(), address(tl)), "AM: timelock lacks UPGRADER_ROLE");
        assertTrue(fl.hasRole(fl.UPGRADER_ROLE(), address(tl)), "FL: timelock lacks UPGRADER_ROLE");
    }

    // -----------------------------------------------------------------------
    // Regression pins for the part of the handoff that already worked (M10):
    // DEFAULT_ADMIN_ROLE moved to the timelock and revoked from the deployer.
    // -----------------------------------------------------------------------
    function testHandoff_Deployer_HasNoAdminRole_AllThree() public view {
        assertFalse(ae.hasRole(ae.DEFAULT_ADMIN_ROLE(), deployer), "AE: deployer still holds DEFAULT_ADMIN_ROLE");
        assertFalse(am.hasRole(am.DEFAULT_ADMIN_ROLE(), deployer), "AM: deployer still holds DEFAULT_ADMIN_ROLE");
        assertFalse(fl.hasRole(fl.DEFAULT_ADMIN_ROLE(), deployer), "FL: deployer still holds DEFAULT_ADMIN_ROLE");
    }

    function testHandoff_Timelock_HoldsAdminRole_AllThree() public view {
        assertTrue(ae.hasRole(ae.DEFAULT_ADMIN_ROLE(), address(tl)), "AE: timelock lacks DEFAULT_ADMIN_ROLE");
        assertTrue(am.hasRole(am.DEFAULT_ADMIN_ROLE(), address(tl)), "AM: timelock lacks DEFAULT_ADMIN_ROLE");
        assertTrue(fl.hasRole(fl.DEFAULT_ADMIN_ROLE(), address(tl)), "FL: timelock lacks DEFAULT_ADMIN_ROLE");
    }

    // -----------------------------------------------------------------------
    // P0 assertion 3: a post-handoff upgradeToAndCall from the deployer must
    // REVERT on all three contracts.  Pre-fix this call SUCCEEDED instantly —
    // the exact bypass of multisig + timelock this suite exists to prevent.
    // -----------------------------------------------------------------------
    function testP0_DeployerUpgrade_Reverts_ArbitrageExecutor() public {
        ArbitrageExecutor newImpl = new ArbitrageExecutor();
        bytes32 role = ae.UPGRADER_ROLE();
        vm.expectRevert(abi.encodeWithSelector(ACL_UNAUTHORIZED, deployer, role));
        vm.prank(deployer);
        ae.upgradeToAndCall(address(newImpl), "");
    }

    function testP0_DeployerUpgrade_Reverts_AllowanceManager() public {
        AllowanceManager newImpl = new AllowanceManager();
        bytes32 role = am.UPGRADER_ROLE();
        vm.expectRevert(abi.encodeWithSelector(ACL_UNAUTHORIZED, deployer, role));
        vm.prank(deployer);
        am.upgradeToAndCall(address(newImpl), "");
    }

    function testP0_DeployerUpgrade_Reverts_FlashLoanExecutor() public {
        FlashLoanExecutor newImpl = new FlashLoanExecutor();
        bytes32 role = fl.UPGRADER_ROLE();
        vm.expectRevert(abi.encodeWithSelector(ACL_UNAUTHORIZED, deployer, role));
        vm.prank(deployer);
        fl.upgradeToAndCall(address(newImpl), "");
    }

    // -----------------------------------------------------------------------
    // Positive control: the legitimate upgrade path (multisig schedules on the
    // timelock, waits minDelay, executes) must still work post-handoff.
    // Without this, the "fix" could silently brick upgrades instead.
    // -----------------------------------------------------------------------
    function testUpgradeViaTimelock_StillWorks() public {
        ArbitrageExecutor newImpl = new ArbitrageExecutor();
        bytes memory upgradeCall = abi.encodeCall(ae.upgradeToAndCall, (address(newImpl), ""));
        bytes32 salt = keccak256("role-custody-upgrade");

        vm.prank(multisig);
        tl.schedule(address(ae), 0, upgradeCall, bytes32(0), salt, MIN_DELAY);

        vm.warp(block.timestamp + MIN_DELAY + 1);

        vm.prank(multisig);
        tl.execute(address(ae), 0, upgradeCall, bytes32(0), salt);

        assertEq(
            address(uint160(uint256(vm.load(address(ae), ERC1967_IMPL_SLOT)))),
            address(newImpl),
            "timelock-driven upgrade must land the new implementation"
        );
    }
}
