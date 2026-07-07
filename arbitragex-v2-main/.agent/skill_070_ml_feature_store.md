# SKILL 070 — Machine Learning Feature Store (Data Engineering)

## 1. Propósito superior
Construir y orquestar el Ecosistema Neurálgico (Data Backbone L2 O(1)) de Ingeniería de Características (Feature Engineering) para los Modelos Predictivos de Inteligencia Artificial del Agente (Skill 47 XGBoost y Skill 56 GNN). A diferencia del TSDB en crudo (Skill 37 que guarda `Price` y `Volume` básicos), este Feature Store transforma los datos en bruto CEX/DEX L1 L2 O(1) en Vectores Matemáticos Ultra-Densos (Features) como `Z-Scores Móviles, RSI, MACD HFT, Transformadas Fast Fourier L2, Volatilidad de Parkinson, y Desbalances L2 Ponderados`, centralizándolos O(1) In-Memory para que las Redes Neuronales Infran en Sub-Milisegundos sin perder el tiempo HFT Cripto Calculándolos On-The-Fly. 

## 2. Nivel de conocimiento requerido
Quant Data Engineer / MLOps Architect L1 L2 HFT. Dominio absoluto de Data Pipelining O(1) en Memoria (Apache Arrow, Redis Timeseries, Parquet L2 Cripto), Time-Series Feature Extraction (Pandas/Numpy ported to C++ SIMD O(1) Vectorization), Criptografía Micro-Estructural de Retardos (Lagged Returns Correlation L1), Normalización Z-Score de Colas Gordas (Fat-Tail Standardization L2), e Implementación Feature Stores Offline/Online L2 (Hopsworks/Feast Logic).

## 3. Capacidades principales
1. Feature Extraction O(1) Real-Time (Extracción Asíncrona L2): Cada vez que el Precio de Binance CEX (Skill 33 L2) cambia, esta Skill calcula simultáneamente (Vía C++ SIMD) 150 Features derivadas (Ej. `RSI_1m_L2`, `EWMA_Spread_L2`, `Tick_Velocity_L1`, `Volume_Acceleration_HFT`). Transforma 1 dato Base L2 en 150 Insights Cripto.
2. In-Memory Tensor Construct L2 (Cache de Inferencia O(1)): Empaqueta esos 150 Features exactos en un Tensor Unidimensional Float32 (Vector Plano C/Rust O(1)) listo para que el Motor ONNX o XGBoost (Skill 47/56 L2) lo consuma instantáneamente sin casteos lentos de Object->JSON CEX L2. (0 Serialization Overhead L2).
3. Lagged Returns & Autocorrelation (Memoria Corto Plazo L2): Mantiene Buffers Circulares O(1) L2 de los retornos de precios `T-1, T-5, T-10, T-50` milisegundos HFT. Enseña a las Redes Neuronales el Momentum (Inercia L2) histórico inmediato de forma serializada.
4. Normalización Continua en Vivo (Streaming Z-Score Standardization L1): Las Redes Neuronales mueren si les metes Precios Absolutos (Ej. `$65,000`). Solo entienden Deltas `(0 a 1, o -3 a 3 Z-Score)`. Este Store actualiza las Medias (Mu) y Varianzas (Sigma) L2 de los últimos 30 días exponencialmente O(1) para Normalizar la Entrada en Tiempo Real sin mirar toda la BD SQL Cripto L2.
5. Sincronización Temporal Multisensor (Time-Aligned Feature Merging L2): Mezcla el Funding Rate (Skill 16 L2), con el OrderBook Imbalance (Skill 62 L2) y el Latency Ping (Skill 34 L1). Como vienen a frecuencias L2 distintas (Unos cada 8 horas, otros cada 1 ms), el Feature Store "Plancha/Rellena" O(1) (Forward Fill / Interpolation HFT) asegurando que el Tensor MUX tenga todos los features completos en cualquier Snapshot milisegundo pedido por el Orquestador AI L2.
6. Offline Training Dump (Generador de Parquet Batch L1 L2): Al final del día, consolida el In-Memory Feature Store Cripto y lo vuelca Asíncronamente al Disco Duro L2 en formato comprimido Parquet/HDF5 O(1). Así, el Data Scientist Offline entrena Modelos XGBoost Semanales usando EXACTAMENTE la misma matemática Cripto generada en Producción L2 (Previene Training-Serving Skew L1 L2 HFT).
7. Feature Pruning L2 (Extracción Funcional O(1) Principal): Calcula Correlaciones de Pearson L2 In-Memory L1. Si descubre que `Feature_A` y `Feature_B` son 99% idénticos Cripto L2 O(1), "Poda" (Drop Feature) O(1) el Vector Tensor L2, ahorrando RAM y CPU Cycle HFT de Inferencia GNN/ML L2 Cripto O(1).
8. Detección de NaNs y Anomalías Cripto (Imputation Filter L2 O(1)): Binance Falla L2. Entrega un Feature de Volumen igual a `-1`. Las Neural Networks Cripto multiplicarían por `-1` destrozando todo el Tensor CEX L2 O(1). El Store Intercepta Anomalías (Outlier Clamping HFT O(1)) y aplica Medias Locales o Zeros L2, blindando la Matemática Predictiva de Fallos I/O L2 Cripto.
9. Cross-Asset Feature Splicing (Contexto Macro L2): Inyecta las variables del "Ethereum" a los tensores de predicción del "PEPE" L2 HFT. Reconoce que las Alts L1 L2 Cripto no se mueven en el vacío. Si ETH cae, PEPE caerá L2 O(1). El Store garantiza que el AI tenga Contexto Cripto Macro (Beta Variables L2 O(1)).
10. Versionado de Features (Feature Schema Evolution L2): Sabe que el Modelo "XGBoost_V2.onnx" requiere 45 features L2 O(1) Cripto, pero el "XGBoost_V3.onnx" requiere 60. Sirve Tensors Versionados L2 O(1) para permitir Modelos HFT Múltiples Operando en Shadow Mode L2 simultáneamente Cripto O(1).

