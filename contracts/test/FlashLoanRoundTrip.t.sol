// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-13: FlashLoanRoundTrip — end-to-end proof that the flash-loan fund-handoff
// gap (§7) is closed.
//
// Before SC-13, FlashLoanExecutor `forceApprove`d ArbitrageExecutor for the
// borrowed amount but the executor never pulled it (executeArbitrage required a
// pre-funded balance and never transferFrom'd), so a real flash-loan arbitrage
// reverted InsufficientBalance and could not execute on ANY provider path.
//
// This suite wires the REAL FlashLoanExecutor + REAL ArbitrageExecutor + a
// realistic Balancer-style provider that actually disburses the loan and collects
// repayment, and drives the full path via requestFlashLoan(). It asserts the
// borrow -> fund -> trade -> return -> repay -> net-profit round trip completes,
// and that an unprofitable route reverts the whole flash loan atomically.
// =============================================================================

import "forge-std/Test.sol";
import "../src/ArbitrageExecutor.sol";
import "../src/FlashLoanExecutor.sol";
import "../src/interfaces/IFlashLoanProvider.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

contract RTToken is ERC20 {
    constructor() ERC20("RoundTrip", "RT") {}
    function mint(address to, uint256 amount) external { _mint(to, amount); }
}

/// @dev Router that mints `profit` of RTToken to the executor on call — simulates a
///      swap that nets positive output back into the executor's balance.
contract RTProfitRouter {
    RTToken public token;
    address public executor;
    uint256 public profit;
    constructor(address _token, address _executor, uint256 _profit) {
        token = RTToken(_token);
        executor = _executor;
        profit = _profit;
    }
    fallback() external { token.mint(executor, profit); }
}

/// @dev Router that does nothing — produces no gross profit (drives the revert path).
contract RTZeroRouter {
    fallback() external {}
}

