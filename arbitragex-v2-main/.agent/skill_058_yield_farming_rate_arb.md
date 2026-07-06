# SKILL 058 — Cross-Protocol Yield Farming (Rate Arbitrage)

## 1. Propósito superior
Detectar e interceptar discrepancias gigantescas en las Tasas de Rendimiento (APR/APY) entre diferentes protocolos DeFi, Mercados Monetarios (Money Markets) y sistemas de Staking centralizados/descentralizados de forma atómica y dinámica. Funciona como un "Banco Central Autónomo": Si en el protocolo A te pagan 20% anual por depositar USDC, y en el protocolo B te prestan USDC al 5% anual, esta Skill enruta el capital del fondo para pedir prestado en B al 5% y re-depositarlo en A al 20%, "Imprimiendo" un Spread pasivo libre de riesgo neto (Carry Trade / Rate Arbitrage) de 15% que gotea continuamente mientras la firma ejecuta HFT con el resto del capital.

## 2. Nivel de conocimiento requerido
Ingeniero en Macroeconomía DeFi (DeFi Money Markets Architect). Especialista profundo en Protocolos de Préstamo (Aave, Compound, Euler, Morpho), Dinámicas de Tasa de Interés Variable/Estable, Emisiones de Liquidez Sintética (Farming Rewards Tokens), Arquitectura de Loop-Folding (Apalancamiento iterativo de Yield), Smart Contract Risk Management y Coste de Oportunidad de Capital.

## 3. Capacidades principales
1. Escáner Multi-Cadena de Tasas (Interest Rate Telemetry): Lee on-chain y off-chain en tiempo real las Supply Rates y Borrow Rates de cientos de pools (Aave, Spark, Venus, Mango, CEX Earn API).
2. Cálculo de Yield Real vs Inflacionario (Net APY Calculation): Descompone el "150% APY" mágico de un protocolo nuevo, entendiendo que el 5% es interés real en Base Token y el 145% se paga en el "Token Nativo Inútil" de la plataforma, aplicando un descuento masivo de volatilidad al modelo matemático.
3. Arbitraje de Tasa Inter-Protocolo (Cross-Protocol Carry Trade): Automáticamente mueve fondos de A a B. Ejemplo: Pide prestado ETH en Aave al 2%, transfiere a Lido (stETH) donde el Yield nativo L1 es 4%. Gana +2% neutralmente (Base Rate Arbitrage).
4. Auto-Compounding Continuo (Harvest Engine): Dispara la recolección periódica de Recompensas (Claim/Harvest Rewards) de protocolos secundarios, vendiéndolos atómicamente a USDC/ETH mediante la Skill 51 (Proxy Contract) y reinyectándolos al principal creando el efecto de Nieve Geométrica Compuesta.
5. Auto-Fold / Auto-Looping (Apalancamiento de Yield): Usa Flash Loans (Skill 28) para depositar USDC, pedir prestado USDC, re-depositar USDC, repitiendo el ciclo en un solo bloque 5 veces, transformando un interés diferencial de 1% neto en un 5% neto de apalancamiento, monitoreando el Health Factor de no liquidarse (Skill 52).
6. Auto-Deleveraging de Emergencia (Unwinding Parachute): Si la tasa de préstamo Borrow APR de Aave se dispara absurdamente del 5% al 60% por falta de liquidez en la red (Liquidity Crunch), la operación deja de ser rentable. La Skill lee el cambio e invoca un Unwind flash, desarmando la posición en segundos para evitar que la deuda se coma el capital base.
7. Migración Autónoma de Fondos (Yield Hopping): Si MakerDAO (DSR) eleva su tasa del 4% al 8% overnight, el Agente extrae todo el capital aparcado (Idle Capital) ineficiente de Compound y lo mueve a MakerDAO a velocidad de bloque sin intervención humana (Capital Efficiency Maxing).
8. Filtro de Auditoría Restrictivo (Risk Score Oracle): Jamás enruta $1 Millón de la tesorería hacia un protocolo que promete 10,000% APR creado hace 2 días (Riesgo de Rug Pull/Hackeo 99%). Solo escanea Whitelists institucionales de Protocolos Blue-Chip validados o cruza con la Skill de Detección de Honeypots.
9. Contabilidad de Pérdida Transitoria Sombreada (Impermanent Loss Tracking): Si la oportunidad recae sobre Yield Farming de Liquidity Pools (DEX AMMs como Uniswap V3), descuenta a tiempo real el modelo de divergencia de precio para predecir si el Yield supera la pérdida transitoria teórica.
10. Gestión de Liquidez Fraccional Excedente: El Bot es ante todo HFT (Alta frecuencia). El capital en HFT debe ser líquido. El Yield Farming de esta Skill actúa sobre el "Capital Idle" (Ej. los fondos congelados del Cold Storage Skill 43, o el 20% del inventario que matemáticamente el bot no puede operar hoy). Funciona como "Tesorería Pasiva".

