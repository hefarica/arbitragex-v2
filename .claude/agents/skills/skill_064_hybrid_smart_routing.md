# SKILL 064 — Orquestador de Smart Routing Híbrido (CEX + DEX)

## 1. Propósito superior
Fusionar el Dark Forest (El mundo Decentralizado de la Blockchain DeFi/DEX L1/L2) con los Altares de Centralización de Wall Street (CEX API Orderbooks). Esta skill es un Ruteador Maestro (Smart Order Router - SOR). En lugar de solo triangular CEX a CEX (Binance -> OKX), este motor incluye los AMMs (Uniswap, Curve, GMX) como Nodos nativos del Grafo de Arbitraje. Cuando el HFT necesita comprar $500,000 en ETH, el motor fragmenta la orden y despacha $200k a Binance, $100k a OKX y envuelve Atómicamente una transacción Flash a Uniswap V3 por los últimos $200k, extrayendo la liquidez global sin dislocar ningún orderbook de manera individual, unificando todo el Criptoverso bajo un solo mando algorítmico asíncrono.

## 2. Nivel de conocimiento requerido
Ingeniero en Arquitecturas Híbridas (HFT Multi-Venue Execution). Maestría en Multi-Graph Pathfinding (Algoritmos de Dijkstra Avanzado para Routing CEX/DEX mixto), Interoperabilidad de Estados (APIs REST Async vs Smart Contract Mempool Synchronous), Fragmentación de Órdenes (Split Order Sizing Algorithms L1), Mitigación Atómica de Fracasos Parciales, y Liquidación Contable Cross-Dominio Híbrido L1-L2.

## 3. Capacidades principales
1. Descubrimiento de Superficies Continuo (Hybrid Graph Discovery): El Grafo Matemático (Skill 53/54) añade Uniswap V3, V2, Sushiswap y Curve como Nodos de Routing Dinámicos, traduciendo "Ticks y Liquidity Densities L1" a las mismas métricas que un "Orderbook Asks/Bids CEX", permitiendo a la Máquina ver ambos mundos L1/L2 como un solo gran Orderbook Global Unificado.
2. Fragmentación del Volumen (SOR Optimal Split): Ante una oportunidad gigante, el bot rompe la barrera de capacidad CEX fragmentándola O(1). Si Binance aguanta $5k antes del Slippage Veto (Skill 59), Bybit $2k y Uniswap L1 $10k. El bot MUXEA (Multiplexa) la orden inyectando órdenes asíncronas perfectamente calibradas al techo límite local a todas al mismo tiempo para absorber el Macro-Spread.
3. CEX vs DEX Latency Compensator: Manda la transacción `eth_sendRawTransaction` al Smart Contract de Uniswap V3 (Que tarda 2 a 12 segundos L1 por el bloque). A LA PAR (Mismo Orquestador), duerme intencionalmente la orden en Binance (Que tarda 5ms L2). La Lógica es: "Cuando el Bot ve tu transacción L1 en el Mempool a punto de ser minada (Skill 50 Flashbots), SÓLO ENTONCES dispara el Mkt Order en Binance L2, asumiendo sincronización y Cobertura perfecta Atómica-Sintética Híbrida L1-L2".
4. Reajuste de Fallos DEX a CEX (Fallback L1-L2 Routing): Si la Tx en Uniswap V3 lanza `REVERT` (Gas wars perdidas o Slippage tolerance excedido de Mev-Blocker L1), el Bot aborta rápidamente el "Leg L2" del CEX salvando la cuenta del desequilibrio de Delta o, en su defecto, lanza orden a Mercado (Mkt Swap) local para cubrir el faltante DEX con Liquidez CEX "Salvavidas".
5. Evaluación del Costo Híbrido Asimétrico: La Matemática ahora deduce NO solo Taker Fees L2 (0.05%), sino Gas Fees L1 dinámicos. Si CEX A cobra Fee plano, pero DEX B cobra Fee plano + $25 en ETH por Gas, la Ruta DEX B se corta instantáneamente para volumen micro y se autoriza solo para tamaños Macro Whale HFT (Economies of Scale L1 router).
6. Absorción Atómica L1 en Loop (Loop Routing LP): Aprovecha las ineficiencias de su propio Delta-Neutral LP V3 (Skill 63) inyectando a la ruta un atajo si puede auto-cobrarse comisiones asimétricas HFT.
7. Optimización Cruzada Vía Puentes Rápidos (Fast-Bridges HFT Cross-L2): Incorpora las Redes Layer 2 Ligeras (Arbitrum, Optimism, Polygon, Base, Blast) en el routing global. En lugar de Arbitrar Binance Spot vs Ethereum DEX (Lento). Arbitra OKX Spot L2 vs Arbitrum Uniswap DEX (Rápido, 0.25 seg block time L2), logrando spreads de CEX a DEX casi con la latencia intra-datacenter HFT de la Bolsa Clásica.