/// @dev Balancer-style flash provider implementing IFlashLoanProvider. Holds its own
///      liquidity, disburses `amount` to the receiver, invokes the Balancer callback
///      (receiveFlashLoan), and is repaid by the receiver's safeTransfer back to it.
///      Doubles as the `balancerVault` trust anchor (receiveFlashLoan checks msg.sender).
contract RTBalancerProvider is IFlashLoanProvider {
    function flashLoan(address receiver, address asset, uint256 amount, bytes calldata params) external override {
        uint256 balBefore = IERC20(asset).balanceOf(address(this));
        IERC20(asset).transfer(receiver, amount); // disburse the loan

        IERC20[] memory tokens = new IERC20[](1);
        tokens[0] = IERC20(asset);
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory fees = new uint256[](1);
        fees[0] = 0; // Balancer V2: 0% fee

        // The receiver repays via safeTransfer(msg.sender == this, amount) inside the callback.
        FlashLoanExecutor(receiver).receiveFlashLoan(tokens, amounts, fees, params);

        // Provider must be made whole (loan fully repaid) by the end of the callback.
        require(IERC20(asset).balanceOf(address(this)) >= balBefore, "loan not repaid");
    }
    function flashLoanFee(uint256) external pure override returns (uint256) { return 0; }
    function maxFlashLoan(address) external pure override returns (uint256) { return type(uint256).max; }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

contract FlashLoanRoundTripTest is Test {
    ArbitrageExecutor internal executor;
    FlashLoanExecutor internal flashExec;
    RTBalancerProvider internal provider;
    RTToken internal token;

    address internal admin;
    address internal executorRole; // holds EXECUTOR_ROLE on FlashLoanExecutor
    bytes4 internal constant MOCK_SELECTOR = bytes4(keccak256("swap()"));

    uint256 internal constant LIQUIDITY = 1_000_000e18;

    function setUp() public {
        admin = address(this);
        executorRole = makeAddr("executorRole");
        token = new RTToken();

        // Real ArbitrageExecutor (UUPS proxy).
        ArbitrageExecutor aeImpl = new ArbitrageExecutor();
        executor = ArbitrageExecutor(payable(address(new ERC1967Proxy(
            address(aeImpl),
            abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, admin)
        ))));

        // Real FlashLoanExecutor (UUPS proxy) pointing at the real executor.
        // aavePool is unused on the provider path; pass a throwaway non-zero address.
        FlashLoanExecutor flImpl = new FlashLoanExecutor();
        flashExec = FlashLoanExecutor(address(new ERC1967Proxy(
            address(flImpl),
            abi.encodeWithSelector(FlashLoanExecutor.initialize.selector, admin, makeAddr("aavePool"), address(executor))
        )));

        provider = new RTBalancerProvider();
        token.mint(address(provider), LIQUIDITY); // seed provider liquidity

        // Wiring:
        // - FlashLoanExecutor must hold EXECUTOR_ROLE on the executor (the §7 deploy requirement).
        executor.grantRole(executor.EXECUTOR_ROLE(), address(flashExec));
        // - executorRole may request flash loans.
        flashExec.grantRole(flashExec.EXECUTOR_ROLE(), executorRole);
        // - provider is both the IFlashLoanProvider and the trusted Balancer vault.
        flashExec.setFlashLoanProvider(address(provider));
        flashExec.setBalancerVault(address(provider));
        // - executor approves the loan token.
        executor.setTokenApproval(address(token), true);
    }

    function _wireProfitRouter(uint256 profit) internal returns (address[] memory routers, bytes[] memory payloads) {
        RTProfitRouter router = new RTProfitRouter(address(token), address(executor), profit);
        executor.setRouterApproval(address(router), true);
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);
        routers = new address[](1);
        routers[0] = address(router);
        payloads = new bytes[](1);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);
    }

    function _flashParams(
        uint256 amountIn,
        uint256 minProfit,
        address[] memory routers,
        bytes[] memory payloads
    ) internal view returns (bytes memory) {
        return abi.encodeWithSelector(
            ArbitrageExecutor.executeArbitrageFlashFunded.selector,
            bytes32("route"),
            address(token),
            address(token),
            amountIn,
            minProfit,
            routers,
            payloads
        );
    }

    // The headline test: a profitable flash-loan arbitrage executes end to end.
    function testRoundTrip_ProfitableFlashArb_RepaysAndNets() public {
        uint256 amount = 10_000e18;
        uint256 profit = 200e18;

        (address[] memory routers, bytes[] memory payloads) = _wireProfitRouter(profit);
        bytes memory params = _flashParams(amount, 1e18, routers, payloads);

        vm.prank(executorRole);
        flashExec.requestFlashLoan(address(token), amount, params);

        // Provider is whole (0% fee); the executor netted to zero; the net profit
        // remains in the FlashLoanExecutor (the borrower keeps the spread).
        assertEq(token.balanceOf(address(provider)), LIQUIDITY, "provider fully repaid");
        assertEq(token.balanceOf(address(executor)), 0, "executor holds nothing after the round trip");
        assertEq(token.balanceOf(address(flashExec)), profit, "net profit retained by the borrower");
    }

    // An unprofitable route must revert the WHOLE flash loan atomically — nothing moves.
    function testRoundTrip_UnprofitableFlashArb_RevertsAtomically() public {
        uint256 amount = 10_000e18;

        RTZeroRouter router = new RTZeroRouter();
        executor.setRouterApproval(address(router), true);
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);
        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        bytes memory params = _flashParams(amount, 0, routers, payloads);

        vm.prank(executorRole);
        vm.expectRevert(); // executor reverts ZeroGrossProfit -> FL_ArbitrageExecutionFailed -> bubbles up
        flashExec.requestFlashLoan(address(token), amount, params);

        // Atomic: provider liquidity intact, nothing stranded anywhere.
        assertEq(token.balanceOf(address(provider)), LIQUIDITY, "provider liquidity intact after revert");
        assertEq(token.balanceOf(address(executor)), 0, "executor holds nothing after revert");
        assertEq(token.balanceOf(address(flashExec)), 0, "flash executor holds nothing after revert");
    }

    // Regression guard: the OLD broken behaviour. If params encode the SELF-funded
    // executeArbitrage (which never pulls), the executor sees a zero balance and the
    // flash loan reverts — confirming the round trip specifically requires the new
    // flash-funded entrypoint, and that self-funded execution is unfunded here.
    function testRoundTrip_SelfFundedEntrypoint_StillRevertsUnfunded() public {
        uint256 amount = 10_000e18;
        (address[] memory routers, bytes[] memory payloads) = _wireProfitRouter(200e18);

        bytes memory params = abi.encodeWithSelector(
            ArbitrageExecutor.executeArbitrage.selector,
            bytes32("route"), address(token), address(token), amount, 1e18, routers, payloads
        );

        vm.prank(executorRole);
        vm.expectRevert(); // executeArbitrage never pulls -> InsufficientBalance -> bubbles up
        flashExec.requestFlashLoan(address(token), amount, params);

        assertEq(token.balanceOf(address(provider)), LIQUIDITY, "provider intact");
    }
}

