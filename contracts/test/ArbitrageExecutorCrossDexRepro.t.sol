// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// ROOT-CAUSE REPRO (systematic-debugging Phase 4.1): the deployed
// ArbitrageExecutor._runRoute can ONLY execute routes where every hop spends
// `tokenIn` (it grants each router an ephemeral allowance for `tokenIn` only —
// ArbitrageExecutor.sol:377 `_boundedRouterCall(tokenIn, router, amountIn, pld)`).
// The off-chain system, however, detects and encodes CROSS-DEX 2-router arbs
// (forward leg on DEX A: tokenIn->tokenOut; backward leg on DEX B:
// tokenOut->tokenIn — sim_encoder.rs:462-463 resolves two distinct routers).
// The backward leg spends `tokenOut`, which the executor never approves, so the
// backward router's transferFrom(tokenOut) reverts -> low-level call fails ->
// SwapFailed. This test proves that mismatch empirically, with NO contract change.
//
// Every pre-existing ArbitrageExecutor test uses single-token mock routers
// (pull tokenIn, return tokenIn), so this execution path was never exercised.
// ---------------------------------------------------------------------------

/// @dev Minimal ERC20 with public mint.
contract ReproToken is ERC20 {
    constructor(string memory s) ERC20(s, s) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev A FAITHFUL DEX router: pulls `inputAmount` of `inputToken` from the caller
///      (the ArbitrageExecutor) via transferFrom — exercising the executor's ephemeral
///      per-router ERC20 allowance — then delivers `outputAmount` of `outputToken` back.
///      This is what a real Uniswap-style swap does: the router pulls the token it was
///      approved for and returns the other token. Unlike the existing single-token mocks,
///      input and output tokens DIFFER — the essence of a 2-leg cross-DEX arbitrage.
contract ReproCrossDexRouter {
    IERC20 public immutable inputToken;
    IERC20 public immutable outputToken;
    uint256 public immutable inputAmount;
    uint256 public immutable outputAmount;

    constructor(address inToken, address outToken, uint256 inAmt, uint256 outAmt) {
        inputToken = IERC20(inToken);
        outputToken = IERC20(outToken);
        inputAmount = inAmt;
        outputAmount = outAmt;
    }

    fallback() external {
        // Pull the INPUT token from the executor (requires the executor to have approved
        // THIS token to this router), then deliver the OUTPUT token back.
        inputToken.transferFrom(msg.sender, address(this), inputAmount);
        outputToken.transfer(msg.sender, outputAmount);
    }
}

/// @dev Sender-surcharge ("reflection"/deflationary-on-send) ERC20: the RECIPIENT always
///      receives `value` IN FULL, but when the configured `feeFrom` account is the SENDER an
///      EXTRA `feeBps` is skimmed from it on top of the main move. Mirrors MockOutboundFeeERC20
///      in ArbitrageExecutor.t.sol. As a cross-DEX tokenOut this is the HIGH-finding token:
///      leg-1's outbound transferFrom(executor -> router) of the intermediate delivers the
///      router its full amount, but additionally debits the executor `feeBps` — draining
///      pre-existing tokenOut working capital B_out with NO revert in the faithful accounting.
contract ReproSenderSurchargeToken is ERC20 {
    address private constant SINK = address(0xdead);
    address public feeFrom;
    uint256 public feeBps; // e.g. 100 = 1%

    constructor(string memory s) ERC20(s, s) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function setSurcharge(address account, uint256 bps) external {
        feeFrom = account;
        feeBps = bps;
    }

    function _update(address from, address to, uint256 value) internal override {
        super._update(from, to, value); // faithful main move: recipient gets `value` in full
        // Sender-pays surcharge ONLY when the configured account sends on a real transfer.
        if (feeFrom != address(0) && from == feeFrom && to != address(0) && feeBps > 0) {
            uint256 fee = (value * feeBps) / 10_000;
            if (fee > 0) super._update(from, SINK, fee);
        }
    }
}

/// @dev Single-token circular router: pulls tokenIn, returns more tokenIn. This is the
///      ONLY shape `_runRoute` supports (the hop spends tokenIn). Positive control.
contract ReproCircularRouter {
    ReproToken public immutable token;
    uint256 public immutable pullAmount;
    uint256 public immutable returnAmount;

    constructor(address t, uint256 pull, uint256 ret) {
        token = ReproToken(t);
        pullAmount = pull;
        returnAmount = ret;
    }

    fallback() external {
        token.transferFrom(msg.sender, address(this), pullAmount);
        token.transfer(msg.sender, returnAmount);
    }
}

/// @dev Cross-DEX router that pulls STRICTLY LESS input than it was approved for, then
///      delivers the output. Used to prove the leg-1 ephemeral allowance is RESET to 0 by
///      the executor (not merely consumed by the router's transferFrom): even though this
///      router leaves part of its allowance unspent, the executor must zero it after the call.
contract ReproUnderspendRouter {
    IERC20 public immutable inputToken;
    IERC20 public immutable outputToken;
    uint256 public immutable pullAmount; // < the approved (intermediate) amount
    uint256 public immutable outputAmount;

    constructor(address inToken, address outToken, uint256 pullAmt, uint256 outAmt) {
        inputToken = IERC20(inToken);
        outputToken = IERC20(outToken);
        pullAmount = pullAmt;
        outputAmount = outAmt;
    }

    fallback() external {
        inputToken.transferFrom(msg.sender, address(this), pullAmount);
        outputToken.transfer(msg.sender, outputAmount);
    }
}

contract ArbitrageExecutorCrossDexReproTest is Test {
    ArbitrageExecutor internal executor;
    address internal admin;
    address internal executorRole;

    bytes4 internal constant SWAP_SELECTOR = bytes4(keccak256("swap()"));

    function setUp() public {
        admin = address(this);
        executorRole = makeAddr("executorRole");

        ArbitrageExecutor impl = new ArbitrageExecutor();
        bytes memory initData = abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, admin);
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        executor = ArbitrageExecutor(payable(address(proxy)));

        executor.grantRole(executor.EXECUTOR_ROLE(), executorRole);
    }

    /// TARGET SPEC (1a): a real cross-DEX 2-router round trip COMPLETES. Leg 0 approves
    /// tokenIn (amountIn); leg 1 approves the tokenOut INTERMEDIATE produced by leg 0
    /// (its on-chain delta), so the backward router can pull the tokenOut it needs and
    /// returns more tokenIn. The executor's tokenIn balance rises by (finalOut - amountIn).
    function test_CrossDexTwoRouter_Succeeds() public {
        ReproToken tokenIn = new ReproToken("IN");
        ReproToken tokenOut = new ReproToken("OUT");

        uint256 amountIn = 1_000e18;
        uint256 intermediate = 900e18; // tokenOut received from the forward leg
        uint256 finalOut = 1_100e18; // tokenIn received from the backward leg (+100 profit)

        // Forward router A: pull tokenIn(amountIn) -> deliver tokenOut(intermediate).
        ReproCrossDexRouter routerA =
            new ReproCrossDexRouter(address(tokenIn), address(tokenOut), amountIn, intermediate);
        // Backward router B: pull tokenOut(intermediate) -> deliver tokenIn(finalOut).
        ReproCrossDexRouter routerB =
            new ReproCrossDexRouter(address(tokenOut), address(tokenIn), intermediate, finalOut);

        // Fund: executor holds the principal; routers hold their output inventories.
        tokenIn.mint(address(executor), amountIn);
        tokenOut.mint(address(routerA), intermediate);
        tokenIn.mint(address(routerB), finalOut);

        // Allowlists the contract requires (token + router + selector).
        executor.setTokenApproval(address(tokenIn), true);
        executor.setTokenApproval(address(tokenOut), true);
        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), SWAP_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), SWAP_SELECTOR, true);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);

        uint256 balBefore = tokenIn.balanceOf(address(executor));

        // Leg 0 approves tokenIn(amountIn); leg 1 approves the tokenOut delta (intermediate)
        // produced by leg 0 -> backward router pulls it -> round trip completes.
        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenOut), amountIn, 0, routers, payloads);

        // tokenIn balance rose by exactly (finalOut - amountIn) = +100; no tokenOut left over.
        assertEq(tokenIn.balanceOf(address(executor)), balBefore + (finalOut - amountIn));
        assertEq(tokenOut.balanceOf(address(executor)), 0);
    }

    /// POSITIVE CONTROL: the ONLY shape `_runRoute` supports — a single hop that spends
    /// tokenIn and returns more tokenIn — succeeds. Proves the revert above is specifically
    /// the cross-token (tokenOut) approval gap, not a harness artifact.
    function test_Control_SingleHopSpendsTokenIn_Succeeds() public {
        ReproToken tokenIn = new ReproToken("IN");

        uint256 amountIn = 1_000e18;
        uint256 returnAmount = 1_100e18; // +100 tokenIn profit

        ReproCircularRouter routerC = new ReproCircularRouter(address(tokenIn), amountIn, returnAmount);

        tokenIn.mint(address(executor), amountIn);
        tokenIn.mint(address(routerC), returnAmount);

        executor.setTokenApproval(address(tokenIn), true);
        executor.setRouterApproval(address(routerC), true);
        executor.setRouterSelectorApproval(address(routerC), SWAP_SELECTOR, true);

        address[] memory routers = new address[](1);
        routers[0] = address(routerC);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);

        // tokenIn == tokenOut (circular): the single hop spends and returns tokenIn -> succeeds.
        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenIn), amountIn, 0, routers, payloads);

        // The route pulled amountIn and returned returnAmount, so the executor retains the proceeds.
        assertEq(tokenIn.balanceOf(address(executor)), returnAmount);
    }

    // =========================================================================
    // (1a) SECURITY GATE TESTS for the reworked _runRoute
    // =========================================================================

    /// SC-13 (tokenOut working-capital retention): the executor pre-holds B_out of tokenOut
    /// as working capital. A cross-DEX route must consume ONLY the intermediate IT produced
    /// (leg-0 delta), leaving the pre-existing B_out untouched. Proves the backward leg
    /// approves the route's own delta, NOT the executor's full tokenOut balance.
    function test_Security_TokenOutWorkingCapitalRetained() public {
        ReproToken tokenIn = new ReproToken("IN");
        ReproToken tokenOut = new ReproToken("OUT");

        uint256 amountIn = 1_000e18;
        uint256 intermediate = 900e18; // tokenOut produced by leg 0
        uint256 finalOut = 1_100e18; // tokenIn from leg 1 (+100 profit)
        uint256 preExistingTokenOut = 500e18; // executor's OWN tokenOut working capital B_out

        ReproCrossDexRouter routerA =
            new ReproCrossDexRouter(address(tokenIn), address(tokenOut), amountIn, intermediate);
        ReproCrossDexRouter routerB =
            new ReproCrossDexRouter(address(tokenOut), address(tokenIn), intermediate, finalOut);

        tokenIn.mint(address(executor), amountIn);
        tokenOut.mint(address(executor), preExistingTokenOut); // B_out — must be retained
        tokenOut.mint(address(routerA), intermediate);
        tokenIn.mint(address(routerB), finalOut);

        executor.setTokenApproval(address(tokenIn), true);
        executor.setTokenApproval(address(tokenOut), true);
        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), SWAP_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), SWAP_SELECTOR, true);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenOut), amountIn, 0, routers, payloads);

        // Backward leg approved only the leg-0 delta (intermediate), so it pulled exactly
        // that and the pre-existing B_out is fully retained.
        assertEq(tokenOut.balanceOf(address(executor)), preExistingTokenOut, "tokenOut working capital B_out leaked");
        // tokenIn ends at finalOut (principal amountIn was spent then finalOut returned);
        // i.e. profit = finalOut - amountIn = +100, unaffected by the extra tokenOut capital.
        assertEq(tokenIn.balanceOf(address(executor)), finalOut);
    }

    /// (1a) Route-length gate: routers.length > 2 reverts fail-closed with UnsupportedRouteLength.
    function test_Security_RouteLengthAboveTwoReverts() public {
        ReproToken tokenIn = new ReproToken("IN");
        tokenIn.mint(address(executor), 1_000e18);
        executor.setTokenApproval(address(tokenIn), true);

        // 3 routers (approved as tokenIn would be irrelevant — length gate fires first).
        ReproCircularRouter r = new ReproCircularRouter(address(tokenIn), 1, 1);
        executor.setRouterApproval(address(r), true);
        executor.setRouterSelectorApproval(address(r), SWAP_SELECTOR, true);

        address[] memory routers = new address[](3);
        routers[0] = address(r);
        routers[1] = address(r);
        routers[2] = address(r);
        bytes[] memory payloads = new bytes[](3);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);
        payloads[2] = abi.encodePacked(SWAP_SELECTOR);

        vm.prank(executorRole);
        vm.expectRevert(abi.encodeWithSelector(UnsupportedRouteLength.selector, uint256(3)));
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenIn), 1_000e18, 0, routers, payloads);
    }

    /// SC-12 (leg-1 exact ephemeral allowance reset): after a cross-DEX route the executor's
    /// tokenOut allowance to the backward router is 0 — even when the router UNDERSPENDS its
    /// approval. Proves the executor itself resets the leg-1 allowance (not just consumption).
    function test_Security_Leg1EphemeralAllowanceResetToZero() public {
        ReproToken tokenIn = new ReproToken("IN");
        ReproToken tokenOut = new ReproToken("OUT");

        uint256 amountIn = 1_000e18;
        uint256 intermediate = 900e18; // tokenOut produced by leg 0 = leg-1 approval
        uint256 pulledByB = 400e18; // routerB deliberately pulls LESS than approved
        uint256 finalOut = 1_100e18;

        ReproCrossDexRouter routerA =
            new ReproCrossDexRouter(address(tokenIn), address(tokenOut), amountIn, intermediate);
        // Underspend router: approved `intermediate` but only pulls `pulledByB`.
        ReproUnderspendRouter routerB =
            new ReproUnderspendRouter(address(tokenOut), address(tokenIn), pulledByB, finalOut);

        tokenIn.mint(address(executor), amountIn);
        tokenOut.mint(address(routerA), intermediate);
        tokenIn.mint(address(routerB), finalOut);

        executor.setTokenApproval(address(tokenIn), true);
        executor.setTokenApproval(address(tokenOut), true);
        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), SWAP_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), SWAP_SELECTOR, true);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenOut), amountIn, 0, routers, payloads);

        // Leg-1 allowance was reset to 0 by the executor despite the router underspending it.
        assertEq(tokenOut.allowance(address(executor), address(routerB)), 0, "leg-1 tokenOut allowance not reset");
        // Leg-0 allowance to routerA is also 0 (defense-in-depth).
        assertEq(tokenIn.allowance(address(executor), address(routerA)), 0, "leg-0 tokenIn allowance not reset");
    }

    /// Profit gate (no silent loss): a cross-DEX route that nets a LOSS (finalOut < amountIn)
    /// reverts ZeroGrossProfit — the round trip cannot complete at a loss.
    function test_Security_CrossDexNetLossReverts() public {
        ReproToken tokenIn = new ReproToken("IN");
        ReproToken tokenOut = new ReproToken("OUT");

        uint256 amountIn = 1_000e18;
        uint256 intermediate = 900e18;
        uint256 finalOut = 950e18; // < amountIn -> net LOSS of 50

        ReproCrossDexRouter routerA =
            new ReproCrossDexRouter(address(tokenIn), address(tokenOut), amountIn, intermediate);
        ReproCrossDexRouter routerB =
            new ReproCrossDexRouter(address(tokenOut), address(tokenIn), intermediate, finalOut);

        tokenIn.mint(address(executor), amountIn);
        tokenOut.mint(address(routerA), intermediate);
        tokenIn.mint(address(routerB), finalOut);

        executor.setTokenApproval(address(tokenIn), true);
        executor.setTokenApproval(address(tokenOut), true);
        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), SWAP_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), SWAP_SELECTOR, true);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);

        // finalOut (950) < amountIn (1000): balanceAfter <= balanceBefore -> ZeroGrossProfit,
        // the whole tx reverts atomically (no silent loss, no partial state).
        vm.prank(executorRole);
        vm.expectRevert(ZeroGrossProfit.selector);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenOut), amountIn, 0, routers, payloads);
    }

    // =========================================================================
    // HIGH (execution-core audit): symmetric tokenOut capital-retention guard
    // =========================================================================

    /// HIGH repro: the cross-DEX 2-leg path measured profit + capital retention ONLY in
    /// tokenIn. Leg 1 approves the tokenOut delta the route produced — which preserves a
    /// FAITHFUL tokenOut's pre-existing B_out. But a SENDER-SURCHARGE tokenOut makes leg-1's
    /// outbound transferFrom(executor -> router) of `intermediate` additionally debit the
    /// executor a fee, draining B_out with NO revert in the tokenIn-only accounting. This
    /// asserts the route now REVERTS TokenOutRetentionViolation and B_out is fully retained.
    ///
    /// Red→green: before the post-leg1 guard this route did NOT revert and the executor lost
    /// `fee` of its pre-existing tokenOut B_out; with the guard it fail-closes and rolls back.
    function test_Security_TokenOutSenderSurchargeReverts() public {
        ReproToken tokenIn = new ReproToken("IN");
        ReproSenderSurchargeToken tokenOut = new ReproSenderSurchargeToken("OUT");
        // Surcharge applies ONLY when the EXECUTOR is the sender (its outbound leg-1 transfer).
        tokenOut.setSurcharge(address(executor), 100); // 1%

        uint256 amountIn = 1_000e18;
        uint256 intermediate = 900e18; // tokenOut produced by leg 0
        uint256 finalOut = 1_100e18; // tokenIn from leg 1 (+100 tokenIn profit)
        uint256 B_out = 500e18; // executor's OWN tokenOut working capital — must be retained

        ReproCrossDexRouter routerA =
            new ReproCrossDexRouter(address(tokenIn), address(tokenOut), amountIn, intermediate);
        ReproCrossDexRouter routerB =
            new ReproCrossDexRouter(address(tokenOut), address(tokenIn), intermediate, finalOut);

        tokenIn.mint(address(executor), amountIn);
        tokenOut.mint(address(executor), B_out); // B_out — must be retained on revert
        tokenOut.mint(address(routerA), intermediate);
        tokenIn.mint(address(routerB), finalOut);

        executor.setTokenApproval(address(tokenIn), true);
        executor.setTokenApproval(address(tokenOut), true);
        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), SWAP_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), SWAP_SELECTOR, true);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);

        // Leg-1's outbound surcharge would drain `fee` of B_out; the post-leg1 guard catches
        // the executor's tokenOut balance falling below its pre-route snapshot and reverts.
        vm.prank(executorRole);
        vm.expectRevert(TokenOutRetentionViolation.selector);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenOut), amountIn, 0, routers, payloads);

        // Atomic revert: B_out retained EXACTLY — no silent leak of pre-existing tokenOut.
        assertEq(tokenOut.balanceOf(address(executor)), B_out, "tokenOut working capital B_out leaked");
        // tokenIn principal untouched (route rolled back).
        assertEq(tokenIn.balanceOf(address(executor)), amountIn, "tokenIn principal not restored on revert");
    }

    /// (LOW) Aliased 2-leg shape: a 2-router route declared with tokenIn == tokenOut is a
    /// shape the off-chain never emits (cross-DEX routes have distinct tokens; circular routes
    /// are 1 leg). The 2-leg tokenOut-delta accounting is undefined for an aliased token, so it
    /// is rejected fail-closed with a named revert — making the route-shape gate exhaustive.
    function test_Security_AliasedTwoLegSameTokenReverts() public {
        ReproToken tokenIn = new ReproToken("IN");

        uint256 amountIn = 1_000e18;
        tokenIn.mint(address(executor), amountIn);
        executor.setTokenApproval(address(tokenIn), true);

        ReproCircularRouter rA = new ReproCircularRouter(address(tokenIn), amountIn, amountIn);
        ReproCircularRouter rB = new ReproCircularRouter(address(tokenIn), amountIn, amountIn);
        executor.setRouterApproval(address(rA), true);
        executor.setRouterApproval(address(rB), true);
        executor.setRouterSelectorApproval(address(rA), SWAP_SELECTOR, true);
        executor.setRouterSelectorApproval(address(rB), SWAP_SELECTOR, true);

        address[] memory routers = new address[](2);
        routers[0] = address(rA);
        routers[1] = address(rB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);

        // tokenIn == tokenOut on a 2-leg route: rejected before any swap dispatches.
        vm.prank(executorRole);
        vm.expectRevert(AliasedTwoLegRoute.selector);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenIn), amountIn, 0, routers, payloads);
    }

    /// (1a) Zero-intermediate gate: a 2-leg cross-DEX route whose FORWARD router delivers
    /// ZERO tokenOut (the leg-0 delta == 0) reverts ZeroIntermediate BEFORE leg-1 dispatch.
    /// Closes the one route-shape gate branch (`_runRoute`: `if (intermediate == 0) revert
    /// ZeroIntermediate()`) that lacked a direct red->green assertion — the aliased test above
    /// hits AliasedTwoLegRoute first, so it never exercises this guard. Here tokenIn != tokenOut
    /// and the route is well-formed up to the forward leg producing no output.
    function test_Security_ZeroIntermediateReverts() public {
        ReproToken tokenIn = new ReproToken("IN");
        ReproToken tokenOut = new ReproToken("OUT");

        uint256 amountIn = 1_000e18;

        // Forward router A: pulls tokenIn(amountIn) but delivers ZERO tokenOut -> the
        // executor's tokenOut balance delta (the intermediate) is 0.
        ReproCrossDexRouter routerA = new ReproCrossDexRouter(address(tokenIn), address(tokenOut), amountIn, 0);
        // Backward router B is well-formed but MUST NEVER be reached (the guard fires on the
        // leg-0 delta, before leg-1 dispatch).
        ReproCrossDexRouter routerB = new ReproCrossDexRouter(address(tokenOut), address(tokenIn), 1, 1_100e18);

        tokenIn.mint(address(executor), amountIn); // routerA delivers 0 tokenOut -> needs no inventory

        executor.setTokenApproval(address(tokenIn), true);
        executor.setTokenApproval(address(tokenOut), true);
        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), SWAP_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), SWAP_SELECTOR, true);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(SWAP_SELECTOR);
        payloads[1] = abi.encodePacked(SWAP_SELECTOR);

        // Forward leg produces a 0 tokenOut delta -> ZeroIntermediate, fail-closed, before
        // any leg-1 approval/dispatch.
        vm.prank(executorRole);
        vm.expectRevert(ZeroIntermediate.selector);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenOut), amountIn, 0, routers, payloads);
    }
}
