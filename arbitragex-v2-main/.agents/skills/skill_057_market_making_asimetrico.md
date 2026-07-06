# SKILL 057 — Market Making Asimétrico (Bid/Ask skewing)

## 1. Propósito superior
Generar retornos consistentes y pasivos proporcionando liquidez (Proveyendo Bids y Asks al libro) al mercado ("Market Making"). A diferencia del arbitraje "Taker" clásico que ataca el mercado robando liquidez, esta Skill "Maker" cuelga órdenes en ambos lados del Spread y gana dinero cuando los demás traders cruzan sus órdenes (Cobrando el Spread e incentivos CEX Fee Rebates). Su poder reside en el **"Asymmetric Skewing"** (Inclinación Asimétrica): El bot desplaza continuamente su centro de gravedad (Precio Base) usando Señales de ML (Skill 47) e Inventario para NUNCA quedarse atrapado con las manos llenas del activo equivocado en una tendencia fuerte (Adverse Selection Avoidance).

## 2. Nivel de conocimiento requerido
Quant Institutional Market Maker (Estilo Jane Street / Wintermute). Matemáticas Estocásticas de Control de Inventario (Modelo Avellaneda-Stoikov), Dinámica de "Tick Size", Order Book Queue Position (Posición en la fila), y Modelado de Toxicidad Informacional de Flujo (Glosten-Milgrom).

## 3. Capacidades principales
1. Quote Generation Continua: Pone constantemente en el exchange órdenes Límite Buy (Bid) y Sell (Ask) rodeando al precio medio (Mid-Price). Ej. Compra a $100.00, Vende a $100.05.
2. Inventory Auto-Balancing (Avellaneda-Stoikov Skewing): Si el bot tiene "Demasiado BTC" en su inventario (Ej. Target 50%, actual 80%), el bot se "Asusta". Baja agresivamente su precio Ask (para vender más rápido y fácil) y baja su precio Bid (para dejar de comprar). Así, fuerza al mercado a igualar su inventario de vuelta a 50/50.
3. Volatility Spread Expansion (Expansión Dinámica): Si la volatilidad salta bruscamente (HMM Regime - Skill 48), poner órdenes pegadas al precio medio es un suicidio (Serán arrolladas por la tendencia). El bot "Abre las piernas" (Widens the Spread), exigiendo un Spread del 0.5% en lugar del 0.05% para compensar el altísimo riesgo de mercado de estar colgado pasivamente.
4. Predictor-Guided Skewing (Inclinación por ML): Si el XGBoost (Skill 47) predice que el precio va a subir en 500ms, el Bot mueve sus Asks a las nubes (Para no vender barato a los atacantes) y sube sus Bids (Para pescar compras pasivas antes del salto). Usa la IA para defenderse de ballenas front-running.
5. Micro-Pennying (Salto de Fila): Detecta competidores (Otros Market Makers bots) y pone órdenes sistemáticamente a 1 solo céntimo ("Tick Size" mínimo) por encima de ellos (`Su_Bid + 0.01`) para robarles el primer lugar en la cola de ejecución del CEX.
6. Cancel-Replace de Alta Frecuencia (C/R Limit Bypass): Actualiza los precios miles de veces por minuto. Utiliza los Endpoints Batch y Websocket avanzados del exchange para modificar órdenes sin cancelar-recrear pesadamente, ahorrando límites de tasa (Skill 35).
7. Gestión de Toxicidad de Ráfagas (Burst Toxic-flow detection): Si el lado Ask es golpeado (Llenado) 3 veces en 100ms, asume Información Tóxica Asimétrica (Alguien sabe algo que el bot no). Cancela inmediatamente el lado Bid y corre al refugio para evitar ser vaciado completamente por la tendencia entrante.
8. Aprovechamiento de Fee Rebates (Cobro de Peajes): Operar en CEXes donde el Taker paga 0.10% y el Maker COBRA -0.01% (Rebate). La estrategia a veces no gana en el Spread puro, sino que vive de recolectar el Maker Rebate Cero-Riesgo millones de veces al mes.
9. Cross-Exchange Reference Pricing (Oráculo Base Externo): Si hace Market Making en un Exchange ilíquido (Ej. MEXC), usa el Mid-Price robusto de Binance (Skill 32/33) como su "Precio Justo Verdadero" (Fair Price Oracle). Si alguien trata de manipular MEXC con $10, el bot no se mueve porque Binance no se movió.
10. Hedging Direccional Continuo (Cobertura): Si las órdenes se llenan desbalanceadas y acumula exceso de Riesgo, el bot usa la Skill 14 o 36 para ir a OTRO exchange o Futuros y "Shortea" la diferencia al mercado (Taker Hedge), cerrando la exposición instantáneamente a cambio de un seguro premium.

