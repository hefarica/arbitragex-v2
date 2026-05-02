# SKILL 073 — Análisis de Sentimiento NLP (Twitter/News Scraper HFT)

## 1. Propósito superior
Incorporar datos No Estructurados (Texto Humano L2 O(1)) al Motor Predictivo de Inteligencia Artificial (Skill 47 y 56 L2 O(1)). Los Orderbooks L2 dicen *cuánto* van a comprar los Traders, pero Twitter y las Noticias (Bloomberg, Coindesk L1 L2) dicen *POR QUÉ* van a comprar. Esta Skill es un rastreador en tiempo real de Natural Language Processing (NLP HFT O(1)). Escanea masivamente WebSockets de Twitter (X), Telegram VIP Channels, y APIs de Noticias Cripto L2 O(1), traduciendo el "Pánico", "Euforia" o "Anuncios Oficiales (Listings)" en Vectores Matemáticos (Sentiment Z-Scores L2 O(1)) inyectados atómicamente a la Máquina de Arbitraje CEX.

## 2. Nivel de conocimiento requerido
NLP Engineer / Data Scientist L2 HFT. Conocimiento en Modelos Transformers Ligeros (RoBERTa L2, DistilBERT, FinBERT O(1) in-memory C++), Scrapeo Distribuido (Proxies Residenciales), Procesamiento Semántico de Entidades Nombradas (NER HFT L2 Cripto para detectar $TICKERS), y Arquitectura de Señales Event-Driven (News Sniping L2 Cripto O(1)).

## 3. Capacidades principales
1. Event Sniping Cripto O(1) (Coinbase/Binance Listings L2): Los grandes Pumps HFT Cripto ocurren cuando Binance anuncia "Listing de Símbolo X L2". La Skill se conecta al WebSocket/API no documentado de Anuncios CEX L2. Al detectar el Texto "Listaremos $COIN", la inferencia NLP lo confirma en 1ms O(1). El Bot HFT (Skill 64 L1 L2) compra Atómicamente `$COIN` en todos los DEX descentralizados HFT antes de que los humanos lean el tweet.
2. Extracción Semántica NER (Entity Recognition L2 O(1)): Lee un Tweet de Elon Musk "Amo a Floki". No busca solo la palabra Floki L2. El Modelo Transformer Local (ONNX C++) Extrae la Entidad `$FLOKI` y le asocia el Sentimiento `EUFORIA EXTREMA +0.95`. Emite Evento Vectorial al XGBoost Cripto L2 O(1).
3. Filtrado Anti-Spam/Bots Cripto (Trust Scorer L2 O(1)): Twitter está lleno de Bots Scammers diciendo "Buy $SHIB 1000x". Si el Scraper inyecta esto, el HFT Bot Quebrará L1 L2. La Skill pondera (Trust Weighting L2 O(1)) la Señal NLP por Reputación del Autor (Ej. @VitalikButerin = Weight 100.0, BotRandom_01 = Weight 0.00). Filtro Bayesiano Cripto O(1) Antirruido L2.
4. Tensor Transformation (Sentiment a Vector Float32 O(1)): Traduce el caos del Texto Humano L1 L2 en 3 métricas limpias para el Feature Store (Skill 70 L2 O(1)): `Bullish_Score`, `Bearish_Score`, y `Social_Volume_Acceleration_L2_O1`.
5. Detección de Exploits/Hacks (Panic Event L1 L2 O(1)): Escanea canales de Seguridad Cripto (PeckShield, CertiK Alerts L1). Si lee "Protocolo X explotado por $50M L1", dispara una Venta en Corto (Short Perp Hedger Skill 61 L2 O(1)) o desactiva las operaciones LP V3 L1 (Skill 63 O(1)) en ese protocolo Atómicamente L1 L2 Cripto.
6. Rumor vs Fact Classifier L2 (Análisis de Certeza HFT): El Modelo ONNX FinBERT O(1) clasifica los textos como "Hecho Consolidado (Fact)" o "Especulación Aleatoria (Rumor)". Los Hechos inyectan Alpha determinista Cripto L2, los Rumores inyectan Volatilidad (Alpha para Market Making Skill 57 L2 O(1) y Options Skill 65 L2).
7. Discord y Telegram VIP Parsing (Alpha Groups L2 O(1)): Escucha Canales Cerrados/Privados L1 L2 de Señales Cuantitativas HFT a través de Cuentas Sincronizadas Telethon/Discord.js L2. Extrae Comandos L2 Cripto y los mapea con el Risk Engine HFT (Copy-Trading Institucional O(1)).
8. Predicción de Anomalías L2 Cripto (Silence Detection L2): Si una moneda solía tener 10,000 tweets por hora L2 Cripto y de repente cae a 10 tweets/hora (Muerte Social L2 O(1)). La Skill L2 emite Bandera Roja al Módulo de Liquidez (Skill 59 L2) advirtiendo "Spread Inminente por Abandono Social L2 O(1)".
9. Macro-Sentiment Index (Fear & Greed Local L2 O(1)): Computa un Índice de Pánico General O(1) evaluando Noticias Bloomberg Crypto. Si el índice marca Extremo Pánico L2 O(1), Desactiva estrategias Mean-Reverting L2 (Skill 55 Pairs) y Enciende Estrategias Momentum/Hedging HFT (Skill 61 L2 O(1)).
10. Low-Latency Pipeline C++ (Cero Python O(1)): Los News Snipers clásicos usan Python Requests y Spacy (Latencia 200ms L2). HFRC usa Rust/C++ Tokenizers y Reducción de Modelos (Quantization INT8 L2 O(1)). Clasifica texto NLP y dispara orden API CEX en `< 5ms L2 O(1)`.

