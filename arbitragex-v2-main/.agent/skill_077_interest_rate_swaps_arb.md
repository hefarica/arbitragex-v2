# SKILL 077 — Orquestador de Arbitraje de Tasas de Interés (Interest Rate Swaps IRS L1/L2)

## 1. Propósito superior
Colonizar la última frontera del Arbitraje Financiero HFT: Las Tasas de Interés (Yield Arbitrage & Interest Rate Swaps L1/L2 O(1)). Mientras el Bot Clásico busca diferencias de precios de Tokens, este Orquestador extrae Alpha inyectando billones de dólares asintóticos buscando diferenciales de Tasa de Préstamo (Borrow/Lend APY) entre protocolos DeFi L1 (Aave, Compound, Morpho, Euler L1) y Exchanges L2 (Binance Margin, OKX Loans L2). Concreta la manipulación temporal del Capital creando Sintéticos (Fixed vs Variable Rate Swaps L1/L2 O(1)) para el Fondo HFRC.

## 2. Nivel de conocimiento requerido
Quant Fixed-Income Architect L1 L2 O(1). Dominio profundo de Curvas de Rendimiento (Yield Curves Cripto L1 O(1)), Contratos de Swaps de Tasas de Interés (IRS - Pendle Finance, Voltz Protocol L1 O(1)), Matemáticas de Cash-And-Carry Extremo, Arbitraje Tri-Partito L1 L2 de Márgenes, y Convexidad de Tasas L1 (Utilization Ratio Math O(1)).

