# SKILL 052 — Liquidation Engine Tracker (Aave/Compound)

## 1. Propósito superior
Rastrear los Protocolos de Préstamos DeFi (Lending Protocols como Aave, Compound, MakerDAO) para identificar posiciones de deuda de usuarios ("Trove/CDP") que están al borde de ser Liquidadas. Cuando el mercado sufre una caída (Flash Crash), los prestatarios sub-colateralizados son rematados. El Bot actúa como un Liquidator/Keeper (Liquidador Bot), pagando la deuda del prestatario malo y obteniendo a cambio su colateral (Ej. WBTC o ETH) con un descuento masivo garantizado por contrato (Bonus de Liquidación de 5% al 10%). Es una fuente de alpha hiper-lucrativa y agnóstica a la dirección general del mercado.

## 2. Nivel de conocimiento requerido
Ingeniero MEV Especialista en Liquidaciones. Comprensión profunda de Health Factor (Factor de Salud), Dinámicas de Oráculos L1 (Chainlink Heartbeat and Deviation thresholds), Close Factors (Cuánto % del CDP puedes liquidar de un golpe), Mecanismos LTV (Loan-to-Value) y Liquidation Penalties integradas en los Smart Contracts DeFi.

## 3. Capacidades principales
1. Shadow Tracking de Cuentas (State Syncer): Rastrea el Evento `Borrow` y `Deposit` emitido por el contrato de AAVE para mantener una Base de Datos Local de TODAS las cuentas de deudores en la red junto a sus saldos de deuda y colateral, en O(1) in-memory.
2. Predicción de Disparo de Oráculo (Oracle Front-Running): Sabe que Chainlink actualiza el precio cuando la desviación supera el 0.5%. Si el bot ve que el precio CEX ha bajado un 0.6%, *sabe* matemáticamente que en el siguiente bloque o dos, Chainlink enviará la actualización a Aave. El bot prepara el "Bundle" atómico de liquidación y lo alinea para que impacte en el exacto milisegundo/bloque donde el oráculo dispara, matando al deudor antes que los rivales.
3. Health Factor Calculation en Memoria: Calcula continuamente el factor de salud (`(Colateral_en_USD * Umbral_Liquidacion) / Deuda_en_USD`) para decenas de miles de carteras simultáneamente sin depender de peticiones RPC L1 (Que son carísimas y asfixian el servidor).
4. Auto-Swapping Atómico: El bot liquida la deuda ajena usando un préstamo Flash Loan (Skill 28), se queda con el WBTC de premio, lo vende por USDC en el mismo bloque para pagar el Flash Loan, y se guarda el Spread de ganancia libre de riesgo en su Proxy L1 (Skill 51).
5. Optimizador de Gas Competitivo: La liquidación DeFi es un deporte sangriento de 1 milisegundo (Gas Wars). Esta skill decide si pagar a Flashbots (Skill 50) el 95% del premio (Ej. Regalar 4.5% de ganancia de 5%) solo para robar la liquidación y ganar el 0.5% en volumen industrial.
6. Gestión Multicadena (Cross-chain Keeper): Vigila no solo Aave Ethereum, sino Radiant en Arbitrum, Benqi en Avalanche, o Venus en BSC. Las redes menores tienen menos competencia, y los "Bonus de liquidación" quedan vivos por varios segundos en vez de milisegundos.
7. Liquidación Parcial vs Total: Dependiendo del "Close Factor" (Ej. Aave V3 permite liquidar 50% de la deuda si HealthFactor está entre 0.95 y 1.0, pero 100% si cae bajo 0.95), el bot optimiza el tamaño del payload exacto para extraer la máxima rentabilidad L1 sin revertir.
8. Monitoreo de Yield-Bearing Collaterals (LSDs/aTokens): Saber que liquidar a un deudor respaldado por `wstETH` no te da `ETH` líquido, te da `wstETH`. Debe calcular la ruta de swap del LSD (Curve Stableswap) para realizar la ganancia real a Fiat/Stables.
9. Detección de Deuda Toxica (Bad Debt Filter): No liquidar posiciones hiper-ilíquidas (Ej. Alguien puso CRV como colateral pero no hay liquidez en DEX para vender esos CRV resultantes de la liquidación y recuperar el dinero). Evita "Bags" ilíquidos que estancan el portafolio.
10. Sincronización Automática de Interés Compuesto (Rate Accrual Sync): La deuda DeFi crece en cada bloque por el APR variable. El balance registrado localmente hace un mes ahora es +1% mayor por el interés. El Bot aplica funciones de `RAY_MATH` (Aave Math) locales para inflar las deudas simuladas al compás del contrato real sin necesidad de consultas lentas al RPC.

