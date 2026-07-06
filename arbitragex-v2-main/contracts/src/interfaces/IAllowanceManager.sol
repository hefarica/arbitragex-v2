// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-5: IAllowanceManager — view interface for AllowanceManager integration
//
// Purpose: Allow ArbitrageExecutor (and other consumers) to verify that a
// (token, spender) pair has a live allowance managed by AllowanceManager
// before executing a swap step.
//
// Design rationale:
//   AllowanceManager holds ERC-20 allowances FROM ITSELF to router spenders,
//   via forceApprove(). The actual allowance lives on each ERC-20 token contract
//   at: IERC20(token).allowance(address(AllowanceManager), spender).
//   Therefore isApproved() and getAllowance() delegate to the token contract —
//   they do NOT store additional state on AllowanceManager.
//
// This interface is intentionally minimal (read-only). Write functions
// (grantAllowance, revokeAllowance, batch variants) are admin-only and
// consumers of this interface should never need them.
// =============================================================================

/// @title IAllowanceManager — read interface for AllowanceManager
/// @notice Exposes allowance query functions used by ArbitrageExecutor
///         to verify router approvals before executing swap routes.
interface IAllowanceManager {
    /// @notice Returns true if `spender` currently has a non-zero allowance
    ///         from this AllowanceManager for `token`.
    /// @dev    Equivalent to IERC20(token).allowance(address(this), spender) > 0.
    ///         Returns false if the allowance was never granted or was revoked.
    /// @param token    ERC-20 token address.
    /// @param spender  Router or contract to query.
    /// @return         True if allowance > 0.
    function isApproved(address token, address spender) external view returns (bool);

    /// @notice Returns the current allowance `spender` has from this
    ///         AllowanceManager for `token`.
    /// @dev    Equivalent to IERC20(token).allowance(address(this), spender).
    ///         Returns 0 if the allowance was never granted or was fully consumed.
    /// @param token    ERC-20 token address.
    /// @param spender  Router or contract to query.
    /// @return         Allowance amount in token's native units.
    function getAllowance(address token, address spender) external view returns (uint256);
}
