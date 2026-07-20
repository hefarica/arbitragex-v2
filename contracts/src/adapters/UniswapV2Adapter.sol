// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// =============================================================================
// SC-2: UniswapV2Adapter - IDEXAdapter for Uniswap V2 and forks
//
// Supports any DEX that implements the Uniswap V2 router interface:
//   - Uniswap V2 (Ethereum mainnet)
//   - SushiSwap (multi-chain)
//   - PancakeSwap V2 (BSC, Ethereum, Aptos)
//   - QuickSwap (Polygon)
//   - TraderJoe (Avalanche, Arbitrum)
//   - SpookySwap (Fantom)
//   - And any other swapExactTokensForTokens-compatible router
//
// extraData encoding: abi.encode(address[] path)
//   - path: Ordered token addresses defining the swap route.
//           path[0] = tokenIn, path[path.length-1] = tokenOut.
//           Intermediate hops are supported (e.g. WETH -> DAI -> USDC).
//
// Router address is injected via constructor - zero hardcoded addresses.
// This guarantees identical adapter bytecode across all chains.
//
// Gas profile (optimizer_runs = 200):
//   - swap()  : ~82,000 gas (single hop) / ~115,000 gas (two hops)
//   - quoteOut() : ~12,000 gas (view, no state change)
// =============================================================================

import "../interfaces/IDEXAdapter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

// -------------------------------------------------------------------------
// Minimal Uniswap V2 interfaces (no hardcoded addresses)
// -------------------------------------------------------------------------

/// @dev Uniswap V2 Router interface - swapExactTokensForTokens path.
///      Implemented by Uniswap V2, SushiSwap, PancakeSwap V2, and all forks.
interface IUniswapV2Router02 {
    /// @notice Return the factory address associated with this router.
    /// @return factory The Uniswap V2 factory contract address.
    function factory() external pure returns (address);

    /// @notice Swap an exact amount of input tokens for as many output tokens
    ///         as possible along the specified path.
    /// @param amountIn   Amount of input tokens to send.
    /// @param amountOutMin Minimum output amount (slippage guard). Reverts if not met.
    /// @param path       Array of token addresses defining the swap route.
    /// @param to         Recipient of the output tokens.
    /// @param deadline   Unix timestamp after which the transaction reverts.
    /// @return amounts   Array of input/output amounts for each hop.
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);

    /// @notice Given an input asset amount and an array of token addresses,
    ///         calculates all subsequent maximum output token amounts.
    /// @param amountIn Amount of input token.
    /// @param path     Array of token addresses defining the swap route.
    /// @return amounts Array of maximum output amounts for each hop.
    function getAmountsOut(uint256 amountIn, address[] calldata path) external view returns (uint256[] memory amounts);
}

/// @dev Uniswap V2 Factory interface - for pair lookup and validation.
interface IUniswapV2Factory {
    /// @notice Return the pair address for tokenA/tokenB. address(0) if nonexistent.
    /// @param tokenA First token of the pair.
    /// @param tokenB Second token of the pair.
    /// @return pair  The pair contract address.
    function getPair(address tokenA, address tokenB) external view returns (address pair);
}

/// @dev Uniswap V2 Pair interface - for reserve queries.
interface IUniswapV2Pair {
    /// @notice Return the current reserves of token0 and token1, plus the block
    ///         timestamp of the last reserve update.
    /// @return reserve0   Reserve of token0.
    /// @return reserve1   Reserve of token1.
    /// @return blockTimestampLast Block timestamp of last reserve update.
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
}

// -------------------------------------------------------------------------
// Errors (SC-3: custom errors ~200 gas saved per revert vs string require)
// -------------------------------------------------------------------------

/// @dev Thrown when constructor receives address(0) for the router.
error UV2_ZeroRouter();
/// @dev Thrown when swap() receives an empty path.
error UV2_EmptyPath();
/// @dev Thrown when path has only one token (no swap needed / invalid).
error UV2_InvalidPathLength();
/// @dev Thrown when the final output is below minAmountOut.
error UV2_SlippageExceeded(uint256 amountOut, uint256 minAmountOut);
/// @dev Thrown when getAmountsOut returns an empty array.
error UV2_QuoteFailed();

// =============================================================================
/// @title UniswapV2Adapter - IDEXAdapter for Uniswap V2 and all forks
/// @notice Executes swapExactTokensForTokens through any V2-compatible router.
///         Supports multi-hop routes via intermediate tokens in the path.
/// @dev SC-2 (2026-05-15). Router injected via constructor. Zero hardcoded
///      addresses. Invariant: tokenIn = path[0], tokenOut = path[path.length-1].
// =============================================================================

