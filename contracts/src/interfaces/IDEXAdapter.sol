// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-2: IDEXAdapter - DEX-agnostic swap interface
//
// Purpose: Decouple ArbitrageExecutor from concrete DEX implementations.
//          Each adapter handles the token transfer, approval, swap, and return
//          of funds internally. The executor just calls swap() with a budget.
//
// Design invariants:
//   1. Adapter is responsible for pulling tokenIn from msg.sender and pushing
//      tokenOut back to msg.sender atomically.
//   2. extraData encodes DEX-specific params (pool address, indices, fee tier, etc.).
//   3. quoteOut() is a view function - gas-free preview for route planning off-chain.
//   4. Adapters MUST revert if amountOut < minAmountOut (slippage guard).
//
// Implemented adapters (SC-2, 2026-05-08):
//   - CurveAdapter     - exchange() + get_dy() via ICurvePool
//   - PancakeV3Adapter - UniV3-fork ISwapRouter with Pancake factory
//
// Skeleton adapters (follow-up):
//   - GMXAdapter        - GMX Router.swap() (complex perpetuals integration)
//   - SynthetixAdapter  - Synthetix.exchange() via sUSD intermediate
// =============================================================================

/// @title IDEXAdapter - abstract interface for a single DEX swap adapter
/// @notice Stateless adapter that wraps a DEX swap into a standard interface.
///         The caller is responsible for ensuring sufficient tokenIn balance
///         before calling swap().
interface IDEXAdapter {
    /// @notice Execute a swap from `tokenIn` to `tokenOut` using this DEX.
    /// @dev Implementation MUST:
    ///      1. Pull `amountIn` of `tokenIn` from msg.sender (safeTransferFrom).
    ///      2. Approve the underlying pool/router for `amountIn`.
    ///      3. Execute the swap.
    ///      4. Push the resulting `amountOut` of `tokenOut` back to msg.sender (safeTransfer).
    ///      5. Revert if amountOut < minAmountOut.
    /// @param tokenIn      ERC-20 token to sell.
    /// @param tokenOut     ERC-20 token to receive.
    /// @param amountIn     Amount of tokenIn to spend.
    /// @param minAmountOut Minimum acceptable output (slippage guard). Reverts if not met.
    /// @param extraData    DEX-specific encoded parameters (pool, indices, fee tier, etc.).
    /// @return amountOut   Actual output amount received.
    function swap(address tokenIn, address tokenOut, uint256 amountIn, uint256 minAmountOut, bytes calldata extraData)
        external
        returns (uint256 amountOut);

    /// @notice Preview the output of a swap without executing it.
    /// @dev Must not modify state. May revert if the DEX does not support
    ///      a view-only quote (in which case off-chain simulation via eth_call
    ///      is the alternative).
    /// @param tokenIn    ERC-20 token to sell.
    /// @param tokenOut   ERC-20 token to receive.
    /// @param amountIn   Amount of tokenIn to price.
    /// @param extraData  DEX-specific encoded parameters (same encoding as swap()).
    /// @return amountOut Estimated output amount (not guaranteed - subject to slippage).
    function quoteOut(address tokenIn, address tokenOut, uint256 amountIn, bytes calldata extraData)
        external
        view
        returns (uint256 amountOut);
}
