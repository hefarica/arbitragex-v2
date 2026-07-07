# SKILL 012 — Arbitraje CEX-CEX

## 1. Propósito superior
Dominar la ejecución atómica simulada (Statistical Arbitrage) entre múltiples Exchanges Centralizados (CEX). Esta skill rige la compra en el CEX A y la venta simultánea en el CEX B, gestionando la asincronía de APIs REST/WS, protegiendo el capital de la desincronización de libros (Leg Risk), y coordinando el rebalanceo de inventarios sin depender de transferencias lentas on-chain durante el trade.

## 2. Nivel de conocimiento requerido
Experto en Arquitectura de Microservicios, Redes HFT (High-Frequency Trading) y APIs de Exchanges Crypto (Binance, Kraken, Bybit, OKX). Conocimiento profundo de concurrencia, asincronía (Promesas/Futures), gestión de Rate Limits distribuidos y manejo de errores HTTP/Websocket (502 Bad Gateway, Timeout, Connection Reset).

## 3. Capacidades principales
1. Ejecución concurrente ultra-rápida (Scatter-Gather pattern) para enviar órdenes a dos CEXes exactamente en el mismo milisegundo.
2. Gestión del "Inventory Risk" (Riesgo de desbalance de cartera) manteniendo capital base y quote pre-depositado en ambos CEXes.
3. Tratamiento de respuestas asíncronas de ejecución parcial (Partial Fills).
4. Normalización instantánea de IDs de pares y reglas de precisión (ej. `BTC/USDT` vs `BTCUSDT` vs `XBTUSDT`).
5. Detección proactiva de "Withdrawal/Deposit Suspended" status en el CEX, bloqueando oportunidades si el rebalanceo posterior es imposible.
6. Lógica de "Hedge-Fallback" si la pata B falla después de que la pata A ya se ejecutó (ejecutar a market, o esperar a VWAP).
7. Mapeo de latencias de API endpoint para compensar la orden enviando primero al CEX más lento.
8. Validación de "API Key Permissions" en vuelo para evitar fallo de "Unauthorized" en el último segundo.
9. Tolerancia a fallos REST mediante re-lectura de estado por WebSocket (Read-after-write consistency workaround).
10. Sincronización estricta de relojes locales con los servidores del exchange mediante firmas HMAC-SHA256 sin error temporal.

## 4. Entradas requeridas
- `arbitrage_signal`: Orden pre-calculada y aprobada (Asset, Side A, Side B, Price A, Price B, Size).
- `api_clients`: Pool de conexiones TCP keep-alive establecidas con los exchanges.
- `inventory_state`: Balance en tiempo real de los CEXes involucrados.
- `exchange_health`: Estado de los endpoints (Latencia, Rate limits).

## 5. Salidas esperadas
- `execution_receipts`: Arrays de confirmaciones con el Fill Price real de ambos CEXes.
- `net_pnl_realized`: Ganancia en USD registrada post-trade tras calcular el spread ejecutado.
- `inventory_delta`: Cambio neto en el balance para el sistema de rebalanceo futuro.
- `error_state`: Descripción precisa si una de las patas falló, activando el modo emergencia.

## 6. Reglas inmutables
- Nunca ejecutar un arbitraje CEX-CEX esperando enviar los fondos on-chain en el medio del trade. Todo el capital (Base y Quote) debe estar pre-depositado en ambos lados.
- Disparar ambas patas simultáneamente (o compensadas por latencia); nunca de forma secuencial síncrona.
- Nunca usar órdenes Market (MKT) puras si el Order Book no asegura profundidad; usar Limit Orders agresivas (FOK - Fill or Kill / IOC - Immediate or Cancel).
- Rechazar el trade si el CEX A o B tiene las transferencias de la moneda involucrada suspendidas, salvo que sea arbitraje estadístico de reversión a la media estricto.

## 7. Algoritmos o métodos que debe conocer
- FIX Protocol (Financial Information eXchange) o emulación sobre REST/WebSocket.
- Multi-threading / Asynchronous IO (Event Loop de Node, o Tokio/Async-std en Rust).
- NTP (Network Time Protocol) synchronization mechanics para firmas de API.
- Patrón de compensación Saga para transacciones distribuidas sin Atomicidad nativa.

## 8. Fórmulas críticas
- **Compensación de Latencia**: `Delay_A = max(0, Latencia_B - Latencia_A)` (Disparar la orden al exchange rápido con `Delay_A` de retraso para que lleguen al Matching Engine a la vez).
- **Inventario Máximo Operable**: `Min(Balance_USD_A, Balance_BTC_B * Precio_BTC)`
- **PnL Realizado**: `(Vol_Llenado_A * Precio_Llenado_A) - (Vol_Llenado_B * Precio_Llenado_B) - Fees_A - Fees_B`

## 9. Casos extremos
- Interrupción masiva de la API (502 Bad Gateway) en un solo CEX justo al disparar la orden, dejándonos sobre-expuestos en el otro.
- Deslistado repentino de un token (Delisting) que dispara la oportunidad matemática pero imposibilita el trade de salida.
- Un "Fat Finger" del mercado que nos hace ganar la pata A, pero colapsa la API del CEX B por sobrecarga, perdiendo la venta.
- Inconsistencia de la API REST que responde "Timeout" pero el WebSocket indica que la orden *sí* se llenó.