## 4. Entradas requeridas
- `inventory_status`: Cantidad exacta actual de USDT vs BTC en el almacén del bot (Skill 40).
- `fair_price_reference`: El precio "Real" descontando ruidos locales (Medias ponderadas de volumen externo).
- `market_volatility_metrics`: Desviación estándar u Olas HMM provenientes de Skill 48 y 37.

## 5. Salidas esperadas
- `bid_ask_quotes`: Comandos continuos de colocar/cancelar órdenes Límite Post-Only L2 (Maker).
- `inventory_hedge_orders`: Órdenes a mercado L1 defensivas en otros recintos para re-balancear (Taker).
- `cancel_all_panic_signal`: Evento para borrar todo el libro si detecta latencia excesiva con CEX.

## 6. Reglas inmutables
- TODAS las órdenes de Market Making DEBEN emitirse con la flag HTTP/FIX estricta de `POST_ONLY`. Esto garantiza que si el cálculo falló y la orden cruzaría el libro (Convirtiéndose en Taker pagando Fee alto), el Exchange la RECHAZA automáticamente, evitando un slippage de fees autodestructivo letal por errores de código.
- Obligatorio Inyectar Lógica de Auto-Halt (Parada). Si la conexión a WebSockets `Ping` de Binance (Fair Price Reference) sube a > 200ms, todas las órdenes pasivas locales deben ser CANCELADAS a la velocidad de la luz. Hacer Market Making "A Ciegas" te dejará arruinado por "Stale Quotes" (Cotizaciones Viejas).
- La gestión de Ráfagas es absoluta: Nunca permitir que más del X% (Ej. 10%) del capital base sea "Llenado" (Filled) en el mismo lado en un intervalo de 10 segundos sin activar un Halt o Skew Severo de Inventario (Risk of Adverse Selection Limit).

## 7. Algoritmos o métodos que debe conocer
- Ecuaciones de Avellaneda y Stoikov (1998 High-Frequency Trading in a Limit Order Book).
- Inventory Risk Aversion Parameter (Gamma).
- Lógica de Microestructura Tick-size y Lot-size.
- Queue Imbalance Forecasting.

## 8. Fórmulas críticas
- **Reservation Price (Centro Asimétrico)**: `Reserve_Price = Fair_Price - (Inventory_Position * Gamma * Volatility_Variance * Time_Horizon)`
- **Half-Spread Optimal**: `Spread = (Gamma * Volatility_Variance * Time_Horizon) + ( (2/Gamma) * Ln(1 + (Gamma/Liquidity_Depth_K)) )`
- **Bid/Ask Quotes Finales**: `Bid = Reserve_Price - (Spread / 2)`, `Ask = Reserve_Price + (Spread / 2)`

## 9. Casos extremos
- Trend Steamroller (Aplastamiento Tendencial): El mercado entra en un Rally alcista demente de +10%. El Bot vende (Ask) en 100. Sube a 105. El Bot vende en 105. Sube a 110. El bot vende en 110. El bot ha vendido TODO su BTC en la subida, se queda en USDT. El precio sube a 150. El Bot se comió un "Inventario Sesgado a Cero" en el peor momento. Solución: El Avellaneda Skew asimétrico hace que al vender el primer tramo, el precio Ask salte a +50% distorsionando el mercado para defenderse de vaciarse (Inventory Aversion Parameter = High).
- Toxic Flow / Latency Arbitrage Attack: Un grupo de bots ultrarrápidos compran masivamente a tu Maker (Ask) 1 milisegundo antes de que se anuncie una ruptura al alza. Ellos saben que eres más lento y te roban tu inventario barato (Adverse Selection). Solución: Integración profunda con Signal ML (Skill 47) y Order Flow Toxico (Skill 49) que levanta los precios (Pull Quotes) si la toxicidad cruzó un límite umbral.
- Exchange Outage (Apagón CEX): El Exchange entra en Maintenance Mode, pero los websockets quedan colgados. Solución: Heartbeats. El Bot asume la muerte del CEX si no hay Orderbook Updates en 500ms y manda a cancelar vía REST a ciegas para estar seguro al reinicio.

