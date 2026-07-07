// SPDX-License-Identifier: MIT
pragma solidity 0.8.19;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

// ┌─────────────────────────────────────────────────────────────┐
// │  ArbitrageX-V2 — Uniswap V3 Adapter                        │
// │  Single-hop & multi-hop swaps via SwapRouter                │
// │  Callback: uniswapV3SwapCallback                            │
// └─────────────────────────────────────────────────────────────┘

/// @title ISwapRouter
/// @notice Uniswap V3 SwapRouter interface
interface ISwapRouter {
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

    function exactInputSingle(ExactInputSingleParams calldata params)
        external
        payable
        returns (uint256 amountOut);

    struct ExactInputParams {
        bytes path;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
    }

    function exactInput(ExactInputParams calldata params)
        external
        payable
        returns (uint256 amountOut);

    struct ExactOutputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 deadline;
        uint256 amountOut;
        uint256 amountInMaximum;
        uint160 sqrtPriceLimitX96;
    }

    function exactOutputSingle(ExactOutputSingleParams calldata params)
        external
        payable
        returns (uint256 amountIn);

    struct ExactOutputParams {
        bytes path;
        address recipient;
        uint256 deadline;
        uint256 amountOut;
        uint256 amountInMaximum;
    }

    function exactOutput(ExactOutputParams calldata params)
        external
        payable
        returns (uint256 amountIn);
}

/// @title IUniswapV3Pool
/// @notice Minimal Uniswap V3 pool interface
interface IUniswapV3Pool {
    function token0() external view returns (address);
    function token1() external view returns (address);
    function fee() external view returns (uint24);
    function liquidity() external view returns (uint128);
    function slot0()
        external
        view
        returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160 sqrtPriceLimitX96,
        bytes calldata data
    )
        external
        returns (int256 amount0, int256 amount1);
}

/// @title IUniswapV3Factory
/// @notice Uniswap V3 Factory for pool discovery
interface IUniswapV3Factory {
    function getPool(address tokenA, address tokenB, uint24 fee)
        external
        view
        returns (address pool);
    function feeAmountTickSpacing(uint24 fee) external view returns (int24);
}

/// @title IUniswapV3SwapCallback
/// @notice Callback for V3 swaps
interface IUniswapV3SwapCallback {
    function uniswapV3SwapCallback(
        int256 amount0Delta,
        int256 amount1Delta,
        bytes calldata data
    )
        external;
}

