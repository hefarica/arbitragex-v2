// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Cross-DEX (2-router) arbitrage repro.
//
// Root cause: _runRoute → _boundedRouterCall historically approved `tokenIn` for
// EVERY hop. In a true 2-router arb (tokenIn → tokenOut on router A, then
// tokenOut → tokenIn on router B) the RETURN leg sells `tokenOut`, so router B
// must be able to pull `tokenOut`. Because only `tokenIn` was ever approved,
// router B's transferFrom(tokenOut) reverted → SwapFailed. The whole class of
// genuine cross-DEX arbs was unexecutable.
//
// Why the existing suite never caught it: every other ArbitrageExecutor test uses
// a SINGLE token (tokenIn == tokenOut), and the mock routers mint/pull that same
// token. No existing test makes the second hop pull a DIFFERENT asset, so the
// missing tokenOut approval was never exercised.
//
// The fix approves the per-hop SOLD token: hop 0 sells tokenIn (capped at amountIn
// to protect working capital); later hops sell tokenOut (capped at the held
// balance received). No ABI change — tokenIn and tokenOut are already in the
// executeArbitrage signature.
// ---------------------------------------------------------------------------

/// @dev Minimal mintable ERC20 for this repro (kept local to avoid coupling to the
///      mocks in ArbitrageExecutor.t.sol).
contract XToken is ERC20 {
    constructor(string memory n, string memory s) ERC20(n, s) {}
    function mint(address to, uint256 amount) external { _mint(to, amount); }
}

/// @dev Router that PULLS `pullAmount` of `tokenInPull` from the caller (the
///      executor) via transferFrom — exercising the executor's per-hop allowance
///      for that SOLD token — then mints `giveAmount` of `tokenOutGive` to the
///      caller. Modelled on MockPullRouter but CROSS-TOKEN, which is the whole
///      point: the return leg pulls a different asset than the principal.
contract CrossDexRouter {
    XToken public tokenInPull;
    XToken public tokenOutGive;
    uint256 public pullAmount;
    uint256 public giveAmount;

    constructor(address _pull, address _give, uint256 _pullAmount, uint256 _giveAmount) {
        tokenInPull = XToken(_pull);
        tokenOutGive = XToken(_give);
        pullAmount = _pullAmount;
        giveAmount = _giveAmount;
    }

    fallback() external {
        // Requires the executor to have approved THIS router for tokenInPull.
        tokenInPull.transferFrom(msg.sender, address(this), pullAmount);
        tokenOutGive.mint(msg.sender, giveAmount);
    }
}

/// @dev Malicious (but allowlisted + selector-whitelisted) return-leg router that abuses the
///      per-hop intermediate-token allowance: it pulls the FULL approved `pullToken` balance
///      (standing inventory + interim) to `attacker`, then mints `giveAmount` of `giveToken`
///      back so the tokenIn-only net-profit gate still sees a "profit". Models the in-scope
///      adversary (compromised EXECUTOR_ROLE / malicious approved router) from the contract's
///      A5 / SC-13 threat model.
contract MaliciousDrainRouter {
    XToken public pullToken;
    XToken public giveToken;
    address public attacker;
    uint256 public giveAmount;

    constructor(address _pull, address _give, address _attacker, uint256 _giveAmount) {
        pullToken = XToken(_pull);
        giveToken = XToken(_give);
        attacker = _attacker;
        giveAmount = _giveAmount;
    }

    fallback() external {
        // Attempt to grab the executor's ENTIRE balance of the intermediate token (standing
        // inventory + interim), not just the route-created delta. With the per-hop delta cap
        // the executor approved only the in-flight delta, so this over-pull exceeds the
        // allowance and reverts (InsufficientAllowance) -> bubbles up as SwapFailed; the
        // standing inventory is never reachable.
        uint256 amt = pullToken.balanceOf(msg.sender);
        pullToken.transferFrom(msg.sender, attacker, amt);
        // Return enough tokenIn so the tokenIn-only profit gate would otherwise pass.
        giveToken.mint(msg.sender, giveAmount);
    }
}

