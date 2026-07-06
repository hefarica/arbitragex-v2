// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// SC-1: AaveV3FlashAdapter — concrete IFlashLoanProvider for Aave V3
//
// Fee: 0.09% (FLASHLOAN_PREMIUM_TOTAL on Aave V3 mainnet).
// Callback: Aave calls executeOperation(asset, amount, premium, initiator, params)
//           on the `receiver` address passed in flashLoan().
// Priority: LOWEST among zero/low-fee providers (Balancer 0% > dYdX 0% > Aave 0.09%).
// =============================================================================

import "../interfaces/IFlashLoanProvider.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

interface IAaveV3PoolAdapter {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

/// @title AaveV3FlashAdapter — IFlashLoanProvider adapter for Aave V3 flashLoanSimple
/// @notice Full implementation. Fee = 0.09% (9 bps). Referral code fixed to 0.
/// @dev SC-1 (2026-05-08). Part of multi-provider flash loan architecture.
///
/// TODO(M12, audit 2026-05-10): wire this adapter into ArbitrageExecutor in a follow-up sprint.
/// Currently ArbitrageExecutor uses router.call(payload) directly with a per-router selector
/// whitelist (A5). This adapter provides the IFlashLoanProvider abstraction but is NOT
/// yet invoked from the hot path. Migration to typed adapters is planned for Sprint 4+.
contract AaveV3FlashAdapter is IFlashLoanProvider {
    /// @notice Aave V3 lending pool address.
    IAaveV3PoolAdapter public immutable aavePool;

    /// @notice Aave V3 flash loan fee in basis points (9 = 0.09%).
    uint256 public constant FEE_BPS = 9;

    constructor(address _aavePool) {
        require(_aavePool != address(0), "AaveV3FlashAdapter: zero pool");
        aavePool = IAaveV3PoolAdapter(_aavePool);
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev Delegates to Aave V3 flashLoanSimple with referralCode=0.
    ///      Receiver must implement IFlashLoanReceiver (executeOperation callback).
    function flashLoan(
        address receiver,
        address asset,
        uint256 amount,
        bytes calldata params
    ) external override {
        aavePool.flashLoanSimple(receiver, asset, amount, params, 0);
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev Returns (amount * 9) / 10_000 — Aave V3 FLASHLOAN_PREMIUM_TOTAL = 0.09%.
    ///      Uses unchecked: amount * 9 cannot overflow uint256 for any realistic loan.
    function flashLoanFee(uint256 amount) external pure override returns (uint256) {
        unchecked {
            return (amount * FEE_BPS) / 10_000;
        }
    }

    /// @inheritdoc IFlashLoanProvider
    /// @dev Aave V3 does not expose a direct maxFlashLoan view without the
    ///      DataProvider contract. Return type(uint256).max as a conservative
    ///      "no hard limit on this side" signal — callers should verify
    ///      protocol reserves separately.
    function maxFlashLoan(address /*asset*/) external pure override returns (uint256) {
        return type(uint256).max;
    }
}
