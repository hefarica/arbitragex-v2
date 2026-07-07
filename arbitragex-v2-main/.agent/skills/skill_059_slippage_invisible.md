# SKILL 059 — Detección de Slippage Invisible (Exchange slippage profiling)

## 1. Propósito superior
Modelar, perfilar y compensar matemáticamente los engaños en tiempo de ejecución causados por el "Slippage Fantasma" (Invisible Slippage) y los sesgos estructurales de cada CEX. En la realidad, si el Orderbook de un exchange exótico dice que puedes comprar BTC a $65,000, la API a menudo te ejecuta a $65,010 con excusas de "Retraso del motor interno" o "Peores condiciones de mercado asíncronas". Este módulo crea una Base de Datos Forense de Confianza (Trust Score) por par y por Exchange, degradando matemáticamente el profit ilusorio antes de mandar la orden para evitar ser desangrado por los motores de coincidencia (Matching Engines) deshonestos o de baja calidad.

## 2. Nivel de conocimiento requerido
Ingeniero Forense de Microestructura (HFT Forensics). Experiencia en Reconciliación Post-Trade, Modelado Analítico de Ejecución Empírica, Identificación de Latencias de Matching Engine (Asymmetric Engine Delay), Costos Friccionales No-Lineales y Construcción de Oráculos de Impacto de Precio Probabilístico.

## 3. Capacidades principales
1. Tracking Post-Trade Forense (Execution Reconciliation): Tras cada trade realizado por la Máquina, el bot intercepta el recibo (Trade Confirmation / Fill Message) y lo compara con el Precio/Condición teórica que "Prometía" el Orderbook en el milisegundo en que se envió la orden, calculando la Diferencia de Deslizamiento Exacto.
2. Perfilado de Exchange (CEX Profiling Matrix): Construye un mapa tridimensional dinámico en Memoria: `Exchange -> Pair -> Liquidity_Size -> Expected_Slippage_Bps`. Descubriendo, por ejemplo, que KuCoin tiene 3bps de slippage invisible en $1k trades, pero Binance tiene 0.1bps en los mismos trades.
3. Descuento A-Priori del Profit Teórico (Slippage Degradation Math): Si la Skill Matemática (Skill 13) detecta un Arbitraje del 0.15%, esta Skill lee la ruta y dice: "Pasa por el CEX Z. CEX Z históricamente desliza 0.10% en contra nuestra. Profit Real Probable: 0.05%". Abortando la operación automáticamente al estar debajo del umbral de rentabilidad limpio.
4. Auto-Aprendizaje Constante (Empirical Auto-Tuning): A medida que el bot hace miles de trades, la base empírica se vuelve a prueba de balas. Si Binance cambia el código de su Motor de Emparejamiento y comienza a deslizar órdenes, la skill lo capta en minutos elevando la "Penalidad Matemática Temporal", protegiendo la cuenta al instante.
5. Limit Order Post-Only Fallback Analyzer: Si la solución para no sufrir slippage es usar "Post-Only" (Fijar precio inamovible), la skill mide la penalidad paralela: ¿Cuántas órdenes Post-Only son Ignoradas/Rebotadas porque el mercado se alejó de ti? Mide el Costo de Oportunidad (Missed Opportunity Cost) vs el Costo de Ejecución Taker.
6. Detección de Asimetría (Asymmetric Slippage Bias): Se da cuenta que el CEX "Y" permite slippages gigantes cuando son en contra del usuario (El CEX gana dinero oscuro embolsillando la diferencia), pero cuando el deslizamiento jugaría a tu favor (Price Improvement), curiosamente la API falla o ajusta al centavo. Capta la "Deshonestidad Cripto" estadística (Exchange Shadow-matching logic).
7. Impacto de Capacidad (Sizing Penalty Adjuster): Entiende y perfila que el Slippage no escala de forma lineal. Un trade de $1,000 en MEXC sufre 0% de slippage, pero un trade de $10,000 sufre 1.5% de slippage exponencial por vacío oculto. La skill pasa una ecuación polinomial limitante al Optimizador de Sizing (Skill 2).
8. Detección de Latencia Interna de Interfaz CEX (Engine Ingestion Lag): Cruza la información del Ping de red (Skill 34) y determina: "Tengo 5ms de Ping a Bybit. La orden llegó rápido. Pero Bybit tardó 35ms internos de su base de datos en procesarla". Aisla el Jitter del servidor remoto y te previene de operar en picos de CPU del exchange.
9. Red Flag / Blacklisting Automático: Si un Par exótico roba slippage sistemáticamente superior al 5% a pesar de usar precios Límite (Falla masiva del exchange), el bot asume "Peligro de Contraparte Grave", saca al par/exchange de la WhiteList general y bloquea futuras ejecuciones.
10. Optimización de Fill-Or-Kill (FOK Constraints): Usar la data forense para establecer precios "Límite" de contingencia inteligentísimos. Si quieres cruzar mercado (Taker) no mandas un "Market Order" ciego, mandas una "Limit IOC" con el precio tope establecido exactamente en el límite de Slippage Tolerable Perfilado.

