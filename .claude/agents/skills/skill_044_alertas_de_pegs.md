# SKILL 044 — Alertas de desvío de pegs (Stablecoins/LSDs)

## 1. Propósito superior
Vigilar y reaccionar de forma atómica a la ruptura del vínculo matemático (De-Pegging) de activos anclados 1:1, como Stablecoins (USDT, USDC, DAI, FDUSD) y Liquid Staking Derivatives (wstETH, rETH, cbETH). En HFT y Market Making institucional, asumir que 1 USDT = 1 USD todo el tiempo es un error mortal. Esta skill permite dos cosas: (A) Proteger el inventario convirtiendo todo a Fiat si una Stablecoin colapsa estilo Terra LUNA (UST), y (B) Cazar arbitrajes colosales explotando las recuperaciones del Peg mediante algoritmos de reversión a la media.

## 2. Nivel de conocimiento requerido
Experto en Microestructura de Stablecoins (Mecanismos de Mint/Burn colateralizados vs Algorítmicos), Contratos de Oráculos (Chainlink, Pyth Network, TWAP), y Trading de Paridad (Pairs Trading). Comprensión de las dinámicas del Pool 3pool de Curve Finance, dinámicas de Arbitraje de Canje L1 (Redemption Arbitrage), y Riesgo Sintético de Liquid Staking.

## 3. Capacidades principales
1. Monitorización de Paridad Continua (Peg Tracker): Chequear cruzando los Libros de Órdenes CEX (USDT/USD, USDC/USD) y los Pools on-chain (USDC/USDT Curve) si el anclaje base está en $1.00 +/- 0.05%.
2. Alerta de Caída en Cascada (De-Peg Cascade): Identificar en microsegundos si el USDC pierde la paridad (Ej. Marzo 2023 SVB Crisis, cuando USDC cayó a $0.88). Si sucede, el bot debe detener TODA operativa que asuma que USDC vale 1 Dólar de poder de compra.
3. Explotación de Desvíos Seguros (LSD Arbitrage): wstETH a veces cae a 0.99 ETH en DEXes ilíquidos pero está respaldado matemáticamente 1:1 en la red Beacon (Lido). El sistema dispara compras agresivas (Arbitraje L1 de canje a largo plazo), reteniendo el token para cobrar el "Free Yield" de 1%.
4. Análisis de Imbalance de Curve: Vigilar la métrica profunda de los "Balances del Pool". Si el 3Pool de Curve pasa de 33% / 33% / 33% a estar compuesto un 80% por USDT y 10% USDC/DAI, es una alerta probabilística extrema de que el mercado masivo está vendiendo (dumping) USDT en pánico, prediciendo el desvío antes de que ocurra en el precio.
5. Inactivación Dinámica de Rutas (Route Suppression): Si BUSD sufre de-peg estructural o es cancelado por el emisor (Paxos), esta skill avisa al Orquestador que marque todos los pares `X/BUSD` como radiactivos y prohíbe calcular spreads matemáticos cruzándolos contra USDT.
6. Auto-Conversión "Flight to Quality" (Vuelo a la Calidad): Si USDT baja del límite de dolor preconfigurado (Ej. $0.98 sostenido por 2 minutos), ejecutar una orden límite/market en todos los exchanges vendiendo el inventario completo de USDT por USD Fiat, Euro, o USDC para salvar el capital base.
7. Ajuste Matemático de Precios Base: Sustituye en el Algoritmo Principal de Arbitraje el supuesto de `1 USDT = 1 USD` por su precio spot real (`1 USDT = 0.998 USD`), refinando el cálculo de PnL y márgenes de slippage cruzado.
8. Monitorización del Contrato de Reserva: Lectura asincrónica del Proof of Reserves de Chainlink o el Mint/Burn emitido por las carteras oficiales del tesoro (Tether Treasury).
9. Arbitraje Inverso de Peg (Shorting the Premium): Si una stablecoin se vuelve ilíquida y salta al alza ($1.02) por exceso de demanda (Ej. un short squeeze), pedir préstamos Flash o en Aave de la moneda en premio y dumpearla en el mercado.
10. Protección de Colateral Perp/Margin: Si el CEX requiere colateral valuado a $1.00, pero la stablecoin está cayendo, el Margin Ratio se destruye (Liquidación en cascada). El sistema auto-transfiere otros colaterales (BTC) a la cuenta de futuros para compensar la caída del peso de la stable.

