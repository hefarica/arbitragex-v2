# SKILL 047 — Generador de señales ML (Random Forest/XGBoost)

## 1. Propósito superior
Dotar al agente de capacidades predictivas (Machine Learning / IA Cuantitativa) de alta velocidad para micro-decisiones estructurales. Mientras las matemáticas puras de las Skills 1 al 30 encuentran los arbitrajes en el "ahora" exacto, el modelo de ML evalúa el ruido y el flujo del libro de órdenes y predice la probabilidad direccional del micro-tick en los próximos 100 a 500 milisegundos. Esta skill actúa como un Filtro (Alpha Filter) y Oráculo Predictivo local.

## 2. Nivel de conocimiento requerido
Ingeniero en Machine Learning Cuantitativo (Quant ML Researcher). Dominio de árboles de decisión Gradient Boosted (XGBoost, LightGBM, Random Forest), Feature Engineering sobre datos de alta frecuencia (Orderbook Imbalance, Trade Flow Toxicity, VWAP Slope), Normalización de datos estacionarios (Stationarity, Returns over Prices), y ejecución en inferencia de ultrabaja latencia (ONNX Runtime, C-bindings).

## 3. Capacidades principales
1. Inferencia Ultra-rápida (Microsecond Scoring): Evalúa un vector de 50 variables (features) en < 1 milisegundo utilizando un modelo pre-entrenado compilado (ej. formato ONNX o TensorRT) para generar un `Score` de 0 a 1.
2. Predicción Direccional a Corto Plazo (Micro-Trend): Estima si la probabilidad de que el precio del activo suba en los próximos 200ms es > 70% basándose en el historial reciente del Bid/Ask imbalance.
3. Evaluación de Toxicidad de Flujo (Order Flow Toxicity): Detecta patrones algorítmicos enemigos (ej. "Spoofing" institucional, grandes bloques de órdenes falsas) y le avisa al motor "Este spread es una trampa, no ejecutes".
4. Feature Engineering Dinámico (Generación Continua): Convierte los Torrentes de datos (Deltas, Volumen, Latencia) en métricas estáticas en ventanas móviles (Rolling Windows).
5. Optimizador de Taker Edge (Midiendo la urgencia): Decide la "agresividad" de la ejecución. Si el modelo dice "El precio va a escapar hacia arriba inminentemente", ordena cruzar el spread (Market Order Taker). Si el modelo dice "Precio en letargo", ordena usar órdenes Limit pasivas.
6. Aislamiento del Proceso de Entrenamiento (Offline Training): El modelo NUNCA se entrena en tiempo real. Se entrena semanalmente de forma offline usando el Data Lake (Skill 37) y solo se importa el binario o archivo de pesos al Hilo Operativo para Inferencia.
7. Alerta de Decadencia de Modelo (Concept Drift): Monitoriza si el modelo de ML ha empezado a perder precisión (Accuracy Drop). Los mercados cambian de régimen (Skill 48); si el modelo se vuelve inútil, la skill notifica para un re-entrenamiento urgente.
8. Clasificación de Spread Falso: Analiza si un arbitraje es "Fantasma". Si XGBoost detecta que spreads similares históricamente siempre fallaron por latencia oculta, bloquea el intento.
9. Ensemble Voting (Consenso): Combinar 3 modelos ligeros distintos. Si Random Forest y LightGBM coinciden, la señal se emite. Si difieren, la señal es neutra (No-trade).
10. Detección de VPIN (Volume-Synchronized Probability of Informed Trading): Cálculo matemático para detectar información asimétrica privilegiada en un CEX antes de que impacte el precio final.

## 4. Entradas requeridas
- `realtime_features`: Vector de datos transformado (ej. `[bid_ask_spread, book_imbalance_10_levels, trade_intensity_1s, ... ]`).
- `pre_trained_model_binary`: Archivo `.onnx` o `.json` con los pesos de los árboles de decisión generados offline.
- `target_horizon`: Ventana temporal de predicción (Ej. 500ms).

