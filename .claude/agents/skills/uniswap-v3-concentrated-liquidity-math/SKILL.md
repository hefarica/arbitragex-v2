# Uniswap V3 Concentrated Liquidity Math

## Propósito
Calcular con precisión de wei (1e-18) el output de un swap a través de posiciones de liquidez concentrada (ticks) sin simulación on-chain costosa.

## Conocimiento esencial
A diferencia de V2, V3 distribuye liquidez `L` en rangos de precios (ticks). El precio está representado como la raíz cuadrada del precio (`sqrtPriceX96`).

## Fórmulas
`L = delta_y / delta_sqrt_P`
`L = delta_x / (1/sqrt(P1) - 1/sqrt(P2))`
