# SKILL 011 — Microestructura de mercado

## 1. Propósito superior
Comprender la dinámica interna del "cómo se forman los precios" leyendo la huella oculta en el flujo de órdenes (Order Flow). Esta skill no analiza velas (candlesticks), analiza la plomería del exchange: Iceberg orders, Tick rules, Latency arbitrage institucional, y dinámicas de "Maker-Taker" de ultra alta frecuencia para deducir la intención direccional del libro de órdenes y anticipar caídas de liquidez.

## 2. Nivel de conocimiento requerido
Experto Senior en Microestructura de Mercados Financieros y Trading Cuantitativo. Dominio del análisis de Order Flow Imbalance (OFI), Volume Profile tick a tick, detección de algoritmos TWAP/VWAP de instituciones, Tape Reading moderno, y mecánicas de los motores de Matching (FIFO, Pro-Rata).

## 3. Capacidades principales
1. Reconstrucción y análisis del L3 Order Book (Order by Order) cuando está disponible, o agregación heurística del L2.
2. Cálculo de Order Flow Imbalance (OFI) y VPIN (Volume-Synchronized Probability of Informed Trading) para predecir movimientos inminentes de precio a nivel milisegundo.
3. Detección de Spoofing, Layering y "Quote Stuffing" para limpiar la liquidez visual falsa del libro.
4. Identificación de "Iceberg Orders" (Órdenes ocultas) mediante el análisis de trades que se rellenan sin consumir volumen visible.
5. Inferencia de intencionalidad: Quién está agresando el book (Takers presionando el bid vs el ask).
6. Adaptación al Tick Size y Lot Size de cada exchange (entendiendo zonas de redondeo forzado).
7. Cálculo de "Queue Position" (Posición en la cola FIFO) para predecir probabilidades de llenado pasivo (Maker strategy).
8. Análisis de latencia estructural de CEXes (Matching Engine tick-batching).
9. Mapeo de fragmentación de liquidez: Determinar qué CEX actúa como "Price Discovery Leader" y cuáles son "Followers".
10. Construcción del perfil de volatilidad a nivel del "Bid-Ask Bounce" (Ruido de rebote del spread).

## 4. Entradas requeridas
- `tick_trades`: Stream continuo de todos los trades ejecutados (Taker side, Size, Price, Timestamp).
- `l2_order_book_deltas`: Actualizaciones incrementales del libro (Level 2).
- `exchange_rules`: Límites estructurales (Tick size, Limit Order rules).

## 5. Salidas esperadas
- `microstructure_health`: Score de estabilidad de la liquidez.
- `imbalance_vector`: Presión compradora vs vendedora a 5ms, 10ms y 50ms.
- `leader_exchange`: Identificador de la plataforma donde el movimiento real se origina.
- `hidden_liquidity_flag`: Booleano y estimación de órdenes ocultas.
- `spoofing_alert`: Alerta de manipulación de book en curso.

## 6. Reglas inmutables
- Toda decisión debe tomarse basándose en eventos de Nivel 2 o 3 y trades reales. Los indicadores técnicos basados en velas de tiempo (MACD, RSI) están estrictamente prohibidos por ser lentos.
- La liquidez que parpadea a un ritmo mayor al tiempo de reacción humano (ej. < 100ms) se descuenta agresivamente considerándose algoritmo creador de mercado tóxico.
- Para pares cruzados, identificar obligatoriamente el mercado líder y usarlo como oráculo de microestructura del seguidor.
- El sistema no asume el spread visible como estático; modela la probabilidad de que el spread cruce antes de ejecutar.

## 7. Algoritmos o métodos que debe conocer
- Cálculo exacto de Order Flow Imbalance (OFI).
- Modelos estadísticos de Hawked Processes (Procesos de punto espacial/temporal de llegadas de órdenes).
- Detección de Anomalías Tick-By-Tick (Z-score de volumen agredido).
- Modelo de Kyle (Para información asimétrica y Market Impact).
- Modelado de colas en motores FIFO limit order book (LOB).

## 8. Fórmulas críticas
- **OFI (Nivel Básico)**: `OFI_t = (BidVol_t >= BidVol_{t-1}) * BidSize - (AskVol_t <= AskVol_{t-1}) * AskSize` (versión simplificada de la diferencia de presión).
- **Ratio de Absorción**: `Taker_Volume / Delta_Book_Level`.
- **Probabilidad de Cambio de Mid-Price**: Función logística dependiente del Imbalance del libro.

## 9. Casos extremos
- Un "Flash Boys" attack donde algoritmos inyectan y retiran liquidez en nanosegundos (Quote Stuffing) para degradar el parseo local de JSON.
- Order book "Vaciado" (Hollow book): Bid y Ask están allí, pero no hay nada en los siguientes 50 niveles.
- El CEX colapsa bajo carga y los WS emiten trades desordenados temporalmente (Out of Order Tick delivery).
- "Stop-Loss Cascades" identificadas empíricamente.

## 10. Validaciones obligatorias
- PRE: Validar integridad de la secuencia de Ticks (SecNum o Timestamp para detectar pérdida de paquetes UDP/TCP).
- CÁLCULO: Aplicar filtro pasa-bajos a las vibraciones del Order Flow para evitar "whipsaw" en la dirección de la señal.
- POST: Si la microestructura indica "Mercado Altamente Tóxico" (Toxic Flow), reportar señal de bloqueo a las skills superiores de riesgo.

