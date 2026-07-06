# SKILL 055 — Statistical Arbitrage (Pairs Trading / Co-integration)

## 1. Propósito superior
Desplegar una capa de "Arbitraje Estadístico" o "Reversión a la Media" (Mean Reversion). A diferencia del Arbitraje HFT puro (Donde el Spread es matemática de Suma Cero determinística intra-segundo), esta Skill busca dos activos que históricamente se mueven siempre en paralelo (Cointegración, ej. `BTC` vs `WBTC` temporal, `stETH` vs `ETH`, o `PEPE` vs `SHIB` - Altamente correlacionados). Cuando la correlación estadística "Se Rompe" y uno de ellos sube y el otro no (Z-Score > 2.0), el bot compra el que se quedó atrás (Lagging) y vende en corto el que subió (Leading), apostando a que, invariablemente, volverán a unirse como lo dicta la física del mercado en las próximas horas.

## 2. Nivel de conocimiento requerido
Quant Researcher (Estadística Avanzada y Econometría Cuantitativa). Dominio de Pruebas de Co-integración (Engle-Granger, Johansen Test), Stationarity Tests (Augmented Dickey-Fuller - ADF), OLS Regression (Ordinary Least Squares), Z-Score Modeling de Residuos, Análisis de Series Temporales (Time-Series) y Gestión Dinámica de Carteras de Largo Corto / Largo Fijo (Long/Short Delta Neutral Hedging).

## 3. Capacidades principales
1. Descubrimiento de Pares (Cointegration Scanner): Escanea asíncronamente el Data Lake (Skill 37) usando el historial completo de la semana pasada cruzando todos los activos de Layer 1 o Memecoins contra otros para detectar pares que co-integran al 99% (p-value < 0.01).
2. Cálculo de Residuos (Spread Modeling): Si PEPE y SHIB se mueven juntos en una proporción de "1 PEPE = 1500 SHIB", el bot crea un Activo Sintético llamado "Spread", restando el precio de A del precio de B proyectado.
3. Evaluación del Z-Score (The Trigger): Convierte el Spread puro en "Desviaciones Estándar" desde la Media. Si el Z-Score es `>= 2.5` (Divergencia brutal y rara - Anomaly), dispara el Trade asumiendo la reversión inminente.
4. Auto-Ejecución Delta-Neutral (Long/Short Muxing): No compra la moneda barata y se queda expuesto al riesgo del mercado (Beta). Ejecuta una compra en el Mercado Spot del Activo A (El Lagging), y simultáneamente abre un Short (Venta en Futuros Perpetuos) en el Activo B (El Overpriced), cancelando 100% el riesgo direccional si BTC cae a la mitad.
5. Hedge Ratio Calculation (Hedge Dinámico): Evita el error amateur de operar 1:1. Si Ethereum es doblemente volátil que Bitcoin, el bot sabe mediante el "Beta Residual de la Regresión" que debe comprar $100 de ETH y ShorTear $120 de BTC para anular el riesgo matemáticamente perfecto.
6. Gestión de Cierre Asíncrono (The Unwind): Una vez que el Z-Score retorna a `0.0` (o a un Take-Profit seguro como `0.5`), el Orquestador lanza órdenes Market/Limit cruzadas vendiendo el Long y Cerrando el Short a la vez, embolsando el Delta en Spread capturado.
7. Alerta de Quiebre Estructural (Structural Break Detection): Si un contrato es Hackeado o cambia de Tokenomics, los pares dejarán de cointegrar para siempre (Cointegration Break). El sistema usa Roll-Windows (Ventana móvil) de Pruebas ADF para detectar que la relación murió, ejecutando un `Stop-Loss` urgente (Z-Score > 5 o Parada de Vida Media).
8. Filtro de Half-Life Reversion: Usa la Ecuación de Ornstein-Uhlenbeck (OU Process) para calcular la "Vida Media" matemática del spread. Si tarda 5 días en volver a cero, el bot rechaza la estrategia porque el costo de Funding Rate (Comisiones de mantener el short) devorará el profit.
9. Cost-of-Carry Integrator: Resta del Profit Teórico el cobro por hora de Margin/Perps (Skill 16) para evaluar si aguantar el trade 12 horas saldrá rentable.
10. Oportunista de Correlación Intra-Clúster: Agrupa todos los "Exchange Tokens" (BNB, KCS, OKB, MX) y si uno de ellos diverge de la masa sin noticias propias, ataca el spread esperando la corrección sectorial (Sector Rotation Alpha).

