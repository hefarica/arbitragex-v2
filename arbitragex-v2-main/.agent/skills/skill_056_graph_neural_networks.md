# SKILL 056 — Graph Neural Networks para predicción de flujos

## 1. Propósito superior
Detectar ineficiencias matemáticas que se propagan en "Efecto Dominó" a través de redes interconectadas de criptoactivos antes de que sucedan. Utilizando Redes Neuronales de Grafos (GNNs / Spatial-Temporal Graph Convolutional Networks), el Agente modela todo el mercado como un tejido conectivo masivo (Nodos = Monedas, Aristas = Correlación de liquidez). Si una ballena hunde el precio del activo A (Bitcoin), la GNN predice matemáticamente con milisegundos de anticipación que los activos B, C y D conectados a él van a colapsar en cadena, permitiéndole al bot "Shortear" o cancelar órdenes en B, C y D mucho antes de que el flujo tóxico los alcance visualmente en sus respectivos Orderbooks.

## 2. Nivel de conocimiento requerido
Científico de Machine Learning Geométrico (Deep Learning en Grafos). Maestría en Message Passing Neural Networks (MPNN), Graph Attention Networks (GAT), Spectral Graph Theory, Spatial-Temporal Convolutional Blocks, PyTorch Geometric, ONNX Export, y procesamiento concurrente asíncrono Tensor-Based.

## 3. Capacidades principales
1. Inferencia Topológica Predictiva (Spatial-Temporal Forecasting): Alimentar la GNN con los Deltas de los 100 principales tokens cada segundo. El modelo escupe una predicción probabilística de flujos de liquidez para el siguiente 1 segundo de *todo* el ecosistema simultáneamente.
2. Contagio de Liquidez (Liquidity Spillover Detection): Entender que si el pool de Uniswap `ETH/USDC` se drena masivamente, la presión matemática fluirá inevitablemente hacia `ETH/USDT`, `wstETH/ETH` y Arbitrum `ETH/USDC`. El modelo capta la "onda expansiva".
3. Auto-descubrimiento de Aristas (Edge Weights Dynamic Learning): La red neuronal aprende por sí misma qué monedas están verdaderamente conectadas (Ej. PEPE y DOGE), ajustando los "Pesos de Atención" (GAT) dinámicamente si la narrativa del mercado cambia, sin que un humano tenga que hardcodear correlaciones.
4. Identificación de "Lead-Lag" (Líder-Rezagado Estructural): Determinar en milisegundos qué Nodos en el grafo son "Fuente" (Líderes que originan el movimiento) y qué Nodos son "Sumidero" (Laggers que reciben el impacto tardío), otorgando Alpha para arbitrar el rezago.
5. Veto de Cascada de Riesgo: Si el modelo predice una "Contracción del Grafo" (Graph Contraction / Market Crash inminente), notifica al Risk Engine (Skill 41) para suspender cualquier arbitraje pasivo en los nodos periféricos.
6. Aislamiento Asíncrono Híbrido: Extrae la inferencia pesada (O(V+E)) a un servidor de inferencia TensorRT o a la GPU (CUDA) local, entregando resultados al Event Loop Node.js/Rust principal de forma no bloqueante a través de memoria compartida o sockets Unix rápidos.
7. Feature Extraction (Nodal Features): Cada nodo se alimenta de variables en tiempo real (Volumen L2, Imbalance, Funding Rate, Latency). El modelo convoluciona estas features combinando vecinos (Message Passing) para inferir un estado global "Meta".
8. Detección de Spoofing Distribuido: Si alguien hace Spoofing (Skill 49) en 5 CEXes distintos a la vez en monedas hermanas, la GNN lo identifica como un solo Ataque Estructural Coordinado (Sybil Attack), algo imposible de ver mirando un solo orderbook.
9. Graph Sparsification (Poda en Vivo): Para poder ejecutar inferencia en < 5ms, el algoritmo dinámicamente "Poda" aristas irrelevantes (Ej. desconecta temporalmente a Solana de Ethereum si no hay volumen puente).
10. Señal Direccional Pura (Alpha Source): Si la GNN dice que el vector resultante es +2% para el nodo `AVAX`, el bot no busca un "Spread" contra otra moneda. Activa un Market Buy Direccional directo confiando en el Alpha predictivo (Stat-Arb complementario a Skill 55).