## 4. Entradas requeridas
- `dex_pool_reserves`: Balances de los contratos L1 de Stableswap (Skill 26).
- `oracle_price_feeds`: Chainlink feeds y oráculos de CEX directos.
- `inventory_exposure`: Qué cantidad del portfolio actual está sentada sobre la moneda que está sufriendo el ataque de paridad.

## 5. Salidas esperadas
- `peg_health_matrix`: Estado en tiempo real del anclaje de todos los tokens estables y LSDs del ecosistema.
- `depeg_panic_event`: Trigger que fuerza ventas u operaciones evasivas en el Orquestador maestro.
- `arbitrage_discount_signal`: Señal alcista si el token desviado representa un descuento irracional y seguro de comprar.

## 6. Reglas inmutables
- Nunca operar una compra en un "De-Peg" hacia abajo (Comprar algo a 0.90 esperando que suba a 1.00) si el token es Algorítmico Puro sin colateral sobrecolateralizado (Evita la "Caza de Cuchillos Cayendo" como UST/LUNA o IRON Titanium). Este Arbitraje "Value/Distressed" aplica SOLO a stables con colateral fiat auditable en cuentas de banco de USA (USDC) o protocolos sintéticos hiper-colateralizados (DAI) o respaldados por L1 real (wstETH/CBETH).
- La valoración Mark-to-Market de Skill 38 y 40 DEBE multiplicarse por el Factor de Peg (Peg Ratio). Si hay 1 Millón de TUSD, y el TUSD cotiza a $0.90, el NAV global reporta 900k, no 1 Millón. Asumir paridad matemática rígida distorsiona la gestión de riesgos en extremo.

## 7. Algoritmos o métodos que debe conocer
- Aritmética de Invariante de Stableswaps (Curve Invariant, Skill 26).
- Reversión a la media (Mean Reversion Statistics).
- Algoritmo de "Flight to Safety" (Vaciado masivo a liquidez profunda).

## 8. Fórmulas críticas
- **Peg Deviation Pct**: `Deviation = (Current_Price - Target_Peg) / Target_Peg * 100`
- **Curve Imbalance Ratio**: `Imbalance = Asset_Reserve / (Total_Pool_Liquidity / N_Coins)` (Normal = 1.0. Anormal = >1.5).
- **LSD Yield Arbitrage Discount**: `if (Price_LSD < (1 - Network_Yield_APY) * Peg) { Execute Arbitrage Accumulation }`

## 9. Casos extremos
- Depeg Masivo Institucional (Crisis bancaria Circle / Silicon Valley Bank): USDC pierde la paridad hasta $0.85 por pánico de insolvencia de custodios tradicionales. Todos los Spreads CEX/DEX se vuelven locos. La Skill 44 lee la caída, calcula que la exposición al colapso excede su umbral de tolerancia ($0.97), corta toda operativa normal de arbitraje y convierte a la fuerza todo el USDC local a ETH/BTC por seguridad "On-chain", asumiendo el spread pero evitando la ida a 0 teórica de una quiebra bancaria.
- Depeg Técnico de Oráculo L2 (Falso Positivo): El nodo de Pyth en Arbitrum arroja temporalmente que USDT = $0.50 debido a un ataque Flash Loan o congestión de su red de actualizadores de precio, pero en Binance sigue estando a $1.00 y en Curve Mainnet a $1.00. La Skill de Alertas debe buscar "Consenso" multi-fuente. Si 2 fuentes serias dicen $1.00, descarta la alerta del Oráculo roto protegiendo al bot de un ataque y evitando vender el portfolio entero a pérdida (Slippage trap).
- Premium Rate Arbitrage: Durante un bull market brutal, FDUSD puede saltar a $1.005 en Binance porque la gente lo necesita para farmear el Launchpool. La skill envía la orden de comprar USDT a 1.000, cambiarlo a FDUSD en OTC o un pool barato, y vender en Binance para obtener medio céntimo libre de riesgo millones de veces por hora.

## 10. Validaciones obligatorias
- PRE: Chequear la desviación del Peg usando un TWAP (Time-Weighted Average Price) para descartar caídas provocadas por alguien vendiendo a mercado por error (Slippage momentáneo) que se recupera en el siguiente bloque.
- CÁLCULO: Incorporar en tiempo real el Ratio de Redención oficial de los LSD (Ej. El contrato de Lido dictamina que hoy 1 wstETH equivale a 1.155 ETH. El bot debe apuntar a la paridad matemática de 1.155, no a 1.00).
- POST: Si se inicia el `Depeg_Panic_Event`, forzar la actualización de todas las colas de procesamiento de inventario para purgar el balance del token atacado lo más velozmente posible sin bloqueos concurrentes.