## 4. Entradas requeridas
- `data_lake_time_series`: 5,000 puntos de datos de precios (Velas 1m o Tick Tiempos Regulares).
- `perpetual_funding_rates`: Telemetría actual de cupones de apalancamiento (Skill 16).
- `pair_whitelist_clusters`: Sugerencias de familias de activos a comparar (DeFi, L1s, GameFi).

## 5. Salidas esperadas
- `cointegrated_pairs_matrix`: Lista en RAM de pares aptos operativamente.
- `statistical_trade_signal`: Comandos al Orquestador (Ej: "Long LDO_USDT / Short RPL_USDT - Size: $5k").
- `z_score_live_telemetry`: Curva visualizable que rebota entre -3 y +3.

## 6. Reglas inmutables
- JAMÁS basar un Pairs Trade simplemente en "Alta Correlación" de Pearson. (El Error del Novato: "BTC y ETH tienen 0.95 de correlación"). Dos activos que se van a infinito tienen correlación casi 1, pero su *Spread Absoluto* no revierte a un valor estacionario. Se EXIGE matemáticamente usar el ADF Test de Estacionariedad sobre el Residuo.
- Todo Stat-Arb Long/Short requiere de Inyección de Stop-Loss Dinámico. El "Apalancamiento Infinito con la esperanza de volver a la media" causó la bancarrota de LTCM (Long-Term Capital Management en 1998 por el Rublo ruso). Si el Z-Score de la divergencia supera las `4.5` desviaciones, el Bot asume quiebre paramétrico y liquida con pérdidas para salvar el fondo general.
- Jamás cruzar Long/Short entre un activo Centralizado seguro (Spot Binance) y un Activo en un DEX exótico muy pequeño, el riesgo de Liquidación Asimétrica del colateral rompe la supuesta neutralidad (El Funding en el DEX puede saltar a 500% APR destruyendo tu short).

## 7. Algoritmos o métodos que debe conocer
- Augmented Dickey-Fuller (ADF) Test y Engle-Granger 2-Step.
- Ordinay Least Squares (OLS) Regression Method (Eigenvector analysis para Cointegración de Johansen si es Multi-Variable).
- Proceso Estocástico Ornstein-Uhlenbeck (Vida Media del Residuo).

## 8. Fórmulas críticas
- **Spread Residual (Log Space)**: `Residuo_t = Log(Price_A_t) - (Beta * Log(Price_B_t)) - Alfa`
- **Z-Score del Spread**: `Z = (Residuo_t - Media_Residuo_Ventana) / Desviacion_Std_Residuo`
- **Hedge Ratio (Capital Alloc)**: `$Position_A = Base_Size`, `$Position_B = Base_Size * Beta_Hedge`
- **Condición de Entrada Estricta**: `if (Z_Score >= 2.5 AND ADF_p_value < 0.05 AND Half_Life_Hours < 6) { EXECUTE }`

## 9. Casos extremos
- Hackeo L1 / Fork Network (Ruptura Catastrófica de Cointegración): El bot juega al rebote entre LUNA y ANC (Anchor Protocol), asumiendo siempre co-integración 99%. LUNA colapsa por una fuga técnica o inflación. El modelo Z-Score dice: "Z es -10, la mejor compra de la historia de la matemática". Si el bot compra LUNA y shortea ANC, la relación nunca regresa, se liquidan los fondos. El Filtro de Noticias Fundamentales / Halt Trading Filter se impone como Veto del Risk Engine (Skill 41).
- Liquidation Squeeze en el Short Leg (Short Squeeze): Shorteas un Token ilíquido creyendo que bajará para encontrarse con la media. Un fondo enloquece y sube el token 1000% en minutos. Tu posición Short en futuros explota (Liquidación). Tu Long en el otro par gana poco. Quedas arruinado (Margin Trap). Solución: Stop-Loss en Z-Score, Límite estricto a tokens Top-100 Alta Liquidez para Stat-Arb.

