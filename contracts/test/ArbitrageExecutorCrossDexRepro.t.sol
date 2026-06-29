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

    /// ROOT CAUSE: a real cross-DEX 2-router round trip reverts at the BACKWARD leg,
    /// because the executor only ever grants an allowance for `tokenIn`, never `tokenOut`.
    function test_RootCause_CrossDexTwoRouter_RevertsAtBackwardLeg() public {
        ReproToken tokenIn = new ReproToken("IN");
        ReproToken tokenOut = new ReproToken("OUT");

        uint256 amountIn = 1_000e18;
        uint256 intermediate = 900e18; // tokenOut received from the forward leg
        uint256 finalOut = 1_100e18; // tokenIn received from the backward leg (would be +100 profit)

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

        // Forward leg succeeds (spends tokenIn, approved). Backward leg pulls tokenOut,
        // which the executor never approved to routerB -> transferFrom reverts -> SwapFailed.
        vm.prank(executorRole);
        vm.expectRevert(SwapFailed.selector);
        executor.executeArbitrage(bytes32(0), address(tokenIn), address(tokenOut), amountIn, 0, routers, payloads);
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
}
