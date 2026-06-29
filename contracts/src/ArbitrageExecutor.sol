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
//   slot 0: approvedRouters    (mapping(address => bool))
//   slot 1: approvedTokens     (mapping(address => bool))
//   slot 2: allowanceManager   (IAllowanceManager — address, 20 bytes)  ← SC-5
//   slot 3: approvedSelectors  (mapping(address => mapping(bytes4 => bool)))  ← A5
//
// CRITICAL: When adding new state variables in V2, V3, etc., you MUST append
// them AFTER slot 3.  NEVER insert variables between existing ones — that
// would corrupt the storage layout and brick all proxies pointing at this impl.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/utils/ReentrancyGuardUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "./interfaces/IAllowanceManager.sol";

// =============================================================================
// SC-3: Custom errors (~200 gas saved per revert vs string require)
// =============================================================================

/// @dev Thrown when a zero address is passed where a non-zero one is required.
error ZeroAddress();
/// @dev Thrown when the caller does not hold EXECUTOR_ROLE.
error NotExecutor();
/// @dev Thrown when routers and payload arrays differ in length.
error LengthMismatch();
/// @dev Thrown when tokenIn (or tokenOut, when tokenOut != tokenIn) is not in the approved token set.
error TokenNotApproved(address token);
/// @dev Thrown when a router in the route is not in the approved router set.
error RouterNotApproved(address router);
/// @dev Thrown when the contract holds less than amountIn before execution.
error InsufficientBalance();
/// @dev Thrown when a low-level router call returns success=false.
error SwapFailed();
/// @dev Thrown when the route produces no gross profit (balanceAfter <= balanceBefore).
error ZeroGrossProfit();
/// @dev SC-13: thrown when the flash-funded pull does not credit EXACTLY amountIn
///      (e.g. a fee-on-transfer / rebasing tokenIn). The caller-funded round trip's
///      capital-retention guarantee assumes a faithful 1:1 pull, so such tokens are
///      rejected fail-closed instead of silently leaking the executor's own balance.
error FlashFundedPullMismatch();
/// @dev SC-13 (independent adversarial review): thrown when the flash-funded round trip
///      would NOT leave this contract holding exactly its pre-call working capital B —
///      i.e. the OUTBOUND return of (principal + profit) shorted the executor (e.g. an
///      outbound-lossy / rebasing tokenIn that skims the sender). Symmetric to
///      FlashFundedPullMismatch on the pull leg: makes the capital-retention identity
///      fail-closed on the RETURN leg too, so a compromised EXECUTOR_ROLE key can never
///      leak B even through an exotic (and explicitly unsupported) token.
error FlashFundedCapitalRetentionViolation();
/// @dev Thrown when profit < minProfit (slippage guard).
error InsufficientProfit();
/// @dev Thrown when the ETH balance is zero on withdrawETH.
error ZeroBalance();
/// @dev DEPRECATED (SC-12, 2026-06-28): no longer raised. The AllowanceManager
///      isApproved registry was removed from the spend-safety path (it conferred no
///      actual spend authority over this contract's balance). Declaration retained
///      for ABI/selector stability. Spend control is now the executor's own exact,
///      ephemeral per-router allowance (see executeArbitrage).
error RouterAllowanceNotGranted(address router, address token);
/// @dev Thrown when a router's calldata payload is shorter than 4 bytes (no selector).
///      SECURITY (A5): any call with fewer than 4 bytes cannot carry a valid selector and
///      is rejected before the whitelist check fires.
error AE_PayloadTooShort(address router);
/// @dev Thrown when the selector extracted from a payload is not in the per-router whitelist.
///      SECURITY (A5): closes the arbitrary-function-call surface on approved routers.
error AE_RouterSelectorNotApproved(address router, bytes4 selector);
/// @dev Thrown when a circular route ends holding LESS of the intermediate token (tokenOut)
///      than it started with — i.e. standing tokenOut inventory was drained. The net-profit
///      gate measures tokenIn only and is blind to a tokenOut loss; this guard is symmetric
///      to FlashFundedCapitalRetentionViolation (which protects tokenIn) and fail-closes any
///      net tokenOut drain a per-hop balanceOf-bounded allowance would otherwise permit.
error TokenOutRetentionViolation();

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
    // slot 2 — SC-5: AllowanceManager reference (retained for storage layout).
    /// @notice Optional AllowanceManager reference, kept for backward compatibility
    ///         and as an external approval REGISTRY only.
    ///         SC-12 (2026-06-28): this is NO LONGER a spend gate — executeArbitrage
    ///         does not consult it. Spend control is enforced by the executor's own
    ///         exact, ephemeral per-router allowance. Slot retained (append-only UUPS).
    IAllowanceManager public allowanceManager;
    // slot 3 — A5: per-router function-selector whitelist.
    /// @notice Per-router whitelist of allowed 4-byte function selectors.
    /// SECURITY (audit A5, 2026-05-10): without selector gating, an EXECUTOR_ROLE
    /// compromise could invoke arbitrary functions on approved routers
    /// (e.g. transferFrom, withdraw, setOwner). Whitelisting bytes4 selectors
    /// closes that attack surface.
    /// Fail-closed by design: a router with no selector entries will cause every
    /// executeArbitrage call to revert with AE_RouterSelectorNotApproved until the
    /// operator explicitly approves (router, selector) pairs.
    /// approvedSelectors[router][selector] = true iff the router may be called
    /// with that 4-byte function selector.
    mapping(address => mapping(bytes4 => bool)) public approvedSelectors;
    // APPEND new variables below this line in future upgrades. Never above.

    /// @notice Emitted when an arbitrage route completes successfully.
    /// @param routeHash  Unique identifier of the executed route (for indexers).
    /// @param tokenIn    Input/output token of the circular route.
    /// @param tokenOut   Intermediate token of the route. Equal to tokenIn for simple
    ///                   circular arb; distinct (and pre-validated via approvedTokens)
    ///                   for multi-hop routes (M8, audit 2026-05-10).
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

    /// @notice Emitted when the AllowanceManager integration address is updated.
    /// @param allowanceManager  New AllowanceManager address (address(0) = disabled).
    event AllowanceManagerUpdated(address indexed allowanceManager);

    /// @notice Emitted when a per-router selector approval is changed.
    /// @param router    Router address whose selector whitelist is being modified.
    /// @param selector  4-byte function selector being approved or revoked.
    /// @param status    True = approved, false = revoked.
    event RouterSelectorApproved(address indexed router, bytes4 indexed selector, bool status);

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
    ///      SPEND CONTROL (SC-12, 2026-06-28): before each router call the executor
    ///      grants that router an exact, ephemeral tokenIn allowance of `amountIn`
    ///      and resets it to zero immediately after — so an approved router can pull
    ///      at most `amountIn` of tokenIn PER HOP and no standing allowance survives.
    ///      This bounds spend per hop, NOT per route: an N-hop route authorizes up to
    ///      N*amountIn of gross tokenIn outflow. Route-level loss is bounded separately
    ///      by the ZeroGrossProfit/minProfit balance gate below — the route must end in
    ///      a net tokenIn gain >= minProfit or the entire tx reverts atomically.
    ///      The AllowanceManager registry is no longer consulted as a spend gate.
    ///
    ///      BREAKING CHANGE NOTE (SC-05, 2026-05-08):
    ///      tokenOut was added to this signature. Any external caller (e.g. relays-client)
    ///      must pass the intermediate token explicitly. Current paper-trade deploy has no
    ///      external callers — safe to change now. Wire this up in relays-client Sprint 4+.
    ///
    /// @param routeHash  Unique hash of the route for event indexing.
    /// @param tokenIn    Input token (also the output since this is a circular route).
    /// @param tokenOut   Intermediate token in the route.
    ///                   When tokenOut == tokenIn (simple circular arb) no extra approval is needed.
    ///                   When tokenOut != tokenIn the token must be in approvedTokens (M8, audit 2026-05-10).
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
        // Self-funded path: this contract must already hold >= amountIn of tokenIn
        // (operator-provisioned working capital), and any net profit accrues to (stays
        // in) this contract. Behaviour unchanged since SC-12; the gated route core is
        // shared with the flash-funded path via _runRoute.
        _runRoute(routeHash, tokenIn, tokenOut, amountIn, minProfit, routers, payload);
    }

    /// @notice Execute an arbitrage route funded by the CALLER for the duration of the
    ///         call (e.g. a flash loan), returning principal + profit to the caller.
    /// @dev SC-13 (flash-loan fund-handoff, 2026-06-28). Closes the gap where a flash-
    ///      loan provider wrapper (FlashLoanExecutor) approved this contract for the
    ///      borrowed amount but the funds were never pulled — so executeArbitrage saw a
    ///      zero balance, reverted InsufficientBalance, and flash-funded arbitrage could
    ///      not execute on any provider path.
    ///
    ///      Flow: pull `amountIn` of tokenIn from msg.sender (who MUST have approved at
    ///      least amountIn) -> run the same gated route as executeArbitrage -> transfer
    ///      `amountIn + profit` back to msg.sender so it can repay the loan and keep the
    ///      net. This contract forwards ONLY what the call added: the amount returned
    ///      equals (balanceAfter - preCallBalance) by construction, so this contract's own
    ///      working capital B (in tokenIn) is provably retained. Retention is enforced by
    ///      the net-profit gate (ZeroGrossProfit/minProfit) plus the exact `amountIn +
    ///      profit` return — NOT by the per-hop spend cap, which authorizes up to
    ///      N*amountIn of gross intra-route tokenIn outflow that is recovered-or-reverted.
    ///      A compromised EXECUTOR_ROLE key therefore cannot drain B here. The identity is
    ///      tokenIn-scoped: any OTHER token this contract happens to hold is protected only
    ///      by the approvedRouters/approvedSelectors allowlist and the absence of a standing
    ///      allowance, not by this math. The same A5 allowlist, pause, and reentrancy guard apply.
    ///
    ///      Caller requirements: msg.sender holds EXECUTOR_ROLE and has approved >=
    ///      amountIn of tokenIn to this contract (FlashLoanExecutor already does the
    ///      forceApprove). This contract is LOAN-AGNOSTIC — it never sees the flash-loan
    ///      premium; covering it is the CALLER's responsibility, so minProfit SHOULD be set
    ///      >= premium. If it is not, this contract still cannot lose money (B is retained);
    ///      the provider's own atomic repayment revert is the final backstop. Protection of
    ///      the return-leg transfer against pausable/blocklist tokens is EVM atomicity, not
    ///      the pull-side FlashFundedPullMismatch guard.
    /// @param routeHash  Opaque route identifier for off-chain correlation (event only).
    /// @param tokenIn    Input/output token of the circular route; pulled from msg.sender.
    /// @param tokenOut   Intermediate token (== tokenIn for simple circular arb).
    /// @param amountIn   Principal pulled from msg.sender at the start of the call.
    /// @param minProfit  Minimum acceptable net profit in tokenIn units.
    /// @param routers    Approved router addresses, one per swap step.
    /// @param payload    Encoded calldata for each swap step (length must equal routers).
    function executeArbitrageFlashFunded(
        bytes32 routeHash,
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minProfit,
        address[] calldata routers,
        bytes[] calldata payload
    ) external onlyExecutor whenNotPaused nonReentrant {
        // Pull the borrowed principal from the caller (it has approved exactly amountIn).
        // Fail-closed on fee-on-transfer / rebasing tokens: the capital-retention identity
        // (return == balanceAfter - preCallBalance) and the `amountIn + profit` return both
        // assume the pull credits EXACTLY amountIn. If the received amount differs, revert
        // rather than leak this contract's own balance to transfer fees or overstate profit.
        uint256 balBeforePull = IERC20(tokenIn).balanceOf(address(this));
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);
        if (IERC20(tokenIn).balanceOf(address(this)) != balBeforePull + amountIn) {
            revert FlashFundedPullMismatch();
        }

        uint256 profit = _runRoute(routeHash, tokenIn, tokenOut, amountIn, minProfit, routers, payload);

        // Return principal + profit to the caller. By construction this equals
        // balanceAfter - preCallBalance, so this contract's own working capital is never
        // forwarded — only this call's pulled principal and the route profit.
        IERC20(tokenIn).safeTransfer(msg.sender, amountIn + profit);

        // Defense-in-depth (independent SC-13 review): make the capital-retention identity
        // fail-closed on the OUTBOUND leg too. The pull is already guarded
        // (FlashFundedPullMismatch); here we assert that after returning principal + profit
        // this contract again holds EXACTLY its pre-call working capital B. For every
        // supported faithful 1:1 token this is a no-op (balance == balBeforePull by
        // construction). It only reverts the unsupported outbound-lossy / rebasing case —
        // where the return transfer shorted the executor — rather than silently leaking B.
        if (IERC20(tokenIn).balanceOf(address(this)) != balBeforePull) {
            revert FlashFundedCapitalRetentionViolation();
        }
    }

    /// @dev Shared, gated route core for both the self-funded (executeArbitrage) and
    ///      flash-funded (executeArbitrageFlashFunded) entrypoints. Assumes this contract
    ///      already holds >= amountIn of tokenIn (pre-provisioned, or just pulled by the
    ///      flash-funded wrapper). Enforces token approval, the SC-12 per-router spend
    ///      cap, the A5 router/selector allowlist, and the net-profit gate; returns the
    ///      realised profit in tokenIn units. Moves no funds in/out of this contract
    ///      beyond the per-hop router swaps — the caller decides custody of the profit.
    function _runRoute(
        bytes32 routeHash,
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minProfit,
        address[] calldata routers,
        bytes[] calldata payload
    ) internal returns (uint256 profit) {
        if (routers.length != payload.length) revert LengthMismatch();
        if (!approvedTokens[tokenIn]) revert TokenNotApproved(tokenIn);

        // M8 (audit 2026-05-10): validate intermediate token when it differs from tokenIn.
        // Circular arb (tokenOut == tokenIn) is approved by definition — no extra SLOAD needed.
        // Non-circular routes (tokenOut != tokenIn) must pass through the same approval registry.
        // Fail-closed: unapproved intermediate tokens revert before any state change.
        if (tokenOut != tokenIn && !approvedTokens[tokenOut]) revert TokenNotApproved(tokenOut);

        uint256 balanceBefore = IERC20(tokenIn).balanceOf(address(this));
        if (balanceBefore < amountIn) revert InsufficientBalance();

        // Snapshot the pre-route balance of the intermediate (tokenOut) token. Used for BOTH
        // (a) the per-hop DELTA CAP in _dispatchHop — intermediate legs approve only the
        // route-CREATED amount (currentBalance - tokenOutBefore), never the executor's
        // STANDING tokenOut inventory; and (b) the post-loop tokenOut RETENTION assertion
        // (defense-in-depth, symmetric to SC-13's tokenIn guard). For the single-token
        // circular case (tokenOut == tokenIn) the baseline is the principal balance, so a
        // later same-token hop is likewise bounded to route-created gains, never working
        // capital B. Both close a standing-inventory drain the tokenIn-only net-profit gate
        // below cannot see (it measures tokenIn only).
        uint256 tokenOutBefore = tokenOut == tokenIn ? balanceBefore : IERC20(tokenOut).balanceOf(address(this));

        // SC-12 (executor self-cap, 2026-06-28): on-chain spend control is enforced
        // by an exact, ephemeral per-router allowance below (forceApprove amountIn →
        // call → reset to 0). The AllowanceManager isApproved registry is deliberately
        // NOT consulted here: an allowance held by the AllowanceManager confers no
        // spend authority over THIS contract's balance, so gating on it was false
        // assurance. Router/selector allowlisting (approvedRouters / approvedSelectors)
        // is retained as defense-in-depth.
        // Each hop is validated + dispatched in its own frame (_dispatchHop) so this
        // function stays under the EVM stack limit. _dispatchHop enforces the router
        // allowlist + A5 selector gate and applies the per-hop bounded, ephemeral
        // allowance of the token actually SOLD at that hop.
        for (uint256 i = 0; i < routers.length;) {
            _dispatchHop(i, tokenIn, tokenOut, amountIn, tokenOutBefore, routers[i], payload[i]);
            unchecked { ++i; }
        }

        // tokenOut retention (defense-in-depth backstop): a circular tokenIn->tokenOut->tokenIn
        // route converts the intermediate fully back to tokenIn, so it MUST end holding no LESS
        // tokenOut than it started. The per-hop delta cap already makes this structurally true
        // (intermediate hops approve only route-created tokenOut, never standing inventory), so
        // on honest routes this is a no-op. It is retained as (a) the guarantee that bounds the
        // multi-hop "self-mint tokenOut to inflate a later hop's cap" vector to route-created
        // funds only — note tokenOutBefore is the STALE pre-loop baseline, not recomputed per
        // hop — and (b) the safety net should a future refactor ever weaken the delta cap. A net
        // decrease means standing tokenOut was drained -> fail-closed (the tokenIn-only profit
        // gate below is blind to a tokenOut loss).
        if (tokenOut != tokenIn && IERC20(tokenOut).balanceOf(address(this)) < tokenOutBefore) {
            revert TokenOutRetentionViolation();
        }

        uint256 balanceAfter = IERC20(tokenIn).balanceOf(address(this));
        if (balanceAfter <= balanceBefore) revert ZeroGrossProfit();

        profit = balanceAfter - balanceBefore;
        if (profit < minProfit) revert InsufficientProfit();

        // SC-05 fix: emit tokenOut (intermediate token) so indexers can identify the route.
        emit ArbitrageExecuted(routeHash, tokenIn, tokenOut, profit);
    }

    /// @dev Validate and dispatch a single route hop in its own stack frame (extracted
    ///      from _runRoute's loop to keep that function under the EVM stack limit).
    ///      Enforces the router allowlist + the A5 selector whitelist, then applies an
    ///      EXACT, ephemeral allowance of the token actually SOLD at this hop around the
    ///      swap call:
    ///        - hop 0 sells the principal (tokenIn), capped at `amountIn` — bounds spend
    ///          over this contract's tokenIn-denominated working capital;
    ///        - later hops sell the intermediate (tokenOut), capped at the route-CREATED
    ///          delta (current balance minus the pre-route baseline `tokenOutBefore`), so a
    ///          standing tokenOut inventory is NEVER granted to a router — closing a drain
    ///          surface the tokenIn-only net-profit gate cannot see;
    ///        - route-level loss is still bounded by the ZeroGrossProfit/minProfit gate
    ///          (tokenIn) plus the post-loop tokenOut retention assertion in _runRoute;
    ///        - no standing allowance survives (reset to 0 on success; the whole tx
    ///          reverts on failure, so nothing is left granted either way).
    ///      Without the per-hop sold token, a genuine 2-router arb's return leg (which
    ///      sells tokenOut) reverted SwapFailed because only tokenIn was ever approved.
    ///      forceApprove zeroes a non-zero current allowance first (USDT-safe). Supported
    ///      route shapes: 1-hop (tokenIn==tokenOut) and 2-token circular
    ///      tokenIn->tokenOut->tokenIn; a 3+ token route (tokenIn->X->Y->tokenIn) is
    ///      fail-safe — the offending hop approves the wrong token, the router pull reverts
    ///      SwapFailed, and the whole tx rolls back (a capability gap, not a leak).
    function _dispatchHop(
        uint256 hopIndex,
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 tokenOutBefore,
        address router,
        bytes calldata pld
    ) internal {
        if (!approvedRouters[router]) revert RouterNotApproved(router);

        // A5: selector whitelist gate. Require >= 4 bytes (a valid ABI selector), then
        // verify the extracted selector is approved for THIS router. Defense-in-depth
        // above EXECUTOR_ROLE compromise: even a compromised key cannot invoke
        // transferFrom/withdraw/setOwner on an approved router unless the operator has
        // explicitly whitelisted that selector.
        if (pld.length < 4) revert AE_PayloadTooShort(router);
        bytes4 selector;
        // Extract the leading 4 bytes without a memory allocation (gas-optimal).
        assembly {
            selector := calldataload(pld.offset)
        }
        if (!approvedSelectors[router][selector]) {
            revert AE_RouterSelectorNotApproved(router, selector);
        }

        // Approve the per-hop SOLD token only, bounded to the in-flight amount: hop 0 sells
        // the principal (tokenIn) capped at amountIn; later hops sell the intermediate
        // (tokenOut) capped at the route-CREATED delta (current balance minus tokenOutBefore),
        // so the executor's STANDING tokenOut inventory is never approved or exposed.
        address soldToken = hopIndex == 0 ? tokenIn : tokenOut;
        uint256 approveAmount = hopIndex == 0 ? amountIn : (IERC20(tokenOut).balanceOf(address(this)) - tokenOutBefore);
        IERC20(soldToken).forceApprove(router, approveAmount);
        (bool success, ) = router.call(pld);
        if (!success) revert SwapFailed();
        IERC20(soldToken).forceApprove(router, 0);
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

    // -------------------------------------------------------------------------
    // SC-5: AllowanceManager integration
    // -------------------------------------------------------------------------

    /// @notice Wire this executor to an AllowanceManager instance (external approval
    ///         REGISTRY only). Pass address(0) to clear it.
    /// @dev    SC-12 (2026-06-28): setting this NO LONGER affects executeArbitrage's
    ///         spend safety — the executor enforces its own exact, ephemeral per-router
    ///         allowance regardless of this value. Retained for backward compatibility
    ///         and off-chain registry/observability use; it does not custody or move
    ///         this contract's funds and is not a spend gate.
    /// @param _am  AllowanceManager proxy address (IAllowanceManager). Pass
    ///             address(0) to clear the integration.
    function setAllowanceManager(address _am) external onlyRole(ADMIN_ROLE) {
        allowanceManager = IAllowanceManager(_am);
        emit AllowanceManagerUpdated(_am);
    }

    // -------------------------------------------------------------------------
    // A5: Per-router function-selector whitelist administration
    // -------------------------------------------------------------------------

    /// @notice Approve or revoke a single function selector for a specific router.
    /// @dev    Only DEFAULT_ADMIN_ROLE (via timelock in production). Emits RouterSelectorApproved.
    ///         BREAKING POST-DEPLOY: any existing ArbitrageExecutor deployment will have an
    ///         empty approvedSelectors mapping, causing ALL executeArbitrage calls to revert
    ///         with AE_RouterSelectorNotApproved until the operator calls this function (or
    ///         batchSetRouterSelectorApproval) for each (router, selector) pair in active use.
    ///         See DEPLOY.md §A5 for the required post-deploy step.
    /// @param router    Router address whose selector whitelist is being modified.
    /// @param selector  4-byte function selector to approve or revoke.
    /// @param status    True to approve, false to revoke.
    function setRouterSelectorApproval(
        address router,
        bytes4 selector,
        bool status
    ) external onlyRole(DEFAULT_ADMIN_ROLE) {
        approvedSelectors[router][selector] = status;
        emit RouterSelectorApproved(router, selector, status);
    }

    /// @notice Batch-approve or batch-revoke multiple selectors for a single router in one tx.
    /// @dev    Typical use: approve swapExactTokensForTokens / exactInput / exactInputSingle
    ///         for UniV2/V3 in a single transaction instead of N separate admin calls.
    ///         Only DEFAULT_ADMIN_ROLE. Emits RouterSelectorApproved for each selector.
    /// @param router     Router address whose selector whitelist is being modified.
    /// @param selectors  Array of 4-byte function selectors to approve or revoke.
    /// @param status     True to approve all, false to revoke all.
    function batchSetRouterSelectorApproval(
        address router,
        bytes4[] calldata selectors,
        bool status
    ) external onlyRole(DEFAULT_ADMIN_ROLE) {
        uint256 len = selectors.length;
        for (uint256 i = 0; i < len;) {
            approvedSelectors[router][selectors[i]] = status;
            emit RouterSelectorApproved(router, selectors[i], status);
            unchecked { ++i; }
        }
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