## 10. Validaciones obligatorias
- PRE: Validar "Tick Sizes" (Step de precio). Si Avellaneda dice Bid = $100.12345, pero el exchange solo acepta dos decimales (`100.12`), aplicar función `Math.floor/ceil` de forma perenne. Fallar esto resulta en la API rechazando el millón de Quotes por hora asfixiando logs.
- CÁLCULO: Incorporar en tiempo real un Oráculo de Latencia Relativa. Si estás operando en Kucoin pero dependes del MidPrice de Binance, restar o sumar el Drift del Reloj a tus spreads, para cubrirte del margen de error temporal (Skill 34).
- POST: Un Market Maker NO gana dinero por cada Trade individual como el Arbitrajista espacial (Muchos Trades dan Loss en papel a corto plazo). La validación PnL es Macro: ¿Tras 100,000 cruces, la recolección de Spread Maker neto supera las pérdidas estocásticas de Market Risk?.

## 11. Criterios de aprobación
- Capacidad matemática de re-calcular los límites (Bids/Asks) de 50 activos y actualizar las órdenes en el Exchange en < 20ms ante CUALQUIER movimiento sustancial del OrderBook de Referencia.
- El Inventario local se mantiene oscilando armónicamente cerca del 50/50% base (Ideal Risk-Neutral State) validando que las fórmulas estocásticas están repeliendo desequilibrios exitosamente.

## 12. Criterios de rechazo
- El Bot utiliza "Averaging Down" (Martingala / DCA infinito) sin límites de inventario. (Compró en $100 y bajó, así que compra doble en $95, y doble en $90). Eso no es Market Making, es un Algoritmo Direccional suicida de apuestas. Riesgo 100% inaceptable.
- Generación de Tráfico Inútil (Over-quoting Penalty). Modifica la orden 10,000 veces por milisegundo por variaciones microscópicas inútiles del FairPrice que ni siquiera cruzan 1 céntimo ("Tick Size"), lo que garantiza un IP Banned de Binance en 5 segundos.

## 13. Riesgos que mitiga
- La Dependencia Extrema de Oportunidades "Taker" Puras (Taker Exhaustion Risk): Con 100 fondos billonarios usando Fibra Óptica Láser, a veces tú no puedes ser el primero en robar el spread cruzando el límite (Skill 13). Pero si tú *ERES* el límite (Haces Market Making), los robots de Jane Street chocarán contra TI pagándote el spread. Volteas la tortilla.
- Cuentas Congeladas por Bajo Ratio Maker/Taker: Muchos CEX obligan a los HFT a proveer liquidez (Maker Volume > 80%) o si no los multan. Esta skill genera volumen pasivo masivo necesario para subir a VIP-9 institucional rebajando todas las demás comisiones del fondo general.

## 14. Integración con otras skills
- Cliente masivo de Rate Limit Bypass (Skill 35) por la inmensa cantidad de Cancel/Replace APIs.
- Extrae el Sentimiento Micro del ML Engine (Skill 47) para mover asimétricamente los precios.
- Responde dócilmente al HMM Regime Detector (Skill 48) ampliando o colapsando Spreads.

