# SKILL 006 — Optimización estocástica bajo incertidumbre

## 1. Propósito superior
Incorporar la volatilidad del mundo real y la latencia asíncrona a los cálculos de viabilidad. Reconoce que el arbitraje nunca se ejecuta sobre el precio que se ve, sino sobre el precio *futuro* en el instante en que la orden llega al matcher. Utiliza cálculo de probabilidades y simulación estocástica para penalizar oportunidades que matemáticamente son viables, pero que con alta probabilidad desaparecerán antes de su confirmación.

## 2. Nivel de conocimiento requerido
Máster/PhD en Matemática Cuantitativa, Procesos Estocásticos o Finanzas Computacionales. Dominio de Cadenas de Markov, Movimiento Browniano Geométrico intradiario, Modelado de Latencia de Red, Cópulas para correlación cruzada de latencia-precio, e inferencia Bayesiana aplicada a HFT.

## 3. Capacidades principales
1. Modelado de distribución de latencia de red hacia los distintos exchanges (Media, Varianza, Jitter, Cola pesada al 99p).
2. Cálculo de volatilidad intradiaria microscópica (Micro-volatility a nivel de milisegundos).
3. Simulación de probabilidad de decaimiento del Order Book (Order Book Imbalance decay).
4. Asignación de un "Intervalo de Confianza" al ROI neto proyectado.
5. Ajuste penalizador (Discount Factor) basado en el tiempo de confirmación requerido (ej. bloques de Ethereum vs BSC vs CEX matching).
6. Implementación de una función de penalidad asimétrica para la pata de riesgo (la última pata de un trade multi-leg).
7. Cálculo del *Expected Shortfall* (CVaR) para arbitrajes con riesgo de ejecución parcial.
8. Calibración continua del modelo estocástico basada en la tasa de fallo/éxito de los últimos N trades ejecutados.
9. Integración de la varianza del costo del gas (Base Fee Volatility) para arbitraje DEX.
10. Generación de Scores de Riesgo Normalizados (0 a 100) para bloqueo dinámico de operaciones.

## 4. Entradas requeridas
- `theoretical_net_roi`: ROI calculado por Skill 1 asumiendo latencia cero.
- `network_latency_stats`: Histórico reciente de latencia y jitter hacia los endpoints objetivo.
- `asset_micro_volatility`: Varianza del precio del activo en la ventana de T-1 segundo.
- `order_book_imbalance`: Ratio de liquidez Bid vs Ask en nivel 1 y 2.
- `gas_volatility_index`: Histórico de saltos de gas de los últimos 10 bloques.

## 5. Salidas esperadas
- `stochastic_expected_roi`: El ROI esperado ponderado por todas las probabilidades de fallo.
- `execution_probability`: Probabilidad % de que la orden cruce intacta.
- `cvar_95`: Cuánto dinero se pierde en el peor 5% de los escenarios de deslizamiento por latencia.
- `is_safe_to_execute`: Booleano final tras someter el ROI teórico al estrés probabilístico.

## 6. Reglas inmutables
- Nunca asumir que el precio visto en `T=0` será idéntico en `T+Latencia`. Siempre debe existir una banda de error.
- El modelo estocástico no puede incrementar el ROI esperado; solo puede reducirlo o mantenerlo (es un modelo estrictamente defensivo).
- Si la volatilidad microscópica de un activo excede el margen del arbitraje, la operación es bloqueada independientemente de la latencia.
- Todo parámetro estadístico debe recalibrarse con datos reales de la red local, nunca hardcodear promedios de latencia o varianza.

## 7. Algoritmos o métodos que debe conocer
- Monte Carlo Simulation simplificado (Random Walks en HFT).
- Aproximación Analítica (Taylor Expansion) del Valor Esperado bajo Movimiento Browniano Geométrico.
- Análisis de decaimiento de Poisson para llegada de trades competitivos.
- Value at Risk (VaR) y Conditional Value at Risk (CVaR) paramétrico.

## 8. Fórmulas críticas
- **Probabilidad de Supervivencia del Profit**: `P(Profit > 0) = Φ( (ROI_Teorico - Volatilidad * sqrt(Latencia)) / (Volatilidad * sqrt(Latencia)) )` (Distribución Normal Estándar de la deriva).
- **ROI Esperado Estocástico**: `E[ROI] = ROI_Teorico * P_Exito + ROI_Fallo_Estimado * (1 - P_Exito)`
- **Penalidad por Imbalance**: `Imbalance_Discount = exp(-k * |Bid_Vol - Ask_Vol|)`
- **Varianza de Latencia**: Varianza exponencialmente suavizada (EWMA) del RTT de los WebSockets.

## 9. Casos extremos
- Interrupción masiva de AWS que aumenta la varianza de la latencia a 5 segundos repentinamente.
- Evento macro (Noticia de la FED) que cuadruplica la micro-volatilidad en un segundo.
- "Slippage Flash": Cuando la probabilidad de éxito indica 99% pero la profundidad del book se evapora subrepticiamente (Spoofing).
- Congestión extrema de RPCs de blockchain.

## 10. Validaciones obligatorias
- PRE: Validar que existan suficientes datos en la ventana (ej. N > 100 ticks) para calcular una varianza estadística significativa.
- CÁLCULO: Mantener un límite inferior para el ROI esperado estocástico; si baja a cero, truncar el cálculo y abortar.
- POST: Confirmar que `cvar_95` no excede la pérdida máxima tolerable (Max Drawdown Limit) de la cuenta.

