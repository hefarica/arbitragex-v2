# SKILL 066 — Orquestador de Liquidación Cross-Chain (Bridges Arb L1)

## 1. Propósito superior
Explotar los Descalces de Liquidez (Liquidity Mismatches) y Arbitrajes Espaciales que ocurren *entre* diferentes Blockchains Layer 1 y Layer 2 (Ej. Ethereum vs Arbitrum vs Polygon vs Solana). Utiliza Puentes Criptográficos (Bridges como Stargate, Across, Hop, Celer) y Mensajería Cross-Chain (LayerZero) para mover activos atómicamente cuando el token "USDC" vale $1.00 en Ethereum pero $1.05 en Solana debido a un pánico local de iliquidez. El Bot actúa como un Proveedor de Liquidez Cross-Chain HFT, ganando spreads masivos y comisiones de puentes asimétricos sin exponerse al precio del activo subyacente.

## 2. Nivel de conocimiento requerido
Ingeniero en Arquitecturas Cross-Chain L1/L2. Entendimiento Profundo de Protocolos de Mensajería Inter-cadena (LayerZero, Wormhole, Celer IM), Dinámicas de Lock-and-Mint vs Burn-and-Mint vs Liquidity Networks (Bridges Nativos vs Stableswap Bridges). Modelado de Riesgos Finality (Re-orgs), Validadores Multi-Sig de Puentes, y Optimizador de Tarifas de Gas Multi-Red.

## 3. Capacidades principales
1. Escáner de Arbitraje de Puentes (Bridge Rate Arbitrage): Vigila constantemente los AMMs de puentes (Ej. Stargate Finance pools). Si todos quieren huir de Avalanche y mover sus USDC a Arbitrum, el pool de Avalanche se llena y el de Arbitrum se seca. El Puente "Paga" (Premium) a los bots que muevan USDC en la dirección opuesta (Arbitrum -> Avalanche). La skill detecta y captura ese Yield libre de riesgo direccional instantáneamente.
2. Predicción de Latencia Cross-Chain (Time-in-Flight Oracle): Mover dinero de Arbitrum a Optimism tarda 2 minutos. Mover dinero de Ethereum a Arbitrum tarda 15 minutos (Block Finality). El Orquestador computa este `Delay` como "Riesgo de Exposición Delta L1" e inyecta la Cobertura HFT (Skill 61) acorde.
3. Ruteador de Puentes Óptimo (Cross-Chain Aggregator O(1)): No se casa con un solo Bridge. Evalúa Across, Stargate, Synapse y Thorchain en milisegundos O(1) in-memory L2. Determina la ruta que ofrece el Mejor Tasa de Cambio Final neta tras Gas L1, Liquidity LP Fee y Slippage del Bridge.
4. Hedging Direccional Sintético "In-Transit": Si el Bot envía 10 ETH desde Polygon a Base (Tarda 10 minutos), el Bot está "Long 10 ETH ciegos" durante 10 minutos críticos. La Skill obliga a la Skill 61 L2 a abrir un "Short 10 ETH Perpetuo" durante exactamente 10 minutos, protegiendo el capital de desplomes mientras vuela por el éter L1.
5. Explotación de Descalce de Stablecoins (Peg Arbitrage L1-L2): `USDC.e` (Bridged) vs `USDC` (Native). En L2 a veces pierden la paridad (El bridged cae a $0.98). El Bot compra el token barato L2, usa el Puente Oficial (Native Bridge), espera los 7 días de Withdrawal y recibe $1.00 USD Real en Mainnet. Gana 2% seguro HFT Asíncrono puro (Long-duration Arbitrage O(1)).
6. Bypass de Gas con Gas-Sponsoring (Relayers): Usa APIs como Biconomy o Gelato para pagar el Gas de la Red Destino usando el Token enviado. (Si envías USDC a Solana y no tienes SOL para pagar el Gas local, el Relayer lo paga y te cobra en USDC, evitando que la Tx quede Atascada / Stranded Asset L1).
7. Identificación de "Honeypots" de Puentes (Bridge Exploit Defense): Los puentes son hackeados cada mes. La Skill audita el Oráculo. Si el Puente de repente empieza a acuñar Billones de tokens (Inflación Infinita / Mint Hack), el Bot activa VETO CROSS-CHAIN y niega cualquier envío de capital por esa ruta.
8. Re-balanceo de Tesorería General (Logística L1): Cuando la Tesorería General (Skill 40) detecta que el Bot en OKX se quedó sin plata, pero la Wallet en Arbitrum está reventando de USDC, invoca esta Skill no para ganar Arbitraje, sino como Proveedor Logístico de Rebalanceo al Menor Costo Posible L1.
9. Oráculo de Re-Organizaciones L1 (Finality Monitor): Vigila los Bloques Confirmados (Confirmations O(1)). Nunca asume que un Envío es Exitoso solo porque apareció 1 bloque L1 (Peligro de Orphan Block). Exige N Confirmaciones L1 (Ej. 64 en Ethereum, 2 en Arbitrum) antes de ejecutar el "Des-Hedging" o liberar el Arbitraje CEX.
10. Sincronización Multi-Signature L1: Permite orquestar transferencias L1 que requieren aprobaciones Múltiples de la Tesorería Fría (Cold Storage) interactuando con Contratos Gnosis Safe a través de APIs de Simulación L1.

