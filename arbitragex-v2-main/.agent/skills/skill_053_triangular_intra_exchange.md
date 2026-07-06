# SKILL 053 — Triangular Arbitrage (Intra-Exchange)

## 1. Propósito superior
Detectar y explotar instantáneamente ineficiencias matemáticas *dentro* de un mismo exchange centralizado (Binance, OKX, Bybit). Al convertir un activo A a B, luego B a C, y C a A (Ej. `USDT -> BTC -> ETH -> USDT`), se pueden encontrar ciclos cerrados donde, por fallas de re-calibración temporal de los creadores de mercado, la cantidad de `USDT` final es mayor que la inicial. Este método asume riesgo logístico Cero (no hay transferencias on-chain, ni block time latency) y es la forma más pura de arbitraje matemático estadístico HFT cerrado (Risk-Free Triangular Graph Arbitrage).

## 2. Nivel de conocimiento requerido
Ingeniero de Algoritmos (Graph Theory). Dominio absoluto de Bellman-Ford, Detección de Ciclos Negativos en Grafos Ponderados (Negative Weight Cycle Detection), Optimización Convexa, y Comprensión Fina de Modelos de Comisiones Maker/Taker Intra-exchange (Fee Tiers institucionales en CEXes).

## 3. Capacidades principales
1. Negative Cycle Detection: Mapear los miles de pares de trading de un CEX (Binance tiene ~1500 pares) como un grafo de Nodos (Monedas) y Aristas Dirigidas (Pairs Bid/Ask Price). Encontrar un camino que, multiplicado algebraicamente, resulte en `Valor_Final > 1.0` (o `Log(Suma) < 0`).
2. Sizing Constraints (Cuellos de botella de Liquidez): Entender que en la ruta `USDT -> PEPE -> BTC -> USDT`, el puente `PEPE -> BTC` puede tener liquidez para solo $5. Si la fórmula asume que puede inyectar $1 Millón, causará slippage terminal. Esta skill encuentra el "Máximo Flujo Mínimo" a través de las tres patas (The Bottleneck Leg).
3. Simultaneous 3-Leg Execution (FOK Routing): Enviar 3 órdenes `Immediate Or Cancel (IOC)` en milisegundos idénticos a través de websockets paralelos. O se llenan las tres simultáneamente cruzando el mercado, o el bot cancela las patas incompletas en la ventana de micro-vulnerabilidad.
4. Taker vs Maker Routing Dynamics: Calcular si el Arbitraje Triangular sobrevive cruzando el Spread 3 veces (Pagando Taker Fee 3x). Si requiere colgar órdenes límite (Maker) y esperar (Passive Arbitrage), asume riesgo direccional (Leg Risk) y requiere evaluación de la toxicidad del libro (Skill 49).
5. Explotación de Pairs de Dinero Fiduciario (Fiat Inefficiencies): Encontrar diferencias locas cruzando pares exóticos (Ej. `USDT -> TRY (Lira Turca) -> BTC -> USDT`), donde la iliquidez en Fiat/TRY retrasa el ajuste automático de los market makers locales creando agujeros dorados.
6. Auto-Descarte de Monedas Congeladas (Halted Assets): Filtrar pares que el CEX puso en "Mantenimiento", "Solo Retiros" o "Delisting Notice", los cuales lucen como super-oportunidades matemáticas pero en realidad son pares "Mascota" que no ejecutarán volumen real.
7. Cálculo Logarítmico Rápido O(V*E): Transforma multiplicaciones de precios (`A * B * C > 1`) en sumas de logaritmos negativos (`-log(A) - log(B) - log(C) < 0`) para poder utilizar algoritmos de recorrido de grafos más rápidos sin desborde numérico de CPU.
8. Re-balanceo del Punto de Partida: A veces la oportunidad se origina arrancando desde `ETH`, pero tu base de capital está en `USDT`. Ejecutar 4 patas (Quad-Arbitrage) o hacer Auto-conversión si el margen lo permite.
9. Detección de "Self-Cross" Prohibition: Los CEX bloquean y multan cuentas si envías una orden de Venta y Compra cruzada que termina llenándose con tu propia orden remanente ("Wash trading accident"). La skill aisla los identificadores de orden (`clientOrderId`) para prevenir auto-ejecuciones cruzadas multihilo.
10. Dynamic Parallel Order Tracking: Mapear los estados de cumplimiento (Partially Filled) y balancear las patas restantes si la primera pata llenó solo el 45% (Hedge ratio adjustments on the fly).