## 4. Entradas requeridas
- `money_markets_api_stream`: RPC Calls on-chain solicitando datos como `getReserveData()`.
- `idle_inventory_status`: Cantidad de dinero sentada "sin hacer nada" del Inventario unificado (Skill 40).
- `gas_cost_oracles`: Tarifas actuales de gas L1/L2 para saber si mover el dinero a farmear gasta más gas que lo que dará de ganancia en 1 semana.

## 5. Salidas esperadas
- `yield_allocation_commands`: Órdenes `deposit()`, `borrow()`, `repay()`, `withdraw()` on-chain.
- `harvesting_trigger`: Llamadas diarias/semanales de `claimRewards()`.
- `net_yield_tracker_usd`: Log asíncrono sumando el dinero "goteado" diariamente a los PnL del bot central (Skill 38).

## 6. Reglas inmutables
- Nunca operar Yield Farming o Carry Trades que incurran en Riesgo de Descalce Direccional (Currency Risk Mismatch). Ej. Prestar USDC al 2% para pedir prestado Ethereum y apostarlo al 5%. Estás expuesto a que si el ETH sube un 50% frente al USDC te liquidan toda la cartera por insolvencia temporal (Beta Trap). Arbitraje de tasas SÓLO Like-for-Like (ETH vs ETH, USDC vs USDT, BTC vs WBTC).
- Cualquier estrategia de Yield de bucle apalancado (Loop-Fold) requiere un escudo anti-liquidaciones paramétrico local de 2 niveles de redundancia. (El HF nunca puede acercarse al límite 1.0, debe mantenerse > 1.5 a prueba de fallos de oráculo).
- La ganancia Proyectada del Yield DEBE amortizar los Costes Fijos de Entrada (Gas L1 Transfer, Swap de Reward) en un plazo configurable (Ej. `Break_Even_Days < 10`). De lo contrario, se anula la entrada al pool por ineficiencia de desgaste (Gas burn).

## 7. Algoritmos o métodos que debe conocer
- Jump-Rate Interest Models (Modelos Kink Curve en DeFi, donde el interés salta de 5% a 100% cuando la "Utilization Rate" pasa el 80%).
- Algoritmo de Flujo de Optimización Continua (Continuous APY Compounding Math).
- Smart Contract de Bóveda Proxy de Deleverage Atómico (Contratos de desenredo Flash).

## 8. Fórmulas críticas
- **Carry Trade Neto**: `Yield_Neto_Base = (Supply_APR_A * Allocation_Size) - (Borrow_APR_B * Allocation_Size)`
- **Interés Compuesto Anualizado (APY)**: `APY = (1 + APR / N) ^ N - 1` (Donde N es frecuencia de autocompounding).
- **Utilization Rate (Kink Danger)**: `U = TotalBorrows / TotalLiquidity`. Si `U > Optimal_Point_Kink`, el `BorrowRate` crece en modelo hiperbólico destructivo.

