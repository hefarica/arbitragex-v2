# SKILL 049 — Detección de order flow toxico (Spoofing)

## 1. Propósito superior
Proteger al agente de engaños deliberados perpetrados por ballenas institucionales, HFTs enemigos (Predatory Trading) y algoritmos manipuladores. Esta skill intercepta trampas visuales de liquidez falsa ("Spoofing", "Layering", "Flickering Orders") y previene que el bot intente ejecutar un arbitraje contra una orden masiva que, en el instante en que envíes el trade, será cancelada maliciosamente por el atacante dejándote atrapado con un precio indeseado (Slippage de ruina).

## 2. Nivel de conocimiento requerido
Experto en Microestructura de Manipulación (Market Abuse Detection), Análisis de Series de Cancelación de Ticks L2, Algorítmica Probabilística de "Order Cancellation Rate" (OCR), Tipología de Market Makers (HFT Flow toxicity). Conocimiento de regulaciones SEC/CFTC sobre Spoofing y tácticas de trampa en Orderbooks ciegos de CEX/DEX.

## 3. Capacidades principales
1. Detección de Spoofing (Asimetría Fantasma): Identifica si aparece una muralla brutal de venta (Bid Wall / Ask Wall) de $1 Millón en Binance y, concurrentemente, miles de micro-cancelaciones (Flickers) ocurren justo antes del impacto de otras órdenes de mercado.
2. Order Cancellation Ratio (OCR): Calcula cuántas órdenes límite son colocadas vs canceladas sin haber sido "llenadas" (Filled). Si el ratio supera el 95%, el Orderbook está bajo ataque manipulador y es "Tóxico".
3. Identificación de Iceberg Orders (Órdenes Ocultas): Detecta anomalías donde el precio no se mueve, a pesar de que el flujo de trades (Market Executions) ataca furiosamente el nivel. Infiere que un Market Maker tiene liquidez oculta ilimitada protegiendo un soporte y la incorpora al OrderBook local como liquidez real escondida.
4. Veto de Liquidez Espejismo (Phantom Liquidity Veto): Notifica al Optimizador de Tamaño (Skill 2) que NO cuente con la liquidez del Nivel 1 o 2 de Asks para cálculos matemáticos, tachándolos como "Órdenes Trampa" (Bait Orders).
5. Ping-Pong y Wash Trading Detections: Detecta bots defectuosos que compran y venden contra sí mismos creando picos artificiales de volumen. El bot ignora esos picos para no mal-identificarlos como un interés genuino direccional.
6. Protección de Falso Despeg de Stables (False De-peg Spoof): Ballenas colocando bloques de $50M en USDC a $0.99 para asustar minoristas, cancelándolos al segundo. El bot aísla el ruido y no cae en un Vuelo de Pánico a Liquidez (Skill 44) en vano.
7. Tracking de Imbalance Visual vs Imbalance Real (OFI): Si el Libro muestra 90% fuerza compradora (Spoofers inflacionando Bids para forzar a la gente a comprar), pero los Trades Reales ejecutados (Tape) muestran un 90% vendiendo a la baja, existe "Divergencia Tóxica".
8. Identificación de Ataque de Capa (Layering): Visualiza y prohíbe el seguimiento matemático cuando alguien pone 5 órdenes de compra escalonadas y las mueve todas hacia arriba un centavo cada milisegundo "arrinconando" al bot hacia una ejecución pésima.
9. Reseteo en Cascada (Cascading Filter): Apaga la "Caza de Cuchillos" (Catching knives) cuando detecta liquidaciones en masa (Stop-Loss hunting) donde todo el OrderBook está siendo desmantelado en un segundo.
10. Sombra Táctica (Stealth Mode): Si este módulo detecta alta toxicidad, desactiva todas las órdenes Pasivas "Maker" del bot (Si hace Market Making) para evitar ser atropellado por los Depredadores (Toxic Takers).

## 4. Entradas requeridas
- `order_book_deltas`: Ráfaga masiva de cambios `(Price, Size_Change)` en nanosegundos (De la Skill 33).
- `recent_public_trades`: Tick-by-tick real trade executions (El "Tape" o "Tick Data").
- `target_arbitrage_opportunity`: La ruta de trade calculada lista para enviarse.