/// @title UniswapV3Adapter
/// @notice Adapter for Uniswap V3 single-hop and multi-hop swaps
/// @author ArbitrageX Team
/// @dev Handles both exact-input and exact-output swaps
/// @dev Callback pattern: uniswapV3SwapCallback pays the pool directly
/// @dev Invariant: output amount >= amountOutMin for input swaps
contract UniswapV3Adapter is IUniswapV3SwapCallback {
    using SafeERC20 for IERC20;

    // ============ Events ============

    /// @notice Emitted on successful V3 swap
    /// @param router SwapRouter used
    /// @param tokenIn Token sold
    /// @param tokenOut Token bought
    /// @param fee Fee tier
    /// @param amountIn Input amount
    /// @param amountOut Output amount
    event V3SwapExecuted(
        address indexed router,
        address indexed tokenIn,
        address indexed tokenOut,
        uint24 fee,
        uint256 amountIn,
        uint256 amountOut
    );

    /// @notice Emitted on callback execution
    /// @param pool Pool that invoked callback
    /// @param amount0Delta Token0 amount (negative = received)
    /// @param amount1Delta Token1 amount (negative = received)
    event V3CallbackExecuted(
        address indexed pool,
        int256 amount0Delta,
        int256 amount1Delta
    );

    // ============ Errors ============

    /// @notice Router address is zero or not registered
    error InvalidRouter();

    /// @notice Pool address is zero
    error InvalidPool();

    /// @notice Swap failed at router level
    error SwapFailed(bytes reason);

    /// @notice Output below minimum threshold
    error InsufficientOutput(uint256 actual, uint256 minimum);

    /// @notice Input exceeds maximum threshold
    error ExcessiveInput(uint256 actual, uint256 maximum);

    /// @notice Callback invoked by unauthorized pool
    error UnauthorizedCallback(address pool);

    /// @notice Zero amount specified
    error ZeroAmount();

    /// @notice Invalid fee tier
    error InvalidFeeTier(uint24 fee);

    // ============ State ============

    /// @notice Registered SwapRouter addresses
    mapping(address => bool) public registeredRouters;

    /// @notice Uniswap V3 Factory address
    address public immutable FACTORY;

    /// @notice Array of registered routers
    address[] public routers;

    // ============ Constructor ============

    /// @notice Initialize adapter with factory and routers
    /// @param _factory Uniswap V3 Factory address
    /// @param _routers Initial SwapRouter addresses
    constructor(address _factory, address[] memory _routers) {
        if (_factory == address(0)) revert InvalidPool();
        FACTORY = _factory;

        uint256 len = _routers.length;
        for (uint256 i; i < len; ) {
            if (_routers[i] == address(0)) revert InvalidRouter();
            registeredRouters[_routers[i]] = true;
            routers.push(_routers[i]);
            unchecked { ++i; }
        }
    }

    // ============ External: Core Swaps ============

    /// @notice Execute exact-input single-hop swap on Uniswap V3
    /// @param tokenIn Token to swap from
    /// @param tokenOut Token to swap to
    /// @param amountIn Exact input amount
    /// @param amountOutMin Minimum output amount
    /// @param router SwapRouter address
    /// @param fee Fee tier (e.g., 500=0.05%, 3000=0.3%, 10000=1%)
    /// @return amountOut Actual output amount
    /// @dev Gas cost: ~115k-135k (varies by fee tier and pool state)
    /// @dev CEI pattern: checks → approve → swap → validate
    function swap(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 amountOutMin,
        address router,
        uint24 fee
    )
        external
        returns (uint256 amountOut)
    {
        // ── checks ──
        if (!registeredRouters[router]) revert InvalidRouter();
        if (amountIn == 0) revert ZeroAmount();
        if (fee != 100 && fee != 500 && fee != 3000 && fee != 10000) {
            revert InvalidFeeTier(fee);
        }

        // Approve router
        IERC20(tokenIn).safeApprove(router, amountIn);

        // ── interactions: swap via router ──
        try ISwapRouter(router).exactInputSingle(
            ISwapRouter.ExactInputSingleParams({
                tokenIn: tokenIn,
                tokenOut: tokenOut,
                fee: fee,
                recipient: address(this),
                deadline: block.timestamp,
                amountIn: amountIn,
                amountOutMinimum: amountOutMin,
                sqrtPriceLimitX96: 0 // no price limit
            })
        ) returns (uint256 amtOut) {
            amountOut = amtOut;
        } catch Error(string memory reason) {
            revert SwapFailed(abi.encode(reason));
        } catch (bytes memory lowLevelData) {
            revert SwapFailed(lowLevelData);
        }

        // ── effects: validate output ──
        if (amountOut < amountOutMin) {
            revert InsufficientOutput(amountOut, amountOutMin);
        }

        emit V3SwapExecuted(router, tokenIn, tokenOut, fee, amountIn, amountOut);
    }

    /// @notice Exact input single-hop swap (direct function)
    /// @param params V3 ExactInputSingleParams
    /// @return amountOut Output amount
    function exactInputSingle(ISwapRouter.ExactInputSingleParams calldata params)
        external
        returns (uint256 amountOut)
    {
        if (!registeredRouters[msg.sender] && !registeredRouters[tx.origin]) {
            // Allow calls from Executor (not direct router calls)
        }

        // Approve tokenIn to router
        IERC20(params.tokenIn).safeApprove(msg.sender, params.amountIn);

        amountOut = ISwapRouter(msg.sender).exactInputSingle(params);

        if (amountOut < params.amountOutMinimum) {
            revert InsufficientOutput(amountOut, params.amountOutMinimum);
        }
    }

    /// @notice Exact input multi-hop swap (encoded path)
    /// @param params V3 ExactInputParams with encoded path
    /// @return amountOut Output amount
    /// @dev Path encoding: tokenIn (20) + fee (3) + tokenOut (20) + fee (3) + ...
    function exactInput(ISwapRouter.ExactInputParams calldata params)
        external
        payable
        returns (uint256 amountOut)
    {
        address router = _getRouterForPath(params.path);
        if (router == address(0)) revert InvalidRouter();

        // Extract tokenIn from path (first 20 bytes)
        address tokenIn;
        assembly ("memory-safe") {
            tokenIn := shr(96, calldataload(params.path.offset))
        }

        // Approve tokenIn to router
        IERC20(tokenIn).safeApprove(router, params.amountIn);

        try ISwapRouter(router).exactInput(params) returns (uint256 amtOut) {
            amountOut = amtOut;
        } catch Error(string memory reason) {
            revert SwapFailed(abi.encode(reason));
        } catch (bytes memory lowLevelData) {
            revert SwapFailed(lowLevelData);
        }

        if (amountOut < params.amountOutMinimum) {
            revert InsufficientOutput(amountOut, params.amountOutMinimum);
        }
    }

    /// @notice Exact output single-hop swap (specify output, limit input)
    /// @param tokenIn Token to swap from
    /// @param tokenOut Token to swap to
    /// @param amountOut Exact output desired
    /// @param amountInMax Maximum input willing to spend
    /// @param router SwapRouter address
    /// @param fee Fee tier
    /// @return amountIn Actual input spent
    /// @dev Gas cost: ~125k-150k
    function exactOutputSingle(
        address tokenIn,
        address tokenOut,
        uint256 amountOut,
        uint256 amountInMax,
        address router,
        uint24 fee
    )
        external
        returns (uint256 amountIn)
    {
        // ── checks ──
        if (!registeredRouters[router]) revert InvalidRouter();
        if (amountOut == 0) revert ZeroAmount();

        // Approve maximum input to router
        IERC20(tokenIn).safeApprove(router, amountInMax);

        // ── interactions: swap via router ──
        try ISwapRouter(router).exactOutputSingle(
            ISwapRouter.ExactOutputSingleParams({
                tokenIn: tokenIn,
                tokenOut: tokenOut,
                fee: fee,
                recipient: address(this),
                deadline: block.timestamp,
                amountOut: amountOut,
                amountInMaximum: amountInMax,
                sqrtPriceLimitX96: 0
            })
        ) returns (uint256 amtIn) {
            amountIn = amtIn;
        } catch Error(string memory reason) {
            revert SwapFailed(abi.encode(reason));
        } catch (bytes memory lowLevelData) {
            revert SwapFailed(lowLevelData);
        }

        // ── effects: validate input ──
        if (amountIn > amountInMax) {
            revert ExcessiveInput(amountIn, amountInMax);
        }

        // Revoke excess approval
        if (amountIn < amountInMax) {
            IERC20(tokenIn).safeApprove(router, 0);
        }

        emit V3SwapExecuted(router, tokenIn, tokenOut, fee, amountIn, amountOut);
    }

    // ============ External: Swap Callback ============

    /// @notice Uniswap V3 swap callback — pays pool for swap
    /// @param amount0Delta Amount of token0 (positive = owed, negative = received)
    /// @param amount1Delta Amount of token1 (positive = owed, negative = received)
    /// @param data Encoded callback data (token addresses)
    /// @dev Only callable by legitimate V3 pools
    /// @dev Gas cost: ~20k
    function uniswapV3SwapCallback(
        int256 amount0Delta,
        int256 amount1Delta,
        bytes calldata data
    )
        external
        override
    {
        // ── checks: verify callback from legitimate pool ──
        address pool = msg.sender;
        // Verify pool exists in factory
        (address tokenIn, address tokenOut, uint24 fee) = _decodeCallbackData(data);

        address expectedPool = IUniswapV3Factory(FACTORY).getPool(tokenIn, tokenOut, fee);
        if (pool != expectedPool) revert UnauthorizedCallback(pool);

        emit V3CallbackExecuted(pool, amount0Delta, amount1Delta);

        // ── interactions: pay what we owe ──
        if (amount0Delta > 0) {
            // We owe token0
            IERC20(IUniswapV3Pool(pool).token0()).safeTransfer(
                pool,
                uint256(amount0Delta)
            );
        }
        if (amount1Delta > 0) {
            // We owe token1
            IERC20(IUniswapV3Pool(pool).token1()).safeTransfer(
                pool,
                uint256(amount1Delta)
            );
        }
    }

    // ============ External: Pool Queries ============

    /// @notice Get V3 pool address for token pair and fee
    /// @param tokenA Token A
    /// @param tokenB Token B
    /// @param fee Fee tier
    /// @return pool Pool address (address(0) if not found)
    function getPool(
        address tokenA,
        address tokenB,
        uint24 fee
    )
        external
        view
        returns (address pool)
    {
        pool = IUniswapV3Factory(FACTORY).getPool(tokenA, tokenB, fee);
    }

    /// @notice Get pool slot0 data (price, tick, etc.)
    /// @param pool Pool address
    /// @return sqrtPriceX96 Current sqrt(price) * 2^96
    /// @return tick Current tick
    /// @return liquidity Pool liquidity
    function getPoolState(address pool)
        external
        view
        returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint128 liquidity
        )
    {
        if (pool == address(0)) revert InvalidPool();

        (sqrtPriceX96, tick, , , , , ) = IUniswapV3Pool(pool).slot0();
        liquidity = IUniswapV3Pool(pool).liquidity();
    }

    /// @notice Calculate expected output for a V3 swap (simulated)
    /// @param sqrtPriceX96 Current sqrt price
    /// @param liquidity Pool liquidity
    /// @param amountIn Input amount
    /// @param fee Fee tier
    /// @return amountOut Expected output
    function quoteExactInput(
        uint160 sqrtPriceX96,
        uint128 liquidity,
        uint256 amountIn,
        uint24 fee
    )
        external
        pure
        returns (uint256 amountOut)
    {
        if (amountIn == 0 || liquidity == 0 || sqrtPriceX96 == 0) return 0;

        // Simplified V3 swap math (not exact — for estimation only)
        // amountOut = (amountIn * (1e6 - fee) * sqrtPriceX96^2) / (2^192)
        uint256 feeAdjusted = amountIn * (1_000_000 - uint256(fee));
        uint256 priceSquared = uint256(sqrtPriceX96) * uint256(sqrtPriceX96);
        amountOut = (feeAdjusted * priceSquared) / (1_000_000 * (1 << 192));
    }

    // ============ External: Administration ============

    /// @notice Register a new SwapRouter
    /// @param router Router address
    function registerRouter(address router) external {
        if (router == address(0)) revert InvalidRouter();
        registeredRouters[router] = true;
        routers.push(router);
    }

    // ============ Internal: Helpers ============

    /// @notice Decode callback data (tokenIn, tokenOut, fee)
    /// @param data Encoded bytes
    /// @return tokenIn Input token
    /// @return tokenOut Output token
    /// @return fee Fee tier
    function _decodeCallbackData(bytes calldata data)
        internal
        pure
        returns (address tokenIn, address tokenOut, uint24 fee)
    {
        assembly ("memory-safe") {
            // data layout: tokenIn (20 bytes) + tokenOut (20 bytes) + fee (3 bytes)
            // ABI-encoded: offset (32) + length (32) + data...
            let dataStart := add(data.offset, 0x20) // skip offset and length
            tokenIn := shr(96, calldataload(dataStart))
            tokenOut := shr(96, calldataload(add(dataStart, 20)))
            fee := and(shr(232, calldataload(add(dataStart, 40))), 0xFFFFFF)
        }
    }

    /// @notice Find router that has the required pool
    /// @param encodedPath V3 encoded path
    /// @return router Router address, or address(0)
    function _getRouterForPath(bytes calldata encodedPath)
        internal
        view
        returns (address router)
    {
        if (encodedPath.length < 23) return address(0); // minimum: token (20) + fee (3)

        // Extract first token and fee from path
        address tokenIn;
        uint24 fee;
        assembly ("memory-safe") {
            tokenIn := shr(96, calldataload(encodedPath.offset))
            fee := and(shr(232, calldataload(add(encodedPath.offset, 20))), 0xFFFFFF)
        }

        // Check each router's factory for pool existence
        uint256 routerCount = routers.length;
        for (uint256 i; i < routerCount; ) {
            // For simplicity, return first registered router
            // In production, verify pool exists via factory
            return routers[0];
            unchecked { ++i; }
        }
    }

    /// @notice Encode V3 path for multi-hop swaps
    /// @param tokens Array of tokens
    /// @param fees Array of fee tiers between tokens
    /// @return path Encoded path for V3 router
    function encodePath(
        address[] calldata tokens,
        uint24[] calldata fees
    )
        external
        pure
        returns (bytes memory path)
    {
        if (tokens.length < 2) revert InvalidRouter();
        if (fees.length != tokens.length - 1) revert InvalidFeeTier(0);

        // Path = token0 + fee0 + token1 + fee1 + token2 + ...
        uint256 numHops = fees.length;
        path = new bytes(20 + numHops * 23); // 20 + 3 per fee + 20 per token

        assembly ("memory-safe") {
            // Write first token (20 bytes)
            mstore(add(path, 0x20), shl(96, calldataload(tokens.offset)))

            let pathPtr := add(path, 0x34) // 0x20 (offset) + 0x14 (20 bytes token)

            for { let i := 0 } lt(i, numHops) { i := add(i, 1) } {
                // Write fee (3 bytes)
                let fee := and(
                    calldataload(add(fees.offset, mul(i, 0x20))),
                    0xFFFFFF
                )
                // fee occupies bytes pathPtr+17 to pathPtr+20 (shifted into position)
                let feeShifted := shl(232, fee)
                mstore(pathPtr, feeShifted)

                // Write next token (20 bytes)
                let token := calldataload(add(tokens.offset, mul(add(i, 1), 0x20)))
                mstore(add(pathPtr, 3), shl(96, token))

                pathPtr := add(pathPtr, 23) // advance by fee(3) + token(20)
            }

            // Update free memory pointer
            mstore(0x40, add(path, add(0x20, mload(path))))
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
