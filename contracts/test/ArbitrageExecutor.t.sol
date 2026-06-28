// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/AllowanceManager.sol";
import "../src/interfaces/IAllowanceManager.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

/// @dev Minimal ERC20 with public mint for test setup
contract MockERC20 is ERC20 {
    constructor() ERC20("MockToken", "MTK") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Router that mints `profitAmount` extra tokens to executor on call
contract MockProfitRouter {
    MockERC20 public token;
    address public executor;
    uint256 public profitAmount;

    constructor(address _token, address _executor, uint256 _profit) {
        token = MockERC20(_token);
        executor = _executor;
        profitAmount = _profit;
    }

    fallback() external {
        // Simulate swap profit: mint extra tokens to executor
        token.mint(executor, profitAmount);
    }
}

/// @dev Router that does nothing — balance unchanged → no gross profit
contract MockZeroProfitRouter {
    fallback() external {}
}

/// @dev Malicious router that attempts to re-enter executeArbitrage.
/// Uses only primitive-type storage to avoid nested calldata array copy
/// (which requires via_ir; we keep via_ir = false per foundry.toml).
/// The re-entrant call uses itself as the sole router with empty payload.
contract MaliciousReentrantRouter {
    ArbitrageExecutor public target;
    bytes32 public routeHash;
    address public tokenIn;
    uint256 public amountIn;
    uint256 public minProfit;

    bool private _attacking;

    constructor(address _target) {
        target = ArbitrageExecutor(payable(_target));
    }

    function setAttackParams(
        bytes32 _routeHash,
        address _tokenIn,
        uint256 _amountIn,
        uint256 _minProfit
    ) external {
        routeHash = _routeHash;
        tokenIn = _tokenIn;
        amountIn = _amountIn;
        minProfit = _minProfit;
    }

    fallback() external {
        if (!_attacking) {
            _attacking = true;
            // Build attack call in memory (avoids nested calldata → storage copy)
            address[] memory reentrantRouters = new address[](1);
            reentrantRouters[0] = address(this);

            bytes[] memory reentrantPayloads = new bytes[](1);
            reentrantPayloads[0] = "";

            // Attempt re-entry — must revert with ReentrancyGuardReentrantCall
            // tokenOut == tokenIn is valid for circular arb; used here as observability metadata
            target.executeArbitrage(routeHash, tokenIn, tokenIn, amountIn, minProfit, reentrantRouters, reentrantPayloads);
        }
    }
}

/// @dev Minimal V2 implementation for upgrade state-preservation test.
///      Adds a marker function; no new state variables.
contract ArbitrageExecutorV2 is ArbitrageExecutor {
    function version() external pure returns (string memory) {
        return "v2";
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

contract ArbitrageExecutorTest is Test {
    ArbitrageExecutor internal executor;
    MockERC20 internal token;

    address internal admin;
    address internal executorRole;
    address internal stranger;

    /// @dev Shared mock selector used across tests.
    ///      MockProfitRouter / MockZeroProfitRouter expose fallback(), which accepts
    ///      any calldata — so any 4-byte-prefixed payload is valid there.
    bytes4 internal constant MOCK_SELECTOR = bytes4(keccak256("swap()"));

    function setUp() public {
        admin = address(this);
        executorRole = makeAddr("executorRole");
        stranger = makeAddr("stranger");

        // SC-08: deploy via ERC1967Proxy with initialize() call
        ArbitrageExecutor impl = new ArbitrageExecutor();
        bytes memory initData = abi.encodeWithSelector(
            ArbitrageExecutor.initialize.selector,
            admin
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        executor = ArbitrageExecutor(payable(address(proxy)));

        token = new MockERC20();

        // Grant EXECUTOR_ROLE to executorRole account
        executor.grantRole(executor.EXECUTOR_ROLE(), executorRole);

        // Approve token
        executor.setTokenApproval(address(token), true);
    }

    // -----------------------------------------------------------------------
    // testHappyPath_ExecuteArbitrage_PositiveProfit
    // -----------------------------------------------------------------------
    function testHappyPath_ExecuteArbitrage_PositiveProfit() public {
        uint256 amountIn = 1_000e18;
        uint256 profit = 50e18;
        uint256 minProfit = 10e18;

        // Fund executor with initial balance
        token.mint(address(executor), amountIn);

        // Deploy router that mints profit to executor
        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);
        // A5: approve the selector the payload will carry.
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);

        address[] memory routers = new address[](1);
        routers[0] = address(router);

        bytes[] memory payloads = new bytes[](1);
        // A5: payload must start with the whitelisted selector (fallback() accepts any calldata).
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        // SC-05 fixed: tokenOut is now a distinct parameter. In this simple happy-path
        // we pass tokenOut == tokenIn (valid for circular arb routes). The important
        // invariant is that the event now reflects what the caller provided.
        vm.expectEmit(true, true, true, true, address(executor));
        emit ArbitrageExecutor.ArbitrageExecuted(bytes32(0), address(token), address(token), profit);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, minProfit, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testRevert_InsufficientProfit
    // SC-3: expect InsufficientProfit custom error selector instead of string
    // -----------------------------------------------------------------------
    function testRevert_InsufficientProfit() public {
        uint256 amountIn = 1_000e18;
        uint256 tinyProfit = 1; // router returns only 1 wei
        uint256 minProfit = 100e18; // but we require 100 tokens

        token.mint(address(executor), amountIn);

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), tinyProfit);
        executor.setRouterApproval(address(router), true);
        // A5: selector must be approved so the call reaches the profit check.
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);

        address[] memory routers = new address[](1);
        routers[0] = address(router);

        bytes[] memory payloads = new bytes[](1);
        // A5: payload must carry the whitelisted selector.
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        // SC-3: custom error replaces string "Slippage / Min profit guard failed"
        vm.expectRevert(InsufficientProfit.selector);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, minProfit, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testRevert_OnlyExecutor (access control)
    // SC-3: expect NotExecutor custom error selector instead of string
    // -----------------------------------------------------------------------
    function testRevert_OnlyExecutor() public {
        address[] memory routers = new address[](0);
        bytes[] memory payloads = new bytes[](0);

        // SC-3: custom error replaces string "Not executor"
        vm.expectRevert(NotExecutor.selector);

        vm.prank(stranger);
        executor.executeArbitrage(bytes32(0), address(token), address(token), 0, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testRevert_TokenNotApproved
    // SC-3: expect TokenNotApproved custom error
    // -----------------------------------------------------------------------
    function testRevert_TokenNotApproved() public {
        MockERC20 unapproved = new MockERC20();
        unapproved.mint(address(executor), 1_000e18);

        address[] memory routers = new address[](0);
        bytes[] memory payloads = new bytes[](0);

        vm.expectRevert(abi.encodeWithSelector(TokenNotApproved.selector, address(unapproved)));

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(unapproved), address(unapproved), 100e18, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testRevert_RouterNotApproved
    // SC-3: expect RouterNotApproved custom error
    // -----------------------------------------------------------------------
    function testRevert_RouterNotApproved() public {
        uint256 amountIn = 100e18;
        token.mint(address(executor), amountIn);

        address unapprovedRouter = makeAddr("unapprovedRouter");
        address[] memory routers = new address[](1);
        routers[0] = unapprovedRouter;

        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

        vm.expectRevert(abi.encodeWithSelector(RouterNotApproved.selector, unapprovedRouter));

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testRevert_LengthMismatch
    // SC-3: expect LengthMismatch custom error
    // -----------------------------------------------------------------------
    function testRevert_LengthMismatch() public {
        address[] memory routers = new address[](2);
        bytes[] memory payloads = new bytes[](1); // length mismatch

        vm.expectRevert(LengthMismatch.selector);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), 0, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testReentrancy_Blocked
    // The inner re-entrant call hits ReentrancyGuard, which causes SwapFailed
    // to propagate from the outer call (the swap call itself fails).
    //
    // A5 note: the outer payload carries MOCK_SELECTOR (4 bytes) so it passes
    // the selector check. The inner re-entrant payload (built inside
    // MaliciousReentrantRouter.fallback()) is "" (0 bytes), but the
    // ReentrancyGuard modifier fires BEFORE the function body on the inner call,
    // so it reverts with ReentrancyGuardReentrantCall before the selector check
    // is ever reached. SwapFailed is still the correct expected error.
    // -----------------------------------------------------------------------
    function testReentrancy_Blocked() public {
        uint256 amountIn = 1_000e18;

        // Deploy malicious router first (needs executor address)
        MaliciousReentrantRouter malicious = new MaliciousReentrantRouter(address(executor));
        executor.setRouterApproval(address(malicious), true);
        // A5: approve the selector so the outer call passes the whitelist gate
        //     and reaches router.call(), which triggers the re-entry attempt.
        executor.setRouterSelectorApproval(address(malicious), MOCK_SELECTOR, true);

        address[] memory attackRouters = new address[](1);
        attackRouters[0] = address(malicious);

        bytes[] memory attackPayloads = new bytes[](1);
        // A5: outer payload carries whitelisted selector (fallback accepts any calldata).
        attackPayloads[0] = abi.encodePacked(MOCK_SELECTOR);

        // Provide attack params to re-entrant router (only primitives — no bytes[] storage)
        malicious.setAttackParams(bytes32(0), address(token), amountIn, 0);

        token.mint(address(executor), amountIn);

        // The re-entrant inner call is blocked by ReentrancyGuard (ReentrancyGuardReentrantCall).
        // That revert propagates through the low-level router.call() and is caught by SwapFailed.
        // SC-3: SwapFailed custom error replaces string "Swap failed in route"
        vm.expectRevert(SwapFailed.selector);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, attackRouters, attackPayloads);
    }

    // -----------------------------------------------------------------------
    // testPausable_BlocksWhenPaused
    // -----------------------------------------------------------------------
    function testPausable_BlocksWhenPaused() public {
        // Admin pauses the contract
        executor.pause();

        address[] memory routers = new address[](0);
        bytes[] memory payloads = new bytes[](0);

        // OZ 5.x Pausable reverts with EnforcedPause() custom error
        vm.expectRevert(abi.encodeWithSelector(bytes4(keccak256("EnforcedPause()"))));

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), 0, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testEvent_ArbitrageExecuted_BugFix
    //
    // SC-05 fixed: tokenOut is now an explicit parameter. This test verifies the
    // event correctly reflects a distinct intermediate token (tokenIn != tokenOut),
    // which is the normal multi-hop case (e.g. USDC → ETH → USDC route).
    // -----------------------------------------------------------------------
    function testEvent_ArbitrageExecuted_BugFix() public {
        uint256 amountIn = 1_000e18;
        uint256 profit = 50e18;

        // tokenOut represents the intermediate token (e.g. ETH in USDC→ETH→USDC)
        MockERC20 tokenOut = new MockERC20();
        // M8 (audit 2026-05-10): when tokenOut != tokenIn the intermediate token must
        // be in approvedTokens, otherwise executeArbitrage reverts TokenNotApproved
        // before any router dispatch. (The original comment here predated M8.)
        executor.setTokenApproval(address(tokenOut), true);

        token.mint(address(executor), amountIn);

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);
        // A5: approve the selector so the call passes the whitelist gate.
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);

        address[] memory routers = new address[](1);
        routers[0] = address(router);

        bytes[] memory payloads = new bytes[](1);
        // A5: payload must carry the whitelisted selector.
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        // Verify event carries tokenIn != tokenOut — the SC-05 bug emitted tokenIn twice
        vm.expectEmit(true, true, true, true, address(executor));
        emit ArbitrageExecutor.ArbitrageExecuted(bytes32(0), address(token), address(tokenOut), profit);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(tokenOut), amountIn, profit, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testM8_ExecuteArbitrage_RevertsWhenTokenOutNotApproved
    //
    // M8 (audit 2026-05-10): when tokenOut != tokenIn, the intermediate token
    // must be in the approvedTokens set. An unapproved tokenOut must revert with
    // TokenNotApproved(tokenOut) before any router call is dispatched.
    // -----------------------------------------------------------------------
    function testM8_ExecuteArbitrage_RevertsWhenTokenOutNotApproved() public {
        MockERC20 unapprovedIntermediate = new MockERC20();
        // Deliberately: do NOT call executor.setTokenApproval(address(unapprovedIntermediate), true)

        uint256 amountIn = 1_000e18;
        token.mint(address(executor), amountIn);

        address[] memory routers = new address[](0);
        bytes[] memory payloads = new bytes[](0);

        // tokenIn (token) is approved; tokenOut (unapprovedIntermediate) is NOT.
        // Expect revert with the unapproved intermediate address.
        vm.expectRevert(abi.encodeWithSelector(TokenNotApproved.selector, address(unapprovedIntermediate)));

        vm.prank(executorRole);
        executor.executeArbitrage(
            bytes32(0),
            address(token),
            address(unapprovedIntermediate),
            amountIn,
            0,
            routers,
            payloads
        );
    }

    // -----------------------------------------------------------------------
    // testM8_ExecuteArbitrage_CircularArbTokenOutEqualsTokenIn_NoExtraApproval
    //
    // M8 (audit 2026-05-10): when tokenOut == tokenIn (circular arb default),
    // no extra approval lookup is required. The existing happy-path with a
    // single approved token must succeed without registering tokenOut separately.
    // Regression guard: this path existed before M8 and must remain unchanged.
    // -----------------------------------------------------------------------
    function testM8_ExecuteArbitrage_CircularArbTokenOutEqualsTokenIn_NoExtraApproval() public {
        uint256 amountIn = 500e18;
        uint256 profit   = 10e18;

        token.mint(address(executor), amountIn);

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);

        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        // tokenOut == tokenIn — no second approval entry needed (M8 short-circuit).
        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);

        // Verify profit landed
        assertGt(token.balanceOf(address(executor)), amountIn, "balance must exceed amountIn after profit");
    }

    // -----------------------------------------------------------------------
    // testReceiveETH_AcceptsTransfer
    // SC-07: receive() allows ETH to land in the contract without reverting.
    // -----------------------------------------------------------------------
    function testReceiveETH_AcceptsTransfer() public {
        uint256 sendAmount = 1 ether;
        vm.deal(address(this), sendAmount);

        uint256 balBefore = address(executor).balance;

        // Low-level call with value — must succeed (receive() present)
        (bool ok, ) = address(executor).call{value: sendAmount}("");
        assertTrue(ok, "ETH transfer to executor must succeed");

        assertEq(address(executor).balance, balBefore + sendAmount, "Executor ETH balance mismatch");
    }

    // -----------------------------------------------------------------------
    // testWithdrawETH_OnlyAdmin_RescuesStuckETH
    // SC-07: admin can rescue ETH; event is emitted; stranger is blocked.
    // -----------------------------------------------------------------------
    function testWithdrawETH_OnlyAdmin_RescuesStuckETH() public {
        uint256 stuckAmount = 2 ether;

        // Force ETH into the executor (simulates selfdestruct / forced send)
        vm.deal(address(executor), stuckAmount);
        assertEq(address(executor).balance, stuckAmount, "Setup: ETH not credited");

        address payable recipient = payable(makeAddr("recipient"));
        uint256 recipientBefore = recipient.balance;

        // Expect the ETHWithdrawn event
        vm.expectEmit(true, false, false, true, address(executor));
        emit ArbitrageExecutor.ETHWithdrawn(recipient, stuckAmount);

        // Admin (address(this)) has DEFAULT_ADMIN_ROLE — call succeeds
        executor.withdrawETH(recipient);

        assertEq(address(executor).balance, 0, "Executor must be drained");
        assertEq(recipient.balance, recipientBefore + stuckAmount, "Recipient did not receive ETH");

        // Non-admin is blocked
        vm.expectRevert();
        vm.prank(stranger);
        executor.withdrawETH(recipient);
    }

    // -----------------------------------------------------------------------
    // SC-08: testUpgrade_OnlyUpgrader_CanUpgrade
    // A non-UPGRADER_ROLE address must not be able to upgrade the proxy.
    // -----------------------------------------------------------------------
    function testUpgrade_OnlyUpgrader_CanUpgrade() public {
        ArbitrageExecutorV2 newImpl = new ArbitrageExecutorV2();

        // stranger has no UPGRADER_ROLE — upgradeToAndCall must revert
        vm.expectRevert();
        vm.prank(stranger);
        executor.upgradeToAndCall(address(newImpl), "");
    }

    // -----------------------------------------------------------------------
    // SC-08: testUpgrade_PreservesState
    // Deploy proxy, set approvedTokens, upgrade to V2 impl, verify state preserved.
    // -----------------------------------------------------------------------
    function testUpgrade_PreservesState() public {
        // Confirm token approval set in setUp is present
        assertTrue(executor.approvedTokens(address(token)), "token must be approved before upgrade");

        // Add an additional token to make state check unambiguous
        MockERC20 extraToken = new MockERC20();
        executor.setTokenApproval(address(extraToken), true);
        assertTrue(executor.approvedTokens(address(extraToken)), "extraToken must be approved before upgrade");

        // Deploy V2 implementation
        ArbitrageExecutorV2 newImpl = new ArbitrageExecutorV2();

        // Admin (address(this)) has UPGRADER_ROLE — upgrade must succeed
        executor.upgradeToAndCall(address(newImpl), "");

        // Cast proxy to V2 interface to verify new function is accessible
        ArbitrageExecutorV2 executorV2 = ArbitrageExecutorV2(payable(address(executor)));
        assertEq(executorV2.version(), "v2", "V2 marker function must be accessible after upgrade");

        // Storage must be intact — both token approvals survive the upgrade
        assertTrue(executorV2.approvedTokens(address(token)), "token approval must survive upgrade");
        assertTrue(executorV2.approvedTokens(address(extraToken)), "extraToken approval must survive upgrade");
    }

    // =========================================================================
    // SC-5: AllowanceManager integration tests
    // =========================================================================

    /// @dev Helper: deploy AllowanceManager proxy with `address(this)` as admin.
    function _deployAllowanceManager() internal returns (AllowanceManager am) {
        AllowanceManager impl = new AllowanceManager();
        bytes memory initData = abi.encodeWithSelector(
            AllowanceManager.initialize.selector,
            address(this)
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        am = AllowanceManager(address(proxy));
    }

    // -----------------------------------------------------------------------
    // testSC5_SetAllowanceManager_UpdatesStorage
    // setAllowanceManager() must update the allowanceManager slot and emit event.
    // -----------------------------------------------------------------------
    function testSC5_SetAllowanceManager_UpdatesStorage() public {
        AllowanceManager am = _deployAllowanceManager();

        // Default must be zero address
        assertEq(address(executor.allowanceManager()), address(0), "default allowanceManager must be address(0)");

        // Expect AllowanceManagerUpdated event
        vm.expectEmit(true, false, false, false, address(executor));
        emit ArbitrageExecutor.AllowanceManagerUpdated(address(am));

        executor.setAllowanceManager(address(am));

        assertEq(address(executor.allowanceManager()), address(am), "allowanceManager must be updated");
    }

    // -----------------------------------------------------------------------
    // testSC5_SetAllowanceManager_OnlyAdmin
    // Non-admin must be blocked from setAllowanceManager.
    // -----------------------------------------------------------------------
    function testSC5_SetAllowanceManager_OnlyAdmin() public {
        AllowanceManager am = _deployAllowanceManager();

        vm.expectRevert();
        vm.prank(stranger);
        executor.setAllowanceManager(address(am));
    }

    // -----------------------------------------------------------------------
    // testSC5_ClearAllowanceManager_PassZeroAddress
    // Passing address(0) to setAllowanceManager disables the check.
    // -----------------------------------------------------------------------
    function testSC5_ClearAllowanceManager_PassZeroAddress() public {
        AllowanceManager am = _deployAllowanceManager();
        executor.setAllowanceManager(address(am));
        assertEq(address(executor.allowanceManager()), address(am));

        executor.setAllowanceManager(address(0));
        assertEq(address(executor.allowanceManager()), address(0), "allowanceManager must be cleared");
    }

    // -----------------------------------------------------------------------
    // testSC5_ExecuteArbitrage_WithAllowanceManager_HappyPath
    //
    // When AllowanceManager is set AND the router has a live allowance for
    // tokenIn on the AllowanceManager, executeArbitrage must succeed normally.
    // -----------------------------------------------------------------------
    function testSC5_ExecuteArbitrage_WithAllowanceManager_HappyPath() public {
        uint256 amountIn = 1_000e18;
        uint256 profit   = 50e18;
        uint256 minProfit = 10e18;

        AllowanceManager am = _deployAllowanceManager();
        executor.setAllowanceManager(address(am));

        // Deploy router and approve it in ArbitrageExecutor
        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);
        // A5: approve the selector so the call passes the whitelist gate.
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);

        // Grant allowance from AllowanceManager to router for tokenIn.
        // AllowanceManager.grantAllowance() calls IERC20(token).forceApprove(router, amount),
        // which sets allowance FROM the AllowanceManager contract TO the router.
        // IAllowanceManager.isApproved(token, router) then reads that allowance.
        am.grantAllowance(address(token), address(router), 1_000_000e18);

        // Verify isApproved works
        assertTrue(am.isApproved(address(token), address(router)), "router must be approved in AM");

        token.mint(address(executor), amountIn);

        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        // A5: payload must carry the whitelisted selector.
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, minProfit, routers, payloads);

        // Verify profit landed (executor still holds it, not transferred — per current design)
        uint256 finalBalance = token.balanceOf(address(executor));
        assertGe(finalBalance, amountIn + minProfit, "executor must have amountIn + at least minProfit");
    }

    // -----------------------------------------------------------------------
    // testSC5_ExecuteArbitrage_RevertsWhenRouterNotInAllowanceManager
    //
    // When AllowanceManager is set but router has NO allowance for tokenIn,
    // executeArbitrage must revert with RouterAllowanceNotGranted.
    // -----------------------------------------------------------------------
    function testSC5_ExecuteArbitrage_RevertsWhenRouterNotInAllowanceManager() public {
        uint256 amountIn = 1_000e18;

        AllowanceManager am = _deployAllowanceManager();
        executor.setAllowanceManager(address(am));

        // Deploy router — approved in ArbitrageExecutor but NOT in AllowanceManager
        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), 50e18);
        executor.setRouterApproval(address(router), true);
        // Deliberately: do NOT call am.grantAllowance(...)

        token.mint(address(executor), amountIn);

        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

        vm.expectRevert(
            abi.encodeWithSelector(RouterAllowanceNotGranted.selector, address(router), address(token))
        );

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testSC5_ExecuteArbitrage_SkipsCheckWhenNoAllowanceManager
    //
    // When allowanceManager == address(0) (default), the allowance check is
    // entirely skipped. The existing happy-path behavior is unchanged.
    // Backward-compatibility regression guard.
    // -----------------------------------------------------------------------
    function testSC5_ExecuteArbitrage_SkipsCheckWhenNoAllowanceManager() public {
        uint256 amountIn = 1_000e18;
        uint256 profit   = 50e18;

        // No setAllowanceManager() call — default is address(0)
        assertEq(address(executor.allowanceManager()), address(0));

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);
        // A5: still need selector approval even when AllowanceManager is disabled.
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);
        token.mint(address(executor), amountIn);

        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        // A5: payload must carry the whitelisted selector.
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        // Must succeed: AllowanceManager check is not active
        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testSC5_AllowanceManager_IsApproved_ReturnsFalseAfterRevoke
    //
    // Verifies the IAllowanceManager view functions reflect real token state:
    //   isApproved → false after revokeAllowance.
    //   getAllowance → 0 after revokeAllowance.
    // -----------------------------------------------------------------------
    function testSC5_AllowanceManager_IsApproved_ReturnsFalseAfterRevoke() public {
        AllowanceManager am = _deployAllowanceManager();
        address spender = makeAddr("spender");

        // Initially no allowance
        assertFalse(am.isApproved(address(token), spender), "no allowance initially");
        assertEq(am.getAllowance(address(token), spender), 0, "allowance must be 0 initially");

        // Grant allowance
        am.grantAllowance(address(token), spender, 500e18);
        assertTrue(am.isApproved(address(token), spender), "must be approved after grant");
        assertEq(am.getAllowance(address(token), spender), 500e18, "allowance must match grant amount");

        // Revoke
        am.revokeAllowance(address(token), spender);
        assertFalse(am.isApproved(address(token), spender), "must not be approved after revoke");
        assertEq(am.getAllowance(address(token), spender), 0, "allowance must be 0 after revoke");
    }

    // =========================================================================
    // A9: Kill-switch regression tests (audit 2026-05-10)
    // =========================================================================
    //
    // Tag prefix: testA9_* — grep-able by future audit re-runs to confirm each
    // A9 finding has a dedicated regression test.
    //
    // Existing testPausable_BlocksWhenPaused covers the basic pause path with
    // an empty routers array.  The A9 tests add two distinct angles:
    //   1. Pause blocks even when all guards would otherwise pass (full payload).
    //   2. The paused() view is readable off-chain for kill-switch monitoring.

    // -----------------------------------------------------------------------
    // testA9_ExecuteArbitrage_Reverts_WhenPaused_AcrossAllRevertCases
    //
    // A9 regression: even with a fully-approved router, selector, and token,
    // the pause modifier fires FIRST and reverts with EnforcedPause() before
    // any business logic executes.  This test exercises the worst-case path —
    // an operator who has wired every approval but forgets the kill-switch
    // must still be protected.
    // -----------------------------------------------------------------------
    function testA9_ExecuteArbitrage_Reverts_WhenPaused_AcrossAllRevertCases() external {
        // Setup: mint funds + approve router + approve selector — a valid execution path.
        uint256 amountIn = 500e18;
        uint256 profit   = 20e18;
        token.mint(address(executor), amountIn);

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);

        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        // Pause via admin (address(this) holds DEFAULT_ADMIN_ROLE).
        executor.pause();

        // OZ 5.x Pausable reverts with EnforcedPause() even when the entire
        // route is valid.  The modifier executes before any state read.
        vm.expectRevert(abi.encodeWithSelector(bytes4(keccak256("EnforcedPause()"))));

        vm.prank(executorRole);
        executor.executeArbitrage(
            bytes32(0), address(token), address(token), amountIn, 0, routers, payloads
        );
    }

    // -----------------------------------------------------------------------
    // testA9_PausedView_AccessibleToOffChain
    //
    // A9 regression: the paused() view function must be readable by off-chain
    // monitoring (kill-switch dashboard, searcher-rs circuit breaker).
    // This test verifies the state transition from live → paused → unpaused
    // is correctly reflected in the public view, which is the contract surface
    // that monitoring tools poll.
    // -----------------------------------------------------------------------
    function testA9_PausedView_AccessibleToOffChain() external {
        // Initial state: contract is live (not paused).
        assertFalse(executor.paused(), "executor must not be paused after initialize");

        // Admin triggers kill-switch.
        executor.pause();
        assertTrue(executor.paused(), "executor.paused() must return true after pause()");

        // Admin lifts the kill-switch — monitoring must observe recovery.
        executor.unpause();
        assertFalse(executor.paused(), "executor.paused() must return false after unpause()");
    }

    // =========================================================================
    // A5: Router function-selector whitelist tests (audit 2026-05-10)
    // =========================================================================

    // -----------------------------------------------------------------------
    // testA5_SetRouterSelectorApproval_OnlyAdmin
    //
    // Only DEFAULT_ADMIN_ROLE can call setRouterSelectorApproval.
    // Non-admin must be blocked. The approved mapping must update correctly
    // and emit RouterSelectorApproved.
    // -----------------------------------------------------------------------
    function testA5_SetRouterSelectorApproval_OnlyAdmin() public {
        address router = makeAddr("anyRouter");
        bytes4 sel = bytes4(keccak256("transfer(address,uint256)"));

        // Default: not approved
        assertFalse(executor.approvedSelectors(router, sel), "must default to false");

        // Stranger (no ADMIN_ROLE) must be blocked
        vm.expectRevert();
        vm.prank(stranger);
        executor.setRouterSelectorApproval(router, sel, true);

        // executorRole (EXECUTOR_ROLE only, not ADMIN) must also be blocked
        vm.expectRevert();
        vm.prank(executorRole);
        executor.setRouterSelectorApproval(router, sel, true);

        // Admin (address(this) — holds DEFAULT_ADMIN_ROLE in setUp) succeeds
        vm.expectEmit(true, true, false, true, address(executor));
        emit ArbitrageExecutor.RouterSelectorApproved(router, sel, true);
        executor.setRouterSelectorApproval(router, sel, true);
        assertTrue(executor.approvedSelectors(router, sel), "must be approved after admin call");

        // Admin can also revoke
        vm.expectEmit(true, true, false, true, address(executor));
        emit ArbitrageExecutor.RouterSelectorApproved(router, sel, false);
        executor.setRouterSelectorApproval(router, sel, false);
        assertFalse(executor.approvedSelectors(router, sel), "must be revoked after admin call");
    }

    // -----------------------------------------------------------------------
    // testA5_ExecuteArbitrage_RevertsWhenSelectorNotApproved
    //
    // When a router is address-approved but its selector is NOT in the whitelist,
    // executeArbitrage must revert with AE_RouterSelectorNotApproved.
    // This is the primary A5 invariant: address approval alone is insufficient.
    // -----------------------------------------------------------------------
    function testA5_ExecuteArbitrage_RevertsWhenSelectorNotApproved() public {
        uint256 amountIn = 1_000e18;
        token.mint(address(executor), amountIn);

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), 50e18);
        executor.setRouterApproval(address(router), true);
        // Deliberately: do NOT call setRouterSelectorApproval — selector remains false.

        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        vm.expectRevert(
            abi.encodeWithSelector(AE_RouterSelectorNotApproved.selector, address(router), MOCK_SELECTOR)
        );

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testA5_ExecuteArbitrage_RevertsWhenPayloadTooShort
    //
    // A payload shorter than 4 bytes cannot carry a valid ABI selector.
    // executeArbitrage must revert with AE_PayloadTooShort before attempting
    // any selector whitelist lookup or router call.
    // -----------------------------------------------------------------------
    function testA5_ExecuteArbitrage_RevertsWhenPayloadTooShort() public {
        uint256 amountIn = 1_000e18;
        token.mint(address(executor), amountIn);

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), 50e18);
        executor.setRouterApproval(address(router), true);
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);

        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        // 3-byte payload — shorter than 4, no valid selector extractable.
        payloads[0] = hex"aabbcc";

        vm.expectRevert(
            abi.encodeWithSelector(AE_PayloadTooShort.selector, address(router))
        );

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // testA5_BatchSetRouterSelectorApproval
    //
    // batchSetRouterSelectorApproval approves multiple selectors for a router
    // in a single tx. Verifies: all selectors set, events emitted, only admin.
    // -----------------------------------------------------------------------
    function testA5_BatchSetRouterSelectorApproval() public {
        address router = makeAddr("batchRouter");

        bytes4 sel1 = bytes4(keccak256("exactInput(bytes,uint256,uint256)"));
        bytes4 sel2 = bytes4(keccak256("exactInputSingle(address,address,uint24,address,uint256,uint256,uint256,uint160)"));
        bytes4 sel3 = bytes4(keccak256("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)"));

        bytes4[] memory sels = new bytes4[](3);
        sels[0] = sel1;
        sels[1] = sel2;
        sels[2] = sel3;

        // Stranger blocked on batch variant too
        vm.expectRevert();
        vm.prank(stranger);
        executor.batchSetRouterSelectorApproval(router, sels, true);

        // Admin approves all three in one tx
        executor.batchSetRouterSelectorApproval(router, sels, true);

        assertTrue(executor.approvedSelectors(router, sel1), "sel1 must be approved");
        assertTrue(executor.approvedSelectors(router, sel2), "sel2 must be approved");
        assertTrue(executor.approvedSelectors(router, sel3), "sel3 must be approved");

        // Batch-revoke
        executor.batchSetRouterSelectorApproval(router, sels, false);

        assertFalse(executor.approvedSelectors(router, sel1), "sel1 must be revoked");
        assertFalse(executor.approvedSelectors(router, sel2), "sel2 must be revoked");
        assertFalse(executor.approvedSelectors(router, sel3), "sel3 must be revoked");
    }
}