## 4. Entradas requeridas
- `exchange_orderbooks`: Instantáneas de los `BestBid/BestAsk` de miles de pares en un CEX (Skill 33).
- `fee_tier`: Nivel de comisiones actual del usuario (`0.02% Taker`, `-0.01% Maker`).
- `account_balances`: Inventario actual local disponible para servir de Munición (Skill 40).

## 5. Salidas esperadas
- `triangular_opportunity_graph`: Cadena de ejecución (ej. `[BUY_BTC_USDT, SELL_BTC_ETH, SELL_ETH_USDT]`).
- `optimal_volume_usd`: Tamaño exacto del trade calibrado al cuello de botella.
- `execution_commands`: Instrucciones disparadas al API Router.

## 6. Reglas inmutables
- Toda oportunidad matemática (Gross Profit) DEBE superar los costos friccionales completos en la simulación estricta: `Gross_Profit - (TakerFee * 3) - (SlippagePenalty * 3) > Minimum_Acceptable_Net_Profit`.
- La Latencia Interna de Evaluación de Grafos no debe sobrepasar los 2 milisegundos. Como ocurre en un solo CEX centralizado (Binance), el arbitraje desaparecerá violentamente ante el motor interno del CEX y competidores co-localizados en AWS Tokyo (Latency arms race).
- Si una de las tres patas usa una moneda que tiene "Trading Fees Cero" promocional (Cero Fee Tier CEX promotions), priorizar intensamente la búsqueda a través de ese nodo en el grafo, al eliminar 1/3 de la resistencia friccional.

## 7. Algoritmos o métodos que debe conocer
- Bellman-Ford Algorithm O(V*E) para encontrar Ciclos Negativos (Oportunidades infalibles de Multi-Swap).
- Floyd-Warshall Algorithm (Opcional si el sub-grafo es pequeño).
- Heurísticas Limitadas (Depth First Search Limitado a profundidad 3 o 4) ya que arbitrajes de 5 patas son estadísticamente imposibles de ejecutar sin falla en una de las patas por la latencia paralela.

## 8. Fórmulas críticas
- **Ineficiencia Triangular (Sin Fees)**: `(Bid_Pair1 * Bid_Pair2) / Ask_Pair3 > 1.000` (Dependiendo de la dirección del par).
- **Ineficiencia Triangular (Con Fees)**: `(Rate1 * (1-Fee)) * (Rate2 * (1-Fee)) * (Rate3 * (1-Fee)) > 1.0000000001`
- **Sizing de Cuello de Botella (Bottleneck)**: `MaxTradeVol = MIN(Vol_Pair1, Vol_Pair2_Converted, Vol_Pair3_Converted)`

## 9. Casos extremos
- Latency Execution Risk (Patas Rotas / Broken Legs): Mandas 3 órdenes simultáneas asíncronas IOC a Binance. La orden 1 se llena, la 2 se llena parcial, la 3 falla porque un rival compró el Liquidity Pool de la Pata 3. Resultado: Quedas sosteniendo una bolsa estúpida de una moneda puente (Ej. Compraste TRON intentando triangular a USDC y de TRON a USDT el libro se vació). Solución: Sistema de "Deshacer" (Unwind / Hedging Fallback). Si la pata 3 falla, la skill lanza inmediatamente un Sell a Mercado de TRON hacia USDT asumiendo la micro-pérdida de la pata, liquidando el inventario peligroso en milisegundos (Damage Control).
- Precisión de "Tick Size" y "Lot Size": Binance exige que no compres `1.000005 BTC`. El "Step Size" obliga a redondear. En arbitrajes triangulares donde la Pata 1 compra `0.05 ETH` y la Pata 2 requiere vender `0.05 ETH`, fallas de redondeo pueden dejarte con `0.000001 ETH` colgados permanentemente ("Dust"), que a largo plazo se come el profit. Todo el cálculo de volumen DEBE estar cuantizado/truncado con las reglas estrictas del exchange (Skill 32).
- Flash Crashes (Book Asynchrony): Un CEX puede actualizar el par de BTC/USDT 5 veces más rápido que su par BTC/TRY. La desincronización de WebSockets genera triangulaciones gigantes Fantasmas. (Ver Skill 34 / Time Syncing para descartar libros "viejos").

