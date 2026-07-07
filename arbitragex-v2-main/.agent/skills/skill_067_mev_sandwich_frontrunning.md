# SKILL 067 — MEV Arbitrage (Sandwich Attack / Front-Running Simulation) (Ethical/Defensive)

## 1. Propósito superior
Dominar la Capa Más Oscura y Rentable de la infraestructura On-Chain: El Maximal Extractable Value (MEV). Este módulo permite al Agente "Leer el Futuro" analizando el Mempool Público de Ethereum / L2s buscando transacciones Gigantes de ballenas y bots ineficientes *Antes* de que sean incluidas en la Blockchain. El Agente HFRC ejecuta un "Sandwich Attack Defensivo" o Front-Running Arbitrage (Insertar su transacción O(1) justo antes de la transacción objetivo) para extraer la ineficiencia generada por el impacto de precio de los demás, protegiendo al mismo tiempo al propio bot de ser devorado por los Searchers L1 enemigos de Dark Forest. 

## 2. Nivel de conocimiento requerido
MEV Searcher Elite / Arquitecto Blockchain L1 Core. Entendimiento a nivel EVM-Bytecode de los Mempools Públicos, Flashbots / MEV-Boost Relays, Block Building Dynamics (Proposer/Builder Separation - PBS), Simulación de Transacciones Off-Chain EVM (Revm/Geth Tracing O(1)), Gas Bribing Equations, y Extracción Atómica de Valor (Flashloans L1 HFT).

## 3. Capacidades principales
1. Mempool P2P Sniffing (Detección Avanzada L1): El Bot HFRC se conecta a Nodos Full Geth Locales P2P L1 y extrae todas las "Transacciones Pendientes" L1 en tiempo real (Pending Mempool Stream HFT O(1)).
2. Simulación EVM Speculativa O(1) In-Memory: Toma una transacción de una Ballena (Ej. "Comprar $10M de ETH en Uniswap L1"). Sin gastar gas, Simula el impacto de esa Tx localmente usando el motor `Revm` de Rust O(1). Predice que el precio de ETH en el Pool L1 subirá 2% temporalmente.
3. El Ataque Sandwich (MEV Extraction L1): Sabiendo que el Precio subirá 2% en 1 milisegundo (Mismo bloque). La Skill orquesta un Payload MUX O(1):
   - `TX_1 (Front-Run)`: Bot Compra ETH antes que la Ballena (Lo sube 0.1%).
   - `TX_2 (Target)`: La Ballena Compra (Lo sube 1.9%).
   - `TX_3 (Back-Run)`: Bot Vende ETH a la Ballena a +2.0% L1 Profit asegurado L1.
