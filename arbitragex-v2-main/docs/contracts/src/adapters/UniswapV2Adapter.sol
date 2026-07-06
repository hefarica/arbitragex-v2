// SPDX-License-Identifier: MIT
pragma solidity 0.8.19;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

// ┌─────────────────────────────────────────────────────────────┐
// │  ArbitrageX-V2 — Uniswap V2 / SushiSwap Adapter            │
// │  Gas-optimized swaps with inline assembly                    │
// └─────────────────────────────────────────────────────────────┘

/// @title IUniswapV2Router02
/// @notice Minimal interface for Uniswap V2 Router interactions
interface IUniswapV2Router02 {
    function factory() external pure returns (address);
    function WETH() external pure returns (address);

    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    )
        external
        returns (uint256[] memory amounts);

    function swapTokensForExactTokens(
        uint256 amountOut,
        uint256 amountInMax,
        address[] calldata path,
        address to,
        uint256 deadline
    )
        external
        returns (uint256[] memory amounts);

    function getAmountsOut(
        uint256 amountIn,
        address[] calldata path
    )
        external
        view
        returns (uint256[] memory amounts);

    function getAmountsIn(
        uint256 amountOut,
        address[] calldata path
    )
        external
        view
        returns (uint256[] memory amounts);

    function addLiquidity(
        address tokenA,
        address tokenB,
        uint256 amountADesired,
        uint256 amountBDesired,
        uint256 amountAMin,
        uint256 amountBMin,
        address to,
        uint256 deadline
    )
        external
        returns (uint256 amountA, uint256 amountB, uint256 liquidity);
}

/// @title IUniswapV2Factory
/// @notice Minimal interface for V2 factory (pair lookup)
interface IUniswapV2Factory {
    function getPair(address tokenA, address tokenB)
        external
        view
        returns (address pair);
    function allPairs(uint256) external view returns (address pair);
    function allPairsLength() external view returns (uint256);
}

/// @title IUniswapV2Pair
/// @notice Minimal interface for V2 pair (reserves + swap)
interface IUniswapV2Pair {
    function token0() external view returns (address);
    function token1() external view returns (address);
    function getReserves()
        external
        view
        returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    function swap(
        uint256 amount0Out,
        uint256 amount1Out,
        address to,
        bytes calldata data
    )
        external;
    function skim(address to) external;
    function sync() external;
}