## 4. Entradas requeridas
- `onchain_lending_events`: Stream Websocket L1 L2 filtrando `Deposit`, `Withdraw`, `Borrow`, `Repay`.
- `oracle_prices_realtime`: Precios milisegundo-a-milisegundo de Binance (CEX) vs Precio en Contrato Oráculo L1.
- `defi_protocol_configs`: JSON conteniendo Liquidator Bonus Pct, LTV Máximos y Reserve Factors.

## 5. Salidas esperadas
- `liquidation_target`: Array indicando `[DeudorAddress, DebtAsset, CollateralAsset, AmountToLiquidate]`.
- `profit_estimate`: Float calculando "Bonus - FlashloanFee - GasCost - SwapSlippage".
- `liquidation_execution_bundle`: Payload final atómico enrutado a la Skill 50.

## 6. Reglas inmutables
- El motor NUNCA hace "Polling" masivo (`getAccountData(address)`) a la Capa 1 RPC para 100,000 cuentas en un loop infinito. Eso resultará en un "Ban por Abuso de Rate Limits de Alchemy/Infura" y bloqueará el sistema. Toda la tabla de estados de los deudores DEBE calcularse localmente usando una réplica (Shadow State) en RAM.
- Todas las Liquidaciones DEBEN estar envueltas (Wrapped) en lógica de Swap Atómico a Stablecoin o Activo Mayor (ETH/BTC) al final de la transacción en el Smart Contract L1. Nunca quedarse con el Colateral Tóxico de un tercero bajo "Market Risk".
- Una oportunidad de Liquidación NO SE ENVÍA si el cálculo predice un profit neto negativo al restar tarifas de Flashbots y gas de red.

## 7. Algoritmos o métodos que debe conocer
- WAD/RAY Math (Aritmética decimal de 27 y 18 ceros implementada en Solidity, replicada en JS/Rust `WadRayMath`).
- Grafos de Estado en Memoria Continua (In-Memory Shadow EVM o Subgraphs locales).
- Algoritmo de "Pending Oracle Trigger" (Caza del Update).

## 8. Fórmulas críticas
- **Health Factor (AAVE)**: `HF = sum(CollateralInEth * LiquidationThreshold) / TotalDebtInEth`
- **Condición de Liquidación**: `if (HF < 1.0) { TargetIsLiquidatable = TRUE }`
- **Rentabilidad de Liquidación**: `Profit = (DebtRepaid_USD * Liquidation_Penalty_Pct) - FlashLoanFee - SwapSlippage - BribeFee`

## 9. Casos extremos
- Flash Crash (Crypto Black Thursday): El mercado colapsa 30% en 1 hora (Ej. Caída de COVID-19 o FTX). El número de cuentas que caen por debajo de HF=1.0 pasa de 10 a 5,000 al mismo tiempo. El Bot (Node.js) crashea por saturación de RAM o ahogo de API si su estructura `O(N)` está mal diseñada (Iterar 5000 cálculos bloqueantes). El sistema debe operar en "Batch Priority Queues" priorizando liquidar la cuenta de $10 Millones antes que las 4,999 cuentas de $10 dólares, maximizando retorno computacional.
- Deuda Incobrable (Protocol Insolvency): Hack de LUNA/UST en Venus BSC. LUNA cae a 0 en CEX, pero el Oráculo L1 falla y lo marca como si valiera algo. Si el Bot intenta liquidarlo, recibirá LUNA (valor $0 real) y pagará dólares reales por él. Obligatorio implementar control de "Oracles Divergence" y suspender liquidaciones en activos de De-Pegging craso (Integración Skill 44).
- Race Condition Flashbots: Mandas a liquidar un gordo de $1M ganando el Bonus del 5% ($50k). 100 bots hacen lo mismo. Tu bundle compite. Terminas pagando el 99% a Flashbots ($49.5k a mineros) y ganando $500. La skill evalúa la "Curva de Rentabilidad Agresiva" contra bots competidores observando los mempool históricos.