4. Auto-Bundling y Bribe L1 (Empaquetamiento Privado L1): Para que el Minero/Builder ordene las transacciones mágicamente (`Bot->Ballena->Bot`), la Skill envuelve todo en un "Bundle" Privado de Flashbots y Paga un Soborno Atómico O(1) L1 (Ej. 95% de la ganancia va al minero L1, Bot se queda 5% sin riesgo L1 HFT).
5. Back-Running Arbitrage CEX-DEX (Defensive Alpha L2): En lugar de atacar a la ballena (Causándole Slippage), el Bot espera que la ballena distorsione Uniswap (Moviendo ETH 2% arriba del Precio Binance CEX L2). El Bot inyecta una transacción de "Back-Run" (Inmediatamente después de la ballena) vendiendo ETH a la ballena en Uniswap V3 L1 O(1) y comprándolo simultáneamente en Binance L2, capturando el descalce espacial de latencia Híbrido O(1).
6. Auto-Defensa (The Toxic Trap Detector L1): Sabe que otros Bots hacen MEV L1. Analiza el Mempool para detectar si NOSOTROS estamos siendo Sandwichados. Si nuestra Orden HFT L2 CEX-DEX (Skill 64) va a ser devorada por un Bot MEV Predator L1, la Skill CANCELA/Veta nuestra orden On-chain L1 e instruye usar ruteadores Ocultos L1 RPC Privados estrictos.
7. Uncle Block / Re-Org Sniper L1: Calcula que si un bloque L1 se reorganiza (Re-org O(1)), transacciones previamente minadas regresan al Mempool L1, abriendo ventanas de arbitraje milisegundo que asustan a los retailers L1 HFT.
8. Optimización de Gas Exacto (Wei-Level Optimization L1): En MEV, el Bot que paga 1 Wei de Gas más que tú gana los $10,000 L1. El Bot hace cálculo diferencial O(1) de la ganancia máxima para cederle exactamente lo necesario al Block Builder, superando pujas (Blind Auctions L1) de la competencia.
9. Blind Arbitrage (Arbitraje EVM Ciego O(1)): Escucha Contratos Inteligentes L1 que NO SABE para qué sirven L1 (New Meme Coins). Si detecta transferencias de valor puro, empaqueta Calldata copiando el ataque de un enemigo (Transaction Copying/Replay O(1)) y enviando la firma más rápido L1 (Generalized Front-Running L1).
10. Sincronización JIT Liquidity Maker (LP Attack L1): Si la Ballena usa Uniswap V3, en vez de comprar/vender (Taker), la Skill inyecta Liquidez Just-In-Time Concentrada (Skill 63 L1) atajando todo el Swap de la ballena y cobrándole la Fee pasiva de un golpe O(1), retirándose en el mismo bloque sin riesgo Beta L1.

## 4. Entradas requeridas
- `mempool_p2p_stream`: Nodos Geth/Erigon locales O(1) que envían los Bytes Hash de cada TX no confirmada.
- `state_root_memory_clone`: Clon de la Blockchain EVM en Memoria RAM Local (Para Simular transacciones sin latencia RPC O(1)).
- `flashbots_relay_apis`: Endpoints HTTPS L1 para inyectar "Bundles" ocultos a los mineros.

## 5. Salidas esperadas
- `mev_bundle_payload`: Array EVM Transacciones (`[BotTx1, TargetTx, BotTx2]`) firmado y enviado al Relay.
- `target_veto_signal`: Orden al HFT CEX-DEX (Skill 64) diciendo "Pausa, la ruta está infestada de Bots L1".
- `atomic_bribe_computation`: Integer uint256 O(1) del Gas/Wei transferido al Builder `block.coinbase`.

## 6. Reglas inmutables
- JAMÁS enviar una transacción de Extracción MEV (Front-Run/Sandwich) a la Mempool Pública Abierta L1 O(1). Si lo haces, los bots Generalizados Superiores Copiarán tu transacción, la subirán de Gas, y se robarán tu ganancia mientras tú pierdes Fees de Revert L1. Las inyecciones O(1) MEV DEBEN ir estrictamente encriptadas por Endpoints "Mev-Share / Flashbots / Titan".
- Toda transacción de extracción Back-Run / Sandwich DEBE ejecutarse Atómicamente en un solo Contrato Proxy (Skill 51 L1). Si el Beneficio L1 es menor a la simulación O(1), el Smart Contract tiene la cláusula `require(profit > min)` para hacer `Revert` a toda la cascada L1, asegurando Imposibilidad de Pérdida Financiera (Riesgo Cero L1 Base HFT).
- El simulador de EVM local DEBE operar en Rust (Revm) o C++ O(1). Usar simulaciones RPC Http normales (`eth_call`) para evaluar 5,000 transacciones del Mempool demorará minutos, perdiendo el bloque de 12 segundos L1. (Latency EVM Death L1).

## 7. Algoritmos o métodos que debe conocer
- MEV-Boost / Proposer-Builder Separation (PBS) Architecture L1.
- State-Diff Simulation O(1) EVM Architecture (Geth Tracer Custom).
- Knapsack Problem Optimization (Para el Block Builder local C++ si armamos bloques enteros HFT O(1)).