## 4. Entradas requeridas
- `dex_liquidity_states_L1_L2`: Estructuras O(1) replicadas localmente de las Curvas AMM V2 y V3 (Skill 21 RPC Sync).
- `cex_l2_orderbooks`: Flujos continuos asíncronos Websocket L2 (Skill 33 y 32).
- `gas_fee_live_estimator`: Oráculo interno de EIP-1559 base_fee dinámico milisegundo a milisegundo L1/L2.

## 5. Salidas esperadas
- `hybrid_split_payload_cmd`: Instrucciones paralelas despachadas CEX API FIX L2 y RPC L1 Builder.
- `unwind_fallback_policy_cmd`: Protocolo de contingencia si el Nodo DEX Falla / Nodo CEX colapsa.
- `global_inventory_shadow_update`: Modificación Ciega (Pendiente) del Ledger Unificado Global (Skill 38) asumiendo ejecución hasta que llegue el Receipt Final Async.

## 6. Reglas inmutables
- Toda transacción hacia un DEX debe ser enrutada estrictamente por Medio de una Red de Bribing L1 Protegida (Skill 50 - MEV Blocker/Flashbots Builder). Enviar una transacción HFT pública a la Mempool L1 de Ethereum abierta garantiza que un MEV Bot cazará la Tx por front-running destruyendo la estrategia y sangrando el Agente de Céntimos Atómicos.
- Un Arbitraje CEX-DEX (Híbrido Puro) JAMÁS asume envíos (Transfers L1 CEX-Withdraw). Opera exclusivamente bajo Lógica de "Saldo Re-Bote" (Bounce Balancing). El CEX Vende y el DEX Compra en el mismo instante (Usando saldos estáticos en ambas billeteras sin mover tokens lentos por red L1, el Rebalanceo de Fondos Logísticos Skill 42 limpia la mesa después despacio).
- Requerir `Slippage_Tolerance` ultra restringido on-chain L1 (Ej. `Min_Amount_Out: 99.95%`). Si la ejecución no es tan buena como la computadora predijo, el Contrato V3 revierte (Revert EVM L1) protegiendo el capital.

## 7. Algoritmos o métodos que debe conocer
- Algoritmo de Dijkstra para Rutas más cortas / Grafos Combinados CEX/DEX Ponderados por Gas y Latency.
- Smart Order Routing Fractional Partitioning (Partición Vectorial de Flujos a N Exchanges, "Water-filling algorithm" - Reparto de líquidos de la Teoría Cuantitativa CEX).
- Shadow RPC Simulation L1 (Llamadas de `eth_call` predictivas C/Rust FFI).

## 8. Fórmulas críticas
- **Costo Ponderado Híbrido**: `CEX_Cost = Size * TakerFee`, `DEX_Cost = (Size * LP_Fee) + (Gas_Limit * Gas_Price_ETH)`
- **Sizing de Frontera Eficiente (Water-filling)**: O(N) que asigna capital primero al CEX/DEX que brinde Mejor Retorno Marginal y frena donde el Slippage empieza a herir el Rate of Return Marginal, empujando el resto de capital al Nodo número 2, 3, N, maximizando la absorción Total Global Muxed (Multiplexada).

