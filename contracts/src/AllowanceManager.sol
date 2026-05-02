// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title AllowanceManager
/// @dev Módulo para centralizar y auditar las aprobaciones (Approve) 
/// de tokens a diferentes Routers desde los contratos operativos.
contract AllowanceManager is AccessControl {
    using SafeERC20 for IERC20;

    bytes32 public constant ADMIN_ROLE = DEFAULT_ADMIN_ROLE;
    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");

    event AllowanceGranted(address indexed token, address indexed spender, uint256 amount);
    event AllowanceRevoked(address indexed token, address indexed spender);

    constructor() {
        _grantRole(ADMIN_ROLE, msg.sender);
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
}