## 5. Salidas esperadas
- `ml_signal`: Float `[-1.0, 1.0]` indicando fuerza direccional predicha (-1 fuerte baja, 1 fuerte alza).
- `confidence_score`: Nivel de confianza del árbol `[0.0, 1.0]`.
- `filter_decision`: `TRADE_APPROVED` o `TRADE_BLOCKED`.

## 6. Reglas inmutables
- JAMÁS inyectar el precio crudo absoluto (ej. $65,000) en un árbol de Machine Learning. El ML sobreponderará el nivel nominal y fallará en el futuro si el precio sube a $75,000. SIEMPRE pasar Rendimientos (Returns), Log-Ratios (Log-Prices) y métricas normalizadas/estacionarias al input de inferencia.
- El tiempo total desde la recepción del Feature Array hasta la entrega del Score no debe sobrepasar los 2 milisegundos. Quedan prohibidas las redes neuronales profundas masivas (Deep Learning, Transformers gigantes) en la ruta crítica HFT local. Solo árboles optimizados o SVM ligeras.
- El Score del ML no dispara operaciones por sí solo; en Arbitraje HFT actúa exclusivamente como un FILTRO (Veto) para la matemática determinista. La matemática encuentra el Spread, el ML lo aprueba o lo beta.

## 7. Algoritmos o métodos que debe conocer
- LightGBM / XGBoost C-API Binding.
- Orderbook Imbalance Metrics (OFI - Order Flow Imbalance).
- Moving Average Convergence/Divergence sobre Ticks (no sobre velas).

## 8. Fórmulas críticas
- **Orderbook Imbalance (OFI)**: `(Vol_Top_Bid - Vol_Top_Ask) / (Vol_Top_Bid + Vol_Top_Ask)`
- **Condición de Veto de Señal**: `if (Math_Profit > Min_Profit && ML_Direction_Confidence < Veto_Threshold) { ABORT_TRADE() }`

## 9. Casos extremos
- Explosión de Volatilidad Desconocida (Out-of-Distribution Data): Un hack L1 desploma el precio un 40% en un segundo. El modelo fue entrenado en un mes aburrido de rango. Las features enviadas al modelo están a 10 desviaciones estándar de su entrenamiento normal. El XGBoost pierde total precisión. El sistema detecta "Out-of-Distribution Feature" y APAGA la ponderación del ML automáticamente recurriendo 100% a la matemática determinista cruda.
- Inferencia Bloqueante de Hilo (Thread Blocking Inferrence): Cargar un modelo de XGBoost en Node.js de forma síncrona detiene el Event Loop por 20ms en cada tick de datos. Esta pausa arruina el HFT. Obligatorio trasladar el binario predictor a un Worker Thread aislado o a un componente Rust FFI que escupa las señales asíncronamente al buffer del orquestador.
- Model Over-fitting en Microestructura: El modelo se "aprende" que cierto CEX siempre rebota el precio en el Tick ending in `99` (Ilusión óptica del libro). Cambian la política de ticks en el exchange y el modelo comienza a arrojar falsos positivos masivos. Requiere rotación de modelos y Degradación Suave (Graceful Degradation).

## 10. Validaciones obligatorias
- PRE: Validar que el Array de Inferencia (Features) no contiene `NaN` o `Infinity` causados por divisiones por cero al haber un orderbook vacío momentáneamente. (Los NaNs destruyen la inferencia ONNX).
- CÁLCULO: Mantener una tabla en memoria local para escalar/estandarizar (Z-Score Normalization) las entradas en milisegundos, usando medias y desviaciones pre-calculadas en el entrenamiento.
- POST: Realizar seguimiento empírico asíncrono. Guardar en el Data Lake la predicción que dio el bot vs lo que realmente pasó en la realidad 500ms después. Si el Score Empírico cae bajo el 50% (Pura Suerte/Random), reportar para desconexión y re-entrenamiento.

## 11. Criterios de aprobación
- Entrega probabilidades del árbol de decisión (Sigmoid / Softmax) en < 1ms de RTT (Round-Trip Time).
- Integración nativa sin dependencias pesadas de Python (El agente vive en un ecosistema compilado o V8 puro, requiriendo ONNX Runtime o bindings binarios para velocidad C).

