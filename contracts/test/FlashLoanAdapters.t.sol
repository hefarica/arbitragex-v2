// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-1 + SC-2: Combined adapter tests
//
// Covers:
//   SC-1: AaveV3FlashAdapter, BalancerFlashAdapter (full impl)
//         UniV3FlashAdapter, DyDxFlashAdapter (skeleton, vm.skip)
//         FlashLoanExecutor: setFlashLoanProvider, selectCheapestProvider
//   SC-2: CurveAdapter, PancakeV3Adapter (full impl)
//         GMXAdapter, SynthetixAdapter (skeleton, vm.skip)
//
// Skeletons use vm.skip(true) - they document the architecture is ready
// but the underlying integration is non-trivial (see each adapter's header).
// =============================================================================

import "forge-std/Test.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "../src/FlashLoanExecutor.sol";
import "../src/interfaces/IFlashLoanProvider.sol";
import "../src/interfaces/IDEXAdapter.sol";
import "../src/flashloans/AaveV3FlashAdapter.sol";
import "../src/flashloans/BalancerFlashAdapter.sol";
import "../src/flashloans/UniV3FlashAdapter.sol";
import "../src/flashloans/DyDxFlashAdapter.sol";
import "../src/dexes/CurveAdapter.sol";
import "../src/dexes/PancakeV3Adapter.sol";
import "../src/dexes/GMXAdapter.sol";
import "../src/dexes/SynthetixAdapter.sol";

// =============================================================================
// Shared test token
// =============================================================================

