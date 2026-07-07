# SKILL 010 — Análisis de sensibilidad financiera

## 1. Propósito superior
Proyectar de manera determinista cómo responderá la rentabilidad neta de una oportunidad si las variables del entorno sufren estrés. Es un modelo defensivo "What-If" que pre-calcula los peores escenarios (Gas Spikes, Slippage asimétrico, Fee tier demotion) en microsegundos para asegurar que el margen de seguridad de la operación es robusto frente a perturbaciones violentas del sistema.

## 2. Nivel de conocimiento requerido
Máster/Experto Senior en Ingeniería Financiera y Gestión de Riesgos Cuantitativa. Dominio del cálculo de derivadas parciales multivariables (Griegas financieras aplicadas a Spot/Arbitraje), matrices Jacobianas, y simulaciones de escenarios de estrés paramétrico a alta velocidad.

## 3. Capacidades principales
1. Cálculo de Derivadas Parciales de Profit respecto a cada input (dProfit/dGas, dProfit/dSlippage, dProfit/dTime).
2. Simulación de perturbación de +10%, +50% y +300% en el Base Fee de la blockchain objetivo.
3. Evaluación del impacto de una caída repentina de Nivel VIP de comisiones en el CEX.
4. Estimación de resiliencia del Break-Even Point (Cuántos puntos de slippage inesperado pueden tolerarse antes de incurrir en pérdida).
5. Trazado del "Frente de Ruina" (Ruin Boundary): La combinación exacta de eventos que convierte el arbitraje en pérdida crítica.
6. Construcción de Matrices de Sensibilidad para rutas multi-leg.
7. Cálculo de Elasticidad de la Liquidez (Cómo responde el precio al absorber 2x el volumen propuesto por error).
8. Generación de Vectores de Sensibilidad asimétricos (El slippage siempre empeora, nunca mejora).
9. Mapeo de riesgo de "Leg Exposure" (Si la pata 1 ejecuta y la pata 2 falla temporalmente, cuánto fluctúa el precio en 10 segundos).
10. Cuantificación del "Margin of Safety" (Margen de Seguridad de Benjamin Graham aplicado a HFT).

## 4. Entradas requeridas
- `mathematical_report`: Modelo base exacto generado por Skill 1.
- `current_gas_base_fee`: Costo real de la red en ese instante.
- `historical_max_gas`: Registro del mayor gas visto en los últimos 100 bloques.
- `current_volatility_bps`: Movimiento medio por segundo del activo en los últimos 60 segundos.

## 5. Salidas esperadas
- `margin_of_safety_bps`: Buffer de ganancia neta libre de sensibilidad.
- `max_tolerable_gas`: Precio máximo de GWEI que la operación soporta antes de quedar a pérdida.
- `max_tolerable_slippage`: Slippage adicional máximo absorbible.
- `sensitivity_score`: Clasificación global del trade (A: Inquebrantable, C: Frágil).
- `is_robust`: Booleano final indicando aprobación estructural.

## 6. Reglas inmutables
- Toda oportunidad cuyo margen de seguridad (diferencia entre profit proyectado y el peor escenario de 1 sigma) sea menor o igual a cero, se rechaza sin contemplación.
- Nunca calcular sensibilidad usando perturbaciones simétricas (ej. gas baja 10% y sube 10%); el mercado adverso siempre empeora las métricas de ejecución.
- La evaluación de sensibilidad debe consumir menos de 0.5 milisegundos de cómputo adicional.
- Si el "max_tolerable_gas" es inferior a la desviación estándar actual del gas, abortar operación.

## 7. Algoritmos o métodos que debe conocer
- Jacobiana para análisis de perturbación de primer orden en sistemas multivariables.
- Simulación de Escenarios de Estrés (Stress Testing paramétrico).
- Método Delta-Normal para cálculo de exposición lineal.
- Análisis de Componentes Principales (PCA) simplificado para identificar el factor de riesgo más pesado de la ruta.

## 8. Fórmulas críticas
- **Sensibilidad a Variable Xi**: `S(Xi) = (Profit(Xi + Delta) - Profit(Xi)) / Delta`
- **Break-Even de Slippage**: `BE_Slippage = Net_Profit_USD / Volume_USD` (Puntos base máximos de error).
- **Riesgo Multivariable Total (Worst Case)**: `WC_Profit = Profit - |dProfit/dGas| * MaxDeltaGas - |dProfit/dSlippage| * MaxDeltaSlippage`
- **Margin of Safety**: `MoS = WC_Profit / Profit_Teorico`

## 9. Casos extremos
- Rutas altamente dependientes de un fee transaccional mínimo, donde un pequeño pico de gas convierte un trade estelar en una pérdida neta devastadora.
- Trades de inmenso volumen y escaso margen: Toleran infinito incremento de gas, pero no soportan ni 0.01% de error en el slippage.
- Trades de bajísimo volumen y gran margen: No les importa el slippage de la red, pero mueren con el mínimo incremento en comisiones fijas de retiro.

## 10. Validaciones obligatorias
- PRE: Asegurar que el reporte matemático base tenga ganancias netas positivas.
- CÁLCULO: Validar la linearidad de la perturbación (algunos AMMs escalan exponencialmente, requieren cálculo exacto, no solo el Delta).
- POST: Si `Margin of Safety < 0.2` (20% del profit se borra con el ruido ambiental), lanzar alerta y requerir aprobación superior (Skill 41).

