// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/AllowanceManager.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

// ---------------------------------------------------------------------------
// Mock
// ---------------------------------------------------------------------------

contract MockERC20AM is ERC20 {
    constructor() ERC20("AMToken", "AMT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
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

        manager = new AllowanceManager();
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
        manager.grantAllowance(address(token), spender);
    }

    // -----------------------------------------------------------------------
    // testGrantAllowance_SetsMaxAllowance
    // grantAllowance(token, spender) -> token.allowance(manager, spender) == type(uint256).max
    // -----------------------------------------------------------------------
    function testGrantAllowance_SetsMaxAllowance() public {
        manager.grantAllowance(address(token), spender);

        uint256 allowance = token.allowance(address(manager), spender);
        assertEq(allowance, type(uint256).max, "Allowance must be uint256.max after grant");
    }

    // -----------------------------------------------------------------------
    // testRevokeAllowance_ZerosAllowance
    // revokeAllowance(token, spender) -> token.allowance(manager, spender) == 0
    // -----------------------------------------------------------------------
    function testRevokeAllowance_ZerosAllowance() public {
        // First grant
        manager.grantAllowance(address(token), spender);
        assertEq(token.allowance(address(manager), spender), type(uint256).max);

        // Then revoke
        manager.revokeAllowance(address(token), spender);
        assertEq(token.allowance(address(manager), spender), 0, "Allowance must be 0 after revoke");
    }
}
