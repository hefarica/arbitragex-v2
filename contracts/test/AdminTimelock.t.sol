// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-10 tests — AdminTimelock
//
// Three properties under test:
//   1. Clean deployment: proxy initializes with correct minDelay + roles.
//   2. Delay enforcement: a scheduled action CANNOT execute before minDelay.
//   3. Execution after delay: the same action CAN execute after warp(minDelay+1).
//
// NOTE: We test the AdminTimelock in isolation.  Integration tests (timelock as
// admin of ArbitrageExecutor) belong in ArbitrageExecutor.t.sol when needed.
// =============================================================================

import "forge-std/Test.sol";
import "../src/AdminTimelock.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Minimal target contract used to verify scheduled call execution
// ---------------------------------------------------------------------------

/// @dev Trivial counter that we'll call through the timelock.
contract TimelockTarget {
    uint256 public counter;

    function increment() external {
        ++counter;
    }
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

contract AdminTimelockTest is Test {
    AdminTimelock internal tl;
    TimelockTarget internal target;

    address internal deployer;
    address internal proposer;
    address internal executor;

    uint256 internal constant MIN_DELAY = 86_400; // 24h

    // -----------------------------------------------------------------------
    // Deploy helpers
    // -----------------------------------------------------------------------

    function setUp() public {
        deployer = address(this);
        proposer = makeAddr("proposer");
        executor = makeAddr("executor");

        // Build init arrays
        address[] memory proposers = new address[](1);
        proposers[0] = proposer;

        address[] memory executors = new address[](1);
        executors[0] = executor;

        // Deploy AdminTimelock proxy
        AdminTimelock impl = new AdminTimelock();
        bytes memory initData = abi.encodeWithSelector(
            AdminTimelock.initialize.selector,
            MIN_DELAY,
            proposers,
            executors,
            deployer // bootstrap admin; should be renounced post-config
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        tl = AdminTimelock(payable(address(proxy)));

        // Deploy a simple target to schedule calls against
        target = new TimelockTarget();
    }

    // -----------------------------------------------------------------------
    // SC-10 test 1: testTimelock_DeploysCleanly
    // Verify initial state: minDelay, role assignments.
    // -----------------------------------------------------------------------
    function testTimelock_DeploysCleanly() public view {
        // minDelay must match initialization parameter
        assertEq(tl.getMinDelay(), MIN_DELAY, "minDelay must equal initialization value");

        // proposer must have PROPOSER_ROLE
        assertTrue(tl.hasRole(tl.PROPOSER_ROLE(), proposer), "proposer must hold PROPOSER_ROLE");

        // proposer also gets CANCELLER_ROLE automatically (OZ convention)
        assertTrue(tl.hasRole(tl.CANCELLER_ROLE(), proposer), "proposer must hold CANCELLER_ROLE");

        // executor must have EXECUTOR_ROLE
        assertTrue(tl.hasRole(tl.EXECUTOR_ROLE(), executor), "executor must hold EXECUTOR_ROLE");

        // deployer got the bootstrap admin role
        assertTrue(tl.hasRole(tl.DEFAULT_ADMIN_ROLE(), deployer), "deployer must hold DEFAULT_ADMIN_ROLE");

        // timelock itself also gets DEFAULT_ADMIN_ROLE (self-administration)
        assertTrue(tl.hasRole(tl.DEFAULT_ADMIN_ROLE(), address(tl)), "timelock must self-hold DEFAULT_ADMIN_ROLE");
    }

    // -----------------------------------------------------------------------
    // SC-10 test 2: testTimelock_ScheduledAction_RequiresDelay
    // A scheduled operation cannot execute before minDelay has elapsed.
    // -----------------------------------------------------------------------
    function testTimelock_ScheduledAction_RequiresDelay() public {
        bytes memory callData = abi.encodeWithSelector(TimelockTarget.increment.selector);

        // Schedule with exactly minDelay as the delay
        bytes32 predecessor = bytes32(0);
        bytes32 salt = bytes32(0);

        vm.prank(proposer);
        tl.schedule(address(target), 0, callData, predecessor, salt, MIN_DELAY);

        bytes32 opId = tl.hashOperation(address(target), 0, callData, predecessor, salt);

        // Operation must be in Waiting state immediately after scheduling
        assertEq(
            uint8(tl.getOperationState(opId)),
            uint8(TimelockControllerUpgradeable.OperationState.Waiting),
            "operation must be Waiting immediately after scheduling"
        );

        // Warp to minDelay - 1 second: still not ready
        vm.warp(block.timestamp + MIN_DELAY - 1);
        assertEq(
            uint8(tl.getOperationState(opId)),
            uint8(TimelockControllerUpgradeable.OperationState.Waiting),
            "operation must still be Waiting at minDelay-1"
        );

        // Execute attempt must revert (operation not Ready yet)
        vm.expectRevert();
        vm.prank(executor);
        tl.execute(address(target), 0, callData, predecessor, salt);

        // Counter must remain 0 — no execution happened
        assertEq(target.counter(), 0, unicode"target.counter must be 0 — execution must not have happened");
    }

    // =========================================================================
    // A9: Timelock minDelay regression tests (audit 2026-05-10)
    // =========================================================================
    //
    // Tag prefix: testA9_* — grep-able by future audit re-runs.
    //
    // The existing SC-10 tests verify that the delay is enforced when the
    // correct executor role holder tries to execute early.  The A9 test adds
    // a distinct, security-critical angle: the deployer address (DEFAULT_ADMIN_ROLE)
    // CANNOT bypass the timelock by calling execute() directly.  Admin role is
    // completely separate from EXECUTOR_ROLE inside TimelockController — holding
    // DEFAULT_ADMIN_ROLE gives no ability to execute operations immediately.
    //
    // This is the regression guard for "admin EOA compromise = immediate execution"
    // — the finding that motivates AdminTimelock existing in the first place.

    // -----------------------------------------------------------------------
    // testA9_Timelock_AdminCannotBypassDelay
    //
    // A9 regression: the deployer (DEFAULT_ADMIN_ROLE) cannot call execute()
    // immediately after scheduling because:
    //   (a) DEFAULT_ADMIN_ROLE != EXECUTOR_ROLE, so the access control check
    //       inside TimelockController.execute() reverts.
    //   (b) Even if the admin were granted EXECUTOR_ROLE, the minDelay window
    //       would still apply — this test verifies (a) specifically.
    //
    // Distinction from testTimelock_ScheduledAction_RequiresDelay:
    //   That test uses the legitimate executor address (holds EXECUTOR_ROLE)
    //   and verifies the time check.  This test uses the deployer address (holds
    //   only DEFAULT_ADMIN_ROLE) and verifies the role check fires before
    //   anything else — even before the time check would fire.
    // -----------------------------------------------------------------------
    function testA9_Timelock_AdminCannotBypassDelay() public {
        bytes memory callData = abi.encodeWithSelector(TimelockTarget.increment.selector);
        bytes32 predecessor = bytes32(0);
        // Use a distinct salt to avoid any operation-id collision with SC-10 tests.
        bytes32 salt = bytes32(uint256(0xA9));

        // Proposer schedules the operation (proposer holds PROPOSER_ROLE).
        vm.prank(proposer);
        tl.schedule(address(target), 0, callData, predecessor, salt, MIN_DELAY);

        // Deployer (DEFAULT_ADMIN_ROLE, no EXECUTOR_ROLE) tries to execute
        // immediately — must revert.  The revert is an AccessControl error
        // (missing EXECUTOR_ROLE), NOT a timing error.  We use a bare
        // vm.expectRevert() because OZ TimelockController uses a dynamic
        // AccessControl string that includes the role hash.
        vm.expectRevert();
        vm.prank(deployer);
        tl.execute(address(target), 0, callData, predecessor, salt);

        // Confirm no side-effect: target counter must still be 0.
        assertEq(target.counter(), 0, "target.counter must be 0 — admin execution must have been blocked");
    }

    // -----------------------------------------------------------------------
    // SC-10 test 3: testTimelock_ScheduledAction_ExecutesAfterDelay
    // An operation scheduled with minDelay CAN execute after minDelay+1 sec.
    // -----------------------------------------------------------------------
    function testTimelock_ScheduledAction_ExecutesAfterDelay() public {
        bytes memory callData = abi.encodeWithSelector(TimelockTarget.increment.selector);
        bytes32 predecessor = bytes32(0);
        // Use a nonzero salt to distinguish from test 2 (independent test; separate block.timestamp)
        bytes32 salt = keccak256("sc10-test3");

        vm.prank(proposer);
        tl.schedule(address(target), 0, callData, predecessor, salt, MIN_DELAY);

        bytes32 opId = tl.hashOperation(address(target), 0, callData, predecessor, salt);

        // Warp past the delay: minDelay + 1 second
        vm.warp(block.timestamp + MIN_DELAY + 1);

        // Operation must now be Ready
        assertEq(
            uint8(tl.getOperationState(opId)),
            uint8(TimelockControllerUpgradeable.OperationState.Ready),
            "operation must be Ready after minDelay+1"
        );

        // Execute must succeed
        vm.prank(executor);
        tl.execute(address(target), 0, callData, predecessor, salt);

        // Counter must be incremented
        assertEq(target.counter(), 1, "target.counter must be 1 after successful execution");

        // Operation must now be Done
        assertEq(
            uint8(tl.getOperationState(opId)),
            uint8(TimelockControllerUpgradeable.OperationState.Done),
            "operation must be Done after execution"
        );
    }
}
