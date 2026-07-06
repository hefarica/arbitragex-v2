# CFMM Optimal Routing

## Propósito
Calcula la ruta matemática óptima a través de múltiples Constant Function Market Makers (CFMMs) maximizando el profit final. Transforma el ruteo DEX en un problema de optimización convexa.

## Conocimiento esencial
En un grafo de liquidez con N pools, encontrar la ruta óptima requiere modelar cada pool por su función invariante. Para Uniswap V2 (CPMM), la invariante es `x * y = k`. El routing óptimo no solo busca la mejor ruta discreta, sino que divide el capital a través de múltiples rutas paralelas para reducir el slippage global.

## Integración con ARBITRAGEX
El `GraphBuilder` en Rust traduce las reservas de memoria a un grafo. El `OptimalRouter` evalúa rutas usando aproximaciones de búsqueda de sección dorada (Golden Section Search) para encontrar el monto de entrada óptimo.