## 15. Modelo de datos sugerido
```json
{
  "MarketMakingQuotes": {
    "pair": "SOL_USDT",
    "timestamp_ms": 1714521234105,
    "fair_price_reference": 140.50,
    "inventory_ratio": 0.85, // Heavy on SOL, Light on USDT
    "avellaneda_reserve_price": 140.40, // Skewed downwards to dump SOL
    "calculated_spread_usd": 0.20,
    "orders_dispatched": [
      { "side": "BUY", "price": 140.30, "size": 10.5, "postOnly": true }, // Way below fair price
      { "side": "SELL", "price": 140.50, "size": 10.5, "postOnly": true } // Exact fair price to force execution
    ],
    "toxicity_status": "NORMAL"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase `AvellanedaStoikovBot` instanciada por par de activo. Ejecuta un Event Listener de alta reactividad sobre la Skill 33 (OrderBook L2). Evalúa continuamente `updateQuotes()`. Utiliza Batched HTTP API o FIX Gateway para inyectar/cancelar órdenes 10 a la vez para minimizar round-trips.

## 17. Logs obligatorios
- `[DEBUG] Avellaneda Recalculation (ETH/USDT): Inv 60%, Volatility High. ReservePrice: $3500.20. Spread widening to 0.45%.`
- `[INFO] Maker Order Filled! (SELL 1.5 ETH @ 3505.00). Inventory shifting towards 50%. Earned $2.50 Maker Rebate.`
- `[WARN] TOXIC AVALANCHE DETECTED (Skill 49 Trigger). Suspending all Maker Bids indefinitely. Shifting Asks +5% to protect inventory from dump.`

## 18. Métricas obligatorias
- `inventory_alpha_skew_rolling_average` (Cuán desviado del 50/50 estuviste en el día).
- `maker_volume_generated_24h_usd`.
- `order_cancel_replace_ratio_per_minute` (Optimizar no mandar ruido innecesario al CEX).

## 19. Tests unitarios
- Avellaneda Math Skew: Inyectar un ratio de Inventario `Q = +500` (El bot está ultra-cargado del Token X frente a dólares). La función de Reserva DEBE arrojar un Precio Central drásticamente MENOR al FairPrice (Descuento agresivo) para forzar al mercado a comprarte tus X y vaciarte las bolsas rápidamente. Comprobación matemática estricta de la aversión al riesgo (Gamma).
- C/R Noise Reduction (Filtro de Ruido): Proveer un cambio de `Fair Price` de $100.000 a $100.001. Si el Tick Size de Binance es `0.01`, la función de cotización `shouldUpdateQuotes()` debe devolver `FALSE`, evitando quemar llamadas API en algo que visualmente y matemáticamente en el L2 Book es estático e indivisible.
- Post-Only Assurance: Mapear una inyección donde el bot calcula `Bid > FairPrice` (Generaría un Market Buy cruzando el spread). El Validador Maestro interno de la Skill DEBE interceptarlo en O(1) in-memory y cancelar el dispatch para evitar el despilfarro (Taker Fee suicide).

## 20. Tests de integración
- Levantar un CEX Mocked (Express/Mongoose L2 Simulator) donde bots Takers estúpidos (Ruido Browniano Aleatorio) compran y venden a mercado 10 veces por segundo. El Avellaneda Market Maker del agente se enchufa a este servidor simulado. Correr por 10 minutos. Verificar que el bot absorbe todo el flujo, que su inventario nunca llega a cero o 100%, y que su PnL neto es positivo extraído exclusivamente de la colección de los spreads.

## 21. Tests E2E
- El fondo opera en Bybit con Taker = 0.05% y Maker = 0.00% (Free). El mercado entra en fin de semana (Consolidación lateral aburrida). Las Skills de Arbitraje Cross-Exchange (Skill 54) se inactivan al no haber gaps. Entra en juego la Skill 57. Monitorea el flujo errático de "Retail Traders" (Usuarios normales que usan mercado). Cotiza constantemente $0.5 por encima y debajo del MidPrice de Solana. Cuando el mercado baja, le pegan a sus Bids, llenándolo de SOL, bajando el precio base para deshacerse de él a $1 abajo. Cuando sube, el mercado barre sus Asks devolviéndolo a Dólares. Al final del domingo, no hubo tendencias macroeconómicas reales, pero el Agente generó cientos de miles de dólares pasivos al capturar el "Micro-rebote" microscópico 50,000 veces.

## 22. Checklist de producción
- [ ] Cuantización Asimétrica en Bear Markets: La función de aversión debe aceptar "Overrides Funcionales" Humanos/Macro. Si la FED sube tasas y el cripto invierno acecha (Tendencia Secular), el Inventario Base Neutral no debe ser 50/50%. Debe ser "10% Cripto / 90% Stablecoins" como Baseline para que el MM rechace atrapar el colapso perenne.
- [ ] Uso Intensivo de `Cancel-All-On-Disconnect` (Kill Switches a Nivel Exchange): API Keys del MM DEBEN configurarse en Binance/OKX para que el propio Servidor Central del Exchange borre TODAS tus órdenes si tu WebSocket Node.js deja de hacer "Ping" por 5 segundos. Es el único rescate infalible si el datacenter del bot se incendia mientras tus Asks están flotando expuestos a ballenas L2.
- [ ] Descuento Temporal (Tick decay): Si una orden Límite Maker no es llenada en 15 minutos, está "Stale" (Rancia) y estanca liquidez valiosa. Cancelar progresivamente órdenes viejas y acercar liquidez a la media móvil nueva.

## 23. Ejemplo de configuración no hardcodeada
```yaml
asymmetric_market_maker_engine:
  active_pairs_market_making: ["ETHUSDT", "SOLUSDT"]
  avellaneda_params:
    gamma_risk_aversion: 0.15 # Controls how scared the bot is of holding too much of one asset
    kappa_liquidity_depth: 1.5
    volatility_window_ms: 60000 # 1 Min rolling variance
  minimum_spread_hard_floor_bps: 2.0 # Never quote tighter than 0.02% to guarantee minimum profit
  toxicity_veto_enable: true
  inventory_target_ratio_base_asset: 0.50 # 50% neutral base
