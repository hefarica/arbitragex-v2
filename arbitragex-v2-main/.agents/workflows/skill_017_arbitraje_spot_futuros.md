# SKILL 017 — Arbitraje spot-futuros

## 1. Propósito superior
Capitalizar la ineficiencia estructural entre el precio del mercado Spot (al contado) y el mercado de Futuros con vencimiento (Delivery Futures) o Perpetuos (Perpetual Swaps). Garantiza beneficios libres de riesgo direccional operando la convergencia del Basis (Spread entre Spot y Futuro) cuando este se encuentra anormalmente amplio (Contango) o invertido (Backwardation), asegurando un retorno fijo a la fecha de expiración o mediante el cobro de Funding Rates.

## 2. Nivel de conocimiento requerido
Experto en Derivados Financieros Tradicionales (TradFi) y Crypto (CeFi/DeFi). Nivel Máster en valoración de opciones/futuros (Cost of Carry Model), matemáticas de liquidación (Mark-to-Market), gestión cruzada de colateral (Cross-Margin) y ejecución atómica de carteras Delta-Neutral.

## 3. Capacidades principales
1. Detección de "Premium" anormal en futuros trimestrales/mensuales respecto al índice Spot.
2. Cálculo exacto del APR (Annual Percentage Rate) de un Cash and Carry Trade hasta la fecha de expiración.
3. Apertura atómica de `Long Spot + Short Futures` en mercados en Contango.
4. Apertura atómica de `Short Spot (Borrow/Margin) + Long Futures` en mercados en Backwardation extremo.
5. Liquidación física implícita: Gestión del cierre del trade exactamente en el milisegundo de la expiración (Settlement) cuando el Basis es matemáticamente cero.
6. Gestión de Rollover: Migrar la posición corta de un futuro a punto de expirar hacia el siguiente trimestre si el Basis sigue siendo favorable.
7. Cálculo de "Capital Efficiency" usando el Spot comprado como colateral (Multi-Asset Margin) para respaldar el Short del Futuro.
8. Rebalanceo automático de margen para evitar margin calls provocados por un Short Squeeze.
9. Monitoreo del "Index Price" frente al "Mark Price" para evitar cacería de stop-loss (Wick hunting).
10. Detección de "Early Unwind" (Cierre anticipado): Si el spread converge a 0 antes del vencimiento, se cierra el trade capturando el 100% del profit inmediatamente.

## 4. Entradas requeridas
- `spot_order_book`: Liquidez del mercado al contado.
- `futures_order_book`: Liquidez del contrato derivado objetivo.
- `expiration_timestamp`: Fecha y hora exacta de la liquidación del contrato.
- `borrow_interest_rate`: Tasa de interés en caso de necesitar pedir prestado para hacer Short Spot.
- `margin_rules`: Requisitos de margen de mantenimiento y colateral admitido del exchange.

## 5. Salidas esperadas
- `basis_opportunity`: Detalle del spread operable.
- `annualized_yield`: Tasa de interés anualizada bloqueada por la operación.
- `execution_payload`: Lote de órdenes acopladas para enviar a la API.
- `unwind_trigger_price`: Nivel de Basis en el que conviene cerrar anticipadamente la operación.

## 6. Reglas inmutables
- Nunca ejecutar la pata Spot en un exchange y la pata de Futuros en otro exchange si el usuario no tiene habilitado un sistema instantáneo de transferencia de margen. Ambas patas deben estar en la misma plataforma (ej. Binance Spot + Binance Coin-M/USD-M).
- El rendimiento calculado debe descontar SIEMPRE los fees de entrada (2x) y los fees de salida (2x), más el costo de capital inmovilizado.
- Nunca usar órdenes "Market" en activos ilíquidos para armar la posición, el "Slippage de entrada" puede devorarse meses de rendimiento pasivo.
- Proteger el límite de liquidación: La cuenta debe sobrevivir un movimiento en contra del 50% del precio del activo sin requerir inyección externa.

## 7. Algoritmos o métodos que debe conocer
- Modelo Cost of Carry: `F = S * e^(r*t)`
- Dinámica de Liquidación (Settlement Price Calculation) de cada exchange (Suele ser un TWAP de la última hora).
- Algoritmos TWAP/VWAP para armar/desarmar posiciones pesadas gradualmente sin alterar el Basis.

## 8. Fórmulas críticas
- **Basis Neto (Net Spread)**: `((Precio_Futuro * (1 - Fee_Taker_Fut)) - (Precio_Spot * (1 + Fee_Taker_Spot))) / Precio_Spot`
- **APR Efectivo**: `(Basis_Neto_Porcentual / Dias_Restantes) * 365 * 100`
- **Break-even de Desarme (Unwind)**: `Spread_Actual < (Fee_Taker_Spot + Fee_Taker_Fut)` (Si el spread es menor a los fees de salir, mejor esperar a vencimiento).

## 9. Casos extremos
- Interrupción de la red al ejecutar: Compras 1 BTC en Spot, la API falla al vender el contrato de futuros, el precio del BTC colapsa, dejándote con una pérdida masiva no cubierta.
- Altcoin Delisting: El exchange decide forzar la liquidación del contrato 2 meses antes del vencimiento a un precio de índice arbitrario.
- Ataque al Mark Price: Una ballena mueve el precio del contrato en futuros (sin mover el spot) solo para liquidar posiciones de cobertura insuficientemente capitalizadas.