## 3. Capacidades principales
1. Arbitraje de Tasa Cruzada Pura L1 L2 O(1): Detecta que Binance Margin L2 presta USDT a 2% APY O(1), pero Aave L1 Paga 8% APY por Depósitos O(1). El Motor Pide Prestado Masivamente en L2 (CEX), usa el Ruteador L1-L2 (Bridge Atómico O(1)), Deposita en Aave L1, y bloquea un Beneficio Cripto Delta-Neutral del 6% APY Neto Libre de Riesgo HFT L1 L2 O(1).
2. Interest Rate Swaps (IRS L1 O(1)): Interactúa con Pendle Finance o Voltz L1 O(1). Intercambia Tasas Variables (Variable APY L1) por Tasas Fijas (Fixed APY L1). Si el Agente XGBoost (Skill 47 L2) predice que las tasas DeFi van a Caer (Yield Collapse L1 O(1)), Ejecuta "Receive Fixed, Pay Variable L1 O(1)" ganando millones mientras el Mercado Cripto entra en Invierno L1 L2 O(1).
3. Loop Lending (Apalancamiento Recursivo de Tasa L1 O(1)): Aave L1 Paga 10% APY en Matic y Presta a 5% APY O(1). El Motor deposita $1M Matic L1, Pide Prestado $500k Matic L1, Lo Vuelve a Depositar L1, Pide Prestado $250k L1 O(1). Ejecuta un Payload Flashloan Re-Entrante L1 O(1) multiplicando x3 el Yield Cripto HFT sin Riesgo Direccional Absoluto O(1).
4. Liquid Staking Derivatives (LSD Arbitrage L1 O(1)): Arbitra el Despeg (Desanclaje) de stETH / ETH O(1). Si stETH cae a 0.95 ETH en Uniswap L1. El Motor HFT O(1) Pide ETH Prestado en Aave, Compra stETH (Ganando 5% Arbitraje L1 O(1)), Se queda el Staking APY (4%), Cubre el Riesgo L2 CEX O(1), y Desarma Atómicamente L1 L2 Cripto.
5. Auto-Migración de Bóvedas de Tasa (Yield Optimizer O(1)): Morpho L1 paga 6%, Compound L1 paga 5%, Aave L1 paga 7% O(1). El Orquestador HFT Mueve el Capital de la Tesorería (Skill 42 L1) Atómicamente entre L1 Pools O(1) Cripto persiguiendo los "Spikes" de Tasa cada milisegundo HFT O(1), sin que los humanos se enteren (Flash-Yield Cripto O(1)).
6. Manipulación Estructural (Utilization Rate Squeezing L1 O(1)): Si el Bot tiene Liquidez Masiva O(1) (Ej. $50M), puede "Secar" un Pool de Aave L1 O(1) (Llevando la Utilización a 99% O(1)). Esto hace que la Tasa L1 O(1) salte a 150% APY O(1). Sus Enemigos (Bots Menores O(1)) quedan Liquidables. Luego el Orquestador Suelta la Liquidez L1 O(1) y Captura las Liquidaciones con la Skill 74 (Liquidation Sniping L1 O(1)). Ataque Combinado HFT L1.
7. Hedging Contra Ataques Apy L2 (Funding Rate Immunization): Cubre las posiciones Spot HFT en CEX O(1) contra Disparos de Tasa L2. Si Binance eleva el Funding Rate L2 al 200% Anual O(1), Cierra las posiciones Atómicamente L2 y Migra el Exposure O(1) Sintético L1 O(1) (Yield Cripto Arbitrage O(1)).
8. Convexidad Criptográfica L1 O(1): Analiza las Fórmulas EVM Duras (Kink Rates O(1) de Aave/Compound L1). Sabe que a partir del 80% de Utilización O(1), el Costo de Préstamo L1 es Exponencial. Emite Alertas O(1) y Cierra HFT Arbitrajes L1 justo antes de que la curva Quiebre en contra O(1).
9. Mapeo de Subsidios (Token Rewards Extraction L1 O(1)): Un Protocolo nuevo DeFi (Ej. Radiant L1) lanza su Token y Subsidia la Tasa dando Tokens RNDT Gratis a los Lenders L1 O(1). El Motor Extrae el Token O(1), Rutea a DEX L1 O(1), Dumpea a Dólar (USDC L1 O(1)), Retornando Yield Sintético Extra al Capital Base O(1) Cripto L1 HFT.
10. Arbitraje Multi-Cadena Temporal L1 O(1): Descubre que Polygon L1 O(1) tiene Tasas del 2%, pero Base L1 O(1) tiene 10%. Ejecuta Flash Cross-Chain Bridges L1 (Skill 66 O(1)), Rutea Capital Cripto, Embolsa Tasa L1 L2 O(1) y mantiene el Portafolio Estabilizado Cripto L2.

## 4. Entradas requeridas
- `defi_yield_curve_matrix_l1_o1`: Tensor RAM C/Rust O(1) con TODOS los APYs (Borrow/Supply) O(1) de TODOS los tokens en L1 y L2 CEX en Tiempo Real.
- `pendle_yield_token_orderbook_l1_o1`: Orderbook Descentralizado L1 O(1) de Tokens de Tasa Fija/Variable O(1) HFT.
- `global_treasury_idle_capital_l1_l2_o1`: Capital Flotante O(1) Cripto buscando Renta HFT.

## 5. Salidas esperadas
- `yield_farm_atomic_rebalance_l1_o1`: Transacciones EVM Complejas O(1) HFT depositando/moviendo Millones L1 O(1) para Renta Fija Cripto.
- `synthetic_irs_swaps_l1_o1`: Contratos Ejecutados para Fijar Tasas Variables L1 (Interest Rate Swaps O(1)).
- `flashloan_recursive_leverage_payload_l1_o1`: Multiplexación de Capital L1 O(1) In-Memory Cripto HFT.