## 4. Entradas requeridas
- `expected_trade_intent`: Snapshot de lo que el Bot "Creía" que iba a pasar y qué OrderBook Snapshot usó para disparar.
- `actual_trade_execution_receipts`: Evento de Websocket de tu propia cuenta (`executionReport`) devolviendo precios Fill Price reales, Fee Real cobrado, y Fills Parciales.
- `historical_db_reference`: Base de datos de los últimos 10,000 trades ejecutados por el bot.

## 5. Salidas esperadas
- `expected_slippage_bps_penalty`: Modificador flotante que castiga severamente la esperanza matemática cruda.
- `exchange_confidence_score_matrix`: Mapa de confiabilidad para el Ruteo Global (Skill 54).
- `auto_blacklist_event`: Disparador de seguridad si el CEX falla crasamente al cumplir promesas L2.

## 6. Reglas inmutables
- JAMÁS confiar en la salida neta prometida por la Matemática Base sin haberle restado el Deslizamiento Invisible Pre-Modelado y los Fees Dinámicos. La ingenuidad computacional ("El libro decía que compraría a X y vendería a Y exacto") te enviará directamente a pérdida real por "Slippage Fantasma" devorando márgenes netos de 0.05%.
- NUNCA usar Market Orders (Órdenes a Mercado) reales en Criptomonedas Exóticas CEX. Se debe simular un comportamiento Market enviando una Orden Límite `IOC` (Immediate Or Cancel) o `FOK` donde el límite es `MidPrice +/- Slippage_Calculado_Skill59`. Garantiza legalmente que el Exchange no abuse tu orden.
- El perfilado es Dinámico (No Hardcodeado). Un exchange puede ser terrible de Lunes a Viernes y excelente el fin de semana por reducción de tráfico en su Backend. El modelo debe tener una ventana de decadencia temporal (Decay Memory).

## 7. Algoritmos o métodos que debe conocer
- Simple Moving Average & Exponential Weighted Moving Average (EWMA) of Errors.
- Función de Coste de Ejecución (Execution Shortfall / Implementation Shortfall calculation, Marco de Perold 1988).
- Ecuaciones de Impacto de Precio de Almgren-Chriss (Modelos Cuantitativos Avanzados de fricción).

## 8. Fórmulas críticas
- **Slippage Empírico de un Trade**: `Slippage_Bps = ABS(Execution_Price_Real - Intended_Snapshot_Price) / Intended_Snapshot_Price * 10000`
- **Tasa Histórica Descontada (Penalidad A-Priori)**: `Expected_Penalty = EWMA(Slippages_Last_N_Trades_Pair_Exchange)`
- **Umbral Decisivo del Arbitraje Limpio**: `if (Raw_Gross_Profit_Bps < TakerFees + Expected_Penalty + Safe_Margin) { BLOCK_TRADE }`

## 9. Casos extremos
- Asimetría Desleal CEX (Exchange Sniping the User): El bot lanza un Arbitraje Límite Taker a $100 en un CEX pequeño. El Matching Engine del CEX, que es operado por los mismos dueños del exchange y que ven tus cartas, hace "Front Running Interno". Compra a 99.8, te vende a 100 y retiene el gap en secreto. Esta Skill detecta el patrón, anota la "Corrupción de Deslizamiento" cruda en el perfil estadístico del CEX y descalifica sus pseudo-oportunidades matemáticas futuras, blindando al bot.
- API Glitch Delay (The Ghost Execution): Envías orden a Bybit, WebSockets reportan Ejecución 3 Segundos tarde. En esos 3 segundos tu Risk Engine se desesperó y mandó una cobertura innecesaria porque creyó que no compraste. La Skill 59 usa un Timestamp Interno cruzado con el Timestamp CEX de "MatchedTime" y aísla que el Delay es visual del WebSocket y no de Ejecución Real Lógica, previniendo reacciones dobles catastróficas.
- Latencia Geográfica Extrema (AWS to Cloudflare Edge): El perfilador detecta que el "Slippage Invisible" en un exchange no viene de su deshonestidad, sino del Ingress de Cloudflare descartando paquetes UDP/TCP. El perfil penalizará el ruteo hacia allá o reducirá la esperanza estadística forzando a la Máquina a buscar Spreads más abultados antes de cruzar la frontera.