## 8. Fórmulas críticas
- **Bribe Optimal Bidding (Pujar Minero)**: `Bribe = Expected_Profit * (0.90 + Dynamic_Competitive_Premium)` (Cedes el 90-99% al Minero, ganas el 1% de $1M por volumen de bloque).
- **Price Impact Sandwich (Constant Product L1 V2)**: `Impact = AmountIn / (ReserveIn + AmountIn)` (Calculas cuánto subirá el precio la víctima HFT L1).

## 9. Casos extremos
- Bribe War Trap (Trampa de Subasta Ciega L1): Tú detectas un Arbitraje de $10,000 L1. Mandas un Bundle pagando $9,000 al minero (Profit $1,000). Otro bot manda $9,500. Tú actualizas a $9,900. La subasta infinita Ciega termina con un bot pagando $10,001 (Pérdida Neta L1) solo por robar la ejecución O(1). El módulo DEBE implementar Capping Estocástico y negarse a sobrepujar por encima del Cost-of-Capital CEX L2 de la firma (Risk HFT O(1)).
- Poisoned Honeypots L1 (Trampa del Token Falso): Un Hacker (Bait Creator L1) crea un Token L1 donde el Smart Contract permite Comprar pero bloquea a los Bots de Vender (Blacklist function O(1)). El Hacker emite una "Compra Gigante Falsa" en Mempool. Los Bots MEV se lanzan a Sandwichar (Comprando primero O(1)). Quedan Atrapados en el token sin poder vender la pierna 3 del Sandwich L1. Módulo REQUIERE Simulaciones de Trazabilidad Completa End-To-End (Back-run dry-run O(1)) probando Físicamente el Unwind L1 CEX en memoria RAM antes del Disparo EVM Físico.
- Flashbots Revert L1 Chaos: Envías Bundle Flashbots Privado L1 O(1). Pero el Validador del Bloque actual en Ethereum (Ej. Binance Validator o Nodos Ocultos) NO usa Flashbots. Tu Tx es ignorada L1, y la transacción objetivo se procesa públicamente. Tu Arbitraje MEV muere por "Block Miss". El Orquestador L2 CEX asimila el error limpiamente L1 sin destrozar los Hedges (Skill 61 HFT L2 Delta).

## 10. Validaciones obligatorias
- PRE: Chequeo de Invariantes L1. Si la transacción objetivo (Victima L1) configuró `Slippage_Tolerance = 0.00%`, el Sandwich L1 ES IMPOSIBLE O(1) EVM L1 (Cualquier Front-run hará que la Tx de la victima revierta, destrozando la pata central de tu Sandwich L1). El Simulador O(1) lo detecta y descarta instantáneamente.
- CÁLCULO: Mantener Copia Parcial EVM RAM State. (Shadow Database). Sincronizar O(1) cada bloque las reservas AMM de todos los tokens L1 para computar el impacto matemático al Microsegundo en paralelo C/Rust.
- POST: Si el MEV Arbitraje es "CEX-DEX Backrun" (Skill 64 HFT). Una vez ejecutada la Pata L1, se MUXEA Async L2 FIX la pata CEX a Binance para liquidar el Delta direccional asumido L1.

## 11. Criterios de aprobación
- Simulación EVM Compleja Local en < 2 Milisegundos O(1) (State Diff Test C/Rust).
- Capacidad Real de interceptar el Mempool L1 O(1) P2P Node, Encontrar un Taker L1 Gigante y construir el Bundle de Reacción O(1) Flashbots antes de que el nuevo Bloque EVM se acuñe (Time Window < 12 Segundos L1).

