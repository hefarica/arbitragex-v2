// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/FlashLoanExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// =============================================================================
// ATOMIC-CAUSE-01 (2026-10-23) — every named error is fired and its selector
// + args are asserted byte-exactly.
//
// REA sources (FICHA_REA4_ATOMIC.md):
//   flashbots/simple-blind-arbitrage@73c6c270b27059e6dc0469a6c08e0f7739e5d531
//     - src/BlindBackrunLogic.sol:266-268 — `(bool success, ) = _to.call(...)`
//       DISCARDS returndata (the anonymous pattern this PR eliminates in
//       ArbitrageExecutor._boundedRouterCall and FlashLoanExecutor callbacks).
//     - src/BlindBackrunLogic.sol:148/166 — the bare `isqrt(presqrt) - 1000`
//       subtraction underflows with an ANONYMOUS Panic(0x11); the destination
//       instead checks the delta sign BEFORE subtracting and names the cause
//       (AccountingDeltaUnderflow).
//   aave/aave-v3-core@782f51917056a53a2c228701058a6c3fb233684a
//     - contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol +
//       FlashLoanLogic.sol:248-252 — repay-or-revert invariant (PULL
//       safeTransferFrom of amount+premium reverts the WHOLE tx). The
//       invariant is INTACT here; only the VISIBILITY of the failure cause
//       changed (FL_RepaymentShortfall now carries available vs. owed).
//
// Selector pins below were computed OUT of band with `cast keccak` (foundry
// 1.7.2-nightly c5e44b5e) — see each test_AC01_Selectors_Pinned constant.
// =============================================================================

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