contract MockERC20Adapter is ERC20 {
    constructor() ERC20("AdapterToken", "AT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

// =============================================================================
// Mock Aave V3 Pool (minimal - records calls only)
// =============================================================================

contract MockAaveV3PoolSC1 {
    address public lastReceiver;
    address public lastAsset;
    uint256 public lastAmount;

    function flashLoanSimple(address receiverAddress, address asset, uint256 amount, bytes calldata, uint16) external {
        lastReceiver = receiverAddress;
        lastAsset = asset;
        lastAmount = amount;
    }
}

// =============================================================================
// Mock Balancer Vault (records call, does NOT transfer tokens)
// =============================================================================

contract MockBalancerVault {
    address public lastRecipient;
    address public lastToken;
    uint256 public lastAmount;

    uint256 public balance; // simulated token balance

    function setBalance(uint256 _balance) external {
        balance = _balance;
    }

    function flashLoan(
        address recipient,
        address[] calldata tokens,
        uint256[] calldata amounts,
        bytes calldata /*userData*/
    )
        external
    {
        lastRecipient = recipient;
        lastToken = tokens[0];
        lastAmount = amounts[0];
        // In a real Balancer, tokens would be transferred and receiveFlashLoan called.
        // Not simulated here - tested separately via direct receiveFlashLoan call.
    }
}

// =============================================================================
// Mock token balance oracle for Balancer maxFlashLoan test
// =============================================================================

contract MockTokenVaultBalance is ERC20 {
    constructor() ERC20("VaultToken", "VT") {}

    function setBalanceFor(address target, uint256 amount) external {
        // Mint to target (vault) to simulate vault liquidity
        _mint(target, amount);
    }
}

// =============================================================================
// Mock Curve Pool
// =============================================================================

contract MockCurvePool {
    uint256 public constant MOCK_RATE = 1010; // 1.01 USDC per DAI (10 bps gain)
    uint256 public constant MOCK_DENOMINATOR = 1000;

    // Coins registered by index, mirroring a real Curve pool. Without this the mock
    // only returned a number; the adapter then had no tokenOut to forward and held
    // residual tokenIn. A real exchange() pulls tokenIn and pays tokenOut to msg.sender.
    address[2] public coins;

    function setCoins(address c0, address c1) external {
        coins[0] = c0;
        coins[1] = c1;
    }

    function exchange(int128 i, int128 j, uint256 dx, uint256 min_dy) external returns (uint256 dy) {
        dy = (dx * MOCK_RATE) / MOCK_DENOMINATOR;
        require(dy >= min_dy, "MockCurvePool: slippage");
        // Pull tokenIn (the adapter approved us) and pay tokenOut, as a real pool does.
        MockERC20Adapter(coins[uint256(uint128(i))]).transferFrom(msg.sender, address(this), dx);
        MockERC20Adapter(coins[uint256(uint128(j))]).transfer(msg.sender, dy);
    }

    function get_dy(int128, int128, uint256 dx) external pure returns (uint256) {
        return (dx * 1010) / 1000;
    }
}

// =============================================================================
// Mock PancakeSwap V3 Router
// =============================================================================

contract MockPancakeV3Router {
    MockERC20Adapter internal _tokenOut;
    uint256 public constant MOCK_RATE = 990; // 0.99 output per input (0.1% fee)

    constructor(address tokenOut) {
        _tokenOut = MockERC20Adapter(tokenOut);
    }

    function exactInputSingle(ISwapRouterTest.ExactInputSingleParams calldata params)
        external
        returns (uint256 amountOut)
    {
        amountOut = (params.amountIn * MOCK_RATE) / 1000;
        require(amountOut >= params.amountOutMinimum, "MockPancakeRouter: slippage");
        // Mint tokenOut directly to recipient (simulates router swap)
        _tokenOut.mint(params.recipient, amountOut);
    }
}

/// @dev Minimal interface for test - matches PancakeV3Adapter's internal ISwapRouter.
interface ISwapRouterTest {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }
}

// =============================================================================
// Mock FlashLoanExecutor (minimal stub for provider selection test)
// =============================================================================

contract MockFlashLoanExecutorHelper is Test {
    FlashLoanExecutor public executor;
    MockAaveV3PoolSC1 public mockAave;
    MockERC20Adapter public token;

    function deployExecutor() public {
        mockAave = new MockAaveV3PoolSC1();
        token = new MockERC20Adapter();
        FlashLoanExecutor impl = new FlashLoanExecutor();
        bytes memory initData = abi.encodeWithSelector(
            FlashLoanExecutor.initialize.selector,
            address(this), // admin
            address(mockAave), // aavePool
            address(0xBEEF) // arbitrageExecutor (stub)
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        executor = FlashLoanExecutor(address(proxy));
    }
}

// =============================================================================
// SC-1: AaveV3FlashAdapter tests
// =============================================================================

contract AaveV3FlashAdapterTest is Test {
    AaveV3FlashAdapter internal adapter;
    MockAaveV3PoolSC1 internal pool;
    MockERC20Adapter internal token;

    function setUp() public {
        pool = new MockAaveV3PoolSC1();
        token = new MockERC20Adapter();
        adapter = new AaveV3FlashAdapter(address(pool));
    }

    /// @notice Constructor rejects zero address.
    function testConstructor_RejectsZeroPool() public {
        vm.expectRevert("AaveV3FlashAdapter: zero pool");
        new AaveV3FlashAdapter(address(0));
    }

    /// @notice flashLoan delegates to Aave flashLoanSimple with correct args.
    function testFlashLoan_DelegatesToAave() public {
        bytes memory params = abi.encode(uint256(1));
        adapter.flashLoan(address(this), address(token), 1000e18, params);

        assertEq(pool.lastReceiver(), address(this), "receiver mismatch");
        assertEq(pool.lastAsset(), address(token), "asset mismatch");
        assertEq(pool.lastAmount(), 1000e18, "amount mismatch");
    }

    /// @notice flashLoanFee returns 9 bps of amount.
    function testFlashLoanFee_Returns9Bps() public view {
        uint256 amount = 10_000e18;
        uint256 fee = adapter.flashLoanFee(amount);
        // 10_000e18 * 9 / 10_000 = 9e18
        assertEq(fee, 9e18, "fee must be 0.09%");
    }

    /// @notice flashLoanFee rounds down correctly for small amounts.
    function testFlashLoanFee_SmallAmount() public view {
        // 100 wei * 9 / 10_000 = 0 (rounds down)
        assertEq(adapter.flashLoanFee(100), 0);
        // 10_001 wei * 9 / 10_000 = 9 wei
        assertEq(adapter.flashLoanFee(10_001), 9);
    }

    /// @notice maxFlashLoan returns max uint256 (no hard limit on adapter side).
    function testMaxFlashLoan_ReturnsMaxUint() public view {
        assertEq(adapter.maxFlashLoan(address(token)), type(uint256).max);
    }

    /// @notice aavePool is stored correctly.
    function testConstructor_StoresPool() public view {
        assertEq(address(adapter.aavePool()), address(pool));
    }
}

// =============================================================================
// SC-1: BalancerFlashAdapter tests
// =============================================================================

contract BalancerFlashAdapterTest is Test {
    BalancerFlashAdapter internal adapter;
    MockBalancerVault internal vault;
    MockTokenVaultBalance internal token;

    function setUp() public {
        vault = new MockBalancerVault();
        token = new MockTokenVaultBalance();
        adapter = new BalancerFlashAdapter(address(vault));
    }

    /// @notice Constructor rejects zero address.
    function testConstructor_RejectsZeroVault() public {
        vm.expectRevert("BalancerFlashAdapter: zero vault");
        new BalancerFlashAdapter(address(0));
    }

    /// @notice flashLoan wraps into Balancer array interface and forwards to vault.
    function testFlashLoan_DelegatesToVault() public {
        bytes memory params = abi.encode(uint256(99));
        adapter.flashLoan(address(this), address(token), 500e18, params);

        assertEq(vault.lastRecipient(), address(this), "recipient mismatch");
        assertEq(vault.lastToken(), address(token), "token mismatch");
        assertEq(vault.lastAmount(), 500e18, "amount mismatch");
    }

    /// @notice flashLoanFee always returns 0 (Balancer V2 is free).
    function testFlashLoanFee_AlwaysZero() public view {
        assertEq(adapter.flashLoanFee(0), 0);
        assertEq(adapter.flashLoanFee(1e18), 0);
        assertEq(adapter.flashLoanFee(type(uint256).max / 2), 0);
    }

    /// @notice maxFlashLoan returns the vault's token balance.
    function testMaxFlashLoan_ReturnsVaultBalance() public {
        // Mint tokens to vault to simulate liquidity
        token.setBalanceFor(address(vault), 42_000e18);
        uint256 max = adapter.maxFlashLoan(address(token));
        assertEq(max, 42_000e18, "maxFlashLoan must equal vault balance");
    }

    /// @notice maxFlashLoan returns 0 when vault holds no tokens.
    function testMaxFlashLoan_ZeroWhenEmpty() public view {
        assertEq(adapter.maxFlashLoan(address(token)), 0);
    }

    /// @notice vault address is stored correctly.
    function testConstructor_StoresVault() public view {
        assertEq(address(adapter.vault()), address(vault));
    }
}

// =============================================================================
// SC-1: UniV3FlashAdapter skeleton tests (vm.skip)
// =============================================================================

contract UniV3FlashAdapterTest is Test {
    /// @notice SC-1 follow-up: UniV3 flash requires pool-level callback wiring.
    ///         Full implementation in dedicated session when UniV3 flash is prioritized.
    function testTODO_UniV3FlashAdapter_NotImplemented() public {
        vm.skip(true);
        // When implemented:
        // - UniV3 flash is per-pool, not per-vault
        // - Callback is uniswapV3FlashCallback(fee0, fee1, data)
        // - Fee = pool's swap fee tier (100/500/3000/10000 bps / 100)
        // - FlashLoanExecutor must implement uniswapV3FlashCallback
    }
}

// =============================================================================
// SC-1: DyDxFlashAdapter skeleton tests (vm.skip)
// =============================================================================

contract DyDxFlashAdapterTest is Test {
    /// @notice SC-1 follow-up: dYdX uses ISoloMargin.operate() with 3 actions.
    ///         Full implementation after viability assessment (protocol frozen).
    function testTODO_DyDxFlashAdapter_NotImplemented() public {
        vm.skip(true);
        // When implemented:
        // - Encode Account.Info + 3 ActionArgs (Withdraw/Call/Deposit)
        // - Map token to dYdX marketId (WETH=0, USDC=2, DAI=3)
        // - Fee = 1 wei (effectively 0%)
        // - Implement ICallee.callFunction callback on FlashLoanExecutor
    }
}

// =============================================================================
// SC-1: FlashLoanExecutor.setFlashLoanProvider + selectCheapestProvider tests
// =============================================================================

contract FlashLoanExecutorProviderTest is Test {
    FlashLoanExecutor internal executor;
    AaveV3FlashAdapter internal aaveAdapter;
    BalancerFlashAdapter internal balancerAdapter;
    MockAaveV3PoolSC1 internal mockAavePool;
    MockBalancerVault internal mockVault;
    MockTokenVaultBalance internal token;

    address internal admin;
    address internal attacker;

    function setUp() public {
        admin = address(this);
        attacker = makeAddr("attacker");

        mockAavePool = new MockAaveV3PoolSC1();
        mockVault = new MockBalancerVault();
        token = new MockTokenVaultBalance();

        aaveAdapter = new AaveV3FlashAdapter(address(mockAavePool));
        balancerAdapter = new BalancerFlashAdapter(address(mockVault));

        FlashLoanExecutor impl = new FlashLoanExecutor();
        bytes memory initData = abi.encodeWithSelector(
            FlashLoanExecutor.initialize.selector, admin, address(mockAavePool), address(0xBEEF)
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        executor = FlashLoanExecutor(address(proxy));
    }

    /// @notice setFlashLoanProvider stores the address and emits event.
    function testSetFlashLoanProvider_StoresAndEmits() public {
        vm.expectEmit(true, false, false, false, address(executor));
        emit FlashLoanExecutor.FlashLoanProviderUpdated(address(balancerAdapter));

        executor.setFlashLoanProvider(address(balancerAdapter));
        assertEq(executor.flashLoanProvider(), address(balancerAdapter));
    }

    /// @notice setFlashLoanProvider reverts for non-admin.
    function testSetFlashLoanProvider_OnlyAdmin() public {
        vm.expectRevert();
        vm.prank(attacker);
        executor.setFlashLoanProvider(address(balancerAdapter));
    }

    /// @notice setFlashLoanProvider accepts address(0) (clears provider, reverts to legacy Aave).
    function testSetFlashLoanProvider_AcceptsZero() public {
        executor.setFlashLoanProvider(address(balancerAdapter));
        executor.setFlashLoanProvider(address(0)); // clear
        assertEq(executor.flashLoanProvider(), address(0));
    }

    /// @notice selectCheapestProvider returns Balancer when it has sufficient liquidity.
    function testSelectCheapestProvider_PrefersBalancer() public {
        // Give vault 1M tokens
        token.setBalanceFor(address(mockVault), 1_000_000e18);
        uint256 amount = 100_000e18;

        (address cheaper, uint256 fee) =
            executor.selectCheapestProvider(address(aaveAdapter), address(balancerAdapter), address(token), amount);

        assertEq(cheaper, address(balancerAdapter), "Balancer (0%) must be preferred");
        assertEq(fee, 0, "Balancer fee must be 0");
    }

    /// @notice selectCheapestProvider falls back to Aave when Balancer has no liquidity.
    function testSelectCheapestProvider_FallsBackToAave() public {
        // Vault is empty - Balancer cannot service the loan
        uint256 amount = 100_000e18;

        (address cheaper, uint256 fee) =
            executor.selectCheapestProvider(address(aaveAdapter), address(balancerAdapter), address(token), amount);

        assertEq(cheaper, address(aaveAdapter), "Aave must be returned when Balancer is empty");
        // Aave fee: 100_000e18 * 9 / 10_000 = 90e18
        assertEq(fee, 90e18, "Aave fee must be 0.09%");
    }

    /// @notice selectCheapestProvider returns (address(0), 0) when neither can service.
    function testSelectCheapestProvider_NeitherAvailable() public {
        // Both address(0) - neither provider configured
        (address cheaper, uint256 fee) = executor.selectCheapestProvider(address(0), address(0), address(token), 1e18);

        assertEq(cheaper, address(0), "must return zero when no provider available");
        assertEq(fee, 0);
    }

    /// @notice flashLoanProvider slot defaults to address(0) after initialize.
    function testInitialize_ProviderDefaultsToZero() public view {
        assertEq(executor.flashLoanProvider(), address(0));
    }
}

// =============================================================================
// SC-2: CurveAdapter tests
// =============================================================================

contract CurveAdapterTest is Test {
    CurveAdapter internal adapter;
    MockCurvePool internal pool;
    MockERC20Adapter internal dai;
    MockERC20Adapter internal usdc;

    address internal caller;

    function setUp() public {
        caller = makeAddr("caller");
        pool = new MockCurvePool();
        dai = new MockERC20Adapter();
        usdc = new MockERC20Adapter();
        adapter = new CurveAdapter();

        // Register coins so exchange() can move funds by index (i=0 dai, j=1 usdc).
        pool.setCoins(address(dai), address(usdc));

        // Seed the pool with USDC so it can "pay out" on exchange
        usdc.mint(address(pool), 100_000e18);
    }

    /// @notice swap pulls tokenIn, calls exchange, returns tokenOut.
    function testSwap_ExecutesExchange() public {
        uint256 amountIn = 1000e18;
        dai.mint(caller, amountIn);

        vm.startPrank(caller);
        dai.approve(address(adapter), amountIn);

        bytes memory extraData = abi.encode(address(pool), int128(0), int128(1));
        uint256 amountOut = adapter.swap(address(dai), address(usdc), amountIn, 0, extraData);
        vm.stopPrank();

        // MockCurvePool returns (amountIn * 1010) / 1000 = 1010e18
        assertEq(amountOut, 1010e18, "amountOut mismatch");
        // Caller received tokenOut
        assertEq(usdc.balanceOf(caller), 1010e18, "caller must hold tokenOut");
        // Adapter holds no residual tokens
        assertEq(dai.balanceOf(address(adapter)), 0, "adapter must hold no DAI residual");
    }

    /// @notice swap reverts with CA_ZeroPoolAddress when pool is zero.
    function testSwap_RevertsOnZeroPool() public {
        uint256 amountIn = 100e18;
        dai.mint(caller, amountIn);

        vm.startPrank(caller);
        dai.approve(address(adapter), amountIn);

        bytes memory extraData = abi.encode(address(0), int128(0), int128(1));
        vm.expectRevert(CurveAdapter.CA_ZeroPoolAddress.selector);
        adapter.swap(address(dai), address(usdc), amountIn, 0, extraData);
        vm.stopPrank();
    }

    /// @notice swap reverts when pool's exchange output is below minAmountOut.
    function testSwap_RevertsOnSlippage() public {
        uint256 amountIn = 1000e18;
        dai.mint(caller, amountIn);

        vm.startPrank(caller);
        dai.approve(address(adapter), amountIn);

        bytes memory extraData = abi.encode(address(pool), int128(0), int128(1));
        // minAmountOut > mock rate output (1010e18)
        uint256 minAmountOut = 2000e18;
        vm.expectRevert();
        adapter.swap(address(dai), address(usdc), amountIn, minAmountOut, extraData);
        vm.stopPrank();
    }

    /// @notice quoteOut delegates to pool.get_dy without state change.
    function testQuoteOut_DelegatesToGetDy() public view {
        bytes memory extraData = abi.encode(address(pool), int128(0), int128(1));
        uint256 quote = adapter.quoteOut(address(dai), address(usdc), 1000e18, extraData);
        assertEq(quote, 1010e18, "quote must match get_dy output");
    }

    /// @notice quoteOut reverts with CA_ZeroPoolAddress when pool is zero.
    function testQuoteOut_RevertsOnZeroPool() public {
        bytes memory extraData = abi.encode(address(0), int128(0), int128(1));
        vm.expectRevert(CurveAdapter.CA_ZeroPoolAddress.selector);
        adapter.quoteOut(address(dai), address(usdc), 1000e18, extraData);
    }
}

// =============================================================================
// SC-2: PancakeV3Adapter tests
// =============================================================================

contract PancakeV3AdapterTest is Test {
    PancakeV3Adapter internal adapter;
    MockPancakeV3Router internal mockRouter;
    MockERC20Adapter internal tokenIn;
    MockERC20Adapter internal tokenOut;

    address internal caller;

    function setUp() public {
        caller = makeAddr("caller");
        tokenIn = new MockERC20Adapter();
        tokenOut = new MockERC20Adapter();
        mockRouter = new MockPancakeV3Router(address(tokenOut));
        adapter = new PancakeV3Adapter(address(mockRouter));
    }

    /// @notice Constructor rejects zero router.
    function testConstructor_RejectsZeroRouter() public {
        vm.expectRevert("PancakeV3Adapter: zero router");
        new PancakeV3Adapter(address(0));
    }

    /// @notice swap executes exactInputSingle via router, tokenOut arrives at caller.
    function testSwap_ExecutesExactInputSingle() public {
        uint256 amountIn = 1000e18;
        tokenIn.mint(caller, amountIn);

        vm.startPrank(caller);
        tokenIn.approve(address(adapter), amountIn);

        // fee=500 (0.05%), no price limit
        bytes memory extraData = abi.encode(uint24(500), uint160(0));
        uint256 amountOut = adapter.swap(address(tokenIn), address(tokenOut), amountIn, 0, extraData);
        vm.stopPrank();

        // MockRouter returns amountIn * 990 / 1000 = 990e18
        assertEq(amountOut, 990e18, "amountOut mismatch");
        // tokenOut arrives at caller (router mints directly to recipient = msg.sender)
        assertEq(tokenOut.balanceOf(caller), 990e18, "caller must hold tokenOut");
    }

    /// @notice swap reverts when router output is below minAmountOut.
    function testSwap_RevertsOnSlippage() public {
        uint256 amountIn = 1000e18;
        tokenIn.mint(caller, amountIn);

        vm.startPrank(caller);
        tokenIn.approve(address(adapter), amountIn);

        bytes memory extraData = abi.encode(uint24(500), uint160(0));
        // minAmountOut > 990e18 - should trigger slippage revert in mock router
        vm.expectRevert("MockPancakeRouter: slippage");
        adapter.swap(address(tokenIn), address(tokenOut), amountIn, 995e18, extraData);
        vm.stopPrank();
    }

    /// @notice quoteOut reverts with informative message (Quoter not yet integrated).
    function testQuoteOut_RevertsWithMessage() public {
        bytes memory extraData = abi.encode(uint24(500), uint160(0));
        vm.expectRevert("PancakeV3Adapter: use Quoter contract for off-chain quotes");
        adapter.quoteOut(address(tokenIn), address(tokenOut), 1000e18, extraData);
    }

    /// @notice router address is stored correctly.
    function testConstructor_StoresRouter() public view {
        assertEq(address(adapter.router()), address(mockRouter));
    }
}

// =============================================================================
// SC-2: GMXAdapter skeleton tests (vm.skip)
// =============================================================================

contract GMXAdapterTest is Test {
    /// @notice SC-2 follow-up: GMX V1/V2 decision needed before implementation.
    ///         V1 is atomic but may have declining liquidity. V2 is non-atomic.
    function testTODO_GMXAdapter_NotImplemented() public {
        vm.skip(true);
        // When implemented:
        // - Decide V1 vs V2 based on liquidity data
        // - V1: IGMXRouter.swap(path, amountIn, minOut, receiver)
        // - V2: non-atomic, incompatible with flash loan arbitrage
    }
}

// =============================================================================
// SC-2: SynthetixAdapter skeleton tests (vm.skip)
// =============================================================================

contract SynthetixAdapterTest is Test {
    /// @notice SC-2 follow-up: Synthetix V2 has settlement delay - incompatible with
    ///         flash loan atomicity. Evaluate V3 instant settlement viability.
    function testTODO_SynthetixAdapter_NotImplemented() public {
        vm.skip(true);
        // Evaluation criteria:
        // - V2: ~3 min settlement delay -> NOT atomic -> cannot use in flash arb
        // - V3: assess if instant settlement is available for the relevant synths
        // - If V3 atomic: implement ISynthetix.exchange(srcKey, amount, destKey)
        // - If not: remove from adapter roadmap permanently
    }
}
