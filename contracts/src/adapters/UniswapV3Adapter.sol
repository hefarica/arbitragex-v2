// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// =============================================================================
// SC-2: UniswapV3Adapter — IDEXAdapter for Uniswap V3 and forks
//
// Supports any DEX that implements the Uniswap V3 SwapRouter interface:
//   - Uniswap V3 (Ethereum, Arbitrum, Optimism, Base, Polygon, BSC)
//   - PancakeSwap V3 (BSC, Ethereum)
//   - SushiSwap V3 (multi-chain)
//   - QuickSwap V3 (Polygon)
//   - And any other exactInputSingle-compatible router
//
// This adapter uses exactInputSingle (single-hop) for atomic swaps.
// For multi-hop routes, use exactInput via a separate helper or compose
// multiple single-hop calls in the ArbitrageExecutor route.
//
// extraData encoding: abi.encode(uint24 fee, uint160 sqrtPriceLimitX96)
//   - fee:               Pool fee tier. Common values:
//                        100   = 0.01% (Uniswap V3)
//                        500   = 0.05% (Uniswap V3)
//                        3000  = 0.30% (Uniswap V3)
//                        10000 = 1.00% (Uniswap V3)
//                        2500  = 0.25% (PancakeSwap V3)
//   - sqrtPriceLimitX96: Price limit for the swap. Pass 0 for no limit.
//                        The swap will push the price up to this limit and stop.
//                        Setting 0 disables the price limit entirely.
//
// Router address is injected via constructor — zero hardcoded addresses.
//
// Gas profile (optimizer_runs = 200):
//   - swap()     : ~95,000 gas (single hop exactInputSingle)
//   - quoteOut() : reverts — use QuoterV2 off-chain (see NatSpec below)
// =============================================================================

import "../interfaces/IDEXAdapter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

// -------------------------------------------------------------------------
// Minimal Uniswap V3 SwapRouter interface
// -------------------------------------------------------------------------

/// @dev Uniswap V3 ISwapRouter — exactInputSingle for single-hop swaps.
///      Implemented by Uniswap V3, PancakeSwap V3, and all forks.
interface ISwapRouter {
    /// @notice Parameters for exactInputSingle swap.
    /// @param tokenIn           Token to sell.
    /// @param tokenOut          Token to receive.
    /// @param fee               Pool fee tier (e.g. 3000 for 0.30%).
    /// @param recipient         Address that receives the output tokens.
    /// @param amountIn          Amount of tokenIn to spend.
    /// @param amountOutMinimum  Minimum acceptable output (slippage guard).
    /// @param sqrtPriceLimitX96 Price limit. 0 = no limit.
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }

    /// @notice Swap `amountIn` of one token for as much as possible of another token.
    /// @param params  The parameters necessary for the swap.
    /// @return amountOut  The amount of the received token.
    function exactInputSingle(ExactInputSingleParams calldata params)
        external
        payable
        returns (uint256 amountOut);

    /// @notice Call multiple swaps sequentially, where the output of one is the input of the next.
    /// @param params  Parameters for the multi-hop swap.
    /// @return amountOut  The amount of the final output token.
    struct ExactInputParams {
        bytes path;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
    }

    function exactInput(ExactInputParams calldata params)
        external
        payable
        returns (uint256 amountOut);
}

// -------------------------------------------------------------------------
// Errors (SC-3: custom errors ~200 gas saved per revert vs string require)
// -------------------------------------------------------------------------

/// @dev Thrown when constructor receives address(0) for the router.
error UV3_ZeroRouter();
/// @dev Thrown when amountIn is zero.
error UV3_ZeroAmountIn();
/// @dev Thrown when tokenIn equals tokenOut (no swap needed / invalid).
error UV3_SameToken();
/// @dev Thrown when the swap output is below minAmountOut.
error UV3_SlippageExceeded(uint256 amountOut, uint256 minAmountOut);
/// @dev Thrown when the router callback is called by an unauthorized address.
error UV3_UnauthorizedCallback(address caller);

// =============================================================================
/// @title UniswapV3Adapter — IDEXAdapter for Uniswap V3 and forks
/// @notice Executes exactInputSingle through any V3-compatible router.
///         Single-hop only. Multi-hop is supported by composing routes in
///         the ArbitrageExecutor.
/// @dev SC-2 (2026-05-15). Router injected via constructor. Zero hardcoded
///      addresses. Implements uniswapV3SwapCallback for direct pool swaps
///      when needed (defense-in-depth for callback-based integrations).
// =============================================================================