## 10. Validaciones obligatorias
- PRE: Chequear las comisiones acumulativas (Funding Rate + Slippage Spot/Perps). El profit esperado de la reversión suele ser del 1% al 3%. Si el funding es del -0.5% cada 8 horas y el bot espera 48 horas de vida media, la matemática indica un `Expected_Value < 0`, rechazando la orden antes de iniciar.
- CÁLCULO: Mapear el cálculo de Regresión Lineal OLS no con datos de hace un año, sino usando una "Ventana Dinámica Óptima" de media vida móvil (Ej. últimos 30 días, frecuencia horaria, adaptada exponencialmente).
- POST: Monitorización asíncrona permanente. Mientras el Arbitraje espacial dura 1 milisegundo, un Trade Estadístico queda colgado en memoria por horas o días (Position Management). El Orquestador necesita un gestor de estados persistentes para revivir si el servidor se reinicia, evitando perder la pista de los trades L/S asimétricos abiertos.

## 11. Criterios de aprobación
- Capacidad matemática local optimizada en C++ FFI / Rust (WASM) para calcular la Regresión Lineal y el ADF Test de 5,000 datos en menos de 50 milisegundos sin congelar (Thread-blocking) el Event Loop general del HFT.
- Curva de rendimiento aislada de pruebas Unitarias sobre el año pasado que muestra un comportamiento neutral o nula correlación contra el precio "Buy-and-Hold" del Bitcoin (Verdadera estrategia Alpha Independiente).

## 12. Criterios de rechazo
- El algoritmo usa la simple Correlación de Precios de Cierre (Close Prices) sin aplicar Rendimientos Logarítmicos o Test de Raíz Unitaria, construyendo carteras con series no estacionarias destinadas al quiebre de largo plazo.
- "Sizing" Fijo de posiciones 50% / 50% ignorando la Volatilidad Beta de cada pierna (El Activo A sube un 5%, el B sube un 1%. Si compraste $1000 de cada uno, tu delta neutro es un engaño).

## 13. Riesgos que mitiga
- Riesgo de Escasez de Oportunidades Estructurales (Dry-Spell Mitigation): A veces los mercados pasan meses sin liquidez o diferencias fuertes entre exchanges. Si el Bot solo depende de HFT (Arbitraje L1/L2 rápido), se morirá de hambre y quemará dinero en costos de servidor. El Arbitraje Estadístico florece en esos rangos lentos y calmos, actuando como la Batería Base que le da rentabilidad diaria a la firma cuantitativa en entornos sin turbulencias.

## 14. Integración con otras skills
- Extrae toda su información subyacente masiva del TSDB & Data Lake (Skill 37).
- Delega el Execution Proxy a Perpetual Funding (Skill 16) y Spot Routing (Skill 14).
- Consume telemetría de régimen de mercado de Hidden Markov Models (Skill 48) para activar la caza.

