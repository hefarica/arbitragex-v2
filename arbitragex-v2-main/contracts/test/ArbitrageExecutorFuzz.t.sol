// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-11: ArbitrageExecutor — Foundry fuzz tests
//
// Properties under test:
//   F1. Access control: arbitrary non-executor caller always reverts.
//   F2. Profit threshold: if gross profit < minProfit, always reverts.
//   F3. Balance gate: if balance < amountIn, always reverts.
//   F4. Profit correctness: on a clean swap, profit == grossProfit.
//   F5. minProfit = 0 is always valid when gross profit > 0.
//   F6. Token not approved: always reverts TokenNotApproved.
//
// All inputs are bounded via `bound()` to realistic operational ranges.
// Runs: controlled by foundry.toml [fuzz] runs = 256.
// =============================================================================

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Shared mock contracts (duplicated here to keep this file self-contained)
// ---------------------------------------------------------------------------

/// @dev Minimal ERC20 with public mint for fuzz setup.
contract FuzzMockERC20 is ERC20 {
    constructor() ERC20("FuzzToken", "FTK") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Router that mints a configurable fixed profit amount to the executor.
///      Used in F2, F4, F5.
contract FuzzProfitRouter {
    FuzzMockERC20 public token;
    address public target;
    uint256 public profitAmount;

    constructor(address _token, address _target, uint256 _profit) {
        token = FuzzMockERC20(_token);
        target = _target;
        profitAmount = _profit;
    }

    fallback() external {
        token.mint(target, profitAmount);
    }
}

/// @dev Router that does nothing — gross profit stays zero.
///      Used in F3 balance gate (balance check fires before router call).
contract FuzzNoopRouter {
    fallback() external {}
}

// ---------------------------------------------------------------------------
// Fuzz test contract
// ---------------------------------------------------------------------------

contract ArbitrageExecutorFuzzTest is Test {
    ArbitrageExecutor internal executor;
    FuzzMockERC20 internal token;

    address internal admin;
    address internal execRole;

    // Operational bounds
    uint256 internal constant MIN_AMOUNT = 1;
    uint256 internal constant MAX_AMOUNT = type(uint128).max; // ~340T of 18-dec token

    function setUp() public {
        admin    = address(this);
        execRole = makeAddr("execRole");

        ArbitrageExecutor impl = new ArbitrageExecutor();
        bytes memory initData  = abi.encodeWithSelector(
            ArbitrageExecutor.initialize.selector,
            admin
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        executor = ArbitrageExecutor(payable(address(proxy)));

        token = new FuzzMockERC20();

        executor.grantRole(executor.EXECUTOR_ROLE(), execRole);
        executor.setTokenApproval(address(token), true);
    }

    // -----------------------------------------------------------------------
    // F1: Access control — arbitrary non-executor always reverts NotExecutor
    // -----------------------------------------------------------------------
    /// @dev Any address that does not hold EXECUTOR_ROLE must be rejected.
    ///      We explicitly exclude the execRole address from the fuzzer domain.
    function testFuzz_AE_F1_NonExecutorAlwaysReverts(address caller) public {
        vm.assume(caller != execRole);
        vm.assume(!executor.hasRole(executor.EXECUTOR_ROLE(), caller));

        address[] memory routers  = new address[](0);
        bytes[]   memory payloads = new bytes[](0);

        vm.expectRevert(NotExecutor.selector);
        vm.prank(caller);
        executor.executeArbitrage(bytes32(0), address(token), address(token), 0, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // F2: Profit threshold — gross profit < minProfit always reverts
    // -----------------------------------------------------------------------
    /// @dev The router produces exactly `grossProfit` tokens of gain.
    ///      If minProfit > grossProfit the call must revert InsufficientProfit.
    function testFuzz_AE_F2_InsufficientProfitReverts(
        uint256 amountIn,
        uint256 grossProfit,
        uint256 extraMinProfit
    ) public {
        amountIn      = bound(amountIn,      MIN_AMOUNT, MAX_AMOUNT);
        grossProfit   = bound(grossProfit,   1,          MAX_AMOUNT);
        // minProfit is strictly greater than grossProfit
        extraMinProfit = bound(extraMinProfit, 1, MAX_AMOUNT - grossProfit);
        uint256 minProfit = grossProfit + extraMinProfit;

        token.mint(address(executor), amountIn);

        FuzzProfitRouter router = new FuzzProfitRouter(address(token), address(executor), grossProfit);
        executor.setRouterApproval(address(router), true);

        address[] memory routers  = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

        vm.expectRevert(InsufficientProfit.selector);
        vm.prank(execRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, minProfit, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // F3: Balance gate — balance < amountIn always reverts InsufficientBalance
    // -----------------------------------------------------------------------
    /// @dev The executor holds `balance` tokens.  We request `balance + gap`
    ///      which always exceeds the actual balance.
    function testFuzz_AE_F3_InsufficientBalanceReverts(
        uint256 balance,
        uint256 gap
    ) public {
        balance = bound(balance, 0,          MAX_AMOUNT - 1);
        gap     = bound(gap,     1,          MAX_AMOUNT - balance);
        uint256 amountIn = balance + gap;

        if (balance > 0) {
            token.mint(address(executor), balance);
        }

        FuzzNoopRouter router = new FuzzNoopRouter();
        executor.setRouterApproval(address(router), true);

        address[] memory routers  = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

        vm.expectRevert(InsufficientBalance.selector);
        vm.prank(execRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // F4: Profit correctness — event and balance reflect exact gross profit
    // -----------------------------------------------------------------------
    /// @dev When grossProfit >= minProfit, the call succeeds and the executor's
    ///      net balance gain equals grossProfit exactly.
    function testFuzz_AE_F4_ProfitCorrect(
        uint256 amountIn,
        uint256 grossProfit,
        uint256 minProfit
    ) public {
        amountIn    = bound(amountIn,    MIN_AMOUNT, MAX_AMOUNT);
        grossProfit = bound(grossProfit, 1,          MAX_AMOUNT);
        // minProfit is at most grossProfit (so the call succeeds)
        minProfit   = bound(minProfit,   0,          grossProfit);

        token.mint(address(executor), amountIn);
        uint256 balanceBefore = token.balanceOf(address(executor));

        FuzzProfitRouter router = new FuzzProfitRouter(address(token), address(executor), grossProfit);
        executor.setRouterApproval(address(router), true);

        address[] memory routers  = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

        vm.prank(execRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, minProfit, routers, payloads);

        uint256 balanceAfter = token.balanceOf(address(executor));
        assertEq(balanceAfter - balanceBefore, grossProfit, "net balance gain must equal grossProfit");
    }

    // -----------------------------------------------------------------------
    // F5: minProfit = 0 always succeeds when gross profit > 0
    // -----------------------------------------------------------------------
    /// @dev minProfit = 0 is the weakest acceptable guard.  Any positive
    ///      gross profit must pass it.
    function testFuzz_AE_F5_ZeroMinProfitAcceptsAnyPositiveProfit(
        uint256 amountIn,
        uint256 grossProfit
    ) public {
        amountIn    = bound(amountIn,    MIN_AMOUNT, MAX_AMOUNT);
        grossProfit = bound(grossProfit, 1,          MAX_AMOUNT);

        token.mint(address(executor), amountIn);

        FuzzProfitRouter router = new FuzzProfitRouter(address(token), address(executor), grossProfit);
        executor.setRouterApproval(address(router), true);

        address[] memory routers  = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = "";

        // Must not revert
        vm.prank(execRole);
        executor.executeArbitrage(bytes32(0), address(token), address(token), amountIn, 0, routers, payloads);
    }

    // -----------------------------------------------------------------------
    // F6: Token not approved — arbitrary unapproved token always reverts
    // -----------------------------------------------------------------------
    /// @dev We deploy a fresh ERC20 and never call setTokenApproval on it.
    ///      Any call with that token must revert TokenNotApproved.
    function testFuzz_AE_F6_UnapprovedTokenAlwaysReverts(uint256 amountIn) public {
        amountIn = bound(amountIn, MIN_AMOUNT, MAX_AMOUNT);

        FuzzMockERC20 unapproved = new FuzzMockERC20();
        unapproved.mint(address(executor), amountIn);

        address[] memory routers  = new address[](0);
        bytes[]   memory payloads = new bytes[](0);

        vm.expectRevert(abi.encodeWithSelector(TokenNotApproved.selector, address(unapproved)));
        vm.prank(execRole);
        executor.executeArbitrage(bytes32(0), address(unapproved), address(unapproved), amountIn, 0, routers, payloads);
    }
}
