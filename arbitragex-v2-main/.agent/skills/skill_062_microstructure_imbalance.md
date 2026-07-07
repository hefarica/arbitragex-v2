# SKILL 062 — Análisis de Micro-Estructura (Order Imbalance)

## 1. Propósito superior
Detectar de manera predictiva la dirección microscópica del precio a corto plazo (Milésimas a Segundos) "Leyendo la Mente" y las intenciones matemáticas del Mercado L2 (El Libro de Órdenes). Utilizando Order Imbalance (Desequilibrio de Órdenes), Volume Delta acumulado, y Análisis de Micro-Estructura Pura, esta Skill funciona como el Radar Predictivo Táctico de HFT. Le dice al Bot que NO intente comprar en $100 porque hay un gigantesco "Muro de Órdenes Límite de Venta Falsas" colgadas (Spoofing invisible) y una presión atómica que empujará el precio inevitablemente a $99.90 en los próximos 100 milisegundos. 

## 2. Nivel de conocimiento requerido
Quant HFT (High-Frequency Trading) Especialista en Micro-estructura. Modelos Predictivos de "Order Book Dynamics" (Bid/Ask Queue Theory, Probability of Limit Order Fill), Modelado del VPIN (Volume-Synchronized Probability of Informed Trading), Filtros de Kalman y Cálculo Computacional de L2 Deltas en O(1).

## 3. Capacidades principales
1. Order Book Imbalance (OBI) Real-Time: Sumatoria estática del peso de los Bids frente a los Asks en los primeros N Ticks. Ej. Si hay 1000 BTC buscando Comprar y 5 BTC buscando Vender en el Top-10 Bids/Asks, el "Imbalance" es `+99%`, prediciendo empuje alcista atómico.
2. Order Flow Delta (CVD - Cumulative Volume Delta): En el flujo de "Mercado" (Trades ya ejecutados - Skill 33), resta los Volúmenes "Comprados agresivamente (Takers en Ask)" de los "Vendidos agresivamente (Takers en Bid)". Si el delta es negativo, el Momentum es Taker Sell masivo (Sangrado inminente).
3. Iceberg Order Detection (Detección de Bloques Ocultos): El bot ve a un bot institucional intentando vender 1,000 BTC sin asustar al mercado usando órdenes "Iceberg" (Muestra 10 BTC, y cada vez que se llenan, aparecen mágicamente otros 10). La Skill intercepta matemáticamente el "Refill Rate" asumiendo a una Ballena Oculta distribuyendo y generando un Veto de Riesgo bajista.
4. Tasa de Cancelación Estructural (Spoofing Cancels): Monitorea la vida útil de las órdenes límite colgadas. Si detecta millones de dólares en Asks que "Desaparecen" un milisegundo antes de ser tocados, infiere presión engañosa, informando a la Skill 49 (Toxicidad Lógica) y evitando ser un "Bagholder" del Market Maker malicioso.
5. Bid/Ask Quote Replenishment Speed: Identifica la velocidad a la que los Creadores de Mercado re-ponen (Replenish) la liquidez tras un trade masivo. Si el Lado de Compra tarda 2 segundos en llenarse pero el Lado de Venta se rellena en 5 milisegundos, existe un sesgo (Asymmetric Skew, Skill 57) algorítmico institucional latente hacia abajo.
6. Probability of Execution Oracle (Probabilidad Límite): Calcula probabilísticamente si "Tú" pones una orden Bid a $100.05, ¿En cuántos segundos se ejecutará y a qué riesgo direccional adverso? Ajusta el Avellaneda Model limitando posiciones si P(Fill) es alto pero el Expected Reversion es bajo.
7. Tick-Test/Lee-Ready Inferencing: Muchas APIs no te dicen si un Trade es Buy o Sell. Este Módulo compara el Precio de Ejecución vs el Precio Límite anterior para Inferir Lógicamente quién inició la agresividad direccional (Informed Flow Directionality).
8. Detección de Falsa Liquidez Profunda (Thick/Thin Book Trap): Los creadores pueden poner $10 Millones a -$10% de distancia para que el L2 Book luzca "Sano y Sólido" engañando a los bots torpes. Esta Skill impone Decaimiento Exponencial de Peso al volumen basado en "Distancia al Mid-Price", ignorando matemáticamente el Capital Falso lejano.
9. Queue Position Advantage Tracking: Estima tu propio lugar en la "Fila" de coincidencias L2 ("Queue Priority"). Evitando que canceles y modifiques una orden (Lo que te envía al final de la fila CEX FIFO) si la recompensa (Tick Price) es menor a la pérdida del derecho de antigüedad de la orden.
10. Señal Agresiva de Predicción O(1): Actúa como un Multiplicador de Confianza ("Confidence Boosting") para el Arbitraje Espacial y Estadístico. Si el Arbitraje Espacial L1 quiere Comprar en Binance y Vender en OKX, y el Micro-structure dice: "Binance Orderbook is Imbalanced Bullish, OKX is Imbalanced Bearish", la confianza sube al 200%.

