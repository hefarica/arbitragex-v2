// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-1: UniV3FlashAdapter — SKELETON — IFlashLoanProvider for Uniswap V3 Pool flash
//
// STATUS: SKELETON — vm.skip(true) in tests. Full implementation deferred.
//
// Why deferred:
//   Uniswap V3 flash loans are issued at the POOL level, not a central vault.
//   The caller must pick a specific (tokenA, tokenB, fee) pool, and the callback
//   uniswapV3FlashCallback(fee0, fee1, data) is emitted by that pool's contract.
//   Fee for the flash is the pool's swap fee (e.g. 0.05%, 0.3%, 1%), NOT zero.
//   This means:
//     a) The adapter needs to accept a pool address per call (not just asset+amount).
//     b) flashLoanFee depends on the pool's fee tier (100/500/3000/10000 bps ÷ 100).
//     c) The callback signature differs from both Aave and Balancer.
//     d) FlashLoanExecutor must also implement uniswapV3FlashCallback().
//   All of the above require a dedicated dispatch session.
//
// TODO (follow-up session):
//   1. Accept poolAddress as parameter (extend IFlashLoanProvider or use extraData).
//   2. Implement uniswapV3FlashCallback on FlashLoanExecutor.
//   3. Calculate fee as (amount0 * feeTier / 1_000_000) + 1 (Uniswap V3 ceil math).
//   4. Add pool discovery via IUniswapV3Factory.getPool(tokenA, tokenB, fee).
// =============================================================================

import "../interfaces/IFlashLoanProvider.sol";

/// @title UniV3FlashAdapter — Skeleton IFlashLoanProvider for Uniswap V3 pool flash
/// @notice NOT YET IMPLEMENTED. Deploying this contract will revert on all calls.
/// @dev SC-1 (2026-05-08). Full implementation requires pool-level callback wiring.
///      See file header for the full rationale.
contract UniV3FlashAdapter is IFlashLoanProvider {
    error NotImplemented();

    // solhint-disable-next-line no-empty-blocks
    constructor() {}

    /// @dev TODO: implement with IUniswapV3Pool.flash() + uniswapV3FlashCallback.
    function flashLoan(
        address, /*receiver*/
        address, /*asset*/
        uint256, /*amount*/
        bytes calldata /*params*/
    ) external pure override {
        revert NotImplemented();
    }

    /// @dev TODO: return (amount * feeTier) / 1_000_000 + 1 (per Uniswap V3 math).
    ///      feeTier is pool-specific: 100, 500, 3000, or 10000.
    function flashLoanFee(uint256 /*amount*/) external pure override returns (uint256) {
        revert NotImplemented();
    }

    /// @dev TODO: return IUniswapV3Pool.token0/token1 balance for the chosen pool.
    function maxFlashLoan(address /*asset*/) external pure override returns (uint256) {
        revert NotImplemented();
    }
}
