# SKILL 048 — Modelos de Hidden Markov para regímenes

## 1. Propósito superior
Detectar y clasificar de forma autónoma el "Estado Macro-Físico" (Régimen) en el que se encuentra el mercado en tiempo real. Un algoritmo de arbitraje optimizado para rangos lentos será liquidado si el mercado entra en un régimen de altísima volatilidad direccional (Flash Crash/Pump). Utilizando los Modelos Ocultos de Markov (HMM), esta skill es un "Termostato Ambiental" que clasifica el mercado actual (Ej. "Tendencia Alcista Volátil", "Rango Lateral Muerto", "Pánico Bajista") y ajusta dinámicamente todos los parámetros de las demás skills para adaptarse y sobrevivir al nuevo entorno de forma probabilística.

## 2. Nivel de conocimiento requerido
Científico de Datos Cuantitativos (Quant Researcher). Dominio matemático de Cadenas de Markov, Probabilidad de Transición de Estados, Algoritmo Expectation-Maximization (Baum-Welch), Algoritmo de Viterbi, e inferencia bayesiana en ventanas móviles (Rolling Windows). Comprensión de Dinámica de Sistemas Complejos y Teoría del Caos en Mercados Financieros.

## 3. Capacidades principales
1. Inferencia Continua del Régimen (Online State Decoding): Recibir retornos logarítmicos del precio de BTC o de la volatilidad cada N segundos y correr el Algoritmo Forward-Backward/Viterbi para responder: "¿En qué estado macro estamos ahora mismo con un 95% de confianza?".
2. Mapeo Histórico: Carga un modelo matemático (Entrenado previamente en el Data Lake - Skill 37) que define, por ejemplo, 3 Estados Ocultos latentes del mercado basados en volatilidad histórica.
3. Transición Dinámica de Parámetros de Trading: Si el Modelo HMM pasa del `Régimen 1 (Calma)` al `Régimen 3 (Pánico Altísima Volatilidad)`, este módulo fuerza al Risk Engine (Skill 41) a ampliar la tolerancia de Slippage permitida del bot, y recorta el tamaño de Trade por defecto un 50% para reducir exposición.
4. Prevención de Ruido (State Thrashing Protection): Evita "Pestañear" entre regímenes. Si un milisegundo salta a "Volátil" y al segundo siguiente a "Calma", el HMM requiere una inercia de transición (Matriz de Transición de Markov) para estabilizar la clasificación y no enloquecer a la red paramétrica del bot.
5. Inactivación de Estrategias: Saber que la Estrategia "Funding Rate Arbitrage" (Skill 16) es inútil en un Régimen de "Baja Volatilidad / Bear Market Lateral" y pausarla automáticamente ahorrando llamadas API y procesador.
6. Aislamiento Independiente de Activos (Asset Decoupling): Puede detectar que Bitcoin está en `Régimen Calmo`, pero PEPE está en `Régimen Extremo`. Aplica los ajustes restrictivos exclusivamente a las rutas matemáticas que toquen PEPE, dejando operar con tamaño máximo a BTC.
7. Alerta Temprana Probabilística: Identificar sutiles aumentos en el "volumen de ticks perdidos o gap sizes" que indican probabilísticamente un inminente salto de régimen hacia la volatilidad antes de que el precio colapse en los gráficos estándar.
8. Calibración Continua Liviana: Enviar métricas históricas locales a un worker de baja prioridad para re-calcular levemente las medias y varianzas de los Modelos Gaussianos Emission Matrices (Matriz de Emisión de Markov) semana a semana.
9. Reducción de Tasa de Refresco: En "Rango Lento" puede decidir que el Orquestador maestro evalúe grafos triangulares cada 5 segundos en lugar de cada 5 milisegundos, ahorrando gigantescos costos de cómputo en AWS/GCP (Dinero real de Cloud computing).

## 4. Entradas requeridas
- `returns_stream`: Serie temporal reciente de Log-Rendimientos y Variaciones de Spread extraída de Websockets o del Time-Series (Skill 37).
- `hmm_parameters`: Archivo JSON configuracional que contiene las Matrices pre-entrenadas: Matriz de Transición de Estados, Medias (Means) y Matrices de Covarianza.
- `current_timeframe_window`: Un array flotante (Ej. últimos 100 ticks).

