# MEV Opportunity Prioritization Engine

## Propósito
Esta skill define el núcleo decisional de ArbitrageX. Convierte un espacio de búsqueda masivo de ciclos de arbitraje en una cola priorizada de ejecución, garantizando que el sistema ataque primero las oportunidades con mayor valor esperado (EV), probabilidad de aterrizaje (Landing Probability) y menor riesgo de reversión.

## Cuándo usarla
- En el pipeline principal de Rust (`searcher-rs`), inmediatamente después de detectar ciclos negativos en el grafo de pools.
- Antes de enviar las oportunidades al simulador EVM (`revm`), como un filtro pre-simulación para descartar basura algorítmica.
- En la UI (Next.js) para ordenar el `Live MEV Feed` según criticidad.

## Cuándo no usarla
- No usar para arbitraje inter-exchange (CEX/DEX) puro donde la latencia de red CEX domina.
- No usar en rollups L2 con secuenciadores FIFO sin un modelo de latencia determinista (requiere modificaciones para L2-MEV).

## Conocimiento esencial
El motor de priorización abandona el concepto ingenuo de "beneficio bruto máximo". En ecosistemas oscuros (Dark Forests) como Ethereum, el profit bruto no significa nada sin la *probabilidad de ejecución*. Un profit de $10 con 99% de landing probability es superior a un profit de $10,000 con 0.1% de landing (que probablemente sea una trampa o "bait").

## Principios matemáticos
El modelo central es:
`Opportunity Score = (NetExpectedProfit * LandingProbability * StateFreshness * LiquidityConfidence * ExecutionAtomicity) / (ComputationalCost * ReversalRisk * SlippageRisk * GasVolatilityRisk * TokenRisk)`

## Algoritmos incluidos
- Prioritization Scoring Algorithm (O(1) por oportunidad).
- Dynamic Thresholding Algorithm.

## Datos reales requeridos
- `GrossOutput`, `InputAmount` (DEX Math).
- `GasCost`, `Bribe` (Flashbots/Relays).
- `pool_update_frequency` y `mempool_seen_at`.
- `token_safety_factor` (listas blancas, honeypot checks).

## Pipeline operativo
1. **Ingesta:** Recibe `Opportunity` del Graph Engine.
2. **Scoring Base:** Aplica factores deterministas (Profit, Slippage).
3. **Scoring Dinámico:** Aplica factores estocásticos (Landing, Freshness).
4. **Filtrado:** Descarta score < `min_profit_threshold`.
5. **Encolamiento:** Inserta en PriorityQueue (Min-Max heap en Rust).

## Integración con ARBITRAGEX
- **Rust MEV Engine:** Implementado como el Trait `Prioritizer<T>` en el `mempool-listener`.
- **Redis:** Usa `ZADD` (Sorted Sets) para cachear el Top 100 en memoria caliente.
- **Frontend Next.js:** Lee el ZSET de Redis y renderiza los badges de "Riesgo/Score" en la tabla.

## Señales de entrada
- `RouteCandidate` struct.
- `GasEstimator` struct.
- `BlockState` context.

## Señales de salida
- `ScoredOpportunity` struct, listo para `simulate()`.

## Scoring recomendado
- > 90: Crítico, simular en hilo prioritario.
- 50-89: Normal, encolar en pool de simulación.
- < 50: Descartar / Guardar en BD para backtesting.

## Validaciones
- Si `LandingProbability` == 0, Score debe ser 0.
- Si `NetExpectedProfit` < 0, rechazo inmediato (Fail-Fast).

## Fallos comunes
- **Stale Data Amplification:** Un estado viejo de un pool causa un Score masivo. Mitigación: Penalización agresiva en `StateFreshness`.

## Referencias
- Flashbots Docs: Bidding and Bundle Prioritization.
- EigenPhi: MEV Profitability distributions.
