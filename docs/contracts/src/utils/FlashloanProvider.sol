// SPDX-License-Identifier: MIT
pragma solidity 0.8.19;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title FlashloanProvider
/// @notice Abstraction layer for flashloan providers: Aave V3, Balancer, MakerDAO
/// @author ArbitrageX Team
/// @dev Invariant: total repayment = amount + fee for all providers
/// @dev No hardcoded addresses — all providers set via constructor/immutable
contract FlashloanProvider {
    using SafeERC20 for IERC20;

    // ============ Events ============

    /// @notice Emitted when a flashloan is initiated
    /// @param provider Flashloan provider address
    /// @param token Token being flashloaned
    /// @param amount Loan amount
    event FlashloanInitiated(
        address indexed provider,
        address indexed token,
        uint256 amount
    );

    /// @notice Emitted when a flashloan is successfully repaid
    /// @param provider Flashloan provider address
    /// @param token Token repaid
    /// @param amount Principal repaid
    /// @param fee Flashloan fee paid
    event FlashloanRepaid(
        address indexed provider,
        address indexed token,
        uint256 amount,
        uint256 fee
    );

    /// @notice Emitted when flashloan provider addresses are updated
    /// @param aaveV3Pool New Aave V3 Pool address
    /// @param balancerVault New Balancer Vault address
    /// @param makerDssFlash New MakerDAO DSS Flash address
    event ProvidersUpdated(
        address aaveV3Pool,
        address balancerVault,
        address makerDssFlash
    );

    // ============ Errors ============

    /// @notice Flashloan provider not recognized
    error UnknownProvider(address provider);

    /// @notice Flashloan callback not from expected provider
    error InvalidCallbackOrigin();

    /// @notice Insufficient balance to repay flashloan
    error InsufficientRepaymentBalance(uint256 required, uint256 actual);

    /// @notice Flashloan initiation failed at the provider level
    error FlashloanInitiationFailed();

    /// @notice Callback execution failed
    error CallbackExecutionFailed();

    // ============ Provider Addresses ============

    /// @notice Aave V3 Pool address
    address public AAVE_V3_POOL;

    /// @notice Balancer V2 Vault address
    address public BALANCER_VAULT;

    /// @notice MakerDAO DSS Flash address
    address public MAKDAO_DSS_FLASH;

    /// @notice Owner who can update provider addresses
    address public providerOwner;

    // ============ Callback Interface ============

    /// @notice Callback invoked by flashloan providers after loan disbursement
    /// @param assets Array of loaned token addresses
    /// @param amounts Array of loaned amounts
    /// @param fees Array of flashloan fees
    /// @param initiator Original initiator address
    /// @param params Arbitrary callback data (encoded SwapStep[])
    /// @return success True if callback executed successfully
    function executeOperation(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata fees,
        address initiator,
        bytes calldata params
    )
        external
        virtual
        returns (bool success)
    {
        // Override in Executor for swap logic
        // This base implementation just validates callback origin

        if (msg.sender != AAVE_V3_POOL &&
            msg.sender != BALANCER_VAULT &&
            msg.sender != MAKDAO_DSS_FLASH) {
            revert InvalidCallbackOrigin();
        }

        // Approve repayment by default
        if (assets.length > 0 && amounts.length > 0) {
            uint256 totalRepayment = amounts[0] + fees[0];
            IERC20(assets[0]).approve(msg.sender, totalRepayment);
        }

        return true;
    }

    // ============ Constructor ============

    /// @notice Sets immutable flashloan provider addresses
    /// @param _aaveV3Pool Aave V3 Pool address
    /// @param _balancerVault Balancer V2 Vault address
    /// @param _makerDssFlash MakerDAO DSS Flash address
    /// @dev Zero addresses are allowed — provider will be disabled
    constructor(
        address _aaveV3Pool,
        address _balancerVault,
        address _makerDssFlash,
        address _owner
    ) {
        AAVE_V3_POOL = _aaveV3Pool;
        BALANCER_VAULT = _balancerVault;
        MAKDAO_DSS_FLASH = _makerDssFlash;
        providerOwner = _owner;

        emit ProvidersUpdated(_aaveV3Pool, _balancerVault, _makerDssFlash);
    }

    // ============ Provider Administration ============

    /// @notice Set flashloan provider addresses
    /// @param _aaveV3Pool Aave V3 Pool
    /// @param _balancerVault Balancer Vault
    /// @param _makerDssFlash MakerDAO DSS Flash
    /// @dev Only callable by provider owner
    function setProviders(
        address _aaveV3Pool,
        address _balancerVault,
        address _makerDssFlash
    )
        external
    {
        if (msg.sender != providerOwner) revert InvalidCallbackOrigin();
        AAVE_V3_POOL = _aaveV3Pool;
        BALANCER_VAULT = _balancerVault;
        MAKDAO_DSS_FLASH = _makerDssFlash;
        emit ProvidersUpdated(_aaveV3Pool, _balancerVault, _makerDssFlash);
    }

    // ============ Internal: Aave V3 Flashloan ============

    /// @notice Initiate flashloan via Aave V3 Pool
    /// @param token Token to borrow
    /// @param amount Amount to borrow
    /// @param params Callback data for executeOperation
    /// @dev Gas cost: ~55k base + token transfer overhead
    /// @dev Aave V3 flashloanSimple charges 0.05% - 0.5% fee
    function _flashloanAaveV3(
        address token,
        uint256 amount,
        bytes memory params
    )
        internal
    {
        if (AAVE_V3_POOL == address(0)) revert UnknownProvider(address(0));

        // ── checks ──
        if (token == address(0) || amount == 0) revert FlashloanInitiationFailed();

        emit FlashloanInitiated(AAVE_V3_POOL, token, amount);

        // ── interactions: call Aave V3 Pool.flashLoanSimple ──
        (bool success, ) = AAVE_V3_POOL.call(
            abi.encodeWithSelector(
                bytes4(keccak256("flashLoanSimple(address,address,uint256,bytes,uint16)")),
                address(this), // receiverAddress
                token,         // asset
                amount,        // amount
                params,        // params
                0              // referralCode
            )
        );

        if (!success) revert FlashloanInitiationFailed();
    }

    // ============ Internal: Balancer Flashloan ============

    /// @notice Initiate flashloan via Balancer V2 Vault
    /// @param tokens Array of tokens to borrow
    /// @param amounts Array of amounts to borrow
    /// @param params Callback data for receiveFlashLoan
    /// @dev Gas cost: ~50k base
    /// @dev Balancer flashloans are free (0% fee) — optimal for large amounts
    function _flashloanBalancer(
        address[] memory tokens,
        uint256[] memory amounts,
        bytes memory params
    )
        internal
    {
        if (BALANCER_VAULT == address(0)) revert UnknownProvider(address(0));

        // ── checks ──
        if (tokens.length == 0 || amounts.length == 0) revert FlashloanInitiationFailed();

        emit FlashloanInitiated(BALANCER_VAULT, tokens[0], amounts[0]);

        // ── interactions: call Balancer Vault.flashLoan ──
        (bool success, ) = BALANCER_VAULT.call(
            abi.encodeWithSelector(
                bytes4(keccak256("flashLoan(address,address[],uint256[],bytes)")),
                address(this), // recipient
                tokens,        // tokens
                amounts,       // amounts
                params         // userData
            )
        );

        if (!success) revert FlashloanInitiationFailed();
    }

    /// @notice Balancer flashloan callback (IFlashLoanRecipient interface)
    /// @param tokens Tokens that were flashloaned
    /// @param amounts Amounts that were flashloaned
    /// @param feeAmounts Fees charged by Balancer
    /// @param userData Original user data passed to flashLoan
    function receiveFlashLoan(
        address[] calldata tokens,
        uint256[] calldata amounts,
        uint256[] calldata feeAmounts,
        bytes calldata userData
    )
        external
    {
        // ── checks ──
        if (msg.sender != BALANCER_VAULT) revert InvalidCallbackOrigin();

        // ── Delegate to unified callback ──
        bool success = this.executeOperation(
            tokens,
            amounts,
            feeAmounts,
            address(this),
            userData
        );

        if (!success) revert CallbackExecutionFailed();

        // ── Repay Balancer ──
        uint256 len = tokens.length;
        for (uint256 i; i < len; ) {
            uint256 totalRepayment = amounts[i] + feeAmounts[i];
            IERC20(tokens[i]).safeTransfer(BALANCER_VAULT, totalRepayment);

            emit FlashloanRepaid(
                BALANCER_VAULT,
                tokens[i],
                amounts[i],
                feeAmounts[i]
            );
            unchecked { ++i; }
        }
    }

    // ============ Internal: MakerDAO Flashloan ============

    /// @notice Initiate flashloan via MakerDAO DSS Flash
    /// @param token Token to borrow (must be DAI)
    /// @param amount Amount to borrow
    /// @param params Callback data
    /// @dev Gas cost: ~45k base
    /// @dev MakerDAO flashloans are free (0% fee)
    function _flashloanMaker(
        address token,
        uint256 amount,
        bytes memory params
    )
        internal
    {
        if (MAKDAO_DSS_FLASH == address(0)) revert UnknownProvider(address(0));

        // ── checks ──
        if (token == address(0) || amount == 0) revert FlashloanInitiationFailed();

        emit FlashloanInitiated(MAKDAO_DSS_FLASH, token, amount);

        // ── interactions: call DssFlash.flashLoan ──
        (bool success, ) = MAKDAO_DSS_FLASH.call(
            abi.encodeWithSelector(
                bytes4(keccak256("flashLoan(address,uint256,bytes)")),
                address(this), // receiver
                amount,        // amount
                params         // data
            )
        );

        if (!success) revert FlashloanInitiationFailed();
    }

    /// @notice MakerDAO flashloan callback (ERC3156FlashBorrower interface)
    /// @param initiator Address that initiated the flashloan
    /// @param token Token borrowed
    /// @param amount Amount borrowed
    /// @param fee Fee charged
    /// @param data Callback data
    /// @return success True if callback succeeded
    function onFlashLoan(
        address initiator,
        address token,
        uint256 amount,
        uint256 fee,
        bytes calldata data
    )
        external
        returns (bool success)
    {
        // ── checks ──
        if (msg.sender != MAKDAO_DSS_FLASH) revert InvalidCallbackOrigin();

        // ── Build arrays for unified callback ──
        address[] memory tokens = new address[](1);
        tokens[0] = token;
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;
        uint256[] memory fees = new uint256[](1);
        fees[0] = fee;

        // ── Delegate to unified callback ──
        success = this.executeOperation(
            tokens,
            amounts,
            fees,
            initiator,
            data
        );

        // ── Repay MakerDAO ──
        uint256 totalRepayment = amount + fee;
        IERC20(token).safeApprove(MAKDAO_DSS_FLASH, totalRepayment);

        emit FlashloanRepaid(MAKDAO_DSS_FLASH, token, amount, fee);
    }

    // ============ View Functions ============

    /// @notice Get flashloan fee for a given provider
    /// @param provider Provider address
    /// @param token Token to borrow
    /// @param amount Amount to borrow
    /// @return fee Expected fee amount
    /// @dev Aave V3: 0.05% (5 bps), Balancer: 0%, MakerDAO: 0%
    function getFlashloanFee(
        address provider,
        address token,
        uint256 amount
    )
        external
        view
        returns (uint256 fee)
    {
        if (provider == AAVE_V3_POOL) {
            // Aave V3: 0.05% = 5 bps = amount * 5 / 10000
            fee = (amount * 5) / 10000;
        } else if (provider == BALANCER_VAULT || provider == MAKDAO_DSS_FLASH) {
            // Balancer and MakerDAO: free flashloans
            fee = 0;
        } else {
            revert UnknownProvider(provider);
        }
    }
}