## 5. Salidas esperadas
- `flow_toxicity_score`: Variable de 0.00 (Sano) a 1.00 (Tóxico Letal).
- `safe_execution_depth`: Cuántos niveles del orderbook hacia abajo están clasificados como "Sólidos" y no ilusorios.
- `spoofing_alert_flag`: Veto booleano para abortar o continuar.

## 6. Reglas inmutables
- Una orden masiva en el Nivel 1 del Libro de Órdenes Cuyo Tiempo de Vida (Time-to-Live) es consistentemente menor a 50 milisegundos (Aparece y Desaparece intermitentemente) DEBE ser descartada al 100% de los cálculos del bot. Jamás intentar arbitrar cruzando esa orden (Es un Mirage / Espejismo algorítmico enemigo).
- NUNCA usar la variable "Volumen Total del Libro" (Total Book Depth) como un indicador de solidez sin aplicar primero un filtro de Tasa de Cancelación. Un libro de 10 Millones pero 9.9 Millones flotando/spoofing es un libro estructuralmente vacío.
- Si el "Toxicity Score" es Alto (Ej. > 0.85), el Bot sólo tiene autorizado enviar Órdenes Taker (IOC - Immediate Or Cancel). Jamás Órdenes Maker/Limit pasivas esperando (GTC) porque serán explotadas (Front-run y arrolladas) por el Spoofer direccional.

## 7. Algoritmos o métodos que debe conocer
- Algoritmo de Detección VPIN (Volume-Synchronized Probability of Informed Trading) o derivaciones adaptadas.
- Cancel/Replace (C/R) Ratios Monitoring (Contador de cancelaciones por Tick).
- Divergencia Bid-Ask Imbalance (BAI) vs Trade Imbalance (TI).

## 8. Fórmulas críticas
- **Order Cancellation Ratio (OCR)**: `Cancel_Events / (Cancel_Events + Filled_Events)` en ventana móvil de 1 seg. Si OCR > 0.95 -> Toxic Spoofing Environment.
- **Toxicity Score Combinado**: `WeightedAvg(OCR, Divergencia_OFI_Tape, Flicker_Frequency)`
- **Condición de Veto Crítico**: `if (Opp_Dependency_On_Spoof_Order == TRUE) { ABORT }`

## 9. Casos extremos
- Bateo Institucional (The Institutional Bait): El CEX Kraken muestra que comprar BTC está $50 por debajo de Binance. Enorme arbitraje (+0.1%). Pero los $50 abajo los sostiene una sola orden de $5 Millones de un fondo HFT enemigo. El Bot inexperto manda orden de venta en Binance y compra en Kraken. El enemigo detecta en 1ms la orden en camino a Kraken y cancela su orden de $5M (Spoofing). El bot de arbitraje vende en Binance a buen precio, pero compra en Kraken tragándose el libro un 0.5% abajo, incurriendo en pérdida de -0.4% por "Lag Trampa". El Módulo Toxico predice el Spoof, veta el Arbitraje, salvando el capital.
- Exhaustión de Flujo (Exhaustion Vacuum): Una ballena vende violentamente consumiendo todo el libro de Asks (Market Sell massive sweep). El Orderbook queda "hueco" (Empty Vacuum). El Spread visual salta absurdamente. Las matemáticas de arbitraje ven oro. El Módulo Tóxico ve que la Tasa de Relleno es cero y decreta un "Vacuum Halt" (Parada de Vacío) impidiendo operar ilusiones post-crash que van a rebotar inmediatamente (Reversion/Whipsaw).

## 10. Validaciones obligatorias
- PRE: Corroborar si la oportunidad de Arbitraje descubierta por las Skills (12,13,14) depende estructuralmente (en > 50% del tamaño) de UNA SOLA orden masiva del libro. Si es así, lanzar escáner de Spoofing inmediato sobre ese tick de precio específico.
- CÁLCULO: Mantener de forma rotativa en RAM los últimos 1,000 eventos Canceled/Filled (Ventana Circular). Re-evaluar continuamente el "Order Flow Toxicity" en un worker thread asíncrono.
- POST: Si una oportunidad fue VETADA por el Filtro Tóxico, pero la realidad posterior mostró que la orden de la ballena ERA genuina (Alguien realmente la tradeó y no la cancelaron), el Bot debe relajar sutilmente su umbral de paranoia (Aprendizaje de pesos).

