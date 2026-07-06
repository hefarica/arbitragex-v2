# SKILL 001 — Matemática de arbitraje neto

## 1. Propósito superior
Proveer el marco matemático irrefutable para determinar la viabilidad absoluta de una oportunidad de arbitraje, descontando todos los costos directos, indirectos, probabilísticos y de fricción. Esta skill actúa como la barrera fundamental entre el capital y el mercado: ninguna ejecución ocurre si la matemática subyacente no garantiza un retorno neto positivo bajo estrés.

## 2. Nivel de conocimiento requerido
Nivel PhD en Matemática Financiera y Optimización. Comprensión profunda de estructuras de costos en microestructura de mercado, modelado de slippage no lineal, impacto de mercado, estimación de gas/fees estocásticos y cálculo de valor esperado bajo incertidumbre.

## 3. Capacidades principales
1. Cálculo de ROI bruto en tiempo real para múltiples legs.
2. Descuento exacto de fees de maker/taker por exchange.
3. Estimación dinámica de slippage basada en profundidad de order book y AMM curves.
4. Modelado estocástico de gas fees (base fee + priority fee) en redes blockchain.
5. Inclusión de costos de retiro/transferencia en arbitraje cross-exchange.
6. Cálculo de impacto en el precio (Price Impact) para pools de liquidez.
7. Ajuste probabilístico por latencia de ejecución.
8. Validación de Spread Efectivo (Bid-Ask spread real descontando fees).
9. Cálculo del tamaño crítico (Break-even size) de una operación.
10. Modelado de riesgo de de-peg en stablecoins usadas como puente.
11. Descuento de funding rates en arbitraje spot-futuros.
12. Validación de "Net-Net ROI" (ROI neto después de impuestos/slippage/gas/riesgo).

## 4. Entradas requeridas
- `order_books`: L2 de todos los exchanges/pares involucrados.
- `amm_reserves`: Estado de reservas de pools DEX.
- `fees`: Estructura actual de comisiones (maker/taker/gas/withdraw).
- `gas_oracle`: Estimación de base fee y priority fee en tiempo real.
- `latencia_ms`: Latencia medida hasta el endpoint de ejecución.
- `capital_disponible`: Balance real, sin simulaciones.
- `config`: Límites de slippage máximo permitido y umbral de ROI neto mínimo.

## 5. Salidas esperadas
- `is_viable`: Booleano estricto (true/false).
- `net_roi`: Porcentaje real esperado (e.g., 0.15%).
- `gross_roi`: Porcentaje antes de costos (para logging).
- `expected_profit_usd`: Ganancia proyectada en moneda base.
- `optimal_size`: Tamaño exacto a ejecutar para maximizar ROI neto sin romper el book.
- `rejection_reason`: Razón detallada si `is_viable` es false (ej. "Negative Net ROI after Gas").
- `break_even_point`: Precio o tamaño donde la ganancia se vuelve cero.

## 6. Reglas inmutables
- No hardcodear comisiones; deben ser extraídas del endpoint o de la DB.
- No usar mocks en producción; todos los order books deben ser en tiempo real (< 100ms).
- No inventar datos; si un fee es desconocido, la oportunidad es automáticamente rechazada.
- No mostrar ROI bruto como oportunidad real.
- No ejecutar sin ROI neto positivo que supere el umbral mínimo de riesgo del usuario.
- No ejecutar si el profit esperado es menor al gas/fee consumido (dust profit).

## 7. Algoritmos o métodos que debe conocer
- Constant Product Formula para DEXes (x * y = k).
- Order Book Depth Aggregation (integración numérica del libro de órdenes).
- Kelly Criterion modificado para dimensionamiento asimétrico.
- Ecuaciones de costo de transacción de Almgren-Chriss (versión simplificada para alta frecuencia).
- Fórmulas de Black-Scholes-Merton (adaptadas para volatilidad intradiaria extrema).

## 8. Fórmulas críticas
- **ROI Bruto**: `(Precio Venta / Precio Compra) - 1`
- **Spread Efectivo**: `Bid * (1 - Fee Venta) - Ask * (1 + Fee Compra)`
- **Slippage Esperado (CEX)**: `Σ (Vol_i * Precio_i) / Σ Vol_i - Precio_Best` (Iterando hasta cubrir el tamaño de la orden).
- **Price Impact (DEX)**: `Tamaño_Entrada / (Reserva_Base + Tamaño_Entrada)`
- **Costo de Gas Total**: `(Gas_Limit * (Base_Fee + Priority_Fee)) * Precio_ETH_USD`
- **ROI Neto**: `((Monto_Final_Neto - Costo_Gas_USD - Fees_Fijos) / Capital_Inicial) - 1`

## 9. Casos extremos
- API de precios caída o desactualizada (> 500ms de antigüedad).
- Order book con agujeros de liquidez (spread > 5%).
- "Flash crashes" o mechas durante el cálculo.
- Incremento repentino de gas de 10x en el bloque actual.
- Fee de taker que cambia por pérdida de nivel VIP del usuario.
- Monedas con "Tax" interno (tokens deflacionarios).