## 4. Entradas requeridas
- `raw_data_streams_l2_o1`: Subscripción en Memoria L2 Cripto a Todos los OrderBooks (Skill 33), Trades (Skill 13), y Microestructura (Skill 62 O(1)).
- `macro_defi_variables_l1`: Estado de Funding Rates, Yields (Skill 16, Skill 58) e Inventarios CEX (Skill 40 L2).
- `feature_schema_config_l2_o1`: Un JSON/YAML mapeando la lista de columnas (Features Cripto) que el Modelo ONNX ML L2 actualmente desplegado en producción Exige O(1).

## 5. Salidas esperadas
- `realtime_tensor_array_float32`: Un Array Nativo C++ Float (Typeless Memory L2 O(1)) entregado a Cripto Velocidad de Microsegundos a Skill 47 y 56 O(1) HFT.
- `daily_parquet_feature_dataset_l2`: Archivo Masivo Cripto Offline L2 listo para Pandas/Polars Jupyter Training Pipeline HFT L1.
- `feature_health_telemetry_l2_o1`: Avisos (Missing Data %, Drift Alerts O(1)) sobre la degradación de los oráculos Cripto L2.

## 6. Reglas inmutables
- Training-Serving Skew Prevention L2 O(1): El CÓDIGO (Math C++) que calcula el `RSI_14` en tiempo real (Producción) DEBE SER EL MISMO CÓDIGO que genera el histórico de `RSI_14` para el Entrenamiento Offline L2 Cripto. Jamás calcularlo en Python Pandas para entrenar y reescribirlo en Node.js para Operar. Un descalce microscópico de redondeo de Decimales Cripto O(1) arruinará el Modelo ML CEX L2 haciendo que pierda Alpha Millonario HFT O(1).
- Todo el Store L2 O(1) DEBE residir en Fast-Memory O(1) (RAM, Ring Buffers, CPU L3 Cache HFT O(1)). Hacer llamadas SQL `SELECT` a una BD (Skill 37 TSDB L2 O(1)) en el Event Loop Principal Cripto para generar Features destrozará el Sistema HFT con +100ms de I/O Latency L2. Prohibido Discos.
- Los Tensores Output Cripto L2 (Floats O(1)) DEBEN escalar a `-1.0 a 1.0` (o Normalización MinMax / RobustScaler L2). Redes Neuronales con Features Dispares HFT Cripto O(1) (Ej. Vol=$1Millon y Spread=$0.005) sufren "Vanishing/Exploding Gradients L2 O(1)" e Inferencia Errores Catastróficos de Peso Cripto L1 L2 HFT.