## 4. Entradas requeridas
- `cross_chain_liquidity_states`: Oráculos locales L2 O(1) de los estados de liquidez de los puentes.
- `l1_gas_oracles`: Costo BaseFee + PriorityFee de Múltiples Redes L1 y L2 simultáneamente.
- `arbitrage_intents_cross_l1`: Petición Cruda del Cerebro: "Necesito mover/arbitrar 10k USDC de Red A a Red B, busca la ruta".

## 5. Salidas esperadas
- `bridge_transaction_payloads`: Array de bytes (Calldata EVM O(1)) listos para ser firmados inyectando el ruteo a Stargate/Across L1.
- `in_flight_hedge_commands`: Señales Tácticas al Módulo de Futuros (Skill 61) "Cúbreme mientras esto viaja L1".
- `bridge_status_tracker_async`: Monitoreo Event-Driven L1 que emite alertas cuando el dinero llega a destino exitosamente.

## 6. Reglas inmutables
- Nunca enviar el 100% del Inventario Base de Tesorería L2 a través de UN SOLO puente en UNA SOLA Transacción. Si el Puente (Smart Contract L1) es Hackeado o el RPC se cae (Stuck Transaction L1), el Bot entero muere y va a $0.00. Siempre fraccionar y Muxear Cross-Chain Envíos > $50,000 en múltiples puentes (Ej. 50% Stargate, 50% Across L1 O(1)). (Counter-party Protocol Risk limit).
- Prohibición Absoluta de Puenteo Ciego Direccional en Tokens Volátiles (Non-Stables). Enviar BTC/ETH a través de Puentes que tardan > 1 Minuto SIN activar Cobertura en Futuros Perpetuos CEX equivale a Ruleta Rusa Financiera HFT.
- Restricción de Slippage Tolerance HFT. Todo puente L1 usado debe soportar el parámetro estricto `minAmountOut`. Si el puente no lo tiene, es rechazado algorítmicamente por la Política de Seguridad HFT para evitar ataques Sandwich de Re-Ruteadores MEV L1 (Skill 50).

## 7. Algoritmos o métodos que debe conocer
- Inter-blockchain Communication Protocols (IBC L1/L2).
- Stableswap Invariant Math (Curva de Curve Finance adaptada a Puentes L1).
- Time-Weighted Gas Optimization (Esperar 10 minutos si el Oráculo Predictor dicta que la red estará un 50% más barata y el Arbitraje lo soporta).

## 8. Fórmulas críticas
- **Costo de Ruta de Puente**: `Bridge_Cost = Source_Gas + Dest_Gas + LP_Bridge_Fee + Protocol_Fee + Slippage_Bridge_L1`
- **Spread Neto de Arbitraje Espacial L1-L1**: `Profit_Bps = (Dest_Price - Source_Price) / Source_Price * 10000 - Total_Bridge_Cost_Bps - Hedge_Cost_Bps`
- **Probabilidad de Atasco (Stuck Rate L1)**: Derivado de la saturación actual del Mempool L1 vs Liquidez de LP de Salida del Puente.