## 10. Validaciones obligatorias
- PRE: Asegurar que el Array de Snapshots Mentales que originaron el trade (`intent_snapshot`) siga vivo en la RAM cuando la confirmación L1/CEX devuelva el éxito (10ms a 1 seg después). De lo contrario no hay memoria histórica contra la cual contrastar el fraude o desfase.
- CÁLCULO: Multiplicadores de Regímenes de Mercado (Integración con HMM Skill 48). El "Slippage Invisible" en un régimen Calmo no es el mismo que en Régimen Volátil (Donde los Matching Engines de AWS se retrasan masivamente bajo carga). El descuento de Penalidad debe ser escalado al régimen.
- POST: Incorporación de datos forenses a Time-Series. Enviar a InfluxDB la métrica `intended_price` vs `executed_price` graficable. Así el Arquitecto Humano puede ver a fin de mes qué Exchange CEX le robó más capital oculto.

## 11. Criterios de aprobación
- Cálculo determinista del Slippage Desviado y actualización del diccionario en Memoria O(1) inmediatamente después de procesar un Callback `onTradeFill()`.
- Capacidad de frenar 100% (Veto) los algoritmos que generaban ganancias teóricas inmensas en Backtesting (Skill 46) pero que sangrarían el Live Account en base empírica estricta (Realidad superando la Teoría Matemática).

## 12. Criterios de rechazo
- La Skill "Ignora" Partial Fills. Si lanzas un Buy de $10,000 pero el exchange ejecuta en 5 bloques distintos ($2k, $3k, $1k...), ignorar el precio ponderado promedio y solo mirar el primer trade generará una falsa ilusión de "No slippage". Debe calcular el `VWAP_Execution_Fill` perfecto de la orden entera.
- Calcular Slippage absoluto sin considerar los Tiempos (Delta T). Si tu bot tarda 500ms en lanzar un trade internamente por ineficiencia de Código Node.js, y culpas al "CEX" del Slippage, el modelo ajusta por penalidad errónea. Debe haber profiling preciso del Latency local (T-Emit) vs Latency Externo (T-Execute).

## 13. Riesgos que mitiga
- La Fuga de Capital Micro-Estructural "Death by a Thousand Cuts" (Muerte por mil cortes): La trampa número uno que quiebra HFTs amateurs. El Backtest jura que la estrategia gana 0.05% por trade. Hacen 1000 trades y pierden el 30% de la cuenta porque el CEX local desvía cada Trade 0.06% invisible y se lo esconde en excusas lógicas de Orderbook Refresh rate. Al integrar este Rastreador Invisible, la matemática cruda choca de frente con el "Costo de Ejecución Real" implacable, negándose a jugar si el Spread empírico neto tras peaje corrupto no es positivo.

## 14. Integración con otras skills
- Interceptor Crítico de Order Executions / Webhooks (Skill 31 y 36).
- Suministra al Optimizador de Tallas de Trades (Skill 2) los topes ciegos funcionales para el "Price Impact".
- Envía inputs modificadores para recalibrar el Backtester y volverlo "100% realista a CEX Corruptos" (Skill 46).

