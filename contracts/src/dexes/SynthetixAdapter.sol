// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-2: SynthetixAdapter — SKELETON — IDEXAdapter for Synthetix exchange
//
// STATUS: SKELETON — vm.skip(true) in tests. Full implementation deferred.
//
// Why deferred:
//   Synthetix V2 (legacy) uses an exchange mechanism that is fundamentally
//   different from AMM swaps:
//     a) All exchanges go through sUSD as an intermediate synthetic.
//        A → sUSD → B requires two exchange() calls or the newer
//        exchangeWithTracking() with a referral tracking code.
//     b) Synthetix uses oracle prices (Chainlink aggregators) not AMM curves.
//        There is no liquidity depth — but there IS a "dynamic fee" that can
//        spike dramatically during volatility (up to 10%+ for some pairs).
//     c) Settlement delay: exchanges are not atomic. Synth positions require
//        a ~2-3 minute settlement window. INCOMPATIBLE with flash loan atomicity.
//     d) Synthetix V3 migration is ongoing — V2 is being deprecated.
//   Conclusion: Synthetix V2 is NOT compatible with atomic flash loan arbitrage
//   due to the settlement delay. V3 may address this with instant settlement.
//
// TODO (evaluate in follow-up):
//   1. Verify if Synthetix V3 Perps V3 offers instant swap settlement.
//   2. If atomic: implement ISynthetixExchangeV3.swap() interface.
//   3. If not atomic: remove from adapter roadmap permanently.
// =============================================================================

import "../interfaces/IDEXAdapter.sol";

/// @title SynthetixAdapter — Skeleton IDEXAdapter for Synthetix exchange
/// @notice NOT YET IMPLEMENTED. All calls revert.
/// @dev SC-2 (2026-05-08). May not be implementable due to settlement delay in V2.
///      See file header for the full rationale.
contract SynthetixAdapter is IDEXAdapter {
    error NotImplemented();

    // solhint-disable-next-line no-empty-blocks
    constructor() {}

    /// @dev TODO: evaluate V3 instant settlement. If viable, implement
    ///      ISynthetix.exchange(sourceCurrencyKey, sourceAmount, destinationCurrencyKey).
    function swap(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256, /*amountIn*/
        uint256, /*minAmountOut*/
        bytes calldata /*extraData*/
    ) external pure override returns (uint256) {
        revert NotImplemented();
    }

    /// @dev TODO: if viable, implement via ISynthetixExchangeRates.effectiveValue().
    function quoteOut(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256, /*amountIn*/
        bytes calldata /*extraData*/
    ) external pure override returns (uint256) {
        revert NotImplemented();
    }
}
