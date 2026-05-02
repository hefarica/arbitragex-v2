# SKILL 046 — Orquestador de simulaciones y Backtester interno

## 1. Propósito superior
Proporcionar un "Simulador de Realidad" (Sandbox) donde el Agente HFT pueda viajar al pasado. Esta skill carga el historial masivo de OrderBooks y Trades almacenado en el Data Lake (Skill 37), y hace "Replay" (Reproducción) de los datos a velocidad extrema (ej. x10,000 veces la velocidad real) para evaluar si una nueva estrategia matemática ganaría o perdería dinero. El backtester interno es el árbitro final de la verdad teórica antes de pasar una estrategia a producción (Paper Trading o Live Trading).

## 2. Nivel de conocimiento requerido
Ingeniero Cuantitativo (Quant), Experto en Time-Series Simulation, C/C++ FFI o Rust Optimization para procesar Gigabytes de datos por segundo. Dominio profundo del Modelado de Latencia (Latency Modeling), Impacto de Slippage Simulados, y evitación de los "Pecados del Backtesting" (Look-ahead bias, Survivorship bias, Overfitting).

## 3. Capacidades principales
1. Event Replay Engine: Reproducir los Deltas del OrderBook con la misma secuencia estricta en que ocurrieron en la realidad, emitiendo eventos simulados como si fuesen WebSockets en vivo.
2. Order Matching Engine (Simulado): Si el bot en backtest manda una orden `Buy 10 BTC Limit 65000`, el simulador lee los ticks siguientes; si el precio real cruzó `65000` en la historia, la orden se da por "Llenada" (Filled).
3. Inyección de Latencia Estocástica: El bot nunca ejecuta en 0ms. El backtester inyecta retrasos artificiales aleatorios (ej. 15ms - 50ms según datos históricos de la Skill 34) a las órdenes de simulación. Si la oportunidad desapareció durante esos 30ms artificiales, el trade falla en el backtest (Evitando curvas de PnL mágicas).
4. Market Impact / Slippage Simulator: Si el bot decide comprar 50 BTC en el backtest contra un nivel que solo tenía 5 BTC en la realidad, el simulador debe "Barrar el libro" de forma realista empujando el precio contra el bot.
5. Control de Look-ahead Bias: Arquitectura impenetrable que asegura que el algoritmo de Arbitraje que está siendo probado NUNCA tenga acceso al "Vector de precios futuros" del array, sólo al índice actual.
6. Multi-Venue Sync: Sincronizar el replay de Binance, Kraken y Uniswap usando el mismo reloj temporal de nanosegundos (Merge Sort de eventos temporales).
7. Fee Deduction Emulation: Descontar de cada operación emulada las comisiones Taker/Maker extraídas de la Skill 39.
8. Parallel Parameter Grid Search: Ejecutar 1,000 simulaciones simultáneas cambiando pequeñas variables (ej. Mínimo spread 0.1%, 0.15%, 0.20%) usando todos los núcleos del CPU para encontrar el "Punto Dulce" matemático.
9. Reporte de Desgarro (Tear Sheet Generation): Producir un informe final con métricas profesionales: Sharpe Ratio, Sortino Ratio, Max Drawdown, Win Rate, Profit Factor, y Calmar Ratio.
10. Transición "Sim-to-Real": La estructura de código de una estrategia debe ser literalmente la misma (100% homóloga) tanto si la llama el Orquestador Real (Skill 36) como si la llama el Backtester, sin necesidad de reprogramarla.

## 4. Entradas requeridas
- `historical_data_batches`: Bloques de datos extraídos de la Skill 37 (Data Lake).
- `strategy_module`: Lógica del algoritmo que se va a testear.
- `simulation_parameters`: Latencia media, Capital Inicial, Fee fijo, Rango de fechas.

## 5. Salidas esperadas
- `simulated_pnl_curve`: Gráfica / Array de balance a lo largo del tiempo emulado.
- `trade_log`: Registro detallado de los miles de trades simulados ejecutados.
- `quant_metrics_report`: Ratios institucionales (Sharpe, Max Drawdown).

