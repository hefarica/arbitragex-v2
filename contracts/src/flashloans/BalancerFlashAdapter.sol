// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-1: BalancerFlashAdapter — concrete IFlashLoanProvider for Balancer V2 Vault
//
// Fee: 0% (Balancer flash loans are free as of V2 — the protocol does not
//      charge a fee for flash loans; only LP swaps accrue fees).
// Callback: Balancer calls receiveFlashLoan(tokens, amounts, feeAmounts, userData)
//           on the `recipient` address. The recipient must repay before returning.
// Priority: HIGHEST — use this provider first (0% fee, deep liquidity for blue chips).
//
// Reference: https://docs.balancer.fi/reference/contracts/flash-loans.html
// =============================================================================

import "../interfaces/IFlashLoanProvider.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/// @dev Minimal Balancer V2 Vault interface for flash loans.
interface IBalancerVault {
    /// @param recipient  Contract that will receive tokens and be called back.
    /// @param tokens     Array of ERC-20 token addresses to borrow (must be sorted).
    /// @param amounts    Array of amounts to borrow (parallel with tokens).
    /// @param userData   Opaque calldata forwarded to recipient.receiveFlashLoan().
    function flashLoan(
        address recipient,
        address[] calldata tokens,
        uint256[] calldata amounts,
        bytes calldata userData
    ) external;
}

/// @title BalancerFlashAdapter — IFlashLoanProvider adapter for Balancer V2 Vault
/// @notice Full implementation. Fee = 0%. Preferred provider in the selection hierarchy.
/// @dev SC-1 (2026-05-08). Part of multi-provider flash loan architecture.
///
///      IMPORTANT — callback difference from Aave:
///      Balancer calls `receiveFlashLoan(tokens, amounts, feeAmounts, userData)`
///      on the receiver, NOT `executeOperation`. FlashLoanExecutor must implement
///      this callback to repay Balancer loans. This is handled in FlashLoanExecutor
///      via a dedicated receiveFlashLoan function (see FlashLoanExecutor.sol, slot 3+).
contract BalancerFlashAdapter is IFlashLoanProvider {
    /// @notice Balancer V2 Vault address.
    IBalancerVault public immutable vault;

    constructor(address _vault) {
        require(_vault != address(0), "BalancerFlashAdapter: zero vault");
        vault = IBalancerVault(_vault);
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev Wraps the single-asset request into Balancer's array-based interface.
    ///      tokens[0] = asset, amounts[0] = amount.
    ///      userData is forwarded unchanged to recipient.receiveFlashLoan().
    function flashLoan(
        address receiver,
        address asset,
        uint256 amount,
        bytes calldata params
    ) external override {
        address[] memory tokens = new address[](1);
        tokens[0] = asset;
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        vault.flashLoan(receiver, tokens, amounts, params);
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev Balancer V2 flash loans are free — always returns 0.
    function flashLoanFee(uint256 /*amount*/) external pure override returns (uint256) {
        return 0;
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev Returns the Vault's current balance of `asset`.
    ///      This is the maximum borrowable amount (Balancer requires full repayment
    ///      before the flashLoan call returns, so the entire balance is available).
    function maxFlashLoan(address asset) external view override returns (uint256) {
        return IERC20(asset).balanceOf(address(vault));
    }
}
