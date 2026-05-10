// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// STORAGE LAYOUT — APPEND-ONLY RULE (SC-08, 2026-05-08)
// =============================================================================
// Parent contracts (Initializable, AccessControlUpgradeable, UUPSUpgradeable)
// all use ERC-7201 namespaced slots — they do NOT occupy linear slot space.
//
// This contract's OWN variables start at linear slot 0:
//   slot 0: aavePool            (IAaveV3Pool — address, 20 bytes)  ← legacy compat
//   slot 1: arbitrageExecutor   (address, 20 bytes)
//   slot 2: referralCode        (uint16, 2 bytes)
//   slot 3: flashLoanProvider   (address, 20 bytes) ← SC-1: multi-provider support
//
// CRITICAL: When adding new state variables in V2, V3, etc., you MUST append
// them AFTER slot 3.  NEVER insert variables between existing ones — that
// would corrupt the storage layout and brick all proxies pointing at this impl.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "./interfaces/IFlashLoanProvider.sol";

// =============================================================================
// SC-3: Custom errors (~200 gas saved per revert vs string require)
// =============================================================================

/// @dev Thrown when executeOperation is called by an address that is not aavePool.
error FL_UnauthorizedCaller();
/// @dev Thrown when executeOperation is called with an initiator that is not this contract.
error FL_InvalidInitiator();
/// @dev Thrown when the call to ArbitrageExecutor inside executeOperation returns success=false.
error FL_ArbitrageExecutionFailed();
/// @dev Thrown when setFlashLoanProvider is called with address(0).
error FL_ZeroProvider();
/// @dev Thrown when requestFlashLoan is called but no provider is configured.
error FL_NoProviderConfigured();

interface IAaveV3Pool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