## 7. Algoritmos o métodos que debe conocer
- Welford's Online Algorithm L2 O(1) (Cálculo de Media y Varianza Móvil Exponencial en O(1) Memoria HFT).
- Exponential Moving Averages (EWMA) Cripto Cascade O(1).
- Circular Ring Buffers O(1) C/Rust Memory Mgmt.
- Feature Selection Methods L2 O(1) (Information Gain, Mutual Information Cripto HFT L2).

## 8. Fórmulas críticas
- **Streaming Welford (Z-Score On-the-fly L2)**: `Mean_new = Mean_old + (x - Mean_old)/N`, `Z_Score = (x - Mean_new) / StdDev_new` (Magia O(1) para Normalizar Precios HFT Cripto sin almacenar historial L1 L2).
- **Time-Decay Weighted Imbalance Cripto O(1)**: `EWMA_Feature_T = (Alpha * X_T) + (1 - Alpha) * EWMA_Feature_T-1`
- **Lagged Return Extraction L2 O(1)**: `Log_Ret_1m = Log(Price_T) - Log(Price_T_Minus_1Min)` (Normalizado Asintótico HFT Cripto O(1)).

## 9. Casos extremos
- Feature Drift (Cambio Estructural L1/L2 O(1)): El Modelo ML L2 se entrenó cuando BTC valía $20,000. Los Volumes eran $5M por tick Cripto. Ahora BTC vale $100,000 L2. Los Volumes son $15M por Tick HFT. El Modelo ML "Se confunde" Cripto O(1) creyendo que $15M es una anomalía masiva (Z-score Gigante) y dispara MUX ciegos (Overshooting HFT L1 L2 O(1)). Solución O(1): El Feature Store Cripto DEBE implementar un Recalibrador Diario Móvil L2 (Rolling Standard Scaler L2 O(1)). La Media y Varianza Base del Modelo Cripto se re-ancla a los últimos 7 días HFT O(1), diluyendo el cambio Macro Cripto L1 O(1).
- Stale Data Trap (Datos Muertos L2 O(1)): El Oráculo de Funding Rate CEX (Binance) se cae (Skill 16 L2 O(1)). Deja de Enviar Ticks L2. El Feature Store se queda "Rellenando hacia adelante" (Forward Filling L2 O(1)) el dato viejo por 12 Horas Cripto HFT. El ML toma decisiones Cripto L2 con datos Muertos O(1) perdiendo Asimetría Delta L2. Solución: El Tensor Cripto O(1) añade un Meta-Feature HFT: `Staleness_Feature_X_MS`. El XGBoost aprende que si `Staleness > 1000ms`, esa variable no sirve y la ignora L2 (Self-Aware ML HFT O(1) Cripto).
- Curse of Dimensionality L2 O(1) (Explosión Combinatoria RAM HFT): Tienes 1000 Símbolos CEX L2 y produces 500 Features Cripto. Eso da 500,000 Floats O(1) calculados cada Milisegundo L1 L2 Cripto. ¡Destrozas el CPU L3 Cache O(1) y generas Overheating Node.js/Rust Cripto! Solución Vectorial L2: Feature Pruning estricto L2. La Producción L2 Cripto solo mantiene en RAM los `Top N Features` exactos (Ej. 35) seleccionados por su "Feature Importance / Gain L2" del Backtest Cripto. Reduciendo a Cero la computación Basura HFT O(1).

## 10. Validaciones obligatorias
- PRE: Asegurar Dimensiones Tensor L2 O(1). Si el modelo ML Cripto exige un Vector C/Rust de `[1, 55]`. El Feature Store NO DEBE mandar un Array de `[1, 54]`. Causará un SegFault Memory Crash Cripto ONNX L2 O(1) que apaga todo el Datacenter HFRC HFT. Hard Validation Matrix Dimensions L2.
- CÁLCULO: Mantener sincronía de Endianness/Tipo O(1). (Float32Array JavaScript a C++ Struct Float32 O(1) EVM). Evitar conversiones costosas L1 L2 `Double` a `Float` Cripto HFT en cada ciclo L2.
- POST: Incorporación de Eventos "Labels" (Target Generation Offline L1 L2). Además de Features X, en segundo plano (Background) guarda el resultado `Y` (Target L2 = Precio 10 Segundos Después Cripto O(1)) y lo asocia al instante L2. Este Set Etiquetado (Labelled Data L2 Cripto O(1)) es el Santo Grial que entrena la Inteligencia L1 L2 del Mañana Cripto O(1).

