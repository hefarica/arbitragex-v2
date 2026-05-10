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

// =============================================================================
// SC-3: Custom errors (~200 gas saved per revert vs string require)
// =============================================================================

/// @dev Thrown when a zero address is passed where a non-zero one is required.
error ZeroAddress();
/// @dev Thrown when the caller does not hold EXECUTOR_ROLE.
error NotExecutor();
/// @dev Thrown when routers and payload arrays differ in length.
error LengthMismatch();
/// @dev Thrown when tokenIn is not in the approved token set.
error TokenNotApproved(address token);
/// @dev Thrown when a router in the route is not in the approved router set.
error RouterNotApproved(address router);
/// @dev Thrown when the contract holds less than amountIn before execution.
error InsufficientBalance();
/// @dev Thrown when a low-level router call returns success=false.
error SwapFailed();
/// @dev Thrown when the route produces no gross profit (balanceAfter <= balanceBefore).
error ZeroGrossProfit();
/// @dev Thrown when profit < minProfit (slippage guard).
error InsufficientProfit();
/// @dev Thrown when the ETH balance is zero on withdrawETH.
error ZeroBalance();

/// @title ArbitrageExecutor — UUPS-upgradeable on-chain arbitrage executor
/// @notice Executes atomic multi-hop circular arbitrage routes.
///         Atomic invariant: flash loan → sequential swaps → repay → profit.
///         Reverts entirely if profit < minProfit (RULE §19).
/// @dev Refactored to UUPS proxy pattern (SC-08). All constructor logic
///      moved to initialize(). Use ERC1967Proxy in deployment scripts.
///      SC-3 (2026-05-08): string require() replaced with custom errors.
contract ArbitrageExecutor is
    Initializable,
    AccessControlUpgradeable,
    PausableUpgradeable,
    ReentrancyGuardUpgradeable,
    UUPSUpgradeable
{
    using SafeERC20 for IERC20;

    /// @notice Role required to call executeArbitrage.
    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    /// @notice Admin role — alias for DEFAULT_ADMIN_ROLE.
    bytes32 public constant ADMIN_ROLE = DEFAULT_ADMIN_ROLE;
    /// @notice Separate UPGRADER_ROLE allows key rotation independent of admin.
    ///         In production: admin key can be rotated without losing upgrade rights,
    ///         and vice-versa. Both roles default to the deployer's admin address.
    bytes32 public constant UPGRADER_ROLE = keccak256("UPGRADER_ROLE");

    // slot 0
    /// @notice Set of DeFi router addresses approved to be called during a route.
    /// @dev Only routers in this mapping can be invoked by executeArbitrage.
    mapping(address => bool) public approvedRouters;
    // slot 1
    /// @notice Set of ERC-20 tokens approved as tokenIn for arbitrage routes.
    mapping(address => bool) public approvedTokens;
    // APPEND new variables below this line in future upgrades. Never above.

    /// @notice Emitted when an arbitrage route completes successfully.
    /// @param routeHash  Unique identifier of the executed route (for indexers).
    /// @param tokenIn    Input/output token of the circular route.
    /// @param tokenOut   Intermediate observability token (used only for event indexing).
    /// @param profit     Net profit in tokenIn units.
    event ArbitrageExecuted(bytes32 indexed routeHash, address tokenIn, address tokenOut, uint256 profit);

    /// @notice Emitted when a router's approval status changes.
    event RouterApproved(address router, bool status);

    /// @notice Emitted when a token's approval status changes.
    event TokenApproved(address token, bool status);

    /// @notice Emitted when an ERC-20 token is emergency-withdrawn to the caller.
    event EmergencyWithdrawn(address token, uint256 amount);

    /// @notice Emitted when ETH is rescued from the contract.
    /// @param to     Recipient of the rescued ETH.
    /// @param amount Amount of ETH transferred.
    event ETHWithdrawn(address indexed to, uint256 amount);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initializer — replaces constructor. Must be called exactly once via ERC1967Proxy.
    /// @param admin Address granted DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
    function initialize(address admin) public initializer {
        __AccessControl_init();
        __Pausable_init();
        __ReentrancyGuard_init();
        __UUPSUpgradeable_init();
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(UPGRADER_ROLE, admin);
    }

    /// @dev Reverts with NotExecutor if msg.sender does not hold EXECUTOR_ROLE.
    modifier onlyExecutor() {
        if (!hasRole(EXECUTOR_ROLE, msg.sender)) revert NotExecutor();
        _;
    }

    /// @notice Execute a circular arbitrage route atomically.
    /// @dev Measures tokenIn balance before and after all swaps. Reverts if:
    ///      - routers.length != payload.length (LengthMismatch)
    ///      - tokenIn not approved (TokenNotApproved)
    ///      - initial balance < amountIn (InsufficientBalance)
    ///      - any router not approved (RouterNotApproved)
    ///      - any low-level router call fails (SwapFailed)
    ///      - balanceAfter <= balanceBefore (ZeroGrossProfit)
    ///      - profit < minProfit (InsufficientProfit)
    ///
    ///      BREAKING CHANGE NOTE (SC-05, 2026-05-08):
    ///      tokenOut was added to this signature. Any external caller (e.g. relays-client)
    ///      must pass the intermediate token explicitly. Current paper-trade deploy has no
    ///      external callers — safe to change now. Wire this up in relays-client Sprint 4+.
    ///
    /// @param routeHash  Unique hash of the route for event indexing.
    /// @param tokenIn    Input token (also the output since this is a circular route).
    /// @param tokenOut   Intermediate observability token (not validated; used only for event emission).
    /// @param amountIn   Amount of tokenIn the contract must hold at the start of execution.
    /// @param minProfit  Minimum acceptable net profit in tokenIn units (slippage guard).
    /// @param routers    Approved router addresses, one per swap step.
    /// @param payload    Encoded calldata for each swap step (length must equal routers).
    function executeArbitrage(
        bytes32 routeHash,
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minProfit,
        address[] calldata routers,
        bytes[] calldata payload
    ) external onlyExecutor whenNotPaused nonReentrant {
        if (routers.length != payload.length) revert LengthMismatch();
        if (!approvedTokens[tokenIn]) revert TokenNotApproved(tokenIn);

        uint256 balanceBefore = IERC20(tokenIn).balanceOf(address(this));
        if (balanceBefore < amountIn) revert InsufficientBalance();

        for (uint256 i = 0; i < routers.length;) {
            address router = routers[i];
            if (!approvedRouters[router]) revert RouterNotApproved(router);

            (bool success, ) = router.call(payload[i]);
            if (!success) revert SwapFailed();

            unchecked { ++i; }
        }

        uint256 balanceAfter = IERC20(tokenIn).balanceOf(address(this));
        if (balanceAfter <= balanceBefore) revert ZeroGrossProfit();

        uint256 profit = balanceAfter - balanceBefore;
        if (profit < minProfit) revert InsufficientProfit();

        // SC-05 fix: emit tokenOut (intermediate token) so indexers can identify the route.
        emit ArbitrageExecuted(routeHash, tokenIn, tokenOut, profit);
    }

    /// @notice Approve or revoke a router address for use in routes.
    /// @param router  Router address to configure.
    /// @param status  True to approve, false to revoke.
    function setRouterApproval(address router, bool status) external onlyRole(ADMIN_ROLE) {
        approvedRouters[router] = status;
        emit RouterApproved(router, status);
    }

    /// @notice Approve or revoke a token address as a valid tokenIn.
    /// @param token   Token address to configure.
    /// @param status  True to approve, false to revoke.
    function setTokenApproval(address token, bool status) external onlyRole(ADMIN_ROLE) {
        approvedTokens[token] = status;
        emit TokenApproved(token, status);
    }

    /// @notice Emergency-withdraw the entire balance of an ERC-20 token to the caller.
    /// @param token  ERC-20 token to withdraw.
    function emergencyWithdraw(address token) external onlyRole(ADMIN_ROLE) {
        uint256 bal = IERC20(token).balanceOf(address(this));
        IERC20(token).safeTransfer(msg.sender, bal);
        emit EmergencyWithdrawn(token, bal);
    }

    /// @notice Pause the contract. Blocks executeArbitrage while paused.
    function pause() external onlyRole(ADMIN_ROLE) {
        _pause();
    }

    /// @notice Unpause the contract.
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

    /// @notice Rescue any ETH that ended up in the contract.
    /// @dev Only callable by ADMIN_ROLE. Transfers entire balance to `to`.
    /// @param to  Non-zero recipient address.
    function withdrawETH(address payable to) external onlyRole(ADMIN_ROLE) {
        if (to == address(0)) revert ZeroAddress();
        uint256 bal = address(this).balance;
        if (bal == 0) revert ZeroBalance();
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