## 6. Reglas inmutables
- JAMÁS realizar un backtest usando únicamente precios de "Velas" (Klines OHLCV). El Arbitraje HFT no ocurre en velas de 1 minuto, ocurre en microsegundos dentro de la vela. El backtest sólo es válido si corre sobre datos Level-2 (L2 OrderBook) u operaciones (Tick-by-Tick).
- Penalización Obligatoria de "Phantom Liquidity": Asumir probabilísticamente que el 20% del volumen del nivel top del libro es falso o desaparecerá antes de que el bot lo golpee.
- Si el backtest muestra una curva logarítmica perfecta sin ninguna caída, asume automáticamente que el simulador tiene un bug de "Look-ahead bias" o que la latencia está configurada en 0ms. (En el mundo real no hay curvas perfectas).

## 7. Algoritmos o métodos que debe conocer
- K-Way Merge Algorithm (Para alinear N archivos de series de tiempo simultáneamente).
- Monte Carlo Simulations (Para inyectar ruido a la latencia de red y probar la robustez de la estrategia).
- Métricas Financieras Modernas (Kelly Criterion para sizing).

## 8. Fórmulas críticas
- **Sharpe Ratio Anualizado**: `(Mean_Return_Daily / Std_Dev_Daily) * Sqrt(365)`
- **Impacto de Precio Simulado**: `Avg_Execution_Price = Sum(Volume_i * Price_i) / Total_Volume_Target` (Recorriendo el snapshot del libro emulado).
- **Latencia de Rechazo**: `if (Time_Of_Opp_Disappearance < Time_Of_Opp_Detection + Simulated_RTT) { Register_Failed_Trade() }`

## 9. Casos extremos
- Sesgo de Supervivencia (Survivorship Bias): Testear una estrategia hoy sobre un exchange que incluye a LUNA. El bot gana dinero porque LUNA "sobrevivió" hasta cierto punto, pero ignora decenas de tokens que fueron deslistados (Delisted) durante ese año. El motor debe incluir activos muertos para ser honesto.
- Out of Memory (OOM) en Replay: Cargar 1 año de Ticks L2 de Binance consume ~500 GB de RAM. El simulador debe hacer Streaming del disco duro (Event streams) cargando en buffers de 1GB sin bloquear la memoria, haciendo streaming directo al motor matemático.
- Latency Arbitrage Mirage (El Espejismo de Latencia): El backtester asume que fue el primero en tomar la oportunidad de 1ms de Binance-OKX. En la realidad, Jump Trading o Wintermute tienen una conexión de fibra dedicada y siempre llegarán 100 microsegundos antes. El simulador debe aplicar un "Fill Probability Ratio" (Ej. Solo el 30% de las oportunidades ultrarrápidas se concretan).

## 10. Validaciones obligatorias
- PRE: Asegurar que los sets de datos no tengan "Agujeros" temporales (Gaps) mayores a 1 segundo. Si faltan datos en el Lake, el backtest es "Contaminado" y no concluyente.
- CÁLCULO: Validar el Matching Parcial. Si el bot pide comprar 10 y el orderbook histórico tiene 4, el bot recibe 4 (Partial Fill), paga su fee de 4, y el resto de la orden queda colgando o es cancelada (FOK - Fill or Kill).
- POST: Realizar "Out-of-Sample Testing" (Prueba fuera de muestra). Si el bot se ajusta perfecto a Enero-Marzo, ejecutar la simulación sobre Abril. Si en Abril pierde, el bot está sobreajustado (Overfitted) y se rechaza su paso a producción.

## 11. Criterios de aprobación
- El tiempo de simulación para un mes de datos de ticks no supera los 10 minutos de tiempo real computacional.
- El Sharpe Ratio sobre "Out-of-Sample data" (datos no vistos durante el ajuste de parámetros) es > 2.0 (Retorno Ajustado al Riesgo Institucional).

