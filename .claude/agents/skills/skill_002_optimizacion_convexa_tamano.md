# SKILL 002 — Optimización convexa aplicada a tamaño de operación

## 1. Propósito superior
Calcular dinámicamente el tamaño exacto del capital a inyectar en una oportunidad de arbitraje para maximizar la ganancia neta, encontrando el punto de equilibrio donde el aumento de volumen no devore la rentabilidad debido al slippage (impacto de mercado). Garantiza que nunca se envíe una orden ni muy pequeña (donde el fee absorbe el profit) ni muy grande (donde el slippage genera pérdidas).

## 2. Nivel de conocimiento requerido
Nivel Máster/PhD en Investigación de Operaciones y Optimización Matemática. Dominio del cálculo diferencial para la búsqueda de máximos globales, programación convexa, y modelado de curvas de impacto de mercado (Almgren-Chriss, modelos de raíz cuadrada, ecuaciones AMM).

## 3. Capacidades principales
1. Modelado de la curva cóncava de Profit vs. Volumen.
2. Derivación en tiempo real para encontrar el gradiente cero (punto óptimo).
3. Optimización restringida por límites de capital y límites de riesgo por operación.
4. Cálculo rápido para AMMs usando la fórmula de liquidez concentrada (Uniswap V3) y producto constante (V2).
5. Integración sobre el Order Book discreto (agregación de niveles L2) para CEXes.
6. Aplicación de penalizaciones por incertidumbre (reducción del tamaño óptimo si la volatilidad es alta).
7. Cálculo de lotes fraccionados (Iceberg orders) si la ejecución no requiere atomicidad.
8. Adaptación asimétrica a liquidez de bid y ask.
9. Detección del "Maximum Capacity" antes de incurrir en retornos marginales negativos.
10. Fallback a Kelly Criterion modificado para dimensionamiento general de portafolio.

## 4. Entradas requeridas
- `roi_bruto_esperado`: Margen teórico a size 0.
- `order_book_depth` / `pool_reserves`: Función de liquidez o arreglo de niveles.
- `fixed_costs`: Costos de transacción (Gas, withdraw fees) en moneda base.
- `proportional_costs`: Fees de maker/taker (%).
- `capital_disponible`: Límite duro superior.
- `max_risk_per_trade`: Porcentaje máximo del fondo que se permite exponer.

## 5. Salidas esperadas
- `optimal_size`: Monto exacto a operar.
- `projected_net_profit`: Beneficio esperado usando el `optimal_size`.
- `margin_of_safety`: Distancia entre el `optimal_size` y el `break_even_size`.
- `rejected`: Booleano si el `optimal_size` es inferior al mínimo permitido por el exchange.

## 6. Reglas inmutables
- No ejecutar operaciones de tamaño estático; el tamaño debe ser siempre el resultado de la función de optimización.
- No asumir liquidez infinita ni slippage lineal.
- No exceder nunca el balance real disponible de la wallet o cuenta, menos un buffer de seguridad para gas.
- No recomendar un tamaño que deje un balance "dust" (polvo) intransferible o inoperable.
- No ignorar los "Step Size" o "Min Notional" impuestos por las APIs de los exchanges.

## 7. Algoritmos o métodos que debe conocer
- Búsqueda Dorada (Golden Section Search) para optimización unidimensional rápida.
- Newton-Raphson para aproximación de raíces en funciones de impacto.
- Integración de función de costo de slippage (Slippage Cost Function).
- Algoritmo de ruteo de liquidez (Smart Order Routing optimization).

## 8. Fórmulas críticas
- **Función de Profit**: `P(v) = v * (Spread - Slippage(v) - Fees_Prop) - Fees_Fijos`
- **Condición de Primer Orden (FOC)**: `dP(v)/dv = 0` (Para encontrar el máximo).
- **Slippage AMM V2**: `S(v) = v / (Reserva + v)`
- **Tamaño Óptimo (AMM simplificado)**: `v* = sqrt(Reserva * Ganancia_esperada) - Reserva_ajustada` (Derivación específica dependiendo de la función exacta).

## 9. Casos extremos
- Función de profit monotónicamente decreciente (todo tamaño da pérdida).
- Función plana (spread cubre fees pero el costo fijo es inmenso).
- Límites de precisión ("Tick size" y "Lot size") redondean el tamaño óptimo a cero.
- Liquidez fragmentada que genera curvas no diferenciables (saltos abruptos en slippage).

