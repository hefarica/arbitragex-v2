// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title ArbitrageExecutor - Modulo 100% DeFi
/// @dev Ejecutor on-chain que revierte si no se cumple el Minimum Profit
contract ArbitrageExecutor is AccessControl, Pausable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    bytes32 public constant ADMIN_ROLE = DEFAULT_ADMIN_ROLE;

    mapping(address => bool) public approvedRouters;
    mapping(address => bool) public approvedTokens;

    event ArbitrageExecuted(bytes32 indexed routeHash, address tokenIn, address tokenOut, uint256 profit);
    event RouterApproved(address router, bool status);
    event TokenApproved(address token, bool status);
    event EmergencyWithdrawn(address token, uint256 amount);

    constructor() {
        _grantRole(ADMIN_ROLE, msg.sender);
    }

    modifier onlyExecutor() {
        require(hasRole(EXECUTOR_ROLE, msg.sender), "Not executor");
        _;
    }

    /// @dev Ejecución atómica de un multi-hop.
    /// @param routeHash Hash único de la ruta.
    /// @param tokenIn Token de entrada.
    /// @param minProfit Mínima ganancia esperada (Net ROI) en el tokenBase.
    /// @param routers Arreglo de direcciones de los routers DeFi a usar.
    /// @param payload Datos de llamada (calldata) encodificados para cada paso.
    function executeArbitrage(
        bytes32 routeHash,
        address tokenIn,
        uint256 amountIn,
        uint256 minProfit,
        address[] calldata routers,
        bytes[] calldata payload
    ) external onlyExecutor whenNotPaused nonReentrant {
        require(routers.length == payload.length, "Length mismatch");
        require(approvedTokens[tokenIn], "Token not approved");

        uint256 balanceBefore = IERC20(tokenIn).balanceOf(address(this));
        require(balanceBefore >= amountIn, "Insufficient initial balance");

        for (uint256 i = 0; i < routers.length; i++) {
            address router = routers[i];
            require(approvedRouters[router], "Router not approved");
            
            // Execute the swap (call)
            (bool success, ) = router.call(payload[i]);
            require(success, "Swap failed in route");
        }

        uint256 balanceAfter = IERC20(tokenIn).balanceOf(address(this));
        require(balanceAfter > balanceBefore, "Arbitrage did not generate gross profit");
        
        uint256 profit = balanceAfter - balanceBefore;
        require(profit >= minProfit, "Slippage / Min profit guard failed");

        emit ArbitrageExecuted(routeHash, tokenIn, tokenIn, profit);
    }

    function setRouterApproval(address router, bool status) external onlyRole(ADMIN_ROLE) {
        approvedRouters[router] = status;
        emit RouterApproved(router, status);
    }

    function setTokenApproval(address token, bool status) external onlyRole(ADMIN_ROLE) {
        approvedTokens[token] = status;
        emit TokenApproved(token, status);
    }

    function emergencyWithdraw(address token) external onlyRole(ADMIN_ROLE) {
        uint256 bal = IERC20(token).balanceOf(address(this));
        IERC20(token).safeTransfer(msg.sender, bal);
        emit EmergencyWithdrawn(token, bal);
    }

    function pause() external onlyRole(ADMIN_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(ADMIN_ROLE) {
        _unpause();
    }
}
