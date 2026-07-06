# Curve StableSwap Arbitrage Math

## Propósito
Modelar la invariante StableSwap de Curve Finance para detectar discrepancias sutiles de precio entre stablecoins o activos pegados (ej. stETH/ETH), donde la liquidez es extremadamente profunda cerca de la paridad.

## Conocimiento esencial
La invariante de Curve combina Constant Sum (liquidez densa en 1:1) y Constant Product (protección de liquidez en los extremos) mediante un parámetro de amplificación `A`.

## Principios matemáticos
Invariante de Curve:
`A * n^n * sum(x_i) + D = A * n^n * D + D^(n+1) / (n^n * prod(x_i))`
El cálculo iterativo (Newton-Raphson) es obligatorio para predecir los resultados de los swaps de Curve on-chain.
