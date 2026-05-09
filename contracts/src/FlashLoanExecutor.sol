// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// =============================================================================
// STORAGE LAYOUT — APPEND-ONLY RULE (SC-08, 2026-05-08)
// =============================================================================
// Parent contracts (Initializable, AccessControlUpgradeable, UUPSUpgradeable)
// all use ERC-7201 namespaced slots — they do NOT occupy linear slot space.
//
// This contract's OWN variables start at linear slot 0:
//   slot 0: aavePool            (IAaveV3Pool — address, 20 bytes, packed in slot)
//   slot 1: arbitrageExecutor   (address, 20 bytes)
//
// CRITICAL: When adding new state variables in V2, V3, etc., you MUST append
// them AFTER slot 1.  NEVER insert variables between existing ones — that
// would corrupt the storage layout and brick all proxies pointing at this impl.
// =============================================================================

import "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

interface IAaveV3Pool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

/// @title FlashLoanExecutor — UUPS-upgradeable Aave V3 flash loan wrapper
/// @dev Wrapper para ejecutar arbitrajes fondeados por Aave V3.
///      Refactored to UUPS proxy pattern (SC-08).  aavePool and arbitrageExecutor
///      moved from constructor params to initialize() params.
contract FlashLoanExecutor is
    Initializable,
    AccessControlUpgradeable,
    UUPSUpgradeable
{
    using SafeERC20 for IERC20;

    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    /// @dev Separate UPGRADER_ROLE allows key rotation independent of admin.
    bytes32 public constant UPGRADER_ROLE = keccak256("UPGRADER_ROLE");

    // slot 0
    IAaveV3Pool public aavePool;
    // slot 1
    address public arbitrageExecutor;
    // APPEND new variables below this line in future upgrades. Never above.

    // SC-06: observability events for off-chain monitoring (recon, dashboard)
    /// @dev Emitted when a flash loan is requested to the Aave pool.
    event FlashLoanRequested(address indexed asset, uint256 amount, bytes32 paramsHash);
    /// @dev Emitted when the Aave callback completes and the loan is fully repaid.
    event FlashLoanExecuted(address indexed asset, uint256 amount, uint256 premium, bool success);

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @dev Replaces constructor.  Must be called exactly once via ERC1967Proxy.
    /// @param admin               Address granted DEFAULT_ADMIN_ROLE and UPGRADER_ROLE.
    /// @param _aavePool           Aave V3 lending pool address.
    /// @param _arbitrageExecutor  ArbitrageExecutor proxy address.
    function initialize(
        address admin,
        address _aavePool,
        address _arbitrageExecutor
    ) public initializer {
        __AccessControl_init();
        __UUPSUpgradeable_init();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(UPGRADER_ROLE, admin);
        aavePool = IAaveV3Pool(_aavePool);
        arbitrageExecutor = _arbitrageExecutor;
    }

    /// @dev Solicita el flashloan y delega la ejecución al ArbitrageExecutor
    function requestFlashLoan(address asset, uint256 amount, bytes calldata params) external onlyRole(EXECUTOR_ROLE) {
        aavePool.flashLoanSimple(address(this), asset, amount, params, 0);
        // SC-06: emit after the call so the event is only logged when the pool
        // accepted the request without reverting.
        emit FlashLoanRequested(asset, amount, keccak256(params));
    }

    /// @dev Callback requerido por Aave V3 tras recibir los fondos
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool) {
        require(msg.sender == address(aavePool), "Caller must be AavePool");
        require(initiator == address(this), "Initiator must be this contract");

        // 1. Aprobar fondos al ArbitrageExecutor
        IERC20(asset).forceApprove(arbitrageExecutor, amount);

        // 2. Llamar al ArbitrageExecutor (el payload está encodeado en `params`)
        (bool success, ) = arbitrageExecutor.call(params);
        require(success, "Arbitrage execution failed");

        // 3. Repagar a Aave (Monto + Premium)
        uint256 amountToOwe = amount + premium;
        IERC20(asset).forceApprove(address(aavePool), amountToOwe);

        // SC-06: signal successful completion to off-chain monitors before returning
        emit FlashLoanExecuted(asset, amount, premium, true);
        return true;
    }

    // -------------------------------------------------------------------------
    // SC-08: UUPS upgrade authorization
    // -------------------------------------------------------------------------

    /// @dev Only UPGRADER_ROLE can authorize a new implementation.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE) {}
}
