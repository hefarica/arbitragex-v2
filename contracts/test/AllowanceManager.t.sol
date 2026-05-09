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
    // SEC-2: testGrantAllowance_RejectsAboveSafeCap
    // grantAllowance must revert with "Above safe cap" if maxAmount > MAX_SAFE_ALLOWANCE.
    // -----------------------------------------------------------------------
    function testGrantAllowance_RejectsAboveSafeCap() public {
        uint256 aboveCap = manager.MAX_SAFE_ALLOWANCE() + 1;
        vm.expectRevert("Above safe cap");
        manager.grantAllowance(address(token), spender, aboveCap);
    }

    // -----------------------------------------------------------------------
    // SEC-2: testGrantAllowance_RejectsZeroAmount
    // grantAllowance must revert with "Zero amount" if maxAmount == 0.
    // -----------------------------------------------------------------------
    function testGrantAllowance_RejectsZeroAmount() public {
        vm.expectRevert("Zero amount");
        manager.grantAllowance(address(token), spender, 0);
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
}