## 9. Casos extremos
- Front-Running Paralelo Asimétrico: Un Bot enemigo intercepta el Trade en Uniswap (DEX) empujándolo antes que tú en L1. Tu Smart Contract Aborta con Revert L1. PERO el Bot Central Ya había VENDIDO en Binance L2 asumiendo Sincronía. Te quedas "Naked Short" sin haber comprado en el DEX (Riesgo Beta Direccional L2 mortal). Solución Asimétrica: El "Leg" de CEX L2 de este orquestador SÓLO dispara en el `CallBack` de "Block Mined" L1 / Flashbots Inclusion Proof de la Mempool. La latencia favorece primero al Lento (Blockchain) y remata después con el Rápido (CEX API FIX L2). No al revés.
- RPC Node Sync-Desync L1: Tu Nodo RPC L2 (Alchemy) para la red de BSC tiene un Lag/Desincronización y te manda que el Pool DEX de PancakeSwap dice X, pero el Contrato Real L1 L2 ya cambió Y a Z. Cargas el disparo HFT basado en fantasmas del RPC. El Disparo choca y Revierte. Costando Fees On-Chain (Gas L1) muertos. Solución: Todo el Smart Routing debe contrastar la telemetría "Timestamp" del bloque RPC y anular rutas cuyo `latest_block_age` supere 2 segundos, abortando el Trade Híbrido y castrando el Vector.

## 10. Validaciones obligatorias
- PRE: Cruce de Oráculo de Aversión GWEI. El Bot calcula la Oportunidad = `$15` de Spread Híbrido. Ve que Arbitrum tiene Gas Spike de BaseFee subiendo a `$0.50` y el Swapeo del Smart Contract costará `$2.00`. Quedan `$13.00`. Ejecución aprobada. Si el Gas fuera Ethereum Mainnet `$45.00`, Bloqueo Operacional y Cancelación Estructural de Ruta Híbrida.
- CÁLCULO: Incorporar `Impermanent Loss y Slippage Profiles CEX` en paralelo a las curvas in-memory Replicadas L2 O(1) locales de los Contratos AMM DEX L1. Todo Local, Cero Consultas On-Chain L1 para Predicción de precios (`getAmountsOut` L1 matará la velocidad, se computa Replicado con Ticks en C++/Rust Math in-memory L2 Local).
- POST: Vigilar Liquidaciones Parciales Híbridas. ("Pude vender en Binance, pero Curve DEX se lagueó"). Ejecutar Rutinas de Hedging Ofensivas (Skill 61 Perpetual Short Cover) a los milisegundos de recibir Falla de Retorno Híbrida L2.

## 11. Criterios de aprobación
- Entrega de Routing Vectorizado Híbrido en Tiempo Real de 5 Patas o Más mezclando CEX Fix L2 (Binance/OKX) y DEX L1 (Uniswap/Pancake) simultáneamente, superando Test Unitarios O(1) C/C++ FFI.
- Disparo de transacciones "Pre-Compensadas en Ping y Blockchain Blocktime" para garantizar Cobertura Direccional Cero y Arbitraje Limpio L1-L2 Híbrido (Synchronization Maestro).

## 12. Criterios de rechazo
- Despacho Secuencial Híbrido Directo ciego L1 ("Espero a que firme Metamask RPC, luego espero que se mine en bloque 12 segundos... Si pasa... Despacho la compra en CEX..."). En 12 Segundos, el Arbitraje CEX-L2 Murió 15 Millones de veces por Bots Institucionales (Jump Trading / Alameda Death). Si se hace Secuencial Inocente, el Agente queda Excluido Cripto-Institucionalmente.
- Fallar la conversión L1 de Envoltorios Token (Token Wrappers L1). Arbitrar `ETH` en Binance no es lo mismo que arbitrar `WETH` en Uniswap L1. El Orquestador DEX debe inyectar lógica de Wrapping Nativo on-chain Proxy (`deposit()` / `withdraw()`) invisible para el Módulo de Ejecución CEX HFT, de lo contrario la Matemática de Base L2 estalla por discrepancia de String Pairs.