## 10. Validaciones obligatorias
- PRE: Verificar frescura del timestamp del order book.
- PRE: Validar que el capital_disponible sea > 0 y suficiente para cubrir fees básicos.
- CÁLCULO: Validar que ninguna división por cero ocurra si el bid spread es cero.
- POST: Confirmar que `net_roi >= min_net_roi_config`.

## 11. Criterios de aprobación
- `net_roi >= config.min_target_roi` (Ej. >= 0.05%).
- El capital a comprometer no excede el límite de riesgo por trade.
- El slippage proyectado consume menos del 50% del ROI bruto.
- Todos los datos de input tienen un timestamp < 200ms del reloj del sistema local.

## 12. Criterios de rechazo
- El cálculo arroja un ROI neto negativo o por debajo del umbral mínimo.
- Falta de información de fees para cualquier pata del trade.
- El costo de gas representa más del 80% del beneficio bruto.
- La liquidez disponible en el primer nivel del order book es < 10% del tamaño mínimo de la orden.

## 13. Riesgos que mitiga
- Riesgo de ejecución ruinosa por cálculos optimistas.
- Quema de capital por ignorar gas fees o impact costs.
- Riesgos derivados de falsas señales de arbitraje ("Ghost Arbitrage").

## 14. Integración con otras skills
- Consume datos de: Ingesta de precios (Skill 31), Order book reconstruction (Skill 35).
- Entrega datos a: Optimización convexa de tamaño (Skill 2), Execution State Machine (Skill 54).

## 15. Modelo de datos sugerido
```json
{
  "OpportunityCalc": {
    "timestamp_ms": "int64",
    "route": "string[]",
    "gross_roi_bps": "int",
    "net_roi_bps": "int",
    "gas_cost_usd": "decimal",
    "slippage_bps": "int",
    "viable": "boolean",
    "rejection_reason": "string"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- `POST /internal/math/evaluate_route` (Input: Route + Size, Output: Viability Report).
- Worker local invocado vía memoria directa (ZeroMQ/gRPC local) para latencia < 1ms.

## 17. Logs obligatorios
- `[INFO] Route evaluated: A->B->C, Gross: X%, Net: Y%. Viable: Z.`
- `[WARN] Route rejected: Gas cost (X USD) exceeds gross profit (Y USD).`
- `[ERROR] Invalid calculation: Missing fee tier for Exchange B.`

## 18. Métricas obligatorias
- `math_calc_latency_ms` (Debe ser < 0.5ms).
- `opportunities_evaluated_count`.
- `opportunities_rejected_by_net_roi_count`.
- `slippage_estimation_error_bps` (Para backtesting y ajuste del modelo).

## 19. Tests unitarios
- Test de arbitraje triangular puro con fees cero (espera ganancia teórica).
- Test con fees de taker altos (debe rechazar oportunidad).
- Test de impacto en precio en AMM (validar con x*y=k).
- Test de orden mayor a la liquidez del book (debe proyectar slippage masivo y rechazar).

## 20. Tests de integración
- Test de evaluación consumiendo de un snapshot de Redis real.
- Test inyectando precios sintéticos pero con lógica de feed real para validar latencia.

## 21. Tests E2E
- Simulación de detección, evaluación matemática, y envío de payload de rechazo a la DB de auditoría.

## 22. Checklist de producción
- [ ] Función de cálculo de fees parametrizada y sin constantes hardcodeadas.
- [ ] Timeout de evaluación configurado (ej. 1ms max per route).
- [ ] Manejo de enteros de alta precisión (BigInt/Decimal) para evitar errores de punto flotante en cálculo de tokens ERC20 (18 decimales).

## 23. Ejemplo de configuración no hardcodeada
```yaml
math_engine:
  min_net_roi_bps: 10          # 0.1% minimum
  max_acceptable_slippage_bps: 20
  base_gas_multiplier: 1.15    # Buffer for gas spikes
  precision_decimals: 18
```

## 24. Ejemplo de pseudocódigo
```python
def evaluate_arbitrage(route: Route, capital: Decimal, config: Config) -> MathReport:
    if not is_data_fresh(route):
        return MathReport(viable=False, reason="Stale data")
        
    gross_profit = simulate_execution_path(route, capital)
    
    total_fees = sum([get_exchange_fee(leg) for leg in route])
    gas_cost = estimate_gas_cost(route) * get_current_eth_price()
    slippage = estimate_slippage(route, capital)
    
    net_profit = gross_profit - total_fees - gas_cost - slippage
    net_roi = net_profit / capital
    
    if net_roi < config.min_net_roi:
        return MathReport(viable=False, net_roi=net_roi, reason="Negative or insufficient Net ROI")
        
    return MathReport(viable=True, net_roi=net_roi, expected_profit=net_profit, size=capital)
```

## 25. Criterio final de excelencia
La skill está lista para producción cuando el motor matemático puede procesar 10,000 rutas por segundo con precisión de BigInt, descontando fees, slippage y gas dinámico sin generar ni un solo falso positivo en un dataset histórico de 1 mes de ticks reales.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Precisión de estimación de slippage en mercados muy ilíquidos.
- Dependencias: Order Book Reconstruction (Skill 35), Gas Oracle.
- Próxima skill: Optimización convexa aplicada a tamaño de operación (Skill 2).