## 4. Entradas requeridas
- `social_media_streams_api_o1`: Webhooks L2/Websockets conectados a Twitter Firehose, Telegram MTProto L2, y RSS Bloomberg L2.
- `target_asset_universe_l2_o1`: Lista de 1000 Monedas CEX L2 (Para que el Filtro Regex Cripto ignore el resto O(1)).
- `finbert_onnx_model_l2_o1`: Modelo de IA Transformer (HuggingFace Cripto) Compilado Localmente y Cuantizado INT8 L2 O(1).

## 5. Salidas esperadas
- `nlp_feature_tensor_l2_o1`: Un Vector O(1) inyectado al Feature Store (Skill 70 L2).
- `atomic_news_event_trigger_l2_o1`: Alarma Ejecutiva O(1) que Evade Modelos ML y Envía Orden de Compra/Venta MUX (Skill 64 L2 O(1)) directa y atómica L2 (Ej. Listing Event Sniper O(1)).
- `security_pause_signal_l1_o1`: Si detecta Hack/Scam, Veta el Contrato Inteligente L1 (Skill 68 L1 L2).

## 6. Reglas inmutables
- Nunca ejecutar un "Event Sniper L2 O(1) Atómico" basado en la cuenta de un Influecer Menor (Baja Seguridad Cripto L2). El Sniper Directo Solo dispara en Fuentes Primarias Criptográficas Mapeadas O(1) (Ej. API Oficial Binance, Twitter de @SEC, @Tether_to). Fuentes menores SOLO alimentan el Tensor XGBoost Predictivo L2 O(1), no gatillan comandos atómicos HFT Cripto O(1).
- Inferencias Fuera del Hilo Principal (Off-Main-Thread NLP O(1)). Correr Redes Neuronales NLP (Transformers L2 O(1)) congela el Event Loop de Node/Rust. La Inferencia FinBERT O(1) DEBE ejecutarse en ThreadPools Aislados C++ (WebWorkers / Rayon Rust L2 O(1)), devolviendo Promesas Async L2 sin retrasar 1 solo Tick del Orderbook CEX L2 HFT O(1).

## 7. Algoritmos o métodos que debe conocer
- Transformer Models (DistilRoBERTa, FinBERT Cripto INT8 OnnxRuntime O(1)).
- Named Entity Recognition (CRF o FastText Embeddings L2 O(1)).
- TF-IDF Cripto Adaptado y N-Gram Hashing (Detección de Spam/Copypasta O(1) L2).