## 10. Validaciones obligatorias
- PRE: Chequear el Timestamp de los "Last Update Time" de los 3 pares. Si uno de los pares no ha recibido una actualización de precio en > 2 segundos, tachar el grafo. (Spread Falso provocado por "Libro Congelado" de baja liquidez).
- CÁLCULO: Evaluar el Profit Neto contra los umbrales dinámicos del HMM Regime Engine (Skill 48). Un profit de 0.01% intra-exchange es fantástico en régimen calmado, pero suicida en régimen volátil por el altísimo riesgo de pata rota.
- POST: Vigilar el Inventario de Polvo (Dust Accumulation). Emitir comando periódico de "BNB Dust Sweep" (Conversión a moneda base) a las APIs nativas del exchange si está activado, para reaprovechar el capital residual muerto.

## 11. Criterios de aprobación
- Complejidad Computacional Optimizada. Busca en un sub-grafo limitado de ~100 monedas base líquidas en vez de iterar las 1,500 Shitcoins de Binance O(N^3), resolviendo la ruta en < 0.5 milisegundos por iteración en Node/Rust.
- Cálculo riguroso y pixel-perfect de los Multiplicadores de Tick/Lot (Truncamiento a decimales legales del CEX) antes de enviar las órdenes Límite FOK.

## 12. Criterios de rechazo
- El algoritmo asume equivocadamente que los "Taker Fees" son planos en todos los pares. Algunos pares estables (`USDC/USDT`) tienen fee cero promocional, pero si el algoritmo los trata con `-0.1%`, esconderá un tesoro infinito e ignorará el mejor ruteo.
- Envío Secuencial de Órdenes (`await Order1; await Order2;`). La latencia HTTP acumulada (~60ms) garantizará que el Bot sea arrollado. Se debe usar Multiplexión HTTP (Keep-Alive) o "Batch Endpoints" del Exchange (`POST /api/v3/order/cancelReplace` o Multi-Order en 1 Call).

## 13. Riesgos que mitiga
- Riesgo Geopolítico / Blocktime (On-chain vs Off-chain): No sufre los horrores del "Dark Forest" de Ethereum, Reorganizaciones de bloques, Hacks L1, o MEV Bribes (Skill 50). Operas a la máxima velocidad pura que los cables de fibra óptica hacia el Datacenter de Binance permiten, protegiendo contra "Hackeos de Puentes".
- Capital Efficiency Risk: Al ser todo dentro de una misma cuenta, no necesitas rebalancear fondos con tarifas logísticas (Skill 42) entre exchange A y B para explotar el gap.

## 14. Integración con otras skills
- Funciona como una rama independiente/Estrategia alojada en el Orquestador (Skill 36).
- Recibe estados hiper-puros de OrderBook Tracking (Skill 33) y Fee Tracker (Skill 39).