## 4. Entradas requeridas
- `graph_adjacency_matrix`: Matriz de conexiones y pesos actuales entre los N-Tokens.
- `nodal_features`: Array de tensores en tiempo real (`[Volume, Price_Log_Ret, Spread, VWAP, Funding_Rate]`) para N Nodos.
- `trained_onnx_model`: El archivo compilado de PyTorch Geometric (Entrenado semanalmente offline).

## 5. Salidas esperadas
- `nodal_predictions_tensor`: Array con la dirección de impacto predicha para el próximo Tick de tiempo.
- `cascade_risk_score`: Float de 0 a 1 indicando si la red entera está bajo un choque sistémico inminente.
- `attention_heat_map`: Salida opcional de los pesos de conexión para auditoría visual/humana (Observabilidad).

## 6. Reglas inmutables
- NUNCA entrenar o re-calcular los pesos del Backpropagation (Entrenamiento de la Red) en el servidor de Ejecución HFT de producción. Entrenar GNNs toma horas/días en un clúster de GPUs. El entorno de producción SÓLO ejecuta Inferencia (Forward Pass) cargando el modelo estático para asegurar RTT < 5ms.
- Prevenir el "Oversmoothing" Clásico de GNNs. Si conectas todas las monedas con todas las monedas, la GNN promediará los precios y predecirá que "Todo se moverá 0%". Usar una arquitectura rala (Sparse Graph) donde solo los pares topológicamente relacionados (Mismo CEX, misma red, mismos MM) estén conectados.
- La Inferencia de la GNN DEBE ser un Filtro Asíncrono de Baja Prioridad (Respecto al Arbitraje Aritmético Puro). Si la inferencia se retrasa 50ms, el bot no debe crashear sus patas CEX-CEX rápidas, la GNN simplemente actualiza su veto estadístico "Cuando está lista".

## 7. Algoritmos o métodos que debe conocer
- Graph Attention Networks (GATv2).
- Temporal Graph Convolutional Networks (T-GCN).
- PyTorch a ONNX Export pipeline.
- Eigen-Decomposition / Laplaciano de Grafos (Fundamentos de convolución espectral).

## 8. Fórmulas críticas
- **Message Passing (Formula Base GNN)**: `Node_h(t+1) = Update(Node_h(t), Aggregate(Neighbors_h(t)))`
- **Attention Score (GAT)**: `Alpha_ij = Softmax( LeakyReLU( a_T * [Wh_i || Wh_j] ) )` (Define a qué nodo vecino debe prestarle atención el bot durante un crash).
- **Inference Latency Limit**: `Forward_Pass_Ms < OrderBook_Refresh_Rate` (Debe ser menor al flujo de Websockets, ej. < 100ms max).

## 9. Casos extremos
- Inyección de Features Basura (Missing Values Chaos): Binance se desconecta. El Nodo "BTC" de repente emite `Volume = 0` y `Price = NaN`. Una red neuronal GNN propagará el NaN a través de las aristas multiplicando todo por NaN e inutilizando todo el sistema global. El Pipeline DEBE aplicar Feature Imputation instantánea (Copiar el T-1, o insertar Medias de otros CEXes) antes de inyectar el Tensor a C++ / ONNX.
- El Cisne Negro Estructural (Correlation Breakdown): FTX quiebra. Históricamente, Solana (SOL) y Ethereum (ETH) tenían correlación positiva. De repente, SOL se desploma y ETH se mantiene estable. El modelo GNN pre-entrenado (Que asume que siempre se mueven juntos) lanzará un "Long" masivo a SOL asumiendo que es una divergencia temporal que volverá arriba. El Orquestador debe poseer un Kill-Switch (Skill 41 + 48) que desactive modelos de Deep Learning en regímenes no vistos (Out-of-Distribution Data).
- Saturación de Memoria por Densidad (VRAM OOM): Si metes 5000 Shitcoins de Kucoin en el grafo, la matriz de adyacencia (NxN) tiene 25 Millones de conexiones. Cargar esto explotará la RAM. Filtrar el universo a los 100 Nodos (Monedas) de alto Market Cap + Volumen que de verdad marcan el ritmo macroeconómico.