## 12. Criterios de rechazo
- El Bot utiliza Nodos Públicos (Infura/Alchemy API L1) para escanear el Mempool O(1). Los nodos públicos descartan o retrasan transacciones masivamente. En MEV, tener 100ms de Lag L1 significa ser ciego. Es Absolutamente imperativo el uso de Nodos EVM Propios (Dedicated Bare-Metal RPC Geth/Reth L1 HFT) alojados L1.
- Operar MEV asumiendo Retención de Tokens L1 L2 (Naked Exposure). Todo MEV es un Extractor atómico L1. Empezar y Terminar en Stablecoin/ETH es la Ley EVM HFT 0(1). No se guardan Shitcoins L1 O(1) jamás.

## 13. Riesgos que mitiga
- La Barrera del Orden Atómico EVM L1 (The First-Mover Risk O(1)). Un Taker normal CEX HFT manda la transacción cruzando los dedos para que se ejecute al precio que vio. Un Bot MEV O(1) NO manda la transacción al azar: Él DICTA al minero cómo, cuándo y en qué orden milimétrico de bloque EVM debe colocarse su transacción para que el Spread sea Infalible (Guaranteed Execution O(1)). Este poder divino L1 HFT mitiga todo "Phantom Slippage" (Skill 59) del lado Descentralizado, eliminando el riesgo Cripto L1 al 100% EVM.

## 14. Integración con otras skills
- Alimentador de Alpha Estructural para el Híbrido CEX-DEX Routing (Skill 64 O(1) L2).
- Socio O(1) del Validador de Contratos Proxys L1 CEX (Skill 51 L1).
- Informa Asimetrías de Riesgo al Market Maker L1 V3 L2 (Skill 63 O(1)).