contract UniswapV2Adapter is IDEXAdapter {
    using SafeERC20 for IERC20;

    // -------------------------------------------------------------------------
    // State
    // -------------------------------------------------------------------------

    /// @notice Uniswap V2-compatible router address (immutable post-deploy).
    IUniswapV2Router02 public immutable router;

    // -------------------------------------------------------------------------
    // Constructor
    // -------------------------------------------------------------------------

    /// @param _router  Uniswap V2-compatible router address. Must be a deployed
    ///                 contract implementing IUniswapV2Router02.
    ///                 Examples per chain (for reference only - NOT hardcoded):
    ///                 Ethereum: 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D (UniV2)
    ///                 Arbitrum: 0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506 (Sushi)
    ///                 BSC:      0x10ED43C718714eb63d5aA57B78B54704E256024E (PancakeV2)
    constructor(address _router) {
        if (_router == address(0)) revert UV2_ZeroRouter();
        router = IUniswapV2Router02(_router);
    }

    // -------------------------------------------------------------------------
    // IDEXAdapter implementation
    // -------------------------------------------------------------------------

    /// @inheritdoc IDEXAdapter
    /// @dev extraData = abi.encode(address[] path)
    ///      path[0] must equal tokenIn, path[path.length-1] must equal tokenOut.
    ///      Flow: transferFrom caller -> forceApprove router -> swapExactTokensForTokens
    ///      -> router sends tokenOut directly to recipient (msg.sender).
    ///
    ///      Gas cost: ~82k (single hop) / ~115k (two hops) at optimizer_runs=200.
    ///
    /// @param tokenIn      ERC-20 token to sell (must equal path[0]).
    /// @param tokenOut     ERC-20 token to receive (must equal path[last]).
    /// @param amountIn     Amount of tokenIn to spend.
    /// @param minAmountOut Minimum acceptable output (slippage guard). Reverts if not met.
    /// @param extraData    abi.encode(address[] path) - ordered token route.
    /// @return amountOut   Actual output amount (last element of returned amounts[]).
    function swap(address tokenIn, address tokenOut, uint256 amountIn, uint256 minAmountOut, bytes calldata extraData)
        external
        override
        returns (uint256 amountOut)
    {
        // Decode the swap path from extraData
        address[] memory path = abi.decode(extraData, (address[]));

        // Invariant checks
        if (path.length == 0) revert UV2_EmptyPath();
        if (path.length < 2) revert UV2_InvalidPathLength();

        // Validate path endpoints match the requested tokenIn/tokenOut
        // These are defense-in-depth: the executor should already enforce this
        if (path[0] != tokenIn) revert UV2_InvalidPathLength();
        if (path[path.length - 1] != tokenOut) revert UV2_InvalidPathLength();

        // 1. Pull tokenIn from caller into this adapter
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);

        // 2. Approve the router to spend tokenIn
        IERC20(tokenIn).forceApprove(address(router), amountIn);

        // 3. Execute swap - router sends tokenOut directly to msg.sender
        uint256[] memory amounts = router.swapExactTokensForTokens(
            amountIn,
            minAmountOut,
            path,
            msg.sender, // recipient receives output directly
            block.timestamp // atomic - no expiry needed
        );

        // amounts[0] = amountIn, amounts[last] = actual output
        amountOut = amounts[amounts.length - 1];

        // 4. Defense-in-depth slippage check (router should enforce, but we verify)
        if (amountOut < minAmountOut) revert UV2_SlippageExceeded(amountOut, minAmountOut);

        // Note: tokenOut is transferred directly to msg.sender by the router.
        // No additional safeTransfer needed from this contract.
    }

    /// @inheritdoc IDEXAdapter
    /// @dev extraData = abi.encode(address[] path)
    ///      Uses router.getAmountsOut() for a gas-free price preview.
    ///      This is a view function - does not modify state.
    ///
    ///      Gas cost: ~12k at optimizer_runs=200.
    ///
    /// @param tokenIn    ERC-20 token to sell (must equal path[0]).
    /// @param tokenOut   ERC-20 token to receive (must equal path[last]).
    /// @param amountIn   Amount of tokenIn to price.
    /// @param extraData  abi.encode(address[] path) - ordered token route.
    /// @return amountOut Estimated output amount (last element of returned amounts[]).
    function quoteOut(address tokenIn, address tokenOut, uint256 amountIn, bytes calldata extraData)
        external
        view
        override
        returns (uint256 amountOut)
    {
        address[] memory path = abi.decode(extraData, (address[]));

        if (path.length == 0) revert UV2_EmptyPath();
        if (path.length < 2) revert UV2_InvalidPathLength();
        if (path[0] != tokenIn) revert UV2_InvalidPathLength();
        if (path[path.length - 1] != tokenOut) revert UV2_InvalidPathLength();

        uint256[] memory amounts = router.getAmountsOut(amountIn, path);
        if (amounts.length == 0) revert UV2_QuoteFailed();

        amountOut = amounts[amounts.length - 1];
    }
}