## 4. Entradas requeridas
- `websocket_l2_orderbook_deltas`: Flujo de Ticks O(1) inyectado de la Skill 33 con adiciones/cancelaciones microscópicas de cada nivel de precio.
- `websocket_live_trades_tape`: El "Time and Sales" Stream (Cinta en vivo) reportando el volumen atómico crudo emparejado (Takers Hit).
- `tick_size_and_lot_config`: Las restricciones físicas de precio del activo local.

## 5. Salidas esperadas
- `order_book_imbalance_ratio`: Float continuo entre `-1.0` (Bearish Max) a `+1.0` (Bullish Max).
- `cumulative_volume_delta_1m`: Medidor de presión compradora/vendedora en ventana móvil.
- `micro_trend_prediction_vector`: Tensor o Score unificado para el XGBoost (Skill 47) o la GNN (Skill 56).

## 6. Reglas inmutables
- JAMÁS promediar planamente todo el volumen del Order Book. El Peso (Impacto en Imbalance) del Volumen en el nivel de precio Límite 1 (Bid1) vale exponencialmente más (Ej. 10x más peso) que el volumen en el límite 20 (Bid20). Ignorar el "Distance Decay" creará predicciones estadísticamente ruidosas e inservibles para el HFT Predictivo.
- La Computación del OBI (Order Imbalance) DEBE completarse en la Memoria RAM Local sin Garbage Collection Bloqueante, ejecutándose en < 100 Micro-segundos (0.1ms). Retrasar este cálculo por Serialización JSON/Lógica Pesada equivale a leer un periódico de ayer para esquivar una bala en HFT.
- En regímenes HMM Volátiles Extremos (Skill 48), el Order Imbalance tradicional pierde poder predictivo porque los Market Makers institucionales huyen del libro de órdenes dejando un "Mercado Delgado" (Thin Market) propenso a saltos aleatorios ciegos. El Peso de esta Skill sobre la decisión final debe decaer en Chaos Regimes.

## 7. Algoritmos o métodos que debe conocer
- Decaimiento Exponencial de Ponderación L2 (Exponential Decay Weighting based on Tick Distance).
- Volume Profile & VPVR (Volume Profile Visible Range) a Nivel Microscópico.
- Modelos Hawkes / Poisson para Agrupamiento de Lados de Agresores (Aggressor Trade Clustering).
- Micro-Price Calculation (Alternativa al Precio Medio basada en densidades de Liquidez).

## 8. Fórmulas críticas
- **Order Book Imbalance (Simple)**: `OBI = (Bid_Vol_L1 - Ask_Vol_L1) / (Bid_Vol_L1 + Ask_Vol_L1)`
- **Order Book Imbalance Ponderado (Micro-Price / MidPrice)**: `MicroPrice = Bid_Price * (Ask_Vol / Total_Vol) + Ask_Price * (Bid_Vol / Total_Vol)` (El precio justo tenderá hacia la zona de menor liquidez (Resistencia menor)).
- **CVD (Cumulative Volume Delta)**: `CVD = Sumatoria(Taker_Buy_Volume) - Sumatoria(Taker_Sell_Volume)`

## 9. Casos extremos
- Invasión Algorítmica de Alta Frecuencia (Wash Trading Flood): El mercado se estanca pero de repente la Cinta (Trades L2) empieza a reportar 5,000 operaciones por segundo de tamaño "0.000001 BTC" en un CEX cuestionable. Este volumen inunda las métricas tradicionales. El analizador Micro-estructural detecta el ruido sintético, lo agrupa y filtra por tamaño atípico ("Ping Trades"), desactivando métricas CVD de engaño hasta que el Wash Trading cese (Detección Sybil).
- Fake Walls (Muros Límite Falsos): Se detecta una Orden Ask en 65,000 por valor de $50 Millones, disparando el Order Imbalance brutalmente hacia `-0.95`. Pero a la hora de ejecutarse, apenas un Tick de distancia (64,999) la orden desaparece como magia negra (Flash Cancel). El Imbalance salta instantáneamente a `0`. El sistema DEBE guardar la variable "Spoof_Score_History" por Nivel de Precio para anular mentalmente muros con alta Tasa de Cancelación, purificando la métrica OBI del humo institucional de los Market Makers deshonestos.
- Flash Crashes Latentes (Liquidity Vacuuming): La API reporta que el Spred "Bid-Ask" pasó del normal `0.01%` a `0.20%` repentinamente sin caída de precios (Miedo del Creador de Mercado). El Vacuum Inminente se levanta. Cualquier Market Order Taker destrozará el PnL. Skill 62 prohíbe instantáneamente el uso de Órdenes a Mercado a las Skill 12-15 de Arbitraje.