## 8. Fórmulas críticas
- **Social Impact Score L2 O(1)**: `Impact = Base_Sentiment_Score * Log(Author_Followers) * Viral_Velocity_Retweets` (Pesaje Cripto Institucional O(1)).
- **Event Sniper Delta L2 O(1)**: Si `Texto contiene ("List" AND "Binance") AND NER = Símbolo_Valido` -> Gatilla Latencia 0ms L2 MUX Compra DEX L1.

## 9. Casos extremos
- Account Takeover / Hack (Cisne Negro Falso L2 O(1)): Hackean el Twitter del SEC (Ocurrió con el ETF L2 O(1)). Tuit falso: "Aprobamos el ETF Cripto L2". El Bot compra a mercado 10 Millones HFT L2. El SEC Borra el Tweet y dice que fue Hackeado L2 O(1). Mercado Colapsa L2. El Bot muere Cripto O(1). Solución In-Memory: Todo Event Sniping Masivo (Macro L1 L2 O(1)) exige Corroboración de Dominio Cruzado L2 O(1) (Cross-Domain Verification L2). Ej. Si SEC Twittea, la API Oficial .gov RSS DEBE reflejarlo HFT O(1). De no ser así, el Sizing de Riesgo (Kelly Criterion L2) capa la exposición a Céntimos HFT L2 (Riesgo Controlado O(1)).
- Ticker Confusion (Colisiones Léxicas NLP O(1)): Elon Musk Twitea "Me encanta el cielo AZUL L2 O(1)". El Bot Extrae "AZUL L2" y compra la Criptomoneda `$AZUL` L1 (Meme Coin L1 L2 O(1)). (Falso Positivo Fatal NLP L2 O(1)). Solución NER Cripto O(1): FinBERT es re-entrenado L2 O(1) Localmente (Fine-Tuning L2) con Contexto Financiero Cripto L1 L2. Para activar una Compra HFT L2 O(1), el Tokenizer debe hallar Contexto Sintáctico (`Amo el TOKEN AZUL` O(1) o `$AZUL` Casings Strict L2 O(1)).
- API Rate Limits Sociales L2 O(1): Twitter corta las APIs. El Bot HFT queda ciego L2. Arquitectura requiere Web Scrapers Puros L2 (Puppeteer/Playwright Headless L2 O(1)) rotando IPs Sybil (Skill 71 O(1)) inyectando JavaScript DOM Parsing O(1) a Velocidad Luz L2 evadiendo Captchas para garantizar la entrada de Datos NLP L2 HFT O(1).

## 10. Validaciones obligatorias
- PRE: Chequeo de Caché de Textos L2 O(1). Twitteros Spamean el mismo Tweet 500 veces. El Hash `SHA256(Texto) L2 O(1)` se valida contra Redis L2 O(1). Si es repetido, el NLP Model No Infiere (Ahorro Masivo CPU Cripto L2 O(1) HFT).
- CÁLCULO: Tensor Normalization Cripto L2. La Salida de Sentiment `[-1.0 a 1.0]` L2 O(1) y `Social Volume` `[0 a Infinito]` L2 O(1) DEBEN enviarse al Feature Store (Skill 70 L2 O(1)) para recibir Z-Score Standarization L2 antes de Tocar la Red XGBoost L2 O(1) Cripto HFT.
- POST: Si una Inferencia Disparó un Trade Atómico L2 (Snipe Cripto O(1)), se levanta bandera HFT L2: "Lock Posición L2 por N Milisegundos". Prevenir que la Skill 47 (XGBoost L2) o Risk Engine (Skill 41) vendan instantáneamente la Moneda creyendo que es un Pump Aleatorio (El NLP Engine debe anular Cripto las Ventas HFT por Momentum Algorítmico temporal O(1)).

## 11. Criterios de aprobación
- Extracción de un Tweet L2 O(1), Parseo de Entidad (NER Cripto O(1)), Inferencia de Sentimiento L2 (ONNX FinBERT O(1)) en `< 15 milisegundos O(1) RAM In-Memory`.
- Capacidad de diferenciar Exitosamente (En Muestras Test Cripto L2 O(1)) un Mensaje Sarcástico ("Great, SHIB is dumping again L2 O(1)") de uno Literal, inyectando Pesos Bajistas al Tensor L2 Feature O(1).