## 5. Salidas esperadas
- `current_market_regime`: Etiqueta entera o descriptiva (`0`, `1`, `2` - "CALM", "CHOPPY", "VOLATILE").
- `regime_transition_event`: Alerta que fuerza a las skills satélite a recargar parámetros (Hot-Swap configs).
- `state_confidence_matrix`: Array probabilístico (Ej. `[0.05, 0.10, 0.85]`).

## 6. Reglas inmutables
- JAMÁS realizar inferencias de HMM y cálculo matricial profundo (Multiplicación de matrices O(N^3)) dentro del hilo central de ejecuciones HFT (Event Loop Principal). Estas matemáticas densas corren en un hilo paralelo de Worker o proceso nativo C/Rust asíncrono, que sólo devuelve un "Tag de Etiqueta" ligero al orquestador.
- Las definiciones numéricas de qué constituye cada régimen deben basarse en distribuciones de Mezcla Gausiana (GMM), no en un umbral estático y humano y arbitrario ("Si sube 5%, es volátil"). Los mercados mutan; el HMM se adapta a las estadísticas matemáticas subyacentes.
- Si el HMM decodifica un Régimen `Desconocido/Ruido` o las probabilidades están dispersas `[0.33, 0.33, 0.33]` indicando incertidumbre total, forzar al sistema a adoptar el Régimen más CONSERVADOR y Defensivo predeterminado para proteger el capital.

## 7. Algoritmos o métodos que debe conocer
- Hidden Markov Models (Viterbi Decoding, Forward algorithm).
- Cadenas de Markov (Probability Transition Matrix).
- Multivariate Gaussian Distribution Probability Density Function (PDF).

## 8. Fórmulas críticas
- **Cálculo de Transición Estocástica**: `P(State_t | State_t-1) * P(Observation_t | State_t)` (Ecuación núcleo de Markov).
- **Log-Returns Smoothing**: `Return = Ln(Price_t / Price_t-1)` (Obligatorio para normalizar precios nominales).
- **Control de Adaptación**: `Dynamic_Spread_Threshold = Base_Spread * Regime_Multiplier[Current_State]`

## 9. Casos extremos
- Shock de Régimen Bi-Direccional Rápido (Whipsaw Extreme): Datos de inflación (CPI) se publican. El mercado dispara 10% arriba y luego 10% abajo en 5 segundos. Un clasificador estático se vuelve loco cambiando configuraciones 5 veces por segundo, desajustando todo el sistema en cada cambio de contexto. El HMM previene esto porque la inercia natural de la "Matriz de Transición" exige peso estadístico constante antes de formalizar una declaración de Régimen 3.
- Degradación Estadística (Degenerate Matrices): Tras un largo periodo de bajísima volatilidad histórica (Mercado lateral de meses), las varianzas gaussianas se vuelven diminutas. Cualquier movimiento diminuto (Ej. un salto de $10 en BTC) dispara falsos positivos de "Régimen Pánico". (Controlado aplicando límites de Varianza Mínimos - Variance flooring).

## 10. Validaciones obligatorias
- PRE: Asegurar que el buffer del `Rolling Window` contenga muestras completas y suficientes. Intentar inferir régimen con 3 ticks escupe ruido puro ("Not Enough Observations").
- CÁLCULO: Validar la suma de las probabilidades. El array `[P(S1), P(S2), P(S3)]` debe sumar estricta y matemáticamente `1.0`. Si suma `NaN` (Underflow/Overflow por multiplicación infinita de ceros comunes en HMM largos), aplicar el Log-Sum-Exp Trick para evitar crasheos de la computadora.
- POST: Al detectarse un cambio a Régimen Volátil, notificar con severidad Crítica por el bus de eventos (Event Bus) garantizando que el Risk Engine (Skill 41) levante sus escudos un microsegundo antes de que un spread tóxico impacte.

## 11. Criterios de aprobación
- Decodificación del Viterbi Path (Ventana de 50 observaciones) resolviéndose en < 5 milisegundos en el hilo secundario local de C/Rust o V8/WebAssembly (WASM).
- El motor clasifica eficientemente la quietud del mercado asiático vs la hiper-actividad de la apertura de Wall Street de forma automatizada y sin relojes harcodeados.