// =============================================================================
// Fork-level smoke test: exercise the flash-funded entrypoint against a REAL
// token's transfer semantics (mainnet WETH). Self-skips when MAINNET_RPC_URL is
// unset (local / non-fork CI); runs in the dedicated `test-fork` CI job. Proves
// the pull + atomic refund work with a production ERC20, not just a mock.
// Real-DEX swap fork tests (the F1–F6 matrix) belong to the typed-adapter PR.
// =============================================================================
contract FlashFundedForkTest is Test {
    address internal constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    bytes4 internal constant MOCK_SELECTOR = bytes4(keccak256("swap()"));
    bool internal forkActive;

    function setUp() public {
        string memory rpc = vm.envOr("MAINNET_RPC_URL", string(""));
        if (bytes(rpc).length == 0) {
            forkActive = false;
            return;
        }
        vm.createSelectFork(rpc);
        forkActive = true;
    }

    function testFork_FlashFunded_RealWETH_PullsAndRefundsOnUnprofitable() public {
        if (!forkActive) { vm.skip(true); return; }

        address admin = address(this);
        ArbitrageExecutor executor = ArbitrageExecutor(payable(address(new ERC1967Proxy(
            address(new ArbitrageExecutor()),
            abi.encodeWithSelector(ArbitrageExecutor.initialize.selector, admin)
        ))));
        executor.setTokenApproval(WETH, true);

        // A zero-profit router so the route reverts ZeroGrossProfit after the real pull.
        RTZeroRouter router = new RTZeroRouter();
        executor.setRouterApproval(address(router), true);
        executor.setRouterSelectorApproval(address(router), MOCK_SELECTOR, true);
        address[] memory routers = new address[](1);
        routers[0] = address(router);
        bytes[] memory payloads = new bytes[](1);
        payloads[0] = abi.encodePacked(MOCK_SELECTOR);

        uint256 amount = 5e18;
        address funder = makeAddr("forkFunder");
        executor.grantRole(executor.EXECUTOR_ROLE(), funder);
        deal(WETH, funder, amount); // real WETH balance via storage cheatcode

        vm.startPrank(funder);
        IERC20(WETH).approve(address(executor), amount);
        vm.expectRevert(); // ZeroGrossProfit — real WETH pulled then atomically refunded
        executor.executeArbitrageFlashFunded(bytes32(0), WETH, WETH, amount, 0, routers, payloads);
        vm.stopPrank();

        // Atomic: the real WETH pull rolled back; funder whole, executor holds none.
        assertEq(IERC20(WETH).balanceOf(funder), amount, "real WETH refunded on revert");
        assertEq(IERC20(WETH).balanceOf(address(executor)), 0, "executor holds no WETH");
    }
}