## 10. Validaciones obligatorias
- PRE: Chequear que la estructura del Tensor de entrada coincide exactamente con el `Input Shape` (`[Batch, Nodes, Features]`) del archivo ONNX. Un mismatch generará un SegFault (Crash a nivel de C/OS) destruyendo todo el agente Node/Rust.
- CÁLCULO: Validar la Inferencia usando librerías FFI (Foreign Function Interface) para invocar `TensorRT` (NVIDIA GPU) o `OpenVINO` (Intel CPU) acelerado por hardware para lograr tiempos sub-milisegundo en la multiplicación matricial gigante.
- POST: Calibrar empíricamente la salida (Thresholding). Si la GNN dice `Confidence = 0.55` para que BTC suba, y el umbral de disparo es `0.60`, la señal muere silenciosamente. Si dispara a `0.95`, inyectar Evento Prioridad Cero al Orquestador.

## 11. Criterios de aprobación
- Entrega de un array de "Predicciones Direccionales T+1" para los 100 activos principales en < 5 milisegundos usando paralelización de CPU/GPU.
- Supervivencia en Backtesting (Skill 46) con un Hit Rate Direccional estadísticamente significativo (Ej. > 54% PnL Hit Rate contra Random Walk Baseline del 50%).

## 12. Criterios de rechazo
- El modelo fue alimentado con "Target Leaks" (Fuga de la variable objetivo temporal) durante el entrenamiento (Data Science fail clásico), creyendo tener un Hit Rate del 99% que al conectarse en Vivo a Websockets da PnL negativo por delay.
- Intentar usar la salida bruta de la Red Neuronal para ejecutar "Órdenes de Mercado de Tamaño Gigante" sin pasarlo antes por el Risk Engine (Skill 41) y Toxicity Filter (Skill 49). La GNN es un copiloto táctico, no el Gestor de Riesgo absoluto.

## 13. Riesgos que mitiga
- Riesgo Analítico de Silo (Myopic View Trap): Un Bot Clásico de Arbitraje solo mira el par `A` vs `A` en dos exchanges. Es "Miope" al macro-sistema. Si el par `ETH/USDT` ofrece arbitraje, el bot clásico lo toma. Pero si es miope, no vio que `BTC`, `SOL` y `SP500` acaban de desplomarse un 2%. El Spread de `ETH/USDT` es un "Espejismo Tóxico" de un Lagger que caerá en 10 milisegundos. La GNN le da Ojos en la Nuca (360-degree Vision) al bot, rechazando la trampa miópica.
- Efecto Dominó en Alta Frecuencia (Liquidation Cascades): Ver liquidaciones a través de Graph Networks otorga la capacidad inigualable de predecir dónde y cuándo se ejecutarán las próximas llamadas de Margen CEX.

## 14. Integración con otras skills
- Complemento avanzado al Motor de ML Local (Skill 47). Mientras el Skill 47 es para microestructura rápida (Orderbook), este GNN es para Macro-Flujos multi-activos (Ecosistema Global).
- Extrae la Data Semanal y Mensual para su re-entrenamiento del TSDB Influx (Skill 37).