## 11. Criterios de aprobación
- La detección del depeg se logra combinando Order Book Bid/Ask Tracking (Skill 33) real de los pares directos vs USD o EUR en < 5ms desde la alteración.
- El sistema de Mapeo Interno de la moneda de valuación nunca miente, manteniendo el PnL sincero.

## 12. Criterios de rechazo
- El módulo se deja engañar por Oráculos de baja liquidez y lanza una Alerta Roja causando liquidación innecesaria y costosa (Pánico Injustificado).
- Incapacidad para leer y decodificar el Ratio de Acumulación (Accumulation Rate) de Tokens Yield-Bearing (wstETH/sDAI/RETH).

## 13. Riesgos que mitiga
- Riesgo de Contraparte Múltiple Centralizada: La ilusión de que las Stablecoins son "Dólares Físicos". Este bot retiene grandes bolsas de stables por fracciones de tiempo o como liquidez base; si Tether o Circle quiebran de un instante a otro, esta skill es la única línea de código que vaciará la billetera antes que el resto de los fondos mutuos del mercado y del pánico bancario global lo haga, asegurando ser el primero en cobrar los últimos verdaderos dólares de la piscina.
- Riesgos de Arbitraje Falsificado: Ver un arbitraje de BTC donde parece haber un gap del 2% a favor en Kucoin, pero descubrir que el mercado está en un Gap aparente porque el USDT de Kucoin vale un 2% menos que el USDT de Binance. Esta skill lo detecta e invalida el trade matemáticamente por asimetría de valuación base.

## 14. Integración con otras skills
- Proporciona factores multiplicadores dinámicos al Motor CEX-CEX y DEX-DEX (Skills 12 y 13) para normalizar valoraciones.
- Se comunica directamente con Risk Engine (Skill 41) para ejecutar el Halt y el Flight to Safety.

## 15. Modelo de datos sugerido
```json
{
  "PegHealthMonitor": {
    "asset": "USDC",
    "target_peg": 1.00,
    "current_price_twap": 0.9995,
    "deviation_pct": -0.05,
    "curve_3pool_imbalance": 1.02,
    "status": "HEALTHY_PEG",
    "action_recommended": "NONE",
    "discount_arb_target": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Monitor Estocástico asincrónico. Recibe actualizaciones (Event Emitter) desde el Skill 33 (Order Books) específicos de las duplas "Stables vs Fiat" u oráculos Chainlink en cada actualización L1.

## 17. Logs obligatorios
- `[DEBUG] Peg Tracker: wstETH Ratio is 1.155 ETH. Dex Price is 1.154. Deviation: 0.08% (Within arb threshold margin).`
- `[WARN] Curve 3Pool Imbalance Extreme: USDT comprises 78% of the pool. Highly probable selling pressure. Tuning down USDT exposure parameters.`
- `[CRITICAL] TUSD DE-PEG ALERT DETECTED. Price breached 0.98 USD threshold with high volume consensus. EMERGENCY CONVERSION TO USDC INITIATED FOR ALL HOLDINGS.`

## 18. Métricas obligatorias
- `average_peg_deviation_bps` (Para análisis de tendencias de desestabilización previas a la crisis).
- `stableswap_pool_imbalance_ratio`.
- `flight_to_safety_events_triggered_historical`.

## 19. Tests unitarios
- Falso Positivo TWAP: Alimentar al monitor de precios: `$1.00`, `$1.00`, `$0.20` (Una sola cotización errada), `$1.00`. La alarma de De-peg NO debe dispararse, ya que el TWAP o Media Mediana (Median Price) absorbe y aísla el error del dato aislado espurio.
- Imbalance Threshold (Curva Mágica): Proporcionar saldos: DAI: 1M, USDC: 1M, USDT: 8M. La matemática debe alertar `IMBALANCE > 75%` y emitir la orden de Riesgo Elevado para la moneda predominante.
- Ratio de Yield (LSD): Alimentar la lectura on-chain de un Staking Pool (RocketPool) devolviendo ratio 1:1.10. Alimentar DEX Precio `1:1.05`. El módulo debe detectar `Discount Arbitrage Valid` y pasarlo al optmizador.

## 20. Tests de integración
- Levantar servidor mock JSON RPC simulando un contrato Stableswap con un balance ultra desviado (Simular Hackeo L1). El oráculo L1 detecta el estado del bloque en tiempo real y desencadena el proceso de `Emergency Swap` de Skill 44, probando el ruteo interno de eventos de emergencia del Orquestador.

## 21. Tests E2E
- El ecosistema corre normalmente usando USDC como moneda base. De manera abrupta, se inyecta un mock price de Chainlink marcando que USDC = $0.94 por 10 bloques continuos. El Skill 44 intercepta la métrica, detiene todo el flujo HFT de la instancia, invoca un Flash Swap de mercado vendiendo a mercado todos los USDC por ETH (Para retener colateral volátil pero sólido off-chain) e instruye al Risk Engine (Skill 41) entrar en letargo (Sleep) manual. El Capital es salvado de irse a 0 en una espiral mortal.

## 22. Checklist de producción
- [ ] Incorporación de Alertas Humanas de Baja Frecuencia: Cuando una stable de-pega solo un 0.5% (Riesgo Amarillo), no liquidar, pero sí mandar SMS de urgencia al Portfolio Manager. (Ataques de pánico automatizados errados pueden ser destructivos financieramente).
- [ ] Reglas de Correlación Dura CEX/DEX: A veces el De-peg es un fallo del Exchange (Kraken falló en su par USDC/USD en 2021). Si el De-peg ocurre sólo en Kraken pero NO en Uniswap, es un Arbitraje Puro y Fuerte CEX-DEX (Ganancia Brutal), NO un riesgo sistémico bancario. La validación cruzada evita ventas de pánico erradas.
- [ ] Suscripción a Fuentes Off-chain Secundarias: Escanear menciones de la moneda objetivo (Twitter Sentiment/NLP sobre "Tether Hack") no es descabellado para institucionales si esto se combina con métricas de OrderBook para detectar humo antes de la caída algorítmica pura.

## 23. Ejemplo de configuración no hardcodeada
```yaml
peg_protection_engine:
  stables_monitored: ["USDT", "USDC", "DAI", "FDUSD", "TUSD"]
  lsds_monitored: ["wstETH", "rETH", "cbETH"]
  depeg_panic_threshold_pct: 2.0  # Sell everything if drop exceeds 2% continuously
  depeg_warning_threshold_pct: 0.5 # Pause trading and alert
  flight_to_quality_target_asset: "ETH" # Flee to ETH instead of fiat if bank system fails
  consensus_sources_required: 2