## 12. Criterios de rechazo
- El Backtest asume ejecución contra el precio "Mid-Price" (Media entre Bid y Ask) en lugar de cruzar el spread (Pagar el Ask para comprar, golpear el Bid para vender). Este es el error #1 de desarrolladores novatos y genera fortunas ilusorias.
- El Win-Rate (Tasa de Victoria) excede el 99.5%, lo que señala un bug matemático interno del simulador (Normal en HFT es 60-80% con estricto Risk/Reward).

## 13. Riesgos que mitiga
- La Ilusión del Programador (Confirmation Bias): Creer que has descubierto una máquina de imprimir dinero infinita basándote en fórmulas teóricas. Este backtester actúa como el "Destructor de Sueños" necesario que revela los costos logísticos, el slippage, los rechazos de red y el spread cruzado, filtrando la fantasía de la realidad financiera.
- Quemar fondos reales en Pruebas en Vivo: Impide el testing de algoritmos HFT directamente con dinero real.

## 14. Integración con otras skills
- Reemplaza temporalmente a la capa de WebSockets (Skill 31) durante la prueba, inyectando la data histórica directamente al Normalizador (Skill 32) u Orquestador (Skill 36).
- Lee de Data Lake & TSDB (Skill 37).

## 15. Modelo de datos sugerido
```json
{
  "BacktestReport": {
    "strategy_name": "Triangular_Arb_V2",
    "dataset_range": "2024-01-01 to 2024-03-01",
    "initial_capital_usd": 10000.0,
    "final_capital_usd": 14250.0,
    "total_trades": 145200,
    "win_rate_pct": 68.4,
    "sharpe_ratio": 3.12,
    "max_drawdown_pct": -0.85,
    "simulated_latency_ms": 30.0,
    "assumed_slippage_penalty_bps": 2.0
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Interfaz CLI (Línea de Comandos) aislada (`arbitragex test --strategy=Triangular --start=... --end=... --lat=25ms`). Evita correr el backtester en el mismo hilo/servidor de producción en vivo.

## 17. Logs obligatorios
- `[INFO] Backtest Initializing. Loading 1.4 Billion events from DataLake.`
- `[DEBUG] Simulated Order #40102 FAILED. Opportunity lasted 12ms, simulated latency was 20ms.`
- `[CRITICAL] Out-of-Sample Evaluation Failed. Strategy is curve-fitted (Overfitted). Rejecting push to production.`

## 18. Métricas obligatorias
- `simulation_speed_events_per_sec` (Métrica de rendimiento del procesador local).
- `calmar_ratio` (Annualized Return / Max Drawdown).
- `profit_factor` (Gross Profit / Gross Loss).

## 19. Tests unitarios
- Look-Ahead Bias Trap: Pasar a una estrategia sencilla un mock de precios. Validar a nivel compilador o runtime que la estrategia NO puede leer `prices[current_index + 1]`.
- Partial Fill Logic: Enviar orden Market Buy de 100 unidades a un libro simulado que tiene 3 niveles de 30 unidades cada uno (Total 90). El simulador debe devolver "Partial Fill 90 units" y calcular un Precio Promedio que escale subiendo por los 3 niveles, sin darle el precio base.
- Latency Probability: Pasar un array temporal donde un spread se abre en el ms 10 y se cierra en el ms 30. Ejecutar la estrategia con latencia de 15ms. El bot debe atrapar el spread con una ventana de 5ms extra de sobra (Simulated Success).

## 20. Tests de integración
- Levantar el pipeline completo. Conectar la Skill de Arbitraje CEX-CEX al Backtester en lugar de a Binance. Hacer correr un Dataset Oficial que contiene el Crash del SVB (Marzo 2023). Verificar si las alertas de Pegs (Skill 44) y Risk Engine (Skill 41) se activan correctamente en el entorno de simulación como lo harían en el mundo real.