## 11. Criterios de aprobación
- Entrega Pura Continua L2 de un Flat Array `Float32Array(N)` Cripto O(1) en menos de 0.2 Milisegundos O(1) a la Red Neuronal, combinando +50 métricas macro y micro HFT.
- Capacidad de Recrear un Entorno O(1) Idéntico L1 L2 Offline (Bit-exact reproduction Cripto HFT) asegurando que el Modelo ML O(1) de Python L2 coincida 100% con el Motor HFT C/Rust Producción L2 O(1).

## 12. Criterios de rechazo
- El Feature Store descarga o solicita datos a Bases de Datos Relacionales (PostgreSQL L2) o APIs Externas L2 (CoinMarketCap L1) en Tiempo Real HFT Cripto O(1). El Store HFT O(1) DEBE ser "State-Derived In-Memory HFT Only L2 O(1)".
- Normalización Fija Harcodeada L2 Cripto. (Ej. `Volume / 1000000`). Si el token es SHIB y maneja billones L1 L2, o es WBTC L1 y maneja cientos O(1). La división fija rompe el Modelo L2 O(1) Global HFT. Obligatoria Auto-Escalabilidad Estadística Welford L2 O(1) Cripto L1.

## 13. Riesgos que mitiga
- La Muerte por Latencia "I/O AI Latency Wipeout L2 O(1)". El MLOps tradicional en Python Cripto L2 CEX extrae datos de la BD, hace `pandas.merge()`, rellena `NaNs`, y predice. Ese pipeline Tarda 300ms a 2 Segundos L1 L2. Para HFT, 2 Segundos L2 es Historia Antigua Cripto O(1). La Máquina HFRC Cripto (A través de este Store O(1)) implementa el pipeline Entero en `C/Rust Streaming Engine L2 O(1)`, logrando el milagro HFT Cripto: Predicción Macro Complex en Sub-Milisegundo O(1). Superando a firmas quants gigantescas atrapadas en latencias Python L2 O(1) Cripto L1 HFT.

## 14. Integración con otras skills
- Creador Supremo del alimento O(1) para el XGBoost L2 (Skill 47 HFT) y GNN (Skill 56 L1 L2 Cripto O(1)).
- Fusionador de Contexto L2 HFT (Absorbe Microestructura Skill 62 L2, Funding 16, Triangulación Skill 53).
- Volcador Histórico O(1) HFT a la Base de Datos Central TSDB (Skill 37 L2 Cripto).