## 11. Criterios de aprobación
- `WC_Profit` (Worst Case Profit) de simulación +2 Sigma es `>= 0`.
- La operación es robusta: un retraso de 10 segundos no volatiza la viabilidad económica.

## 12. Criterios de rechazo
- El Margin of Safety es insuficiente para absorber la volatilidad estándar de las patas implicadas.
- La operación califica como "Frágil" (Sensitivity Score D o F).

## 13. Riesgos que mitiga
- Riesgo de "Penny picking in front of a steamroller" (Recoger centavos frente a una aplanadora): Evita tomar ganancias mínimas frágiles que destruyen el fondo ante un evento menor.
- Falsas seguridades de ROI bruto: Expone cómo las comisiones ocultas castigan desproporcionadamente ciertas configuraciones.

## 14. Integración con otras skills
- Funciona como una envoltura (Wrapper) post-análisis de la Matemática Base (Skill 1) y Optimizador de Tamaño (Skill 2).
- Informa límites al motor de Ejecución y Circuit Breakers (Skill 44).

## 15. Modelo de datos sugerido
```json
{
  "SensitivityReport": {
    "base_net_profit_usd": 12.50,
    "margin_of_safety_pct": 0.45,
    "break_even_gas_gwei": 140,
    "break_even_slippage_bps": 15,
    "worst_case_profit_usd": 2.10,
    "fragility_vector": "high_gas_sensitivity",
    "is_robust": true
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Librería matemática compilada, instanciada atómicamente por la cadena de evaluación antes de llamar a las colas de ejecución.

## 17. Logs obligatorios
- `[INFO] Sensitivity Analysis: Trade robust. Can tolerate 5x gas spike and 2x slippage. Margin of Safety: 65%.`
- `[WARN] Sensitivity rejected trade. A 10% increase in Gas removes 120% of net profit. Fragile state.`

## 18. Métricas obligatorias
- `average_margin_of_safety_executed_trades`.
- `fragility_rejection_count`.
- `break_even_prediction_accuracy` (Comparado post-trade con el mercado adverso real).

## 19. Tests unitarios
- Perturbación Delta: Inyectar un aumento simulado de gas, asegurar que dProfit/dGas retorna la derivada correcta.
- Verificación del Break-Even: Si el slippage forzado iguala el `break_even_slippage_bps`, el WC_Profit debe ser exactamente cero.
- Margen asimétrico: Comprobar que una reducción de fees del mercado (caso feliz) no aumenta la puntuación de robustez base (el sistema solo debe estresarse hacia lo negativo).

## 20. Tests de integración
- Ejecutar sensibilidad consumiendo dinámicamente las fluctuaciones de gas oracle reales durante un bloque muy congestionado.

## 21. Tests E2E
- El agente simula una ruta matemáticamente dorada pero con alto riesgo en gas de L1 (Ethereum), el módulo de sensibilidad prevé el cuello de botella, bloquea la orden, y luego se observa cómo el gas efectivamente hizo spike 3 segundos después.

## 22. Checklist de producción
- [ ] Incorporación de derivadas exactas pre-calculadas en código para `dProfit/dGas` sin requerir recalcular iteraciones (Cálculo O(1)).
- [ ] Parametrización estricta de las cotas máximas de perturbación (+3 desviaciones estándar calculadas en tiempo real).
- [ ] Uso exclusivo de variables netas.

## 23. Ejemplo de configuración no hardcodeada
```yaml
sensitivity_analysis:
  stress_multiplier_gas: 2.0         # Simulate gas at 200%
  stress_multiplier_slippage: 1.5    # Simulate slippage at 150%
  min_margin_of_safety_pct: 0.15     # Keep at least 15% of profit under stress
```

## 24. Ejemplo de pseudocódigo
```python
def analyze_sensitivity(math_report, env_context, config):
    # Stress Gas
    stressed_gas_cost = math_report.gas_cost_usd * config.stress_multiplier_gas
    
    # Stress Slippage
    stressed_slippage = math_report.slippage_usd * config.stress_multiplier_slippage
    
    # Calculate Worst Case
    wc_profit = math_report.gross_profit - stressed_gas_cost - stressed_slippage - math_report.fixed_fees
    
    margin_of_safety = wc_profit / math_report.base_net_profit if math_report.base_net_profit > 0 else 0
    
    break_even_gas = (math_report.gross_profit - math_report.slippage_usd - math_report.fixed_fees) / current_eth_price()
    
    is_robust = wc_profit >= 0 and margin_of_safety >= config.min_margin_of_safety_pct
    
    return SensitivityReport(is_robust, margin_of_safety, wc_profit, break_even_gas)
```

## 25. Criterio final de excelencia
La skill debe funcionar como el escudo definitivo de la rentabilidad: ningún trade pasa si su estructura de costos indica que el más leve suspiro en la latencia o en el libro de órdenes lo vuelve una operación deficitaria.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Predecir perturbaciones lineales en entornos no lineales extremos.
- Dependencias: Skill 1 (Matemática base).
- Próxima skill: Microestructura de mercado (Skill 11).