## 15. Modelo de datos sugerido
```json
{
  "ExchangeSlippageProfile": {
    "exchange_id": "mexc_global",
    "pair": "PEPE_USDT",
    "last_updated_ms": 1714521234105,
    "historical_trades_analyzed": 5420,
    "ewma_slippage_penalty_bps": 4.5, // Subtract 0.045% from expected mathematical profit
    "confidence_level": "HIGH_DATA_DENSITY",
    "asymmetry_ratio": 1.4, // Exchange slides price AGAINST us 40% more often than FOR us
    "recommended_action": "APPLY_4.5_BPS_DISCOUNT_TO_ALL_MATH_SPREADS"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Singleton `ExecutionForensicsEngine`. Su principal método `applyEmpiricalDiscount(targetExchange, asset, rawMathematicalProfit)` es llamado Sincrónicamente por la matemática Cruda HFT ANTES de armar el payload.

## 17. Logs obligatorios
- `[DEBUG] Slippage Profiler: Math indicates +0.10% Arb on KuCoin. Profiler applies -0.06% historic invisible latency penalty. Net Expected is +0.04%. Clear to execute.`
- `[INFO] Order #9102 Executed. Target: $65,000. Filled: $65,012. Phantom Slippage: 1.8 bps. Profiler internal EWMA memory updated.`
- `[CRITICAL] FRAUD/LAG ANOMALY SPOTTED ON EXCHANGE [X]. Last 10 trades slipped > 15bps against user despite deep orderbook. Temporarily BLACKLISTING node in Graph Routing.`

## 18. Métricas obligatorias
- `average_phantom_slippage_bps_per_exchange_dashboard`.
- `trades_vetoed_by_forensics_profiler_count`.
- `execution_shortfall_usd` (Costo en dólares real perdido en el éter del deslizamiento, deducido de ganancias netas contables Skill 38).

## 19. Tests unitarios
- VWAP Partial Fills Math: Proveer un "Logro Lógico L2" (Se mandó comprar 10 tokens a $100 prometidos). Proveer un recibo de Websocket de CEX falso con Fills cruzados: `(5 @ $100, 3 @ $102, 2 @ $105)`. El código debe calcular sin fallos O(1) un Precio Ejecutado Promedio VWAP de `$101.60`. Luego restar el Precio Prometido `$100.00` y decretar el `Slippage Bps = 160 bps` castigando al motor severamente en su registro.
- EWMA Memory Tuning: Inyectar 100 trades perfectos `0 Slippage` en la BD. Luego, inyectar un solo trade malo `Slippage 50`. El EWMA Penalty debe subir a `0.5` gradualmente asumiendo el impacto sin reventar histéricamente el bloqueo de trades general (Evitar Oversensitivity / Thrashing).
- Asymmetry Check (Price Improvement): Inyectar casos donde el CEX te ejecutó a "Mejor Precio" del prometido. Esto baja el penalty. La Skill no debe ignorarlo, pero debe asignar un peso distinto ("No celebres tanto la suerte aleatoria CEX, asume el peor caso"), validando cautela conservadora en los pesos algorítmicos.

## 20. Tests de integración
- Levantar Base de Datos Time-Series (Mock). Ejecutar Simulador donde el Orquestador maestro (Skill 36) recibe la orden Límite IOC del Optimizador, se la envía al Profiler, éste lo frena indicando un `Math Profit` destrozado. Validar que la cadena de llamadas Promise (Async/Await) asimile el Parche de Filtro (Reject/Veto) propagando silenciosamente el descarte del Arbitraje para el próximo bloque a esperar algo más limpio.

## 21. Tests E2E
- El bot capta ineficiencia Triangular en la red de Huobi (HTX). Un spread fabuloso de 0.25% usando Shitcoins L1 menores. El Backtest y la Matemática de Grafos afirman ser reyes. Pero el Bot entra en Modo Fuego: Solicita Auditoría Pre-ejecución al `ExecutionForensicsEngine` (Skill 59). Éste rastrea la Memoria Ram de los últimos 200 trades en Huobi usando esa moneda de baja liquidez. Descubre que el CEX simula liquidez y desliza los Takers 0.40% silenciosamente re-cotizando los matching engines en el milisegundo de ejecución. El Filtro le responde al motor principal: "Discount: -0.40%. Net Projection: -0.15%". El Arbitraje Mágico se bloquea al instante y el Agente se niega a enviar el trade suicida. Semanas después, decenas de Bots HFT que no tienen esta skill terminan reventados vaciando sus cuentas atacando ese mismo espejismo de liquidez estático.