## 11. Criterios de aprobación
- Detección asincrónica (Veto) que ocurre en menos de 0.5ms sin bloquear el hilo principal.
- Identificación exitosa empírica de "Flickering orders" (Ordenes Pestañeantes) sin generar falsos positivos con Traders Reales agregando liquidez sana.

## 12. Criterios de rechazo
- El sistema penaliza como "Tóxicas" todas las órdenes de creadores de mercado (Market Makers genuinos), congelando al bot de por vida en "Modo Paranoia" e impidiendo el 100% del trading.
- Complejidad temporal Cúbica (Ej. iterar miles de deltas de orden para hallar correlaciones cruzadas consumiendo 100% de CPU del core).

## 13. Riesgos que mitiga
- Riesgo de Ejecución Truncada (Unilateral Leg Execution): Al mandar un arbitraje CEX-CEX, la pata A compra exitosamente, pero la pata B se estrella contra un Orderbook "Espejismo" que se desapareció 1ms antes. Esto deja al fondo "Descubierto" (Long/Short) y a merced de la volatilidad pura por Slippage Involuntario. Es la falla de quiebra número #2 en firmas cuantitativas.
- Riesgo Legal de Asociación: Algunos exchanges (Binance, OKX) cancelan las cuentas de usuarios que operan sospechosamente y realizan Wash Trading (Fingir compras). Si el bot sigue ciegamente a un bot Wash Trader operando contra él, puede ser baneado del exchange por asociación AML.

## 14. Integración con otras skills
- Receptora primaria del flujo atómico del Local Orderbook Tracking (Skill 33).
- Proporciona el "Toxicity Score" al ML Engine (Skill 47) y al Optimizador de Sizing (Skill 2).

## 15. Modelo de datos sugerido
```json
{
  "OrderFlowToxicityReport": {
    "symbol": "BTC_USDT",
    "timestamp_ms": 1714521234105,
    "toxicity_score": 0.92,
    "state": "TOXIC_SPOOFING_ACTIVE",
    "order_cancellation_ratio": 0.98,
    "flicker_frequency_hz": 45,
    "divergence_tape_book_pct": 85,
    "action": "VETO_MAKER_ORDERS_AND_BAIT_TAKES"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase de Filtro Dinámico (`ToxicFlowDetector.evaluate(arbitrageTarget)`). Corriendo en Pipeline estricto dentro de la validación del Orquestador (Skill 36).

## 17. Logs obligatorios
- `[DEBUG] Flow Status Normal. OCR: 0.15. Toxicity: 0.05. Clear to execute arbitrage route.`
- `[WARN] Flickering Bid detected at level 65,000. 45 cancellation events in 1000ms. Tagging level 0 as PHANTOM_LIQUIDITY. Adjusting spread math.`
- `[CRITICAL] TOXIC SPOOFING TRAP DETECTED! Massive divergence between Tape and Orderbook on Arbitrum L2. Vetoing all $10k+ execution routes.`

## 18. Métricas obligatorias
- `flow_toxicity_score_rolling_avg`.
- `trades_vetoed_by_toxicity_filter_count`.
- `phantom_liquidity_detected_volume_usd`.

## 19. Tests unitarios
- Tasa de Cancelación Falsa: Inyectar 100 eventos `update(vol=100)`, seguidos de `update(vol=0)` (Cancelación) en el mismo nivel de precio en < 100ms sin reportar `Trade_Filled` en el stream de mercado. El detector DEBE emitir el Flag de `SPOOFING_ALERT`.
- Ignorar el Ruido Sano: Inyectar fluctuaciones naturales del 10% en volumen a lo largo del orderbook provenientes de docenas de cuentas. El Score Toxico debe permanecer debajo de `0.3` sin levantar pánico falso.
- Phantom Math Filter: El Optimizador envía consulta "Cuento con $50,000 aquí?". El Orderbook tiene $50,000, pero la flag de Tóxico está activa. El filtro debe interceptar y devolver `$0.00 valid liquidity` al matemático, desarticulando el arbitraje perdedor.

## 20. Tests de integración
- Levantar un Backtest interno (Skill 46) corriendo sobre el histórico del colapso de FTX o eventos de liquidación macro. Validar cuántos "Falsos Arbitrajes Positivos" que la matemática juraba perfectos y perdedores, son interceptados y bloqueados por esta protección de Toxicidad, elevando el Sharpe Ratio neto.

## 21. Tests E2E
- El Agente está activo en Bybit y Binance. Bybit presenta un Spike colosal (Un spoofer lanza $25 Millones en compras a 1 centavo bajo el precio). El spread visual es 1.5% a favor del bot HFRC. El Algoritmo 13 manda la alerta de Fuego. El Algoritmo 36 invoca al Filtro Tóxico 49. El filtro 49 nota que el Cancellation Ratio de esa muralla de $25M es absurdo y nunca se ejecuta un trade real. Lanza el Veto. El Orquestador mata el hilo. Al milisegundo siguiente, la muralla de $25M desaparece mágicamente de Bybit, y cientos de otros bots novatos que saltaron al pozo quedan aniquilados por el Slippage perdiendo capital. El bot HFRC sigue con su PnL intacto y espera el siguiente tick sano.

## 22. Checklist de producción
- [ ] Incorporación de Alisamiento Exponencial (EWMA) en el Toxicity Score. El ratio de toxicidad no debe brincar 0-1-0 agresivamente en cada tick, debe seguir una ola fluida (Smooth Moving Average) para no hacer "Thrashing" (Prender y apagar el bot por microsegundos).
- [ ] Considerar Diferencias entre Spot y Derivados: En Futuros Perpetuos es común el "Hedging" dinámico de market makers que parece Spoofing pero es lícito. Relajar el umbral tóxico en Perpetuos respecto a Spot y DEXes.
- [ ] Lectura del "Tape" (Market Trades reales). Jamás fiarse únicamente de Deltas L2. El Flujo Tóxico sólo se revela al comparar lo que "Dicen que van a hacer" (Orderbook) contra lo que "Realmente hacen y se confirma L1" (El Tape).

## 23. Ejemplo de configuración no hardcodeada
```yaml
toxicity_detection_engine:
  max_acceptable_order_cancellation_ratio: 0.85
  flicker_detection_time_window_ms: 1000
  minimum_size_for_spoof_wall_usd: 50000.0
  toxicity_score_panic_threshold: 0.80
  decay_factor_ewma: 0.15 # Higher means faster drop off of toxicity memory
