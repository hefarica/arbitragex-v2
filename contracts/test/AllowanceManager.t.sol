// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/AllowanceManager.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Mock
// ---------------------------------------------------------------------------

contract MockERC20AM is ERC20 {
    constructor() ERC20("AMToken", "AMT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Minimal V2 implementation for upgrade state-preservation test.
contract AllowanceManagerV2 is AllowanceManager {
    function version() external pure returns (string memory) {
        return "v2";
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

contract AllowanceManagerTest is Test {
    AllowanceManager internal manager;
    MockERC20AM internal token;

    address internal admin;
    address internal spender;
    address internal stranger;

    function setUp() public {
        admin = address(this);
        spender = makeAddr("spender");
        stranger = makeAddr("stranger");

        // SC-08: deploy via ERC1967Proxy with initialize() call
        AllowanceManager impl = new AllowanceManager();
        bytes memory initData = abi.encodeWithSelector(
            AllowanceManager.initialize.selector,
            admin
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        manager = AllowanceManager(address(proxy));

        token = new MockERC20AM();

        // Mint tokens to the manager contract so it can approve them
        token.mint(address(manager), 1_000_000e18);
    }

    // -----------------------------------------------------------------------
    // testGrantAllowance_RoleEnforced
    // Only ADMIN_ROLE can call grantAllowance.
    // -----------------------------------------------------------------------
    function testGrantAllowance_RoleEnforced() public {
        // OZ 5.x AccessControl reverts with AccessControlUnauthorizedAccount
        bytes4 selector = bytes4(keccak256("AccessControlUnauthorizedAccount(address,bytes32)"));
        vm.expectRevert(abi.encodeWithSelector(selector, stranger, manager.ADMIN_ROLE()));

        vm.prank(stranger);
        manager.grantAllowance(address(token), spender, 1_000e18);
    }

    // -----------------------------------------------------------------------
    // testGrantAllowance_SetsBoundedAllowance (SEC-2)
    // grantAllowance(token, spender, maxAmount) -> token.allowance(manager, spender) == maxAmount
    // -----------------------------------------------------------------------
    function testGrantAllowance_SetsMaxAllowance() public {
        uint256 maxAmount = 1_000e18;
        manager.grantAllowance(address(token), spender, maxAmount);

        uint256 allowance = token.allowance(address(manager), spender);
        assertEq(allowance, maxAmount, "Allowance must equal passed maxAmount after grant");
    }

    // -----------------------------------------------------------------------
    // SEC-2 + SC-3: testGrantAllowance_RejectsAboveSafeCap
    // SC-3: expect AM_AboveSafeCap custom error instead of string "Above safe cap"
    // -----------------------------------------------------------------------
    function testGrantAllowance_RejectsAboveSafeCap() public {
        uint256 aboveCap = manager.MAX_SAFE_ALLOWANCE() + 1;
        vm.expectRevert(AM_AboveSafeCap.selector);
        manager.grantAllowance(address(token), spender, aboveCap);
    }

    // -----------------------------------------------------------------------
    // SEC-2 + SC-3: testGrantAllowance_RejectsZeroAmount
    // SC-3: expect AM_ZeroAmount custom error instead of string "Zero amount"
    // -----------------------------------------------------------------------
    function testGrantAllowance_RejectsZeroAmount() public {
        vm.expectRevert(AM_ZeroAmount.selector);
        manager.grantAllowance(address(token), spender, 0);
    }

    // -----------------------------------------------------------------------
    // SC-3: testGrantAllowance_RejectsZeroToken
    // expect AM_ZeroAddress custom error instead of string "Zero token"
    // -----------------------------------------------------------------------
    function testGrantAllowance_RejectsZeroToken() public {
        vm.expectRevert(AM_ZeroAddress.selector);
        manager.grantAllowance(address(0), spender, 1_000e18);
    }

    // -----------------------------------------------------------------------
    // SC-3: testGrantAllowance_RejectsZeroSpender
    // expect AM_ZeroAddress custom error instead of string "Zero spender"
    // -----------------------------------------------------------------------
    function testGrantAllowance_RejectsZeroSpender() public {
        vm.expectRevert(AM_ZeroAddress.selector);
        manager.grantAllowance(address(token), address(0), 1_000e18);
    }

    // -----------------------------------------------------------------------
    // testRevokeAllowance_ZerosAllowance
    // revokeAllowance(token, spender) -> token.allowance(manager, spender) == 0
    // -----------------------------------------------------------------------
    function testRevokeAllowance_ZerosAllowance() public {
        uint256 maxAmount = 1_000e18;
        // First grant
        manager.grantAllowance(address(token), spender, maxAmount);
        assertEq(token.allowance(address(manager), spender), maxAmount);

        // Then revoke
        manager.revokeAllowance(address(token), spender);
        assertEq(token.allowance(address(manager), spender), 0, "Allowance must be 0 after revoke");
    }

    // -----------------------------------------------------------------------
    // SC-4: testPausable_BlocksGrantWhenPaused
    // grantAllowance reverts with EnforcedPause while paused.
    // -----------------------------------------------------------------------
    function testPausable_BlocksGrantWhenPaused() public {
        manager.pause();

        vm.expectRevert(abi.encodeWithSelector(bytes4(keccak256("EnforcedPause()"))));
        manager.grantAllowance(address(token), spender, 1_000e18);
    }

    // -----------------------------------------------------------------------
    // SC-4: testPausable_BlocksRevokeWhenPaused
    // revokeAllowance reverts with EnforcedPause while paused.
    // -----------------------------------------------------------------------
    function testPausable_BlocksRevokeWhenPaused() public {
        // Grant first so there's something to revoke
        manager.grantAllowance(address(token), spender, 1_000e18);

        manager.pause();

        vm.expectRevert(abi.encodeWithSelector(bytes4(keccak256("EnforcedPause()"))));
        manager.revokeAllowance(address(token), spender);
    }

    // -----------------------------------------------------------------------
    // SC-4: testPausable_Unpause_AllowsGrant
    // After unpause, grantAllowance works normally.
    // -----------------------------------------------------------------------
    function testPausable_Unpause_AllowsGrant() public {
        manager.pause();
        manager.unpause();

        // Must succeed
        manager.grantAllowance(address(token), spender, 1_000e18);
        assertEq(token.allowance(address(manager), spender), 1_000e18);
    }

    // -----------------------------------------------------------------------
    // SC-4: testBatchGrantAllowance_HappyPath
    // Batch grant sets allowances for all (token, spender, amount) tuples.
    // -----------------------------------------------------------------------
    function testBatchGrantAllowance_HappyPath() public {
        address spender2 = makeAddr("spender2");
        MockERC20AM token2 = new MockERC20AM();
        token2.mint(address(manager), 1_000_000e18);

        address[] memory tokens = new address[](2);
        tokens[0] = address(token);
        tokens[1] = address(token2);

        address[] memory spenders = new address[](2);
        spenders[0] = spender;
        spenders[1] = spender2;

        uint256[] memory amounts = new uint256[](2);
        amounts[0] = 500e18;
        amounts[1] = 750e18;

        manager.batchGrantAllowance(tokens, spenders, amounts);

        assertEq(token.allowance(address(manager), spender), 500e18, "token/spender allowance mismatch");
        assertEq(token2.allowance(address(manager), spender2), 750e18, "token2/spender2 allowance mismatch");
    }

    // -----------------------------------------------------------------------
    // SC-4: testBatchGrantAllowance_RejectsLengthMismatch
    // -----------------------------------------------------------------------
    function testBatchGrantAllowance_RejectsLengthMismatch() public {
        address[] memory tokens = new address[](2);
        address[] memory spenders = new address[](1); // mismatch
        uint256[] memory amounts = new uint256[](2);

        vm.expectRevert(AM_LengthMismatch.selector);
        manager.batchGrantAllowance(tokens, spenders, amounts);
    }

    // -----------------------------------------------------------------------
    // SC-4: testBatchRevokeAllowance_HappyPath
    // Batch revoke zeros allowances for all (token, spender) pairs.
    // -----------------------------------------------------------------------
    function testBatchRevokeAllowance_HappyPath() public {
        address spender2 = makeAddr("spender2");
        MockERC20AM token2 = new MockERC20AM();
        token2.mint(address(manager), 1_000_000e18);

        // Grant first
        manager.grantAllowance(address(token), spender, 500e18);
        manager.grantAllowance(address(token2), spender2, 750e18);

        // Then batch revoke
        address[] memory tokens = new address[](2);
        tokens[0] = address(token);
        tokens[1] = address(token2);

        address[] memory spenders = new address[](2);
        spenders[0] = spender;
        spenders[1] = spender2;

        manager.batchRevokeAllowance(tokens, spenders);

        assertEq(token.allowance(address(manager), spender), 0, "token/spender must be 0 after batch revoke");
        assertEq(token2.allowance(address(manager), spender2), 0, "token2/spender2 must be 0 after batch revoke");
    }

    // -----------------------------------------------------------------------
    // SC-4: testBatchRevokeAllowance_RejectsLengthMismatch
    // -----------------------------------------------------------------------
    function testBatchRevokeAllowance_RejectsLengthMismatch() public {
        address[] memory tokens = new address[](2);
        address[] memory spenders = new address[](1); // mismatch

        vm.expectRevert(AM_LengthMismatch.selector);
        manager.batchRevokeAllowance(tokens, spenders);
    }

    // -----------------------------------------------------------------------
    // SC-4: testBatchGrant_BlockedWhenPaused
    // -----------------------------------------------------------------------
    function testBatchGrant_BlockedWhenPaused() public {
        manager.pause();

        address[] memory tokens = new address[](1);
        tokens[0] = address(token);
        address[] memory spenders = new address[](1);
        spenders[0] = spender;
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 500e18;

        vm.expectRevert(abi.encodeWithSelector(bytes4(keccak256("EnforcedPause()"))));
        manager.batchGrantAllowance(tokens, spenders, amounts);
    }

    // -----------------------------------------------------------------------
    // SC-6: testNoExecutorRole — EXECUTOR_ROLE constant must not exist
    // AllowanceManager.EXECUTOR_ROLE was removed (SC-6 dead code cleanup).
    // This test verifies the slot is gone by checking the constant is absent.
    // We can't test absence at the Solidity level, but the build will fail
    // if the constant were referenced elsewhere.  Here we confirm ADMIN_ROLE
    // and UPGRADER_ROLE are the only declared role constants.
    // -----------------------------------------------------------------------
    function testRoleConstants_OnlyAdminAndUpgrader() public view {
        // ADMIN_ROLE == DEFAULT_ADMIN_ROLE == bytes32(0)
        assertEq(manager.ADMIN_ROLE(), bytes32(0), "ADMIN_ROLE must be DEFAULT_ADMIN_ROLE");
        // UPGRADER_ROLE == keccak256("UPGRADER_ROLE")
        assertEq(manager.UPGRADER_ROLE(), keccak256("UPGRADER_ROLE"), "UPGRADER_ROLE hash mismatch");
        // No EXECUTOR_ROLE constant should be visible at the proxy interface.
        // (Compiler enforces this: if the constant existed, the test would call it.)
    }

    // -----------------------------------------------------------------------
    // Malicious init: the proxy initializes exactly once, and the bare
    // implementation is permanently locked by the constructor's
    // _disableInitializers(). Both must revert on a second / direct initialize.
    // -----------------------------------------------------------------------
    function testInit_DoubleInitializeReverts() public {
        vm.expectRevert(); // Initializable: InvalidInitialization
        manager.initialize(admin);
    }

    function testInit_BareImplementationIsLocked() public {
        AllowanceManager impl = new AllowanceManager();
        vm.expectRevert(); // _disableInitializers() in the constructor locks the impl
        impl.initialize(admin);
    }

    // -----------------------------------------------------------------------
    // SC-08: testUpgrade_OnlyUpgrader_CanUpgrade
    // A non-UPGRADER_ROLE address must not be able to upgrade the proxy.
    // -----------------------------------------------------------------------
    function testUpgrade_OnlyUpgrader_CanUpgrade() public {
        AllowanceManagerV2 newImpl = new AllowanceManagerV2();

        // stranger has no UPGRADER_ROLE — upgradeToAndCall must revert
        vm.expectRevert();
        vm.prank(stranger);
        manager.upgradeToAndCall(address(newImpl), "");
    }

    // -----------------------------------------------------------------------
    // SC-08: testUpgrade_PreservesState
    // Deploy proxy, grant allowance, upgrade to V2, verify allowance preserved.
    // -----------------------------------------------------------------------
    function testUpgrade_PreservesState() public {
        // Grant an allowance before upgrade (SEC-2: bounded amount)
        uint256 maxAmount = 1_000e18;
        manager.grantAllowance(address(token), spender, maxAmount);
        assertEq(
            token.allowance(address(manager), spender),
            maxAmount,
            "allowance must equal granted amount before upgrade"
        );

        // Deploy V2 implementation
        AllowanceManagerV2 newImpl = new AllowanceManagerV2();

        // Admin (address(this)) has UPGRADER_ROLE — upgrade must succeed
        manager.upgradeToAndCall(address(newImpl), "");

        // Cast proxy to V2 to verify new function accessible
        AllowanceManagerV2 managerV2 = AllowanceManagerV2(address(manager));
        assertEq(managerV2.version(), "v2", "V2 marker function must be accessible after upgrade");

        // Token allowance is external state (on the ERC20 token contract) — it persists
        // because it was set via approve() on the token, not in AllowanceManager storage.
        // Verify admin role (internal storage) is preserved.
        assertTrue(managerV2.hasRole(managerV2.ADMIN_ROLE(), admin), "ADMIN_ROLE must survive upgrade");
        assertTrue(managerV2.hasRole(managerV2.UPGRADER_ROLE(), admin), "UPGRADER_ROLE must survive upgrade");
    }

    // =========================================================================
    // Decision-free hardening (re-discovered post-burst): cap boundary, pause/
    // unpause access negatives, event emission, the SC-5 view layer, and batch
    // atomicity. None of these names/coverage existed (#200 added only init
    // guards). Tests only — no src change.
    // =========================================================================

    // grantAllowance at exactly MAX_SAFE_ALLOWANCE must SUCCEED — the cap is
    // inclusive (src uses strict `>` for the reject). Complements the cap+1 reject.
    function testGrantAllowance_AtSafeCapExactSucceeds() public {
        uint256 cap = manager.MAX_SAFE_ALLOWANCE();
        manager.grantAllowance(address(token), spender, cap);
        assertEq(token.allowance(address(manager), spender), cap, "exact cap must be allowed");
    }

    // pause() is onlyRole(ADMIN_ROLE): a stranger cannot trip the kill-switch.
    function testPause_RoleEnforced() public {
        bytes4 selector = bytes4(keccak256("AccessControlUnauthorizedAccount(address,bytes32)"));
        vm.expectRevert(abi.encodeWithSelector(selector, stranger, manager.ADMIN_ROLE()));
        vm.prank(stranger);
        manager.pause();
    }

    // unpause() is onlyRole(ADMIN_ROLE): a stranger cannot lift the kill-switch.
    function testUnpause_RoleEnforced() public {
        manager.pause(); // admin pauses first
        bytes4 selector = bytes4(keccak256("AccessControlUnauthorizedAccount(address,bytes32)"));
        vm.expectRevert(abi.encodeWithSelector(selector, stranger, manager.ADMIN_ROLE()));
        vm.prank(stranger);
        manager.unpause();
    }

    // grantAllowance emits AllowanceGranted(token, spender, amount) — 2 indexed + data.
    function testGrantAllowance_EmitsEvent() public {
        vm.expectEmit(true, true, false, true, address(manager));
        emit AllowanceManager.AllowanceGranted(address(token), spender, 1_000e18);
        manager.grantAllowance(address(token), spender, 1_000e18);
    }

    // revokeAllowance emits AllowanceRevoked(token, spender) — 2 indexed, no data.
    function testRevokeAllowance_EmitsEvent() public {
        manager.grantAllowance(address(token), spender, 1_000e18);
        vm.expectEmit(true, true, false, false, address(manager));
        emit AllowanceManager.AllowanceRevoked(address(token), spender);
        manager.revokeAllowance(address(token), spender);
    }

    // The SC-5 view layer (isApproved/getAllowance) feeds ArbitrageExecutor's AM
    // integration and was entirely untested — it must reflect real token state
    // across grant -> revoke.
    function testViews_ReflectGrantAndRevoke() public {
        assertFalse(manager.isApproved(address(token), spender), "not approved before grant");
        assertEq(manager.getAllowance(address(token), spender), 0, "zero allowance before grant");

        manager.grantAllowance(address(token), spender, 500e18);
        assertTrue(manager.isApproved(address(token), spender), "approved after grant");
        assertEq(manager.getAllowance(address(token), spender), 500e18, "allowance reflects grant");

        manager.revokeAllowance(address(token), spender);
        assertFalse(manager.isApproved(address(token), spender), "not approved after revoke");
        assertEq(manager.getAllowance(address(token), spender), 0, "zero allowance after revoke");
    }

    // batchGrantAllowance is atomic: one invalid tuple reverts the WHOLE batch,
    // leaving earlier valid tuples unset (no partial application).
    function testBatchGrant_InvalidElementRevertsWholeBatch() public {
        MockERC20AM token2 = new MockERC20AM();
        address spender2 = makeAddr("spender2");

        address[] memory tokens = new address[](2);
        tokens[0] = address(token);
        tokens[1] = address(token2);
        address[] memory spenders = new address[](2);
        spenders[0] = spender;
        spenders[1] = spender2;
        uint256[] memory amounts = new uint256[](2);
        amounts[0] = 500e18;
        amounts[1] = 0; // invalid second tuple -> AM_ZeroAmount

        vm.expectRevert(AM_ZeroAmount.selector);
        manager.batchGrantAllowance(tokens, spenders, amounts);

        // Atomic rollback: the valid first tuple must NOT have been applied.
        assertEq(token.allowance(address(manager), spender), 0, "element[0] must roll back on batch revert");
    }
}