```

## 24. Ejemplo de pseudocódigo
```javascript
class PegSafetyMonitor {
    constructor() {
        this.historicalPrices = new Map(); // asset -> Array[prices]
    }

    async analyzePegHealth(asset, currentPricesMap) {
        // Median consensus from 3+ sources (e.g. Binance, Kraken, Uniswap V3)
        const prices = Object.values(currentPricesMap);
        const consensusPrice = calculateMedian(prices);
        
        // Push to TWAP series
        this.updateHistorical(asset, consensusPrice);
        const twap = calculateTWAP(this.historicalPrices.get(asset), 5 /* mins */);
        
        const targetPeg = await this.getDynamicTargetPeg(asset); // 1.0 for Stables, dynamic for LSDs
        
        const deviationPct = ((twap - targetPeg) / targetPeg) * 100;
        
        if (Math.abs(deviationPct) >= CONFIG.depeg_panic_threshold_pct && isStablecoin(asset)) {
            // Check if deviation is unilateral (e.g. only on one CEX, meaning arb opportunity) or universal (Collapse)
            if (isUniversalDepeg(currentPricesMap)) {
                this.initiateFlightToQuality(asset);
            }
        }
        
        return { status: "OK", twap, deviationPct };
    }
    
    initiateFlightToQuality(toxicAsset) {
        log.critical(`UNIVERSAL DEPEG ON ${toxicAsset}! Initiating liquidation to ${CONFIG.safe_asset}.`);
        EventBus.emit('PANIC_DUMP_INVENTORY', { from: toxicAsset, to: CONFIG.safe_asset });
    }
}
```

## 25. Criterio final de excelencia
El Rastreador de Pegs funciona como un sistema antibalístico (Iron Dome) que evalúa constantemente los ladrillos fundamentales de valoración (La ilusión del dólar). Impide que un algoritmo perfectamente programado sea engullido por el colapso de una variable económica externa subyacente que todos los demás asumían como inmutable, garantizando una resiliencia terminal del Agente HFT.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Illiquidez total durante "Flight to Quality" (Al tratar de vender la moneda venenosa, el libro de órdenes del exchange queda literalmente en vacío absoluto y la orden revierte sin importar qué precio aceptes).
- Dependencias: Order Book Tracking (Skill 33) o CEX Oracles directos.
- Próxima skill: Gestión dinámica de llaves API/Private keys (Skill 45).
