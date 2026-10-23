// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-9: IFlashLoanProvider - provider-agnostic flash loan abstraction
//
// Purpose: Decouple FlashLoanExecutor from a concrete Aave V3 interface so that
// SC-1 (multi-provider flash loans: Balancer 0%, dYdX 0%, Aave 0.05%) can be
// implemented by swapping the concrete adapter without touching the executor logic.
//
// Adapters to implement (SC-1, deferred):
//   - BalancerFlashLoanAdapter  - IBalancerVault.flashLoan() (0% fee)
//   - DyDxFlashLoanAdapter      - ISoloMargin.operate() (0% fee)
//   - AaveV3FlashLoanAdapter     - IAaveV3Pool.flashLoanSimple() (0.05% fee)
//
// Provider selection priority per §19 / §29 gas optimization:
//   Balancer (0%) > dYdX (0%) > Aave (0.05%)
// =============================================================================

/// @title IFlashLoanProvider - abstract flash loan provider interface
/// @notice Standard interface for borrowing tokens atomically, repaying in the
///         same transaction. Implementations adapt different protocols
///         (Aave V3, Balancer, dYdX) to this common interface.
/// @dev SC-1 follow-up: FlashLoanExecutor will accept an IFlashLoanProvider
///      instead of a hardcoded IAaveV3Pool. This interface is created now
///      to lock the API contract before adapters are written.
interface IFlashLoanProvider {
    /// @notice Initiate a flash loan of `amount` of `asset` to `receiver`.
    /// @dev The receiver must implement the provider-specific callback
    ///      (e.g. executeOperation for Aave, receiveFlashLoan for Balancer).
    ///      The receiver is responsible for repaying `amount + fee(amount)`
    ///      before the call returns, or the transaction reverts.
    /// @param receiver  Contract that receives the loan and implements the callback.
    /// @param asset     ERC-20 token to borrow.
    /// @param amount    Amount to borrow.
    /// @param params    Arbitrary calldata forwarded to the receiver callback.
    function flashLoan(address receiver, address asset, uint256 amount, bytes calldata params) external;

    /// @notice Compute the fee charged by this provider for a given loan amount.
    /// @dev Returns 0 for zero-fee providers (Balancer, dYdX).
    ///      Returns (amount * feeBps) / 10_000 for fee-charging providers (Aave 0.05%).
    /// @param amount  Hypothetical loan amount to price.
    /// @return fee    Fee in the same units as `amount`.
    function flashLoanFee(uint256 amount) external view returns (uint256 fee);

    /// @notice Maximum borrowable amount for `asset` from this provider.
    /// @dev Returns 0 if the asset is not supported or liquidity is exhausted.
    ///      Callers should fall back to a cheaper/deeper provider if this returns 0.
    /// @param asset  ERC-20 token to query.
    /// @return max   Maximum flash-loanable amount.
    function maxFlashLoan(address asset) external view returns (uint256 max);
}