## 12. Criterios de rechazo
- Retraso Inducido Mayor al Spread (Latency > Spread Decay). Si usar Machine Learning hace que el bot tarde 5ms extras, y la vida útil del arbitraje es de 3ms, la inteligencia arruina la ganancia. Fallo estructural del diseño de la skill.
- Feature Leakage (Fuga de Información Futura): Confirmación de que el modelo fue alimentado erróneamente en entrenamiento con "El precio futuro" como un feature de entrada actual, resultando en un modelo 100% perfecto inútil en la realidad.

## 13. Riesgos que mitiga
- Riesgo de "Adverse Selection" (Selección Adversa): Ocurre cuando el bot ejecuta órdenes pero invariablemente termina operando contra creadores de mercado institucionales mucho más listos que él. Si un spread está abierto, suele ser porque la Ballena "Permite" que lo tomes como trampa de liquidez (Toxic Flow). El XGBoost detecta la trampa por el comportamiento inusual de las órdenes que rodean al spread y evita la trampa, parando la operación "rentable pero venenosa".
- Riesgo de Falsa Dirección Unilateral (Phantom Leg Risk).

## 14. Integración con otras skills
- Veta y califica directamente el resultado de la matemática de Arbitraje CEX-CEX / DEX-DEX (Skills 12/13).
- Usa intensivamente el motor general del Data Lake (Skill 37) para el reentrenamiento semanal offline.

## 15. Modelo de datos sugerido
```json
{
  "MachineLearningSignal": {
    "model_version": "v1.4_XGB_ETH_USDT",
    "timestamp_ns": 1714521234105,
    "input_features_hash": "a1b2c3d...",
    "predicted_micro_direction": 1,
    "confidence_pct": 87.5,
    "adverse_selection_risk": "LOW",
    "actionable_signal": "APPROVE_ARBITRAGE",
    "latency_us": 450
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Background compilado a binario que carga el archivo de Inferencia (ONNX) usando `onnxruntime-node` (o su homólogo en Rust `tract` / `ort`). Se expone una función `predictProbability(Float32Array features)` llamada por el Orquestador maestro.

## 17. Logs obligatorios
- `[DEBUG] ML Engine Veto: Math found $5 spread. XGBoost scored -0.92 confidence (High Toxicity Risk). Trade Blocked.`
- `[INFO] ML Model Swap applied. Swapped v1.4 to v1.5 from disk dynamically with zero downtime.`
- `[CRITICAL] Concept Drift Alert! Model empirical accuracy fell below 48% over last 1000 events. ML Engine manually bypassed to avoid losses.`

## 18. Métricas obligatorias
- `ml_inference_latency_microseconds` (Vital, debe ser monitoreado en tiempo real).
- `ml_trade_vetoes_triggered_count` (Para analizar qué tan "miedoso" o conservador es el modelo).
- `empirical_predictive_accuracy_pct` (Medición real del performance tras 500ms).

## 19. Tests unitarios
- Tiempos de Inferencia (Profiling): Ejecutar 10,000 llamadas secuenciales a la función de inferencia. Si tarda > 10ms en total, abortar el paso a producción por ser excesivamente pesado el modelo (Podar árboles - Pruning - necesario).
- Normalization Constraints: Pasar un array de features sin estandarizar. La capa pre-predictiva debe interceptar, aplicar `(x - mean)/std` antes de pasarlo al modelo C/C++, garantizando consistencia.
- Fallback Failsafe: Renombrar el archivo binario del modelo a un nombre falso. El sistema debe lanzar alerta y continuar trabajando con la matemática determinística pura de las Skills 12-20, no crashear (Graceful Degradation).

## 20. Tests de integración
- Levantar un Backtest del mes pasado (Skill 46) con el modelo ML activo (Veto Mode) y un Backtest con ML apagado (Raw Math). El "ML Activo" debe mostrar un Sharpe Ratio mayor o igual, y un número de trades mucho menor (Filtrando basura), incrementando la calidad del PnL por trade.

## 21. Tests E2E
- El motor matemático puro detecta que FTX y Binance tienen un desvío de 0.2%. Prepara los cañones para abrir un arbitraje masivo. Un milisegundo antes, el array de la microestructura del mercado es inyectado al modelo XGBoost. El modelo detecta una "Presión de venta masiva oculta en niveles 4-10" característica de la venta institucional de grandes ballenas (Spoofing). El modelo escupe una señal `Score = -0.99`. El trade se bloquea. En los siguientes 500ms, la liquidez fantasma de FTX se evapora, destrozando el spread. El modelo ML salvó al fondo de un Slippage destructivo validando su necesidad arquitectónica.

## 22. Checklist de producción
- [ ] Optimización Numérica: Compilar el modelo ONNX en `Float16` o Inferencia Cuantizada (`INT8`) en lugar de `Float64` si la precisión marginal de los decimales no afecta la direccionalidad cruda. Reduce el tamaño del modelo un 75% y acelera la inferencia masivamente en CPU.
- [ ] Mapeo de Variables Fijas: Asegurarse de que el orden exacto del Array de Features (Índice 0 = Imbalance, Índice 1 = VWAP...) sea idéntico entre el código Python de Data Science que generó el modelo, y el TypeScript/Rust que alimenta la inferencia en vivo.
- [ ] Implementación de "Rolling Buffers" Circulares eficientes en Memoria para capturar las medias móviles de 1 segundo de las Features, evitando realocar arrays en cada tick recibido de WebSockets.

## 23. Ejemplo de configuración no hardcodeada
```yaml
ml_signal_engine:
  model_path: "./models/xgb_micro_arb_v1_4.onnx"
  inference_threads: 2
  minimum_confidence_to_approve: 0.70
  veto_mode_enabled: true  # True = Filters math. False = Only logs predictions
  empirical_monitoring_interval_ms: 1000
  auto_disable_on_accuracy_drop_pct: 49.0