## 15. Modelo de datos sugerido
```json
{
  "StatArbOpportunity": {
    "pair_a": "LDO_USDT",
    "pair_b": "RPL_USDT",
    "timestamp_ms": 1714521234105,
    "adf_p_value": 0.003, // Highly Cointegrated
    "half_life_hours": 12.5,
    "current_z_score": 3.14, // Extreme divergence observed
    "hedge_ratio_beta": 1.45, // Sell $1.45 of B for every $1.00 of A bought
    "recommended_action": {
      "long": { "asset": "LDO_USDT", "market": "SPOT", "usd_amount": 10000 },
      "short": { "asset": "RPL_USDT", "market": "PERPETUAL", "usd_amount": 14500 }
    },
    "stop_loss_z_score": 5.0,
    "take_profit_z_score": 0.5
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Worker Asíncrono de Baja Frecuencia. Ejecuta un "Barrido" matricial O(N^2) sobre todos los activos cada hora o cada minuto (No requiere velocidad de microsegundos, al buscar reversiones de horas). Usa un proceso Node Child o FFI Thread.

## 17. Logs obligatorios
- `[INFO] Stat-Arb Scanner: 15 pairs found co-integrating with p < 0.05 and HL < 24h.`
- `[DEBUG] Pair ETH/wstETH divergence extreme. Z-Score = 2.85. Firing Delta-Neutral Long/Short leg sequence to Orchestrator.`
- `[CRITICAL] Z-Score Breached 5.0 Threshold (Structural Break) on AAVE/COMP pair! Assuming Cointegration Failure. Hard Liquidating Long/Short positions with $45.00 realized loss.`

## 18. Métricas obligatorias
- `stat_arb_open_positions_count`.
- `average_mean_reversion_time_hours` (Si difiere enormemente de la Half-Life predicha, la matemática local está asumiendo falsos parámetros).
- `accumulated_funding_fees_paid_usd` (Monitoreo de fricciones de apalancamiento).

## 19. Tests unitarios
- Engle-Granger Cointegration Math: Pasar una matriz JSON de 2 vectores de datos falsos de Random Walk (Paseo aleatorio). El script OLS + ADF DEBE escupir un `p-value > 0.10` y clasificarlo como `NOT_COINTEGRATED`. Llenar con dos matrices atadas a una ecuación `Y = 2X + Ruido`. El Script DEBE escupir `p-value < 0.01` y detectar la relación matemática en la neblina con error < 0.001.
- Hedge Ratio Balancing: Asignar un `Beta = 2.5` entre dos tokens. Ejecutar el Allocator. Si pide Invertir $1,000 Total, debe asignar Long: $285 y Short: $715 manteniendo el equilibrio financiero direccional Cero ($285 * 2.5 = $712.5).
- Stop-Loss Discipline (Z-Score Escalation): Simular un Trade vivo. El array de precios entra y el Z-Score pasa a 2.5 (Apertura de posición). Luego el array de precios se descarrila. El Z-Score toca 3, 4 y llega a 5.1. El motor de Monitoreo debe disparar incondicionalmente el Callback de Liquidación de Emergencia (Skill 41) matando ambas piernas, aceptando el "Stop Loss Estadístico" (Hard Break).

## 20. Tests de integración
- Conexión al Data Lake (InfluxDB/TimescaleDB). Configurar el SQL/Flux query engine para calcular las medias móviles de 30 días de 100 activos. Verificar la inyección sin latencia del vector de datos masivo al Motor Matemático en Rust local usando puentes IPC o Shared Array Buffers para no colapsar la RAM de la VM principal.

## 21. Tests E2E
- El bot monitorea el Clúster de Activos "Liquid Staking Derivatives" (Lido, RocketPool, Frax). Descubre que LDO y RPL siempre se mueven 98% a la par por estar atados a la misma métrica del protocolo base L1. De repente, una ballena vende violentamente RPL para irse de vacaciones, hiriendo el precio y creando un Z-Score de `-3.0` (RPL Lagging). El bot detecta que no hay noticias estructurales fundamentales (Hack), que justifica la anomalía. Emite un Long Spot a RPL y un Short Perpetuo a LDO usando la Skill 36 de ruteo y usando $5,000. El ratio Hedging Beta lo cubre de que si todo Cripto cae, la pérdida del Long se compense con ganancia del Short. Siete horas más tarde, el mercado vuelve a valorar a RPL, restaurando el ratio lógico. El Z-Score llega a `0.0`. El sistema vende todo, cancela el Hedge, y retiene un beneficio compuesto limpio de 2.5% sin depender de los movimientos macroeconómicos del Bitcoin, probando el Alpha Alpha (Retorno Absoluto Descorrelacionado) de la firma.

## 22. Checklist de producción
- [ ] Incorporación de Análisis de Componentes Principales (PCA - Principal Component Analysis): En vez de pares binarios, crear estrategias Multi-Cesta (Long a 5 Monedas, Short a otras 5) para aislar mejor el Riesgo Idiosincrático y crear Portafolios Estadísticos a nivel "Quant Fund".
- [ ] Integración con Factor Investing o Smart Beta: Usar datos como "Volume Flow" o "Orderbook Imbalance" como variables dentro del Modelo de Regresión para mejorar el poder predictivo del Half-Life (Vida Media de Reversión), apoyado fuertemente por Machine Learning (Skill 47).
- [ ] Filtro de Noticia vs Fluctuación de Liquidez. Si un protocolo acaba de anunciar Bancarrota y el Z-score explota a 10, no es una "oportunidad de revertir". Es la muerte permanente del activo. Idealmente usar Filtros de NLP de Noticias o un "Hard Stop en Volumen" para no quedar atrapados "Haciendo Hedging" a activos muertos (LUNA Death Spiral).

## 23. Ejemplo de configuración no hardcodeada
```yaml
statistical_arbitrage_engine:
  enable_pairs_trading: true
  analysis_window_days: 30
  recalculation_cron_minutes: 60
  z_score_trigger_entry: 2.5
  z_score_trigger_exit: 0.5
  z_score_stop_loss_hard: 5.0
  max_half_life_hours_accepted: 72 # 3 days max wait time
  minimum_adf_confidence: 0.95 # P-value < 0.05
  capital_allocation_per_pair_usd: 10000.0
