# SKILL 054 — Cross-Exchange Triangular Arbitrage

## 1. Propósito superior
Detectar y capturar ineficiencias matemáticas compuestas que abarcan múltiples plataformas de Exchange de forma simultánea. A diferencia del arbitraje de 2 puntos directo (Ej. Comprar BTC en Binance y vender en Kraken), este Agente mapea una red global (Ej. Comprar ETH con USDT en Binance -> Enviar ETH a OKX -> Vender ETH por USDC en OKX -> Comprar USDT con USDC en Kraken). Amplía exponencialmente el volumen de rutas (Nodos del Grafo) rompiendo barreras de liquidez estancada en un solo exchange.

## 2. Nivel de conocimiento requerido
Matemático en Teoría de Grafos Distribuidos, Arquitecto de Latencia y Ruteo (Routing Systems Architect). Conocimiento profundo de Restricciones Lógicas (Inventario distribuido asimétrico, Tiempos de Retiro de APIs inter-bancarias), Control de Exposición Cambiaria Temporal, e Interoperabilidad Contable Multi-Entidad (Shadow Ledgers globales).

## 3. Capacidades principales
1. Búsqueda de Rutas en Redes Distribuidas: Extender el algoritmo de detección de ciclos negativos de la Skill 53 (Bellman-Ford O(V*E)) para que las "Aristas" (Edges) del grafo no solo sean Pares Bid/Ask, sino también "Transferencias L1 simuladas" (Binance->OKX Network Bridge Cost) o "Virtual Inventory Hops" (Ejecuciones simétricas sin transferencia física de fondos).
2. Synthetic Hedging (Ejecución sin envío): Si Binance tiene el USDT, y OKX tiene el ETH y el puente está saturado, el Agente no "envía el dinero". Realiza las órdenes en paralelo (Consume USDT en Binance, Liquida ETH en OKX) y deja la tarea de igualar las billeteras al Auto-Rebalanceador (Skill 42) en modo perezoso (Lazy execution). Esencial para mantener la latencia en 5 milisegundos en vez de 5 minutos de espera L1.
3. Optimización del Capital Distribuido (Knapsack Problem): El bot encuentra la ruta maestra dorada, pero OKX tiene solo el 10% del capital necesario, Kraken el 50% y Binance el 40%. La skill ajusta los volúmenes para que NINGÚN nodo de la pata se ahogue por falta de "Inventory Ammo" (Munición de inventario - Skill 40).
4. Sincronización Temporal Global: Enviar el Batch de Órdenes a los datacenters de Tokyo (Binance), Londres (Kraken) y Hong Kong (OKX) calculando los Tiempos de Vuelo (Ping - Skill 34). Retrasar intencionalmente el envío a Binance por 10ms si Kraken está 10ms más lejos, para que las órdenes impacten en los motores de emparejamiento (Matching Engines) en el *mismo milisegundo UTC*.
5. Deducción de Fees Inter-Sistema Combinados: Restar la sumatoria total de comisiones y multiplicadores Maker/Taker de tres plataformas con reglas legales y redondeos dispares.
6. Auto-Saneamiento Lógico (Descarte de Redes Cerradas): Si la pata intermedia requiere usar "LUNA_ERC20" en Kucoin, pero Kucoin tiene depósitos LUNA bloqueados (Withdrawal Suspended), la ruta matemática se vuelve inútil físicamente y es tachada del grafo O(1).
7. Cross-Margin Collateral Muxing (Optional): Usar el valor fiat total global para apalancarse y abrir "Patas" de la triangulación en mercados de Futuros Perpetuos como cobertura sintética si el Spot se queda sin liquidez base (Ej. Comprar Spot en Binance, Vender Futuro en Bybit, Comprar Spot en Kraken).
8. Liquidación Dinámica de Remanentes Inter-Exchange (Global Unwind): Si falla la Pata 2 en Kraken, la Pata 1 en Binance (Ya ejecutada) queda huérfana y direccionalmente riesgosa. La Skill invoca a todos los CEXes disponibles buscando el cierre más barato para descargar el inventario en < 1 segundo para anular el riesgo.
9. Oráculo de Correlación Multi-Entidad: Identifica "Líderes de Precio" (Price Leaders vs Price Laggers). Si Binance siempre sube primero, la triangulación debe originarse asumiendo a Binance como el "Oráculo Maestro" y ejecutando las patas en los CEX menores (Laggers) antes de que ellos recalibren.
10. Monitor de Tasas de Interés Cruzadas (Si hay margin activo): Incluir en la fricción matemática si usar el saldo en Bybit le cobrará un Fee de Préstamo por Hora y restarlo del beneficio proyectado.

