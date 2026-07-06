# Modified Bellman-Ford Arbitrage Detection

## Propósito
Detecta ineficiencias de precios cíclicas (arbitraje) en el grafo de tokens mediante la identificación de ciclos negativos, utilizando pesos logarítmicos negativos de los rates de cambio con comisiones.

## Principios matemáticos
Un ciclo `c = (v1, v2, ..., vk, v1)` es rentable si el producto de los ratios de cambio a través de los pools es > 1.
Tomando logaritmos negativos: `sum(-ln(R_i)) < 0`. 
Esto reduce el problema a encontrar ciclos negativos en un grafo dirigido.

## Algoritmos incluidos
- Bellman-Ford (Modificado para relajar aristas usando rates en lugar de distancias puras y detenerse al encontrar ciclos negativos).
- SPFA (Shortest Path Faster Algorithm) iterativo.
