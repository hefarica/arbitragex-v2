# SKILL 032 — Normalización de datos multi-fuente

## 1. Propósito superior
Actuar como la Torre de Babel universal del sistema de arbitraje. Los exchanges y protocolos DeFi utilizan decenas de formatos de datos distintos, nomenclaturas de pares divergentes y bases numéricas incompatibles. Esta skill ingiere el caos en tiempo real y expulsa un Flujo de Datos Unificado, Estandarizado y Tipado, asegurando que las funciones matemáticas puras (Skill 1) comparen manzanas con manzanas sin importar si provienen de Binance, de Uniswap o de un WebSocket oscuro.

## 2. Nivel de conocimiento requerido
Experto en Arquitectura de Datos de Alta Velocidad, Estructuras de Datos Universales (Canonical Data Models) y Algoritmia de Mapeo (String Interning, Hash Maps). Dominio de limpieza de strings, resolución de conflictos de tickers (Ticker Collision) y aritmética de "Float to BigInt" segura.

## 3. Capacidades principales
1. Normalización de Tickers: Traducir `BTC-USDT` (Kucoin), `BTCUSDT` (Binance), `XBTUSDT` (Kraken) y `WBTC/USDC` (DeFi) a una ID Universal interna `BTC_USDT_NORM`.
2. Normalización Decimal (DeFi a CeFi): Convertir la representación de un precio DeFi (`reserve_usd / reserve_token` escalado por `1e18`) a un número flotante puro temporal, y convertir formatos JSON Float de CeFi a BigInt (wei-equivalents) para matemáticas de precisión.
3. Estandarización de Sides (Dirección): Mapear enumeradores extraños (`1`/`2`, `"bid"`/`"ask"`, `"buy"`/`"sell"`) hacia un estricto `OrderSide.BUY` o `OrderSide.SELL`.
4. Extracción de Tiempo (Timestamp alignment): Sincronizar marcas de tiempo. Unos envían milisegundos (`1680000000000`), otros nanosegundos y otros segundos. Todo se estandariza a milisegundos absolutos Unix.
5. Ingesta de Payload Híbrido: Analizar el `event_type` de múltiples CEX para discernir si un mensaje es "Snapshot" completo del order book, o un "Delta" (Actualización).
6. Prevención de Colisión de Nombres (Naming Collisions): Distinguir entre `$FRONT` (Frontier) y `$FRONT` (Falso token SCAM) basándose en contexto adicional provisto por el adaptador del exchange.
7. Mapeo de comisiones (Fees struct): Adosar al dato normalizado la comisión estándar del exchange origen pre-calculada.
8. Filtrado de Polvo (Dust filtering): Si la normalización revela que la liquidez disponible en Binance es de 0.000000001 BTC, dropear el dato por irrelevante antes de pasarlo al Order Book.
9. Traducción de Eventos (Event Mapping): Convertir `Order_Filled`, `Order_Canceled`, `Order_Partially_Filled` a estados internos de State Machine unificados.
10. Caché Local de Diccionarios: Carga rápida en memoria O(1) de los mapeos (`XBT` -> `BTC`) sin consultas asíncronas pesadas.

## 4. Entradas requeridas
- `raw_payload`: Cadena JSON o Buffer binario (e.g. BSON/Protobuf) recibido del WebSocket.
- `source_identifier`: Origen de la data (`binance`, `kraken`, `uniswap_v3`).
- `reference_dictionaries`: Reglas de mapeo cargadas al iniciar el sistema.

## 5. Salidas esperadas
- `canonical_data_object`: Objeto tipado (ej. `NormalizedOrderBookUpdate`, `NormalizedTrade`).
- `processing_error`: Notificación silenciosa si el payload es inútil o está corrupto.
- `discarded_flag`: Booleano para descartar ruido.

## 6. Reglas inmutables
- Nunca ejecutar bucles O(N) para buscar símbolos en el diccionario. Utilizar estrictamente Mapas Hash (`Map` en JS, `HashMap` en Rust) para complejidad O(1). Las normalizaciones ocurren miles de veces por segundo; cada microsegundo cuenta.
- TODA la precisión fraccionaria (Strings como `"0.0000034"`) debe tratarse evitando el parseo con `parseFloat()` nativo a menos que sea seguro para el rango; preferible usar calculadoras decimales o escalar a Enteros (`BigInt`) dependiendo de su destino final.
- Si el "source_identifier" no coincide con ningún Adapter registrado, el sistema arroja alerta estructural e ignora la data, bloqueando inyecciones no reconocidas.

