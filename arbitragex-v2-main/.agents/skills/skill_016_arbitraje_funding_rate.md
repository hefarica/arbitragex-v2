# SKILL 016 — Arbitraje por funding rate

## 1. Propósito superior
Explotar el mecanismo estructural que mantiene los Perpetual Futures atados al precio Spot: el Funding Rate. Esta skill no realiza operaciones intradiarias de microsegundos, sino "Delta Neutral Strategies" (Cash and Carry). Gana la tasa de interés cobrada por el exchange a los longs/shorts manteniendo una cobertura perfecta con cero exposición a la volatilidad del precio del activo (Market Neutrality).

## 2. Nivel de conocimiento requerido
Experto en Derivados Financieros, Perpetual Swaps y Hedging algorítmico. Entendimiento matemático del Mark Price, Index Price, Premium Index, y la liquidación periódica (cada 8h o 1h) del Funding Fee. Dominio del manejo de márgenes (Isolated/Cross) y optimización del uso de capital para evitar Margin Calls (Liquidaciones).

## 3. Capacidades principales
1. Búsqueda cruzada masiva de Funding Rates negativos o muy positivos en Binance, Bybit, OKX y DEX Perpetuals (dYdX, Hyperliquid).
2. Cálculo del "Annualized Yield" (APR/APY) proyectado basado en la tasa histórica reciente y premium index instantáneo.
3. Apertura atómica (simultánea) de posición Spot (Compra) y posición Perpetual (Short) del mismo tamaño (Delta = 0).
4. Gestión de Capital Efficiency: Uso de activos colaterales y préstamos cross-margin para maximizar el retorno de la inversión total.
5. Rebalanceo automático de margen: Mover fondos de la cuenta spot a la de futuros si el precio del activo sube agresivamente, para evitar liquidación del short.
6. Cálculo de "Costo de Entrada y Salida" (Slippage + Maker/Taker fees) frente al Funding Rate para definir la rentabilidad mínima necesaria.
7. Monitoreo predictivo del siguiente Funding Tick (Estimación de la tasa antes de la ejecución de la hora H).
8. Cierre automatizado o desenrosque (Unwind) cuando el Funding Rate revierte al estado neutro o se vuelve en contra.
9. Integración de riesgo de liquidación (Liquidation Price Tracking).
10. Detección del efecto "Funding Squeeze" y rechazo de estrategias ante volatilidad demencial.

## 4. Entradas requeridas
- `funding_rates`: Tasa de financiación de los exchanges globales (Predictiva actual y realizada previa).
- `spot_futures_basis`: Diferencia de precio (Spread) entre el Spot y el Futuro Perpetuo.
- `margin_balances`: Saldo de márgenes de las cuentas involucradas.
- `fees`: Costos de ejecución de entrada y salida, y "Borrow Interest" (si se usa apalancamiento).

## 5. Salidas esperadas
- `delta_neutral_position`: Objeto detallando el estado exacto de cobertura (Side, Tamaño, Exchanges).
- `projected_apy`: Rendimiento anualizado estimado en curso.
- `liquidation_distance_pct`: Margen de seguridad antes de una liquidación forzosa de la pata corta.
- `funding_income_realized`: Flujo de caja recolectado durante la vida de la operación.

## 6. Reglas inmutables
- Toda posición iniciada por esta skill debe ser ESTRICTAMENTE Delta Neutral. Comprar 1 BTC debe corresponder inmediatamente con Short 1 BTC.
- El sistema de rebalanceo de margen (Margin Rebalancer) debe ser altamente prioritario (Skill nivel riesgo vital). Si el exchange se cae y no podemos añadir margen, el trade no debió abrirse sin un colchón de liquidación superior al 30%.
- El APR proyectado para ejecutar la estrategia debe superar holgadamente (ej. > 15% APY base) el rendimiento pasivo libre de riesgo, descontando fees de entrada/salida.
- Si el activo base de la cuenta marginal es diferente a USDT (Ej. Coin-M futures), el cálculo matemático de la cobertura inversa (1/P) debe ser exacto, evitando "Gamma exposure" accidental.

