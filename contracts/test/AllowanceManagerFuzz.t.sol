// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-11: AllowanceManager - Foundry fuzz tests
//
// Properties under test:
//   F1. Bounded grant: any amount within cap succeeds and sets exact allowance.
//   F2. Above cap always reverts: any amount strictly above MAX_SAFE_ALLOWANCE
//       must revert AM_AboveSafeCap.
//   F3. Zero amount always reverts: grantAllowance(token, spender, 0) reverts.
//   F4. Revoke idempotency: revokeAllowance always sets allowance to 0,
//       regardless of what it was before.
//   F5. Non-admin always reverts on grant: arbitrary caller without ADMIN_ROLE
//       is always rejected.
//   F6. Non-admin always reverts on revoke: same check for revokeAllowance.
//
// Runs: controlled by foundry.toml [fuzz] runs = 256.
// =============================================================================

import "forge-std/Test.sol";
import "../src/AllowanceManager.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Mock ERC20
// ---------------------------------------------------------------------------

contract FuzzMockERC20AM is ERC20 {
    constructor() ERC20("FuzzAMToken", "FAMT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

// ---------------------------------------------------------------------------
// Fuzz test contract
// ---------------------------------------------------------------------------

contract AllowanceManagerFuzzTest is Test {
    AllowanceManager internal manager;
    FuzzMockERC20AM internal token;

    address internal admin;
    address internal spender;

    function setUp() public {
        admin = address(this);
        spender = makeAddr("spender");

        AllowanceManager impl = new AllowanceManager();
        bytes memory initData = abi.encodeWithSelector(AllowanceManager.initialize.selector, admin);
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        manager = AllowanceManager(address(proxy));

        token = new FuzzMockERC20AM();
        // Mint enough tokens to the manager so approve() calls don't fail on
        // token-side balance checks (forceApprove does not require a balance,
        // but mint gives a realistic state).
        token.mint(address(manager), type(uint128).max);
    }

    // -----------------------------------------------------------------------
    // F1: Bounded grant - any valid amount sets exact allowance
    // -----------------------------------------------------------------------
    /// @dev For any amount in [1, MAX_SAFE_ALLOWANCE], the on-chain allowance
    ///      after grantAllowance must equal that exact amount.
    function testFuzz_AM_F1_BoundedGrantSetsExactAllowance(uint256 amount) public {
        amount = bound(amount, 1, manager.MAX_SAFE_ALLOWANCE());

        manager.grantAllowance(address(token), spender, amount);

        assertEq(token.allowance(address(manager), spender), amount, "allowance must equal granted amount");
    }

    // -----------------------------------------------------------------------
    // F2: Above cap always reverts
    // -----------------------------------------------------------------------
    /// @dev Any amount strictly above MAX_SAFE_ALLOWANCE must revert.
    function testFuzz_AM_F2_AboveCapAlwaysReverts(uint256 overage) public {
        // overage ∈ [1, type(uint256).max - MAX_SAFE_ALLOWANCE]
        overage = bound(overage, 1, type(uint256).max - manager.MAX_SAFE_ALLOWANCE());
        uint256 amount = manager.MAX_SAFE_ALLOWANCE() + overage;

        vm.expectRevert(AM_AboveSafeCap.selector);
        manager.grantAllowance(address(token), spender, amount);
    }

    // -----------------------------------------------------------------------
    // F3: Zero amount always reverts
    // -----------------------------------------------------------------------
    /// @dev grantAllowance with amount=0 must always revert AM_ZeroAmount,
    ///      regardless of other parameters.
    function testFuzz_AM_F3_ZeroAmountAlwaysReverts(address arbitrarySpender) public {
        vm.assume(arbitrarySpender != address(0)); // zero spender reverts first with AM_ZeroAddress

        vm.expectRevert(AM_ZeroAmount.selector);
        manager.grantAllowance(address(token), arbitrarySpender, 0);
    }

    // -----------------------------------------------------------------------
    // F4: Revoke idempotency - always zeros the allowance
    // -----------------------------------------------------------------------
    /// @dev After revokeAllowance, the allowance is always 0, regardless of
    ///      what amount was granted beforehand (or if nothing was granted).
    function testFuzz_AM_F4_RevokeAlwaysZeros(uint256 priorAmount) public {
        priorAmount = bound(priorAmount, 0, manager.MAX_SAFE_ALLOWANCE());

        if (priorAmount > 0) {
            manager.grantAllowance(address(token), spender, priorAmount);
        }

        manager.revokeAllowance(address(token), spender);

        assertEq(token.allowance(address(manager), spender), 0, "allowance must be 0 after revoke");
    }

    // -----------------------------------------------------------------------
    // F5: Non-admin always reverts on grantAllowance
    // -----------------------------------------------------------------------
    /// @dev Any address that does not hold ADMIN_ROLE must be rejected.
    function testFuzz_AM_F5_NonAdminGrantAlwaysReverts(address caller, uint256 amount) public {
        vm.assume(caller != admin);
        vm.assume(!manager.hasRole(manager.ADMIN_ROLE(), caller));
        amount = bound(amount, 1, manager.MAX_SAFE_ALLOWANCE());

        vm.expectRevert();
        vm.prank(caller);
        manager.grantAllowance(address(token), spender, amount);
    }

    // -----------------------------------------------------------------------
    // F6: Non-admin always reverts on revokeAllowance
    // -----------------------------------------------------------------------
    function testFuzz_AM_F6_NonAdminRevokeAlwaysReverts(address caller) public {
        vm.assume(caller != admin);
        vm.assume(!manager.hasRole(manager.ADMIN_ROLE(), caller));

        // Pre-grant so there is something to revoke (tested from admin)
        manager.grantAllowance(address(token), spender, 1_000e18);

        vm.expectRevert();
        vm.prank(caller);
        manager.revokeAllowance(address(token), spender);
    }
}
