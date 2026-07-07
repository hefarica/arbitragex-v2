// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

// =============================================================================
// SC-2: BalancerVaultAdapter — IDEXAdapter for Balancer V2 Vault
//
// Supports Balancer V2 Vault swaps on any EVM chain:
//   - Ethereum: 0xBA12222222228d8Ba445958a75a0704d566BF2C8
//   - Arbitrum: 0xBA12222222228d8Ba445958a75a0704d566BF2C8
//   - Optimism: 0xBA12222222228d8Ba445958a75a0704d566BF2C8
//   - Polygon:  0xBA12222222228d8Ba445958a75a0704d566BF2C8
//   - Base:     0xBA12222222228d8Ba445958a75a0704d566BF2C8
//   - Gnosis:   0xBA12222222228d8Ba445958a75a0704d566BF2C8
//
// extraData encoding: abi.encode(bytes32 poolId, uint256 limit)
//   - poolId: Balancer pool identifier (32 bytes).
//             Found in Balancer's subgraph or pool detail page.
//   - limit:  Maximum amount willing to pay (for given-in swaps, limit = amountIn).
//
// Uses IVault.swap() with SwapKind.GIVEN_IN for single swaps.
// For batch swaps (multi-hop), use batchSwap() directly via a helper or compose
// multiple single swaps in the ArbitrageExecutor route.
//
// Vault address is injected via constructor — zero hardcoded addresses.
//
// Gas profile (optimizer_runs = 200):
//   - swap()     : ~110,000 gas (Balancer Vault single swap)
//   - quoteOut() : reverts — use Balancer SDK or Vault queryBatchSwap off-chain
// =============================================================================

import "../interfaces/IDEXAdapter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

// -------------------------------------------------------------------------
// Minimal Balancer V2 Vault interfaces
// -------------------------------------------------------------------------

/// @dev Balancer V2 Vault — single swap and batch swap entry points.
///      Canonical address is the same on all chains (deployed via CREATE3).
interface IVault {
    /// @notice Enum defining swap direction.
    enum SwapKind {
        GIVEN_IN,  // Exact input: give amountIn, receive computed amountOut
        GIVEN_OUT  // Exact output: give computed amountIn, receive amountOut
    }

    /// @notice Parameters for a single swap.
    /// @param poolId   The Balancer pool identifier.
    /// @param kind     GIVEN_IN or GIVEN_OUT.
    /// @param assetIn  Token entering the pool.
    /// @param assetOut Token leaving the pool.
    /// @param amount   Amount of assetIn (for GIVEN_IN) or assetOut (for GIVEN_OUT).
    /// @param userData Opaque data forwarded to the pool's onSwap hook. Usually empty.
    struct SingleSwap {
        bytes32 poolId;
        SwapKind kind;
        address assetIn;
        address assetOut;
        uint256 amount;
        bytes userData;
    }

    /// @notice Token sender/recipient and fund management flags.
    /// @param sender              Account from which tokens are taken.
    /// @param fromInternalBalance Use internal Balance for sender (false = ERC20 transfer).
    /// @param recipient           Account to which tokens are sent.
    /// @param toInternalBalance   Deposit to internal Balance for recipient (false = ERC20 transfer).
    struct FundManagement {
        address sender;
        bool fromInternalBalance;
        address payable recipient;
        bool toInternalBalance;
    }

    /// @notice Execute a single swap.
    /// @param singleSwap  Swap parameters (pool, direction, amounts).
    /// @param funds       Sender/recipient configuration.
    /// @param limit       Maximum amount willing to pay (for GIVEN_IN, min amountOut).
    ///                     Reverts if the actual amount exceeds this limit.
    /// @param deadline    Unix timestamp after which the transaction reverts.
    /// @return amountOut  The amount of tokens received (for GIVEN_IN) or spent (for GIVEN_OUT).
    function swap(
        SingleSwap memory singleSwap,
        FundManagement memory funds,
        uint256 limit,
        uint256 deadline
    ) external payable returns (uint256 amountOut);

    /// @notice Query a batch swap without executing it. Returns asset deltas.
    /// @param kind         GIVEN_IN or GIVEN_OUT for the entire batch.
    /// @param swaps        Array of swap steps defining the route.
    /// @param assets       Ordered array of all tokens involved in the swaps.
    /// @param funds        Sender/recipient configuration (used for query context).
    /// @return assetDeltas Array of token deltas (positive = received, negative = sent).
    function queryBatchSwap(
        SwapKind kind,
        BatchSwapStep[] memory swaps,
        address[] memory assets,
        FundManagement memory funds
    ) external view returns (int256[] memory assetDeltas);

