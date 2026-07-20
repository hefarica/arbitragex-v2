// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-1: DyDxFlashAdapter - SKELETON - IFlashLoanProvider for dYdX SoloMargin
//
// STATUS: SKELETON - vm.skip(true) in tests. Full implementation deferred.
//
// Why deferred:
//   dYdX SoloMargin (deployed on Ethereum mainnet) does not have a dedicated
//   flashLoan() function. The pattern is to chain three ISoloMargin.operate()
//   actions in a single call:
//     Action 1: Withdraw(accountId, token, amount) - pull funds from your dYdX account.
//     Action 2: Call(accountId, callee, data)       - callback where you do the arb.
//     Action 3: Deposit(accountId, token, amount+1) - repay (dYdX charges 1 wei fee).
//   Constraints:
//     a) Caller must have a dYdX margin account (accountNumber, owner).
//     b) ISoloMargin.operate() is a complex multi-action struct ABI.
//     c) dYdX only supports WETH, USDC, DAI (market IDs 0, 2, 3).
//     d) The "fee" is 1 wei (effectively 0%).
//     e) dYdX SoloMargin is no longer actively maintained - protocol frozen.
//        Evaluate whether to prioritize vs. other 0%-fee alternatives.
//
// TODO (follow-up session):
//   1. Implement ISoloMargin with Account.Info, Actions.ActionArgs structs.
//   2. Map token address to dYdX marketId (WETH=0, USDC=2, DAI=3).
//   3. Implement the callee interface (ICallee.callFunction callback).
//   4. Manage account ownership (accountId = keccak256(owner, number)).
//   5. Assess protocol activity and liquidity before prioritizing over Balancer.
// =============================================================================

import "../interfaces/IFlashLoanProvider.sol";

/// @title DyDxFlashAdapter - Skeleton IFlashLoanProvider for dYdX SoloMargin operate-call
/// @notice NOT YET IMPLEMENTED. Deploying this contract will revert on all calls.
/// @dev SC-1 (2026-05-08). Full implementation requires ISoloMargin struct encoding.
///      Fee is effectively 0% (1 wei per loan). See file header for full rationale.
contract DyDxFlashAdapter is IFlashLoanProvider {
    error NotImplemented();

    // solhint-disable-next-line no-empty-blocks
    constructor() {}

    /// @dev TODO: encode 3-action ISoloMargin.operate() call (Withdraw/Call/Deposit).
    function flashLoan(
        address, /*receiver*/
        address, /*asset*/
        uint256, /*amount*/
        bytes calldata /*params*/
    )
        external
        pure
        override
    {
        revert NotImplemented();
    }

    /// @dev dYdX charges 1 wei per flash loan (effectively 0%).
    ///      TODO: return 1 when fully implemented. Currently reverts.
    function flashLoanFee(
        uint256 /*amount*/
    )
        external
        pure
        override
        returns (uint256)
    {
        revert NotImplemented();
    }

    /// @dev TODO: query ISoloMargin for the market balance of the given token.
    ///      Only WETH (market 0), USDC (market 2), DAI (market 3) are supported.
    function maxFlashLoan(
        address /*asset*/
    )
        external
        pure
        override
        returns (uint256)
    {
        revert NotImplemented();
    }
}