## 9. Casos extremos
- Secado de Liquidez (Liquidity Crunch): Depositas 1000 ETH ganando 10%. Vas a sacarlo mañana. Resulta que todos los ETH fueron prestados a terceros y la piscina tiene un balance líquido de 0 ETH. Tu dinero queda "Trabado/Atascado" (Locked) hasta que alguien devuelva su préstamo. Esto arruina la tesis de Liquidez Dinámica HFT del fondo. Solución: La Skill parametriza un "Utilization Penalty Cap". No deposita capital en Pools cuya Utilización de Fondos exceda el 80%, garantizando Retiros Instantáneos a cualquier hora del día.
- De-peg del Activo Yield-Bearing: `stETH` (Lido) vs `ETH` históricamente da +4% y es seguro. Tras un retraso en la blockchain de retiros de Shanghai (Merge), el mercado entra en pánico y vende sus `stETH` a 0.95 ETH. Si tu bot estaba metido haciendo Fold, es liquidado a mercado. (Solución cruzada: Integration Flight-to-Safety con Skill 44 - Peg Alerts; el bot desenvuelve el staking en el mercado secundario DEX salvando el núcleo al primer indicio del gap).
- Emisión de Rewards Tóxicas a Cero: La estrategia rendía 20% porque el Protocolo Dapp Regalaba "SuperFarm Token". De repente, SuperFarm Token crashea a valor $0 en el mercado por hiper-inflación del código. El Yield real pasa de 20% a 0.5% overnight. El motor debe leer los Oráculos Spot al vuelo y reaccionar migrando los fondos el mismo día, desechando cálculos de APY teóricos inflados mostrados en el frontend de la DApp engañosa.

## 10. Validaciones obligatorias
- PRE: Ejecutar un Análisis de Riesgo de Contrato Inteligente local (TVL > $1 Billón auditado mínimo) ignorando por completo piscinas ultra-exóticas (Rug-Pulls) que dan señales en falso del 4,000,000% APR.
- CÁLCULO: Mapear la Tasa APY en Equivalencia APY base Continua. Las Dapps muestran APY compuesto que asume que "reclamas el premio cada 1 minuto" mágico. El bot ajusta y degrada el número usando el coste del Gas real del Auto-Compound programado.
- POST: Si la Tasa Nominal "Borrow APR" de mi apalancamiento supera a la Tasa Nominal "Supply APR", se acciona el Paracaídas Automático Flash (Auto-Unwind Loop) eliminando la deuda inmediatamente.

## 11. Criterios de aprobación
- Capacidad asíncrona de modelar 10-20 Mercados de dinero base globales (Aave Ethereum, Aave Polygon, Radiant, Maker, CEX Earn) consolidando el mejor ruteo en el Dashboard central.
- Extracción autónoma de beneficios de forma desatendida (Zero-Click Harvesting) empujando stablecoins limpias a la bóveda central una vez por semana como si fuera nómina pasiva institucional.

## 12. Criterios de rechazo
- El sistema inyecta "Fondos Activos HFT de Trabajo" a las bóvedas de Yield, asfixiando a las Skills de Arbitraje 12 a 15, dejándolas sin "Pólvora" para atacar ineficiencias de corto plazo (La peor ineficiencia de Capital). El Yield Farming SOLO debe comer de las sobras congeladas.
- Enfoque Direccional. Tomar préstamo en USDT para farmear en BNB Pool. (Descuadre cambial prohibido sin Delta-Neutral Hedging total).

## 13. Riesgos que mitiga
- La Erosión Oculta del Capital Inactivo (Inflationary Drain): Mantener $10 Millones de Dólares (USDT/USDC) en Binance Spot esperando arbitrajes significa un Costo de Oportunidad perdido abismal del 5%-10% anual (Risk-Free rate global + Inflación macro). El bot, siendo una IA completa, entiende que "Dinero que no se mueve es dinero que se pudre", parcheando el agujero estacionario más grande de los fondos retail.
- Riesgo de Pérdida Transitoria Absoluta (Impermanent Loss Trap): Muchos caen en hacer AMM Uniswap Liquidity Provisioning ciego. El Agente usa "Arbitraje de Tasa Pura" prestamista sobrecollateralizado, esquivando las matemáticas predadoras del Constant Product AMM que a menudo licúan el beneficio a Cero.

## 14. Integración con otras skills
- Revisor pasivo del "Idle Inventory" global (Skill 40).
- Consume Smart Contracts L1 (Skill 51) para la migración atómica de capital o reclamación de Recompensas (Harvesting API).
- Integración Vital con Alertas de Desvío Peg LST/Stables (Skill 44) como sistema Anti-Liquidation Panic Alert.