## 6. Reglas inmutables
- Inmunidad a Préstamos Tóxicos L1 O(1). El Agente NUNCA Mueve Capital a Pools L1 Menores (TVL < $50 Millones O(1)) por más que ofrezcan 5000% APY. Solo interactúa con Protocolos Whitelisteados Grado-A (Aave V3, Compound V3, Maker O(1)) evitando Honeypots/Hacks L1 (Skill 68 L1 Cripto HFT O(1)).
- Control Estricto de Descalce L1 L2 (Asset/Liability Mismatch O(1)). Pides USDT Prestado en Binance L2, Envías USDT a Polygon L1. Depositas USDT, Recibes USDC Prestado. ¡Riesgo Peg USDC/USDT O(1) L1 L2! El Motor DEBE Garantizar Swap Atómico o Cobertura Perpetua L2 de la Paridad Estable Cripto (Skill 61 L2 O(1)) o Veta O(1) el Arbitraje de Tasa.
- Precisión APY/APR Continua L1 O(1) C/Rust. No calcular el APY usando APIs REST. Extraer la Curva (BaseRate, Multiplier, Kink O(1)) Directamente del Smart Contract Bytecode L1 O(1) HFT, Modelando el Salto Futuro (Predictive APY L1 O(1)).

## 7. Algoritmos o métodos que debe conocer
- Cálculo de Continuous Compounding APY C/Rust L1 O(1) (`APY = e^(APR) - 1`).
- Aave V3 Reserve Interest Rate Strategy Math L1 O(1) Cripto.
- Pendle Finance AMM V2 Invariant Math L1 O(1).

## 8. Fórmulas críticas
- **Aave V3 Borrow Rate L1 O(1)**: `If U < U_optimal: R_t = R_0 + (U / U_optimal) * R_slope1`. `Else: R_t = R_0 + R_slope1 + ((U - U_optimal) / (1 - U_optimal)) * R_slope2` (El Orquestador Simula esto en C++ HFT O(1)).
- **Recursive Yield Loop PnL L1 O(1)**: `Net_Yield = Supply_APY + (Supply_APY - Borrow_APY) * (LTV / (1 - LTV))` L1 O(1) Cripto.

## 9. Casos extremos
- Flash-Crash del Token de Reward L1 O(1): Protocolo L1 O(1) Promete 50% APY Pagado en Shitcoin X L1 O(1). El Orquestador HFT Deposita. En 10 Minutos, la Shitcoin X se desploma -90% L1 O(1). Tu Yield Real Cae a 5% O(1), Quedaste Atrapado L1. Solución: Cosecha Continua O(1) HFT Cripto (Micro-Harvesting L1 L2 O(1)). El Bot Llama `claimRewards()` Cada 10 Segundos L1 O(1) HFT y Swappea a USDT L1 O(1) Cripto, Cristalizando la Tasa en Moneda Fuerte O(1) (Risk-Free L1 L2).
- Utilization Squeeze L1 (Riesgo Bancarrota L1 O(1)): Depositaste $1M USDC en Aave L1 O(1). Otro Bot Enemigo (Searcher L1) Pide Prestado TODOS los USDC del Pool L1 O(1). La Utilización llega a 100%. TÚ YA NO PUEDES RETIRAR TUS FONDOS L1 O(1) (Liquidity Lock O(1)). Solución Predictiva HFT O(1): El Motor Monitorea el Mempool L1 O(1) (Skill 67). Si Detecta una TX que secará el Pool L1, El Orquestador "Front-Runea L1 O(1)" el Retiro, Sacando tu Plata antes de la Asfixia Liquida L1 Cripto.
- Interest Rate Swap Peg Break L1 O(1): Estás haciendo IRS en Pendle L1 O(1) y el Contrato Inteligente sufre Desvinculación de Tasa O(1). El Orquestador Inyecta DAG Inverso L1 L2 (Skill 75 O(1)) liquidando posiciones Subyacentes Atómicamente L1 Cripto O(1).

