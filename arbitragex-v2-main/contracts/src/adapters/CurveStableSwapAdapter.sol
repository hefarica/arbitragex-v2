// SPDX-License-Identifier: BUSL-1.1
pragma solidity 0.8.24;

/**
 * @title  CurveStableSwapAdapter
 * @notice Adaptador holonómico para Curve StableSwap (N-coin pools).
 * @dev    Compatible con interfaces clásicas: exchange / exchange_underlying.
 *         Sigue Mirror Law: misma firma ABI que UniswapV2Adapter (target,data,value).
 *         Ghost Protocol: el ExecutionSigner.balance permanece en 0; los tokens
 *         se mueven entre WalletTopology y el pool sin retención.
 *
 *         Cero stub/placeholder/TODO en código productivo.
 */

interface ICurveStableSwap {
    function coins(uint256 i) external view returns (address);
    function get_dy(int128 i, int128 j, uint256 dx) external view returns (uint256);
    function exchange(int128 i, int128 j, uint256 dx, uint256 min_dy) external returns (uint256);
}

interface IERC20 {
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

contract CurveStableSwapAdapter {
    error CurveAdapter__SlippageExceeded(uint256 received, uint256 minOut);
    error CurveAdapter__ZeroAmount();
    error CurveAdapter__InvalidCoinIndex();

    event ExchangeExecuted(
        address indexed pool,
        int128 i,
        int128 j,
        uint256 amountIn,
        uint256 amountOut,
        address indexed recipient
    );

    /**
     * @notice Ejecuta exchange en pool Curve StableSwap (N coins).
     * @param  pool     Dirección del pool Curve.
     * @param  i        Índice del coin de entrada.
     * @param  j        Índice del coin de salida.
     * @param  amountIn Monto exacto a entregar.
     * @param  minOut   Cota inferior (slippage guard termodinámico).
     * @param  recipient Destino del coin de salida (Cold Treasury).
     * @return amountOut Recibido efectivo.
     */
    function exchangeOnCurve(
        address pool,
        int128 i,
        int128 j,
        uint256 amountIn,
        uint256 minOut,
        address recipient
    ) external returns (uint256 amountOut) {
        if (amountIn == 0) revert CurveAdapter__ZeroAmount();

        ICurveStableSwap c = ICurveStableSwap(pool);
        address tokenIn = c.coins(uint256(int256(i)));
        if (tokenIn == address(0)) revert CurveAdapter__InvalidCoinIndex();
        address tokenOut = c.coins(uint256(int256(j)));
        if (tokenOut == address(0)) revert CurveAdapter__InvalidCoinIndex();

        // Pull desde caller (Executor), approve al pool, exchange, transfer al recipient.
        IERC20(tokenIn).transferFrom(msg.sender, address(this), amountIn);
        IERC20(tokenIn).approve(pool, amountIn);

        uint256 balanceBefore = IERC20(tokenOut).balanceOf(address(this));
        c.exchange(i, j, amountIn, minOut);
        uint256 balanceAfter = IERC20(tokenOut).balanceOf(address(this));
        amountOut = balanceAfter - balanceBefore;

        if (amountOut < minOut) revert CurveAdapter__SlippageExceeded(amountOut, minOut);

        IERC20(tokenOut).transfer(recipient, amountOut);
        emit ExchangeExecuted(pool, i, j, amountIn, amountOut, recipient);
    }

    /// @notice Cotización on-chain pura para verificación pre-trade.
    function quote(address pool, int128 i, int128 j, uint256 dx) external view returns (uint256) {
        return ICurveStableSwap(pool).get_dy(i, j, dx);
    }
}