## 10. Validaciones obligatorias
- PRE: Validar que los costos fijos sean finitos y conocidos.
- CÁLCULO: Asegurar que la segunda derivada `d2P/dv2 < 0` (garantiza concavidad y existencia de máximo global).
- POST: `optimal_size` ajustado a las reglas del exchange (`floor(optimal_size / step_size) * step_size`).

## 11. Criterios de aprobación
- Existe un máximo global de profit > 0 para un tamaño `v* > min_notional`.
- `v*` no compromete más del capital máximo permitido por riesgo.

## 12. Criterios de rechazo
- El costo marginal supera el ingreso marginal desde el primer dólar.
- El `optimal_size` redondeado según reglas del exchange resulta en un profit neto negativo.

## 13. Riesgos que mitiga
- "Winner's Curse" (Curse del ganador): Atrapar una oportunidad pero enviar una orden tan grande que barre el libro y resulta en pérdidas.
- Sub-optimización: Operar $10 cuando había liquidez segura para $1000 sin afectar el precio.
- Riesgo de iliquidez en la salida (quedar atrapado en una pata).

## 14. Integración con otras skills
- Recibe viabilidad teórica de Matemática de Arbitraje (Skill 1).
- Pasa el tamaño a Risk Engine Institucional (Skill 41) para validación final.
- Consume perfiles de liquidez de AMM Mathematics (Skill 24).

## 15. Modelo de datos sugerido
```json
{
  "SizeOptimization": {
    "symbol": "string",
    "theoretical_max_profit": "decimal",
    "optimal_size_base": "decimal",
    "optimal_size_quote": "decimal",
    "projected_slippage_bps": "int",
    "step_size_applied": "boolean"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Librería interna de alto rendimiento cargada en memoria (`calculate_optimal_size`). No usar HTTP para evitar latencia; usar in-process o FFI (si se llama desde Node a Rust/C++).

## 17. Logs obligatorios
- `[INFO] Optimal size calculated for route: X units. Projected net profit: Y USD.`
- `[DEBUG] Optimization curve flattened at size Z due to liquidity wall.`

## 18. Métricas obligatorias
- `optimization_compute_time_us` (Microsegundos, debe ser extremo rápido).
- `average_size_ratio` (Tamaño óptimo / Tamaño máximo posible).
- `slippage_estimation_accuracy`.

## 19. Tests unitarios
- Función cóncava estándar (debe encontrar el vértice exacto).
- Curva con liquidez infinita pero tope de capital (debe retornar el capital máximo permitido).
- Redondeo de "Step Size" (debe ajustar 10.123 a 10.1 si step es 0.1).

## 20. Tests de integración
- Test de inyección de order book real para verificar el cálculo integral discreto del slippage.

## 21. Tests E2E
- Ejecutar pipeline desde la detección, calcular tamaño óptimo, y comprobar que el payload final de la orden lleva la cantidad exacta.

## 22. Checklist de producción
- [ ] Validación estricta de `step_size` y `min_notional` del exchange configurada por par.
- [ ] Mecanismo de seguridad que limita `v*` al X% del Average Daily Volume (ADV) o del Nivel 1 del book.
- [ ] Benchmarks en Rust/C++ o JIT si se nota cuello de botella en JS/Python.

## 23. Ejemplo de configuración no hardcodeada
```json
{
  "size_optimizer": {
    "max_capital_utilization_pct": 0.85,
    "fallback_method": "linear_approximation",
    "max_iterations_newton": 50,
    "tolerance_usd": 0.01
  }
}
```

## 24. Ejemplo de pseudocódigo
```python
def find_optimal_size(order_book, fees, gas_cost, max_capital):
    def profit_function(v):
        gross = calculate_gross_return(order_book, v)
        slippage = calculate_slippage(order_book, v)
        return (v * (gross - slippage - fees.variable)) - gas_cost

    optimal_v = golden_section_search(profit_function, low=0, high=max_capital)
    
    optimal_v = apply_exchange_rules(optimal_v, exchange.step_size)
    
    if profit_function(optimal_v) <= 0:
        return 0 # Not viable
        
    return optimal_v
```

## 25. Criterio final de excelencia
Esta skill es de excelencia si el sistema jamás empuja el precio de mercado más allá de lo matemáticamente planeado y el `slippage` sufrido en la realidad tiene un error absoluto menor al 5% respecto a la proyección matemática.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Cambios de liquidez durante el microsegundo de cómputo.
- Dependencias: Skill 1, Data Feeds en tiempo real.
- Próxima skill: Álgebra lineal para rutas multi-leg (Skill 3).