## 15. Modelo de datos sugerido
```json
{
  "GraphNeuralPrediction": {
    "timestamp_ms": 1714521234105,
    "model_hash": "gnn_spatio_temporal_v2",
    "inference_latency_ms": 2.4,
    "global_cascade_risk_score": 0.88, // HIGH ALERT
    "nodal_forecasts": [
      { "node": "BTC", "predicted_delta_bps": -45, "confidence": 0.92 },
      { "node": "ETH", "predicted_delta_bps": -50, "confidence": 0.89 },
      { "node": "USDT", "predicted_delta_bps": 5, "confidence": 0.60 }
    ],
    "action_recommended": "HALT_ALL_LONG_POSITIONS_AND_PREPARE_SHORT_HEDGES"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Worker Híbrido C++/Python/ONNX aislado en su propio hilo de SO que recibe un struct Binario continuo cada 200ms, emite el Forward Pass GNN, y dispara un Callback JSON / Protobuf con el Vector de Salida de V predicciones.

## 17. Logs obligatorios
- `[INFO] GNN Inference Engine initialized. Loaded Graph with 120 Nodes, 14,000 Edges. TensorRT Backend Active.`
- `[DEBUG] GNN Predicts Liquidity Contraction for L2_Tokens subset. GAT Attention Weights shifted heavily towards BTC (Leader Node).`
- `[CRITICAL] GNN MACRO-CASCADE ALERT. Predicted synchronous -5% drop across 80% of graph nodes. Sending Veto Signal to Risk Engine!`

## 18. Métricas obligatorias
- `gnn_inference_speed_ms` (Monitoreo de cuellos de botella del HW).
- `gnn_directional_accuracy_rolling_window_pct` (La exactitud empírica medida 1 segundo después).
- `edge_sparsity_ratio` (Nivel de poda de la red para mantener rendimiento).

## 19. Tests unitarios
- Input Sanitation (NaN Defense): Inyectar un Vector de Características (Features) donde el Nodo "DOGE" tiene valores `Infinity` (Por división por cero de volumen nulo). El Sanitizador Pre-GNN DEBE atraparlo, reemplazarlo con 0 o la Media de su Clúster, y permitir que la inferencia continúe y entregue resultado `Number` para los demás Nodos sin fallar la matriz.
- Speed Validation Threshold: Ejecutar 1,000 inferencias seguidas usando el archivo ONNX en un bucle cerrado. Si la media aritmética de tiempo de inferencia > 10ms, rechazar automáticamente el Commit/Deploy al servidor de Producción por destruir los SLAs de baja latencia del bot.
- Dummy Weights Output: Empujar un Vector de Features Lleno de Ceros absolutos (Mercado literalmente pausado/muerto sin volumen ni cambio). La GNN debe escupir Deltas Predichos muy cercanos a 0.0 sin arrojar predicciones extremas de ruido.

## 20. Tests de integración
- Levantar el motor Data Lake Mock (Skill 37). Configurar el GNN Engine para recibir un Frame cada 50ms (Pipeline de Alta Frecuencia). Verificar usando `htop` o `perf` que el Thread de Inferencia no consume el 100% del CPU central causando Latency Spikes al Orquestador (Skill 36) (Debe ejecutarse en Cores físicos segregados usando `taskset` en Linux).

## 21. Tests E2E
- El agente HFRC detecta que Arbitrum L2 está sufriendo una congestión de red real y el precio de su moneda ARB se desmorona en Kucoin. El par BTC/USDT sigue tranquilo. La GNN ingiere esta micro-anomalía. Su Matriz de Atención aprendida cruza Arbitrum con Optimism, Matic y GMX. Predice con 92% de Confianza que la presión vendedora cruzará el puente en T+500ms hacia los demás ecosistemas L2. El bot emite una alerta, cancela órdenes de Market Making pasivo en GMX y Optimism, y se pone en modo "Taker". 600ms después, el contagio inunda los orderbooks en cadena; el bot se salvó de "Coger Cuchillos Callaendo" (Catching Falling Knives) gracias a la visión predictiva N-Dimensional del tejido de grafos.

## 22. Checklist de producción
- [ ] Incorporación de Cuantización (Quantization FP16 / INT8): Si se corre en CPUs (VPS de AWS estándar), un modelo Float32 matará el RTT. Reducirlo a INT8 con ONNXQuantize acelera un 300% el forward pass perdiendo un insignificante 0.5% de precisión (Vital para HFT).
- [ ] Dynamic Graph Structure: El mercado cripto cambia cada día. Si añades PEPE_USDT a Binance hoy, ¿El modelo estático entrenado hace 1 mes lo entiende? No. Diseñar el sistema GNN de tal forma que acepte "Feature Embeddings" en lugar de "Ids Fijos", o forzar un re-entrenamiento Semanal Mandatorio del Data Lake al Archivo ONNX (CI/CD Pipeline Data-Ops).
- [ ] Descarte Seguro si GNN crashea: Envolver toda la llamada ONNX en un `try-catch` robusto. Si la IA falla, el Bot vuelve instantáneamente al "Modo Clásico Matemático" (Fallback Degradation), jamás crasheando el proceso base.

## 23. Ejemplo de configuración no hardcodeada
```yaml
gnn_forecasting_engine:
  model_path: "./models/tgcn_macro_flow_v2.onnx"
  hardware_backend: "tensorrt" # Can be 'cpu', 'cuda', 'tensorrt'
  inference_frequency_ms: 100 # Feed data into model every 100ms
  cascade_alert_threshold: 0.85
  nodes_monitored_count: 100
  feature_imputation_fallback: "cluster_mean" # How to handle NaNs in live streams
  enable_directional_alpha_trades: false # If True, GNN acts as an Execution signal, not just Veto