## 10. Validaciones obligatorias
- PRE: Chequeo de Costos de Gas L1 O(1). Si depositar en Aave Mainnet L1 O(1) Cuesta $150 USD de Gas O(1), y Genera $5 USD al Día L1. Tardarás 30 Días en Cubrir el Costo O(1). El Bot Beta-Calcula O(1) el `Break_Even_Days_L1` HFT y si es Mayor a 24 Horas L1 O(1), Veta O(1) la Operación de Tasa L1 Cripto.
- CÁLCULO: Chequeo de Volatilidad (Skill 76/78 L1 O(1)). El Arbitraje de Tasa exige Tiempos Largos (Low Turnover L1 L2 O(1)). Si la Varianza Macro Cripto O(1) es Alta (Crash O(1)), El Capital Base DEBE Estar Libre O(1), Cancelando Arbitrajes de Yield L1.
- POST: Si Operó Yield HFT L1 O(1) Cripto, se asignan Cronjobs Locales O(1) C++ para Monitorear el Saldo/APY L1 O(1) cada 1 Seg.

## 11. Criterios de aprobación
- Capacidad de Hallar y Ejecutar Arbitrajes Cruzados O(1) de Tasa L1 L2 (CEX Borrow -> DEX Supply L1 O(1)) Computando Costos de Swap y Bridge (Skill 66 O(1)) Retornando Spread Real L1 O(1).
- Implementación de Recursive Leveraging L1 O(1) usando Flashloans (Skill 28 L1 O(1)) Acelerado por ReVM In-Memory L1 O(1).

## 12. Criterios de rechazo
- Basarse Solamente en APIs L2 O(1) (Ej. Coinmarketcap APY O(1)). El Bot DEBE Leer el EVM Storage O(1) de Aave/Compound HFT Localmente O(1) para Hallar las Tasas en el Bloque Preciso L1 O(1) Cripto. El Polling REST Cripto Miente y Está Desfasado.
- Ignorar Riesgo Smart Contract L1 O(1). Yield no es Risk-Free L1 O(1). Solo Se Activa Si el Antibody L1 (Skill 68) Da Luz Verde Cripto O(1).

## 13. Riesgos que mitiga
- La Erosión O(1) HFT de la Tesorería. Si el HFT Cripto gana Dinero (Skill 42 L1 L2) y deja $10 Millones Inactivos en Dólares L1 L2. La Inflación Cripto (Cost Of Capital L1 O(1)) los Destruye. Esta Skill O(1) Convierte el Capital Ocioso L1 L2 O(1) en un Depredador Pasivo que Absorbe la Renta O(1) Financiera de todo el Ecosistema Cripto Atómicamente, sin Aumentar el Riesgo de la Operativa HFT O(1).

## 14. Integración con otras skills
- Alimentador L1 L2 O(1) de la Tesorería Central (Skill 42 L2 O(1)).
- Componente de Arbitraje Tensorial Múltiple L1 L2 (Skill 75 O(1)).
- Usa Flashloans L1 (Skill 28 O(1)).