## 12. Criterios de rechazo
- Usar OpenAI / GPT-4 API L2 O(1) para procesar el sentimiento. Las APIs en la Nube de LLMs tardan entre 1000ms y 5000ms L2 (Latencia de Red + Generación LLM Cripto L2). En HFT L2, 1 Segundo es inaceptable. Obligatorio el uso de Modelos SLMs (Small Language Models L2 O(1) 50-100MB) Cuantizados corriendo Localmente en CPU/GPU del Servidor de HFT L2 O(1).
- Inyectar el Ruido Crudo L2 O(1). (Ej. Mandar la cantidad de Retweets L2 directo al Bot). La Competencia manipula Retweets comprando Granjas Sociales L2 Cripto. El Bot Mapea y descarta Engagement Inflado O(1) midiendo Dispersión Genuina de Orígenes O(1) L2.

## 13. Riesgos que mitiga
- La Miopía del Alpha Cuantitativo L2 O(1) (Blind HFT Traps L2). Los Orderbooks y los Gráficos L2 CEX muestran el *pasado o el presente inmediato*. NO muestran Información Clasificada/Noticias HFT O(1). Si el FBI anuncia que confiscará Tether L1 L2, el Orderbook Tarda 5 Segundos en Desplomarse (Human Reaction L2). Este Módulo NLP NLP Lee el RSS del FBI L2 O(1) en Milisegundos Cripto, Shortea Cripto HFT Atómicamente (Skill 61 L2 O(1)), ganando Arbitraje Espacial-Temporal Masivo y sobreviviendo Armagedones de Información (Information Asymmetry Alpha L2 O(1)).

## 14. Integración con otras skills
- Alimentador de Alpha Alternativo para el Feature Store ML (Skill 70 L2 O(1)).
- Gatillo Atómico del Orquestador Híbrido MUX (Skill 64 L1 L2 O(1)).
- Módulo Protector Anti-Hack L1 (Skill 68 y Risk Engine 41 L2 O(1)).