## 9. Casos extremos
- Secado de Liquidez en Destino (The Stranded Asset Trap L1): Envías 100k USDC de Arbitrum a Optimism usando Stargate. La red confirma. PERO el Smart Contract de Stargate en Optimism tiene 0 USDC Físicos disponibles para pagarte. Tu dinero se convierte en "SgUSDC" (Un IOU/Pagaré) atascado L1. Pierdes Liquidez L2 CEX por horas/días, asesinando el AUM Operativo HFT. Solución O(1): La Skill DEBE hacer Fetch al Remote RPC L1 de Destino (Llamada `balanceOf()`) ANTES de iniciar el envío L1, verificando O(1) Localmente si hay suficiente liquidez para recibir el dinero.
- Re-Orgs Mortales en Redes Rápidas (Polygon 157-Block ReOrg): El Bot envía dinero a Polygon L1. Recibe 1 Confirmación L1. Quita la Cobertura Short L2 HFT. Polygon sufre Reorganización Profunda de cadena (Re-Org). La Tx Desaparece. El Dinero vuelve al Origen. Pero el Bot CEX L2 ya quitó la cobertura y vendió HFT. Descalce Contable Atroz L1-L2. Solución: Estricto Oráculo de Finalidad L1. Mínimo 256 Bloques para Polygon antes de dar por Ejecutada la Liquidación L1 (Wait-for-Finality Flag O(1)).
- Cierre de Seguridad (Protocol Pause L1): Se descubre un Hack de 100 Millones en un Puente (Ej. Multichain/Nomad). Los desarrolladores llaman a `pause()`. Si el Orquestador envió la TX L1 1 milisegundo después del `pause`, se Revierte quemando Gas L1. Si la envió 1 milisegundo antes, el dinero se congela L1 indefinidamente. Reacción rápida del Risk Engine HFRC para anular en Mempool (Skill 50 Cancel Tx) si detecta alertas de seguridad OSINT (Skill 44 / 69).

## 10. Validaciones obligatorias
- PRE: Chequear Saldo GWEI/ETH de Gas Nativo L1. "Tengo 50,000 USDC en Polygon, pero tengo 0.00 MATIC". Intentar firmar Payload L1 causará error `INSUFFICIENT_FUNDS_FOR_GAS`. El orquestador DEBE pre-validar las Gas Wallets L1 antes de generar el Calldata EVM O(1), e invocar Auto-Refill (Skill 42) o Gas-Relayers HFT.
- CÁLCULO: Incorporar `In-Transit Value` (Valor en Tránsito L1) al Global Ledger AUM (Skill 38 O(1)). Ese dinero "Flotante en la Nube" L1 sigue siendo parte del Riesgo y AUM HFT, debe contabilizarse para los límites de Kelly Criterion L2 (Skill 60 O(1)).
- POST: Confirmación Atómica (Receipt Verification L1 L2). Leer el `TransactionReceipt` L1 EVM y Decodificar los Eventos Internos L1 (Event Logs `Transfer` o `Swap`) para estar 100% seguros matemáticamente del Balance final recibido en WEl L1, evitando asumir el monto enviado y comerse Mismatches contables L2.

## 11. Criterios de aprobación
- Routing Multicadena Completado. Capaz de Generar Payload O(1) EVM C++/Rust en < 5ms para cualquier par origen/destino usando el Puente Óptimo tras barrido de Liquidez L1.
- Inmunidad a Fondos Atascados L1. Acreditación 100% de que la validación On-Chain Remote RPC de "Liquidez Destino" O(1) bloquea envíos a puentes secos L1.