## 15. Modelo de datos sugerido
```json
{
  "MevArbitrageEngine": {
    "job_id": "MEV_BACKRUN_UNI_V2_ETH_001",
    "timestamp_ms": 1714521234105,
    "target_tx_hash_l1": "0xWhaleSwapTransactionHash...",
    "detected_in_mempool_latency_ms": 15,
    "evm_simulation_o1": {
      "predicted_price_impact_pct": 2.5, // Target will push price up 2.5%
      "revert_risk_analyzed": "SAFE",
      "simulation_compute_time_ms": 1.2
    },
    "bundle_payload_l1": {
      "type": "CEX_DEX_BACKRUN", // We wait for whale to pump price, we sell in DEX, buy in Binance CEX L2
      "bribe_wei_to_builder": 15000000000000000, // 0.015 ETH Bribe
      "net_profit_projected_usd_l1_l2": 245.50
    },
    "status": "BUNDLE_SUBMITTED_TO_FLASHBOTS_RELAY_AWAITING_BLOCK"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Rust Local O(1) `MevMempoolSniperCore`. Sincroniza P2P Layer 1. Filtra Transacciones (Descarta Transfers/Minting vacíos). Simula Swaps. Envía Comandos Binarios al Core Maestro HFT Node.js/Rust para MUXear la orden si el Profit EVM Replicado excede umbrales Bribe.

## 17. Logs obligatorios
- `[INFO] MEV P2P Sniper L1: Target Whale Swap detected (10M USDC -> WETH). Simulating L1 O(1)... State Diff indicates 2% Impact. Constructing Sandwich Bundle L1.`
- `[DEBUG] Bribe Auction Engine HFT L1: Current estimated competitor bribe 0.01 ETH. Bot outbidding dynamically to 0.012 ETH (85% of projected profit). Bundle transmitted via Flashbots.`
- `[CRITICAL] HONEYPOT MEV TRAP DETECTED L1 O(1)! Simulated Back-Run Reverted during RAM EVM Dry-Run L1 due to malicious Transfer Tax logic injected by Contract Owner. Bundle DESTROYED. Agent saved from Wipeout L1.`

## 18. Métricas obligatorias
- `mev_bundles_landed_vs_missed_ratio` (Win-Rate en subastas ocultas de la EVM L1 HFT).
- `simulation_time_ms_o1` (Si se degrada a >10ms, se reinicia Geth Node RAM L1).
- `bribe_efficiency_ratio` (Auditoría si el bot está regalando demasiada plata a los Validadores O(1)).

## 19. Tests unitarios
- EVM State Diff Simulation O(1): Dar una Base de Datos Falsa RAM con Pool `A/B` Liquidez = 100/100. Simular Target Swap de `10 A`. El Engine en C++ DEBE computar `Constant Product K` e indicar Exactamente que el Precio final será modificado a `90.9 B` devolviendo el Spread proyectado sin invocar RPC HTTPS Lentísimo L1.
- Sandwich Profit Calculator L1: Dados Target Size L1, Slippage Victim Limit L1, y Fees de Pool L1. El Optimizador DEBE derivar el "Tamaño Óptimo Front-Run" de tu Bot L1 EVM (Cálculo del Máximo Global de una ecuación parabólica) O(1). Si sobre-impactas a la victima HFT L1, su Tx Revierte (Fallo Sandwich). El Math Engine de derivadas C++ Cripto EVM valida O(1) in-memory la precisión de la bala.

## 20. Tests de integración
- Levantar `Anvil` (Local Mainnet Fork L1). Habilitar Modo `AutoMine=False` (Simular Mempool HFT L1). Inyectar Transacción Víctima al RPC Falso L1. El Bot MEV DEBE detectar la Tx Pendiente O(1). Construir Payload Flashbots L1 RPC Dummy O(1). Llamar al Mined Block Manualmente en Test L1. Validar que la Transacción Sandwich `(Bot_Buy -> Victima_Buy -> Bot_Sell)` fue empaquetada L1 en el orden EVM inmutable, garantizando el profit L1 y validando EVM Nonce Criptográfico Local.

## 21. Tests E2E
- El agente HFRC detecta que el par Spot CEX L2 está congelado sin Alpha Direccional HFT O(1). Activa el MEV Sniper L1 O(1). Lee un Nodo Ethereum Mainnet Local Bare-Metal P2P. Entra una Transacción L1 de un Retardado (Slippage Infinito 99% en DEX Swap O(1) de Uniswap v2). El Agente corre Revm Simulation O(1) en 1.5ms. Ve que puede extraer $15,000 Dólares L1 impactándolo. Pide un Flash Loan Atómico Aave L1 HFT (Skill 28), Front-Runea al Usuario EVM L1 O(1), el Usuario Ejecuta L1 y le sube el precio O(1), El Agente Back-Runea cerrando el Arbitraje L1 O(1). Todo empaquetado L1. Ofrece $14,000 dólares al Validador L1 Block Builder (Bribe de victoria garantizada O(1)). Se mina el bloque de Ethereum. El Agente HFRC Cripto CEX se queda $1,000 Libres de Riesgo en el Balance (Puro Edge Compute Arbitrage L1 HFT O(1)) sin mover 1 solo dedo direccional L2.

## 22. Checklist de producción
- [ ] Incorporar Protección Anti-Uncle Bandit L1 HFT: A veces el Block Builder L1 Desempaqueta (Unbundles) tu Sandwich O(1), se roba tu Front-Run EVM L1, bota tu Back-Run O(1) al olvido y liquida al Bot Maestro (Robo minero O(1)). El Smart Contract MEV Proxy L1 DEBE contener `require(block.coinbase == FlashbotsRelayer_or_Approved)` O(1) y atar las Transacciones Criptográficamente mediante Contratos EVM (Atomic Execution Require L1) para castigar Mineros maliciosos EVM O(1).
- [ ] Inventory Offloading L1 CEX Async O(1): Usar MEV no para robar a retailers L1, sino para Vender Gigantes HFT propios (Self-Sandwiching L1/L2). Si tienes 50M de Shitcoins L2 y las vendes ciego L2, destrozas el precio L2 CEX O(1). Haces Self-MEV Muxing L1/L2 O(1) protegiendo el Impacto de Mercado Global HFT (Ejecución Institucional Silenciosa Darkpool O(1)).

## 23. Ejemplo de configuración no hardcodeada
```yaml
mev_sandwich_extraction_engine:
  enable_mempool_sniper: true
  local_geth_ipc_path_o1: "/root/.ethereum/geth.ipc" # Local lightning fast P2P socket
  min_target_impact_pct_l1: 1.0 # Ignore small fishes, target whales generating >1% L1 jumps
  bribe_optimization_strategy: "AGGRESSIVE_90_PCT_REBATE" # Give miner 90% of profit to guarantee block inclusion
  simulation_engine_backend_o1: "rust_revm_ffi"
  safety_revert_margin_bps_l1: 10 # Buffer built into the Smart Contract to prevent any L1 execution losses