## 7. Algoritmos o métodos que debe conocer
- Punteros de Diccionario (Symbol Interning).
- Deserialización Optimista vs Pesimista (Asumir la estructura correcta en try/catch rápido frente a validar campos profundamente `Zod`/`Joi` que añade latencia inaceptable para HFT).
- Data casting seguro.

## 8. Fórmulas críticas
- **Conversión de Escala DeFi/CeFi**: `Precio_BigInt = DecimalString_a_BigInt(Precio_JSON) * Escala_Universal_Defi`
- **Tolerancia de Timestamp**: `Abs(Timestamp_Exchange - Timestamp_Local) > 500ms` -> Marcar como `Stale`.

## 9. Casos extremos
- Cambios de Ticker del Exchange sin avisar: Binance migra `BUSD` a `FDUSD` o cambia un nombre (Ej. `MATIC` a `POL`). El diccionario antiguo falla, las normalizaciones arrojan "UNKNOWN_SYMBOL" en masa. El sistema debe lanzar alerta de reconfiguración y suspender arbitraje para esa ruta.
- Payload Polimórfico: OKX envía bajo el mismo stream un array de strings si es un tipo de error, y un array de objetos si es data. Un parser rígido crashea de inmediato.
- Floats malformados: Recepción de `"1.000.00"` (Fallo interno del servidor del exchange), crasheando la aritmética posterior.

## 10. Validaciones obligatorias
- PRE: Validar integridad del payload. Un evento vacío `{}` debe ser descartado en la primera línea del pipeline (`if (!data.length)`).
- CÁLCULO: Validar la orientación Bid/Ask de manera rígida. Un error en normalizar Bid por Ask causa que el bot intente comprar caro y vender barato.
- POST: Incorporar "Timestamp local de recepción" (`received_at`) al objeto canónico para que las skills posteriores calculen latencia de transporte.

## 11. Criterios de aprobación
- El objeto saliente cumple con la interfaz estricta (Interface TypeScript/Rust) que exige el Motor Matemático.
- La latencia de conversión `Raw -> Canonical` es de `< 0.1ms`.

## 12. Criterios de rechazo
- El Ticker del exchange no está registrado en el `Universal Dictionary` del bot.
- Falta de campos obligatorios (Ej. Faltan los volúmenes, solo vienen los precios).

## 13. Riesgos que mitiga
- Riesgo de Fragmentación Lógica (Spaghetti Code): Sin normalización, cada Skill tendría `if (exchange == "binance") { ... } else if (exchange == "kraken") { ... }`. Esto hace el bot inmantenible y frágil a cualquier adición de un nuevo exchange.
- Riesgo de Precisión Numérica (Slippage Matemático Falso): Comparar un número de 6 decimales con uno de 8 decimales erróneamente multiplicados provoca órdenes enviadas con precios inválidos (HTTP 400 Bad Request).

## 14. Integración con otras skills
- Es el puente directo entre WebSockets (Skill 31) y el Seguimiento de Order Book (Skill 33).
- Proporciona la estructura canónica requerida por la Detección de Ciclos (Skill 4).

