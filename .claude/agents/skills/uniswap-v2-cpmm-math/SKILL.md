# Uniswap V2 CPMM Math

## Propósito
Proveer las primitivas matemáticas exactas para simular trades, calcular slippage y encontrar tamaños de trade óptimos en pools de Constant Product Market Maker (x * y = k) bajo el estándar Uniswap V2, considerando las tarifas dinámicas de los DEX forks (SushiSwap, PancakeSwap).

## Conocimiento esencial
El modelo `x * y = k` asume liquidez infinita teórica, pero un slippage asintótico. Para que un trade sea validado sin consultar el estado on-chain constantemente, el searcher debe calcular localmente el `amountOut` exacto dado un `amountIn`, considerando la comisión (fee) del pool (típicamente 0.3%).

## Principios matemáticos
Dadas reservas `R_in` y `R_out`, y un monto de entrada `A_in` con comisión `f` (ej. 0.003):
`A_in_with_fee = A_in * (1 - f)`
`A_out = (A_in_with_fee * R_out) / (R_in + A_in_with_fee)`

Para calcular el input exacto necesario para un output deseado:
`A_in = (R_in * A_out) / ((R_out - A_out) * (1 - f)) + 1`

## Integración con ARBITRAGEX
El crate `dex_math` en Rust implementará estas ecuaciones usando aritmética de punto fijo (U256) para evitar desbordamientos, garantizando consistencia absoluta con la EVM.
