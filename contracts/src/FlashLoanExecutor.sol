// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// STORAGE LAYOUT — APPEND-ONLY RULE (SC-08, 2026-05-08)
// =============================================================================
// Parent contracts (Initializable, AccessControlUpgradeable, UUPSUpgradeable)
// all use ERC-7201 namespaced slots — they do NOT occupy linear slot space.
//
// This contract's OWN variables start at linear slot 0:
//   slot 0: aavePool            (IAaveV3Pool — address, 20 bytes, packed in slot)
//   slot 1: arbitrageExecutor   (address, 20 bytes)
//   slot 2: referralCode        (uint16, 2 bytes — packed with next var when added)
//
// CRITICAL: When adding new state variables in V2, V3, etc., you MUST append
// them AFTER slot 2.  NEVER insert variables between existing ones — that
// would corrupt the storage layout and brick all proxies pointing at this impl.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

// =============================================================================
// SC-3: Custom errors (~200 gas saved per revert vs string require)
// =============================================================================

/// @dev Thrown when executeOperation is called by an address that is not aavePool.
error FL_UnauthorizedCaller();
/// @dev Thrown when executeOperation is called with an initiator that is not this contract.
error FL_InvalidInitiator();
/// @dev Thrown when the call to ArbitrageExecutor inside executeOperation returns success=false.
error FL_ArbitrageExecutionFailed();

interface IAaveV3Pool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

/// @title FlashLoanExecutor — UUPS-upgradeable Aave V3 flash loan wrapper
/// @notice Requests Aave V3 flash loans and delegates execution to ArbitrageExecutor.
/// @dev Refactored to UUPS proxy pattern (SC-08). aavePool and arbitrageExecutor
///      moved from constructor params to initialize() params.
///      SC-3 (2026-05-08): string require() replaced with custom errors.
///      SC-8 (2026-05-08): referralCode is now operator-configurable via setReferralCode().
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

    // slot 0
    /// @notice Aave V3 lending pool address that will call executeOperation.
    IAaveV3Pool public aavePool;
    // slot 1
    /// @notice ArbitrageExecutor proxy address that receives the delegated execution call.
    address public arbitrageExecutor;
    // slot 2 — SC-8: operator-configurable referral code for Aave flash loans.
    /// @notice Aave referral code passed to flashLoanSimple. Defaults to 0.
    ///         Set via setReferralCode() to enable referral rewards if applicable.
    uint16 public referralCode;
    // APPEND new variables below this line in future upgrades. Never above.

    // SC-06: observability events for off-chain monitoring (recon, dashboard)

    /// @notice Emitted when a flash loan is requested to the Aave pool.
    /// @dev Fired after aavePool.flashLoanSimple() returns without reverting.
    /// @param asset       ERC-20 asset borrowed.
    /// @param amount      Loan amount.
    /// @param paramsHash  keccak256 of the params bytes (for off-chain correlation).
    event FlashLoanRequested(address indexed asset, uint256 amount, bytes32 paramsHash);

    /// @notice Emitted when the Aave callback completes and the loan is fully repaid.
    /// @param asset    ERC-20 asset borrowed.
    /// @param amount   Loan amount.
    /// @param premium  Fee charged by Aave.
    /// @param success  Always true (false paths revert before reaching this point).
    event FlashLoanExecuted(address indexed asset, uint256 amount, uint256 premium, bool success);

    /// @notice Emitted when the referral code is updated by an admin.
    /// @param code  New referral code value.
    event ReferralCodeUpdated(uint16 code);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initializer — replaces constructor. Must be called exactly once via ERC1967Proxy.
    /// @param admin               Address granted DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
    /// @param _aavePool           Aave V3 lending pool address.
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
        referralCode = 0; // SC-8: explicit default; operator can change via setReferralCode
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
    // Flash loan flow
    // -------------------------------------------------------------------------

    /// @notice Request a flash loan from Aave V3 and delegate execution to ArbitrageExecutor.
    /// @dev Emits FlashLoanRequested after the pool call returns (i.e. after full repayment).
    ///      Uses the current `referralCode` storage value (SC-8).
    /// @param asset   ERC-20 asset to borrow.
    /// @param amount  Amount to borrow.
    /// @param params  Encoded calldata forwarded to ArbitrageExecutor inside executeOperation.
    function requestFlashLoan(address asset, uint256 amount, bytes calldata params) external onlyRole(EXECUTOR_ROLE) {
        aavePool.flashLoanSimple(address(this), asset, amount, params, referralCode);
        // SC-06: emit after the call so the event is only logged when the pool
        // accepted the request without reverting.
        emit FlashLoanRequested(asset, amount, keccak256(params));
    }

    /// @notice Aave V3 callback. Called by aavePool after disbursing loan funds.
    /// @dev Reverts with FL_UnauthorizedCaller if msg.sender != aavePool.
    ///      Reverts with FL_InvalidInitiator if initiator != address(this).
    ///      Approves ArbitrageExecutor for `amount`, calls it with `params`,
    ///      then approves aavePool for `amount + premium` (repayment).
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

        // 1. Approve funds to ArbitrageExecutor
        IERC20(asset).forceApprove(arbitrageExecutor, amount);

        // 2. Call ArbitrageExecutor (the payload is encoded in `params`)
        (bool success, ) = arbitrageExecutor.call(params);
        if (!success) revert FL_ArbitrageExecutionFailed();

        // 3. Repay Aave (amount + premium)
        uint256 amountToOwe = amount + premium;
        IERC20(asset).forceApprove(address(aavePool), amountToOwe);

        // SC-06: signal successful completion to off-chain monitors before returning
        emit FlashLoanExecuted(asset, amount, premium, true);
        return true;
    }

    // -------------------------------------------------------------------------
    // SC-08: UUPS upgrade authorization
    // -------------------------------------------------------------------------

    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}
}