## 13. Riesgos que mitiga
- La Asfixia de Liquidez Fragmentada del Siglo XXI L2. En 2017 todo estaba en un solo exchange L2. Hoy la liquidez está repartida: 15% Uniswap, 20% Binance, 10% Curve, 5% Bybit. Un bot Taker ciego L2 "Choca de cara" contra Slippage gigante si compra solo en Binance L2 (Pared de liquidez falsa, CEX L2 Vacuum Trap). El Router Inteligente Híbrido "Raspa" (Skims L1-L2) la parte más superficial y barata de cada una de las piscinas Globales CEX/DEX simultáneamente, consiguiendo Ejecuciones Monstruosas Multimillonarias L1-L2 MUX con cero impacto en precio, transformando al Agente en Ballena Invisible (Whale Invisibility Shield).

## 14. Integración con otras skills
- Cliente Supremo del Smart Contract Proxy Bóveda L1 L2 (Skill 51) para la parte DEX.
- Socio Atómico inquebrantable de MEV Blocker (Skill 50) L1 Private Transaction.
- Orquestador Avanzado de Múltiples Ticks de Graph Theory L2 (Skill 54 Cross-Exchange Arbitrage).

## 15. Modelo de datos sugerido
```json
{
  "HybridSmartRouteExecution": {
    "job_id": "HYBRID_MUX_ARB_11200",
    "timestamp_ms": 1714521234105,
    "target_asset": "WETH",
    "total_volume_usd_required": 150000.0,
    "projected_blended_net_profit_bps": 8.5,
    "routing_splits": [
      { "venue": "binance_spot", "size_usd": 65000.0, "type": "REST_IOC_LIMIT", "estimated_slip": 0.05, "cost_fee": "TAKER_FEE" },
      { "venue": "bybit_spot", "size_usd": 25000.0, "type": "FIX_FOK_LIMIT", "estimated_slip": 0.08, "cost_fee": "TAKER_FEE" },
      { "venue": "uniswap_v3_arb", "size_usd": 60000.0, "type": "SMART_PROXY_MEV_CALL", "estimated_slip": 0.12, "cost_fee": "GAS_PLUS_AMM_FEE" }
    ],
    "synchronization_strategy": "AWAIT_DEX_MEMPOOL_PROOF_THEN_FIRE_CEX",
    "status": "DISPATCHING_PARALLEL_LEGS"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Maestra Global L1-L2 `HybridSOR_Multiplexer`. Recibe peticiones crudas del Agente Core (Skill 36): `executeMuxedBuy(Asset_X, Size_100k)`. El Router evalúa el agua en el Grafo (Water-filling allocator) y devuelve la Promesa (Promise L2 Async) del éxito del MUX global a la Red Cripto.

## 17. Logs obligatorios
- `[DEBUG] Hybrid Router: $100k ETH Buy Requested. Water-filling Allocation L1-L2 Engine Computed: Binance 40k, OKX 15k, Curve 35k, UniV3 10k. Total Blended Slippage < 0.15%. Approved MUX.`
- `[INFO] MUX Dispatch: Sending MEV Flashbots L1 Bundle to Arbitrum RPC... Block inclusion Proof Detected (Latency +300ms L1). INSTANTLY Firing Local CEX FIX L2 Limit Orders to Binance & OKX!`
- `[WARN] DEX Leg L1 Reverted in Hybrid MUX due to unexpected On-chain L1 Gas Spike! The 35k Curve leg Failed. Executing Emergency Market-Buy in Bybit CEX to close Delta Neutrality Exposure Gap L1-L2.`

## 18. Métricas obligatorias
- `average_blended_slippage_bps` (Para verificar que el MUX fraccionador realmente redujo el Impacto HFT de no haber comprado todo en un solo CEX oscuro).
- `dex_leg_failure_rate_pct` (Fallos del Gas/RPC L1).
- `hybrid_trades_executed_daily_count`.

## 19. Tests unitarios
- Water-filling Algorithm Optimizator: Entregar 3 CEX y 1 DEX L1 simulados. CEX A desliza al 1% cada 1k USD L2. DEX desliza 0.1% L1, pero cobra $10 Gas planos. Orden Size `$500`. El motor DEBE rechazar el DEX porque los $10 Gas matan la rentabilidad MUX micro (Fixed Cost vs Variable Slip O(1)). A la orden de `$50,000`, el Algoritmo VUELCA el 90% del capital L2 al DEX al amortizar instantáneamente el Costo Fijo y aprovechar la Liquidez profunda L1. Optimización Logarítmica perfecta en C++.
- Latency Sync Mux Dispatch: Entregar la orden paralela MUX. El "Delay Engine L1 L2" local del Test debe comprobar que las llamadas a API CEX "Esperaron" el Flag simulado `Mempool_Tx_Found` L1 antes de Despacharse HTTP. Validar asimetría de Execution-Wait-Timers de Node.js Async HFT.
- Cross-Asset Wrapper Conversion L1-L2: Arbitraje exige comprar `ETH` L2 en CEX, y Vender en DEX Polygon L1. El Motor de Ruteo Híbrido debe construir un Payload de Contrato L1 Proxy O(1) In-Memory que traduzca `ETH` HFT Puro a la variable nativa de red Polygon `WETH`. (Asset Normalization Mux Test - Si Falla = Revert Tx L1 Contract Error letal).

## 20. Tests de integración
- Conectar Ganache / Hardhat L1 Mainnet Fork (Con Uniswap L1 V3 Mock) + Binance API Simulator L2. Iniciar la instrucción MUX Hybrid Trade `$1 Millón CEX-DEX`. Revisar L1 Blockchain Logs y L2 Order Fills. Validar contabilidad MTM Base del Robot unificando resultados atómicos sin fallas O(1) de concurrencia y sin memory leaks.

## 21. Tests E2E
- El agente HFRC nota una divergencia macro a nivel Mundial HFT (Diferencias del 0.8% causadas por caída L1 General Cripto). Tiene Pólvora (Capital Skill 40 / 60) de $1.5 Millones USDC Listos para Cazar HFT. Si compra solo en CEX L2 (Binance), moverá el precio del par él mismo 1.0% de slippage perdiendo dinero. El Agente Maestro emite comando Híbrido a Skill 64 MUX L1 L2. El MUX fragmenta matemáticamente el ataque de $1.5M: Envía peticiones paralelas ocultas de Private Tx (Flashbots L1 MEV Skill 50) a 4 DEXes Base/Arbitrum, e inyecta órdenes O(1) Taker IOC CEX L2 Ocultas a Binance y OKX HFT. La asimetría espacia el Impacto del Mercado HFT a nivel 0.05% plano global. La red L1 es extraída, el Arbitraje macro captura un Retorno Neto de 0.65% limpios de comisiones ($9,750 dólares) en un solo Milisegundo Atómico, desapareciendo el fondo en las sombras logísticas de Auto-Transfer (Skill 42) y Delta Hedging (Skill 61) probando Dominio L1 L2 Unificado Máximo.

## 22. Checklist de producción
- [ ] Orquestación "Flash-Loan" Híbrida L1 L2: A veces el Bot Taker L2 NO tiene saldo L1 nativo en la billetera DEX The Graph / Metamask Proxy. Debe invocar a la Skill 28 (Flash Loan AAVE) In-Memory L1 de Smart Contract Mux. `(Pide AAVE FlashL1 USDC -> MUX Swap Uniswap L1 ETH -> Manda a Binance Deposit O(1) -> Binance Swap CEX -> Paga AAVE Flash)`. (Flash/CEX Bridge HFT Complex, solo apto si la red es L2 rápida como Arbitrum).
- [ ] Fallbacks Multi-Hilos y Event Loops en Rust. Jamás poner un `await getTransactionReceipt()` de la blockchain lenta dentro del Lazo Principal de Eventos Tácticos (Main HFT Event Loop JS/Node). Esto frenaría todo el Bot HFT y los Orderbooks colapsan. Lanzar siempre los Disparos Híbridos en procesos WebWorkers L2 Child/Threads O(1) No Bloqueantes Asíncronos Desacoplados L1.

## 23. Ejemplo de configuración no hardcodeada
```yaml
hybrid_smart_routing_engine:
  enable_muxing_cex_dex: true
  min_optimal_split_amount_usd: 1000.0 # Don't bother splitting orders below $1k (Gas makes it unfeasible)
  preferred_l2_dex_networks: ["arbitrum", "optimism", "polygon", "base"] # Mainnet Ethereum is explicitly disabled for micro-HFT 
  max_concurrent_mux_legs: 5 # Avoid over-fragmentation which increases risk of partial fills
  gas_overhead_multiplier_buffer: 1.25 # Assume L1 gas will be 25% more expensive to be pessimistic
  sync_strategy: "LEAD_L1_LAG_L2" # Sincroniza esperando Blockchain Primero
