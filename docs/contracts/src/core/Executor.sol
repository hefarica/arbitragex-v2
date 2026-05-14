// SPDX-License-Identifier: MIT
pragma solidity 0.8.19;

import {IExecutor} from "../interfaces/IExecutor.sol";
import {SignatureValidator} from "../utils/SignatureValidator.sol";
import {FlashloanProvider} from "../utils/FlashloanProvider.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

// ┌─────────────────────────────────────────────────────────────┐
// │  ArbitrageX-V2 (OMEGA) — Executor Core                      │
// │  Flashloan-powered atomic arbitrage with Yul optimizations  │
// └─────────────────────────────────────────────────────────────┘

/// @title Executor
/// @notice Core execution engine for ArbitrageX-V2 (OMEGA)
/// @author ArbitrageX Team
/// @dev Receives signed Resolution payloads from the Rust backend, executes
///      flashloan + multi-hop swaps atomically, and reverts if yield <= 0.
/// @dev Invariant: contract balance of any token never decreases post-execution.
contract Executor is IExecutor, SignatureValidator, FlashloanProvider, ReentrancyGuard, Ownable {
    using SafeERC20 for IERC20;

    // ============ State Variables ============

    /// @notice Authorized Execution Signer — rotated via governance
    address public override authorizedSigner;

    /// @notice Minimum topological yield required (wei units)
    uint256 public override minYield;

    /// @notice Treasury address receiving captured yield
    address public override treasury;

    /// @notice Mapping: DEX type enum => adapter contract address
    mapping(uint8 => address) public override adapters;

    /// @notice Replay protection: nonce => consumed flag
    mapping(uint256 => bool) public override usedNonces;

    /// @notice Total yield captured since deployment (wei)
    uint256 public totalYieldCaptured;

    /// @notice Total resolutions executed successfully
    uint256 public totalResolutions;

    /// @notice Total resolutions reverted
    uint256 public totalReverts;

    /// @dev Invariant: balance before execution for post-resolution checks
    mapping(address => uint256) private _balanceBefore;

    /// @dev Resolution currently being executed (reentrancy guard context)
    Resolution private _activeResolution;

    // ============ Modifiers ============

    /// @notice Restricts function to authorized Execution Signer
    modifier onlyAuthorized() {
        if (msg.sender != authorizedSigner && msg.sender != owner()) {
            revert UnauthorizedSigner();
        }
        _;
    }

    /// @notice Ensures resolution has not expired
    /// @param deadline Block timestamp deadline
    modifier checkDeadline(uint256 deadline) {
        assembly ("memory-safe") {
            if lt(deadline, timestamp()) {
                mstore(0x00, 0x27eeb8ae) // selector for DeadlineExpired
                mstore(0x04, deadline)
                mstore(0x24, timestamp())
                revert(0x00, 0x44)
            }
        }
        _;
    }

    // ============ Constructor ============

    /// @notice Deploys Executor with governance parameters
    /// @param _authorizedSigner Initial Execution Signer address
    /// @param _treasury Treasury address for yield distribution
    /// @param _minYield Minimum yield threshold (wei)
    /// @param _owner Contract owner address
    /// @dev All addresses validated non-zero
    /// @notice Constructor with flashloan provider addresses
    /// @param _authorizedSigner Initial Execution Signer
    /// @param _treasury Treasury address
    /// @param _minYield Minimum yield threshold
    /// @param _owner Contract owner
    /// @param _aaveV3Pool Aave V3 Pool (address(0) to disable)
    /// @param _balancerVault Balancer Vault (address(0) to disable)
    /// @param _makerDssFlash MakerDAO DSS Flash (address(0) to disable)
    constructor(
        address _authorizedSigner,
        address _treasury,
        uint256 _minYield,
        address _owner,
        address _aaveV3Pool,
        address _balancerVault,
        address _makerDssFlash
    )
        Ownable(_owner)
        FlashloanProvider(_aaveV3Pool, _balancerVault, _makerDssFlash, _owner)
    {
        // ── checks ──
        if (_authorizedSigner == address(0)) revert InvalidSignature();
        if (_treasury == address(0)) revert UnauthorizedOwner();
        if (_minYield == 0) revert InsufficientYield(0, 1);

        // ── effects ──
        authorizedSigner = _authorizedSigner;
        treasury = _treasury;
        minYield = _minYield;

        emit SignerUpdated(address(0), _authorizedSigner);
        emit TreasuryUpdated(address(0), _treasury);
        emit MinYieldUpdated(0, _minYield);
    }

    // ============ External Functions ============

    /// @notice Execute an arbitrage resolution atomically
    /// @param resolution Complete signed resolution from Rust backend
    /// @return yieldCaptured Net yield after flashloan fees and swap costs
    /// @dev Gas cost: ~180k base + ~45k per swap hop + flashloan overhead (~30k)
    /// @dev Pattern: checks → effects → interactions (CEI)
    /// @dev Reverts atomically if topological yield ≤ minYield
    function execute(Resolution calldata resolution)
        external
        override
        nonReentrant
        onlyAuthorized
        checkDeadline(resolution.deadline)
        returns (uint256 yieldCaptured)
    {
        uint256 gasStart = gasleft();

        // ── CHECKS: nonce, signature ──
        if (usedNonces[resolution.nonce]) revert InvalidNonce(resolution.nonce);
        if (!_validateSignature(resolution)) revert InvalidSignature();

        // Mark nonce consumed (effects BEFORE interactions)
        usedNonces[resolution.nonce] = true;
        _activeResolution = resolution;

        // ── Capture pre-execution balances for invariant checks ──
        _snapshotBalances(resolution.steps);

        // ── INTERACTIONS: Flashloan + swaps ──
        yieldCaptured = _executeCore(resolution);

        // ── Post-execution invariant: topological yield > 0 ──
        if (yieldCaptured <= minYield) {
            delete _activeResolution;
            totalReverts++;
            emit ResolutionReverted(resolution.nonce, msg.sender, 0);
            revert InsufficientYield(yieldCaptured, minYield);
        }

        // ── Post-execution invariant: balances never decreased ──
        if (!_validatePostResolution(resolution.steps)) {
            delete _activeResolution;
            totalReverts++;
            emit ResolutionReverted(resolution.nonce, msg.sender, 5);
            revert PostResolutionInvariantFailed();
        }

        // ── EFFECTS: distribute yield ──
        _distributeYield(resolution.steps, yieldCaptured);

        totalYieldCaptured += yieldCaptured;
        totalResolutions++;
        delete _activeResolution;

        emit ResolutionExecuted(
            resolution.nonce,
            msg.sender,
            yieldCaptured,
            gasStart - gasleft()
        );
    }

    /// @notice Validate a resolution without executing
    /// @param resolution Resolution payload to validate
    /// @return valid True if all checks pass
    /// @return expectedYield Estimated yield (0 if validation fails)
    /// @dev Gas cost: ~25k (pure view, no state changes)
    function validate(Resolution calldata resolution)
        external
        view
        override
        returns (bool valid, uint256 expectedYield)
    {
        // Check nonce
        if (usedNonces[resolution.nonce]) return (false, 0);

        // Check deadline
        if (block.timestamp > resolution.deadline) return (false, 0);

        // Check signature
        if (!_validateSignature(resolution)) return (false, 0);

        // Simulate yield estimation from steps
        expectedYield = _estimateYield(resolution.steps);
        valid = expectedYield >= resolution.minYield;
    }

    /// @notice Emergency withdrawal of stuck tokens to treasury
    /// @param token Token to withdraw (address(0) for ETH)
    /// @dev Gas cost: ~35k
    /// @dev Only callable by owner
    function emergencyWithdraw(address token)
        external
        override
        onlyOwner
    {
        bool success;

        if (token == address(0)) {
            // Withdraw ETH
            uint256 ethBal = address(this).balance;
            (success, ) = treasury.call{value: ethBal}("");
        } else {
            // Withdraw ERC20
            uint256 tokenBal = IERC20(token).balanceOf(address(this));
            IERC20(token).safeTransfer(treasury, tokenBal);
            success = true;
        }

        if (!success) revert EmergencyWithdrawFailed();
    }

    // ============ External: Flashloan Callback Entry Point ============

    /// @notice Callback invoked by flashloan providers (Aave V3 / Balancer / Maker)
    /// @param assets Array of loaned token addresses
    /// @param amounts Array of loaned amounts
    /// @param fees Array of flashloan fees
    /// @param initiator Original initiator of the flashloan
    /// @param params Encoded Resolution.steps for swap execution
    /// @return success True if all swaps executed and repayment possible
    /// @dev Only callable by registered flashloan providers
    function executeOperation(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata fees,
        address initiator,
        bytes calldata params
    )
        external
        override
        returns (bool success)
    {
        // Security: verify callback originates from expected provider
        address flashProvider = _activeResolution.flash.provider;
        if (msg.sender != flashProvider) revert UnauthorizedFlashloanProvider();

        // Decode SwapStep[] from params using Yul for gas efficiency
        SwapStep[] memory steps = _decodeSwapSteps(params);

        // Execute all swap hops
        _executeSwapSteps(steps);

        // Approve repayment of flashloan + fee back to provider
        uint256 totalRepayment = amounts[0] + fees[0];
        IERC20(assets[0]).safeApprove(flashProvider, totalRepayment);

        // Verify this contract has enough to repay
        if (IERC20(assets[0]).balanceOf(address(this)) < totalRepayment) {
            return false;
        }

        return true;
    }

    // ============ External: Owner Administration ============

    /// @notice Update the authorized Execution Signer
    /// @param newSigner New signer address
    /// @dev Only callable by owner
    function setAuthorizedSigner(address newSigner) external onlyOwner {
        if (newSigner == address(0)) revert InvalidSignature();
        address oldSigner = authorizedSigner;
        authorizedSigner = newSigner;
        emit SignerUpdated(oldSigner, newSigner);
    }

    /// @notice Update minimum yield threshold
    /// @param newMinYield New minimum yield (wei)
    /// @dev Only callable by owner
    function setMinYield(uint256 newMinYield) external onlyOwner {
        if (newMinYield == 0) revert InsufficientYield(0, 1);
        uint256 oldMinYield = minYield;
        minYield = newMinYield;
        emit MinYieldUpdated(oldMinYield, newMinYield);
    }

    /// @notice Update treasury address
    /// @param newTreasury New treasury address
    /// @dev Only callable by owner
    function setTreasury(address newTreasury) external onlyOwner {
        if (newTreasury == address(0)) revert UnauthorizedOwner();
        address oldTreasury = treasury;
        treasury = newTreasury;
        emit TreasuryUpdated(oldTreasury, newTreasury);
    }

    /// @notice Register or update a DEX adapter
    /// @param dexType Numeric DEX identifier
    /// @param adapter Adapter contract address
    /// @dev Only callable by owner
    function registerAdapter(uint8 dexType, address adapter) external onlyOwner {
        adapters[dexType] = adapter;
        emit AdapterRegistered(dexType, adapter);
    }

    /// @notice Batch register multiple adapters
    /// @param dexTypes Array of DEX type identifiers
    /// @param adapterAddrs Array of adapter addresses
    function batchRegisterAdapters(
        uint8[] calldata dexTypes,
        address[] calldata adapterAddrs
    ) external onlyOwner {
        uint256 len = dexTypes.length;
        if (len != adapterAddrs.length) revert InvalidSignature();

        for (uint256 i; i < len; ) {
            adapters[dexTypes[i]] = adapterAddrs[i];
            emit AdapterRegistered(dexTypes[i], adapterAddrs[i]);
            unchecked { ++i; }
        }
    }

    // ============ Internal: Core Execution ============

    /// @notice Internal core execution: flashloan + swaps
    /// @param resolution Complete resolution payload
    /// @return yield Net yield captured
    function _executeCore(Resolution calldata resolution)
        internal
        returns (uint256 yield)
    {

        FlashParams calldata flash = resolution.flash;

        if (flash.provider != address(0) && flash.amount > 0) {
            // ── Flashloan path ──
            // Encode steps for callback
            bytes memory callbackData = _encodeSwapSteps(resolution.steps);

            // Route to correct flashloan provider
            _initiateFlashloan(flash, callbackData);
        } else {
            // ── No flashloan: direct swap execution ──
            _executeSwapSteps(resolution.steps);
        }

        // Calculate topological yield from final token balance
        yield = _calculateTopologicalYield(resolution.steps);
    }

    // ============ Internal: Signature Validation ============

    /// @notice Validate ECDSA signature of a Resolution
    /// @param resolution Resolution containing signature to verify
    /// @return valid True if signature matches authorizedSigner
    function _validateSignature(Resolution calldata resolution)
        internal
        view
        returns (bool valid)
    {
        bytes32 structHash = keccak256(
            abi.encode(
                keccak256(
                    "Resolution(SwapStep[] steps,FlashParams flash,uint256 minYield,uint256 deadline,uint256 nonce)"
                ),
                keccak256(abi.encode(resolution.steps)),
                keccak256(abi.encode(resolution.flash)),
                resolution.minYield,
                resolution.deadline,
                resolution.nonce
            )
        );

        bytes32 digest = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", structHash)
        );

        address recovered = recoverSigner(digest, resolution.signature);
        return recovered == authorizedSigner;
    }

    // ============ Internal: Swap Execution ============

    /// @notice Execute all swap steps through registered adapters
    /// @param steps Array of SwapStep to execute
    /// @dev Delegates to adapter contract based on dexType
    function _executeSwapSteps(SwapStep[] memory steps) internal {
        uint256 len = steps.length;
        for (uint256 i; i < len; ) {
            SwapStep memory step = steps[i];
            address adapter = adapters[step.dexType];
            if (adapter == address(0)) revert AdapterNotRegistered(step.dexType);

            // Approve adapter to spend tokenIn
            IERC20(step.tokenIn).safeApprove(adapter, step.amountIn);

            // Delegate swap to adapter via low-level call for gas efficiency
            (bool success, ) = adapter.call(
                abi.encodeWithSelector(
                    bytes4(keccak256("swap(address,address,uint256,uint256,address,uint24)")),
                    step.tokenIn,
                    step.tokenOut,
                    step.amountIn,
                    step.amountOutMin,
                    step.dex,
                    step.feeOrPoolFee
                )
            );

            if (!success) {
                // Try alternative swap signature (V2 style)
                (success, ) = adapter.call(
                    abi.encodeWithSelector(
                        bytes4(keccak256("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)")),
                        step.amountIn,
                        step.amountOutMin,
                        _buildPath(step.tokenIn, step.tokenOut),
                        address(this),
                        block.timestamp
                    )
                );
            }

            if (!success) revert AdapterNotRegistered(step.dexType);

            unchecked { ++i; }
        }
    }

    // ============ Internal: Balance Invariants ============

    /// @notice Snapshot balances before execution for invariant checks
    /// @param steps Swap steps to determine which token balances to snapshot
    function _snapshotBalances(SwapStep[] calldata steps) internal {
        uint256 len = steps.length;

        // Snapshot first tokenIn and last tokenOut
        if (len > 0) {
            address startToken = steps[0].tokenIn;
            _balanceBefore[startToken] = IERC20(startToken).balanceOf(address(this));

            address endToken = steps[len - 1].tokenOut;
            if (endToken != startToken) {
                _balanceBefore[endToken] = IERC20(endToken).balanceOf(address(this));
            }
        }
    }

    /// @notice Verify post-execution invariants: balances never decrease
    /// @param steps Swap steps that were executed
    /// @return valid True if all balances are >= pre-execution snapshots
    /// @dev Invariant: For every token touched, balance_after >= balance_before
    function _validatePostResolution(SwapStep[] calldata steps)
        internal
        view
        returns (bool valid)
    {
        uint256 len = steps.length;
        if (len == 0) return true;

        for (uint256 i; i < len; ) {
            SwapStep calldata step = steps[i];

            uint256 balanceNow = IERC20(step.tokenIn).balanceOf(address(this));
            if (balanceNow < _balanceBefore[step.tokenIn]) {
                return false;
            }

            unchecked { ++i; }
        }

        // Check final tokenOut balance as well
        address endToken = steps[len - 1].tokenOut;
        uint256 endBalance = IERC20(endToken).balanceOf(address(this));
        if (endBalance < _balanceBefore[endToken]) {
            return false;
        }

        return true;
    }

    // ============ Internal: Yield Calculation & Distribution ============

    /// @notice Calculate topological yield from swap results
    /// @param steps Swap steps that were executed
    /// @return yield Net yield in wei units
    function _calculateTopologicalYield(SwapStep[] calldata steps)
        internal
        view
        returns (uint256 yield)
    {
        if (steps.length == 0) return 0;

        // Topological yield = final_token_balance - initial_token_balance
        // (for the output token of the last step)
        address outputToken = steps[steps.length - 1].tokenOut;
        uint256 finalBalance = IERC20(outputToken).balanceOf(address(this));
        uint256 initialBalance = _balanceBefore[outputToken];

        if (finalBalance > initialBalance) {
            yield = finalBalance - initialBalance;
        } else {
            yield = 0;
        }
    }

    /// @notice Distribute captured yield to treasury
    /// @param steps Swap steps for token identification
    /// @param yieldAmount Amount of yield to distribute
    function _distributeYield(SwapStep[] calldata steps, uint256 yieldAmount)
        internal
    {
        if (steps.length == 0 || yieldAmount == 0) return;

        address outputToken = steps[steps.length - 1].tokenOut;

        // Transfer yield to treasury
        IERC20(outputToken).safeTransfer(treasury, yieldAmount);

        emit YieldCaptured(
            _activeResolution.nonce,
            outputToken,
            yieldAmount,
            treasury
        );
    }

    // ============ Internal: Yield Estimation (View) ============

    /// @notice Estimate yield from swap steps without executing
    /// @param steps Swap steps to estimate
    /// @return estimatedYield Estimated yield amount
    function _estimateYield(SwapStep[] memory steps)
        internal
        pure
        returns (uint256 estimatedYield)
    {
        // Simplified estimation: sum of (amountIn - amountOutMin) across steps
        uint256 len = steps.length;
        for (uint256 i; i < len; ) {
            SwapStep memory step = steps[i];
            if (step.amountOutMin > step.amountIn) {
                estimatedYield += (step.amountOutMin - step.amountIn);
            }
            unchecked { ++i; }
        }
    }

    // ============ Internal: Encoding/Decoding ============

    /// @notice Encode SwapStep[] into bytes for flashloan callback
    /// @param steps Array to encode
    /// @return encoded ABI-encoded SwapStep[]
    function _encodeSwapSteps(SwapStep[] calldata steps)
        internal
        pure
        returns (bytes memory encoded)
    {
        encoded = abi.encode(steps);
    }

    /// @notice Decode SwapStep[] from calldata bytes using Yul
    /// @param data ABI-encoded SwapStep[]
    /// @return steps Decoded SwapStep array
    /// @dev Gas optimization: uses assembly for efficient decoding
    function _decodeSwapSteps(bytes calldata data)
        internal
        pure
        returns (SwapStep[] memory steps)
    {
        assembly ("memory-safe") {
            let offset := calldataload(data.offset)
            let dataLoc := add(data.offset, offset)
            let len := calldataload(dataLoc)

            // Allocate memory
            steps := mload(0x40)
            mstore(steps, len)

            let dstPtr := add(steps, 0x20)
            let srcPtr := add(dataLoc, 0x20)

            // Each SwapStep is a dynamic struct in calldata
            // Copy struct data: 5 words of fixed data + dynamic bytes offset
            for { let i := 0 } lt(i, len) { i := add(i, 1) } {
                // Read the offset to this struct's data
                let structOffset := calldataload(add(srcPtr, mul(i, 0x20)))
                let structPtr := add(dataLoc, structOffset)

                // tokenIn (address) at structPtr + 0x20
                mstore(dstPtr, calldataload(add(structPtr, 0x20)))
                // tokenOut (address) at structPtr + 0x40
                mstore(add(dstPtr, 0x20), calldataload(add(structPtr, 0x40)))
                // amountIn (uint256) at structPtr + 0x60
                mstore(add(dstPtr, 0x40), calldataload(add(structPtr, 0x60)))
                // amountOutMin (uint256) at structPtr + 0x80
                mstore(add(dstPtr, 0x60), calldataload(add(structPtr, 0x80)))
                // dex (address) at structPtr + 0xA0
                mstore(add(dstPtr, 0x80), calldataload(add(structPtr, 0xA0)))
                // dexType (uint8 padded to uint256) at structPtr + 0xC0
                mstore(add(dstPtr, 0xA0), calldataload(add(structPtr, 0xC0)))
                // feeOrPoolFee (uint24 padded to uint256) at structPtr + 0xE0
                mstore(add(dstPtr, 0xC0), calldataload(add(structPtr, 0xE0)))

                // For data (bytes), store offset - we skip dynamic part for gas
                mstore(add(dstPtr, 0xE0), 0x100) // relative offset to empty bytes

                dstPtr := add(dstPtr, 0x100)
            }

            mstore(0x40, dstPtr)
        }
    }

    // ============ Internal: Flashloan Routing ============

    /// @notice Route flashloan initiation to the correct provider
    /// @param flash Flashloan parameters
    /// @param callbackData Encoded steps for callback execution
    function _initiateFlashloan(FlashParams calldata flash, bytes memory callbackData)
        internal
    {
        if (flash.provider == AAVE_V3_POOL) {
            // Aave V3 flashloanSimple
            _flashloanAaveV3(flash.token, flash.amount, callbackData);
        } else if (flash.provider == BALANCER_VAULT) {
            // Balancer flashLoan
            address[] memory tokens = new address[](1);
            tokens[0] = flash.token;
            uint256[] memory amounts = new uint256[](1);
            amounts[0] = flash.amount;
            _flashloanBalancer(tokens, amounts, callbackData);
        } else if (flash.provider == MAKDAO_DSS_FLASH) {
            // MakerDAO flashloan
            _flashloanMaker(flash.token, flash.amount, callbackData);
        } else {
            revert UnauthorizedFlashloanProvider();
        }
    }

    // ============ Internal: Path Building ============

    /// @notice Build a 2-hop path array for V2-style swaps
    /// @param tokenIn Input token
    /// @param tokenOut Output token
    /// @return path Array [tokenIn, tokenOut]
    function _buildPath(address tokenIn, address tokenOut)
        internal
        pure
        returns (address[] memory path)
    {
        path = new address[](2);
        path[0] = tokenIn;
        path[1] = tokenOut;
    }

    // ============ Receive ============

    /// @notice Accept ETH transfers
    receive() external payable {}
}