## 11. Criterios de aprobación
- `stochastic_expected_roi >= config.min_stochastic_roi`
- `execution_probability >= config.min_exec_probability` (ej. 85%).
- El riesgo de cola (CVaR 95) es inferior al buffer de ganancia del mes en curso.

## 12. Criterios de rechazo
- La volatilidad es tan alta que el margen de arbitraje está dentro de 1 desviación estándar (1 Sigma) del ruido del mercado.
- La varianza de latencia indica que el riesgo de "Partial Fill" es inminente.

## 13. Riesgos que mitiga
- Riesgo de Ilusión Óptica de Mercado: Operar señales donde el profit nominal solo existe porque la latencia del operador es más lenta que el mercado.
- Toxicidad de Ejecución (Adverse Selection): Comprar de un creador de mercado justo en el milisegundo en que el precio "real" ya ha colapsado.

## 14. Integración con otras skills
- Consume outputs de: Matemática de Arbitraje Neto (Skill 1), Timestamp Synchronization (Skill 40).
- Provee datos a: Risk Engine Institucional (Skill 41) y Circuit Breakers (Skill 44).

## 15. Modelo de datos sugerido
```json
{
  "StochasticEval": {
    "theoretical_roi_bps": 15,
    "execution_prob_pct": 82.5,
    "stochastic_roi_bps": 9.2,
    "cvar_95_bps": -25,
    "volatility_penalty_applied": true,
    "approved": false,
    "reason": "Stochastic ROI below 10bps threshold"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Librería de cálculo cuantitativo rápida embebida en el Execution Pipeline. Se invoca sincronamente tras el cálculo algebraico.

## 17. Logs obligatorios
- `[INFO] Stochastic eval: Theoretical ROI=0.15%. ExecProb=88%. Stochastic ROI=0.11%. Approved.`
- `[WARN] Rejected by Stochastic Optimizer: Micro-volatility (X) exceeds safety margins for Latency (Y ms).`

## 18. Métricas obligatorias
- `stochastic_penalty_bps_average`
- `execution_probability_prediction_error` (Crucial: si el modelo predice 90% de éxito, pero solo el 50% de los trades pasan, el modelo debe auto-ajustarse).
- `cvar_exceedance_events` (Cuántas veces la pérdida real superó el peor 5% estimado).

## 19. Tests unitarios
- Escenario base: Latencia baja y volatilidad baja (Stochastic ROI debe ser casi igual al teórico).
- Escenario tóxico: Latencia de 500ms y volatilidad extrema (Debe castigar severamente el ROI hasta negativizarlo y rechazar).
- Cálculo paramétrico normalizado de CVaR con vectores inyectados pre-calculados.

## 20. Tests de integración
- Consumir métricas reales del módulo de Observabilidad (latencias recientes) para penalizar una oportunidad simulada de Skill 1.

## 21. Tests E2E
- Demostrar que un arbitraje minúsculo de 0.02% pasa en tiempos de paz (latencia/vol baja) pero es bloqueado firmemente el día del NFP (Non-Farm Payrolls) por alta varianza.

## 22. Checklist de producción
- [ ] Módulo de cálculo estadístico altamente optimizado (usar GSL C++ o numba/Rust).
- [ ] Bucle de feedback cerrado que castigue o alivie el factor de penalidad basado en el P&L (Profit and Loss) empírico del bot.
- [ ] Timeout de 500 microsegundos para toda la evaluación probabilística.

## 23. Ejemplo de configuración no hardcodeada
```yaml
stochastic_optimizer:
  min_execution_prob_pct: 85.0
  latency_sigma_multiplier: 1.645   # 90% confidence interval
  volatility_lookback_ms: 5000
  imbalance_penalty_weight: 0.2
```

## 24. Ejemplo de pseudocódigo
```python
import math

def calculate_stochastic_viability(theoretical_roi, market_volatility, latency_ms, config):
    # Convert parameters to compatible time horizons
    horizon_factor = math.sqrt(latency_ms / 1000.0)
    expected_drift = theoretical_roi
    uncertainty_band = market_volatility * horizon_factor * config.sigma_multiplier
    
    # Worst case slippage under current volatility
    pessimistic_roi = expected_drift - uncertainty_band
    
    # Calculate Gaussian probability of positive execution
    z_score = expected_drift / (market_volatility * horizon_factor + 0.000001)
    probability_of_success = normal_cdf(z_score)
    
    # Stochastic Expected Value
    stochastic_expected_roi = expected_drift * probability_of_success + pessimistic_roi * (1 - probability_of_success)
    
    approved = (stochastic_expected_roi > config.min_acceptable_roi) and (probability_of_success > config.min_prob)
    
    return StochasticReport(approved, stochastic_expected_roi, probability_of_success, pessimistic_roi)
```

## 25. Criterio final de excelencia
Esta skill es de excelencia absoluta si el sistema logra auto-ajustarse ante la congestión de la red y el ruido del mercado, logrando que el "Win Rate" de las operaciones ejecutadas jamás baje del 95%, bloqueando agresivamente todo trade dudoso mediante la fuerza ineludible de la estadística Bayesiana.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Suposición de normalidad de rendimientos que subestima las "colas gordas" (fat tails).
- Dependencias: Observabilidad de red, Medidor de micro-volatilidad.
- Próxima skill: Cálculo diferencial aplicado a slippage (Skill 7).