## 10. Validaciones obligatorias
- PRE: Chequear balance hard/soft antes de emitir la firma HMAC.
- PRE: Chequear el status global del sistema (ej. Binace System Status API).
- CÁLCULO: Aplicar precisión exacta a Lot Size y Tick Size, truncando, no redondeando matemáticamente al alza.
- POST: Reconciliación cruzada (REST vs WS vs Account Balance) para certificar el estado del trade.

## 11. Criterios de aprobación
- Recepción de confirmaciones "Filled" con estado IOC/FOK exitoso de ambas contrapartes.
- El PnL reportado post-trade es `> -tolerancia_permitida`.

## 12. Criterios de rechazo
- El módulo de riesgo detiene el envío por sobrepasar el límite de capital asignado a un exchange específico.
- Pérdida de latencia: El ping previo al CEX superó los 200ms.

## 13. Riesgos que mitiga
- Leg Risk (Execution Risk): Quedarse "cojo" en un arbitraje porque la plataforma B falló y ahora se tiene el activo al descubierto.
- Riesgo de Precisión: Enviar a un CEX una orden con 9 decimales cuando solo admite 8, resultando en rechazo `HTTP 400 Bad Request` y perdiendo la oportunidad.

## 14. Integración con otras skills
- Es controlado por Execution State Machine (Skill 54).
- Alimenta la base de datos PostgreSQL de Auditoría (Skill 57).

## 15. Modelo de datos sugerido
```json
{
  "CexArbitrageExecution": {
    "trade_id": "uuid",
    "leg_a": { "exchange": "binance", "status": "FILLED", "fill_price": 30000, "qty": 0.1 },
    "leg_b": { "exchange": "kraken", "status": "FILLED", "fill_price": 30050, "qty": 0.1 },
    "latency_offset_ms": 12,
    "net_profit_realized_usd": 4.80,
    "inventory_balanced": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Sistema de Workers concurrentes con un Coordinador de Trade (Saga Pattern) que espera un `Promise.allSettled()` o su equivalente en Rust.

## 17. Logs obligatorios
- `[INFO] CEX-CEX Arbitrage executed: Bought 0.1 BTC on Binance, Sold 0.1 BTC on Kraken. PnL: +$4.80.`
- `[CRITICAL] Leg B failed (HTTP 500) on OKX. Executing emergency market hedge on Binance to flatten position.`

## 18. Métricas obligatorias
- `cex_execution_success_rate`.
- `leg_risk_failure_count` (Cuántas veces falló una sola pata).
- `average_slip_vs_expected` (Comparativa del fill price con la predicción de Skill 7).

## 19. Tests unitarios
- Mockear `Promise.all()` con latencias diferentes, verificar que el orquestador maneja bien que A responda en 10ms y B responda en 300ms.
- Forzar un HTTP 500 en la pata B; verificar que se lanza el estado de emergencia o reversión si es posible.
- Formateo de cantidades: Truncar `10.123456789` al Step Size `0.01` -> `10.12`.

## 20. Tests de integración
- Ejecutar trades en la Testnet real de Binance/Bybit. Certificar HMAC signatures y recepción por WebSocket.

## 21. Tests E2E
- El orquestador supremo identifica arbitraje, dispara en Testnet de Binance y OKX, reconcilia balances, y verifica que el ROI post-fees en la base de datos es correcto y positivo.

## 22. Checklist de producción
- [ ] Conexiones HTTP configuradas con `keep-alive` habilitado y reuse de sockets TCP (Agent en Node.js, `reqwest` connection pool en Rust).
- [ ] Desactivación estricta del logueo de API Keys y Secrets (Skill 63).
- [ ] Uso de endpoints específicos para HFT / colocación masiva de órdenes si el exchange los tiene (Ej. endpoint batch de Binance).

## 23. Ejemplo de configuración no hardcodeada
```yaml
cex_execution:
  order_type: "IOC" # Immediate or Cancel
  latency_offset_compensation: true
  emergency_hedge_timeout_ms: 2000
  retry_on_network_error: false # Never retry HFT trades automatically
```

## 24. Ejemplo de pseudocódigo
```javascript
async function executeCexArbitrage(opportunity, inventory) {
    const orderA = formatOrder(opportunity.legA);
    const orderB = formatOrder(opportunity.legB);
    
    // Fire and forget mechanism wrapped in Promises
    const sendA = exchangeA.client.placeOrder(orderA);
    const sendB = exchangeB.client.placeOrder(orderB);
    
    const [resultA, resultB] = await Promise.allSettled([sendA, sendB]);
    
    if (resultA.status === 'fulfilled' && resultB.status === 'fulfilled') {
        return calculateRealizedPnl(resultA.value, resultB.value);
    } 
    else if (resultA.status === 'fulfilled') {
        return triggerEmergencyHedge(resultA.value, "A", exchangeA);
    }
    else if (resultB.status === 'fulfilled') {
        return triggerEmergencyHedge(resultB.value, "B", exchangeB);
    }
    else {
        return handleTotalFailure(resultA.reason, resultB.reason);
    }
}
```

## 25. Criterio final de excelencia
El ejecutor CEX-CEX logra un "Success Fill Rate" superior al 98%, nunca envía una orden mal firmada y jamás deja el portafolio direccionalmente expuesto durante más de 1 segundo si una de las plataformas falla.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: APIs que responden "Error" pero ejecutan la orden internamente (Requiere re-validación de balance WS constante).
- Dependencias: API Rate Limit, Criptografía, Manejo de Inventario.
- Próxima skill: Arbitraje DEX-DEX (Skill 13).