## 12. Criterios de rechazo
- El modelo devuelve etiquetas estáticas "0" perpetuamente porque las "Emission Probabilities" están mal escaladas (Log-likelihood fallida).
- La inferencia HMM sobrecarga la RAM acumulando arrays recursivos históricos infinitos (El buffer de observación debe ser estricto y de tamaño fijo).

## 13. Riesgos que mitiga
- Riesgo de Agotamiento de Táctica (Strategy Exhaustion): Obligar al bot a correr una estrategia de Market Making puro (Proveeduría pasiva de liquidez) durante un colapso del mercado destruye las cuentas. Identificar un régimen tendencial-agresivo obliga al bot a comportarse como un Taker (Agresor del mercado) y no como un Maker. Cambiar de sombrero en milisegundos asegura la supervivencia a largo plazo y es el secreto del Arbitraje Institucional de élite.
- Sobre-Costos Cloud: Calcular rutas complejas de 500 tokens cada milisegundo es extremadamente caro en CPU. En régimen "Muerto", el bot se relaja y ahorra cómputo, activando su agresividad 100% de CPU sólo cuando el Régimen justifica la pelea.

## 14. Integración con otras skills
- Provee la variable `Regime_Multiplier` al Optimizador Estocástico (Skill 6), Ajustador de Tamaño (Skill 2) y Sensibilidad (Skill 10).
- Consume telemetría de Volatilidad y Precios Limpios de Normalización (Skill 32).