## 7. Algoritmos o métodos que debe conocer
- Gestión dinámica de Portfolio Hedging.
- Black-Scholes (Contexto de riesgo implícito) y modelo de GARCH para volatilidad esperada.
- Optimal Unwind Algorithms (Cuándo y cómo cerrar la posición impactando el mercado lo mínimo).
- API Margin Transfers de alta frecuencia (ej. `POST /sapi/v1/margin/transfer`).

## 8. Fórmulas críticas
- **Delta Total**: `Delta = Spot_Quantity_Held - Fut_Quantity_Shorted` (Debe ser = 0).
- **Funding Income**: `Income = Position_Value_Notional * Funding_Rate_Pct`
- **Break-Even temporal**: `(Entry_Fee + Exit_Fee + Slippage) / Avg_Funding_Rate` (Muestra cuántos ciclos (ej. 8 horas) debe sobrevivir el trade para empatar comisiones).
- **APR Anualizado**: `(Funding_Rate_8h * 3 * 365) * 100`

## 9. Casos extremos
- Short Squeeze colosal (ej. Elon Musk twittea sobre DOGE): El precio hace un 500% en minutos. La cuenta de futuros es liquidada por falta de margen antes de poder vender el Spot para rebalancear, perdiendo todo.
- Reversión repentina de Funding: El mercado entra en un Bear Market violento, y el funding rate del short pasa de dar +0.1% a cobrar -0.5% en un ciclo, aniquilando ganancias de una semana.
- API bloqueada durante congestión de red, impidiendo el Unwind (cierre de la operación) cuando las tasas se vuelven adversas.

## 10. Validaciones obligatorias
- PRE: Validar cálculo de break-even. Si requiere 14 días de funding positivo constante para pagar los fees de entrada, RECHAZAR operación.
- CÁLCULO: Validar la compatibilidad de "Contract Multipliers" (Un contrato de OKX puede valer $10, uno de Binance 1 BTC). El tamaño nocional debe igualarse con precisión perfecta.
- POST: Monitor en tiempo real de `liquidation_distance_pct`. Si cae a menos de 15%, emitir orden de "Auto-Deleverage" (Unwind de emergencia parcial) o transferir capital de reserva.

## 11. Criterios de aprobación
- La oportunidad (Funding alto sostenido o Premium Index en pico) asegura un "Return on Capital" que justifica el bloqueo de fondos durante la ventana proyectada.
- Las patas Spot y Futuro se abrieron con un Delta Neutral comprobado y slippage mínimo.

## 12. Criterios de rechazo
- El spread entre el Spot y el Futuro perpetuo (Basis) es adverso de inicio, requiriendo comerse un "Negative Premium" solo para entrar al trade.
- Volatilidad histórica extrema en el par objetivo, elevando el riesgo de "Wick Liquidation" en el futuro.

## 13. Riesgos que mitiga
- Riesgo de Mercado Direccional: El sistema gana dinero si BTC sube, baja o va lateral, gracias al hedge perfecto.
- Riesgo de Opportunity Cost: Evita inmovilizar capital en activos con bajo retorno; el capital busca dinámicamente las monedas (Altcoins meme, tokens nuevos) con las tasas de financiación más escandalosas.

## 14. Integración con otras skills
- Alimentado por Arbitraje Spot-Futuros (Skill 17 - Variación a término fijo).
- Regulado estrechamente por Drawdown control (Skill 43) y Risk Engine (Skill 41).