    /// @notice Parameters for one step of a batch swap.
    /// @param poolId        The Balancer pool identifier.
    /// @param assetInIndex  Index in `assets` of the input token.
    /// @param assetOutIndex Index in `assets` of the output token.
    /// @param amount        Amount to swap in this step (0 = use output from previous step).
    /// @param userData      Opaque data forwarded to the pool. Usually empty.
    struct BatchSwapStep {
        bytes32 poolId;
        uint256 assetInIndex;
        uint256 assetOutIndex;
        uint256 amount;
        bytes userData;
    }

    /// @notice Execute a batch swap (multi-hop).
    /// @param kind          GIVEN_IN or GIVEN_OUT for the entire batch.
    /// @param swaps         Array of swap steps.
    /// @param assets        Ordered array of all tokens involved.
    /// @param funds         Sender/recipient configuration.
    /// @param limits        Max/min token deltas per asset (index-matched to `assets`).
    ///                      Positive = max to send, negative = min to receive.
    /// @param deadline      Unix timestamp after which the transaction reverts.
    /// @return assetDeltas  Actual token deltas per asset.
    function batchSwap(
        SwapKind kind,
        BatchSwapStep[] memory swaps,
        address[] memory assets,
        FundManagement memory funds,
        int256[] memory limits,
        uint256 deadline
    ) external payable returns (int256[] memory assetDeltas);
}

// -------------------------------------------------------------------------
// Errors (SC-3: custom errors ~200 gas saved per revert vs string require)
// -------------------------------------------------------------------------

/// @dev Thrown when constructor receives address(0) for the vault.
error BV_ZeroVault();
/// @dev Thrown when amountIn is zero.
error BV_ZeroAmountIn();
/// @dev Thrown when tokenIn equals tokenOut (no swap needed / invalid).
error BV_SameToken();
/// @dev Thrown when poolId is empty (bytes32(0)).
error BV_ZeroPoolId();
/// @dev Thrown when the swap output is below minAmountOut.
error BV_SlippageExceeded(uint256 amountOut, uint256 minAmountOut);

// =============================================================================
/// @title BalancerVaultAdapter — IDEXAdapter for Balancer V2 Vault
/// @notice Executes swaps through the Balancer V2 Vault using the swap() function.
///         Supports both single swaps and batch swaps for multi-hop routes.
/// @dev SC-2 (2026-05-15). Vault address injected via constructor. Zero hardcoded
///      addresses. Uses SwapKind.GIVEN_IN for all single swaps.
// =============================================================================