```

## 24. Ejemplo de pseudocódigo
```javascript
class InferenceEngine {
    constructor(config) {
        this.session = ONNXRuntime.InferenceSession.create(config.model_path);
        this.scalerParams = loadScalerConfig();
        this.rollingFeatures = new Float32Array(50); // Pre-allocated O(1) buffer
    }

    // Called asynchronously 100s of times per second
    evaluateSpreadOpportunity(mathOpportunityObj, rawFeatureDict) {
        if (!CONFIG.veto_mode_enabled) return true; // Bypass
        
        // 1. Z-Score Normalization using SIMD/Fast loops
        for(let i = 0; i < 50; i++) {
             this.rollingFeatures[i] = (rawFeatureDict[i] - this.scalerParams[i].mean) / this.scalerParams[i].std;
        }

        // 2. Wrap into Tensor format for ONNX
        const tensor = new ONNXRuntime.Tensor('float32', this.rollingFeatures, [1, 50]);
        
        // 3. Ultra-fast inference (< 0.5ms on C-backend)
        const outputMap = this.session.runSync({ input: tensor });
        const predictedProb = outputMap.output.data[0]; 
        
        // 4. Decision Logic
        // If Model predicts the price will collapse (Toxic), and we are buying...
        if (mathOpportunityObj.side === 'BUY' && predictedProb < 0.3) {
            log.debug("XGBoost VETO. Predicted dump on Bid side.");
            return false; // VETO
        }
        
        return true; // APPROVE
    }
}
```

## 25. Criterio final de excelencia
El Motor de Señales de ML convierte al bot de una simple "Calculadora determinista" a una Bestia Cuantitativa "Predictiva". Otorga una "vista de águila" estadística sobre el caos microscópico, identificando instintivamente trampas institucionales indetectables a simple matemática de suma y resta, aumentando drásticamente la calidad y supervivencia general de la ejecución en entornos depredadores.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Decadencia del Modelo Silenciosa (Concept Drift) donde un cambio estructural del mercado (Ej. Cambio del tamaño mínimo del Tick Size de Binance) deja obsoleto el entrenamiento anterior destruyendo la precisión repentinamente.
- Dependencias: Data Lake (Skill 37) para entrenamiento Offline, Soporte FFI Binario ONNX.
- Próxima skill: Modelos de Hidden Markov para regímenes (Skill 48).
