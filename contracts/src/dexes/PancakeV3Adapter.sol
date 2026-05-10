// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-2: PancakeV3Adapter — IDEXAdapter for PancakeSwap V3 (UniV3 fork)
//
// PancakeSwap V3 is a near-identical fork of Uniswap V3. The swap router ABI
// (ISwapRouter.exactInputSingle) is the same. The difference is in the deployed
// factory and router addresses — injected via constructor, not hardcoded.
//
// extraData encoding: abi.encode(uint24 fee, uint160 sqrtPriceLimitX96)
//   - fee:               Pool fee tier: 100, 500, 2500, or 10000 (Pancake tiers).
//   - sqrtPriceLimitX96: Price limit for the swap. Pass 0 for no limit.
//
// Note: Uniswap V3 fee tiers are 100/500/3000/10000. Pancake V3 uses slightly
// different tiers (100/500/2500/10000) — the router routes to the correct pool
// based on fee tier, so passing a Uniswap tier on Pancake will silently fail
// if no pool exists at that tier.
// =============================================================================

import "../interfaces/IDEXAdapter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @dev UniV3-compatible ISwapRouter (shared by PancakeSwap V3 fork).
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

    /// @notice Swap `amountIn` of one token for as much as possible of another token.
    /// @param params  The parameters necessary for the swap.
    /// @return amountOut  The amount of the received token.
    function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
}

/// @title PancakeV3Adapter — IDEXAdapter for PancakeSwap V3 (UniV3-compatible router)
/// @notice Full implementation. Pulls tokenIn from caller, swaps via PancakeSwap V3
///         exactInputSingle, returns tokenOut.
/// @dev SC-2 (2026-05-08). Router address is constructor-injected (no hardcoding).
///      Pool selection is by fee tier encoded in extraData.
contract PancakeV3Adapter is IDEXAdapter {
    using SafeERC20 for IERC20;

    // -------------------------------------------------------------------------
    // State
    // -------------------------------------------------------------------------

    /// @notice PancakeSwap V3 SwapRouter address (immutable post-deploy).
    ISwapRouter public immutable router;

    // -------------------------------------------------------------------------
    // Errors
    // -------------------------------------------------------------------------

    /// @dev Thrown when the swap output is below minAmountOut.
    error PV3_SlippageExceeded(uint256 amountOut, uint256 minAmountOut);

    // -------------------------------------------------------------------------
    // Constructor
    // -------------------------------------------------------------------------

    /// @param _router  PancakeSwap V3 SwapRouter address (e.g. BSC mainnet 0x...).
    constructor(address _router) {
        require(_router != address(0), "PancakeV3Adapter: zero router");
        router = ISwapRouter(_router);
    }

    // -------------------------------------------------------------------------
    // IDEXAdapter implementation
    // -------------------------------------------------------------------------

    /// @inheritdoc IDEXAdapter
    /// @dev extraData = abi.encode(uint24 fee, uint160 sqrtPriceLimitX96)
    ///      deadline = block.timestamp (atomic execution — no expiry needed).
    function swap(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut,
        bytes calldata extraData
    ) external override returns (uint256 amountOut) {
        (uint24 fee, uint160 sqrtPriceLimitX96) = abi.decode(extraData, (uint24, uint160));

        // 1. Pull tokenIn from caller
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);

        // 2. Approve the PancakeSwap V3 router
        IERC20(tokenIn).forceApprove(address(router), amountIn);

        // 3. Execute exactInputSingle — recipient is msg.sender (caller receives tokenOut)
        amountOut = router.exactInputSingle(
            ISwapRouter.ExactInputSingleParams({
                tokenIn: tokenIn,
                tokenOut: tokenOut,
                fee: fee,
                recipient: msg.sender,
                deadline: block.timestamp,
                amountIn: amountIn,
                amountOutMinimum: minAmountOut,
                sqrtPriceLimitX96: sqrtPriceLimitX96
            })
        );

        // 4. Redundant slippage check (router enforces amountOutMinimum; this is defense-in-depth)
        if (amountOut < minAmountOut) revert PV3_SlippageExceeded(amountOut, minAmountOut);
        // Note: tokenOut is transferred directly to recipient (msg.sender) by the router.
        // No additional safeTransfer needed.
    }

    /// @inheritdoc IDEXAdapter
    /// @dev PancakeSwap V3 (like UniV3) does not expose a view-only quote on the router.
    ///      Off-chain callers should use eth_call on exactInputSingle or query
    ///      the Quoter contract (IQuoterV2.quoteExactInputSingle) directly.
    ///      This function always reverts — use the Quoter for previews.
    function quoteOut(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256, /*amountIn*/
        bytes calldata /*extraData*/
    ) external pure override returns (uint256) {
        // TODO: integrate IQuoterV2.quoteExactInputSingle() in follow-up.
        // PancakeSwap V3 Quoter: separate contract, not on ISwapRouter.
        revert("PancakeV3Adapter: use Quoter contract for off-chain quotes");
    }
}