## 10. Validaciones obligatorias
- PRE: Escanear la liquidez profunda en AMMs L1 (Uniswap/Sushiswap) del `CollateralAsset` para asegurar que el DEX puede absorber la venta atómica masiva (Swap a USDC) sin quebrar el Bonus del 5% y volverlo pérdida por slippage.
- CÁLCULO: Mantener una actualización rigurosa del "Índice Variable de Deuda" (Variable Borrow Index). Las deudas crecen en silencio cada segundo según la fórmula de interés compuesto. Ignorarlo arrojará Health Factors de `1.01` (A salvo local) cuando en la blockchain real ya es `0.999` (Liquidado por rivales que afinaron mejor el reloj).
- POST: Limpiar inmediatamente al deudor liquidado del arreglo en RAM tras ejecutar o ver ejecutar a la competencia el remate, ahorrando memoria L3 cache (Garbage Collection).

## 11. Criterios de aprobación
- Capacidad para procesar y actualizar (Shadow State) un universo de > 50,000 cuentas de DeFi en < 5ms a nivel local (Usando árboles de búsqueda o Heaps indexados por Health Factor).
- Emisión del payload de liquidación antes de la confirmación L1 del Oráculo (Front-running the Oracle).

## 12. Criterios de rechazo
- El sistema depende de The Graph (API Externa de Subgrafos) para leer los Health Factors. The Graph tiene un "Lag" de entre 10 y 60 segundos respecto al tip de la cadena. Si usas The Graph, serás el último bot del universo en enterarte del remate. (Rechazo absoluto de arquitectura).
- Intentar ejecutar liquidaciones usando Fondos Base (El cajón local de $50k del fondo) en lugar de Préstamos Flash ($5M de AAVE). Las liquidaciones masivas de Ballenas requieren capital Infinito Temporal (Flashloans).

## 13. Riesgos que mitiga
- Correlación Direccional (Mercado Bajista): Cuando el mercado cae 50%, la Skill 12 (Arbitraje CEX) a veces sufre asfixia. Sin embargo, en caídas estrepitosas, es exactamente cuando el Liquidator Tracker hace fortunas masivas "Rematando" a prestatarios sobre-apalancados (Bear Market Insurance). Esta skill vuelve al portafolio verdaderamente "Market Neutral" y "All-Weather" (Todo terreno).
- Retorno por Ineficiencia Algorítmica (Long-tail markets): En protocolos menores (Silo Finance, Euler), la competencia es tan baja que los bots no pelean en MEV y dejan márgenes obscenos del 10% por liquidación limpias.

## 14. Integración con otras skills
- Alimentado por la conectividad L1 de WebSockets y Bloques RPC (Skill 21).
- Empaquetador obligatorio en MEV Blocker (Skill 50) para ser invisible.
- Requiere de Flash Loan Mastery (Skill 28) para financiar la decapitación financiera.