## 12. Criterios de rechazo
- Usar los "Bridges Oficiales L1-L2" (Ej. Arbitrum Official Bridge) para Arbitrajes de Salida L1 (Withdrawals L1). Estos puentes oficiales tienen períodos de "Challenge" (Fraud Proofs L2) que tardan 7 DÍAS REALES L1 en liberar tus tokens. Retener Liquidez HFT L2 por 7 Días equivale a una quiebra técnica por costo de Oportunidad (Opportunity Cost Wipeout HFT). SÓLO USAR Puentes de Terceros Instantáneos L1 O(1) (Liquidity Networks) para salidas L2.
- El Código se queda "Esperando Sincrónicamente" L1 (`await wait_tx_mined()`). En HFT, esperar 15 segundos un bloque L1 congelará todo el Engine L2, matando el Arbitraje CEX. El Monitoreo L1 debe ser un Subproceso Independiente (Thread Asíncrono / Callback Event L1 O(1)).

## 13. Riesgos que mitiga
- La Balcanización de la Liquidez Cripto (Fragmented Liquidity Desert L1/L2). El Dinero ya no está todo en Binance CEX o Ethereum L1 Mainnet. Está repartido en 50 blockchains de Capa 2 y Alt-L1. Un Arbitrajista encerrado en Ethereum CEX morirá por falta de Spread. Este Orquestador de Liquidación Cross-Chain actúa como la Nave Espacial de Hiper-salto L1 HFT. Traslada el Capital de Trabajo HFT (AUM L2) exactamente adónde el Alpha y el Yield L2 es más alto, logrando que el Agente Opere a Nivel Ecosistema Total Cripto O(1).

## 14. Integración con otras skills
- Socio Logístico del Motor de Routing Híbrido CEX-DEX (Skill 64 L1 L2 MUX).
- Cliente Asíncrono HFT del Delta Hedger Perpetuos (Skill 61 L2 O(1)).
- Consume datos en vivo del Gas Fee Optimizer Local L1 O(1) y MEV Builder Network (Skill 50).