## 15. Modelo de datos sugerido
```json
{
  "CanonicalOrderBookUpdate": {
    "universal_symbol": "BTC_USDT",
    "source_exchange": "kraken",
    "received_at_ms": 1714521234105,
    "exchange_timestamp_ms": 1714521234101,
    "is_snapshot": false,
    "bids": [[64100.5, 0.55], [64100.0, 1.2]],
    "asks": [[64101.0, 0.20], [64101.5, 3.4]],
    "status": "VALID_NORMALIZED"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Patrón "Adapter/Factory": Una interfaz base `IExchangeAdapter` con implementaciones concretas (ej. `BinanceAdapter.normalizeOrderBook(raw_json)`).

## 17. Logs obligatorios
- `[DEBUG] Normalized Kraken WS Msg: XBTUSD -> BTC_USDT. 5 Bids, 5 Asks parsed. Latency: 0.05ms.`
- `[WARN] Normalization failed. Unrecognized Symbol from OKX: "MEME-COIN-FAKE". Discarding payload.`
- `[CRITICAL] Payload parse error in Bybit Adapter. Malformed float detected. Dropping chunk.`

## 18. Métricas obligatorias
- `normalization_throughput_mps` (Messages Per Second, debe escalar a > 10,000 mps sin lag de Event Loop).
- `unrecognized_symbols_count` (Si aumenta repentinamente, faltan mapear listados nuevos).
- `normalization_latency_us` (Medido en microsegundos).

## 19. Tests unitarios
- Ticker Matching: Testear la entrada `XBTUSDT` frente al método del adaptador de Kraken y asegurar que devuelve `BTC_USDT`.
- Conversión de Tiempo: Inyectar timestamps en nanosegundos (19 dígitos) y en segundos (10 dígitos). El normalizador debe detectar y estandarizar ambos a milisegundos de 13 dígitos correctamente.
- String Float Padding: Validar que `["60000", "0.5"]` es procesado aritméticamente igual que `["60000.00", "0.5"]`.

## 20. Tests de integración
- Cargar un JSON de Binance que contiene actualizaciones de Orderbook (Deltas `u` y `U`) mezclado con eventos `24hr_ticker`, y asegurar que el Factory los rutea, desecha el ruido, y emite solo el objeto canónico de OrderBook limpio.

## 21. Tests E2E
- Conectar 3 simuladores Websocket que inyectan datos de Kraken, Binance y Uniswap a un solo bus de datos del Agente. Observar al final de la línea cómo el motor de arbitraje imprime operaciones usando una sola estructura lógica ignorando de qué red vinieron originalmente.

## 22. Checklist de producción
- [ ] Incorporación de un Script Offline que actualice los `Dictionaries` y `Scale Decimals` una vez al día llamando a las APIs REST `/exchangeInfo` de todos los CEX.
- [ ] Uso de validaciones manuales (`if typeof param === 'string'`) en lugar de pesadas validaciones de esquemas tipo JSON-Schema que matan la performance HFT.
- [ ] Descarte inmediato de operaciones "Dust" (Cantidades < Min Notional) directamente en el proceso de normalización para no ocupar RAM del OrderBook en niveles invisibles.

## 23. Ejemplo de configuración no hardcodeada
```yaml
normalization_engine:
  ignore_unmapped_symbols: true
  drop_dust_bids_asks: true
  min_volume_threshold_usd: 10.0 # Bids/Asks smaller than this are filtered out
  timestamp_drift_tolerance_ms: 1000
```

## 24. Ejemplo de pseudocódigo
```javascript
class BinanceAdapter {
    constructor(dictionary) {
        this.dict = dictionary; // HashMap O(1)
    }

    normalizeOrderBookDelta(rawJson) {
        const canonicalSymbol = this.dict.get(rawJson.s);
        if (!canonicalSymbol) return null; // Drop unmapped

        const now = Date.now();
        // Discard massively delayed messages (Drift protection)
        if (now - rawJson.E > 1000) return null;

        return {
            symbol: canonicalSymbol,
            exchange: 'BINANCE',
            receivedAt: now,
            exchangeTime: rawJson.E, // Event time
            isSnapshot: false,
            bids: rawJson.b.map(level => [parseFloat(level[0]), parseFloat(level[1])]),
            asks: rawJson.a.map(level => [parseFloat(level[0]), parseFloat(level[1])])
        };
    }
}
```

## 25. Criterio final de excelencia
El normalizador actúa como un agujero negro que absorbe el caos y escupe orden perfecto. Opera de forma tan rápida y silente que el resto de las matemáticas e integraciones asumen felizmente que todo el ecosistema crypto mundial utiliza el mismo estándar unificado y limpio, simplificando el desarrollo del 90% del bot.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Formatos indocumentados por parte de los CEX (Cambio de API inesperado que rompe el parsing). Se requiere fail-safe y try/catch rápido a nivel global.
- Dependencias: Diccionario Central de Símbolos.
- Próxima skill: Order book bid/ask tracking (Skill 33).