## 15. Modelo de datos sugerido
```json
{
  "DefiLiquidatorTarget": {
    "protocol": "aave_v3",
    "network": "arbitrum",
    "user_address": "0xWhaleOverleveraged...",
    "debt_asset": "USDC",
    "collateral_asset": "WBTC",
    "calculated_health_factor": 0.998,
    "total_debt_usd": 1500000.0,
    "liquidation_penalty_pct": 5.0,
    "projected_profit_usd": 37500.0,
    "liquidation_allowed_percentage": 50, // Half-close allowed
    "status": "ARMED_AND_SENDING_TO_FLASHBOTS"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio State-Syncer en memoria (`AaveShadowEVM`) que suscribe al WSS RPC y recibe todos los logs del contrato de Pool (`Borrow`, `Repay`). Contiene un Heap (Priority Queue) donde las cuentas con HF más cercanos a 1.0 flotan hasta la cima (`O(log n)`).

## 17. Logs obligatorios
- `[INFO] Aave V3 Tracker Synced. Tracking 34,502 Active CDPs locally. Highest Risk HF: 1.002.`
- `[DEBUG] CEX Price Drop Detected (BTC: 61000 -> 60000). Predicting Chainlink Oracle Trigger. 5 CDPs crossing underwater threshold in T-2 seconds.`
- `[CRITICAL] LIQUIDATION OPPORTUNITY SPOTTED: 0xDeudorX. Debt: 5M USDC. HF: 0.98. Sending $2.5M Flashloan Liquidation Payload to MEV Relays! Estimated Profit: $8K.`

## 18. Métricas obligatorias
- `total_active_cdps_tracked` (Para monitorear Memory Leaks).
- `liquidation_win_rate_vs_competitors_pct` (Medir cuántas veces otros Keeper bots nos ganan de mano).
- `latency_oracle_trigger_to_mempool_submission_ms` (Optimizar esta métrica es la diferencia entre 0 y Millones).

## 19. Tests unitarios
- Aave Math Local Replication: Instanciar `calculateHealthFactor(debt, colat, params)`. Alimentarlo con datos crudos de la blockchain de un usuario cuyo HF es 0.992 en la DApp. El algoritmo JS/Rust DEBE escupir exactamente `0.99214434...` validando que la matemática local `RAY_MATH` imita perfecto al Smart Contract, de otro modo se lanzarán liquidaciones inválidas (Reverts).
- Heap Efficiency (Priority Queue): Inyectar 100,000 cuentas en el rastreador. Actualizar el precio de Ethereum un 10% a la baja. El motor debe recalcular los 100,000 Health Factors y arrojar el "Top 100 Cuentas en Peligro" (HF < 1) en menos de `5 milisegundos` utilizando estructuras Min-Heap y no un array `.sort()` lento que congele la V8.
- Flashloan Cost Integration: Enviar oportunidad de ganancia de 10 USDT. El FlashLoan cuesta 0.09% (Aave Fee) sobre 1000 USDT de Deuda = 9 USDT. El Gas L1 cuesta 3 USDT. Ganancia Neta: `-2 USDT`. El motor debe abortar matemáticamente este intento destructivo.

## 20. Tests de integración
- Levantar Ganache / Anvil Fork de Ethereum en un bloque histórico famoso (Por ejemplo Mayo 2021 Caída cripto). Conectar el módulo Liquidator. El módulo debe "despertar", reconstruir el estado de la red (O emitir peticiones multicall masivas para pre-calentarse), detectar decenas de cuentas rojas sumergidas e imprimir payloads válidos `eth_callBundle` masivos demostrando capacidad forense on-chain pura.

## 21. Tests E2E
- Un fin de semana de Pánico Bajista (Weekend Dump). Bitcoin cae $5000 en 15 minutos. El oráculo de Arbitrum actualiza. Decenas de Granjeros DeFi quedan en "Rojo" (`HF < 1`). El bot los tiene en la mira local. Su Priority Queue los escupe a la Skill 51. La Skill 51 empaqueta el Préstamo Aave -> Liquidación -> Venta Uniswap V3 -> Repago. Inyecta al minero. Flashbots lo asimila. Las cuentas son purgadas por el protocolo, liberando el equilibrio económico, y el bot asimila un profit sin haber retenido direccionalmente un solo Bitcoin en ningún momento, actuando como un barrendero de ecosistema automatizado.

## 22. Checklist de producción
- [ ] Optimización "State-Hydration" en el Boot (Cold Start): Al reiniciar la VPS, el bot empieza vacío y no sabe nada de los deudores. Debe hidratarse (Download State) ya sea bajando un Snapshot histórico propio, haciendo Multicall masivo (Lento/Caro) o consultando el estado base en The Graph solo la primera vez para iniciar la RAM.
- [ ] Incorporar el "Close Factor Dynamic": Aave permite liquidar el 50% normalmente, pero cuando HF cae drásticamente bajo `0.95`, el protocolo a veces permite "100% Close". El Bot debe disparar a la máxima capacidad permitida por el contrato para asfixiar competidores (Robar el premio gordo).
- [ ] Filtro contra Tokens Basura Aislados (Isolated Tier): Algunos protocolos permiten usar Memecoins como colateral aislado. El bot no debe participar en subastas de liquidación donde el colateral de premio no tenga una piscina con al menos 1 Millón de Dólares en Uniswap, asumiendo "Bolsas Illíquidas de Peligro".

## 23. Ejemplo de configuración no hardcodeada
```yaml
defi_liquidation_engine:
  protocols_active:
    aave_v3_arbitrum:
      pool_address: "0x794a61358D6845594F94dc1DB02A252b5b4814aD"
      oracle_address: "0xb56c2F0B653B2e0b10C9b928C8580Ac5Df02C7C7"
      flashloan_fee_pct: 0.0005 # Aave v3 applies 5 bps flashloan fee
      min_hf_threshold: 1.0000
      max_close_factor_hf_limit: 0.9500
  minimum_net_profit_usd_to_trigger: 50.0
  max_accounts_in_memory: 200000