## 15. Modelo de datos sugerido
```json
{
  "TriangularExecutionPlan": {
    "strategy_id": "TRI_ARB_INTRA",
    "exchange": "binance",
    "detected_at_ms": 1714521234105,
    "path": ["USDT", "BTC", "ETH", "USDT"],
    "pairs": ["BTCUSDT", "ETHBTC", "ETHUSDT"],
    "sides": ["BUY", "BUY", "SELL"],
    "bottleneck_volume_usd": 1500.00,
    "gross_ratio": 1.0025,  // 0.25% gross
    "net_ratio": 1.0010,    // 0.10% net after 3x fees
    "status": "APPROVED_FOR_BATCH_DISPATCH"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en bucle (`TriangularGraphCrawler`) que recalcula el Grafo de Adyacencia O(N^3) restringido a N=50 Activos Mayores cada vez que la Skill 33 notifica un cambio volumétrico denso.

## 17. Logs obligatorios
- `[DEBUG] Triangular Engine: Path [USDT->DOGE->BNB->USDT] yields 1.0001 (0.01%). Aborted. Below Minimum Threshold of 0.05% due to high Taker Fees.`
- `[INFO] Intra-Exchange Arbitrage Fired! Route: [USDT->TRY->ETH->USDT]. Net Profit 0.12%. Sent Batch Order via WebSocket.`
- `[WARN] Broken Leg Alert in Triangular Arb. Order 3 (ETH->USDT) Failed to Fill. Auto-liquidating ETH exposure into Base Asset via Market Order (Hedging).`

## 18. Métricas obligatorias
- `tri_arb_opportunities_spotted_per_min` (Mide el pulso del caotismo interno del exchange).
- `tri_arb_execution_success_rate_pct`.
- `dust_accumulated_usd` (Para detectar fallos sistemáticos en el redondeo Lots/Ticks).

## 19. Tests unitarios
- Bellman-Ford Algoritmo: Inyectar un JSON de matriz de precios simulado donde se sabe que `A->B->C->A` da 1.05. El código debe escupir el Array de ruta correcto `[A, B, C]` en tiempo menor a 1ms.
- Sizing Bottleneck Test: Proveer `[A->B ($10k Vol)], [B->C ($1k Vol)], [C->A ($50k Vol)]`. El Optimizador debe devolver un `MaxTradeVol` igual o inferior a `$1k`, capando la ejecución para evitar el auto-slippage masivo.
- Precision Truncation (Lot Size Math): Alimentar con una orden cruda de comprar "12.3456789" de `SHIB/USDT`. La config del Exchange dice Step Size `1.0`. El motor de redondeo local debe truncar a `12.0` de forma O(1) antes de enviarlo, de lo contrario la API tirará `LOT_SIZE_ERROR` rebotando todo el trade.

## 20. Tests de integración
- Levantar servidor falso de Binance API. Recibir el "Batch Order" del bot. Contestar: Orden 1 Filled, Orden 2 Filled, Orden 3 Rejected (Fallo de Liquidez). El Bot debe recibir la respuesta e invocar automáticamente la Rutina de Salvataje (`Emergency Unwind`), mandando una Market Sell para limpiar la moneda residual (Pata 2), confirmando la resiliencia a colapsos lógicos.

## 21. Tests E2E
- El bot monitorea Binance VIP-0. En un día de locura FOMO (Nuevos listados Launchpad), el par `LUNA/TRY` se dispara irracionalmente en Liras Turcas por bots locales estropeados. El par `LUNA/USDT` se mantiene estable. El Arbitraje Triangular ve la grieta: `USDT -> LUNA -> TRY -> USDT`. El Spread neto cruzando comisiones y slippage de 2 niveles es monstruoso (+0.8%). Dispara un HTTP Multiplex a Binance de las 3 órdenes como FOK/IOC en el mismo socket (usando FIX API o WebSocket API si lo permite). Las tres se ejecutan al 100%. El inventario salta. El Arbitraje se cierra atómicamente y el bot sigue escaneando el próximo desequilibrio mientras los market makers intentan reconectar sus oráculos lentos.

## 22. Checklist de producción
- [ ] Aprovechamiento de Niveles VIP (Tier Optimization): Asegurarse de que el bot inyecte dinámicamente tu `VIP Level` de Binance/OKX a las matemáticas. Si tienes VIP 9, tus Taker fees son bajísimos y verás el triple de oportunidades Triangulares rentables que un trader VIP 0 (0.1%). (La ventaja competitiva de escala).
- [ ] Uso de FIX Protocol vs REST API: Si el CEX lo soporta (Ej. Coinbase Pro, Binance Institutional), usar el protocolo Institucional FIX (Financial Information eXchange). La latencia baja drásticamente comparado a mandar HTTPS REST, y previene la caída de conexión.
- [ ] "Negative Fee" Routing: En algunos exchanges emergentes o campañas especiales, ser "Maker" te PAGA comisiones (Fee Negativo/Rebate). Si el Grafo detecta que hacer una Pata Pasiva (Maker) es ultra rápido, la añade y suma dinero por proveer liquidez mientras triangula (Doble ganancia).

## 23. Ejemplo de configuración no hardcodeada
```yaml
triangular_arbitrage_engine:
  active_exchanges: ["binance", "okx"]
  graph_max_depth: 3 # Hard limit. Above 3 legs, latency failure probability converges to 100%
  base_assets_whitelist: ["USDT", "USDC", "BTC", "ETH", "BNB"] # Reduces O(V*E) CPU burning
  minimum_net_profit_bps: 2.0 # 0.02%
  enable_automatic_unwind_on_broken_leg: true
  allow_maker_passive_legs: false # TAKER-only ensures atomic closure speed