## 15. Modelo de datos sugerido
```json
{
  "HMMRegimeState": {
    "asset_class": "MAJOR_CAPS",
    "timestamp_ms": 1714521234105,
    "inferred_state_id": 2,
    "inferred_state_label": "HIGH_VOLATILITY_DIRECTIONAL",
    "state_probabilities": [0.01, 0.04, 0.95],
    "transition_detected_ago_secs": 140,
    "recommended_actions": {
      "scale_down_trade_sizes_pct": 50,
      "widen_minimum_spread_bps": 25,
      "pause_market_making": true
    }
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Worker HMM Node/Rust que ejecuta un cron iterativo cada 1000 milisegundos evaluando la ventana móvil de Rendimientos Logarítmicos pasados 100 segundos. Modifica un Singleton State en RAM que el resto lee instatáneamente.

## 17. Logs obligatorios
- `[INFO] HMM Decoder: Regime transition detected for BTC_USDT. [LATERAL_CALM] -> [HIGH_VOLATILITY]. Propagating multiplier changes.`
- `[DEBUG] Viterbi State Confidence for ETH_USDT: Calm (15%), Choppy (80%), Volatile (5%). Locking into CHOPPY mode.`
- `[WARN] HMM Log-Likelihood Underflow intercepted. Applying Log-Sum-Exp Trick to save state computation.`

## 18. Métricas obligatorias
- `time_spent_in_regime_X_minutes` (Analítica profunda del comportamiento del mercado).
- `hmm_decoding_latency_ms`.
- `regime_transition_frequency_hourly` (Para alertar sobre un "Thrashing" indeseado).

## 19. Tests unitarios
- Matriz Transición Lógica: Crear un modelo en papel donde es imposible pasar de Calma (Estado 0) a Caos Absoluto (Estado 2) sin pasar por Choppy (Estado 1). Fijar Transition(0 -> 2) = 0.0. Validar que la decodificación jamás emite un salto 0->2 y que obliga una transición suave.
- PDF (Gaussian Calculation): Mandar un array simple (Ej. [0,0,0,0]). La PDF del estado "Calma" (cuyo mean es 0 y varianza pequeña) debe devolver una probabilidad muy cercana a `1.0`. Si la varianza se estropea (devuelve cero infinito), la matemática tira división por cero, interceptar este caso crítico.
- Underflow Protection: Iterar la fórmula de Markov durante 1000 pasos. Validar que la multiplicación de 1000 números muy pequeños entre `0 y 1` (ej. `0.2 * 0.1 * 0.05...`) no resuelva en un primitivo `0.0000000` nativo del procesador destruyendo el tracker, sino que mantenga la proporción algorítmica.

## 20. Tests de integración
- Cargar los parámetros HMM desde un archivo estático. Conectar el Módulo HMM al Orquestador general (Mocked). Inyectar una serie de datos artificial (Sine Wave calmada, seguido de un Spike violento, seguido de Sine wave). El Módulo debe reportar Reg1 -> Reg3 -> Reg1 consecutivamente, y el Orquestador debe imprimir que modificó sus Spread Constraints como reacción a la alerta interna.

## 21. Tests E2E
- Un viernes pre-aprobación del ETF de Bitcoin. El mercado pasa de quietud total a locura transaccional. La skill de Markov captura el inicio estocástico de las varianzas reventadas. A los 2 segundos de iniciar la locura, el bot entra en "Régimen Pánico y Agresivo". Duplica sus rangos de slippage tolerable, reduce su tamaño de compra a la mitad por gestión de riesgo, apaga la estrategia de Yield, y habilita todos sus recursos computacionales al Arbitraje CEX-CEX agresivo, cazando diferencias monstruosas antes de que los bots estáticos puedan reaccionar o se estanquen.

## 22. Checklist de producción
- [ ] Incorporación de Rutinas de Compilación Científica. Dependiendo del lenguaje, usar librerías densas C (Ej. BLAS/LAPACK) y no cálculos `for-loops` de vainilla (Vanilla JS Array iterations) para calcular gaussianas de ventana múltiple, la CPU se hundirá sin soporte SIMD nativo.
- [ ] Separar Clases de Activos (Clusters). Agrupar las shitcoins en un modelo HMM de Volatilidad Alta, y los Blue Chips (ETH/BTC/SOL) en un modelo HMM Independiente. Correr el algoritmo universal sobre todos a la vez contamina el resultado general asumiendo que el mercado entero está de una sola forma.
- [ ] Cachear Emisiones (Emission probabilities cache): Dado que el bot evalúa el precio muy a menudo, si el spread se repite (ej. 0.02%), no calcular la probabilidad Gausiana compleja de nuevo (Función euler exponencial), extraerla de un diccionario in-memory lookup.

## 23. Ejemplo de configuración no hardcodeada
```yaml
hmm_regime_engine:
  active_models:
    - id: "crypto_majors_hmm_v1"
      assets: ["BTC", "ETH", "SOL"]
      states: 3
      params_file: "./models/hmm_majors.json"
  sliding_window_size_ticks: 100
  inference_interval_ms: 1000
  log_sum_exp_underflow_protection: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class HMMRegimeDetector {
    constructor(modelParams) {
        this.transitionMatrix = modelParams.transition;
        this.emissionMeans = modelParams.means;
        this.emissionVars = modelParams.vars;
        this.windowBuffer = new CircularBuffer(CONFIG.window_size);
    }

    async analyzeRegime(latestReturnLog) {
        this.windowBuffer.push(latestReturnLog);
        if (!this.windowBuffer.isFull()) return; // Wait for warmup
        
        // Viterbi or Forward algorithm implementation (Optimized C/WASM)
        const stateProbabilities = FastMath.computeForwardAlgorithmLogSpace(
            this.windowBuffer.toArray(),
            this.transitionMatrix,
            this.emissionMeans,
            this.emissionVars
        );
        
        const dominantState = getArgMax(stateProbabilities);
        
        if (dominantState !== this.lastState) {
            EventBus.emit('REGIME_CHANGED', { from: this.lastState, to: dominantState, probs: stateProbabilities });
            this.lastState = dominantState;
        }
    }
}
```

## 25. Criterio final de excelencia
Los Hidden Markov Models otorgan Inteligencia Contextual Macro al Bot HFT. En lugar de ejecutar la misma receta rígida 24/7 sin importar el ambiente (fallo #1 de programadores retail), este componente muta el ADN del comportamiento del Agente basándose en la física probabilística actual, blindando los retornos y capitalizando la disrupción al vuelo.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Costo de Cómputo (CPU Profiling) elevado si no se abstraen las inferencias a un lenguaje de nivel máquina C/C++ FFI o Rust, bloqueando el bot (Javascript/Python Vainilla sufrirán).
- Dependencias: Data Lake (Training Offline), Mathematical Matrix Libraries.
- Próxima skill: Detección de order flow toxico (Spoofing) (Skill 49).