## 10. Validaciones obligatorias
- PRE: Asegurar que el Motor de Actualización Local del Orderbook (Skill 33) no sufra de "Libro Desfasado" (Crossed Books, donde localmente tienes `Bid > Ask`). Si la microestructura está corrupta por latencia Websocket L2, este Módulo de Imbalance propagará basura estocástica, induciendo decisiones inversas perjudiciales (Garbage In / Garbage Out Fatal).
- CÁLCULO: Mantener un Histograma Interno Móvil (Circular Buffer O(1)) de los últimos `N` trades y sus Deltas sin disparar pérdidas de memoria (Memory Leaks en lenguajes como JS o Rust sin drop).
- POST: Incorporación directa del `MicroPrice` (Precio Asimétrico OBI) en los cálculos de Avellaneda Stoikov del Market Maker Propio (Skill 57).

## 11. Criterios de aprobación
- Entrega del tensor unificado de Micro-Estado `(OBI, CVD, MicroPrice, Spread_Bps)` al XGBoost en Tiempos de Actualización Sub-Milisegundo tras la recepción del Tick Websocket.
- Lograr separar falsas rupturas (Fakeouts) de Tendencias Verdaderas probando en Backtest HFT puro que el % de "Taker Losses" se reduce sustancialmente gracias a vetos del OBI.

## 12. Criterios de rechazo
- Promediar asíncronamente el L2 Orderbook en ventanas de "1 Segundo" o "5 Segundos" (Interval Bar Aggregation). En HFT, la estructura interna microscópica nace y muere en 50 milisegundos. Promediar es destruir la resolución espectral de los datos informados en la microestructura (Information Smearing Penalty).
- Fallar la matemática de Deltas (Suma simple) al recibir Órdenes L2 Delta `type: "update"` vs `type: "delete"`. Un Error en la gestión del Snapshot Incremental destruye la validez del Imbalance para siempre, exigiendo Request pesado de Snapshot Full REST que interrumpe la operativa.

## 13. Riesgos que mitiga
- Taker Asymmetry Trap (La Selección Adversa CEX): Vas a realizar un Arbitraje Favorable Atómico porque tus cuentas dan verde. Mandas la orden. "Alguien" te gana por 1 ms. Luego tu orden pega en el segundo mejor postor, bajando tu profit a pérdida (Adverse Selection). Pero si hubieras calculado la Micro-Estructura (CVD + Imbalance), la Máquina habría notado que una docena de ballenas ya estaban empujando hacia allá en Trades Taker paralelos (CVD en Pico), por ende, cancelaba el plan Taker ciego al entender la Inercia Mortal en tu contra antes de cruzar la línea HTTP.

## 14. Integración con otras skills
- Alimentador de Alpha Puro a la Skill 47 (Machine Learning XGBoost) y Skill 56 (Graph Neural Networks).
- Modificador Cuántico Asíncrono del Precio Justo (`FairPrice`) del creador de mercado local (Skill 57).
- Gatillo defensivo indirecto del Risk Engine de Módulo CEX (Skill 41).