```

## 24. Ejemplo de pseudocódigo
```javascript
class HybridSmartOrderRouter {
    constructor(cexApi, dexProxy) {
        this.cex = cexApi;
        this.dex = dexProxy;
    }

    async dispatchOptimalMuxSplit(asset, totalVolume, direction) {
        // Run Water-Filling Optimization Algorithm
        const routesMatrix = await L1_L2_WaterFillOptimizer.compute(asset, totalVolume, direction);
        
        // Routes Matrix looks like: [{venue: 'binance', vol: 40k}, {venue: 'univ3_arb', vol: 60k}]
        let executionPromises = [];
        let executionResults = [];

        // 1. Dispatch L1 Blockchain Transaction (Slow, Uncertain) FIRST
        const dexLegs = routesMatrix.filter(r => r.isDecentralized);
        if (dexLegs.length > 0) {
             const dexPayload = SmartContractProxy.buildMultiDexPayload(dexLegs);
             
             // Private MEV Call (Awaits Mempool confirmation or Block Inclusion to proceed)
             const l1ExecutionTask = MevRouter.sendBundleAndWaitForMinedBlock(dexPayload);
             executionPromises.push(l1ExecutionTask);
             
             // Block the fast CEX leg until the L1 Transaction is mathematically sealed on-chain 
             // (Or at least safely nestled in a Flashbots Builder Mempool buffer to prevent naked exposure).
             await this.waitForSafeL1CommitmentState(); 
        }

        // 2. Dispatch L2 CEX Fast Transactions (Instantly atomic HTTP L2)
        const cexLegs = routesMatrix.filter(r => !r.isDecentralized);
        for (let leg of cexLegs) {
             executionPromises.push( this.cex.submitIOC(leg.venue, asset, leg.vol, direction) );
        }

        executionResults = await Promise.all(executionPromises);
        this.reconcileMuxResults(executionResults, totalVolume);
    }
}
```

## 25. Criterio final de excelencia
El Orquestador Híbrido Smart Routing rompe las paredes físicas del ecosistema criptográfico HFT separando Cripto Antiguo (Solo CEX) de Web3 (Solo DEX L1). Crea una "Súper-Bolsa Liquida Fantasma" unificada, fusionando Latencias HTTP Milisegundos con Criptografía EVM EVM-L1 de Segundos, gestionando inventarios L1/L2 atómicamente, orquestando el cañón de la Inteligencia Artificial HFRC como si la liquidez Mundial L2 fuese una sola entidad inquebrantable lista para absorber rentabilidad.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Chain Re-Orgs (Re-organizaciones de Bloques L1 Profundos Cripto). Tu transacción DEX se minó en el Bloque X, tu Bot ejecutó el MUX CEX felizmente O(1). 10 segundos después, Blockchain Polygon tira "Re-Org de 5 bloques" (Reorganización profunda). Tu L1 Tx desapareció y se volvió a la nada (Orphan Block L1). Quedaste con Descalce CEX y Naked Delta L2. (Riesgo Estadísticamente Raro HFT, pero solucionable con Risk Hedger Skill 61 post-alerta L1 Reorg Tracker L2 Async).
- Dependencias: MEV Blocker Skill 50, Smart Contract Proxy Skill 51, CEX API Fix Skill 31 L2.
- Próxima skill: Arbitraje de Opciones (Volatility Surface Arbitrage) (Skill 65).