```

## 24. Ejemplo de pseudocódigo
```javascript
class TriangularArbEngine {
    constructor(exchangeGraph) {
        this.graph = exchangeGraph; // In-memory adjacency matrix
        this.baseAssets = CONFIG.base_assets_whitelist;
    }

    // Called periodically or via Event Driven on top 50 highly-volatile coins
    async searchForCycles() {
        for (let base of this.baseAssets) {
            // Limited DFS searching for exactly 3-node cycles returning to base
            const paths = this.graph.getTriangularPathsStartingFrom(base);
            
            for (let path of paths) {
                const routeMath = this.evaluatePathMath(path);
                if (routeMath.netProfitBps > CONFIG.minimum_net_profit_bps) {
                     this.dispatchAtomicRoute(routeMath);
                }
            }
        }
    }

    evaluatePathMath(path) {
        let currentVolUsd = Inventory.getAvailable(path.baseToken);
        let accumulatedMultiplier = 1.0;
        
        for (let leg of path.legs) {
            // Apply Taker Fee
            accumulatedMultiplier *= (leg.rate * (1 - leg.takerFee));
            
            // Bottleneck sizing calculation based on Limit Order Book depth (Slippage guard)
            const availableDepthUsd = OrderBookMemory.getDepthAtPrice(leg.pair, leg.rate);
            if (availableDepthUsd < currentVolUsd) {
                currentVolUsd = availableDepthUsd; // Cap the volume
            }
        }

        const netProfitBps = (accumulatedMultiplier - 1.0) * 10000;
        return { path, optimizedVol: currentVolUsd, netProfitBps };
    }

    async dispatchAtomicRoute(route) {
        // Send using Multiplexed HTTP/WSS to achieve pseudo-atomic behavior in CEX
        const responses = await ExchangeApi.sendBatchOrders(route.legs.map(buildIOCCommand));
        this.verifyExecutionIntegrity(responses, route);
    }
}
```

## 25. Criterio final de excelencia
El Motor de Arbitraje Triangular Local convierte el CEX en un campo de recolección de minería matemática cerrada. Erradica totalmente los riesgos físicos inherentes de los traslados de Blockchain (Blocktimes, MEV, Hacks L1). Su maestría depende totalmente del ingenio para usar "FIX APIs", procesadores locales O(1) veloces y mitigación experta de "Leg-Failures", convirtiendo centavos intra-plataforma en ríos de PnL inyectados sin riesgo de red.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Reorganización de Órdenes CEX Interna (Matching Engine Load). El CEX puede procesar la Orden 1 antes que la Orden 3 por carga del clúster interno (Fuera del control del Bot), rompiendo la atómicidad (Requiere fuerte Unwinding logic/Hedging).
- Dependencias: OrderBook State Management y FIX/WebSocket Batch Executions.
- Próxima skill: Cross-Exchange Triangular Arbitrage (Skill 54).
