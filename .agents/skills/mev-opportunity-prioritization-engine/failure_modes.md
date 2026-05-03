# Failure Modes

1. **Infinity Score**: Si una métrica de riesgo como `TokenRisk` evalúa a 0 y no está validada, causa división por cero (`NaN` o `Inf`).
   *Mitigación*: Siempre hacer `max(epsilon, risk_metric)`.
2. **False Positives (Bait pools)**: Tokens con liquidez falsa (honeypots) generan profit esperado astronómico, saturando el Top de la cola.
   *Mitigación*: Filtrado estricto `token_safety_factor < 0.1` pone el score en cero.
