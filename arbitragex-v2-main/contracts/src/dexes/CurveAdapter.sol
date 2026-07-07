// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-2: CurveAdapter — IDEXAdapter for Curve Finance plain pools
//
// Supports Curve pools that implement:
//   - exchange(int128 i, int128 j, uint256 dx, uint256 min_dy) external returns (uint256)
//   - get_dy(int128 i, int128 j, uint256 dx) external view returns (uint256)
//
// This covers the majority of Curve plain pools (3pool, stETH, FRAX, etc.).
// Meta-pools (factory pools) use a slightly different ABI — not covered here.
//
// extraData encoding: abi.encode(address pool, int128 i, int128 j)
//   - pool: Curve pool contract address
//   - i:    Index of tokenIn in the pool's coin array
//   - j:    Index of tokenOut in the pool's coin array
//
// Note on int128: Curve uses int128 for coin indices. Solidity ABI-encodes int128
// as a 32-byte word (sign-extended). abi.decode handles this correctly.
// =============================================================================

import "../interfaces/IDEXAdapter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @dev Minimal Curve plain pool interface. Covers StableSwap original and NG variants.
interface ICurvePool {
    /// @notice Perform an exchange between two coins.
    /// @param i       Index of input coin.
    /// @param j       Index of output coin.
    /// @param dx      Amount of i to sell.
    /// @param min_dy  Minimum amount of j to receive (slippage guard at pool level).
    /// @return        Actual amount of j received.
    function exchange(int128 i, int128 j, uint256 dx, uint256 min_dy) external returns (uint256);

    /// @notice Calculate the amount of j received for selling `dx` of i.
    /// @param i   Index of input coin.
    /// @param j   Index of output coin.
    /// @param dx  Amount of i to sell.
    /// @return    Estimated amount of j (subject to on-chain state at time of call).
    function get_dy(int128 i, int128 j, uint256 dx) external view returns (uint256);
}

/// @title CurveAdapter — IDEXAdapter for Curve plain pool exchange()
/// @notice Full implementation. Pulls tokenIn from caller, swaps via Curve, returns tokenOut.
/// @dev SC-2 (2026-05-08). Pool address and coin indices encoded in extraData.
///
/// TODO(M12, audit 2026-05-10): wire this adapter into ArbitrageExecutor in a follow-up sprint.
/// Currently ArbitrageExecutor uses router.call(payload) directly with a per-router selector
/// whitelist (A5). This adapter provides stronger per-DEX typing and validation but is NOT
/// yet invoked from the hot path. Migration to typed adapters is planned for Sprint 4+.
contract CurveAdapter is IDEXAdapter {
    using SafeERC20 for IERC20;

    // -------------------------------------------------------------------------
    // Errors
    // -------------------------------------------------------------------------

    /// @dev Thrown when abi.decode(extraData) yields a zero pool address.
    error CA_ZeroPoolAddress();
    /// @dev Thrown when the swap output is below minAmountOut after execution.
    error CA_SlippageExceeded(uint256 amountOut, uint256 minAmountOut);

    // -------------------------------------------------------------------------
    // IDEXAdapter implementation
    // -------------------------------------------------------------------------

    /// @inheritdoc IDEXAdapter
    /// @dev extraData = abi.encode(address pool, int128 i, int128 j)
    ///      Flow: transferFrom caller → forceApprove pool → exchange() → transfer to caller
    function swap(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut,
        bytes calldata extraData
    ) external override returns (uint256 amountOut) {
        (address pool, int128 i, int128 j) = abi.decode(extraData, (address, int128, int128));
        if (pool == address(0)) revert CA_ZeroPoolAddress();

        // 1. Pull tokenIn from caller
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);

        // 2. Approve Curve pool to spend our tokenIn
        IERC20(tokenIn).forceApprove(pool, amountIn);

        // 3. Execute the swap — pass minAmountOut to pool's own slippage guard
        amountOut = ICurvePool(pool).exchange(i, j, amountIn, minAmountOut);

        // 4. Redundant slippage check (defense in depth — pool may not enforce)
        if (amountOut < minAmountOut) revert CA_SlippageExceeded(amountOut, minAmountOut);

        // 5. Push tokenOut back to caller
        IERC20(tokenOut).safeTransfer(msg.sender, amountOut);
    }

    /// @inheritdoc IDEXAdapter
    /// @dev extraData = abi.encode(address pool, int128 i, int128 j)
    function quoteOut(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256 amountIn,
        bytes calldata extraData
    ) external view override returns (uint256 amountOut) {
        (address pool, int128 i, int128 j) = abi.decode(extraData, (address, int128, int128));
        if (pool == address(0)) revert CA_ZeroPoolAddress();
        return ICurvePool(pool).get_dy(i, j, amountIn);
    }
}