## 15. Modelo de datos sugerido
```json
{
  "InterestRateArbitrageOrchestratorL1_O1": {
    "job_id": "RATE_ARB_AAVE_L1_VS_BINANCE_L2_O1",
    "timestamp_ms_o1": 1714521234105,
    "strategy_o1": "CROSS_DOMAIN_CASH_AND_CARRY_YIELD_O1",
    "target_asset_l1_l2": "USDT",
    "execution_o1": {
      "borrow_l2_cex": "binance_margin_l2",
      "borrow_apy_cost_l2_o1": 1.5, // 1.5% APY
      "supply_l1_dex": "aave_v3_arbitrum_l1",
      "supply_apy_reward_l1_o1": 8.2, // 8.2% APY
      "recursive_leverage_multiplier_l1_o1": 1.0 // Simple Arbitrage O(1)
    },
    "net_spread_apy_l1_l2_o1": 6.7,
    "capital_deployed_usd_o1": 2500000.0,
    "projected_daily_yield_usd_o1": 458.90, // Free Passive HFT O(1)
    "status": "YIELD_MONITORING_ACTIVE_L1_L2_O1"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Subproceso C/Rust HFT L1 L2 O(1) `YieldCurve_Scanner_O1`. Construye la Curva Cripto O(1) Leyendo Nodos EVM O(1) y Websockets CEX L2. Dispara `executeArbitrageL1L2O1()` Atómicamente HFT.

## 17. Logs obligatorios
- `[INFO] IRS Orchestrator L1 L2 O(1): Interest Rate Spike Detected on OKX L2 (Borrow USDT hit 15% APY). Withdrawing AAVE L1 O(1) Liquidity via MUX Bridge O(1), Deploying as Lending Capital on OKX L2 O(1) for Risk-Free Spread HFT.`
- `[DEBUG] Yield Curve Convexity Warning L1 O(1): Compound USDC utilization reached 95% L1. Entering Kink Curve. Withdraw Front-run Prepared in Mempool L1 O(1) to avoid Liquidity Trap O(1) HFT.`
- `[CRITICAL] PENDLE IRS EXECUTION L1 O(1)! Fixed Rate 10% APY Secured. XGBoost Signal Predicts Yield Collapse. Locking in Maximum Arbitrage Spread HFT L1 O(1).`

## 18. Métricas obligatorias
- `global_treasury_blended_apy_l1_l2_o1` (Rendimiento del Fondo Inactivo Cripto L1 L2).
- `utilization_rate_delta_l1_o1`.
- `cross_chain_rate_arbitrage_volume_l1_o1`.

## 19. Tests unitarios
- Aave APY C/Rust O(1) Math: Input `U=0.9, U_opt=0.8, Slope1=0.04, Slope2=0.5`. Output Esperado L1 C++ DEBE ser Exponencial `> 25% APY L1 O(1)`. Simula el Spike Cripto HFT.
- Break Even Gas Calc O(1) L1: Depositar $1000. Yield 10% APY ($100 año). Gas Deposito $10. Gas Retiro $10 L1. Total Fee $20. Output DEBE Ser: `Break_Even = 73 Días L1 O(1)`. El Motor HFT O(1) Veta la operación (Exceso de Tiempo de Exposición).

## 20. Tests de integración
- Levantar Mocks de Aave L1, Pendle L1, Binance Margin L2 O(1). Alterar las Variables O(1) Para crear un Hueco O(1) del +10% APY L2-L1. El Agente DEBE transferir L2, Bridjear L1 (Skill 66 O(1)), Depositar L1 (Atómicamente O(1) HFT), Validar Saldos y Armar Monitoreo Asíncrono O(1) L1 Cripto.

## 21. Tests E2E
- Mercado Crypto Lateraliza. Arbitraje Triangular L2 Muerto. El Agente HFRC Cripto O(1) Migra TODO Su Poder de Cómputo Cripto L1 L2 a la Skill 77 (Interest Rate Swaps O(1)). Detecta que "Voltz L1" Está pagando a los Traders O(1) para Proveer Tasa Fija L1. El Bot Pide Préstamo Flash L1 Cripto, Lo Pasa por Voltz IRS L1, Fija Tasa O(1), Genera un "Risk Free +15% Cripto O(1)" y se Queda Durmiendo Múltiples Días Monitoreando O(1) HFT. Ganó Capital Constante sin tocar el Orderbook L2 CEX HFT Cripto. Hegemonía O(1) Lograda.

## 22. Checklist de producción
- [ ] Orquestador de Liquidación Manual L1 O(1): Si Binance L2 Cripto O(1) Sube la Tasa de Margen L2 Al 100% Cripto (Riesgo). El Bot Tarda Minutos L1 O(1) En Mover Fondos. DEBE Haber un Safety Buffer O(1) Cripto L2 del 20% Para Absorber la Pérdida del Spread HFT O(1) Mientras se hace el Unwind L1 Cripto.
- [ ] Monitor de Emisiones Tóxicas L1 (Reward Dumps O(1)): Extraes Token AAVE O(1) L1 Gratis. NO GUARDES EL TOKEN L1 O(1) HFT. Configura un MUX O(1) Para Auto-Dumpear TODO Token L1 Emitido Como Reward Instantáneamente O(1) en Curve/UniV3 Cripto, Mutando la Renta Exótica en Renta Dólar L1 L2 O(1) Fija.

## 23. Ejemplo de configuración no hardcodeada
```yaml
interest_rate_swaps_arb_l1_l2_o1:
  enable_cross_exchange_yield_arbitrage_l1_l2_o1: true
  enable_recursive_leverage_flashloan_l1_o1: true
  minimum_net_apy_spread_threshold_o1: 3.5 # Minimum 3.5% risk-free APY L1 L2 to execute
  maximum_gas_break_even_days_l1_o1: 7.0 # Don't lock capital for > 7 days to pay L1 Gas
  auto_dump_yield_farm_reward_tokens_l1_o1: true
  emergency_kink_rate_auto_unwind_o1: true