## 4. Entradas requeridas
- `global_inventory`: Estado absoluto de capital por exchange (Skill 40).
- `multi_exchange_orderbooks`: Streams en vivo sincronizados de todas las bolsas conectadas (Skill 33 y 32).
- `network_transfer_fees_api`: Tarifa base dinámica de retiro entre puentes L1 por exchange (Usado si se rutea físicamente).

## 5. Salidas esperadas
- `cross_triangular_route`: Array detallando cada paso (Exchange, Par, Dirección, Volumen Cuantizado).
- `global_dispatch_signal`: Señal al orquestador principal (Skill 36) dictaminando tiempos de disparo compensados (Ping Compensated Firing).
- `unwind_fallback_contingency`: Un plan "B" pre-calculado que se usará si alguna pata sufre rechazo de red o slippage extremo.

## 6. Reglas inmutables
- NUNCA iniciar un Arbitraje Triangular Cross-Exchange Físico (Que requiere enviar Tokens por una L1 bloqueante como Ethereum entre la pata 1 y 2) si no hay Cobertura Direccional Temporal (Hedge). El riesgo del "Tiempo en tránsito" destruirá la matemática en criptomonedas altamente volátiles. Priorizar abrumadoramente el "Arbitraje Simétrico Sin Retiro" (Usar inventario local existente en ambos lados).
- Los cálculos matemáticos de Multiplicación Triangular DEBEN lidiar con pares invertidos sin error aritmético. (Ej. `USDT/BTC` existe en Kraken, pero `BTC/USDT` es el estándar en Binance). El Normalizador (Skill 32) voltea el divisor a multiplicador en memoria (`1 / Precio_Kraken_Base`) asegurando rutas perfectas.
- Antes de emitir el ataque concurrente, el motor verifica el Estado del Risk Engine Global (Skill 41) de TODAS las bolsas implicadas. Si Binance está bajo ataque DDoS y el Risk Engine dictó AMARILLO, la ruta combinada muere enteramente.

## 7. Algoritmos o métodos que debe conocer
- Problema de la Mochila Fraccional (Fractional Knapsack Problem) aplicado a límites de liquidez.
- Bellman-Ford Algoritmo Modificado O(V*E) con Topología de Grafo Multi-Capa (Multilayer Graph Networks).
- Network RTT Time-Sync Delaying (Ajuste Táctico de Retrasos en Hilos).

## 8. Fórmulas críticas
- **Latencia de Impacto Compensado (Ping Delay)**: `Wait_Time_Node_X = Max_Ping_In_Route - Local_Ping_To_Node_X`
- **Ineficiencia Triangular Global (Hedging Mode)**: `(Binance_Rate1 * (1-Fee_B)) * (OKX_Rate2 * (1-Fee_O)) * (Kraken_Rate3 * (1-Fee_K)) > 1.000`
- **Volumen Cap**: `Optimal_Vol = MIN(Binance_Inventory_A, OKX_Inventory_B, Kraken_Inventory_C, Route_Bottleneck_Orderbook_Depth)`

## 9. Casos extremos
- Interrupción Silenciosa de API (Shadow API Outage): Envías un bloque masivo de 3 patas. Binance (Pata 1) ejecuta perfecto. Kraken (Pata 2) tiene la API en mantenimiento no-declarado, responde "HTTP 503 Backend Timeout" tras 15 segundos. Te quedas direccional. Solución: Todo dispatch CEX usa "Timeout=1000ms" estricto (Skill 35 y 31). Si falla, entra al "Unwind Contingency". Se lanza un Sell a Mercado en Binance revirtiendo la pata 1, asumiendo pérdida de comisiones, pero evadiendo pérdida del colapso del activo en sí (Risk Mitiagation).
- Liquidez Espejismo en un Nodo Débil: El Grafo encuentra un Arbitraje inmenso usando `Kucoin` como puente (`ETH -> SHIB -> USDT`). Kucoin tiene fama de Orderbooks Fantasma (Skill 49 Toxicity). Al ejecutar, Kucoin procesa el trade en 2 segundos, y da un slippage de 2%. Binance y OKX dan 0%. El beneficio total de 1% se destruye y da -1%. Solución: Los Exchanges se ponderan probabilísticamente en el Grafo O(N^3). "Kraken Spread = Real. Kucoin Spread = 80% Real". (Uso de Factores de Confianza basados en Backtesting Skill 46).
- Desincronización Cambiaria Fiat: Usar pares Exóticos Cruzados como (USD Kraken vs EUR Binance). El diferencial "Real" USD/EUR fuera del sistema cripto oscila. La matemática debe anclar dinámicamente un oráculo Fiat O(1) in-memory para no cazar arbitrajes falsos si el par base Forex cambió y desajustó el CEX Europeo vs Americano.

