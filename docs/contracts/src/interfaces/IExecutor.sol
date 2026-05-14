// SPDX-License-Identifier: MIT
pragma solidity 0.8.19;

/// @title IExecutor
/// @notice Interface for the ArbitrageX-V2 (OMEGA) Executor core contract
/// @author ArbitrageX Team
/// @dev All functions validate inputs and emit events on critical state changes
interface IExecutor {

    // ============ Structs ============

    /// @notice Represents a single swap step in a multi-hop arbitrage resolution
    /// @param tokenIn Token to swap from
    /// @param tokenOut Token to swap to
    /// @param amountIn Amount of tokenIn to swap
    /// @param amountOutMin Minimum amount of tokenOut to receive
    /// @param dex Address of the DEX router/pool to execute through
    /// @param dexType Identifier for DEX version: 0=V2, 1=V3, 2=Curve, 3=Balancer
    /// @param feeOrPoolFee Fee tier for V3 (e.g., 500=0.05%) or pool identifier
    /// @param data Additional calldata for complex swaps (e.g., Curve routes)
    struct SwapStep {
        address tokenIn;
        address tokenOut;
        uint256 amountIn;
        uint256 amountOutMin;
        address dex;
        uint8   dexType;
        uint24  feeOrPoolFee;
        bytes   data;
    }

    /// @notice Parameters for initiating a flashloan
    /// @param provider Address of the flashloan provider (Aave V3, Balancer, Maker)
    /// @param token Token to flashloan
    /// @param amount Amount to flashloan
    /// @param fee Premium/fee expected for the flashloan
    /// @param data Encoded callback data for the provider
    struct FlashParams {
        address provider;
        address token;
        uint256 amount;
        uint256 fee;
        bytes   data;
    }

    /// @notice Complete resolution payload from the backend Rust searcher
    /// @param steps Ordered array of swap steps composing the arbitrage path
    /// @param flash Flashloan parameters if capital is needed
    /// @param minYield Minimum topological yield required (reverts if not met)
    /// @param deadline Block timestamp deadline for execution
    /// @param signature ECDSA signature from authorized Execution Signer
    /// @param nonce Unique nonce to prevent replay attacks
    struct Resolution {
        SwapStep[] steps;
        FlashParams flash;
        uint256 minYield;
        uint256 deadline;
        bytes   signature;
        uint256 nonce;
    }

    // ============ Events ============

    /// @notice Emitted when a resolution is successfully executed
    /// @param nonce Resolution nonce
    /// @param caller Address that triggered execution
    /// @param yieldCaptured Net profit captured in wei units
    /// @param gasUsed Gas consumed by the transaction
    /// @dev Gas cost documented for off-chain accounting
    event ResolutionExecuted(
        uint256 indexed nonce,
        address indexed caller,
        uint256 yieldCaptured,
        uint256 gasUsed
    );

    /// @notice Emitted when a resolution is reverted atomically
    /// @param nonce Resolution nonce
    /// @param caller Address that triggered execution
    /// @param reason Revert reason code:
    ///        0=Yield below minimum, 1=Signature invalid, 2=Deadline expired,
    ///        3=Swap failed, 4=Flashloan failed, 5=Post-resolution invariant failed
    event ResolutionReverted(
        uint256 indexed nonce,
        address indexed caller,
        uint8   reason
    );

    /// @notice Emitted when yield is captured and sent to treasury
    /// @param nonce Resolution nonce
    /// @param token Token address of captured yield
    /// @param amount Amount of yield captured
    /// @param treasury Recipient treasury address
    event YieldCaptured(
        uint256 indexed nonce,
        address indexed token,
        uint256 amount,
        address indexed treasury
    );

    /// @notice Emitted when the authorized signer is rotated
    /// @param oldSigner Previous signer address
    /// @param newSigner New signer address
    event SignerUpdated(address indexed oldSigner, address indexed newSigner);

    /// @notice Emitted when minimum yield threshold is updated
    /// @param oldMinYield Previous minimum yield
    /// @param newMinYield New minimum yield
    event MinYieldUpdated(uint256 oldMinYield, uint256 newMinYield);

    /// @notice Emitted when treasury address is updated
    /// @param oldTreasury Previous treasury address
    /// @param newTreasury New treasury address
    event TreasuryUpdated(address indexed oldTreasury, address indexed newTreasury);

    /// @notice Emitted when a DEX adapter is registered or updated
    /// @param dexType Numeric identifier for the DEX type
    /// @param adapter Address of the adapter contract
    event AdapterRegistered(uint8 indexed dexType, address indexed adapter);

    // ============ Errors ============

    /// @notice Resolution nonce already used (replay protection)
    error InvalidNonce(uint256 nonce);

    /// @notice Block timestamp exceeds resolution deadline
    error DeadlineExpired(uint256 deadline, uint256 currentTime);

    /// @notice Signature verification failed for Execution Signer
    error InvalidSignature();

    /// @notice Topological yield is zero or negative
    error InsufficientYield(uint256 actualYield, uint256 minYield);

    /// @notice Caller is not the authorized Execution Signer
    error UnauthorizedSigner();

    /// @notice Caller is not the contract owner
    error UnauthorizedOwner();

    /// @notice DEX adapter not registered for given dexType
    error AdapterNotRegistered(uint8 dexType);

    /// @notice Post-resolution invariant check failed
    error PostResolutionInvariantFailed();

    /// @notice Flashloan callback received from unauthorized provider
    error UnauthorizedFlashloanProvider();

    /// @notice Emergency withdrawal failed
    error EmergencyWithdrawFailed();

    // ============ Functions ============

    /// @notice Execute an arbitrage resolution atomically
    /// @param resolution Complete resolution payload signed by Execution Signer
    /// @return yieldCaptured Net yield captured after all costs
    /// @dev Gas cost: ~180k-350k depending on hop count
    /// @dev Reverts atomically if any step fails or yield <= 0
    function execute(Resolution calldata resolution)
        external
        returns (uint256 yieldCaptured);

    /// @notice Validate a resolution without executing it
    /// @param resolution Resolution payload to validate
    /// @return valid True if signature, nonce, and deadline are valid
    /// @return expectedYield Estimated yield if executed
    /// @dev Gas cost: ~25k (read-only, no state changes)
    function validate(Resolution calldata resolution)
        external
        view
        returns (bool valid, uint256 expectedYield);

    /// @notice Emergency withdrawal of stuck tokens to treasury
    /// @param token Token to withdraw (use address(0) for ETH)
    /// @dev Gas cost: ~35k
    /// @dev Only callable by contract owner
    function emergencyWithdraw(address token) external;

    /// @notice Returns true if nonce has been consumed
    function usedNonces(uint256 nonce) external view returns (bool);

    /// @notice Returns the authorized Execution Signer address
    function authorizedSigner() external view returns (address);

    /// @notice Returns the minimum yield threshold
    function minYield() external view returns (uint256);

    /// @notice Returns the treasury address for yield distribution
    function treasury() external view returns (address);

    /// @notice Returns the adapter address for a DEX type
    function adapters(uint8 dexType) external view returns (address);

    /// @notice Returns the contract owner
    function owner() external view returns (address);
}