## 22. Checklist de producción
- [ ] Incorporación de "Slippage Tolerances" On-chain L1 (Aislado de CEX): En protocolos DEX Uniswap, la skill no adivina el penalty CEX, sino que inyecta en Bytecode del Contrato `require(amountOut >= ExpectedOut * 0.999)` forzando la seguridad determinística incondicional y mapeando los Reverts como "Penalties Lógicos".
- [ ] Separación de API Tier Costings: No confundir "Comisión" (Fees Maker/Taker 0.05%) con "Slippage". La API te manda un Receipt que ya redujo la comisión del Total Balanceado. Extraer e Identificar con pinzas matemáticas cuál porción del "Dinero Faltante" es Fee y cuál es Slippage Fantasma antes de penalizar al motor del exchange falsamente.
- [ ] Integrar Alarma de Latency de PING (Skill 34): Si detectas Slippage Alto hoy, pero el PING del AWS estuvo oscilando 150ms... El Slippage Fantasma fue tu propia culpa de Conexión y no el Motor Corrupto del CEX. La Skill 59 debe perdonar el error de la media al percatarse que el Sistema estaba lento, apuntando el error en el Vector Correcto.

## 23. Ejemplo de configuración no hardcodeada
```yaml
execution_forensics_profiler:
  enable_live_slippage_degradation: true
  ewma_half_life_trades: 50 # Forget very old behaviors, adapt to new exchange engine updates quickly
  max_acceptable_phantom_slippage_bps: 15.0 # If an exchange slips > 0.15% invisibly, blacklist pair
  latency_cross_validation_enabled: true
  auto_adjust_limit_orders_prices: true # Automatically tune IOC Limit Prices to include expected slip
```

## 24. Ejemplo de pseudocódigo
```javascript
class ExecutionForensicsEngine {
    constructor() {
        this.exchangeProfiles = new Map(); // K: 'exchange_pair', V: EWMA Data
    }

    // Called instantly when Websocket TradeFill Event triggers
    reconcileTradeExecution(intentSnapshot, executionReceipt) {
        const expectedPrice = intentSnapshot.expectedVwapPrice;
        const realPrice = executionReceipt.actualVwapPrice;
        const side = intentSnapshot.side;
        
        let slippageBps = 0;
        if (side === 'BUY') {
             slippageBps = ((realPrice - expectedPrice) / expectedPrice) * 10000;
        } else {
             slippageBps = ((expectedPrice - realPrice) / expectedPrice) * 10000;
        }

        // Avoid punishing CEX if WE were slow (Latency overlay)
        if (NetworkMetrics.wasLatencyNormal(intentSnapshot.timestampMs)) {
            this.updateEWMAProfile(intentSnapshot.exchange, intentSnapshot.pair, slippageBps);
        }
    }

    // Synchronous Gatekeeper called by Math Engine before generating payloads
    applyEmpiricalDiscount(exchange, pair, rawMathProfitBps) {
        if (!CONFIG.enable_live_slippage_degradation) return rawMathProfitBps;
        
        const profile = this.getExchangeProfile(exchange, pair);
        const expectedPenalty = profile ? profile.ewmaPenaltyBps : 0; // Default 0 if unknown
        
        const netPredictiveProfit = rawMathProfitBps - expectedPenalty;
        
        if (expectedPenalty > CONFIG.max_acceptable_phantom_slippage) {
             log.warn(`Extreme Phantom Slippage Warning on ${exchange} (${expectedPenalty} bps).`);
        }
        
        return netPredictiveProfit;
    }
}
```

## 25. Criterio final de excelencia
El Rastreador y Compensador de Slippage Invisible representa el "Pesimismo Iluminado" dentro de la codicia del Arbitraje Matemático. Despliega un blindaje probabilístico implacable destruyendo "Arbitrajes Rentables de Pizarra" (Teóricos) que en verdad son abismos friccionales o estafas estructurales del Matching Engine. Logra que el bot enfrente a Wall Street y Cryptoverse operando únicamente verdades empíricas comprobables en dólares y centavos caídos a caja.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Cold Start Penalty (Empezar a operar pares totalmente nuevos L1). El bot carece de Histórico Local Forense y puede comerse Slippage 2 o 3 veces antes de que el motor de Promedio Exponencial despierte la alerta. (Minimizable inyectando 'Penalidad Conservadora Global Base' inicial predeterminada alta en pares ilíquidos).
- Dependencias: OrderBook L2 Snapshots, WSS User Execution Streams.
- Próxima skill: Compounding & Reinvestment Engine (Kelly Criterion) (Skill 60).