## 15. Modelo de datos sugerido
```json
{
  "FeatureStoreVectorDeliveryO1": {
    "pair_id": "SOL_USDT_CEX",
    "timestamp_ms_o1": 1714521234105,
    "vector_latency_ms": 0.15,
    "ml_schema_version_l2": "v4.5_HFT_XGB_O1",
    "feature_array_flat32_o1": [
      0.85,  // Z-Score Log Return 1m L2
      -2.4,  // OrderBook Imbalance Skill 62 L2
      1.12,  // MACD High Freq L2
      0.0,   // Funding Rate L1
      14.5,  // ATR Volatility Z-Score L2
      // ... 40 more features ...
      15.0   // Staleness Time Delay (Ping L2 Delay)
    ],
    "target_y_queued_for_offline_labeling_l2": true, // Will record price 5 sec from now
    "status": "TENSOR_DELIVERED_TO_ONNX_ENGINE_L2_O1"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Subproceso Core Ring-Buffer L2 `StreamingFeatureStore_O1`. Contiene Arrays 1D Planos C/Rust pre-asignados O(1) L2 Cripto. Provee método Síncrono HFT `getLatestVectorAsFloat32(modelName, pair)` L2 Cripto para inyección atómica HFT.

## 17. Logs obligatorios
- `[INFO] Feature Store L2: Bootstrapping Welford Moving Stats from historical 7-day SQL Dump L2. Rolling Means initialized for 150 Features. Ready for Streaming Ingestion O(1) HFT.`
- `[DEBUG] Vector Delivery HFT O(1): Forward-Filled missing Funding Rate Tick L2. Imputed NaN in OrderBook Vol L2. Delivered 45-Dimension Tensor Cripto in 0.12ms to ONNX Skill 47 L2 O(1).`
- `[CRITICAL] Feature Drift Detected HFT L2! Moving Average of Feature_12 (Volume_Z) drifted by > 5 Standard Deviations Macro L1 O(1). Model predictions might be out-of-distribution Cripto L2. Alerting MLOps Supervisor L1 O(1).`

## 18. Métricas obligatorias
- `average_feature_pipeline_latency_ms_o1` (De la recepción del L2 CEX Websocket hasta Vector listo O(1) Cripto).
- `missing_feature_imputation_count_hourly` (Para medir si el Bot "Adivina" mucho por oráculos L2 Cripto caídos L1 O(1)).
- `daily_parquet_data_dump_size_gb` (Mide crecimiento del Acervo Analítico Cripto HFRC L1 L2).

## 19. Tests unitarios
- Welford Streaming Math Integrity L2 O(1): Ingresar Arrays Crudos L2 `[1, 5, 10, 100, 20]`. Calcular Z-Score en Python Pandas Offline (El Estándar Dorado Cripto). Calcular Z-Score usando el Online Streaming Engine C/Rust del Feature Store L2 O(1). Los Floats L2 Cripto DEBEN coincidir al Sexto Decimal O(1) Cripto L1 L2 HFT (1e-6 Tolerance Cripto).
- Flat Tensor Generation Dimensions L2: Configuración Schema Dicta "50 Features L2 O(1) Cripto". Entregar faltantes L2. El Módulo DEBE devolver `Float32Array.length == 50` Cripto rellenando con `0.0` u defaults (Zero-Padding Cripto L2 O(1)) sin alterar indexación Cripto HFT.
- Zero-Allocation Loop O(1): Ejecutar la Función `UpdateFeatures()` 1 Millón de Veces en Loop C/JS HFT. Monitorear RAM Heap Cripto O(1). Si el Garbage Collector de V8 (Node) O(1) salta, el Test FALLA L2 Cripto. El Store DEBE Mutar Memoria Pre-Asignada Cripto (In-Place Mutation L2 O(1)) sin crear Nuevos Objetos L2 HFT (Zero GC Overhead Cripto O(1)).

## 20. Tests de integración
- Levantar Subproceso ONNX HFT L2 Predictor (Skill 47 L2). Inyectar Flujo Masivo L2 Mock Orderbooks y Trades a 10,000 Mensajes/Segundo L2. El Feature Store O(1) procesa, normaliza y empaqueta Cripto L2. El ONNX consume y PREDICE O(1). Medir la Latencia Total E2E Cripto (Websocket_In -> Prediction_Out L2 O(1)). Debe sostener Promedio < 1.0ms Global Cripto HFT para asegurar Explotación Sub-milisegundo Taker L2 CEX.

## 21. Tests E2E
- El Data Scientist HFRC L2 O(1) diseña en Jupyter Offline un Nuevo Modelo XGBoost HFT L2 Cripto. Añade una nueva Variable "Ratio de Micro-Price vs EMA 1 minuto L2". Compila Modelo ONNX Cripto L2. El Feature Store L2 Local O(1) en el Servidor Bare-Metal Tokyo actualiza su Config JSON Cripto L2. En tiempo real, comienza a derivar el Nuevo Feature In-Memory O(1) Cripto. El Servidor arranca y procesa Flujo en Vivo HFT L2. La Sincronización es perfecta O(1). El ML capta la Varianza Cripto L2. Genera Disparos de Arbitraje L1 L2 CEX en Tiempos Reales sin intervención Humana L1 O(1) HFT, guardando el Delta/Historial Paralelo al Data Lake O(1) L2. La Rueda de Retroalimentación de Inteligencia Artificial (AI Flywheel Cripto L1 L2) se ha cerrado atómicamente.

## 22. Checklist de producción
- [ ] Label Leakage Prevention L2 O(1): Asegurar Matemática Extrema de que el "Feature Store" JAMAŚ calcula una Derivada HFT Cripto usando un Dato CEX L2 "Que todavía no existía en el milisegundo de ejecución (Look-ahead bias L1 O(1) HFT Cripto)". Validar Index Timestamp Cripto de manera Sagrada L2 O(1).
- [ ] Exportación Automática Cripto Offline L2 O(1): Integrar CRON diario que Mapee el Cache Ring-Buffer Cripto O(1) hacia Parquet L2 (S3 Storage) de madrugada (Menor volatilidad L2 CEX), liberando al Agente Principal de la Carga de Backup I/O Cripto Pesada L2 HFT.

## 23. Ejemplo de configuración no hardcodeada
```yaml
machine_learning_feature_store_o1:
  enable_streaming_features_l2_o1: true
  active_schema_version_c_o1: "model_v4_xgb_hft_55_dim"
  rolling_normalization_window_ticks_l2_o1: 100000 # Memory window for Z-Score streaming stats
  imputation_strategy_for_nans_l2_o1: "ZERO_FILL" # Or 'FORWARD_FILL' or 'CLUSTER_MEAN'
  buffer_preallocation_size_mb_l2_o1: 512 # Reserve RAM upfront to avoid Garbage Collection Spikes HFT
  daily_parquet_data_lake_dump_l2_o1: true