```

## 24. Ejemplo de pseudocódigo
```javascript
class LiquidationHunter {
    constructor(protocolConfig) {
        this.cdpHeap = new MinHeap((a, b) => a.healthFactor - b.healthFactor);
        this.userStates = new Map(); // address -> {debt, colat, lastUpdate}
    }

    // Called on every Oracle Tick or CEX Price update (Off-chain foresight)
    async onPriceUpdate(asset, newPriceUsd) {
        // Fast O(N) or O(LogN) recalculation for assets related
        const compromisedAccounts = this.recalculateHealthFactors(asset, newPriceUsd);
        
        for (let acct of compromisedAccounts) {
            if (acct.healthFactor < 1.0) {
                await this.prepareLiquidationPayload(acct);
            }
        }
    }

    async prepareLiquidationPayload(accountState) {
        // Calculate raw profit
        const penaltyGain = accountState.debtAmount * CONFIG.liquidation_bonus;
        const swapSlippage = await DexPricer.estimateSlippage(accountState.colatAsset, accountState.debtAsset, penaltyGain);
        
        const netProfit = penaltyGain - swapSlippage - CONFIG.flashloan_fee;
        
        if (netProfit > CONFIG.min_profit_target) {
            log.critical(`Armed Liquidation on ${accountState.address}! Expected Net: $${netProfit}`);
            // Delegate to Proxy Smart Contract Orchestrator (Skill 51)
            SmartContractProxy.buildAndFireLiquidation(
                accountState.address, 
                accountState.debtAsset, 
                accountState.colatAsset, 
                accountState.maxLiquidatableAmount
            );
        }
    }
}
```

## 25. Criterio final de excelencia
El Liquidation Engine Tracker despliega al agente en el "Círculo de Buitres" de DeFi (Keeper Networks). Proporciona la cualidad técnica de "Rentabilidad Direccionalmente Inversa", es decir, el Bot se vuelve una Máquina de Hacer Dinero extrema precisamente en los días más sangrientos (Crashes/Caídas Masivas de precios), otorgando un blindaje estocástico perfecto al fondo general frente a los mercados bajistas devastadores.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Costos computacionales por re-ordenar el Min-Heap o el árbol de búsqueda si las deudas se re-calculan cada 1 milisegundo (Requiere uso extremo de Pointers y Fast-Memory en lenguajes nativos).
- Dependencias: Data Normalization (Precios) y WebSockets (RPC Node events).
- Próxima skill: Triangular Arbitrage (Intra-Exchange) (Skill 53).