## 15. Modelo de datos sugerido
```json
{
  "RateArbitragePosition": {
    "job_id": "YIELD_CARRY_USDC_01",
    "timestamp_ms": 1714521234105,
    "asset": "USDC",
    "supply_venue": "maker_dao_dsr", // Paying 8%
    "borrow_venue": "aave_v3_arb", // Costing 3%
    "allocation_usd": 500000.0,
    "net_spread_apr_projected": 5.0, // Clean 5% net
    "unwind_trigger_apr_convergence": 0.5, // If gap shrinks to 0.5%, kill the position
    "accumulated_harvested_rewards_usd": 4125.00,
    "status": "ACTIVE_FARMING_DELEGATION"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Background (Cron-like scheduler) que lee oráculos cada hora y corre rutinas On-Chain para recalcular APY real e Invocaciones Contractuales Proxy si hay convergencia / des-convergencia masiva de APRs L1.

## 17. Logs obligatorios
- `[INFO] Idle Inventory Scan complete. $1.5M USDC unallocated. Found Maker DSR offering 5.2%. Routing idle capital via L1 Proxy Contract.`
- `[DEBUG] Auto-Compound Triggered. Harvested 4,200 COMP Tokens. Swapped via Curve to 345 USDC. Re-supplied to Core Vault. Geometric APY sustained.`
- `[CRITICAL] Kink Danger Limit Breached! Aave USDC Utilization reached 95%. Borrow Rate exploding to 45%. Initiating Emergency Deleverage Flash-Unwind Sequence.`

## 18. Métricas obligatorias
- `passive_yield_generated_monthly_usd`.
- `average_capital_efficiency_deployment_pct` (El fondo nunca debe tener más del X% parado en billeteras inertes The "Sweat your assets" metric).
- `rate_arbitrage_net_spread_live_bps`.

## 19. Tests unitarios
- Tasa APY vs APR Converter: Proveer un contrato DApp que anuncia "20% APR". Simulador que reclama el premio Diario (`N = 365`). La función matemática DEBE escupir un `APY = 22.13%` probando la maestría del interés continuo. Si reclama a Cero (Mensual/Anual), el APY se hunde, dictaminando qué tan rentable es el Auto-Compound frente al Gas.
- Break-Even Gas Optimizer: Simulador donde Depositar $100 paga 100% APR ($100 al año). Pero Mover el dinero a Ethereum Cuesta $15 de Gas. El Bot debe rechazar la operación indicando "Break-Even = 54 días" (Excede la vida útil de un spread volátil DeFi). El Optimizador solo debe aceptar "Sizings Ballena" L1 o "Micros" en L2 para Farmear.
- Kink Rate Simulator: Alimentar con un Pool de Liquidez falso cuyo Uso (Total Borrow) se acerca al límite máximo y entra en hiper-asíntota de costo prestatario. La Ecuación Preditiva Local debe alertar el choque de tasas y ordenar "Unwind" preventivo ANTES del colapso del API en bloque 2.

## 20. Tests de integración
- Forcar Mainnet (Anvil). Concederle al Contrato Proxy 1 Millón de DAI inútiles. Hacer correr el Subproceso Yield Hopper. El Bot debe generar el "Payload Atómico" de depósitos L1 (aToken/cToken mapping). La ejecución sobre el testnet arroja los Saldos Aave/Compound crecientes validando que las llamadas Smart Contract de Aprobación/Supply son 100% compatibles ABI sin revertir.

## 21. Tests E2E
- El agente supremo (Brain) se percata de que por falta de volatilidad mundial, el motor HFT no generará nada hoy. Pasa el 90% de sus dólares locales al Gestor de Tasas (Skill 58). El Gestor descubre que Ethena (USDe) o Pendle están ofreciendo 12% libre de riesgo en Stablecoins puras por programas de puntos L2, mientras tomar prestado USDC en Aave cuesta 3%. El bot orquesta atómicamente un puente L2, bloquea el colateral e inicia una "Ordeñadora Automática" (Farm/Yield Carry). Una semana después, el mercado de Bitcoin vuelve a tener liquidez y gaps triangulares masivos (Regreso al Régimen Pánico). El motor Yield lee la urgencia de liquidez HFT (Brain Event Bus), lanza la liquidación atómica retirando los fondos con su interés acumulado perfecto, y alimenta de nuevo el cañón de la Máquina de Arbitraje CEX de milisegundos sin haber perdido un minuto de ineficiencia capital.

## 22. Checklist de producción
- [ ] Incorporación de Lógica de Desvío del Funding Rate Perpetuo CEX: A veces el "Yield Farming" más brutal del mundo no está en DeFi (Contratos), sino en simplemente Shortear BTC en Binance Perpetuos porque la Tasa de Financiación (Funding Rate) paga un loco 150% anual a los shorts y tú tienes Spot BTC pasivo de base larga (Cash and Carry Arbitrage clásico), integración pura con Skill 16.
- [ ] Prevención de Bloqueo de Emisiones (Vesting Locks): Hay Dapps ("veTokenomics" estilo Curve/Solidly) que bloquean tu depósito 4 años para darte el APY real. El filtro debe rechazar estrictamente cualquier pool con `Withdrawal_Delay > 0` o `Vesting_Time_Lock`. Liquidez perpetua exigida.
- [ ] Lectura del TVL Umbral (Total Value Locked limit). Para evitar "Price Impact" si tienes que sacar dinero masivo. Si el pool tiene $50 Millones total y el bot quiere ingresar $10 Millones (Ocupando el 20%), el bot mismo destruirá el APY del pool ("Diluting the rewards pool"). Auto-Capping volumétrico logarítmico.

## 23. Ejemplo de configuración no hardcodeada
```yaml
cross_protocol_yield_optimizer:
  enabled: true
  minimum_idle_capital_allocation_usd: 10000.0
  minimum_net_spread_apr_trigger: 3.5
  acceptable_protocols_whitelist: ["aave_v3", "maker_dsr", "compound_v3", "spark"]
  unwind_emergency_threshold_borrow_apr: 20.0
  max_gas_cost_to_projected_profit_ratio: 0.05 # Gas can't exceed 5% of monthly expected profit
  auto_compound_trigger_frequency_hours: 168 # Weekly by default to save gas
