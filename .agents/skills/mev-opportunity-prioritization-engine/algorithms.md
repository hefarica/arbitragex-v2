## Prioritization Scoring Algorithm
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