## 10. Validaciones obligatorias
- PRE: Chequear los Inventarios unificados. El Bot detecta la Oportunidad, y confirma que cada Billetera local envuelta tiene "Saldo suficiente no-bloqueado" (Free Balance) para ejecutar su pata sin mover dinero.
- CÁLCULO: Validar la "Precisión Fraccional" de distintas bases (Binance usa 8 decimales, un DEX menor L2 usa 18). Si se envía 1e18 por error a la API de Binance, causará un Reject mortal o un trade no deseado. Re-Normalización antes del Dispatch.
- POST: Al cerrar el ciclo inter-exchange asíncrono asimétrico (Cada CEX te deposita en 30ms diferentes), el Ledger Contable (Skill 38) asienta la "Suma de las Partes" asegurando que el Global NAV subió netamente confirmando el éxito del arbitraje.

## 11. Criterios de aprobación
- La evaluación concurrente de rutas Inter-Exchange (Matriz Multi-Dimesional de N Nodos = Exchanges y V Vértices = Monedas) se reduce lógicamente (Podando - Pruning the tree - shitcoins con bajo volumen) para evaluar y encontrar el arbitraje en < 1 milisegundo por iteración.
- Capacidad de Ejecución Atómica Simulada (Simultaneous Dispatch). Todas las llamadas a red API (`fetch/curl`) o envíos Websocket a las 3 plataformas se despachan en paralelo usando concurrencia I/O nativa (Thread-pooling / Tokio-Rust / libuv).

## 12. Criterios de rechazo
- Ruteo Serial. Ejecutar "Espero a Binance", si pasa, "Ejecuto Kraken". Eso toma ~150 milisegundos en Round-Trips HTTPS. En 150ms el spread de Kraken murió por Market Makers institucionales (Jump/JaneStreet). Rechazo del diseño arquitectónico por incomprensión de HFT.
- "Slippage Leakage" Oculto: Configurar "Órdenes de Mercado" (Market Orders) cruzando Exchanges en vez de Órdenes Limit FOK o IOC. Jamás ceder el precio al Motor del Exchange; exigir siempre Límites Estrictos para proteger el Grafo matemático calculado.

## 13. Riesgos que mitiga
- Aislamiento y Estancamiento Local (Local Minimum Trap): Un bot estancado solo en 2 exchanges ve 10 oportunidades al día. Al usar "Graph Networks" abarcando 10 exchanges, el bot puede triangular "DEX1 -> CEX3 -> CEX1 -> DEX2". Las ineficiencias matemáticas estallan (Aumentan logarítmicamente) mejorando masivamente el Win-Rate diario del Agente sin aumentar el riesgo de capital, capturando retornos en fragmentaciones ilíquidas en la periferia de la red.
- Fallas de Integridad de Precios Global: Las ineficiencias Triangulares a través de entidades independientes suelen ser mucho más duraderas (A veces 100-200ms) que las ineficiencias dentro del mismo Binance (1-5ms), por la incapacidad general de los Market Makers menores de balancear su capital global tan rápidamente.

## 14. Integración con otras skills
- Orquestador del "Ping Compensado" usando las telemetrías de Skill 34 (Latency Map).
- Ejecutor paralelo al Orquestador Atómico Inteligente (Skill 36).
- Obliga al Auto-Rebalanceador (Skill 42) a trabajar el doble tras cada éxito para devolver los balances "al lugar inicial" o estado neutral para el próximo trade.