## 15. Modelo de datos sugerido
```json
{
  "FundingRateArbitrage": {
    "position_id": "delta-882",
    "asset": "PEPE/USDT",
    "spot_exchange": "binance",
    "fut_exchange": "bybit",
    "notional_size_usd": 15000,
    "current_funding_rate_bps": 75,
    "accumulated_income_usd": 125.50,
    "liquidation_distance_pct": 45.2,
    "status": "ACTIVE_HEDGED"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio secundario (Mid-Frequency) que escanea las APIs de Premium/Funding cada 1 minuto, y un worker de alta prioridad monitoreando la distancia de liquidación cada 1 segundo por WebSockets.

## 17. Logs obligatorios
- `[INFO] Initiating Delta Neutral position on PEPE. Funding rate is 0.75% per 8h. Capital locked: $15,000.`
- `[DEBUG] Rebalancing margin: Transferring 500 USDT from Spot to Futures to defend liquidation price.`
- `[WARN] Funding rate reversal predicted on next tick. Executing orderly unwind (Close Spot, Close Short) to secure profits.`

## 18. Métricas obligatorias
- `delta_neutrality_variance_usd` (Debe oscilar muy cercano a $0.00).
- `active_apy_realized` (Ganancia verdadera de funding vs la predicha).
- `margin_call_proximity_events` (Cuántas veces estuvo cerca de la liquidación).

## 19. Tests unitarios
- Cálculo de Delta: Inyectar compra de 0.5 BTC Spot, y Short de 500 contratos de 0.001 BTC. Validar que el delta es matemáticamente Cero absoluto.
- Margin Calculator: Simular un aumento del 30% en precio de activo. Validar que la alerta de Margin Requirement se activa a tiempo.
- Unwind logic: Simular orden de desarme, el sistema debe primero cerrar la pata riesgosa o hacerlo con algoritmos TWAP para no crear exposición no deseada.

## 20. Tests de integración
- Interacción con la API `Margin` del exchange (Ej. mover saldos internos simulados). Certificar que el IAM/Role de la API Key puede transferir internamente pero NO realizar Withdraw externo.

## 21. Tests E2E
- El agente visualiza un histórico (Bull Run 2024 donde el funding llegó a APR > 100%), abre posición, simula pago del funding durante 5 ciclos de 8h, rebalancea colateral 1 vez, desarma el trade por reversión, consolidando la ganancia.

## 22. Checklist de producción
- [ ] Mapeo de reglas de liquidación (Maintenance Margin Rate, Tiered Leverage) que cambia por cada exchange, no usar valores por defecto.
- [ ] Soporte para cobro continuo (Bybit / DEXes) vs cobro discreto (Binance 8h/4h/1h).
- [ ] Filtro estricto anti-bancarrota (Evitar monedas muertas que cobran fees absurdos por fallos del oráculo del exchange).

## 23. Ejemplo de configuración no hardcodeada
```yaml
funding_rate_arbitrage:
  min_target_apy_pct: 25.0
  max_capital_allocation_pct: 15.0
  liquidation_defense_threshold_pct: 20.0  # Top up margin when 20% away
  unwind_trigger_rate_bps: -10             # Close if funding goes negative by 0.1%
```

## 24. Ejemplo de pseudocódigo
```python
def check_funding_opportunity(spot_price, fut_price, predicted_funding_rate, fees):
    # Calculate costs
    total_entry_fees = spot_price * fees.spot_taker + fut_price * fees.fut_taker
    total_exit_fees = spot_price * fees.spot_maker + fut_price * fees.fut_maker
    
    # Calculate required cycles to break even
    income_per_cycle = spot_price * predicted_funding_rate
    cycles_to_breakeven = (total_entry_fees + total_exit_fees) / income_per_cycle
    
    # Heuristic: If it takes more than 2 cycles (16 hours) to pay fees, too risky (rates change fast)
    if cycles_to_breakeven > 2:
        return False, None
        
    projected_apr = (predicted_funding_rate * 3 * 365) * 100
    
    return True, DeltaNeutralTradeSpec(size=optimal_size, apr=projected_apr)

def margin_defense_worker(position):
    if position.liquidation_distance_pct < CONFIG.defense_threshold:
        amount_to_transfer = calculate_safe_margin_topup(position)
        execute_internal_transfer("SPOT", "FUTURES", "USDT", amount_to_transfer)
        log.info(f"Margin defended. Transferred {amount_to_transfer} USDT")
```

## 25. Criterio final de excelencia
El sistema convierte un mercado cripto ultravolátil y peligroso en un bono del tesoro libre de riesgo (Treasury Bond) de altísimo rendimiento, extrayendo flujo de caja diario en piloto automático sin que el inversor se preocupe si la criptomoneda sube o baja de precio.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Exchange API Downtime (Flash Crash con Binance caído) que impida inyectar margen, causando la liquidación del hedge. Requerirá monitoreo ultra-redudante.
- Dependencias: API Margin Management, Risk Engine.
- Próxima skill: Arbitraje spot-futuros (Skill 17).
