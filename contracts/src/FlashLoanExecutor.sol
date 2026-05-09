// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
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

/// @title FlashLoanExecutor
/// @dev Wrapper para ejecutar arbitrajes fondeados por Aave V3
contract FlashLoanExecutor is AccessControl {
    using SafeERC20 for IERC20;

    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");
    IAaveV3Pool public aavePool;
    address public arbitrageExecutor;

    // SC-06: observability events for off-chain monitoring (recon, dashboard)
    /// @dev Emitted when a flash loan is requested to the Aave pool.
    event FlashLoanRequested(address indexed asset, uint256 amount, bytes32 paramsHash);
    /// @dev Emitted when the Aave callback completes and the loan is fully repaid.
    event FlashLoanExecuted(address indexed asset, uint256 amount, uint256 premium, bool success);

    constructor(address _aavePool, address _arbitrageExecutor) {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
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
}