## 15. Modelo de datos sugerido
```json
{
  "CrossChainExecution": {
    "job_id": "X_CHAIN_ARB_ARB_TO_OPT_110",
    "timestamp_ms": 1714521234105,
    "source_network": "arbitrum",
    "dest_network": "optimism",
    "asset": "USDC",
    "amount_in_usd": 75000.0,
    "optimal_bridge_selected": "stargate_v2_l1",
    "estimated_total_fees_usd": 12.50, // Bridge LP + Source Gas + Relayer Dest Gas L1
    "estimated_time_in_flight_seconds": 120, // 2 Minutes Cross-Chain Delay L1
    "hedge_required_l2": false, // Stablecoin logic O(1) bypasses Perpetual hedging
    "remote_liquidity_verified": true, // Destination Pool holds $1.5M USDC (Safe O(1))
    "status": "IN_FLIGHT_AWAITING_DESTINATION_CONFIRMATION_L1"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Subproceso Asíncrono L1 (`CrossChainBridgeOrchestrator`). Posee clientes RPC locales y remotos para cada cadena soportada (Multi-Provider EVM Sync O(1)). Expone API interna `routeAtomicCrossChain(token, amount, src, dst)` a la Skill Maestro 36 HFT.

## 17. Logs obligatorios
- `[INFO] Cross-Chain Scanner O(1): Liquidity Imbalance detected on Across Bridge. Sending 50k USDC from Base -> Ethereum Mainnet nets +0.25% instant Premium. Pre-Flight Checks Passed. Executing EVM Call...`
- `[DEBUG] In-Flight Asset Shield Activated HFT. Sent 5 ETH (Polygon->Arbitrum). Awaiting Finality. Delta Hedger (Skill 61) opened 5 ETH Short on Bybit. Delta Locked O(1).`
- `[CRITICAL] DESTINATION LIQUIDITY DRY ALERT L1! Requested route (Optimism -> Avalanche via Stargate) has only $1k capacity L1. Attempted Transfer of $15k aborted pre-flight to avoid Stranded Asset Trap L1.`

## 18. Métricas obligatorias
- `average_time_in_flight_cross_chain_sec` (Para evaluar calidad de red L1 vs HFT L2 CEX speed).
- `cross_chain_bridge_fees_paid_usd`.
- `stranded_assets_rescued_count` (Auditoría L1 del Oráculo Predictivo de Sequía L1 O(1)).

## 19. Tests unitarios
- Bridge Fee Optimizer: Pasar `amount=$10,000`. Mock `Stargate Fee=$10`, `Across Fee=$12`, `Celer Fee=$8`. El Enrutador O(1) L2 DEBE devolver "Ruta Celer" y empaquetar Calldata L1 Celer O(1) HFT In-Memory. Probar escalabilidad con `$500,000`, si Stargate tiene más Liquidez L1 que Celer y resbala menos, el Ruteador L1 Vuelca la Elección a Stargate demostrando Lógica Asimétrica Volumen/Slippage L1 O(1).
- Time-in-Flight Hedge Sync: Forzar envío de 1 BTC volatil. Emitir `Transaction_Sent`. El motor L2 Local Async DEBE enviar "Signal Hedge 1 BTC" a Skill 61 HFT. Forzar llegada: Emitir `Destination_RPC_Confirmed`. El Motor DEBE enviar "Unwind Hedge 1 BTC" a Skill 61 L2. Confirmar Atadura L1-L2 Inquebrantable HFT (Safety Net Test O(1)).
- Native Gas Pre-Flight Check L1: Vaciar (Mock) la cuenta de MATIC (0.00 MATIC). Ordenar mover USDC por Polygon. El Motor DEBE arrojar C++ Excepción `INSUFFICIENT_NATIVE_GAS_L1` SIN llamar al RPC On-Chain (Ahorrando Latencia de Falla Http L1 y evitando Revert Penalties HFT O(1)).

## 20. Tests de integración
- Forcar Arbitrum y Optimism simultáneamente L1 (Anvil x2). Inyectar RPC Calls O(1) cruzadas. Simular Liquidación de un Agregador Stargate Local Dummy Smart Contract L1. Observar si el Orquestador Genera Firmas Criptográficas Válidas O(1) EIP-712 / Secp256k1 en Memoria L2, y las transfiere limpiamente sin romper los Nonce Locales L1 del Orquestador Global (Skill 42 L1).

## 21. Tests E2E
- El agente HFRC detecta una noticia de pánico extremo sobre una cadena particular (FUD sobre Solana L1). Todo el mercado retail trata de sacar su Liquidez de USDC desde Solana usando Wormhole/Stargate L1. El Pool de USDC de salida en Ethereum se seca completamente. Los minoristas atrapados pagan "Premiums" (Sobrecargos CEX/DEX) del +2.0% solo por cruzar el puente. El Bot (Riesgo Controlado O(1)) tiene capital ocioso en Ethereum CEX (Binance). Usa la Skill 66 para rutear $500,000 USDC desde Ethereum (Contra-corriente) HACIA Solana L1. Gana instantáneamente +2.0% ($10,000 limpios L1) por actuar de Proveedor de Liquidez de Rescate HFT. Luego, tranquilamente, el Auto-rebalanceador Logístico L1 (Skill 42 L2) devuelve el capital centralizándolo en CEX O(1) usando ruteos lentos gratuitos cuando el polvo baja HMM Regime L2, extrayendo Alpha del Pánico Micro-Estructural Cross-Chain L1 HFT puro.

## 22. Checklist de producción
- [ ] Orquestación de Fallbacks Multisig L1 (Riesgo de Protocolo): Hay puentes que exigen aprobaciones O(1) complejas. Las Firmas (Approval) L1 previas a un puente deben limpiarse. ("Set Approval a 0 tras envío L1") porque si el Smart Contract Bridge L1 es hackeado en 2 años, y el bot dejó `Approval=Infinity` On-chain, el hacker vaciará la tesorería L1 completa retroactivamente. Práctica de Seguridad Paranoide HFRC.
- [ ] Caza MEV Cross-Chain (Cross-Domain MEV HFT): Integrar Lógica Predictiva O(1) L1. Si sabes que una ballena HFT L1 movió 10M de ETH de Arbitrum a Polygon L1 (Transacción visible en Bridge RPC), SABES que en 10 minutos (Cuando llegue L1) la Ballena Taker va a Dumpear en DEX L1 de Polygon L1. El Bot "Salta" L2 HFT a Polygon, Shortea L2 (Perps) o posiciona Muros de Arbitraje L1 antes de que llegue la Ola L1 Inyectada. (Predicción Asimétrica Macro HFT O(1)).

## 23. Ejemplo de configuración no hardcodeada
```yaml
cross_chain_liquidation_orchestrator:
  enable_bridge_arbitrage: true
  approved_bridge_protocols_l1: ["stargate_v2", "across_protocol", "cctp_circle"]
  max_acceptable_time_in_flight_seconds: 900 # Max 15 minutes transit. Beyond that, direction hedging becomes too expensive
  enforce_strict_destination_liquidity_precheck_l1: true
  auto_hedge_volatile_assets_in_flight_l2: true
  min_spread_bps_to_trigger_arbitrage_l1: 15.0 # Need at least 0.15% to justify the L1 risks
  revoke_erc20_approvals_post_execution_l1: true # Infinite-Approval Hack Defense 
