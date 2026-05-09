// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// STORAGE LAYOUT — APPEND-ONLY RULE (SC-08, 2026-05-08)
// =============================================================================
// Parent contracts (Initializable, AccessControlUpgradeable, UUPSUpgradeable)
// all use ERC-7201 namespaced slots — they do NOT occupy linear slot space.
//
// This contract has NO additional state variables of its own.
// If state variables are added in future upgrades, they start at slot 0
// and must only be appended — never inserted.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title AllowanceManager — UUPS-upgradeable centralized token allowance manager
/// @dev Módulo para centralizar y auditar las aprobaciones (Approve)
///      de tokens a diferentes Routers desde los contratos operativos.
///      Refactored to UUPS proxy pattern (SC-08).
contract AllowanceManager is
    Initializable,
    AccessControlUpgradeable,
    UUPSUpgradeable
{
    using SafeERC20 for IERC20;

    bytes32 public constant ADMIN_ROLE = DEFAULT_ADMIN_ROLE;
    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    /// @dev Separate UPGRADER_ROLE allows key rotation independent of admin.
    bytes32 public constant UPGRADER_ROLE = keccak256("UPGRADER_ROLE");

    event AllowanceGranted(address indexed token, address indexed spender, uint256 amount);
    event AllowanceRevoked(address indexed token, address indexed spender);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @dev Replaces constructor.  Must be called exactly once via ERC1967Proxy.
    /// @param admin Address granted DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
    function initialize(address admin) public initializer {
        __AccessControl_init();
        __UUPSUpgradeable_init();
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(UPGRADER_ROLE, admin);
    }

    /// @dev Otorga aprobación máxima a un router
    function grantAllowance(address token, address spender) external onlyRole(ADMIN_ROLE) {
        IERC20(token).forceApprove(spender, type(uint256).max);
        emit AllowanceGranted(token, spender, type(uint256).max);
    }

    /// @dev Revoca aprobación a 0 por seguridad
    function revokeAllowance(address token, address spender) external onlyRole(ADMIN_ROLE) {
        IERC20(token).forceApprove(spender, 0);
        emit AllowanceRevoked(token, spender);
    }

    // -------------------------------------------------------------------------
    // SC-08: UUPS upgrade authorization
    // -------------------------------------------------------------------------

    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}
}