```

## 24. Ejemplo de pseudocódigo
```javascript
class YieldHoppingEngine {
    constructor(idleCapitalMap) {
        this.availableInventory = idleCapitalMap;
        this.activeDelegations = new Map();
    }

    async scanMarketAndDeploy() {
        const globalRates = await DeFiRateOracles.fetchAllProtocols();
        
        for (let [asset, idleAmount] of this.availableInventory) {
            if (idleAmount < CONFIG.min_idle_cap) continue;
            
            // Find highest safe APY
            const bestSupply = findMaxApy(globalRates.supply, asset, CONFIG.whitelist);
            const cheapestBorrow = findMinApr(globalRates.borrow, asset, CONFIG.whitelist);
            
            const netCarryTradeApr = bestSupply.apy - cheapestBorrow.apr;
            
            if (netCarryTradeApr > CONFIG.min_net_spread_trigger) {
                await this.initiateCarryTradeAtomic(cheapestBorrow.protocol, bestSupply.protocol, asset, idleAmount);
            }
        }
    }

    async initiateCarryTradeAtomic(borrowVenue, supplyVenue, asset, amountUsd) {
        // Smart Contract (Skill 51) Atomic Routing:
        // 1. Borrow Asset at X%
        // 2. Supply Asset at X+Y%
        // 3. Register Internal Shadow Hedge (Keep LTV under 60% locally and safely)
        
        const payload = SmartContractBuilder.createYieldCarryPayload(borrowVenue, supplyVenue, asset, amountUsd);
        const receipt = await MevRouter.dispatchPrivateTx(payload); // Skill 50 - Stealth deploy
        
        this.activeDelegations.set(jobId, { status: 'ACTIVE', baseAsset: asset, amount: amountUsd });
        log.info(`Idle Capital deployed to ${supplyVenue} at APY difference of +${netCarryTradeApr}%`);
    }
}
```

## 25. Criterio final de excelencia
El Cross-Protocol Yield Farming Engine transforma a tu fondo de un "Scalper Seco en Tiempos Laterales" a una Batería Financiera Cuántica de Flujo Perenne. Logra que el bot imprima Alpha, incluso si el Arbitraje CEX cae muerto por 6 meses por colapso global, actuando como la Columna Vertebral Financiera (Tesorería Institucional Pasiva) que garantiza la supervivencia y apalancamiento incondicional de los recursos estancados de la máquina.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Protocol Smart Contract Exploits (Hackeos al Contrato L1). Solucionable al 90% inyectando integraciones a protocolos de Aseguradoras DeFi (Ej. Nexus Mutual) protegiendo el Capital del Bug si las tasas compensan la Prima de seguro.
- Dependencias: Inventario Local (Skill 40), Orquestador L1 Proxy (Skill 51).
- Próxima skill: Detección de Slippage Invisible (Exchange slippage profiling) (Skill 59).
