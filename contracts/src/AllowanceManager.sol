// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// STORAGE LAYOUT - APPEND-ONLY RULE (SC-08, 2026-05-08)
// =============================================================================
// Parent contracts (Initializable, AccessControlUpgradeable, PausableUpgradeable,
// UUPSUpgradeable) all use ERC-7201 namespaced slots - they do NOT occupy linear
// slot space.
//
// This contract has NO additional state variables of its own.
// If state variables are added in future upgrades, they start at slot 0
// and must only be appended - never inserted.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "./interfaces/IAllowanceManager.sol";

// =============================================================================
// SC-3: Custom errors (~200 gas saved per revert vs string require)
// =============================================================================

/// @dev Thrown when a zero address is passed where a non-zero one is required.
error AM_ZeroAddress();
/// @dev Thrown when a zero amount is passed where a positive one is required.
error AM_ZeroAmount();
/// @dev Thrown when maxAmount exceeds MAX_SAFE_ALLOWANCE (SEC-2).
error AM_AboveSafeCap();
/// @dev Thrown when batch arrays differ in length.
error AM_LengthMismatch();

/// @title AllowanceManager - UUPS-upgradeable centralized token allowance manager
/// @notice Centralizes and audits ERC-20 approve() calls to DeFi routers from
///         operational contracts. Supports Pausable circuit-breaker and batch
///         grant/revoke operations (SC-4).
/// @dev Refactored to UUPS proxy pattern (SC-08).
///      SC-3 (2026-05-08): string require() replaced with custom errors.
///      SC-4 (2026-05-08): PausableUpgradeable + batchGrantAllowance + batchRevokeAllowance.
///      SC-6 (2026-05-08): Dead EXECUTOR_ROLE constant removed.
contract AllowanceManager is
    Initializable,
    AccessControlUpgradeable,
    PausableUpgradeable,
    UUPSUpgradeable,
    IAllowanceManager
{
    using SafeERC20 for IERC20;

    /// @notice Admin role - alias for DEFAULT_ADMIN_ROLE.
    bytes32 public constant ADMIN_ROLE = DEFAULT_ADMIN_ROLE;
    // SC-6: EXECUTOR_ROLE removed - it was declared but never granted or checked anywhere.
    /// @notice Separate UPGRADER_ROLE allows key rotation independent of admin.
    bytes32 public constant UPGRADER_ROLE = keccak256("UPGRADER_ROLE");

    /// @notice Hard cap on allowance amounts granted through this contract (SEC-2).
    /// @dev 1e30 (1 trillion units of an 18-decimal token) covers every realistic
    ///      operational need while preventing the "infinite approval = total drainable
    ///      balance" failure mode. Override only via upgrade if a future strategy needs it.
    uint256 public constant MAX_SAFE_ALLOWANCE = 1_000_000_000_000 * 1e18;

    /// @notice Emitted when an allowance is granted to a spender.
    /// @param token    ERC-20 token for which allowance was set.
    /// @param spender  Address receiving the allowance.
    /// @param amount   Allowance amount set (bounded by MAX_SAFE_ALLOWANCE).
    event AllowanceGranted(address indexed token, address indexed spender, uint256 amount);

    /// @notice Emitted when an allowance is revoked (set to 0).
    /// @param token    ERC-20 token for which allowance was revoked.
    /// @param spender  Address whose allowance was zeroed.
    event AllowanceRevoked(address indexed token, address indexed spender);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initializer - replaces constructor. Must be called exactly once via ERC1967Proxy.
    /// @param admin  Address granted DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
    function initialize(address admin) public initializer {
        __AccessControl_init();
        __Pausable_init();
        __UUPSUpgradeable_init();
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(UPGRADER_ROLE, admin);
    }

    // -------------------------------------------------------------------------
    // SC-4: Pausable circuit-breaker
    // -------------------------------------------------------------------------

    /// @notice Pause the contract. Blocks grant/revoke operations while paused.
    function pause() external onlyRole(ADMIN_ROLE) {
        _pause();
    }

    /// @notice Unpause the contract.
    function unpause() external onlyRole(ADMIN_ROLE) {
        _unpause();
    }

    // -------------------------------------------------------------------------
    // Core allowance management
    // -------------------------------------------------------------------------

    /// @notice Grant a bounded allowance from this contract to `spender` for `token`.
    /// @dev ABI BREAKING vs prior version: requires explicit `maxAmount` bounded by
    ///      MAX_SAFE_ALLOWANCE. Infinite approvals are no longer possible.
    ///      SC-4: reverts when paused.
    /// @param token      ERC-20 token to approve.
    /// @param spender    Address receiving the allowance (e.g. a DeFi router).
    /// @param maxAmount  Allowance amount. Must be > 0 and <= MAX_SAFE_ALLOWANCE.
    function grantAllowance(address token, address spender, uint256 maxAmount)
        external
        onlyRole(ADMIN_ROLE)
        whenNotPaused
    {
        if (token == address(0)) revert AM_ZeroAddress();
        if (spender == address(0)) revert AM_ZeroAddress();
        if (maxAmount == 0) revert AM_ZeroAmount();
        if (maxAmount > MAX_SAFE_ALLOWANCE) revert AM_AboveSafeCap();
        IERC20(token).forceApprove(spender, maxAmount);
        emit AllowanceGranted(token, spender, maxAmount);
    }

    /// @notice Revoke the allowance for `spender` on `token` by setting it to 0.
    /// @dev SC-4: reverts when paused.
    /// @param token    ERC-20 token to revoke.
    /// @param spender  Address whose allowance is zeroed.
    function revokeAllowance(address token, address spender) external onlyRole(ADMIN_ROLE) whenNotPaused {
        IERC20(token).forceApprove(spender, 0);
        emit AllowanceRevoked(token, spender);
    }

    // -------------------------------------------------------------------------
    // SC-4: Batch operations
    // -------------------------------------------------------------------------

    /// @notice Grant allowances to multiple (token, spender, amount) tuples atomically.
    /// @dev Reverts if any tuple violates invariants (ZeroAddress, ZeroAmount, AboveSafeCap).
    ///      All three arrays must have identical length; reverts with LengthMismatch otherwise.
    ///      SC-4: reverts when paused.
    /// @param tokens    ERC-20 token addresses, one per tuple.
    /// @param spenders  Spender addresses, one per tuple.
    /// @param amounts   Allowance amounts, one per tuple. Each must be > 0 and <= MAX_SAFE_ALLOWANCE.
    function batchGrantAllowance(address[] calldata tokens, address[] calldata spenders, uint256[] calldata amounts)
        external
        onlyRole(ADMIN_ROLE)
        whenNotPaused
    {
        if (tokens.length != spenders.length || tokens.length != amounts.length) revert AM_LengthMismatch();
        for (uint256 i = 0; i < tokens.length;) {
            if (tokens[i] == address(0)) revert AM_ZeroAddress();
            if (spenders[i] == address(0)) revert AM_ZeroAddress();
            if (amounts[i] == 0) revert AM_ZeroAmount();
            if (amounts[i] > MAX_SAFE_ALLOWANCE) revert AM_AboveSafeCap();
            IERC20(tokens[i]).forceApprove(spenders[i], amounts[i]);
            emit AllowanceGranted(tokens[i], spenders[i], amounts[i]);
            unchecked {
                ++i;
            }
        }
    }

    /// @notice Revoke allowances for multiple (token, spender) pairs atomically.
    /// @dev Both arrays must have identical length; reverts with LengthMismatch otherwise.
    ///      SC-4: reverts when paused.
    /// @param tokens    ERC-20 token addresses, one per pair.
    /// @param spenders  Spender addresses to revoke, one per pair.
    function batchRevokeAllowance(address[] calldata tokens, address[] calldata spenders)
        external
        onlyRole(ADMIN_ROLE)
        whenNotPaused
    {
        if (tokens.length != spenders.length) revert AM_LengthMismatch();
        for (uint256 i = 0; i < tokens.length;) {
            IERC20(tokens[i]).forceApprove(spenders[i], 0);
            emit AllowanceRevoked(tokens[i], spenders[i]);
            unchecked {
                ++i;
            }
        }
    }

    // -------------------------------------------------------------------------
    // SC-5: IAllowanceManager view functions
    // -------------------------------------------------------------------------

    /// @inheritdoc IAllowanceManager
    /// @dev Reads IERC20(token).allowance(address(this), spender) - no extra storage.
    function isApproved(address token, address spender) external view override returns (bool) {
        return IERC20(token).allowance(address(this), spender) > 0;
    }

    /// @inheritdoc IAllowanceManager
    /// @dev Reads IERC20(token).allowance(address(this), spender) - no extra storage.
    function getAllowance(address token, address spender) external view override returns (uint256) {
        return IERC20(token).allowance(address(this), spender);
    }

    // -------------------------------------------------------------------------
    // SC-08: UUPS upgrade authorization
    // -------------------------------------------------------------------------

    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}
}