## 10. Validaciones obligatorias
- PRE: Asegurar que los contratos de futuros son "Lineales" (USDT-M) o "Inversos" (COIN-M) y calcular la cantidad de contratos (Contract Multiplier) para que igualen exactamente la cantidad Spot.
- CÁLCULO: Validar si la cuenta permite "Unified Margin" (usar el spot como colateral). Si no, el APR se divide a la mitad por tener que inmovilizar capital en USDT extra.
- POST: Verificación incesante de la salud de la cuenta (Margin Ratio) cada segundo durante la vigencia del trade.

## 11. Criterios de aprobación
- El APR proyectado, después de fees, supera el "Hurdle Rate" (ej. > 15% anual).
- Existe liquidez suficiente en ambos libros para armar la posición a mercado usando órdenes IOC.

## 12. Criterios de rechazo
- Faltan menos de 24h para el vencimiento y los fees consumen el 90% del Basis restante.
- Volatilidad extrema (Flash Crash activo): Imposibilidad de garantizar un fill simultáneo.

## 13. Riesgos que mitiga
- Riesgo Direccional: El portfolio ignora si el mercado entra en Crypto Winter o Bull Run.
- Riesgo de Opportunity Cost: Dinero fiat inactivo en cuenta, se convierte a posición yield-bearing de bajo riesgo.

## 14. Integración con otras skills
- Base fundacional para el Arbitraje de Funding Rate (Skill 16).
- Validado estadísticamente por Optimización Estocástica (Skill 6).

## 15. Modelo de datos sugerido
```json
{
  "SpotFuturesArbitrage": {
    "strategy_id": "sf-1234",
    "spot_ticker": "ETH-USDT",
    "future_ticker": "ETH-241227",
    "basis_pct": 3.5,
    "days_to_expiry": 45,
    "projected_apr": 28.3,
    "notional_size_usd": 25000,
    "status": "APPROVED_FOR_ENTRY"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Monitor de Basis en tiempo real (Tick by Tick). El trade puede tardar meses, pero la oportunidad de entrar dura milisegundos.

## 17. Logs obligatorios
- `[INFO] Cash and Carry initiated: Long Spot BTC / Short BTC-0628. Locked Basis: 4.2%. Expiry in 30 days. APR: 51.1%.`
- `[WARN] Basis collapsed early to 0.1%. Executing Early Unwind. Locking in profits 20 days ahead of schedule.`

## 18. Métricas obligatorias
- `basis_tracking_error` (Diferencia entre el spread esperado al entrar y el ejecutado).
- `margin_ratio_avg`.
- `unwind_slippage_usd`.

## 19. Tests unitarios
- Conversión de Nocional: Comprobar que 10 contratos de $100 COIN-M equivale exactamente a la cantidad Spot apropiada dependiendo del precio del momento de apertura.
- Lógica de Early Unwind: Inyectar un spread de 0%, debe desarmar. Inyectar spread de 5%, debe mantener.
- Cálculo de Liquidación Unificada: Validar el algoritmo del Margin Ratio de Binance.

## 20. Tests de integración
- Sincronización API para abrir órdenes simultáneas. Enviar REST calls concurrentes y validar estado.

## 21. Tests E2E
- Simular ciclo completo: El sistema entra al trade, simular paso del tiempo hasta expiración (TWAP settlement), y verificar que el sistema no hace nada y deja que el exchange liquide físicamente, luego cuenta las ganancias.

## 22. Checklist de producción
- [ ] Implementar un modo de recuperación "Leg-Repair" si una pata sufre "Partial Fill".
- [ ] Excluir tokens con eventos próximos (Airdrops, Forks) que alteran la paridad de los futuros.
- [ ] Validar compatibilidad del modo de margen de la cuenta vía API antes de lanzar.

## 23. Ejemplo de configuración no hardcodeada
```yaml
spot_futures:
  min_apr_threshold_pct: 12.0
  early_unwind_basis_bps: 10
  require_unified_margin: true
  max_capital_per_trade_usd: 100000
```

## 24. Ejemplo de pseudocódigo
```python
def check_cash_and_carry(spot_book, future_book, days_to_expiry, fees):
    # Ensure actionable spread (Taker out of Spot Ask, Taker into Future Bid)
    basis_net = ((future_book.bid * (1 - fees.fut_taker)) - (spot_book.ask * (1 + fees.spot_taker))) / spot_book.ask
    
    if basis_net <= 0:
        return False, 0
        
    apr = (basis_net / days_to_expiry) * 365 * 100
    
    if apr >= CONFIG.min_apr:
        return True, apr
        
    return False, apr

def execute_unwind(position, current_basis):
    if current_basis <= CONFIG.early_unwind_basis:
        log.info("Basis collapsed. Triggering early unwind.")
        execute_atomic_close(position)
```

## 25. Criterio final de excelencia
El sistema construye un portafolio "Delta-Neutral" equivalente a un banco, bloqueando APRs superiores al 30% en Bull Markets sin asumir ningún riesgo direccional, resistiendo flash crashes masivos sin sufrir liquidación por fallas de gestión de colateral.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Exchange Bankruptcy. (Tener spot y futuros en FTX garantizaba la pérdida total del fondo independientemente del hedge).
- Dependencias: API Trading, Account Margin Management.
- Próxima skill: Arbitraje back/lay deportivo legal (Skill 18).