## 15. Modelo de datos sugerido
```json
{
  "MicrostructureStateVector": {
    "pair": "BTCUSDT",
    "timestamp_ms": 1714521234105,
    "current_mid_price": 65000.0,
    "imbalance_obi_depth_10": -0.65, // Heavily Bearish limit order pressure
    "micro_price_weighted": 64995.12, // The true center of gravity of L2
    "cumulative_volume_delta_tick_100": -450.5, // 450 BTC dumped passively to Bids in last N ticks
    "bid_ask_spread_bps": 0.05,
    "spoofing_probability_ask_wall": 0.88, // 88% chance the giant sell wall is fake
    "signal_aggregate": "AVOID_LONG_POSITIONS_AT_MARKET"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Objeto Lógico de Actualización Continua (`OrderflowMicrostructureCore`). Cada que Skill 33 lanza el callback `onL2Update(delta)`, este Core consume el delta mediante aritmética estricta de variables in-memory primitivas C++ / Float64Array (Nada de Mapas Objeto Pesados en JavaScript).

## 17. Logs obligatorios
- `[DEBUG] Microstructure Update (ETHUSDT): Ask Liquidity dried up + OBI swung to +0.80. MicroPrice drifted to $3100.25 (Base: $3100.00). Bullish L2 divergence established.`
- `[INFO] Iceberg Detector Triggered. 15 Taker Bids executed on 65,000.00 Limit level, but Ask Volume replenished instantly without price movement. Absorption spotted.`
- `[CRITICAL] CVD Collapsed -5M USD Taker Sells in < 200ms. Market Makers pulling passive bids. Flash Crash Probability Spiked. Triggering System-Wide L2 Pause Event.`

## 18. Métricas obligatorias
- `average_obi_absolute_momentum` (Para definir si el mercado está lateral y ruidoso, o limpio y predictivo).
- `spoof_walls_detected_per_hour` (Para trazar la actividad Institucional del par/exchange y evaluar Toxicidad L2).
- `microprice_divergence_from_midprice_max_bps`.

## 19. Tests unitarios
- Ponderación Decadente de Imbalance: Proveer un Orderbook con 1000 BTC de Sell en Límite 1. Imbalance `OBI = -0.99`. Proveer el mismo Orderbook pero los 1000 BTC Sell están escondidos en el Nivel 25 del libro. La Función MicroPrice y OBI DEBE usar Decaimiento Exponencial y arrojar un `OBI = -0.05` ignorando casi por completo la pared remota, imitando la verdadera presión de impacto empírico HFT.
- Iceberg Absorbing Logic: Inyectar un historial falso de Ticks (Skill 33) donde un nivel de Ask en $100 recibe "Taker Buy=10", pero la "Ask Size" Local reportada por el CEX no se reduce en 10. La lógica del Vector DEBE incrementar el contador Local de "Iceberg Volume Absorbed" por 10 y levantar una Flag de `ABSORPTION_IN_PROGRESS`.
- Latency / OOM Bounds: Inicializar 5,000 pares simulados. Disparar el actualizador Micro-Estructural a 10,000 eventos L2 por segundo. Medir la carga O(1). Si excede 5ms de CPU Time Total Process per tick o consume > 500MB RAM residuales, el código falla por Memory Leak. FFFT (Fast Fourier Flow Time) constraints check.

## 20. Tests de integración
- Levantar Data Lake Histórico (Skill 37). Reproducir 10 minutos de Level 2 Orderbook crudo capturado durante la Caída de FTX (Cisne Negro). Extraer la Salida continua generada por Skill 62 al XGBoost local. Verificar que los vectores Imbalance/CVD giraron salvajemente al Rojo (Bajista) SEGUNDOS completos ANTES de que el MidPrice tradicional siquiera dibujara la caída de los primeros $5,000 dólares, validando la tesis Predictiva de la Microestructura L2 (Alpha Forecaster).

## 21. Tests E2E
- El agente HFRC tiene el Orquestador HFT (Skill 36) activo buscando arbitrajes L1-L2 en Base Network (Aero/USDC). El Orderbook L2 nativo de Bybit para AERGO cruje. El Agente usa Skill 62 (Microestructura). Nota que un Bot Institucional desconocido acaba de poner Muros de Compra (Bids) Falsos gigantes para asustar al mercado haciéndoles creer que hay Soporte Masivo, mientras ese mismo Bot Vende disimuladamente pedacitos (Iceberg) contra el mismo mercado que atrajo (Pump and Dump L2 Clásico). La Microestructura capta la asimetría: `Spoofing_Score=High, CVD=Negative`. Anula de inmediato todos los Cálculos de Arbitraje Espaciales que sugerían Comprar Aergo para revender. En 20 segundos el mercado colapsa, el Muro Falso desaparece (Cancelado) y los Bots tontos Takers caen en el sumidero, mientras el Agente Supremo HFRC se queda con pólvora fresca en Cash asimilando la lección.

## 22. Checklist de producción
- [ ] Incorporar Order ID Tracking en CEX que lo permitan: Algunos exchanges L2/L3 Feed y DEX On-Chain muestran el UID de la Orden Oculta. Usar esos Hashes para mapear Agresores Institucionales ("Ah, el Bot #91A está aquí. Ese Bot me hizo Spoofing ayer, lo ignoro"). (Player Profiling O(N)).
- [ ] Optimizar Float/Integer Arithmetic: El Orderbook O(1) se opera localmente sin objetos JSON. Solo Buffer Arrays Planos (TypedArrays en JS o Vectors en Rust). Calcular MicroPrice y OBI con matemática de punto flotante SIMD si se está usando C++/Rust para escalar al máximo HFT Performance sin sacrificar precisión Tick.
- [ ] Re-Calibración de la Constante Exponencial de Decaimiento por Volatilidad: La distancia que se considera "Lejano al MidPrice" cambia si Bitcoin está saltando $1000 por minuto a si está saltando $1 por minuto. El filtro OBI debe adaptarse dinámicamente usando Ventanas de Varianza de HMM Regimes (Skill 48).

## 23. Ejemplo de configuración no hardcodeada
```yaml
microstructure_analytics_engine:
  enable_live_obi_processing: true
  depth_levels_monitored: 20 # Maximum L2 levels to incorporate in weighted OBI
  obi_decay_constant_kappa: 0.15 # Controls how fast limit orders far away lose their weight
  iceberg_detection_trigger_size_usd: 50000.0
  spoofing_penalty_history_ms: 10000 # Remember cancelled walls for 10 seconds
  calculate_microprice_weighted_mean: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class MicrostructureOrderflowEngine {
    constructor() {
        this.L1_BID_INDEX = 0; // Flattened Typed Arrays for C-like speed
        this.L1_ASK_INDEX = 1;
        this.kappa = CONFIG.obi_decay_constant;
    }

    // Called O(1) immediately post-Orderbook Delta application
    processMicrostructureVector(flatL2OrderbookArray, currentMidPrice) {
        let weightedBidSum = 0.0;
        let weightedAskSum = 0.0;
        
        // Loop over depth levels calculating exponential decay weight
        for (let level = 0; level < CONFIG.depth_levels_monitored; level++) {
            const bidPrice = flatL2OrderbookArray[level * 2];
            const bidVol = flatL2OrderbookArray[level * 2 + 1];
            // Exponential decay: e^(-kappa * distance_ticks)
            const bidDist = Math.abs(currentMidPrice - bidPrice) / TICK_SIZE;
            const weightBid = Math.exp(-this.kappa * bidDist);
            weightedBidSum += (bidVol * weightBid);
            
            const askPrice = flatL2OrderbookArray[level * 2 + 2]; // Simulated offset
            const askVol = flatL2OrderbookArray[level * 2 + 3];
            const askDist = Math.abs(askPrice - currentMidPrice) / TICK_SIZE;
            const weightAsk = Math.exp(-this.kappa * askDist);
            weightedAskSum += (askVol * weightAsk);
        }

        // Output True Micro-State Imbalance [-1.0 to 1.0]
        const currentOBI = (weightedBidSum - weightedAskSum) / (weightedBidSum + weightedAskSum + 1e-9); // 1e-9 protects against zero div
        
        // Calculate the True Center of Gravity (MicroPrice)
        const totalVol = weightedBidSum + weightedAskSum + 1e-9;
        const microPrice = flatL2OrderbookArray[this.L1_BID_INDEX] * (weightedAskSum / totalVol) + 
                           flatL2OrderbookArray[this.L1_ASK_INDEX] * (weightedBidSum / totalVol);

        return { obi: currentOBI, microPrice: microPrice };
    }
}
```

## 25. Criterio final de excelencia
El Analizador de Microestructura dota al HFT Bot de la capacidad Instintiva (Sexto sentido) que diferencia a los Novatos (Que solo ven el precio Actual Histórico) de los Operadores Institucionales (Que leen la Inercia de las Órdenes Inminentes). Neutraliza trampas visuales, Orderbook Spoofing institucional y vacíos de liquidez con vectores matemáticos exactos. Actúa como el filtro predictivo principal, proveyendo Ojos de Microsegundo a las redes neuronales macro, consolidando una máquina táctica HFT a prueba de manipulaciones profundas del libro de emparejamiento L2.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Phantom Books (DEX Aggregators en L2 como 1inch) donde el libro es ininteligible on-chain o "Off-Chain RFQ / Dark Pools" que no publican Bids/Asks antes de la ejecución. Imbalance L2 inútil allí (Solo CVD sirve en Dark Pools).
- Dependencias: O(1) L2 Websocket Streamer (Skill 33), XGBoost Engine (Skill 47).
- Próxima skill: Delta-Neutral Liquidity Provision (DEX LP V3) (Skill 63).