## 15. Modelo de datos sugerido
```json
{
  "NLPSentimentAndSniperEngineL2_O1": {
    "event_id": "SNIPE_BINANCE_LISTING_TWEET_O1",
    "timestamp_ms_o1": 1714521234105,
    "source_o1": "twitter_api_vip_stream",
    "author_trust_score_o1": 99.5, // @binance official
    "raw_text_l2": "Binance will list Magic (MAGIC) in the Innovation Zone...",
    "nlp_inference_o1": {
      "detected_entities_l2": ["MAGIC"],
      "sentiment_score_l2_o1": 0.98, // Ultra-Bullish HFT
      "classification_event_type_o1": "EXCHANGE_LISTING_CONFIRMED_L2",
      "inference_time_ms_o1": 4.2 // Sub-5ms C++ ONNX FinBERT Local HFT
    },
    "action_triggered_l2": "ATOMIC_MARKET_BUY_MUX_L1_DEX_O1", // Pre-empt the CEX
    "status": "SIGNAL_DELIVERED_TO_HFT_ROUTER_O1"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Python/Rust Cripto Local L2 `FinBERT_Inference_Worker_O1`. Conectado vía ZeroMQ o SharedArrayBuffer Cripto al Motor HFT de Node.js. Procesa Webhooks JSON entrantes, clasifica Texto, Retorna Struct Cripto O(1) con Sentimientos.

## 17. Logs obligatorios
- `[INFO] NLP Engine L2 O(1): Binance News Scraped. Keyword "Listing" and Ticker "GMX" identified with 99% Confidence O(1). Firing Atomic Mux Buy L1 O(1) to Uniswap V3 Arbitrum before the Retail Herd L2 Cripto arrives.`
- `[DEBUG] Sentiment Streaming L2: Social Momentum on SHIB is deteriorating. 1m Social Volume Z-Score fell to -3.5. Emitting Bearish NLP Tensor to Skill 70 Feature Store L2 O(1).`
- `[CRITICAL] SMART CONTRACT HACK DETECTED BY PECKSHIELD L1 O(1)! NLP Parser identified "Exploit" and "Curve Finance L1". Immediately dispatching EMERGENCY WITHDRAWAL AND SHORT HEDGE O(1) to Risk Engine (Skill 41 L2). Saved $1.5M AUM L2 from Hack Event L1 O(1).`

## 18. Métricas obligatorias
- `average_nlp_inference_latency_ms_o1` (Mantener debajo de 10ms O(1) HFT).
- `sentiment_accuracy_vs_price_lag_l2_o1` (Correlación de Pearson L2: Verifica si tus inferencias NLP sirven para predecir Precio HFT O(1) en el Backtest).
- `atomic_snipes_executed_lifetime_l2_o1`.

## 19. Tests unitarios
- FinBERT Accuracy and Speed O(1) Test: Inyectar Array de 50 Noticias Históricas Cripto (Mitad Bullish L2, Mitad Hacks Bearish L1 O(1)). El Worker C++ ONNX O(1) DEBE procesar los 50 Strings HFT en `< 100ms O(1) Totales`. y clasificar correctamente con F1-Score > 0.90 Cripto L2.
- Anti-Spam Entity Confusion Test L2 O(1): Inyectar Tweet Falso: `We will list @ScamCoin to the moon $AAPL 1000x`. El modelo NER DEBE rechazar `AAPL` (No es Cripto L2) y catalogar el Tuit como `SPAM_TRUST_0` por heurísticas Hashing O(1), bloqueando Falsos Positivos Fatales HFT.

## 20. Tests de integración
- Levantar Subproceso ONNX HFT L2 Cripto y WebSocket de Origen Falso O(1) (Mock Twitter). Enviar Tuit Oficial HFT "Binance Lists $PEPE". Medir Tiempo Exacto Cripto O(1) desde que el Mock emite el Socket, hasta que el MUX Dispatcher (Skill 64 L1 L2) escupe el CallData C++ EVM "Buy PEPE L1". El Latency HFT Global Cripto L1 L2 debe superar Test de Estrés (< 15 Milisegundos O(1)).

## 21. Tests E2E
- El agente HFRC Cripto O(1) opera Market Making Pasivo L2 (Skill 57 L2) en 500 pares de Kucoin L2. Todo tranquilo. De Repente, Reuters Cripto O(1) emite un Flash en RSS L2: "China Bannea las Criptomonedas Oficialmente L2". El Ordenador de Precios CEX L2 AUN NO HA CAIDO (Los humanos tardan segundos en leer L2 Cripto). La Skill 73 (NLP O(1)) toma el XML/RSS L2 HFT O(1), lo Infiere con el Transformer Local L2 C++ Cripto en 3ms O(1). Emite Signal `EXTREME_PANIC_MACRO_BEARISH_L2_O1`. El Risk Engine (Skill 41 L2 O(1)) despierta Atómicamente. Cancela O(1) TODAS Las Compras Maker HFT L2 In-Memory (Skill 69 O(1)). Emite Market Sell (Dump L2 O(1)) o Short Perp L2 (Skill 61 O(1)) del 80% del Portfolio Cripto. 1 Segundo después, el Mundo Retail lee la noticia L2 y Dumpea. El Precio cae -15% L2. El Bot HFRC se Salvó del Crash HFT O(1) y Quedó Short, Ganando Millones de Dólares HFT en Segundos gracias a la Inferencia Asimétrica de Texto Natural a Velocidad Algorítmica CEX L2.

## 22. Checklist de producción
- [ ] Conexiones Directas B2B API L2 O(1) (Evitar Scraping Inestable Cripto): Los Scrapers (Puppeteer L2) se rompen cuando Twitter cambia el HTML L2 Cripto. Invertir en Endpoints Financieros Dedicados (Ej. Bloomberg Terminal API, The TIE, LunarCrush API Enterprise L2 O(1)) recibiendo Webhooks puros JSON HFT, estabilizando el Input O(1) de Latencia L2 Institucional.
- [ ] Cuantización y Poda del Modelo NLP (ONNX INT8 L2 O(1)): No usar un modelo Transformer FP32 de 2GB L2 O(1). Usa PyTorch L2 para Cuantizar el Modelo a `INT8` O(1) y podar atenciones inútiles (Pruning L2 Cripto). Baja el tamaño del modelo a 50MB L2 y reduce la latencia de Inferencia C++ a 2 Milisegundos O(1) (Magia Cuantitativa HFT Cripto).

## 23. Ejemplo de configuración no hardcodeada
```yaml
nlp_sentiment_hft_engine_l2_o1:
  enable_event_sniping_l2_o1: true
  enable_xgboost_feature_injection_l2_o1: true
  onnx_finbert_model_path_local_o1: "/opt/hfrc/models/finbert_quantized_int8_hft.onnx"
  trusted_sniping_sources_whitelist_l2_o1: ["@binance", "@coinbase", "@secgov", "rss_bloomberg_crypto"]
  max_inference_latency_budget_ms_o1: 15
  sentiment_exponential_decay_half_life_ms_o1: 3600000 # Sentiment feature fades after 1 hour Cripto L2
