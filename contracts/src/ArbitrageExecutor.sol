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
    event ETHWithdrawn(address indexed to, uint256 amount);

    constructor() {
        _grantRole(ADMIN_ROLE, msg.sender);
    }

    modifier onlyExecutor() {
        require(hasRole(EXECUTOR_ROLE, msg.sender), "Not executor");
        _;
    }

    /// @dev Ejecución atómica de un multi-hop (circular arbitrage).
    /// @param routeHash Hash único de la ruta.
    /// @param tokenIn  Token base (start and end of the circular route).
    /// @param tokenOut Intermediate token traversed in the route — used ONLY for
    ///                 observability (event indexing by external dashboards / recon).
    ///                 Profit validation is always measured on `tokenIn` balance delta.
    ///
    ///     BREAKING CHANGE NOTE (SC-05, 2026-05-08):
    ///     tokenOut was added to this signature. Any external caller (e.g. relays-client)
    ///     must pass the intermediate token explicitly. Current paper-trade deploy has no
    ///     external callers — safe to change now. Wire this up in relays-client Sprint 4+.
    ///
    /// @param amountIn  Amount of tokenIn funded at start of route.
    /// @param minProfit Minimum net profit required in tokenIn units.
    /// @param routers   DeFi router addresses for each hop.
    /// @param payload   Encoded calldata for each hop.
    function executeArbitrage(
        bytes32 routeHash,
        address tokenIn,
        address tokenOut,
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

        // SC-05 fix: emit tokenOut (intermediate token) so indexers can identify the route.
        emit ArbitrageExecuted(routeHash, tokenIn, tokenOut, profit);
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

    // -------------------------------------------------------------------------
    // SC-07: ETH rescue
    // -------------------------------------------------------------------------

    /// @dev Accept ETH transfers (e.g. from selfdestruct, forced send, or future
    ///      WETH-unwrap flows). Without this the contract silently rejects ETH and
    ///      funds become permanently inaccessible.
    receive() external payable {}

    /// @dev Rescue any ETH that ended up in the contract.
    ///      Only callable by ADMIN_ROLE. Transfers entire balance to `to`.
    /// @param to Recipient address (must be non-zero).
    function withdrawETH(address payable to) external onlyRole(ADMIN_ROLE) {
        require(to != address(0), "Zero address");
        uint256 bal = address(this).balance;
        require(bal > 0, "No ETH to withdraw");
        (bool ok, ) = to.call{value: bal}("");
        require(ok, "ETH transfer failed");
        emit ETHWithdrawn(to, bal);
    }
}