/// @title UniswapV2Adapter
/// @notice Adapter for Uniswap V2 and forks (SushiSwap, PancakeSwap V2, etc.)
/// @author ArbitrageX Team
/// @dev All router/pair addresses passed via constructor — zero hardcoding
/// @dev Invariant: output amount >= amountOutMin or transaction reverts
/// @dev Uses inline assembly for gas-efficient reserve queries and path building
contract UniswapV2Adapter {
    using SafeERC20 for IERC20;

    // ============ Events ============

    /// @notice Emitted on successful V2 swap
    /// @param router Router used
    /// @param tokenIn Token sold
    /// @param tokenOut Token bought
    /// @param amountIn Input amount
    /// @param amountOut Output amount
    event V2SwapExecuted(
        address indexed router,
        address indexed tokenIn,
        address indexed tokenOut,
        uint256 amountIn,
        uint256 amountOut
    );

    /// @notice Emitted on reserves query
    /// @param pair Pair address
    /// @param reserve0 Reserve of token0
    /// @param reserve1 Reserve of token1
    event V2ReservesQueried(
        address indexed pair,
        uint256 reserve0,
        uint256 reserve1
    );

    // ============ Errors ============

    /// @notice Router address is zero
    error InvalidRouter();

    /// @notice Swap failed at router level
    error SwapFailed(bytes reason);

    /// @notice Output below minimum threshold
    error InsufficientOutput(uint256 actual, uint256 minimum);

    /// @notice Pair does not exist for token pair
    error PairNotFound(address tokenA, address tokenB);

    /// @notice Invalid path length
    error InvalidPathLength(uint256 length);

    // ============ State ============

    /// @notice Mapping: router address => is registered
    mapping(address => bool) public registeredRouters;

    /// @notice Array of registered routers for iteration
    address[] public routers;

    // ============ Constructor ============

    /// @notice Register initial V2 routers
    /// @param _routers Array of router addresses to register
    constructor(address[] memory _routers) {
        uint256 len = _routers.length;
        for (uint256 i; i < len; ) {
            if (_routers[i] == address(0)) revert InvalidRouter();
            registeredRouters[_routers[i]] = true;
            routers.push(_routers[i]);
            unchecked { ++i; }
        }
    }

    // ============ External: Core Swap ============

    /// @notice Execute exact-input swap on Uniswap V2
    /// @param tokenIn Token to swap from
    /// @param tokenOut Token to swap to
    /// @param amountIn Exact amount of tokenIn
    /// @param amountOutMin Minimum amount of tokenOut
    /// @param router Uniswap V2 router address
    /// @param feeOrPoolFee Unused for V2 (fee is always 0.3%)
    /// @return amountOut Actual amount of tokenOut received
    /// @dev Gas cost: ~95k-110k per swap (depends on hop count)
    /// @dev CEI pattern: checks → interactions → output validation
    function swap(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 amountOutMin,
        address router,
        uint24 feeOrPoolFee
    )
        external
        returns (uint256 amountOut)
    {
        // ── checks ──
        if (!registeredRouters[router]) revert InvalidRouter();
        if (tokenIn == address(0) || tokenOut == address(0)) revert InvalidRouter();

        // Approve router
        IERC20(tokenIn).safeApprove(router, amountIn);

        // Build path via Yul for gas efficiency
        address[] memory path = _buildPathTwoHop(tokenIn, tokenOut);

        // ── interactions: swap via router ──
        uint256[] memory amounts;
        try IUniswapV2Router02(router).swapExactTokensForTokens(
            amountIn,
            amountOutMin,
            path,
            address(this),
            block.timestamp
        ) returns (uint256[] memory amts) {
            amounts = amts;
        } catch Error(string memory reason) {
            revert SwapFailed(abi.encode(reason));
        } catch (bytes memory lowLevelData) {
            revert SwapFailed(lowLevelData);
        }

        // ── effects: validate output ──
        amountOut = amounts[amounts.length - 1];
        if (amountOut < amountOutMin) {
            revert InsufficientOutput(amountOut, amountOutMin);
        }

        emit V2SwapExecuted(router, tokenIn, tokenOut, amountIn, amountOut);
    }

    /// @notice Swap exact tokens for tokens with custom path
    /// @param amountIn Exact input amount
    /// @param amountOutMin Minimum output amount
    /// @param path Token path for multi-hop swaps
    /// @param to Recipient address
    /// @param deadline Transaction deadline
    /// @return amounts Array of input/output amounts per hop
    /// @dev Gas cost: ~95k + 15k per additional hop
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    )
        external
        returns (uint256[] memory amounts)
    {
        address router = _findRouterForPath(path);
        if (router == address(0)) revert InvalidRouter();
        if (path.length < 2) revert InvalidPathLength(path.length);

        // Approve first token to router
        IERC20(path[0]).safeApprove(router, amountIn);

        try IUniswapV2Router02(router).swapExactTokensForTokens(
            amountIn,
            amountOutMin,
            path,
            to,
            deadline
        ) returns (uint256[] memory amts) {
            amounts = amts;
        } catch Error(string memory reason) {
            revert SwapFailed(abi.encode(reason));
        } catch (bytes memory lowLevelData) {
            revert SwapFailed(lowLevelData);
        }

        // Validate output
        uint256 amountOut = amounts[amounts.length - 1];
        if (amountOut < amountOutMin) {
            revert InsufficientOutput(amountOut, amountOutMin);
        }

        emit V2SwapExecuted(router, path[0], path[path.length - 1], amountIn, amountOut);
    }

    // ============ External: Quoting ============

    /// @notice Get expected output amounts for a swap path
    /// @param amountIn Input amount
    /// @param path Token path
    /// @param router Router to query
    /// @return amounts Expected output at each hop
    /// @dev Gas cost: ~8k per hop (static calls to pairs)
    function getAmountsOut(
        uint256 amountIn,
        address[] calldata path,
        address router
    )
        external
        view
        returns (uint256[] memory amounts)
    {
        if (!registeredRouters[router]) revert InvalidRouter();
        if (path.length < 2) revert InvalidPathLength(path.length);

        amounts = IUniswapV2Router02(router).getAmountsOut(amountIn, path);
    }

    /// @notice Get required input amounts for desired output
    /// @param amountOut Desired output amount
    /// @param path Token path
    /// @param router Router to query
    /// @return amounts Required input at each hop (reverse order)
    /// @dev Gas cost: ~8k per hop
    function getAmountsIn(
        uint256 amountOut,
        address[] calldata path,
        address router
    )
        external
        view
        returns (uint256[] memory amounts)
    {
        if (!registeredRouters[router]) revert InvalidRouter();
        if (path.length < 2) revert InvalidPathLength(path.length);

        amounts = IUniswapV2Router02(router).getAmountsIn(amountOut, path);
    }

    /// @notice Direct pair reserve query (no router call)
    /// @param factory Factory address
    /// @param tokenA First token
    /// @param tokenB Second token
    /// @return reserveA Reserves of tokenA
    /// @return reserveB Reserves of tokenB
    /// @return pair Address of the pair
    /// @dev Gas cost: ~5k (single SLOAD per reserve)
    function getReserves(
        address factory,
        address tokenA,
        address tokenB
    )
        external
        view
        returns (uint256 reserveA, uint256 reserveB, address pair)
    {
        // ── Get pair address via Yul for gas ──
        pair = _getPair(factory, tokenA, tokenB);
        if (pair == address(0)) revert PairNotFound(tokenA, tokenB);

        // ── Read reserves with inline assembly ──
        uint112 reserve0;
        uint112 reserve1;
        address token0;

        assembly ("memory-safe") {
            // Call token0()
            mstore(0x00, 0x0dfe1681) // selector for token0()
            let success := staticcall(gas(), pair, 28, 4, 0x00, 0x20)
            if iszero(success) {
                revert(0, 0)
            }
            token0 := mload(0x00)

            // Call getReserves()
            mstore(0x00, 0x0902f1ac) // selector for getReserves()
            success := staticcall(gas(), pair, 28, 4, 0x00, 0x60)
            if iszero(success) {
                revert(0, 0)
            }
            reserve0 := mload(0x00)
            reserve1 := mload(0x20)
        }

        // ── Order reserves to match tokenA/tokenB ──
        if (tokenA == token0) {
            reserveA = reserve0;
            reserveB = reserve1;
        } else {
            reserveA = reserve1;
            reserveB = reserve0;
        }
    }

    // ============ External: Pair Discovery ============

    /// @notice Check if a pair exists for a token pair
    /// @param factory Factory address
    /// @param tokenA Token A
    /// @param tokenB Token B
    /// @return exists True if pair exists
    /// @return pair Pair address (address(0) if not found)
    function pairExists(
        address factory,
        address tokenA,
        address tokenB
    )
        external
        view
        returns (bool exists, address pair)
    {
        pair = _getPair(factory, tokenA, tokenB);
        exists = pair != address(0);
    }

    /// @notice Calculate expected output using constant-product formula
    /// @param amountIn Input amount
    /// @param reserveIn Input token reserve
    /// @param reserveOut Output token reserve
    /// @return amountOut Expected output (with 0.3% fee)
    /// @dev Formula: amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
    function quoteV2(
        uint256 amountIn,
        uint256 reserveIn,
        uint256 reserveOut
    )
        external
        pure
        returns (uint256 amountOut)
    {
        if (amountIn == 0) return 0;
        if (reserveIn == 0 || reserveOut == 0) return 0;

        uint256 amountInWithFee = amountIn * 997;
        uint256 numerator = amountInWithFee * reserveOut;
        uint256 denominator = (reserveIn * 1000) + amountInWithFee;

        amountOut = numerator / denominator;
    }

    // ============ External: Administration ============

    /// @notice Register a new router
    /// @param router Router address
    function registerRouter(address router) external {
        if (router == address(0)) revert InvalidRouter();
        registeredRouters[router] = true;
        routers.push(router);
    }

    // ============ Internal: Pair Lookup ============

    /// @notice Get pair address from factory using Yul
    /// @param factory Factory address
    /// @param tokenA Token A
    /// @param tokenB Token B
    /// @return pair Pair address
    function _getPair(
        address factory,
        address tokenA,
        address tokenB
    )
        internal
        view
        returns (address pair)
    {
        assembly ("memory-safe") {
            // Encode getPair(address,address) call
            let ptr := mload(0x40)
            mstore(ptr, 0xe6a4390500000000000000000000000000000000000000000000000000000000)
            mstore(add(ptr, 4), and(tokenA, 0xffffffffffffffffffffffffffffffffffffffff))
            mstore(add(ptr, 36), and(tokenB, 0xffffffffffffffffffffffffffffffffffffffff))

            let success := staticcall(gas(), factory, ptr, 68, 0x00, 0x20)
            if iszero(success) {
                pair := 0
            }
            pair := mload(0x00)
        }
    }

    /// @notice Build a 2-hop path via Yul (gas efficient)
    /// @param tokenA Start token
    /// @param tokenB End token
    /// @return path [tokenA, tokenB]
    function _buildPathTwoHop(address tokenA, address tokenB)
        internal
        pure
        returns (address[] memory path)
    {
        assembly ("memory-safe") {
            path := mload(0x40)
            mstore(path, 2)               // length = 2
            mstore(add(path, 0x20), tokenA)
            mstore(add(path, 0x40), tokenB)
            mstore(0x40, add(path, 0x60)) // update free memory pointer
        }
    }

    /// @notice Find a router that has the required pair
    /// @param path Token path to check
    /// @return router Router address with pair, or address(0)
    function _findRouterForPath(address[] calldata path)
        internal
        view
        returns (address router)
    {
        if (path.length < 2) return address(0);

        uint256 routerCount = routers.length;
        for (uint256 i; i < routerCount; ) {
            address r = routers[i];
            address factory = IUniswapV2Router02(r).factory();
            address pair = _getPair(factory, path[0], path[path.length - 1]);
            if (pair != address(0)) {
                return r;
            }
            unchecked { ++i; }
        }
    }

    // ============ View Functions ============

    /// @notice Get all registered routers
    function getRouters() external view returns (address[] memory) {
        return routers;
    }

    /// @notice Count of registered routers
    function routerCount() external view returns (uint256) {
        return routers.length;
    }
}
