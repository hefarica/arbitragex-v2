// SPDX-License-Identifier: BUSL-1.1
pragma solidity 0.8.24;

/**
 * @title  AaveV3CrossChainAdapter
 * @notice Adaptador holonómico para Aave V3 multichain (Polygon, Arbitrum,
 *         Optimism, Base, Avalanche, BSC).  Sirve depósitos/retiros y la lectura
 *         del estado de salud (health factor) para gates termodinámicos.
 *
 * @dev    NO ejecuta flash-loans aquí (eso lo cubre AaveV3FlashAdapter.sol).
 *         Cero retención de tokens; Ghost Protocol respetado.
 */

interface IAavePool {
    function supply(address asset, uint256 amount, address onBehalfOf, uint16 referralCode) external;
    function withdraw(address asset, uint256 amount, address to) external returns (uint256);
    function getUserAccountData(address user) external view returns (
        uint256 totalCollateralBase,
        uint256 totalDebtBase,
        uint256 availableBorrowsBase,
        uint256 currentLiquidationThreshold,
        uint256 ltv,
        uint256 healthFactor
    );
}

interface IAavePoolAddressesProvider {
    function getPool() external view returns (address);
}

interface IERC20A {
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
}

contract AaveV3CrossChainAdapter {
    error Aave__ZeroAmount();
    error Aave__HealthBelowFloor(uint256 healthFactor, uint256 floor);

    uint256 public constant HEALTH_FACTOR_FLOOR = 1.10e18; // 10% margen sobre liquidación

    event AaveSupply(address indexed pool, address indexed asset, uint256 amount, address indexed onBehalfOf);
    event AaveWithdraw(address indexed pool, address indexed asset, uint256 amount, address indexed to);

    function supply(
        address provider,
        address asset,
        uint256 amount,
        address onBehalfOf
    ) external {
        if (amount == 0) revert Aave__ZeroAmount();
        address pool = IAavePoolAddressesProvider(provider).getPool();

        IERC20A(asset).transferFrom(msg.sender, address(this), amount);
        IERC20A(asset).approve(pool, amount);
        IAavePool(pool).supply(asset, amount, onBehalfOf, 0);

        emit AaveSupply(pool, asset, amount, onBehalfOf);
    }

    function withdraw(
        address provider,
        address asset,
        uint256 amount,
        address to
    ) external returns (uint256 withdrawn) {
        if (amount == 0) revert Aave__ZeroAmount();
        address pool = IAavePoolAddressesProvider(provider).getPool();

        withdrawn = IAavePool(pool).withdraw(asset, amount, to);
        emit AaveWithdraw(pool, asset, withdrawn, to);
    }

    /**
     * @notice Lee el health factor del usuario y revierte si está por debajo del piso.
     * @dev    Gate termodinámico: bloquea ejecución si la posición está sub-collateralizada.
     */
    function assertHealthAboveFloor(address provider, address user) external view {
        address pool = IAavePoolAddressesProvider(provider).getPool();
        (, , , , , uint256 hf) = IAavePool(pool).getUserAccountData(user);
        if (hf < HEALTH_FACTOR_FLOOR) revert Aave__HealthBelowFloor(hf, HEALTH_FACTOR_FLOOR);
    }

    function healthFactor(address provider, address user) external view returns (uint256) {
        address pool = IAavePoolAddressesProvider(provider).getPool();
        (, , , , , uint256 hf) = IAavePool(pool).getUserAccountData(user);
        return hf;
    }
}