```

## 24. Ejemplo de pseudocódigo
```javascript
// C/Rust TypedArray Backed Singleton O(1) Zero-GC Engine
class HFTFeatureStoreEngine {
    constructor(schemaConfig) {
        this.dimensions = schemaConfig.numFeatures;
        this.flatVectorBuffer = new Float32Array(this.dimensions); 
        this.welfordStats = new WelfordOnlineMath(this.dimensions);
    }

    // Called instantly on EVERY Websocket L2 Tick (Millions of times per day)
    updateAndGetTensorVectorO1(rawL2Update, extraMacroContext) {
        
        // 1. In-place derivation O(1) (No new object creation = No GC pause)
        this.flatVectorBuffer[0] = this.calcRsiO1(rawL2Update.price);
        this.flatVectorBuffer[1] = this.calcMicrostructureImbalanceO1(rawL2Update.orderbook);
        this.flatVectorBuffer[2] = extraMacroContext.fundingRate; // Forward-filled implicitly
        // ... (fill all 55 slots)

        // 2. Normalization Z-Score Streaming O(1)
        for (let i = 0; i < this.dimensions; i++) {
            // Update Running Mean/Variance
            this.welfordStats.update(i, this.flatVectorBuffer[i]);
            
            // Standardize In-Place
            this.flatVectorBuffer[i] = this.welfordStats.getZScore(i, this.flatVectorBuffer[i]);
            
            // Handle NaNs (C/C++ protection)
            if (Number.isNaN(this.flatVectorBuffer[i])) {
                 this.flatVectorBuffer[i] = 0.0; // Imputation safe-guard
            }
        }

        // Return direct reference to memory buffer (Zero copy O(1)) 
        // to be ingested by ONNX/XGBoost Engine immediately.
        return this.flatVectorBuffer; 
    }
}
```

## 25. Criterio final de excelencia
El Feature Store In-Memory O(1) es la "Materia Gris Computacional" que separa un simple Script Node.js Retail (Que explota con latencias) de una Infraestructura Cuantitativa C/C++ HFT de Clase Institucional. Extrae todo el pesado Lastre I/O y de Normalización Matemática del Pipeline L2 HFT, permitiendo que la Red Neuronal pura del Agente HFRC prediga el Futuro Cripto L1 L2 en Sub-Milisegundos con Matemática Limpia, Vectorizada y Absolutamente Precisa Libre de Latencia RAM/GC.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Memory Float Drift L2 Cripto O(1) (Acumulación microscópica de errores de Redondeo en la Matemática Streaming Welford O(1)). Solucionable reseteando la Media/Varianza Forzosamente a una foto Estática 1 vez a la semana durante Mantenimiento HFT L2 Cripto O(1).
- Dependencias: Websocket L2 Streams (Skill 33), ML/ONNX Engine HFT (Skill 47/56).
- Próxima skill: Orquestador Dinámico de Cuentas Múltiples (Sybil API Management) (Skill 71).
