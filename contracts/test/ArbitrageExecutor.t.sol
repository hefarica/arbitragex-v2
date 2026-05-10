// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
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

        address[] memory routers = new address[](1);
        routers[0] = address(router);

        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

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

        address[] memory routers = new address[](1);
        routers[0] = address(router);

        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

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
    // -----------------------------------------------------------------------
    function testReentrancy_Blocked() public {
        uint256 amountIn = 1_000e18;

        // Deploy malicious router first (needs executor address)
        MaliciousReentrantRouter malicious = new MaliciousReentrantRouter(address(executor));
        executor.setRouterApproval(address(malicious), true);

        address[] memory attackRouters = new address[](1);
        attackRouters[0] = address(malicious);

        bytes[] memory attackPayloads = new bytes[](1);
        attackPayloads[0] = "";

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
        // tokenOut approval not needed — it's only passed for observability metadata

        token.mint(address(executor), amountIn);

        MockProfitRouter router = new MockProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);

        address[] memory routers = new address[](1);
        routers[0] = address(router);

        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

        // Verify event carries tokenIn != tokenOut — the SC-05 bug emitted tokenIn twice
        vm.expectEmit(true, true, true, true, address(executor));
        emit ArbitrageExecutor.ArbitrageExecuted(bytes32(0), address(token), address(tokenOut), profit);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(token), address(tokenOut), amountIn, profit, routers, payloads);
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
}