## 21. Tests E2E
- El "Grid Search" (Optimizador de Parámetros) arranca un viernes por la noche en un servidor dedicado de 64 núcleos. Carga todas las combinaciones posibles de "Target Profit", "Slippage Tolerance" y "Maximum Drawdown". Tras iterar 50,000 variaciones sobre el año 2023 completo de Orderbooks L2 masivos, escupe el reporte el sábado en la mañana: "Combinación óptima: Threshold 0.12%, Latency tolerance: 40ms, Win rate 71%".

## 22. Checklist de producción
- [ ] Incorporación de Tasas Libres de Riesgo (Risk-Free Rate): Para calcular el Sharpe Ratio real, el bot debe restar el rendimiento base de los bonos del tesoro (T-Bills) o de plataformas DeFi ultra seguras (Maker DSR ~5%). Si el bot en backtest rinde 4% al año, el Sharpe es negativo, porque es mejor no encender el bot y poner el dinero a plazo fijo.
- [ ] Exclusión de Tiempos de Inactividad (Downtime Culling): Binance tiene mantenimientos periódicos. El simulador debe ignorar los vacíos en lugar de asumir que la liquidez quedó congelada horas permitiendo falsos arbitrajes.
- [ ] Habilitación de Profiler (Flamegraphs): El Backtester correrá la lógica matemática mil millones de veces en horas. Cualquier ineficiencia (una función `String.split()` innecesaria) multiplicada por mil millones colapsará la prueba; optimizar a muerte el código.

## 23. Ejemplo de configuración no hardcodeada
```yaml
backtest_engine:
  base_latency_penalty_ms: 15
  stochastic_jitter_ms: 5
  assumed_slippage_bps: 1.5
  maker_fee_override_pct: 0.00
  taker_fee_override_pct: 0.04
  fill_probability_factor_pct: 70 # In competitive ticks, we only win 70% of the races
```

## 24. Ejemplo de pseudocódigo
```javascript
class BacktestSimulator {
    constructor(strategy, dataStream) {
        this.strategy = strategy;
        this.dataStream = dataStream;
        this.pnlTracker = new PnLLedger();
        this.currentTimeNs = 0;
    }

    async run() {
        while(await this.dataStream.hasNextBatch()) {
            const batch = await this.dataStream.getNextBatch(); // 1,000,000 events
            
            for (let event of batch) {
                this.currentTimeNs = event.timestamp_ns;
                
                // Keep shadow orderbook updated
                OrderBookEngine.processMessage(event);
                
                // Allow Strategy to React
                const orderAction = this.strategy.onMarketUpdate(event);
                
                if (orderAction) {
                    this.simulateExecution(orderAction);
                }
            }
        }
        return this.pnlTracker.generateReport();
    }

    simulateExecution(order) {
        // 1. Add Artificial Latency
        const simulatedExecutionTimeNs = this.currentTimeNs + (CONFIG.latency_penalty_ms * 1_000_000);
        
        // 2. Peek into the future data buffer at simulatedExecutionTimeNs to see if liquidity still exists
        const futureBookState = this.dataStream.peekAtTime(simulatedExecutionTimeNs, order.pair);
        
        // 3. Match against the FUTURE book, not the current book
        const fillResult = this.matchOrderAgainstBook(order, futureBookState);
        
        // 4. Record result
        if (fillResult.success) {
            this.pnlTracker.registerWin(fillResult.profitNetMinusFees);
        } else {
            this.pnlTracker.registerLoss(fillResult.gasOrFeeWasted);
        }
    }
}
```

## 25. Criterio final de excelencia
El Backtester Interno es el "Campo de Pruebas de Armas" del agente. Erradica la programación emocional o las corazonadas ("Creo que este spread es rentable") y las reemplaza con veredictos empíricos, matemáticos e ineludibles. Si un algoritmo no puede sobrevivir a este simulador con slippage y latencia estocástica, no tiene el privilegio de tocar la Mainnet.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Overfitting Múltiple Computacional (Probar miles de parámetros hasta que el simulador por pura casualidad encuentra una curva perfecta al pasado, pero inútil al futuro).
- Dependencias: Data Lake (Skill 37).
- Próxima skill: Generador de señales ML (Random Forest/XGBoost) (Skill 47).