## 15. Modelo de datos sugerido
```json
{
  "CrossTriangularRoutePlan": {
    "network_id": "GLOBAL_CROSS_ARB_V2",
    "timestamp_ns": 1714521234105,
    "edges": [
      { "exchange": "binance", "pair": "BTCUSDT", "side": "BUY", "volume": 1.5, "rate": 65000, "latency_offset_ms": 15 },
      { "exchange": "okx", "pair": "ETHBTC", "side": "BUY", "volume": 25.0, "rate": 0.05, "latency_offset_ms": 0 },
      { "exchange": "kraken", "pair": "ETHUSDT", "side": "SELL", "volume": 25.0, "rate": 3500, "latency_offset_ms": 8 }
    ],
    "net_profit_bps_expected": 5.4,
    "hedge_mode": "INVENTORY_SYNCED", // Execution without inter-bank transfer
    "safety_fallback": "MARKET_SELL_ALL_IF_LEG_2_DROPS"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Proceso Heavy-Math (Rust/C++ Worker) que ingiere los Multi-Orderbooks cada X milisegundos y busca Grafos de Bellman-Ford usando algoritmos vectorizados de Single Instruction Multiple Data (SIMD AVX-512) para mapear 15,000 pares combinados en 0.2ms.

## 17. Logs obligatorios
- `[DEBUG] Cross-Graph Searcher Active. Scanned 1.4 Million Multi-Layer Combinations in 0.8ms.`
- `[INFO] Golden Route Detected: [USDT(Bin) -> ADA(Bybit) -> BTC(Kucoin) -> USDT(Bin)]. Net Spread: +0.15%. Emitting Ping-Compensated Batch Dispatch.`
- `[CRITICAL] PING ASYNC OFFSET APPLIED. Delaying Binance execution 15ms locally to guarantee simultaneous hit with Kraken datacenter latency.`

## 18. Métricas obligatorias
- `cross_tri_opportunities_found_vs_executed_ratio`.
- `unwind_fallback_events_triggered_monthly` (Alarma si el Grafo estalla en pedazos con demasiada frecuencia por volatilidad irreal).
- `average_multi_datacenter_rtt_ms` (Telemetría pura para calibrar los disparos).

## 19. Tests unitarios
- Matriz de Ponderación CEX (Discount Factor): Crear sub-grafo con un Exchange "Malo" (Latencia variable o Slippage alto histórico). A pesar de arrojar 2% de Profit Teórico Matemático, el Bellman-Ford modificado del bot DEBE aplicar la Penalidad Histórica al peso de la arista, reduciéndolo a 0.05% y optando por la ruta de los "Exchanges Seguros" (Binance/OKX) que daba 0.15% seguro. (Validación de Risk-Adjusted Routing).
- Fractional Knapsack Inventory Constraint: Inyectar 3 Exchanges. CEX 1 tiene $100k USDT. CEX 2 tiene el equivalente a $50k en ETH. CEX 3 tiene el equivalente a $10k en BTC. La oportunidad permite un volumen de $1 Millón de Dólares. El Optimizador DEBE restringir el dispatch del arreglo entero exactamente a la capacidad del Nodo más Débil (`$10,000 USD Equiv`), asegurando nulos rebotes por `Insufficient Funds` de red API.
- Ping Sync Fire: Orden 1 (`Ping: 50ms`), Orden 2 (`Ping: 10ms`), Orden 3 (`Ping: 5ms`). El Orquestador debe iniciar 3 subprocesos. Hilo 1 dispara inmediatamente (`0ms`). Hilo 2 duerme (`40ms`) y dispara. Hilo 3 duerme (`45ms`) y dispara. Resultado: Los 3 payloads tocan el servidor Nginx/Cloudflare de los exchanges en el milisegundo exacto (`T+50ms`). Pruebas de Time Drift exigidas.

## 20. Tests de integración
- Levantar 3 servidores RPC (Mocks Mongoose/Express) emulando Bybit, Binance, OKX. Conectar la Skill 54 al framework de prueba. Engañar los Websockets inyectando la ruta de rentabilidad mágica. Registrar y grabar en milisegundos cuándo llegan los POST a los 3 servidores simulados. Analizar la bitácora: Si las 3 órdenes llegaron dispersas con > 15ms de diferencia debido a Single-Thread Event Loop blocking (Vanilla Node.js limitation), el bot falla estrepitosamente. Refactorizar usando WebWorkers/Rust Multi-threading.

## 21. Tests E2E
- El bot navega un océano de datos L2 (Libro Profundo). El Spread simple de "Arbitraje a 2 puntos" no existe (Los bots institucionales lo destruyeron). La máquina descubre un Triángulo Global: Un ruso está dumpenado DOGE en OKX. Un europeo está comprando EUR en Binance, y Kraken tiene una anomalía de BTC/EUR. La red cruza las asimetrías fiat y de memecoins asíncronamente conectándolas con USDT y BTC. El despacho concurrente golpea Tokyo y Londres simultáneamente. Se absorbe el 0.3% del remanente en < 50 milisegundos sin inyección L1 física. El balance de la Tesorería General Unificada (Skill 40) sube, demostrando el Nivel Omega de la infraestructura.

## 22. Checklist de producción
- [ ] Incorporar el "Taker/Maker Rebate Logic" (Skill 39) en la raíz del algoritmo de Búsqueda de Camino Más Corto (Shortest Path). Ignorarlo es fallar a la mitad de los arbitrajes (Perdiendo Spread) o arruinarse (Calculando Spread falso).
- [ ] Aislamiento Físico y Redundancia Múltiple AWS (Multi-Region Host): Para arbitrajes Triangulares Globales, no hostear el Bot HFT en una sola VPS en US-EAST. Poner nodos relés (Edge Workers) cerca a Tokyo, Europa y USA y orquestar el ataque usando AWS Global Accelerator para aplanar la latencia inter-continental a límites microscópicos por fibra troncal propietaria.
- [ ] Reducir la N-Profundidad a un máximo duro (Hard limit) de `Max Hops = 3 o 4`. Arriba de 4 patas transaccionales a través de nubes en internet, la Probabilidad Combinada del Fracaso se dispara (`P_Exito = P1 * P2 * P3 * P4 = 0.9*0.9*0.9*0.9 = 65%`). Es suicida financieramente asumiendo Risk Tolerance institucional.

## 23. Ejemplo de configuración no hardcodeada
```yaml
cross_exchange_routing_engine:
  max_graph_hops: 3
  ping_compensation_mode: true
  minimum_global_net_profit_bps: 4.5
  confidence_discount_factors:
    binance: 1.0 # 100% Trust in execution stability
    okx: 0.98
    kucoin: 0.70 # Heavy discount due to phantom orders & lag spikes
  emergency_unwind_acceptable_loss_bps: 5.0 # If leg fails, take up to 5bps loss to close risk