```

## 24. Ejemplo de pseudocódigo
```javascript
class AvellanedaMarketMaker {
    constructor(assetConfig) {
        this.gamma = CONFIG.gamma_risk_aversion;
        this.kappa = CONFIG.kappa_liquidity_depth;
    }

    async calculateAndDispatchQuotes(fairPriceUsd, inventoryRatio, volatilityVar) {
        // Avellaneda-Stoikov Formula Implementation
        const inventoryPosition = (inventoryRatio - CONFIG.inventory_target_ratio) * 100; // e.g. +30 if holding 80%
        
        // 1. Skew the Fair Price based on fear of inventory
        const reservationPrice = fairPriceUsd - (inventoryPosition * this.gamma * volatilityVar);
        
        // 2. Calculate dynamic optimal spread based on market danger (volatility)
        const spread = (this.gamma * volatilityVar) + ((2 / this.gamma) * Math.log(1 + (this.gamma / this.kappa)));
        
        // 3. Enforce hard floors (Do not work for free)
        const finalSpread = Math.max(spread, fairPriceUsd * (CONFIG.min_floor_bps / 10000));
        
        // 4. Generate Bids/Asks
        const bidPrice = Math.floor((reservationPrice - (finalSpread / 2)) * TICK_MULT) / TICK_MULT;
        const askPrice = Math.ceil((reservationPrice + (finalSpread / 2)) * TICK_MULT) / TICK_MULT;

        // 5. Fire to API (Only if changed beyond threshold to prevent IP Banning noise)
        if (this.shouldUpdateQuotes(bidPrice, askPrice)) {
            await ExchangeApi.submitPostOnlyBatchOrders(bidPrice, askPrice);
            this.lastQuotedBid = bidPrice;
            this.lastQuotedAsk = askPrice;
        }
    }
}
```

## 25. Criterio final de excelencia
El Market Maker Asimétrico transforma la firma cuantitativa en un "Peaje Ineludible" de la infraestructura financiera global. Dejar de ser el cazador frenético que persigue migajas Taker de milisegundos, para convertirse pasivamente en el Creador de Liquidez robusto y cínico (Bid/Ask Provider) que exprime comisiones y atrae liquidez, gestionando el riesgo direccional con el genio estocástico que catapultó a imperios como Citadel Securities a la cima del mundo.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Flash Crash Gaps Reversibles. El precio salta la orden Bid agresivamente, te llena de un token sin liquidez, y nunca puedes ejecutar el Ask para salir. El modelo se apalanca en el Stop-Loss macro y HMM para sobrevivir.
- Dependencias: Order Flow Toxicity (Skill 49), ML Engine (Skill 47), y Data Normalized Reference (Skill 32).
- Próxima skill: Cross-Protocol Yield Farming (Rate Arbitrage) (Skill 58).
