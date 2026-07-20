// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-2: GMXAdapter - SKELETON - IDEXAdapter for GMX V1 Router
//
// STATUS: SKELETON - vm.skip(true) in tests. Full implementation deferred.
//
// Why deferred:
//   GMX V1 (Arbitrum + Avalanche) uses a Router.swap() that routes through
//   its GLP liquidity pool. Key complexities:
//     a) Swap path is an address[] array (intermediate tokens), not a pair.
//     b) GMX charges a fee that varies by token (spread-based, not bps).
//     c) GMX V2 (GMX Synthetics / "GM pools") uses a completely different
//        interface: ExchangeRouter + order creation - not atomic.
//     d) V1 Router.swap() requires WETH wrapping for ETH paths.
//     e) Price impact depends on GLP composition - no simple view quote.
//     f) V2 is replacing V1; assessing whether V1 liquidity is still viable
//        for arbitrage purposes before investing implementation time.
//
// TODO (follow-up session):
//   1. Confirm target: GMX V1 Router or GMX V2 ExchangeRouter.
//   2. GMX V1: implement IGMXRouter.swap(path, amountIn, minOut, receiver).
//   3. GMX V2: implement order-based flow (non-atomic, out of scope for flash arb).
//   4. Add fee estimation via IGMXVault.getFeeBasisPoints().
//   5. Validate token whitelist via IGMXVault.whitelistedTokens().
// =============================================================================

import "../interfaces/IDEXAdapter.sol";

/// @title GMXAdapter - Skeleton IDEXAdapter for GMX Router
/// @notice NOT YET IMPLEMENTED. All calls revert.
/// @dev SC-2 (2026-05-08). Full implementation requires GMX V1/V2 decision.
///      See file header for the full rationale.
contract GMXAdapter is IDEXAdapter {
    error NotImplemented();

    // solhint-disable-next-line no-empty-blocks
    constructor() {}

    /// @dev TODO: implement IGMXRouter.swap(path, amountIn, minOut, receiver).
    function swap(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256, /*amountIn*/
        uint256, /*minAmountOut*/
        bytes calldata /*extraData*/
    )
        external
        pure
        override
        returns (uint256)
    {
        revert NotImplemented();
    }

    /// @dev TODO: implement via IGMXVault price feeds or off-chain estimation.
    function quoteOut(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256, /*amountIn*/
        bytes calldata /*extraData*/
    )
        external
        pure
        override
        returns (uint256)
    {
        revert NotImplemented();
    }
}