```

## 24. Ejemplo de pseudocódigo
```javascript
class StatisticalArbitrageEngine {
    constructor(timeSeriesDb) {
        this.db = timeSeriesDb;
        this.activeStatPositions = new Map();
        this.mathematics = new AdvancedQuantFFI(); // Bindings a C++ para velocidad
    }

    // Cron job running every 1 hour
    async seekCointegratedPairs(targetClusters) {
        for (let cluster of targetClusters) {
            const historyMatrix = await this.db.fetchClusterHistoricalLogReturns(cluster, 30 /*days*/);
            
            // O(N^2) pairwise calculation using C++ compiled library for instant CPU resolution
            const results = this.mathematics.findCointegratedPairs(historyMatrix);
            
            for (let res of results) {
                if (res.pValue < 0.05 && res.halfLife < CONFIG.max_half_life_hours) {
                    await this.evaluateLiveZScoreExecution(res);
                }
            }
        }
    }

    async evaluateLiveZScoreExecution(pairModel) {
        // Fetch real-time tick right now
        const currentPrices = await MarketData.getLatest(pairModel.assetA, pairModel.assetB);
        const zScore = this.mathematics.calculateLiveZScore(currentPrices, pairModel.regressionParams);
        
        if (Math.abs(zScore) >= CONFIG.z_score_trigger_entry) {
            this.dispatchHedgedTrade(pairModel, currentPrices, zScore);
        }
    }

    dispatchHedgedTrade(model, prices, currentZ) {
        log.critical(`Z-Score Explosion Detected: ${model.assetA} vs ${model.assetB} (Z = ${currentZ}). Firing Delta-Neutral Long/Short Pair.`);
        // Dispatch to Orchestrator (Skill 36) managing Spot + Futures cross margin integration
        const hedgeRatio = model.regressionParams.beta;
        const totalSize = CONFIG.capital_allocation_per_pair_usd;
        
        // ... (Send Order Execution Command maintaining neutral exposure)
    }
}
```

## 25. Criterio final de excelencia
El Motor de Arbitraje Estadístico rompe el esquema del "Scalper Ultra Rápido y Ansioso" e inyecta la mentalidad fría y letal de un "Hedge Fund Cuantitativo Wall-Street" al Agente. Le da la capacidad de exprimir ineficiencias matemáticas sutiles durante mercados soporíferos (aburridos), gestionando carteras complejas Long/Short perfectamente balanceadas, convirtiéndolo en un cazador omnipotente independientemente de si el mercado viaja en milisegundos de latencia o semanas de deriva temporal.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Costo de Mantenimiento ("Bleeding to death by Funding Rates" / Muerte por tarifas de financiación de CEX). Estar Short 3 días esperando la reversión puede costar dinero en Swaps que destrocen el PnL neto si el cálculo de Half-Life falla. (Solucionado con integradores estocásticos).
- Dependencias: TSDB Mass Data Extraction (Skill 37) y Cross-Margin API Access en CEXes.
- Próxima skill: Graph Neural Networks para predicción de flujos (Skill 56).