```

## 24. Ejemplo de pseudocódigo
```javascript
class FlowToxicityFilter {
    constructor() {
        this.cancelEvents = new RollingWindow(1000 /*ms*/);
        this.fillEvents = new RollingWindow(1000 /*ms*/);
        this.toxicityScore = 0.0;
    }

    onOrderBookDelta(price, oldVolume, newVolume) {
        if (newVolume === 0 && oldVolume > CONFIG.min_spoof_wall) {
            // High probability of cancellation/flicker
            this.cancelEvents.add();
        }
    }

    onPublicTradeFill(volume) {
        this.fillEvents.add(volume);
    }

    evaluateToxicity() {
        const cancels = this.cancelEvents.count();
        const fills = this.fillEvents.count();
        
        // OCR Metric
        let ocr = 0;
        if ((cancels + fills) > 10) { // Require minimum sample size
            ocr = cancels / (cancels + fills);
        }
        
        // EWMA smooth toxicity update
        this.toxicityScore = (this.toxicityScore * (1 - CONFIG.decay_factor)) + (ocr * CONFIG.decay_factor);
        
        if (this.toxicityScore > CONFIG.panic_threshold) {
             EventBus.emit('TOXIC_FLOW_DETECTED', this.toxicityScore);
        }
    }

    // Called synchronously by Math Engine before firing
    isExecutionSafe() {
        return this.toxicityScore < CONFIG.panic_threshold;
    }
}
```

## 25. Criterio final de excelencia
El Filtro de Order Flow Toxico actúa como un Sistema Táctico Anti-Engaños a nivel militar. Mientras todos los otros módulos confían ciegamente en la data empírica del CEX/DEX creyendo que "Si está en pantalla, es real", este componente tiene "Malicia Institucional" instalada, detectando las ilusiones ópticas matemáticas que arruinan fondos quant y bloqueando la codicia del algoritmo al salvar la caja fuerte en la sombra.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Market Makers Genuinos vetados en mercados ilíquidos (Falsos Positivos) si el umbral de OCR es ajustado demasiado estricto. (Ajuste requerido vía Backtesting).
- Dependencias: Order Book Syncing y Trade Stream WebSockets.
- Próxima skill: MEV Blocker & Private Transaction Routing (Skill 50).