```

## 24. Ejemplo de pseudocódigo
```javascript
class CrossExchangeArbitrageNetwork {
    constructor() {
        this.graph = new MultilayerGraph();
    }

    async scanGlobalNetwork(inventoryMatrix, pingMap) {
        // BellmanFord O(V*E) on negative log transformations of prices + fees + confidence penalty
        const bestRoute = this.graph.findHeuristicNegativeCycle(CONFIG.max_hops);
        
        if (bestRoute && bestRoute.netBps > CONFIG.min_profit) {
            const optimalSizing = this.calculateKnapsackInventoryCap(bestRoute, inventoryMatrix);
            
            if (optimalSizing > MIN_TRADE_VALUE) {
                await this.executeSynchronizedDispatch(bestRoute, optimalSizing, pingMap);
            }
        }
    }

    async executeSynchronizedDispatch(route, size, pingMap) {
        const maxPing = Math.max(...route.legs.map(l => pingMap.get(l.exchange)));
        const dispatchPromises = [];

        for (let leg of route.legs) {
            const localPing = pingMap.get(leg.exchange);
            const artificialDelayMs = maxPing - localPing;
            
            // Build the specific instruction for the API Router
            const command = ExchangeApi.buildIOC_Command(leg, size);
            
            // Push to parallel execution pool with intentional delay for time-sync
            dispatchPromises.push(
                 AsyncWorker.fireWithDelay(command, artificialDelayMs)
            );
        }

        const executionResults = await Promise.all(dispatchPromises);
        this.verifyCrossHedgeIntegrity(executionResults, route);
    }
}
```

## 25. Criterio final de excelencia
El Ruteador Cross-Exchange de Grafos eleva al agente de un "Bot simple de 2 patas" a un Depredador Multi-Dimensión de Red Distribuida (Multi-Dimensional Swarm). Al conectar el capital fragmentado del mercado mundial a través de pura sincronización estocástica de latencias y topologías algebraicas complejas, encuentra agua en desiertos donde el resto de los competidores HFT simplemente ignoran el esfuerzo computacional requerido.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: El "Ping Jitter" (Variación Estocástica de Latencia de Red). Tú calibras la latencia calculando `Ping=10ms`. En ese exacto momento, un switch inter-oceánico fluctúa y el ping salta a `80ms`. La sincronización atómica falla y una pata queda expuesta direccionalmente. Solucionable minimizándolo al usar Redes Troncales (Dark Fiber/AWS Global) y un Fuerte Auto-Liquidación de Unwind.
- Dependencias: Latency Mapping (Skill 34), Multi-Exchange Websockets y Unwind Fallback Logic.
- Próxima skill: Statistical Arbitrage (Pairs Trading / Co-integration) (Skill 55).