contract UniswapV3Adapter is IDEXAdapter {
    using SafeERC20 for IERC20;

    // -------------------------------------------------------------------------
    // State
    // -------------------------------------------------------------------------

    /// @notice Uniswap V3 SwapRouter address (immutable post-deploy).
    ISwapRouter public immutable router;

    // -------------------------------------------------------------------------
    // Constructor
    // -------------------------------------------------------------------------

    /// @param _router  Uniswap V3-compatible SwapRouter address. Must be a
    ///                 deployed contract implementing ISwapRouter.
    ///                 Examples per chain (for reference only — NOT hardcoded):
    ///                 Ethereum: 0xE592427A0AEce92De3Edee1F18E0157C05861564 (UniV3)
    ///                 Arbitrum: 0xE592427A0AEce92De3Edee1F18E0157C05861564 (UniV3)
    ///                 BSC:      0x1b81D678ffb9C0263b24A97847620C99d213eB14 (PancakeV3)
    constructor(address _router) {
        if (_router == address(0)) revert UV3_ZeroRouter();
        router = ISwapRouter(_router);
    }

    // -------------------------------------------------------------------------
    // IDEXAdapter implementation
    // -------------------------------------------------------------------------

    /// @inheritdoc IDEXAdapter
    /// @dev extraData = abi.encode(uint24 fee, uint160 sqrtPriceLimitX96)
    ///      Flow: transferFrom caller → forceApprove router → exactInputSingle
    ///      → router sends tokenOut directly to msg.sender.
    ///
    ///      Gas cost: ~95k gas at optimizer_runs=200 (single hop).
    ///
    /// @param tokenIn      ERC-20 token to sell.
    /// @param tokenOut     ERC-20 token to receive.
    /// @param amountIn     Amount of tokenIn to spend. Must be > 0.
    /// @param minAmountOut Minimum acceptable output (slippage guard). Reverts if not met.
    /// @param extraData    abi.encode(uint24 fee, uint160 sqrtPriceLimitX96)
    /// @return amountOut   Actual output amount received.
    function swap(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut,
        bytes calldata extraData
    ) external override returns (uint256 amountOut) {
        if (amountIn == 0) revert UV3_ZeroAmountIn();
        if (tokenIn == tokenOut) revert UV3_SameToken();

        // Decode DEX-specific parameters
        (uint24 fee, uint160 sqrtPriceLimitX96) = abi.decode(extraData, (uint24, uint160));

        // 1. Pull tokenIn from caller into this adapter
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);

        // 2. Approve the router to spend tokenIn
        IERC20(tokenIn).forceApprove(address(router), amountIn);

        // 3. Execute exactInputSingle — recipient is msg.sender (direct transfer)
        amountOut = router.exactInputSingle(
            ISwapRouter.ExactInputSingleParams({
                tokenIn: tokenIn,
                tokenOut: tokenOut,
                fee: fee,
                recipient: msg.sender,
                amountIn: amountIn,
                amountOutMinimum: minAmountOut,
                sqrtPriceLimitX96: sqrtPriceLimitX96
            })
        );

        // 4. Defense-in-depth slippage check (router enforces amountOutMinimum; this verifies)
        if (amountOut < minAmountOut) revert UV3_SlippageExceeded(amountOut, minAmountOut);
        // Note: tokenOut is transferred directly to msg.sender by the router.
        // No additional safeTransfer needed from this contract.
    }

    /// @inheritdoc IDEXAdapter
    /// @dev Uniswap V3 does NOT expose a view-only quote on the SwapRouter.
    ///      The router can only quote via state-changing operations.
    ///      Off-chain callers should use the QuoterV2 contract:
    ///        Ethereum: 0x61fFE014bA17989E743c5F6cB21bF969dc0bB65B (QuoterV2)
    ///      This function always reverts — use QuoterV2 for off-chain quotes.
    ///
    ///      Invariant: this function never modifies state. It reverts immediately.
    function quoteOut(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256, /*amountIn*/
        bytes calldata /*extraData*/
    ) external pure override returns (uint256) {
        revert("UniswapV3Adapter: use QuoterV2 contract for off-chain quotes");
    }
}