## 11. Criterios de aprobación
- OFI es favorable o neutral respecto a la dirección de la pata de ejecución planeada.
- No se detectan patrones de Spoofing masivo en el primer nivel del book.

## 12. Criterios de rechazo
- Intento de comprar contra un nivel de liquidez que está siendo agresado (Market Sell mass execution) a un ritmo mayor de lo que repone.
- El LOB de destino está sufriendo Quote Stuffing.

## 13. Riesgos que mitiga
- Riesgo de Toxicidad de Llenado: Comprar a alguien que *sabe* que el activo acaba de colapsar en Binance (Price Leader) y te está "empapelando" en KuCoin (Follower).
- Riesgo de Market Impact Severo: Entrar cuando el volumen visible es falso, barriendo el mercado real varios porcientos más abajo.

## 14. Integración con otras skills
- Provee datos fundacionales de salud de mercado a Probabilidad Bayesiana (Skill 9) y Optimización Estocástica (Skill 6).
- Actúa como vigilante para el Ejecutor State Machine (Skill 54).

## 15. Modelo de datos sugerido
```json
{
  "MicrostructureState": {
    "symbol": "BTC-USDT",
    "timestamp_ns": 1698765000000100200,
    "ofi_50ms_moving_avg": -15.4,
    "vpin_toxicity_score": 0.88,
    "is_spoofing_detected": true,
    "price_discovery_lag_ms": 45,
    "recommended_action": "halt_maker_orders"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Streamer UDP/TCP interno o Websocket local. Motor atómico de análisis de eventos.

## 17. Logs obligatorios
- `[INFO] Microstructure: Order Flow Imbalance heavily skewed to Ask side. Toxicity rising.`
- `[WARN] Spoofing pattern detected on Bid side (50x cancel/replace in 100ms). Liquidity marked as fake.`

## 18. Métricas obligatorias
- `book_toxicity_index`.
- `tick_processing_latency_us` (Debe ser estelarmente bajo, < 50us).
- `hidden_liquidity_detections`.

## 19. Tests unitarios
- Parseo de Ticks Desordenados: Inyectar un JSON tick desordenado, la lógica debe descartarlo o reordenarlo usando SeqID.
- Detección OFI: Inyectar datos donde el bid se renueva pero el ask se consume, validar que OFI se vuelve positivo fuerte.
- Iceberg Detection: Simular trades que ejecutan al precio Best Bid pero el Best Bid no disminuye su volumen.

## 20. Tests de integración
- Enganchar el módulo L2 Delta Parser directo a este módulo de microestructura y confirmar que el estado se reconcilia con un Snapshot del REST API sin desvíos matemáticos tras miles de mensajes.

## 21. Tests E2E
- El agente lee la cinta real de Binance Futuros vs Binance Spot durante un anuncio macro, deduce correctamente que el derivado lidera al spot en 15 milisegundos, y cancela órdenes Maker expuestas al arbitraje nocivo.

## 22. Checklist de producción
- [ ] Construcción de L2 Delta Book usando un Arbol Rojo-Negro (Red-Black Tree) o arreglos indexados estáticos para mutaciones sub-milisegundo.
- [ ] Medidor estricto de latencia de socket.
- [ ] Buffer anti-inundación si el CEX manda ráfagas de 10,000 WS messages/seg.

## 23. Ejemplo de configuración no hardcodeada
```yaml
microstructure:
  toxicity_vpin_threshold: 0.80
  ofi_window_ms: 100
  spoofing_detection_cancel_ratio: 0.95
  max_acceptable_delay_ms_follower: 50
```

## 24. Ejemplo de pseudocódigo
```python
def process_tick(delta_update, current_book, state):
    # Calculate Order Flow Imbalance
    delta_bid_vol = delta_update.bids_vol - current_book.bids_vol if delta_update.bid_price == current_book.bid_price else 0
    delta_ask_vol = delta_update.asks_vol - current_book.asks_vol if delta_update.ask_price == current_book.ask_price else 0
    
    # Adjust for price shifts
    if delta_update.bid_price > current_book.bid_price:
        delta_bid_vol = delta_update.bids_vol
    if delta_update.ask_price < current_book.ask_price:
        delta_ask_vol = delta_update.asks_vol
        
    ofi = delta_bid_vol - delta_ask_vol
    state.update_ofi_ewma(ofi)
    
    # Detect Toxicity (if OFI goes extremely against our current open passive orders)
    if abs(state.ofi_ewma) > CONFIG.toxicity_limit:
        trigger_toxic_flow_alert()
        
    update_local_l2_book(current_book, delta_update)
```

## 25. Criterio final de excelencia
El componente procesa millones de ticks al día sin pestañear, filtrando el ruido visual de los creadores de mercado y descubriendo la verdadera presión del bloque atómico, permitiendo al bot HFT actuar antes de que el precio visible cruce.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Mutaciones oscuras de la API de CEX que de-sincronicen el libro local.
- Dependencias: Order Book Reconstruction (Skill 35), Websocket resilience (Skill 36).
- Próxima skill: Arbitraje CEX-CEX (Skill 12).