```

## 24. Ejemplo de pseudocódigo
```javascript
class GraphNeuralEngine {
    constructor(config) {
        this.session = ONNXRuntime.InferenceSession.create(config.model_path, { executionProviders: [config.hardware_backend] });
        this.featureSanitizer = new LiveImputationFilter();
        this.lastPrediction = null;
    }

    async processFrameAsync(globalGraphFeaturesMatrix) {
        // Run in detached async worker so we don't block the HFT Math logic
        return new Promise((resolve) => {
             // 1. Sanitize NaNs and Infs (Protect Matrix Multiply)
             const safeMatrix = this.featureSanitizer.clean(globalGraphFeaturesMatrix);
             
             // 2. Wrap as Tensor [Batch=1, Nodes=100, Features=5]
             const inputTensor = new ONNXRuntime.Tensor('float32', safeMatrix, [1, 100, 5]);
             
             // 3. Inference (HW Accelerated)
             this.session.run({ input: inputTensor }).then(output => {
                 this.lastPrediction = this.decodeOutput(output);
                 this.evaluateCascadeRisk(this.lastPrediction);
                 resolve(this.lastPrediction);
             });
        });
    }

    evaluateCascadeRisk(predictionVector) {
        // If 80% of nodes show a severe drop prediction > -2%
        let droppingNodesCount = 0;
        for (let node of predictionVector) {
             if (node.delta < -0.02) droppingNodesCount++;
        }
        
        const riskRatio = droppingNodesCount / 100.0;
        if (riskRatio >= CONFIG.cascade_alert_threshold) {
             EventBus.emit('GNN_CASCADE_PANIC', riskRatio);
        }
    }
}
```

## 25. Criterio final de excelencia
Las Graph Neural Networks convierten al bot de un reactivo de primera línea a un Omnisciente de nivel de ecosistema. Capta las resonancias y fricciones del capital que viajan entre los activos como ondas sísmicas, protegiendo al capital con escudos predictivos antes de que la orden enemiga impacte en el puerto de tu red local, cruzando la barrera entre Matemática Discreta y Topología Deep Learning Cuantitativa.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Concept Drift Acelerado en correlaciones de Cripto (Un hack a gran escala cambia el comportamiento del grafo permanentemente en 1 segundo, invalidando los pesos).
- Dependencias: Data Normalization (Skill 32), Data Lake (Skill 37) y C++ ONNX Bindings.
- Próxima skill: Market Making Asimétrico (Bid/Ask skewing) (Skill 57).
