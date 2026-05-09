// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// STORAGE LAYOUT — APPEND-ONLY RULE (SC-08, 2026-05-08)
// =============================================================================
// UUPS preserves storage across upgrades by delegatecall into a new
// implementation that reads the proxy's storage slots.  The parent contracts
// (Initializable, AccessControlUpgradeable, PausableUpgradeable,
// ReentrancyGuardUpgradeable, UUPSUpgradeable) all use ERC-7201 namespaced
// storage slots — they do NOT occupy the linear slot space [0..N].
//
// This contract's OWN variables start at linear slot 0:
//   slot 0: approvedRouters  (mapping(address => bool))
//   slot 1: approvedTokens   (mapping(address => bool))
//
// CRITICAL: When adding new state variables in V2, V3, etc., you MUST append
// them AFTER slot 1.  NEVER insert variables between existing ones — that
// would corrupt the storage layout and brick all proxies pointing at this impl.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/utils/ReentrancyGuardUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title ArbitrageExecutor — UUPS-upgradeable on-chain arbitrage executor
/// @dev Refactored to UUPS proxy pattern (SC-08).  All constructor logic
///      moved to initialize().  Use ERC1967Proxy in deployment scripts.
///
///      Atomic invariant: flash loan -> sequential swaps -> repay -> profit.
///      executeArbitrage() reverts entirely if profit < minProfit (RULE §19).
contract ArbitrageExecutor is
    Initializable,
    AccessControlUpgradeable,
    PausableUpgradeable,
    ReentrancyGuardUpgradeable,
    UUPSUpgradeable
{
    using SafeERC20 for IERC20;

    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    bytes32 public constant ADMIN_ROLE = DEFAULT_ADMIN_ROLE;
    /// @dev Separate UPGRADER_ROLE allows key rotation independent of admin.
    ///      In production: admin key can be rotated without losing upgrade rights,
    ///      and vice-versa.  Both roles default to the deployer's admin address.
    bytes32 public constant UPGRADER_ROLE = keccak256("UPGRADER_ROLE");

    // slot 0
    mapping(address => bool) public approvedRouters;
    // slot 1
    mapping(address => bool) public approvedTokens;
    // APPEND new variables below this line in future upgrades. Never above.

    event ArbitrageExecuted(bytes32 indexed routeHash, address tokenIn, address tokenOut, uint256 profit);
    event RouterApproved(address router, bool status);
    event TokenApproved(address token, bool status);
    event EmergencyWithdrawn(address token, uint256 amount);
    event ETHWithdrawn(address indexed to, uint256 amount);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @dev Replaces constructor.  Must be called exactly once via ERC1967Proxy.
    /// @param admin Address granted DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
    function initialize(address admin) public initializer {
        __AccessControl_init();
        __Pausable_init();
        __ReentrancyGuard_init();
        __UUPSUpgradeable_init();
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(UPGRADER_ROLE, admin);
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

    // -------------------------------------------------------------------------
    // SC-08: UUPS upgrade authorization
    // -------------------------------------------------------------------------

    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    ///      Called internally by upgradeToAndCall() before applying the upgrade.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}
}