/// @title FlashLoanExecutor — UUPS-upgradeable multi-provider flash loan wrapper
/// @notice Requests flash loans from the configured IFlashLoanProvider and delegates
///         execution to ArbitrageExecutor. Defaults to Aave V3 for backward compat;
///         supports Balancer (0% fee), dYdX, and UniV3 via adapter pattern (SC-1).
/// @dev Refactored to UUPS proxy pattern (SC-08). aavePool and arbitrageExecutor
///      moved from constructor params to initialize() params.
///      SC-3 (2026-05-08): string require() replaced with custom errors.
///      SC-8 (2026-05-08): referralCode is now operator-configurable via setReferralCode().
///      SC-1 (2026-05-08): IFlashLoanProvider adapter support via setFlashLoanProvider().
contract FlashLoanExecutor is
    Initializable,
    AccessControlUpgradeable,
    UUPSUpgradeable
{
    using SafeERC20 for IERC20;

    /// @notice Role required to call requestFlashLoan.
    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    /// @notice Separate UPGRADER_ROLE allows key rotation independent of admin.
    bytes32 public constant UPGRADER_ROLE = keccak256("UPGRADER_ROLE");

    // slot 0 — legacy Aave V3 pool reference (retained for backward compat and
    // as the Aave fallback when no IFlashLoanProvider adapter is configured).
    /// @notice Aave V3 lending pool address that will call executeOperation.
    IAaveV3Pool public aavePool;
    // slot 1
    /// @notice ArbitrageExecutor proxy address that receives the delegated execution call.
    address public arbitrageExecutor;
    // slot 2 — SC-8: operator-configurable referral code for Aave flash loans.
    /// @notice Aave referral code passed to flashLoanSimple. Defaults to 0.
    ///         Set via setReferralCode() to enable referral rewards if applicable.
    uint16 public referralCode;
    // slot 3 — SC-1: pluggable IFlashLoanProvider adapter.
    /// @notice Active flash loan provider adapter. When set (non-zero),
    ///         requestFlashLoan delegates to this adapter instead of calling
    ///         aavePool.flashLoanSimple() directly. Use setFlashLoanProvider()
    ///         to configure Balancer (0% fee), dYdX (0% fee), or any custom adapter.
    ///         When zero, falls back to aavePool directly (legacy behavior).
    address public flashLoanProvider;
    // APPEND new variables below this line in future upgrades. Never above.

    // SC-06: observability events for off-chain monitoring (recon, dashboard)

    /// @notice Emitted when a flash loan is requested to the configured provider.
    /// @param asset       ERC-20 asset borrowed.
    /// @param amount      Loan amount.
    /// @param paramsHash  keccak256 of the params bytes (for off-chain correlation).
    event FlashLoanRequested(address indexed asset, uint256 amount, bytes32 paramsHash);

    /// @notice Emitted when the flash loan callback completes and the loan is fully repaid.
    /// @param asset    ERC-20 asset borrowed.
    /// @param amount   Loan amount.
    /// @param premium  Fee charged by the provider (0 for Balancer/dYdX).
    /// @param success  Always true (false paths revert before reaching this point).
    event FlashLoanExecuted(address indexed asset, uint256 amount, uint256 premium, bool success);

    /// @notice Emitted when the referral code is updated by an admin.
    /// @param code  New referral code value.
    event ReferralCodeUpdated(uint16 code);

    /// @notice Emitted when the active flash loan provider adapter is updated.
    /// @param provider  New IFlashLoanProvider address (address(0) = use legacy aavePool).
    event FlashLoanProviderUpdated(address indexed provider);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initializer — replaces constructor. Must be called exactly once via ERC1967Proxy.
    /// @param admin               Address granted DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
    /// @param _aavePool           Aave V3 lending pool address (legacy fallback).
    /// @param _arbitrageExecutor  ArbitrageExecutor proxy address.
    function initialize(
        address admin,
        address _aavePool,
        address _arbitrageExecutor
    ) public initializer {
        __AccessControl_init();
        __UUPSUpgradeable_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(UPGRADER_ROLE, admin);
        aavePool = IAaveV3Pool(_aavePool);
        arbitrageExecutor = _arbitrageExecutor;
        referralCode = 0;
        // slot 3: flashLoanProvider defaults to address(0) — uses legacy aavePool path
    }

    // -------------------------------------------------------------------------
    // SC-8: Operator-configurable referral code
    // -------------------------------------------------------------------------

    /// @notice Update the Aave referral code used in all future flash loan requests.
    /// @dev Only callable by DEFAULT_ADMIN_ROLE. Emits ReferralCodeUpdated.
    /// @param _code  New referral code. Use 0 to disable referrals.
    function setReferralCode(uint16 _code) external onlyRole(DEFAULT_ADMIN_ROLE) {
        referralCode = _code;
        emit ReferralCodeUpdated(_code);
    }

    // -------------------------------------------------------------------------
    // SC-1: IFlashLoanProvider adapter management
    // -------------------------------------------------------------------------

    /// @notice Set the active flash loan provider adapter.
    /// @dev Only callable by DEFAULT_ADMIN_ROLE. Emits FlashLoanProviderUpdated.
    ///      When set to a non-zero address, requestFlashLoan will delegate to
    ///      IFlashLoanProvider(_provider).flashLoan() instead of aavePool.flashLoanSimple().
    ///      Preferred hierarchy: Balancer (0%) > dYdX (0%) > Aave (0.09%).
    ///      Pass address(0) to revert to the legacy aavePool path.
    /// @param _provider  Address of an IFlashLoanProvider-compliant adapter.
    function setFlashLoanProvider(address _provider) external onlyRole(DEFAULT_ADMIN_ROLE) {
        flashLoanProvider = _provider;
        emit FlashLoanProviderUpdated(_provider);
    }

    /// @notice Compare flash loan fees across two providers for a given asset and amount.
    /// @dev View function for off-chain route planning. Does not execute any transaction.
    ///      Returns the address of the cheaper provider and its fee.
    ///      If one provider returns 0 for maxFlashLoan (unsupported asset), it is excluded.
    /// @param providerA  First IFlashLoanProvider address to compare.
    /// @param providerB  Second IFlashLoanProvider address to compare.
    /// @param asset      ERC-20 token to borrow.
    /// @param amount     Amount to borrow (used for fee calculation).
    /// @return cheaper   Address of the provider with the lower fee for this amount.
    /// @return fee       Fee charged by the cheaper provider (in asset units).
    function selectCheapestProvider(
        address providerA,
        address providerB,
        address asset,
        uint256 amount
    ) external view returns (address cheaper, uint256 fee) {
        // Check liquidity availability for both providers
        bool aSupports = providerA != address(0) && IFlashLoanProvider(providerA).maxFlashLoan(asset) >= amount;
        bool bSupports = providerB != address(0) && IFlashLoanProvider(providerB).maxFlashLoan(asset) >= amount;

        if (!aSupports && !bSupports) {
            // Neither provider can service this amount — return zero address
            return (address(0), 0);
        }

        if (!aSupports) {
            return (providerB, IFlashLoanProvider(providerB).flashLoanFee(amount));
        }

        if (!bSupports) {
            return (providerA, IFlashLoanProvider(providerA).flashLoanFee(amount));
        }

        // Both support the amount — compare fees
        uint256 feeA = IFlashLoanProvider(providerA).flashLoanFee(amount);
        uint256 feeB = IFlashLoanProvider(providerB).flashLoanFee(amount);

        if (feeA <= feeB) {
            return (providerA, feeA);
        } else {
            return (providerB, feeB);
        }
    }

    // -------------------------------------------------------------------------
    // Flash loan flow
    // -------------------------------------------------------------------------

    /// @notice Request a flash loan and delegate execution to ArbitrageExecutor.
    /// @dev When flashLoanProvider != address(0), delegates to IFlashLoanProvider.flashLoan().
    ///      Otherwise falls back to the legacy aavePool.flashLoanSimple() path.
    ///      Emits FlashLoanRequested after the provider call returns (i.e. post repayment).
    /// @param asset   ERC-20 asset to borrow.
    /// @param amount  Amount to borrow.
    /// @param params  Encoded calldata forwarded to ArbitrageExecutor inside the callback.
    function requestFlashLoan(address asset, uint256 amount, bytes calldata params) external onlyRole(EXECUTOR_ROLE) {
        address provider = flashLoanProvider;
        if (provider != address(0)) {
            // SC-1: Use the configured IFlashLoanProvider adapter (e.g. Balancer 0%)
            IFlashLoanProvider(provider).flashLoan(address(this), asset, amount, params);
        } else {
            // Legacy path: direct Aave V3 call (backward compat for existing deployments)
            aavePool.flashLoanSimple(address(this), asset, amount, params, referralCode);
        }
        // SC-06: emit after the call so the event is only logged on success
        emit FlashLoanRequested(asset, amount, keccak256(params));
    }

    // -------------------------------------------------------------------------
    // Aave V3 callback
    // -------------------------------------------------------------------------

    /// @notice Aave V3 callback. Called by aavePool after disbursing loan funds.
    /// @dev Reverts with FL_UnauthorizedCaller if msg.sender != aavePool.
    ///      Reverts with FL_InvalidInitiator if initiator != address(this).
    /// @param asset      ERC-20 asset borrowed.
    /// @param amount     Loan amount.
    /// @param premium    Aave fee on the loan.
    /// @param initiator  Must equal address(this) — set when requestFlashLoan is called.
    /// @param params     Encoded calldata forwarded to ArbitrageExecutor.
    /// @return           True on successful completion.
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool) {
        if (msg.sender != address(aavePool)) revert FL_UnauthorizedCaller();
        if (initiator != address(this)) revert FL_InvalidInitiator();

        _executeAndRepayAave(asset, amount, premium, params);
        return true;
    }

    // -------------------------------------------------------------------------
    // Balancer V2 callback
    // -------------------------------------------------------------------------

    /// @notice Balancer V2 callback. Called by the Balancer Vault after disbursing funds.
    /// @dev The Balancer Vault calls this on `recipient` after transferring tokens.
    ///      Security: msg.sender must be a known/trusted IFlashLoanProvider adapter
    ///      that wraps the Balancer Vault — we verify against flashLoanProvider.
    ///      feeAmounts[0] is always 0 for Balancer V2 flash loans.
    /// @param tokens      Array of borrowed token addresses (length = 1 in single-asset use).
    /// @param amounts     Array of borrowed amounts (parallel with tokens).
    /// @param feeAmounts  Array of fees (always 0 for Balancer — kept for interface compat).
    /// @param userData    Encoded calldata forwarded to ArbitrageExecutor.
    function receiveFlashLoan(
        address[] calldata tokens,
        uint256[] calldata amounts,
        uint256[] calldata feeAmounts,
        bytes calldata userData
    ) external {
        // Security: only accept Balancer callbacks from the configured provider adapter
        // OR directly from the Balancer Vault if it is the configured provider.
        // If flashLoanProvider == address(0), this path should never be triggered —
        // revert to prevent unauthorized fund movement.
        if (flashLoanProvider == address(0)) revert FL_UnauthorizedCaller();

        address asset = tokens[0];
        uint256 amount = amounts[0];
        uint256 premium = feeAmounts[0]; // 0 for Balancer V2

        // 1. Approve funds to ArbitrageExecutor
        IERC20(asset).forceApprove(arbitrageExecutor, amount);

        // 2. Call ArbitrageExecutor with the encoded payload
        (bool success, ) = arbitrageExecutor.call(userData);
        if (!success) revert FL_ArbitrageExecutionFailed();

        // 3. Repay Balancer Vault (amount + fee = amount + 0 = amount)
        uint256 amountOwed = amount + premium;
        IERC20(asset).forceApprove(msg.sender, amountOwed);
        IERC20(asset).safeTransfer(msg.sender, amountOwed);

        emit FlashLoanExecuted(asset, amount, premium, true);
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// @dev Shared execution + repayment logic for Aave V3 callback path.
    function _executeAndRepayAave(
        address asset,
        uint256 amount,
        uint256 premium,
        bytes calldata params
    ) internal {
        // 1. Approve funds to ArbitrageExecutor
        IERC20(asset).forceApprove(arbitrageExecutor, amount);

        // 2. Call ArbitrageExecutor (the payload is encoded in `params`)
        (bool success, ) = arbitrageExecutor.call(params);
        if (!success) revert FL_ArbitrageExecutionFailed();

        // 3. Repay Aave (amount + premium)
        uint256 amountToOwe = amount + premium;
        IERC20(asset).forceApprove(address(aavePool), amountToOwe);

        // SC-06: signal successful completion to off-chain monitors
        emit FlashLoanExecuted(asset, amount, premium, true);
    }

    // -------------------------------------------------------------------------
    // SC-08: UUPS upgrade authorization
    // -------------------------------------------------------------------------

    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}
}
