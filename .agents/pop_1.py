import os

skill_dir = r"C:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\.agents\skills\mev-opportunity-prioritization-engine"

skill_md = """# MEV Opportunity Prioritization Engine

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
"""

algorithms_md = """## Prioritization Scoring Algorithm
### Problema que resuelve
Ordena miles de oportunidades encontradas en un bloque en < 1ms para saturar el simulador de manera inteligente.
### Inputs
- `O`: Objeto de oportunidad.
- `W`: Vector de pesos estáticos de la matriz de Google Sheets.
### Outputs
- `score` flotante normalizado [0, 100].
### Pseudocódigo
```python
def calculate_score(opp, weights):
    profit_net = opp.gross - opp.gas - opp.bribe
    if profit_net <= 0: return 0.0
    
    score = profit_net
    score *= calculate_landing_prob(opp.competition, opp.bribe_ratio)
    score *= math.exp(-opp.age_ms / weights.decay_factor)
    score /= max(1.0, opp.slippage_risk * weights.slip_penalty)
    
    return min(100.0, max(0.0, score * weights.global_multiplier))
```
### Complejidad temporal
O(1) computaciones aritméticas simples por ruta.
### Complejidad espacial
O(1).
### Integración ARBITRAGEX
Codificado en `crates/core/src/scoring/engine.rs`.
"""

formulas_md = """# Fórmulas del Motor de Priorización

## NetExpectedProfit
`NetExpectedProfit = GrossOutput - InputAmount - GasCost - Bribe - FlashLoanFee - DEXFees - SlippageLoss - FailureCost`
- **GrossOutput**: Retorno bruto de la operación swap final.
- **Bribe**: Tip al validator (ej. `profit * 0.9`).
- **FailureCost**: Costo de gas si la tx revierte.

## Opportunity Score (General)
`Score = (NetExpectedProfit * LandingProbability * StateFreshness * LiquidityConfidence * ExecutionAtomicity) / (ComputationalCost * ReversalRisk * SlippageRisk * GasVolatilityRisk * TokenRisk)`
- **Supuestos**: Las variables están normalizadas al rango [0.1, 1.0] o [1.0, 10.0] según si son penalizaciones o multiplicadores.
"""

impl_notes_md = """# Implementation Notes

- **Rust vs Python**: En producción, esto DEBE estar en Rust. Python es demasiado lento para iterar 100,000 caminos y priorizarlos antes de que llegue el siguiente bloque.
- **ZSET en Redis**: Para pasar el top 100 al dashboard Next.js, usa un Sorted Set en Redis con el Score como peso. El Edge Worker solo hace `ZRANGE opportunities -100 -1 REV`.
- **EIP-1559**: El `GasCost` base cambia dinámicamente; el motor de scoring debe escuchar los bloques entrantes para recalibrar el `base_fee`.
"""

val_tests_md = """# Validation Tests

## Static validation
- Todos los pesos de la configuración de Google Sheets suman 1.0, o se normalizan antes de aplicar.

## Runtime validation
- **Prueba de Invariante de Pérdida:** Crear un test unitario donde `Bribe + Gas > Profit`. El Score debe afirmar `0.0`.
- **Prueba de Degradación (Time-decay):** Una oportunidad cacheada durante 12 segundos (1 bloque Ethereum) debe ver su score penalizado severamente si la liquidez en los DEX es altamente volátil.
"""

integration_md = """# Integración con ArbitrageX

1. **Rust (`engine/src/searcher`)**: Instancia el `PrioritizationEngine` cargando pesos desde la configuración. Recibe flujos del `GraphBuilder` y empuja a la cola de `Simulator`.
2. **PostgreSQL**: Registra el `Score` computado junto con la simulación resultante para machine learning futuro (predicción de discrepancia entre score heurístico y profit real simulado).
"""

failure_md = """# Failure Modes

1. **Infinity Score**: Si una métrica de riesgo como `TokenRisk` evalúa a 0 y no está validada, causa división por cero (`NaN` o `Inf`).
   *Mitigación*: Siempre hacer `max(epsilon, risk_metric)`.
2. **False Positives (Bait pools)**: Tokens con liquidez falsa (honeypots) generan profit esperado astronómico, saturando el Top de la cola.
   *Mitigación*: Filtrado estricto `token_safety_factor < 0.1` pone el score en cero.
"""

with open(os.path.join(skill_dir, "SKILL.md"), "w", encoding='utf-8') as f: f.write(skill_md)
with open(os.path.join(skill_dir, "algorithms.md"), "w", encoding='utf-8') as f: f.write(algorithms_md)
with open(os.path.join(skill_dir, "formulas.md"), "w", encoding='utf-8') as f: f.write(formulas_md)
with open(os.path.join(skill_dir, "implementation_notes.md"), "w", encoding='utf-8') as f: f.write(impl_notes_md)
with open(os.path.join(skill_dir, "validation_tests.md"), "w", encoding='utf-8') as f: f.write(val_tests_md)
with open(os.path.join(skill_dir, "integration_arbitragex.md"), "w", encoding='utf-8') as f: f.write(integration_md)
with open(os.path.join(skill_dir, "failure_modes.md"), "w", encoding='utf-8') as f: f.write(failure_md)

print("Generated mev-opportunity-prioritization-engine")