```

## 24. Ejemplo de pseudocódigo
```javascript
class MevSearcherEngine {
    constructor(revmSimulatorC, flashbotsApi) {
        this.simulator = revmSimulatorC;
        this.relay = flashbotsApi;
    }

    async onPendingTransactionDetectedL1(targetTxMempool) {
        // Fast O(1) Pre-filter (Is it a DEX Swap?)
        if (!isDexRouter(targetTxMempool.to)) return;

        // O(1) RAM EVM Trace & Simulation (No network calls)
        const simResult = this.simulator.traceTransactionEffect(targetTxMempool);
        
        if (simResult.priceImpactBps > CONFIG.min_impact_trigger) {
            await this.constructExtractionBundleL1(targetTxMempool, simResult);
        }
    }

    async constructExtractionBundleL1(targetTx, simResult) {
        // Math Optimization: Calculate max front-run size without reverting Victim L1
        const optimalSizeL1 = DerivativeMath.calcOptimalSandwichSize(simResult.poolState, targetTx.slippageTolerance);
        
        // Build Flashbots Bundle [Bot_Buy, Target_Buy, Bot_Sell]
        const frontRunPayload = SmartContractProxy.buildMeFrontRun(optimalSizeL1);
        const backRunPayload = SmartContractProxy.buildMeBackRun(optimalSizeL1);
        
        // Dynamic Bribing L1 Bidding War
        const projectedGrossProfit = simResult.projectedExtractedValueUsd;
        const bribeWeiL1 = calculateCompetitiveBribe(projectedGrossProfit);
        
        const bundle = [frontRunPayload, targetTx.rawTransactionBytes, backRunPayload];
        
        log.info(`MEV L1 Bundle Created. Bribing $${bribeWeiL1}. Sending to Flashbots/Titan/bloXroute L1 O(1).`);
        await this.relay.sendBundle(bundle, targetBlockNumberL1 + 1);
    }
}
```

## 25. Criterio final de excelencia
El MEV Arbitrage Engine transmuta al Agente HFRC de un mero 'Pasajero' que reacciona a los Orderbooks CEX, al rol de Arquitecto Infiltrado (Dark Forest Predator L1). Al manipular y predecir el ordenamiento mismo del Bloque Criptográfico Ethereum antes de que el mundo L1 lo vea y lo asevere en la historia EVM, el bot adquiere ventaja matemática de Extracción Divina, dominando la latencia Negativa O(1) L1 (Reaccionando y monetizando el Futuro Cripto HFT antes de que suceda temporalmente).

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Subastas Bribe Perdidas Cripto HFT L1. Si los Mineros L1 MEV son corruptos y le revelan tu Bundle EVM Cripto a un competidor (Bundle Unbundling/Stealing O(1)). Limitado obligando el `revert` On-Chain EVM L1 HFT.
- Dependencias: Nodos P2P L1 Locales Bare-Metal (Geth/Reth), Revm/EVM Simulator FFI, MEV Relay APIs L1 O(1).
- Próxima skill: Smart Contract Auditor (On-chain Bytecode Analysis) (Skill 68).
