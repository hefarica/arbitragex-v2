// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/FlashLoanExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

/// @dev Mock ERC20 for flash loan asset
contract MockERC20FL is ERC20 {
    constructor() ERC20("FLToken", "FLT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Mock Balancer Vault: simulates the minimal Balancer V2 flash loan
///      callback sequence (transfer tokens -> call receiveFlashLoan -> collect repay).
///      Used in A4 tests to verify the vault-sender check works end-to-end.
contract MockBalancerVault {
    /// @notice Calls receiveFlashLoan on `receiver` with the given params,
    ///         mimicking the real Balancer V2 Vault callback.
    function triggerFlashLoan(address receiver, IERC20 token, uint256 amount, bytes calldata userData) external {
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = token;
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0;

        FlashLoanExecutor(receiver).receiveFlashLoan(tokens, amounts, feeAmounts, userData);
    }
}

/// @dev Mock Aave V3 Pool: records calls, does not move funds
contract MockAavePool {
    address public lastReceiver;
    address public lastAsset;
    uint256 public lastAmount;
    bytes public lastParams;
    uint16 public lastReferralCode;

    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 _referralCode
    ) external {
        lastReceiver = receiverAddress;
        lastAsset = asset;
        lastAmount = amount;
        lastParams = params;
        lastReferralCode = _referralCode;
        // In a real Aave, the pool would call executeOperation.
        // We do NOT simulate the callback here - tested separately.
    }
}

/// @dev Mock ArbitrageExecutor: records that it was called and succeeds
contract MockArbitrageExecutor {
    bool public wasCalled;

    fallback() external {
        wasCalled = true;
    }
}

/// @dev Minimal V2 implementation for upgrade state-preservation test.
contract FlashLoanExecutorV2 is FlashLoanExecutor {
    function version() external pure returns (string memory) {
        return "v2";
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

contract FlashLoanExecutorTest is Test {
    FlashLoanExecutor internal flashExec;
    MockAavePool internal pool;
    MockERC20FL internal token;
    MockArbitrageExecutor internal arbExec;
    MockBalancerVault internal mockVault; // A4 (audit 2026-05-10)

    address internal admin;
    address internal executorRole;
    address internal attacker;

    function setUp() public {
        admin = address(this);
        executorRole = makeAddr("executorRole");
        attacker = makeAddr("attacker");

        pool = new MockAavePool();
        token = new MockERC20FL();
        arbExec = new MockArbitrageExecutor();
        mockVault = new MockBalancerVault(); // A4: canonical mock Balancer Vault

        // SC-08: deploy via ERC1967Proxy with initialize() call
        // FlashLoanExecutor.initialize takes (admin, aavePool, arbitrageExecutor)
        FlashLoanExecutor impl = new FlashLoanExecutor();
        bytes memory initData =
            abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, admin, address(pool), address(arbExec));
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        flashExec = FlashLoanExecutor(address(proxy));

        // Grant EXECUTOR_ROLE so requestFlashLoan can be called
        flashExec.grantRole(flashExec.EXECUTOR_ROLE(), executorRole);

        // A4 (audit 2026-05-10): configure the authorised Balancer Vault address.
        // Tests that call receiveFlashLoan must set this before attempting the callback.
        flashExec.setBalancerVault(address(mockVault));

        // Mint tokens to flash loan executor to cover repayment in the callback test
        token.mint(address(flashExec), 10_000e18);
    }

    // -----------------------------------------------------------------------
    // testReceiveFlashLoan_Authorized
    // Simulates Aave calling executeOperation from the authorized pool address.
    // The callback must not revert and must return true.
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_Authorized() public {
        uint256 loanAmount = 1_000e18;
        uint256 premium = 1e18; // 0.1% fee
        bytes memory params = abi.encodeWithSignature("noop()");

        // Simulate Aave pool calling the callback
        vm.prank(address(pool));
        bool result = flashExec.executeOperation(
            address(token),
            loanAmount,
            premium,
            address(flashExec), // initiator must be this contract
            params
        );

        assertTrue(result, "executeOperation must return true for authorized caller");
    }

    // -----------------------------------------------------------------------
    // testReceiveFlashLoan_Balancer_HappyPath_RepaysVault
    //
    // Balancer V2 callback happy path (0% fee): called by the authorised Vault, the
    // executor invokes the arbitrageExecutor and repays the loan to the Vault. Closes
    // the gap noted in the audit (FlashLoanExecutor.t.sol had only revert-path
    // receiveFlashLoan tests, no success round-trip).
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_Balancer_HappyPath_RepaysVault() public {
        uint256 amount = 1_000e18;

        // receiveFlashLoan's Layer-3 guard requires a provider to be configured.
        flashExec.setFlashLoanProvider(address(mockVault));

        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(address(token));
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0; // Balancer V2 = 0 fee

        uint256 vaultBefore = token.balanceOf(address(mockVault));

        // Called by the authorised Balancer Vault (msg.sender == balancerVault).
        vm.prank(address(mockVault));
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");

        assertTrue(arbExec.wasCalled(), "arbitrageExecutor must be invoked");
        assertEq(token.balanceOf(address(mockVault)) - vaultBefore, amount, "vault must be repaid the loan");
    }

    // -----------------------------------------------------------------------
    // testReceiveFlashLoan_RejectsUnauthorized
    // SC-3: expect FL_UnauthorizedCaller custom error instead of string
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_RejectsUnauthorized() public {
        uint256 loanAmount = 1_000e18;
        uint256 premium = 1e18;
        bytes memory params = "";

        // SC-3: custom error replaces string "Caller must be AavePool"
        vm.expectRevert(FL_UnauthorizedCaller.selector);

        vm.prank(attacker);
        flashExec.executeOperation(
            address(token),
            loanAmount,
            premium,
            address(flashExec), // even with correct initiator - caller is wrong
            params
        );
    }

    // -----------------------------------------------------------------------
    // testReceiveFlashLoan_RejectsInvalidInitiator
    // SC-3: expect FL_InvalidInitiator custom error (new coverage)
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_RejectsInvalidInitiator() public {
        uint256 loanAmount = 1_000e18;
        uint256 premium = 1e18;
        bytes memory params = "";

        vm.expectRevert(FL_InvalidInitiator.selector);

        vm.prank(address(pool));
        flashExec.executeOperation(
            address(token),
            loanAmount,
            premium,
            attacker, // wrong initiator
            params
        );
    }

    // -----------------------------------------------------------------------
    // testEvent_FlashLoanRequested_Emitted
    // SC-06: requestFlashLoan emits FlashLoanRequested with correct args.
    // The MockAavePool records but does NOT call executeOperation, so only
    // this event fires.
    // -----------------------------------------------------------------------
    function testEvent_FlashLoanRequested_Emitted() public {
        uint256 loanAmount = 500e18;
        bytes memory params = abi.encode(uint256(42)); // arbitrary payload

        bytes32 expectedHash = keccak256(params);

        vm.expectEmit(true, false, false, true, address(flashExec));
        emit FlashLoanExecutor.FlashLoanRequested(address(token), loanAmount, expectedHash);

        vm.prank(executorRole);
        flashExec.requestFlashLoan(address(token), loanAmount, params);

        // Also verify the pool received the correct forwarded arguments
        assertEq(pool.lastAsset(), address(token), "Pool: wrong asset");
        assertEq(pool.lastAmount(), loanAmount, "Pool: wrong amount");
    }

    // -----------------------------------------------------------------------
    // testEvent_FlashLoanExecuted_Emitted
    // SC-06: executeOperation (authorized + arbitrage succeeds) emits
    // FlashLoanExecuted with success=true before returning.
    // -----------------------------------------------------------------------
    function testEvent_FlashLoanExecuted_Emitted() public {
        uint256 loanAmount = 1_000e18;
        uint256 premium = 9e17; // ~0.09%
        bytes memory params = abi.encodeWithSignature("noop()");

        vm.expectEmit(true, false, false, true, address(flashExec));
        emit FlashLoanExecutor.FlashLoanExecuted(address(token), loanAmount, premium, true);

        vm.prank(address(pool));
        bool result = flashExec.executeOperation(address(token), loanAmount, premium, address(flashExec), params);

        assertTrue(result, "executeOperation must return true");
    }

    // -----------------------------------------------------------------------
    // SC-8: testSetReferralCode_UpdatesStorage
    // Admin can change referralCode; event is emitted; pool receives new code.
    // -----------------------------------------------------------------------
    function testSetReferralCode_UpdatesStorage() public {
        // Default is 0 after initialize
        assertEq(flashExec.referralCode(), 0, "initial referralCode must be 0");

        // Update to a non-zero code
        uint16 newCode = 42;

        vm.expectEmit(false, false, false, true, address(flashExec));
        emit FlashLoanExecutor.ReferralCodeUpdated(newCode);

        flashExec.setReferralCode(newCode);
        assertEq(flashExec.referralCode(), newCode, "referralCode must be updated");
    }

    // -----------------------------------------------------------------------
    // SC-8: testSetReferralCode_OnlyAdmin
    // Non-admin cannot change referralCode.
    // -----------------------------------------------------------------------
    function testSetReferralCode_OnlyAdmin() public {
        vm.expectRevert();
        vm.prank(attacker);
        flashExec.setReferralCode(99);
    }

    // -----------------------------------------------------------------------
    // SC-8: testRequestFlashLoan_UsesConfiguredReferralCode
    // After setReferralCode(55), pool.lastReferralCode must equal 55.
    // -----------------------------------------------------------------------
    function testRequestFlashLoan_UsesConfiguredReferralCode() public {
        flashExec.setReferralCode(55);

        vm.prank(executorRole);
        flashExec.requestFlashLoan(address(token), 100e18, "");

        assertEq(pool.lastReferralCode(), 55, "pool must receive the configured referral code");
    }

    // -----------------------------------------------------------------------
    // Malicious init: the proxy initializes exactly once, and the bare
    // implementation is permanently locked by the constructor's
    // _disableInitializers(). Both must revert on a second / direct initialize.
    // -----------------------------------------------------------------------
    function testInit_DoubleInitializeReverts() public {
        vm.expectRevert(); // Initializable: InvalidInitialization
        flashExec.initialize(admin, address(pool), address(arbExec));
    }

    function testInit_BareImplementationIsLocked() public {
        FlashLoanExecutor impl = new FlashLoanExecutor();
        vm.expectRevert(); // _disableInitializers() in the constructor locks the impl
        impl.initialize(admin, address(pool), address(arbExec));
    }

    // -----------------------------------------------------------------------
    // receiveFlashLoan array edges. The callback reads tokens[0]/amounts[0]/
    // feeAmounts[0] after authenticating the Vault. Empty or shorter arrays
    // therefore revert on out-of-bounds access; a multi-element array is silently
    // TRUNCATED to element 0 (length is not validated - a length==1 guard is a
    // deferred src hardening, documented here so the truncation is not mistaken
    // for multi-asset support).
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_EmptyArraysRevert() public {
        flashExec.setFlashLoanProvider(address(mockVault)); // satisfy Layer-3 guard
        IERC20[] memory tokens = new IERC20[](0);
        uint256[] memory amounts = new uint256[](0);
        uint256[] memory feeAmounts = new uint256[](0);
        vm.prank(address(mockVault));
        vm.expectRevert(); // out-of-bounds: tokens[0] on an empty array
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");
    }

    function testReceiveFlashLoan_ShorterAmountsArrayReverts() public {
        flashExec.setFlashLoanProvider(address(mockVault));
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(address(token));
        uint256[] memory amounts = new uint256[](0); // mismatched length
        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0;
        vm.prank(address(mockVault));
        vm.expectRevert(); // out-of-bounds: amounts[0] on an empty array
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");
    }

    function testReceiveFlashLoan_MultiElementArrayProcessesOnlyFirst() public {
        flashExec.setFlashLoanProvider(address(mockVault));
        uint256 first = 1_000e18;
        uint256 second = 2_000e18;

        IERC20[] memory tokens = new IERC20[](2);
        tokens[0] = IERC20(address(token));
        tokens[1] = IERC20(address(token));
        uint256[] memory amounts = new uint256[](2);
        amounts[0] = first;
        amounts[1] = second;
        uint256[] memory feeAmounts = new uint256[](2);
        feeAmounts[0] = 0;
        feeAmounts[1] = 0;

        uint256 vaultBefore = token.balanceOf(address(mockVault));
        vm.prank(address(mockVault));
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");

        // Only element 0 is processed: the Vault is repaid `first`, never first+second.
        assertTrue(arbExec.wasCalled(), "arbitrageExecutor invoked once");
        assertEq(token.balanceOf(address(mockVault)) - vaultBefore, first, "only amounts[0] repaid (rest truncated)");
    }

    // -----------------------------------------------------------------------
    // SC-08: testUpgrade_OnlyUpgrader_CanUpgrade
    // A non-UPGRADER_ROLE address must not be able to upgrade the proxy.
    // -----------------------------------------------------------------------
    function testUpgrade_OnlyUpgrader_CanUpgrade() public {
        FlashLoanExecutorV2 newImpl = new FlashLoanExecutorV2();

        // attacker has no UPGRADER_ROLE - upgradeToAndCall must revert
        vm.expectRevert();
        vm.prank(attacker);
        flashExec.upgradeToAndCall(address(newImpl), "");
    }

    // -----------------------------------------------------------------------
    // SC-08: testUpgrade_PreservesState
    // Deploy proxy, upgrade to V2 impl, verify aavePool + arbitrageExecutor preserved.
    // Also verifies slot 4 (balancerVault) survives the upgrade (A4, 2026-05-10).
    // -----------------------------------------------------------------------
    function testUpgrade_PreservesState() public {
        // Confirm state set in initialize() is present
        assertEq(address(flashExec.aavePool()), address(pool), "aavePool must match before upgrade");
        assertEq(flashExec.arbitrageExecutor(), address(arbExec), "arbitrageExecutor must match before upgrade");
        assertEq(flashExec.referralCode(), 0, "referralCode must be 0 before upgrade");
        // A4: balancerVault was set in setUp
        assertEq(flashExec.balancerVault(), address(mockVault), "balancerVault must match before upgrade");

        // Set a non-default referral code to verify slot 2 survives
        flashExec.setReferralCode(77);

        // Deploy V2 implementation
        FlashLoanExecutorV2 newImpl = new FlashLoanExecutorV2();

        // Admin (address(this)) has UPGRADER_ROLE - upgrade must succeed
        flashExec.upgradeToAndCall(address(newImpl), "");

        // Cast proxy to V2 to verify new function accessible
        FlashLoanExecutorV2 flashExecV2 = FlashLoanExecutorV2(address(flashExec));
        assertEq(flashExecV2.version(), "v2", "V2 marker function must be accessible after upgrade");

        // Storage slots 0, 1, 2, and 4 must be intact
        assertEq(address(flashExecV2.aavePool()), address(pool), "aavePool must survive upgrade");
        assertEq(flashExecV2.arbitrageExecutor(), address(arbExec), "arbitrageExecutor must survive upgrade");
        assertEq(flashExecV2.referralCode(), 77, "referralCode must survive upgrade");
        assertEq(flashExecV2.balancerVault(), address(mockVault), "balancerVault must survive upgrade (slot 4)");
    }

    // -----------------------------------------------------------------------
    // A4 (audit 2026-05-10): testReceiveFlashLoan_RevertsWhenVaultNotSet
    // receiveFlashLoan must revert with FL_BalancerVaultNotSet when
    // balancerVault == address(0), preventing spoofed callbacks before
    // the operator has configured a trusted vault.
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_RevertsWhenVaultNotSet() public {
        // Clear the vault set in setUp - simulate unconfigured state
        flashExec.setBalancerVault(address(0));

        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(address(token));
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1_000e18;
        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0;

        vm.expectRevert(FL_BalancerVaultNotSet.selector);
        // Call from attacker - irrelevant, vault-not-set is checked first
        vm.prank(attacker);
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");
    }

    // -----------------------------------------------------------------------
    // A4 (audit 2026-05-10): testReceiveFlashLoan_RevertsOnUnauthorizedSender
    // receiveFlashLoan must revert with FL_UnauthorizedCaller when called
    // from any address that is NOT the configured balancerVault, even if
    // balancerVault is set.  An attacker calling directly cannot spoof the
    // callback and trigger ArbitrageExecutor with loan-scale token approvals.
    // -----------------------------------------------------------------------
    function testReceiveFlashLoan_RevertsOnUnauthorizedSender() public {
        // balancerVault is already set to address(mockVault) in setUp.
        // Call from attacker - must be rejected regardless of token/amount.
        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(address(token));
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1_000e18;
        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0;

        vm.expectRevert(FL_UnauthorizedCaller.selector);
        vm.prank(attacker);
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");
    }

    // -----------------------------------------------------------------------
    // A4 (audit 2026-05-10): testSetBalancerVault_EmitsEvent
    // setBalancerVault must emit BalancerVaultUpdated with previous and new
    // vault addresses. Admin-only enforcement is verified implicitly via
    // the DEFAULT_ADMIN_ROLE path (admin = address(this) in setUp).
    // -----------------------------------------------------------------------
    function testSetBalancerVault_EmitsEvent() public {
        address newVault = makeAddr("newVault");
        address prevVault = address(mockVault); // set in setUp

        vm.expectEmit(true, true, false, false, address(flashExec));
        emit FlashLoanExecutor.BalancerVaultUpdated(prevVault, newVault);

        flashExec.setBalancerVault(newVault);
        assertEq(flashExec.balancerVault(), newVault, "balancerVault must equal newVault after update");
    }

    // -----------------------------------------------------------------------
    // A4 (audit 2026-05-10): testSetBalancerVault_OnlyAdmin
    // Non-admin must not be able to change balancerVault.
    // -----------------------------------------------------------------------
    function testSetBalancerVault_OnlyAdmin() public {
        vm.expectRevert();
        vm.prank(attacker);
        flashExec.setBalancerVault(address(0));
    }

    // =========================================================================
    // A9: Flash-loan callback authorization regression tests (audit 2026-05-10)
    // =========================================================================
    //
    // Tag prefix: testA9_* - grep-able by future audit re-runs.
    //
    // A4 tests cover the same logical invariants. A9 tests add:
    //   1. Explicit A9 tag for traceability in future audit matrices.
    //   2. A distinct angle: Layer 3 guard (flashLoanProvider unset while vault
    //      IS set) - not covered by any existing test.  An attacker who can set
    //      balancerVault but cannot set flashLoanProvider is still blocked.

    // -----------------------------------------------------------------------
    // testA9_ReceiveFlashLoan_RejectsWhenBalancerVaultUnset
    //
    // A9 regression (Layer 1 guard): if balancerVault == address(0) then
    // receiveFlashLoan reverts with FL_BalancerVaultNotSet regardless of the
    // caller, preventing any callback before the operator has configured a
    // trusted Vault.  This mirrors testReceiveFlashLoan_RevertsWhenVaultNotSet
    // (A4) with an explicit A9 tag so audit matrices can cross-reference both.
    // -----------------------------------------------------------------------
    function testA9_ReceiveFlashLoan_RejectsWhenBalancerVaultUnset() external {
        // Clear the vault that was set in setUp.
        flashExec.setBalancerVault(address(0));

        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(address(token));
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1_000e18;
        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0;

        vm.expectRevert(FL_BalancerVaultNotSet.selector);
        // Caller identity is irrelevant - Layer 1 fires before the sender check.
        vm.prank(attacker);
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");
    }

    // -----------------------------------------------------------------------
    // testA9_ReceiveFlashLoan_RejectsWhenSenderIsNotBalancerVault
    //
    // A9 regression (Layer 2 guard): even with balancerVault properly set,
    // a call from any address other than the configured vault reverts with
    // FL_UnauthorizedCaller.  Mirrors testReceiveFlashLoan_RevertsOnUnauthorized-
    // Sender (A4) with explicit A9 tag.  Verifies EOA attacker cannot spoof the
    // Balancer callback regardless of token/amount values passed.
    // -----------------------------------------------------------------------
    function testA9_ReceiveFlashLoan_RejectsWhenSenderIsNotBalancerVault() external {
        // balancerVault is already set to address(mockVault) in setUp.
        // Use a distinct attacker address to make the wrong-sender intent explicit.
        address wrongSender = makeAddr("A9_wrongSender");

        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(address(token));
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 5_000e18;
        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0;

        vm.expectRevert(FL_UnauthorizedCaller.selector);
        vm.prank(wrongSender);
        flashExec.receiveFlashLoan(tokens, amounts, feeAmounts, "");
    }

    // -----------------------------------------------------------------------
    // testA9_ReceiveFlashLoan_RejectsWhenFlashLoanProviderUnset
    //
    // A9 - UNIQUE ANGLE not covered by A4 tests (Layer 3 guard):
    // receiveFlashLoan has a three-layer auth check.  Layer 3 verifies that
    // flashLoanProvider != address(0), confirming the Balancer path was
    // intentionally activated.  This guards against a state where the vault IS
    // configured but the provider adapter was never set (e.g. a deploy that only
    // called setBalancerVault but not setFlashLoanProvider).
    //
    // The mock vault itself calls receiveFlashLoan so msg.sender == balancerVault
    // (Layer 2 passes).  The provider check is Layer 3 and must fire.
    // -----------------------------------------------------------------------
    function testA9_ReceiveFlashLoan_RejectsWhenFlashLoanProviderUnset() external {
        // Precondition: vault is set (setUp), but provider is NOT set
        // (flashLoanProvider defaults to address(0) after initialize).
        assertEq(flashExec.flashLoanProvider(), address(0), "flashLoanProvider must be unset for this test");
        assertNotEq(flashExec.balancerVault(), address(0), "balancerVault must be set for Layer 2 to pass");

        // The MockBalancerVault sends the callback as itself - passes Layer 2.
        // Layer 3 (flashLoanProvider == address(0)) must fire FL_NoProviderConfigured.
        vm.expectRevert(FL_NoProviderConfigured.selector);
        mockVault.triggerFlashLoan(address(flashExec), IERC20(address(token)), 1_000e18, "");
    }

    // =========================================================================
    // Decision-free hardening (re-discovered post-burst): the requestFlashLoan
    // EXECUTOR_ROLE gate + revoke lifecycle + referralCode max boundary. The
    // access gate had ZERO explicit revert coverage (only an invariant try/catch
    // swallow). Tests only - no src change.
    // =========================================================================

    // requestFlashLoan is onlyRole(EXECUTOR_ROLE): an attacker without the role
    // cannot trigger a flash loan - the gate rejects before any pool call.
    function testRequestFlashLoan_RevertsForNonExecutor() public {
        vm.expectRevert(); // AccessControlUnauthorizedAccount (missing EXECUTOR_ROLE)
        vm.prank(attacker);
        flashExec.requestFlashLoan(address(token), 100e18, "");
        assertEq(pool.lastAmount(), 0, "no pool call must have happened");
    }

    // EXECUTOR_ROLE revoke lifecycle: a holder can request, but once admin revokes
    // the role the same caller is blocked and the request never reaches the pool.
    function testExecutorRole_RevokeBlocksRequestFlashLoan() public {
        // executorRole (granted in setUp) can request - legacy pool path records it.
        vm.prank(executorRole);
        flashExec.requestFlashLoan(address(token), 100e18, "");
        assertEq(pool.lastAmount(), 100e18, "executor request must reach the pool");

        // Admin (DEFAULT_ADMIN_ROLE) revokes EXECUTOR_ROLE.
        flashExec.revokeRole(flashExec.EXECUTOR_ROLE(), executorRole);

        // The same caller can no longer request.
        vm.expectRevert();
        vm.prank(executorRole);
        flashExec.requestFlashLoan(address(token), 200e18, "");
        // The blocked request must not have reached the pool (lastAmount unchanged).
        assertEq(pool.lastAmount(), 100e18, "revoked caller must not reach the pool");
    }

    // The uint16 referralCode (packed into slot 1 with arbitrageExecutor) survives
    // the full legacy path at its maximum value type(uint16).max.
    function testRequestFlashLoan_MaxReferralCodeReachesPool() public {
        flashExec.setReferralCode(type(uint16).max); // admin-only

        vm.prank(executorRole);
        flashExec.requestFlashLoan(address(token), 100e18, "");

        assertEq(flashExec.referralCode(), type(uint16).max, "stored referralCode == max");
        assertEq(pool.lastReferralCode(), type(uint16).max, "max referralCode must reach the pool");
    }
}