contract ArbitrageExecutorCrossDexTest is Test {
    ArbitrageExecutor internal executor;
    XToken internal tokenA; // principal / tokenIn
    XToken internal tokenB; // intermediate / tokenOut

    address internal admin;
    address internal executorRole;

    bytes4 internal constant MOCK_SELECTOR = bytes4(keccak256("swap()"));

    function setUp() public {
        admin = address(this);
        executorRole = makeAddr("executorRole");

        // Deploy via ERC1967Proxy + initialize() — same UUPS pattern as the main suite.
        ArbitrageExecutor impl = new ArbitrageExecutor();
        bytes memory initData = abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, admin);
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        executor = ArbitrageExecutor(payable(address(proxy)));

        tokenA = new XToken("TokenA", "A");
        tokenB = new XToken("TokenB", "B");

        executor.grantRole(executor.EXECUTOR_ROLE(), executorRole);

        // Both legs' tokens must be approved (tokenIn and the intermediate tokenOut).
        executor.setTokenApproval(address(tokenA), true);
        executor.setTokenApproval(address(tokenB), true);
    }

    // -----------------------------------------------------------------------
    // The repro: a genuine 2-router cross-DEX arb must execute and capture profit.
    //   hop 0 (routerA): sell amountIn of A  -> receive interimB of B
    //   hop 1 (routerB): sell interimB of B  -> receive finalA of A (finalA > amountIn)
    // RED before the fix (router B cannot pull B → SwapFailed). GREEN after.
    // -----------------------------------------------------------------------
    function test_CrossDex_TwoRouterArb_CapturesProfit() public {
        uint256 amountIn = 1_000e18;
        uint256 interimB = 1_000e18;
        uint256 finalA = 1_050e18; // +50 profit in tokenA
        uint256 minProfit = 1e18;

        CrossDexRouter routerA = new CrossDexRouter(address(tokenA), address(tokenB), amountIn, interimB);
        CrossDexRouter routerB = new CrossDexRouter(address(tokenB), address(tokenA), interimB, finalA);

        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), MOCK_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), MOCK_SELECTOR, true);

        tokenA.mint(address(executor), amountIn);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);
        payloads[1] = abi.encodePacked(MOCK_SELECTOR);

        vm.prank(executorRole);
        executor.executeArbitrage(bytes32(0), address(tokenA), address(tokenB), amountIn, minProfit, routers, payloads);

        // Profit captured in the principal token; no intermediate residue; no lingering allowance.
        assertEq(tokenA.balanceOf(address(executor)), finalA, "executor captured cross-DEX profit in tokenA");
        assertEq(tokenB.balanceOf(address(executor)), 0, "no intermediate (tokenB) residue");
        assertEq(tokenA.allowance(address(executor), address(routerA)), 0, "no residual A allowance to routerA");
        assertEq(tokenB.allowance(address(executor), address(routerB)), 0, "no residual B allowance to routerB");
    }

    // -----------------------------------------------------------------------
    // Guardrail (passes before AND after the fix): the cross-DEX change must NOT
    // weaken the SC-12 principal cap. Even in a 2-hop route where the executor
    // holds far more than amountIn of the principal, hop 0's ephemeral allowance
    // is still bounded by amountIn — an over-pull on the principal leg reverts.
    // -----------------------------------------------------------------------
    function test_CrossDex_PrincipalLegStillCappedAtAmountIn() public {
        uint256 amountIn = 1_000e18;

        // routerA tries to over-pull the principal by 1 wei beyond the amountIn allowance.
        CrossDexRouter routerA = new CrossDexRouter(address(tokenA), address(tokenB), amountIn + 1, 1_000e18);
        CrossDexRouter routerB = new CrossDexRouter(address(tokenB), address(tokenA), 1_000e18, 1_050e18);

        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), MOCK_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), MOCK_SELECTOR, true);

        // Fund FAR more than amountIn so only the cap (not the balance) can stop the over-pull.
        tokenA.mint(address(executor), amountIn * 3);

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);
        payloads[1] = abi.encodePacked(MOCK_SELECTOR);

        vm.prank(executorRole);
        vm.expectRevert(SwapFailed.selector);
        executor.executeArbitrage(bytes32(0), address(tokenA), address(tokenB), amountIn, 1e18, routers, payloads);

        assertEq(tokenA.balanceOf(address(executor)), amountIn * 3, "principal balance untouched after revert");
    }

    // -----------------------------------------------------------------------
    // Security guard (found by adversarial review): a naive per-hop intermediate-token
    // allowance of balanceOf(tokenOut) would expose the executor's STANDING tokenOut
    // inventory to a malicious/compromised allowlisted router — and the net-profit gate is
    // tokenIn-only, blind to a tokenOut drain. The per-hop DELTA CAP approves only the
    // route-created amount, so a router attempting to pull the full balance over-pulls its
    // allowance and reverts (SwapFailed); the post-loop tokenOut retention assertion is the
    // defense-in-depth backstop. Standing inventory must survive intact.
    // -----------------------------------------------------------------------
    function test_CrossDex_StandingTokenOutInventory_NotDrainable() public {
        uint256 amountIn = 1_000e18;
        uint256 standingB = 100e18; // standing tokenB inventory the executor already holds
        uint256 interimB = 1e18;    // tiny interim produced by the honest hop0
        uint256 giveBackA = 1_050e18; // enough tokenA to clear the tokenIn-only profit gate

        address attacker = makeAddr("attacker");

        // hop0 honest: A -> B (small interim)
        CrossDexRouter routerA = new CrossDexRouter(address(tokenA), address(tokenB), amountIn, interimB);
        // hop1 malicious: drains the FULL approved B to attacker, mints A back
        MaliciousDrainRouter routerB = new MaliciousDrainRouter(address(tokenB), address(tokenA), attacker, giveBackA);

        executor.setRouterApproval(address(routerA), true);
        executor.setRouterApproval(address(routerB), true);
        executor.setRouterSelectorApproval(address(routerA), MOCK_SELECTOR, true);
        executor.setRouterSelectorApproval(address(routerB), MOCK_SELECTOR, true);

        tokenA.mint(address(executor), amountIn);
        tokenB.mint(address(executor), standingB); // standing inventory at risk

        address[] memory routers = new address[](2);
        routers[0] = address(routerA);
        routers[1] = address(routerB);
        bytes[] memory payloads = new bytes[](2);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);
        payloads[1] = abi.encodePacked(MOCK_SELECTOR);

        // Delta cap: routerB is approved only the route-created interim, so its attempt to
        // pull the executor's full tokenB balance (standing + interim) over-pulls and reverts.
        vm.prank(executorRole);
        vm.expectRevert(SwapFailed.selector);
        executor.executeArbitrage(bytes32(0), address(tokenA), address(tokenB), amountIn, 1e18, routers, payloads);

        // Atomic revert: standing inventory intact, attacker got nothing.
        assertEq(tokenB.balanceOf(address(executor)), standingB, "standing tokenB inventory retained");
        assertEq(tokenB.balanceOf(attacker), 0, "attacker drained nothing");
    }
}