/// @dev Plain ERC20 with a public mint (tokenIn / tokenOut roles).
contract AC_Token is ERC20 {
    constructor() ERC20("ACToken", "ACT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Router whose call reverts with a KNOWN custom error, so the returndata
///      bubbled through SwapFailed / FL_ArbitrageExecutionFailed is byte-exact.
contract AC_RevertingRouter {
    error BoomRouter(uint256 code);

    fallback() external {
        revert BoomRouter(42);
    }
}

/// @dev Router that pulls `pullAmount` of `spendToken` from the caller (the
///      executor, via its ephemeral exact allowance) and pushes `pullAmount +
///      boost` of `payToken` from its OWN funds to the executor. Net effect for
///      the executor: +boost of payToken. Used to shape exact profit deltas.
contract AC_EchoProfitRouter {
    ERC20 public spendToken;
    ERC20 public payToken;
    uint256 public pullAmount;
    uint256 public boost;

    constructor(address _spendToken, address _payToken, uint256 _pull, uint256 _boost) {
        spendToken = ERC20(_spendToken);
        payToken = ERC20(_payToken);
        pullAmount = _pull;
        boost = _boost;
    }

    fallback() external {
        spendToken.transferFrom(msg.sender, address(this), pullAmount);
        payToken.transfer(msg.sender, pullAmount + boost);
    }
}

/// @dev Router whose call succeeds but moves nothing (zero profit).
contract AC_ZeroProfitRouter {
    fallback() external {}
}

/// @dev Router that PUSHES an exact `pushAmount` of `payToken` to the caller
///      from its OWN funds (from = router, so sender-surcharge tokens stay
///      faithful on this leg). Used to shape exact intermediate outputs of a
///      forward leg without pulling anything.
contract AC_PushTokenRouter {
    ERC20 public payToken;
    uint256 public pushAmount;

    constructor(address _payToken, uint256 _pushAmount) {
        payToken = ERC20(_payToken);
        pushAmount = _pushAmount;
    }

    fallback() external {
        payToken.transfer(msg.sender, pushAmount);
    }
}

/// @dev Router that drains `drainAmount` of `drainToken` FROM the executor via a
///      STALE allowance seeded out-of-band by the test (vm.prank approve). Models
///      "leg 0 drained pre-existing tokenOut" — the case that used to hit a bare
///      subtraction and Panic(0x11).
contract AC_DrainTokenOutRouter {
    ERC20 public drainToken;
    uint256 public drainAmount;

    constructor(address _drainToken, uint256 _drainAmount) {
        drainToken = ERC20(_drainToken);
        drainAmount = _drainAmount;
    }

    fallback() external {
        drainToken.transferFrom(msg.sender, address(this), drainAmount);
    }
}

/// @dev Fee-on-transfer ERC20 (1% to a sink on every plain transfer): the
///      recipient of a transferFrom is credited LESS than `value`, so the
///      flash-funded pull cannot credit exactly amountIn.
contract AC_FeeOnTransferToken is ERC20 {
    address private constant SINK = address(0xdead);

    constructor() ERC20("ACFee", "ACF") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function _update(address from, address to, uint256 value) internal override {
        if (from == address(0) || to == address(0)) {
            super._update(from, to, value);
            return;
        }
        uint256 fee = value / 100; // 1%
        super._update(from, SINK, fee);
        super._update(from, to, value - fee);
    }
}

/// @dev Sender-surcharge token: the recipient always gets `value` in full, but
///      when the configured `surchargeFrom` account is the SENDER it is debited
///      an EXTRA 1% on top. A pull INTO the executor from any other holder is
///      faithful; only transfers OUT of `surchargeFrom` short it. Models the
///      deflationary tokenOut of the execution-core audit (TokenOutRetention)
///      and the outbound-lossy tokenIn (FlashFundedCapitalRetention).
contract AC_OutboundFeeToken is ERC20 {
    address private constant SINK = address(0xdead);
    address public surchargeFrom;

    constructor() ERC20("ACOutFee", "ACO") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function setSurchargeFrom(address account) external {
        surchargeFrom = account;
    }

    function _update(address from, address to, uint256 value) internal override {
        super._update(from, to, value); // faithful main leg: recipient gets `value`
        if (surchargeFrom != address(0) && from == surchargeFrom && to != address(0)) {
            uint256 fee = value / 100; // extra 1% debited from the sender
            if (fee > 0) super._update(from, SINK, fee);
        }
    }
}

/// @dev ETH recipient whose receive() reverts with a KNOWN custom error, so the
///      returndata bubbled through ETHRescueFailed is byte-exact.
contract AC_RevertEthSink {
    error EthSinkBoom(string reason);

    receive() external payable {
        revert EthSinkBoom("nope");
    }
}

/// @dev Mock Aave V3 pool that performs the REAL round trip: disburse the loan
///      (real transfer), then invoke executeOperation on the receiver from this
///      address (the pool configured in FlashLoanExecutor) with the configured
///      premium. Mirrors MockAavePoolRoundTrip in FlashLoanExecutor.t.sol.
contract AC_MockAavePoolRT {
    ERC20 public asset;
    uint256 public premium;

    constructor(address _asset, uint256 _premium) {
        asset = ERC20(_asset);
        premium = _premium;
    }

    function fund(uint256 amount) external {
        AC_Token(address(asset)).mint(address(this), amount);
    }

    function flashLoanSimple(
        address receiverAddress,
        address _asset,
        uint256 amount,
        bytes calldata params,
        uint16 /*referralCode*/
    )
        external
    {
        require(_asset == address(asset), "wrong asset");
        asset.transfer(receiverAddress, amount);
        FlashLoanExecutor(receiverAddress).executeOperation(_asset, amount, premium, receiverAddress, params);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

contract AtomicCauseTest is Test {
    ArbitrageExecutor internal executor;
    AC_Token internal tokenA; // tokenIn
    AC_Token internal tokenB; // tokenOut (cross-DEX)

    address internal admin;
    address internal executorRole;

    /// @dev Shared mock selector (fallback() accepts any calldata).
    bytes4 internal constant MOCK_SELECTOR = bytes4(keccak256("swap()"));

    function setUp() public {
        admin = address(this);
        executorRole = makeAddr("executorRole");

        ArbitrageExecutor impl = new ArbitrageExecutor();
        bytes memory initData = abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, admin);
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        executor = ArbitrageExecutor(payable(address(proxy)));

        tokenA = new AC_Token();
        tokenB = new AC_Token();

        executor.grantRole(executor.EXECUTOR_ROLE(), executorRole);
        executor.setTokenApproval(address(tokenA), true);
        executor.setTokenApproval(address(tokenB), true);
    }

    // -- helpers ------------------------------------------------------------

    function _approveRouter(address router) internal {
        executor.setRouterApproval(router, true);
        executor.setRouterSelectorApproval(router, MOCK_SELECTOR, true);
    }

    function _oneLeg(address router) internal pure returns (address[] memory r) {
        r = new address[](1);
        r[0] = router;
    }

    function _onePayload() internal pure returns (bytes[] memory p) {
        p = new bytes[](1);
        p[0] = abi.encodePacked(MOCK_SELECTOR);
    }

    // =====================================================================
    // SwapFailed(uint256 legIndex, address router, bytes revertData)
    // =====================================================================

    /// @dev A router reverting with a NAMED cause (BoomRouter(42)) must surface
    ///      as SwapFailed carrying leg 0, the router and the RAW returndata.
    ///      Before ATOMIC-CAUSE-01 the returndata was discarded and the revert
    ///      was anonymous. Selector 0xd3512921 (cast keccak).
    function test_AC01_SwapFailed_BubblesReturndata_Leg0() public {
        AC_RevertingRouter router = new AC_RevertingRouter();
        _approveRouter(address(router));

        tokenA.mint(address(executor), 1_000e18);

        vm.expectRevert(
            abi.encodeWithSelector(
                SwapFailed.selector,
                0,
                address(router),
                abi.encodeWithSelector(AC_RevertingRouter.BoomRouter.selector, 42)
            )
        );
        vm.prank(executorRole);
        executor.executeArbitrage(
            bytes32(0), address(tokenA), address(tokenA), 1_000e18, 0, _oneLeg(address(router)), _onePayload()
        );
    }

    /// @dev Same bubbling on the BACKWARD leg (legIndex == 1) of a 2-leg route.
    ///      Leg 0 legitimately produces a tokenOut intermediate; leg 1 reverts.
    function test_AC01_SwapFailed_BubblesReturndata_Leg1() public {
        // Leg 0: push an exact 1_000e18 tokenOut intermediate to the executor.
        AC_PushTokenRouter router0 = new AC_PushTokenRouter(address(tokenB), 1_000e18);
        AC_RevertingRouter router1 = new AC_RevertingRouter();
        _approveRouter(address(router0));
        _approveRouter(address(router1));

        tokenA.mint(address(executor), 1_000e18);
        tokenB.mint(address(router0), 1_000e18);

        address[] memory routers = new address[](2);
        routers[0] = address(router0);
        routers[1] = address(router1);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);
        payloads[1] = abi.encodePacked(MOCK_SELECTOR);

        vm.expectRevert(
            abi.encodeWithSelector(
                SwapFailed.selector,
                1,
                address(router1),
                abi.encodeWithSelector(AC_RevertingRouter.BoomRouter.selector, 42)
            )
        );
        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenA), address(tokenB), 1_000e18, 0, routers, payloads);
    }

    // =====================================================================
    // InsufficientBalance(uint256 available, uint256 required)
    // =====================================================================

    /// @dev Selector 0xcf479181 (cast keccak). Args: observed vs. required.
    function test_AC01_InsufficientBalance_CarriesArgs() public {
        AC_ZeroProfitRouter router = new AC_ZeroProfitRouter();
        _approveRouter(address(router));

        tokenA.mint(address(executor), 5e18); // available
        uint256 amountIn = 10e18; // required

        vm.expectRevert(abi.encodeWithSelector(InsufficientBalance.selector, 5e18, 10e18));
        vm.prank(executorRole);
        executor.executeArbitrage(
            bytes32(0), address(tokenA), address(tokenA), amountIn, 0, _oneLeg(address(router)), _onePayload()
        );
    }

    // =====================================================================
    // ZeroGrossProfit(uint256 balanceAfter, uint256 balanceBefore)
    // =====================================================================

    /// @dev Selector 0x4422c101 (cast keccak). A no-op route leaves
    ///      balanceAfter == balanceBefore: both values must ride the revert.
    function test_AC01_ZeroGrossProfit_CarriesArgs() public {
        AC_ZeroProfitRouter router = new AC_ZeroProfitRouter();
        _approveRouter(address(router));

        uint256 balance = 1_000e18;
        tokenA.mint(address(executor), balance);

        vm.expectRevert(abi.encodeWithSelector(ZeroGrossProfit.selector, balance, balance));
        vm.prank(executorRole);
        executor.executeArbitrage(
            bytes32(0), address(tokenA), address(tokenA), balance, 0, _oneLeg(address(router)), _onePayload()
        );
    }

    // =====================================================================
    // InsufficientProfit(uint256 actual, uint256 required)
    // =====================================================================

    /// @dev Selector 0x4e88422a (cast keccak). The floor (required) still comes
    ///      from the CALLER (operator config), never a literal in the contract.
    function test_AC01_InsufficientProfit_CarriesArgs() public {
        uint256 amountIn = 1_000e18;
        uint256 boost = 1; // realised profit: exactly 1 wei
        uint256 minProfit = 100e18; // operator floor

        AC_EchoProfitRouter router = new AC_EchoProfitRouter(address(tokenA), address(tokenA), amountIn, boost);
        _approveRouter(address(router));

        tokenA.mint(address(executor), amountIn);
        tokenA.mint(address(router), amountIn + boost); // router's own pay funds

        vm.expectRevert(abi.encodeWithSelector(InsufficientProfit.selector, boost, minProfit));
        vm.prank(executorRole);
        executor.executeArbitrage(
            bytes32(0), address(tokenA), address(tokenA), amountIn, minProfit, _oneLeg(address(router)), _onePayload()
        );
    }

    // =====================================================================
    // FlashFundedPullMismatch(uint256 balanceAfter, uint256 expected)
    // =====================================================================

    /// @dev Selector 0x97191793 (cast keccak). A 1% fee-on-transfer tokenIn
    ///      credits amountIn - fee on the pull; the revert names observed vs.
    ///      expected (pre-pull + amountIn).
    function test_AC01_FlashFundedPullMismatch_CarriesArgs() public {
        AC_FeeOnTransferToken feeToken = new AC_FeeOnTransferToken();
        executor.setTokenApproval(address(feeToken), true);

        uint256 amountIn = 1_000e18;
        uint256 fee = amountIn / 100; // 1%

        // Caller (executorRole) funds and approves the exact pull.
        feeToken.mint(executorRole, amountIn);
        vm.prank(executorRole);
        feeToken.approve(address(executor), amountIn);

        // Executor pre-call working capital: 0.
        AC_ZeroProfitRouter router = new AC_ZeroProfitRouter();
        _approveRouter(address(router));

        // expected credit = balBeforePull (0) + amountIn; observed = amountIn - fee.
        vm.expectRevert(abi.encodeWithSelector(FlashFundedPullMismatch.selector, amountIn - fee, 0 + amountIn));
        vm.prank(executorRole);
        executor.executeArbitrageFlashFunded(
            bytes32(0), address(feeToken), address(feeToken), amountIn, 0, _oneLeg(address(router)), _onePayload()
        );
    }

    // =====================================================================
    // FlashFundedCapitalRetentionViolation(uint256 balanceAfter, uint256 expected)
    // =====================================================================

    /// @dev Selector 0x31a7cece (cast keccak). Outbound-lossy tokenIn: the pull
    ///      is faithful, the route profits, but the RETURN transfer surcharges
    ///      the executor 1% — so it ends BELOW its pre-call capital B.
    ///      Arithmetic (all transfers out of the executor pay an extra 1%):
    ///        B = 1_000e18; pull credits +1_000e18 (faithful) → 2_000e18
    ///        route pushes 1_000e18 (from router, faithful) → 3_000e18, profit 1_000e18
    ///        return sends amountIn+profit = 2_000e18, executor debited 2_000e18
    ///          + 20e18 surcharge → 980e18
    ///        guard: 980e18 != B=1_000e18 → violation(980e18, 1_000e18)
    function test_AC01_FlashFundedCapitalRetention_CarriesArgs() public {
        AC_OutboundFeeToken outToken = new AC_OutboundFeeToken();
        executor.setTokenApproval(address(outToken), true);

        uint256 capital = 1_000e18; // pre-call working capital B
        uint256 amountIn = 1_000e18;
        uint256 push = 1_000e18; // route output (profit = 1_000e18)
        uint256 returned = amountIn + push; // 2_000e18
        uint256 fee = returned / 100; // 20e18 surcharged on the outbound return

        outToken.setSurchargeFrom(address(executor));

        // Caller funds + approves the faithful pull.
        outToken.mint(executorRole, amountIn);
        vm.prank(executorRole);
        outToken.approve(address(executor), amountIn);

        // Route: push-only leg (from = router, faithful) → no surcharge inside
        // the route; only the RETURN transfer shorts the executor.
        AC_PushTokenRouter router = new AC_PushTokenRouter(address(outToken), push);
        _approveRouter(address(router));
        outToken.mint(address(router), push);

        outToken.mint(address(executor), capital);

        uint256 expectedAfter = capital + amountIn + push - returned - fee; // = 980e18
        assertEq(expectedAfter, 980e18, "arithmetic sanity");

        vm.expectRevert(abi.encodeWithSelector(FlashFundedCapitalRetentionViolation.selector, expectedAfter, capital));
        vm.prank(executorRole);
        executor.executeArbitrageFlashFunded(
            bytes32(0), address(outToken), address(outToken), amountIn, 0, _oneLeg(address(router)), _onePayload()
        );
    }

    // =====================================================================
    // AccountingDeltaUnderflow(uint256 available, uint256 snapshotBefore)
    // =====================================================================

    /// @dev Selector 0x03bb1851 (cast keccak). A 2-leg route whose leg 0 DRAINS
    ///      pre-existing tokenOut via a stale allowance used to hit the BARE
    ///      subtraction `balanceOf(tokenOut) - tokenOutBefore` and revert with
    ///      an ANONYMOUS Panic(0x11) (selector 0x4e487b71, no cause). It now
    ///      reverts with the named cause BEFORE any subtraction.
    ///      REA: simple-blind-arbitrage BlindBackrunLogic.sol:148/166 leaves the
    ///      equivalent underflow anonymous upstream.
    function test_AC01_AccountingDeltaUnderflow_NamedNotPanic() public {
        uint256 bOut = 1_000e18; // pre-existing tokenOut working capital
        uint256 drain = 400e18; // drained by leg 0 via the stale allowance

        tokenB.mint(address(executor), bOut);
        tokenA.mint(address(executor), 1_000e18);

        AC_DrainTokenOutRouter router0 = new AC_DrainTokenOutRouter(address(tokenB), drain);
        AC_RevertingRouter router1 = new AC_RevertingRouter(); // never reached
        _approveRouter(address(router0));
        _approveRouter(address(router1));

        // Seed the STALE allowance out-of-band: the executor (as token holder)
        // once approved router0 for tokenB. vm.prank lets the test impersonate
        // the proxy address for this one call.
        vm.prank(address(executor));
        tokenB.approve(address(router0), bOut);

        address[] memory routers = new address[](2);
        routers[0] = address(router0);
        routers[1] = address(router1);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);
        payloads[1] = abi.encodePacked(MOCK_SELECTOR);

        // available = bOut - drain; snapshotBefore = bOut.
        vm.expectRevert(abi.encodeWithSelector(AccountingDeltaUnderflow.selector, bOut - drain, bOut));
        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenA), address(tokenB), 1_000e18, 0, routers, payloads);
    }

    // =====================================================================
    // TokenOutRetentionViolation(uint256 balanceAfter, uint256 snapshotBefore)
    // =====================================================================

    /// @dev Selector 0xff597268 (cast keccak). Deflationary tokenOut: leg 1's
    ///      legitimate transferFrom of the intermediate surcharges the executor
    ///      an extra 1%, so its tokenOut balance ends BELOW the pre-route
    ///      snapshot. The revert names both balances.
    ///      Arithmetic: B_out = 1_000e18; leg 0 pushes 500e18 (from router,
    ///      faithful) → balance 1_500e18, intermediate = 500e18; leg 1 pulls
    ///      exactly 500e18 with from = executor → debited 500e18 + 5e18
    ///      surcharge → 995e18 left. A FAITHFUL leg 1 would have ended at
    ///      1_000e18 (equality); the 5e18 surcharge IS the shortfall the guard
    ///      exists to name.
    function test_AC01_TokenOutRetentionViolation_CarriesArgs() public {
        AC_OutboundFeeToken tokenOut = new AC_OutboundFeeToken();
        executor.setTokenApproval(address(tokenOut), true);
        tokenOut.setSurchargeFrom(address(executor));

        uint256 bOut = 1_000e18; // pre-existing B_out
        uint256 intermediate = 500e18; // leg-0 output the backward leg consumes
        uint256 fee = intermediate / 100; // 1% sender surcharge on leg 1

        tokenOut.mint(address(executor), bOut);
        tokenA.mint(address(executor), 1_000e18);

        // Leg 0: push an exact 500e18 tokenOut intermediate (from = router:
        // faithful, no surcharge).
        AC_PushTokenRouter router0 = new AC_PushTokenRouter(address(tokenOut), intermediate);
        _approveRouter(address(router0));
        tokenOut.mint(address(router0), intermediate);

        // Leg 1: consume the exact intermediate allowance granted by the route
        // (from = executor == surchargeFrom → debited 500e18 + fee).
        AC_DrainTokenOutRouter router1 = new AC_DrainTokenOutRouter(address(tokenOut), intermediate);
        _approveRouter(address(router1));

        // bOut + intermediate (leg-0 push) - intermediate (leg-1 pull) - fee.
        uint256 expectedAfter = bOut - fee; // 995e18

        address[] memory routers = new address[](2);
        routers[0] = address(router0);
        routers[1] = address(router1);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);
        payloads[1] = abi.encodePacked(MOCK_SELECTOR);

        vm.expectRevert(abi.encodeWithSelector(TokenOutRetentionViolation.selector, expectedAfter, bOut));
        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenA), address(tokenOut), 1_000e18, 0, routers, payloads);
    }

    // =====================================================================
    // ETHRescueFailed(address to, bytes revertData)
    // =====================================================================

    /// @dev Selector 0x57b4b222 (cast keccak). A recipient whose receive()
    ///      reverts with a named cause must surface through ETHRescueFailed
    ///      with the RAW returndata (previously a discarded-behind-string
    ///      require).
    function test_AC01_ETHRescueFailed_BubblesReturndata() public {
        AC_RevertEthSink sink = new AC_RevertEthSink();
        vm.deal(address(executor), 1 ether);

        vm.expectRevert(
            abi.encodeWithSelector(
                ETHRescueFailed.selector,
                address(sink),
                abi.encodeWithSelector(AC_RevertEthSink.EthSinkBoom.selector, "nope")
            )
        );
        executor.withdrawETH(payable(address(sink)));
    }

    // =====================================================================
    // FL_ArbitrageExecutionFailed(bytes revertData)
    // =====================================================================

    /// @dev FlashLoanExecutor + real round-trip mock pool. The inner
    ///      ArbitrageExecutor call reverts LengthMismatch; the wrapper must
    ///      bubble the executor's returndata instead of discarding it.
    ///      Selector 0x0f70bbda (cast keccak).
    ///      Repay-or-revert invariant intact: after the revert the FLE holds
    ///      nothing (the whole tx, loan disbursement included, reverted).
    function test_AC01_FL_ArbitrageExecutionFailed_BubblesCause() public {
        AC_Token asset = new AC_Token();
        executor.setTokenApproval(address(asset), true);

        AC_MockAavePoolRT pool = new AC_MockAavePoolRT(address(asset), 0);
        pool.fund(1_000e18);

        FlashLoanExecutor fleImpl = new FlashLoanExecutor();
        bytes memory fleInit =
            abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, admin, address(pool), address(executor));
        ERC1967Proxy fleProxy = new ERC1967Proxy(address(fleImpl), fleInit);
        FlashLoanExecutor fle = FlashLoanExecutor(payable(address(fleProxy)));
        fle.grantRole(fle.EXECUTOR_ROLE(), address(this));

        // The inner ArbitrageExecutor call comes FROM the FLE: it needs
        // EXECUTOR_ROLE on the executor so the revert we assert is the SHAPE
        // gate (LengthMismatch), not the access gate.
        executor.grantRole(executor.EXECUTOR_ROLE(), address(fle));

        // params: executeArbitrage calldata that reverts LengthMismatch
        // (routers.length != payload.length) — a NAMED cause with a known 4-byte
        // returndata.
        address[] memory routers = new address[](2);
        bytes[] memory payloads = new bytes[](1);
        bytes memory params = abi.encodeWithSelector(
            ArbitrageExecutor.executeArbitrage.selector,
            bytes32(0),
            address(asset),
            address(asset),
            1_000e18,
            0,
            routers,
            payloads
        );

        uint256 fleBalBefore = asset.balanceOf(address(fle));

        vm.expectRevert(
            abi.encodeWithSelector(
                FL_ArbitrageExecutionFailed.selector, abi.encodeWithSelector(LengthMismatch.selector)
            )
        );
        fle.requestFlashLoan(address(asset), 1_000e18, params);

        // Atomicity: the loan disbursement reverted with the callback — the FLE
        // is left exactly as it started (o ambas patas o ninguna).
        assertEq(asset.balanceOf(address(fle)), fleBalBefore, "repay-or-revert invariant broken");
        assertEq(asset.balanceOf(address(fle)), 0, "loan leaked to FLE after revert");
    }

    // =====================================================================
    // FL_RepaymentShortfall(uint256 available, uint256 owed)
    // =====================================================================

    /// @dev Selector 0xd51bee48 (cast keccak). Profit (5) < premium (10): after
    ///      a SUCCESSFUL arbitrage call the FLE cannot cover amount+premium and
    ///      must revert naming available vs. owed. The repay-or-revert invariant
    ///      is enforced by the provider's own revert (nothing is transferred to
    ///      the pool: repayment was never attempted).
    function test_AC01_FL_RepaymentShortfall_CarriesArgs() public {
        uint256 amount = 1_000e18;
        uint256 premium = 10e18;
        uint256 boost = 5e18; // route profit < premium

        AC_Token asset = new AC_Token();
        executor.setTokenApproval(address(asset), true);

        AC_MockAavePoolRT pool = new AC_MockAavePoolRT(address(asset), premium);
        pool.fund(amount);

        FlashLoanExecutor fleImpl = new FlashLoanExecutor();
        bytes memory fleInit =
            abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, admin, address(pool), address(executor));
        ERC1967Proxy fleProxy = new ERC1967Proxy(address(fleImpl), fleInit);
        FlashLoanExecutor fle = FlashLoanExecutor(payable(address(fleProxy)));
        fle.grantRole(fle.EXECUTOR_ROLE(), address(this));

        // The FLE calls executeArbitrageFlashFunded as msg.sender → it needs
        // EXECUTOR_ROLE on the ArbitrageExecutor.
        executor.grantRole(executor.EXECUTOR_ROLE(), address(fle));

        // Profitable-but-insufficient route: pull amount, return amount + 5.
        AC_EchoProfitRouter router = new AC_EchoProfitRouter(address(asset), address(asset), amount, boost);
        _approveRouter(address(router));
        asset.mint(address(router), amount + boost);

        bytes memory params = abi.encodeWithSelector(
            ArbitrageExecutor.executeArbitrageFlashFunded.selector,
            bytes32(0),
            address(asset),
            address(asset),
            amount,
            0,
            _oneLeg(address(router)),
            _onePayload()
        );

        // FLE after the round trip: amount + 5; owed: amount + 10.
        vm.expectRevert(abi.encodeWithSelector(FL_RepaymentShortfall.selector, amount + boost, amount + premium));
        fle.requestFlashLoan(address(asset), amount, params);

        // Nothing reached the pool: repayment never happened, the whole tx
        // reverted (available was asserted BEFORE any transfer).
        assertEq(asset.balanceOf(address(pool)), amount, "pool balance must be untouched");
    }

    // =====================================================================
    // Selector pins (cast keccak, foundry 1.7.2-nightly c5e44b5e)
    // =====================================================================

    /// @dev The signatures introduced/extended by ATOMIC-CAUSE-01 pinned to
    ///      their `cast keccak` outputs. If a signature drifts, the pin fails —
    ///      the sim/bindings decode table stays honest.
    function test_AC01_Selectors_Pinned() public pure {
        assertEq(
            bytes4(keccak256("InsufficientBalance(uint256,uint256)")),
            bytes4(0xcf479181),
            "InsufficientBalance(uint256,uint256)"
        );
        assertEq(
            bytes4(keccak256("SwapFailed(uint256,address,bytes)")),
            bytes4(0xd3512921),
            "SwapFailed(uint256,address,bytes)"
        );
        assertEq(
            bytes4(keccak256("ZeroGrossProfit(uint256,uint256)")),
            bytes4(0x4422c101),
            "ZeroGrossProfit(uint256,uint256)"
        );
        assertEq(
            bytes4(keccak256("FlashFundedPullMismatch(uint256,uint256)")),
            bytes4(0x97191793),
            "FlashFundedPullMismatch(uint256,uint256)"
        );
        assertEq(
            bytes4(keccak256("FlashFundedCapitalRetentionViolation(uint256,uint256)")),
            bytes4(0x31a7cece),
            "FlashFundedCapitalRetentionViolation(uint256,uint256)"
        );
        assertEq(
            bytes4(keccak256("InsufficientProfit(uint256,uint256)")),
            bytes4(0x4e88422a),
            "InsufficientProfit(uint256,uint256)"
        );
        assertEq(
            bytes4(keccak256("TokenOutRetentionViolation(uint256,uint256)")),
            bytes4(0xff597268),
            "TokenOutRetentionViolation(uint256,uint256)"
        );
        assertEq(
            bytes4(keccak256("AccountingDeltaUnderflow(uint256,uint256)")),
            bytes4(0x03bb1851),
            "AccountingDeltaUnderflow(uint256,uint256)"
        );
        assertEq(
            bytes4(keccak256("ETHRescueFailed(address,bytes)")), bytes4(0x57b4b222), "ETHRescueFailed(address,bytes)"
        );
        assertEq(
            bytes4(keccak256("FL_ArbitrageExecutionFailed(bytes)")),
            bytes4(0x0f70bbda),
            "FL_ArbitrageExecutionFailed(bytes)"
        );
        assertEq(
            bytes4(keccak256("FL_RepaymentShortfall(uint256,uint256)")),
            bytes4(0xd51bee48),
            "FL_RepaymentShortfall(uint256,uint256)"
        );
    }
}