contract BalancerVaultAdapter is IDEXAdapter {
    using SafeERC20 for IERC20;

    // -------------------------------------------------------------------------
    // State
    // -------------------------------------------------------------------------

    /// @notice Balancer V2 Vault address (immutable post-deploy).
    IVault public immutable vault;

    // -------------------------------------------------------------------------
    // Constructor
    // -------------------------------------------------------------------------

    /// @param _vault  Balancer V2 Vault address. Must be a deployed contract
    ///                implementing IVault.
    ///                Examples per chain (for reference only — NOT hardcoded):
    ///                All chains: 0xBA12222222228d8Ba445958a75a0704d566BF2C8
    constructor(address _vault) {
        if (_vault == address(0)) revert BV_ZeroVault();
        vault = IVault(_vault);
    }

    // -------------------------------------------------------------------------
    // IDEXAdapter implementation — single swap
    // -------------------------------------------------------------------------

    /// @inheritdoc IDEXAdapter
    /// @dev extraData = abi.encode(bytes32 poolId, uint256 limit)
    ///      poolId: The Balancer pool identifier (32 bytes).
    ///      limit:  Maximum amountIn for GIVEN_IN swaps. Set to amountIn.
    ///
    ///      Flow: transferFrom caller → forceApprove vault → vault.swap()
    ///      → vault sends tokenOut directly to msg.sender.
    ///
    ///      Gas cost: ~110k gas at optimizer_runs=200.
    ///
    /// @param tokenIn      ERC-20 token to sell.
    /// @param tokenOut     ERC-20 token to receive.
    /// @param amountIn     Amount of tokenIn to spend. Must be > 0.
    /// @param minAmountOut Minimum acceptable output (slippage guard). Reverts if not met.
    /// @param extraData    abi.encode(bytes32 poolId, uint256 limit)
    /// @return amountOut   Actual output amount received.
    function swap(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut,
        bytes calldata extraData
    ) external override returns (uint256 amountOut) {
        if (amountIn == 0) revert BV_ZeroAmountIn();
        if (tokenIn == tokenOut) revert BV_SameToken();

        // Decode DEX-specific parameters
        (bytes32 poolId, uint256 limit) = abi.decode(extraData, (bytes32, uint256));
        if (poolId == bytes32(0)) revert BV_ZeroPoolId();

        // 1. Pull tokenIn from caller into this adapter
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);

        // 2. Approve the Balancer Vault to spend tokenIn
        IERC20(tokenIn).forceApprove(address(vault), amountIn);

        // 3. Execute swap via Balancer Vault
        //    SwapKind.GIVEN_IN = exact input, variable output
        //    sender = address(this) (adapter holds the tokens)
        //    recipient = msg.sender (caller receives output directly)
        amountOut = vault.swap(
            IVault.SingleSwap({
                poolId: poolId,
                kind: IVault.SwapKind.GIVEN_IN,
                assetIn: tokenIn,
                assetOut: tokenOut,
                amount: amountIn,
                userData: "" // opaque data — not needed for standard swaps
            }),
            IVault.FundManagement({
                sender: address(this),
                fromInternalBalance: false, // use ERC20, not internal Balance
                recipient: payable(msg.sender),
                toInternalBalance: false // send ERC20, not internal Balance
            }),
            limit, // max amount willing to pay (for GIVEN_IN = amountIn)
            block.timestamp // atomic — no expiry needed
        );

        // 4. Defense-in-depth slippage check
        if (amountOut < minAmountOut) revert BV_SlippageExceeded(amountOut, minAmountOut);
    }

    /// @inheritdoc IDEXAdapter
    /// @dev Balancer V2 Vault does not expose a view-only single-swap quote.
    ///      Off-chain callers should use queryBatchSwap() for previews:
    ///        queryBatchSwap(SwapKind.GIVEN_IN, swaps, assets, funds)
    ///      Or use the Balancer SDK for JavaScript/TypeScript integrations.
    ///      This function always reverts — use off-chain tools for quotes.
    ///
    ///      Invariant: this function never modifies state. It reverts immediately.
    function quoteOut(
        address, /*tokenIn*/
        address, /*tokenOut*/
        uint256, /*amountIn*/
        bytes calldata /*extraData*/
    ) external pure override returns (uint256) {
        revert("BalancerVaultAdapter: use queryBatchSwap off-chain for quotes");
    }

    // -------------------------------------------------------------------------
    // Extended API: batchSwap for multi-hop routes
    // -------------------------------------------------------------------------

    /// @notice Execute a multi-hop batch swap through Balancer V2 Vault.
    /// @dev This is NOT part of IDEXAdapter — it's an extended function for
    ///      advanced users who need multi-hop routes (e.g. WETH -> DAI -> USDC).
    ///      Uses SwapKind.GIVEN_IN: exact total input, variable total output.
    ///
    ///      Gas cost: ~150k-250k depending on number of hops.
    ///
    /// @param swaps         Array of swap steps. Each step references a pool.
    /// @param assets        Ordered array of all tokens involved in the batch.
    ///                      Must include every token referenced in swaps.
    /// @param limits        Max/min per-asset deltas. Index-matched to `assets`.
    ///                      Positive = max to send, negative = min to receive.
    /// @param deadline      Unix timestamp after which the transaction reverts.
    /// @return assetDeltas  Actual token deltas per asset.
    function batchSwap(
        IVault.BatchSwapStep[] calldata swaps,
        address[] calldata assets,
        int256[] calldata limits,
        uint256 deadline
    ) external returns (int256[] memory assetDeltas) {
        if (swaps.length == 0) revert BV_ZeroAmountIn();
        if (assets.length == 0) revert BV_ZeroAmountIn();
        if (limits.length != assets.length) revert BV_SameToken();

        // Approve the first input token from the caller
        // The vault pulls tokens from msg.sender via transient balance tracking
        address tokenIn = assets[0];
        uint256 totalIn = uint256(limits[0] > 0 ? limits[0] : -limits[0]);
        if (totalIn > 0) {
            IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), totalIn);
            IERC20(tokenIn).forceApprove(address(vault), totalIn);
        }

        assetDeltas = vault.batchSwap(
            IVault.SwapKind.GIVEN_IN,
            swaps,
            assets,
            IVault.FundManagement({
                sender: address(this),
                fromInternalBalance: false,
                recipient: payable(msg.sender),
                toInternalBalance: false
            }),
            limits,
            deadline
        );
    }
}