```

## 24. Ejemplo de pseudocódigo
```javascript
class CrossChainArbitrageOrchestrator {
    constructor(bridgeRouterCore, hedgeEngineSkill61) {
        this.router = bridgeRouterCore;
        this.hedger = hedgeEngineSkill61;
    }

    async evaluateAndDispatchCrossChainArbL1(asset, amountUsd, srcChain, destChain) {
        // 1. O(1) Pre-Flight Liquidity Check via Remote RPC
        const isDestLiquidL1 = await this.router.checkDestinationLiquidityL1(destChain, asset, amountUsd);
        if (!isDestLiquidL1) {
            log.warn(`CrossChain VETO O(1). Destination Network ${destChain} Bridge Pool is DRY. Route aborted.`);
            return false;
        }

        // 2. Select Optimal Path
        const bestRoute = this.router.findCheapestPathO1(asset, amountUsd, srcChain, destChain);
        
        // 3. Optional: Hedge Flight Time (Protect Capital L1-L2)
        let isHedged = false;
        if (!isStablecoin(asset) && CONFIG.auto_hedge_volatile_l2) {
             await this.hedger.emergencyDeltaNeutralize(asset, amountUsd, 'LONG_IN_TRANSIT_L1');
             isHedged = true;
        }

        // 4. EVM Calldata Generation & Dispatch
        const evmPayload = bestRoute.generateEVMCallData();
        const txHash = await MevRouter.sendFlashbotsBundleL1(evmPayload); // Skill 50 L1 Privacy
        
        // 5. Fire Asynchronous Watchdog for L1 Finality Receipt
        this.monitorFinalityAndUnwindL1L2(txHash, destChain, asset, amountUsd, isHedged);
        return true;
    }
}
```

## 25. Criterio final de excelencia
El Orquestador Cross-Chain convierte al Agente HFRC en un Ente O(1) Teletransportador y Agregador Global. Destroza los Silos de Liquidez HFT individuales L2 L1, dándole la habilidad táctica de exprimir capital y rendimiento de los flujos de pánico Inter-Cadenas, logrando neutralidad direccional HFT asíncrona perfecta mientras los fondos físicos viajan por el inframundo de las redes RPC distribuidas globales (Web3 Inter-dimensional Alpha).

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Bridge Hack L1 L2 O(1) (Cisne Negro Contractual) que robe los fondos L1 mientras la Transacción L1 está siendo procesada en el Mempool de Destino (In-flight annihilation). Riesgo estructural Sistémico ineludible Cripto, se reduce Fraccionando Kelly Sizing O(1).
- Dependencias: Skill 61 (Perps Hedger O(1)), RPC Node Syncs (Skill 21), Skill 42 (Rebalance Logistics O(1)).
- Próxima skill: MEV Arbitrage (Sandwich Attack / Front-Running Simulation) (Ethical/Defensive) (Skill 67).