```

## 24. Ejemplo de pseudocódigo
```javascript
// C/Rust Subprocess Yield Searcher L1 L2 O(1)
class InterestRateArbSearcherO1 {
    constructor(defiMatrixL1, cexMatrixL2, routerMUX_L1L2) {
        this.defi = defiMatrixL1;
        this.cex = cexMatrixL2;
        this.router = routerMUX_L1L2;
    }

    async scanGlobalYieldCurveO1() {
        const bestL2Borrow = this.cex.getLowestBorrowApyO1("USDT");
        const bestL1Supply = this.defi.getHighestSupplyApyO1("USDT");

        // O(1) Convex Spread Calc L1 L2
        const netSpread = bestL1Supply.apy - bestL2Borrow.apy;
        
        // HFT Decision O(1) L1 L2
        if (netSpread > CONFIG.min_apy_spread) {
             const breakEvenGas = this.defi.calculateGasBreakEvenDays(bestL1Supply.venue);
             
             if (breakEvenGas < CONFIG.max_days) {
                  log.critical(`RATE ARB O(1): Discovered ${netSpread}% Spread. L2 Borrow -> L1 Supply. Deploying HFT Mux.`);
                  
                  // Atomic Execution L1 L2 O(1)
                  await this.router.executeCrossDomainYieldArbitrage(
                      bestL2Borrow.venue, 
                      bestL1Supply.venue, 
                      "USDT"
                  );
             }
        }
    }
}
```

## 25. Criterio final de excelencia
El Motor IRS (Interest Rate Swaps) Corona al Sistema HFRC L1 L2 O(1) como una Institución de Renta Fija Cripto Superior O(1). Escapa de la volatilidad sangrienta del Spread Direccional L2 CEX O(1) y Monetiza el "Costo del Dinero Cripto" Cíclico O(1). Usando Flashloans HFT y Arbitrajes Tensor L1 L2, Extrae Renta Institucional Matemática Absoluta (Risk Free L1 L2 O(1)) Destruyendo Definitivamente la Necesidad de Movimientos del Mercado Cripto HFT.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: De-Peg del Derivative Liquid Staking O(1) L1 (Ej. stETH Colapsa -10% vs ETH L1). Arruina el Arbitraje Yield L1 L2 O(1). El MUX (Skill 64 L1 L2 O(1)) Debe CORTAR Atómicamente O(1) Ante Ineficiencias Estructurales de Paridad L1 L2.
- Dependencias: Tesorería HFT (Skill 42 L1 L2 O(1)), Ruteador L1 L2 (Skill 64/75 O(1)), Flashloans (Skill 28 L1 O(1)).
- Próxima skill: Simulador Estocástico Monte Carlo HFT (Value At Risk en O(1) L2) (Skill 78).