```

## 24. Ejemplo de pseudocódigo
```javascript
// C/Rust Bound ONNX Worker Process L2 O(1)
class NlpEventSniperO1 {
    constructor(onnxEngineC, routerMuxL1L2) {
        this.ai = onnxEngineC;
        this.router = routerMuxL1L2;
        this.TRUSTED_SOURCES = new Set(CONFIG.trusted_sources);
    }

    // Called asynchronously immediately upon Webhook/Socket JSON L2 arrival
    async onSocialTickL2_O1(source, author, textContent) {
        // Fast Bypass if source is irrelevant
        if (this.isSpamL2(author, textContent)) return;

        // O(1) C++ ONNX FinBERT Inference (Zero-Python IPC Latency)
        const inference = this.ai.inferSentimentAndEntities(textContent);
        
        // 1. Extreme Alpha Sniper Cripto L2 O(1)
        if (this.TRUSTED_SOURCES.has(author) && inference.event_type === 'LISTING' && inference.entities.length > 0) {
             const targetAsset = inference.entities[0];
             log.critical(`NLP SNIPER O(1): Listing detected for ${targetAsset} by ${author}. FIRING ATOMIC BUY L2!`);
             // Dispatch direct MUX without asking ML Model
             this.router.dispatchOptimalMuxSplit(targetAsset, CONFIG.sniper_sizing_usd, 'BUY');
        }

        // 2. Continual Tensor Ingestion L2 O(1) (For ML Predictor)
        FeatureStoreO1.injectNlpSignalL2(
            inference.entities, 
            inference.sentiment_score, 
            inference.certainty
        );
    }
}
```

## 25. Criterio final de excelencia
El Analizador de Sentimiento NLP dota al Bot HFRC HFT Cripto de "Comprensión Lectora Humana" a Velocidad Criptográfica L2 O(1). Cierra la última frontera del Arbitraje Algorítmico: Extraer Alpha Matemático O(1) no de Números o Precios Cripto, sino de las Emociones y Palabras del Mundo Real. Provee al Orquestador de una Inmunidad Profética contra Hacks y Disparos pre-cognitivos en Anuncios Institucionales, separando a los Quants Cripto Matemáticos Puros (Ciegos a la realidad Social L2) de los Apex Predators Omniscientes HFT Cripto L1 L2.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: AI Hallucinations (Falsos Positivos O(1) Cripto L2). El NLP C++ se equivoca e interpreta un artículo "Tether NO ha sido Hackeado" como "Tether Hackeado" (Fallo de Negación Sintáctica L2 O(1)). Mitigado con Fine-Tuning Avanzado L2 y umbrales de Certeza (`Confidence > 0.98`) L2 HFT O(1).
- Dependencias: ONNX Runtime Engine (C++ FFI L2 O(1)), External Socket Data feeds.
- Próxima skill: Estrategia de Liquidation Sniping (Aave/Compound Liquidators) (Skill 74).
