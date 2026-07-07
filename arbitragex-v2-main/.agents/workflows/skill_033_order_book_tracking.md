# SKILL 033 — Order book bid/ask tracking

## 1. Propósito superior
Mantener una réplica local, ultra-rápida y matemáticamente perfecta de los Libros de Órdenes (Order Books) de múltiples exchanges. Esta skill ingiere los mensajes de Snapshot y las continuas actualizaciones (Deltas) enviadas por el normalizador, gestionando la inserción, actualización y borrado de niveles de precio (Bids/Asks) para proporcionar al bot una "fotografía instantánea" de la liquidez global a costo de 0 latencia de red.

## 2. Nivel de conocimiento requerido
Experto en Estructuras de Datos de Alta Eficiencia (High-Performance Data Structures). Nivel Máster en Árboles Binarios de Búsqueda Auto-balanceados (Red-Black Trees / AVL Trees), Listas Doblemente Enlazadas, o Arrays ordenados con Búsqueda Binaria (Binary Search Arrays) para garantizar inserciones y lecturas en orden de complejidad O(log N) o superior.

## 3. Capacidades principales
1. Ingesta de Snapshot (`L2 Full Book`): Inicializa el estado base de un par de trading con cientos de niveles pre-cargados.
2. Ingesta de Deltas (`L2 Update`): Actualiza los volúmenes de un nivel de precio. Si el volumen es `0`, el nivel se elimina de la estructura en memoria. Si el nivel no existe, se inserta en el orden correspondiente.
3. Extracción de "Best Bid" y "Best Ask" (Top of Book) en tiempo de complejidad O(1) para comprobaciones de spread inmediatas.
4. Cálculo de "Market Depth" dinámico: Permite simular órdenes de gran tamaño (Market Orders masivas) iterando a través de los niveles del libro para calcular el precio promedio exacto (VWAP) con slippage milimétrico.
5. Sincronización estricta por `SequenceId` o `UpdateId`: Rechaza actualizaciones desordenadas y fuerza una petición de reinicio (Snapshot request) si se pierde un paquete UDP/TCP.
6. Gestión cruzada de libros: Capacidad de combinar 5 order books de 5 exchanges diferentes para crear un "Libro Sintético Agregado Global" para arbitraje masivo.
7. Evicción de basura (Garbage Collection): Poda de niveles profundos irrelevantes (Ej. Órdenes colocadas un 50% por debajo o encima del spread actual) para mantener la memoria RAM bajo control estricto.
8. Detección de Spread Negativo (Crossed Book): Alerta crítica cuando, dentro de un mismo exchange, el Best Bid cruza al Best Ask (Anomalía de Matching Engine o fallo del bot local).
9. Mapeo de comisiones aplicadas "Taker Fee Deduction" dinámico para que la capa matemática vea el volumen real obtenible post-comisión.
10. Protección Multi-Thread: Estructuras libres de bloqueos (Lock-Free) o con Mutex optimizados si el lector matemático y el escritor WebSocket corren en hilos separados (Rust/C++).

## 4. Entradas requeridas
- `canonical_updates`: Payload con Bids, Asks, Exchange, Símbolo, y Tipo (`snapshot` o `delta`).
- `sequence_identifiers`: Números seriados provistos por el exchange para control lógico de concurrencia.
- `max_depth_config`: Profundidad límite de niveles en RAM (e.g., conservar solo top 100 niveles por lado).

## 5. Salidas esperadas
- `local_order_book`: Estructura en memoria lista para ser consultada.
- `top_of_book_event`: Alerta asíncrona disparada SOLO cuando cambia el mejor Bid/Ask (Evita despertar al motor de arbitraje si solo se alteró un nivel profundo e irrelevante).
- `book_health_status`: `"SYNCED"`, `"OUT_OF_SYNC"`, `"REBUILDING"`.

## 6. Reglas inmutables
- El motor de arbitraje (Skill 1) NUNCA debe consultar el precio a la red de forma asíncrona. Siempre debe llamar sincrónicamente (Caché local de memoria RAM L1) a esta estructura.
- Si el motor procesa un `Delta` pero su `previousUpdateId` no empata exactamente con el `currentUpdateId` en memoria de la última iteración, el libro entero DEBE marcarse como Corrupto y ser descartado hasta que un nuevo Snapshot llegue. No se opera con libros parciales o desordenados.
- Las Búsquedas de Top of Book (Mejor compra / Mejor venta) deben resolver en complejidad garantizada de O(1).
- Un nivel de precio con Volumen `0.00` es una instrucción de borrado (Delete Instruction), no se almacena, se purga del árbol.

## 7. Algoritmos o métodos que debe conocer
- Búsqueda y Ordenamiento (Bisection Search, Red-Black Trees).
- Continuous Array vs Linked List Tradeoffs (Uso de Arrays planos para mejor caché en el CPU (Cache Locality) en vez de nodos dinámicos, si el nivel rara vez excede 100 items).
- Algoritmos de checksum de Orderbook (Validación del CRC32 enviado por el exchange para confirmar la fidelidad del libro).

## 8. Fórmulas críticas
- **Cálculo TWAP/VWAP**: `Sum(Price_i * Volume_i) / Sum(Volume_i)` iterando desde `i=0` hasta satisfacer el `Target_Volume`.
- **Condición de Borrado Lógico**: `if (Update_Volume <= 1e-8) { Remove(Update_Price) }`
- **Tolerancia Desync**: `if (New_Delta_ID != Last_Delta_ID + 1) { Throw "Desync Error" }` (Varía ligeramente por API).

## 9. Casos extremos
- Barrido Masivo (Whale Sweep): Una ballena compra a mercado y borra 50 niveles de Asks en un solo evento. El procesador local debe borrar 50 posiciones del árbol instantáneamente sin bloquear o crear spikes de latencia (Jitter) de >1ms.
- Libros Cruzados Intencionales (Crossed Books): El exchange detiene su Matching Engine temporalmente por un Flash Crash pero permite enviar órdenes. Los Bids sobrepasan los Asks sin ejecutarse. El libro arroja "Crossed Book Alert" y el agente pausa operativa porque el spread carece de sentido lógico.
- Memoria ilimitada (OOM - Out of Memory): Un CEX muy ilíquido (Kucoin altcoins) envía miles de dust orders en niveles lejanos, saturando el límite de la lista de arrays en el servidor local.

## 10. Validaciones obligatorias
- PRE: Chequear que la actualización de precio (Price Level) es mayor estricto que 0.
- CÁLCULO: Mantener de forma persistente y determinista el orden `Asks: de Menor a Mayor Precio` y `Bids: de Mayor a Menor Precio`. (El Bid #0 es el comprador más caro, el Ask #0 es el vendedor más barato).
- POST: Validar con una alarma si `Bids[0].price >= Asks[0].price`. Si ocurre, o hay un bug en el código local o el exchange colapsó.

## 11. Criterios de aprobación
- Las consultas solicitando el precio de "X" cantidad de volumen retornan la cotización exacta (Monto de entrada / Monto de salida simulado) en menos de 0.1ms.
- La secuencia lógica se mantiene ininterrumpida por horas.

## 12. Criterios de rechazo
- El Order Book detecta una pérdida de paquete UDP/WS (Gapping id). El estado de rechazo deshabilita todos los arbitrajes de ese par temporalmente.
- Inserciones de "NaN" o "Undefined" provenientes de un normalizador defectuoso.

## 13. Riesgos que mitiga
- Latencia de "Querying": Consultar a una base de datos o REST API el precio cuesta > 50ms. Consultar esta estructura RAM cuesta microsegundos.
- Riesgo de Ilusión de Spread (Phantom Spreads): Si la estructura no borra los niveles que llegaron a volumen `0` y asume que siguen vivos, el motor creerá ver arbitrajes fantasma eternamente, provocando el colapso del bot.

## 14. Integración con otras skills
- Consume la Normalización Multi-fuente (Skill 32).
- Alimenta directamente la Detección Triangular y Cross-Exchange (Skills 12 y 14) al escupir eventos rápidos de Top of Book.
- Proveedor de datos para la Microestructura (Skill 11).

## 15. Modelo de datos sugerido
```json
{
  "LocalOrderBook": {
    "symbol": "BTC_USDT",
    "exchange": "binance",
    "last_update_id": 451992834,
    "top_bid": { "price": 65000.0, "qty": 1.5 },
    "top_ask": { "price": 65000.1, "qty": 0.8 },
    "bids_count": 150,
    "asks_count": 150,
    "status": "SYNCED_HEALTHY"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase Singleton o Struct central con métodos de inserción de extrema performance. En Javascript, preferir Flat Arrays paralelos (`Float64Array`) y `binaryInsert()` / `binaryDelete()` modificados frente a objetos anidados pesados (`{price: x, vol: y}`).

## 17. Logs obligatorios
- `[DEBUG] OrderBook BTC_USDT (OKX) Synced. Top Spread: 0.10. Bids: 120, Asks: 125.`
- `[WARN] Gap detected in Sequence IDs on Bybit book (Expected 105, got 109). Marking CORRUPT. Requesting Snapshot.`
- `[CRITICAL] OrderBook CROSSED natively (Top Bid 60100 > Top Ask 60099). Suspending matching logic for this pair.`

## 18. Métricas obligatorias
- `order_book_update_latency_us` (Tiempo desde que entra la data normalizada hasta que la RAM está lista).
- `book_desync_events_counter`.
- `average_maintained_depth` (Niveles que el bot realmente necesita guardar vs podar).

## 19. Tests unitarios
- Delta Updates: Inyectar un Snapshot con `Ask = 100`. Inyectar un Delta con `Price = 100, Qty = 0`. El Top de Asks debe ser eliminado, y el siguiente nivel debe ascender a la posición Top.
- Slippage Iterator: Pedir cotización de $100,000 en un libro que solo tiene $50,000 en el primer nivel y $50,000 en el segundo nivel. La función `getVWAP` debe iterar limpiamente cruzando ambos y entregar el precio promedio.
- Ordenamiento estricto: Alimentar el objeto con 50 precios insertados de forma desordenada y aleatoria. Al validar, los Bids deben estar de mayor a menor y Asks de menor a mayor sin errores O(1).

## 20. Tests de integración
- Almacenar un archivo JSON inmenso de un histórico Real (Dump de 1 minuto de Binance) conteniendo miles de deltas de volumen. Correr el bot a velocidad multiplicada x100, asegurar que al finalizar el test, el CRC/Checksum del Top of Book coincide con la realidad de ese minuto específico.

## 21. Tests E2E
- El agente enciende, envía petición REST de snapshot de un altcoin altamente volátil, abre el Websocket, empata correctamente el ID del snapshot con los Deltas acumulados en el buffer de red (State reconciliation), construye el libro y envía instantáneamente un flag verde al Sistema Central informando que "Está listo para el combate".

## 22. Checklist de producción
- [ ] Incorporación de un Límite de Poda (Depth Trimming Threshold). Cada X segundos o N actualizaciones, borrar los elementos que excedan el índice 200 de los arrays de Bids/Asks para proteger la memoria RAM local.
- [ ] Optimización "Fast Path" para Top of Book: Si la actualización altera un nivel profundo del libro (Índice > 5), actualizar memoria silenciosamente, pero NO emitir el evento Node/Rust al Orquestador Supremo, mitigando despertar cálculos innecesarios.
- [ ] Verificación cruzada (Sanity Check): Si han pasado 60 minutos sin que el libro cambie, forzar actualización REST (Posible congelamiento silencioso del WebSocket).

## 23. Ejemplo de configuración no hardcodeada
```yaml
order_book_engine:
  max_depth_to_maintain: 100
  trigger_event_on_top_book_change_only: true
  desync_tolerance_ids: 0  # Strict sequencing enabled
  force_snapshot_rebuild_interval_ms: 3600000 # 1 hour
```

## 24. Ejemplo de pseudocódigo
```javascript
class LocalOrderBook {
    constructor(symbol, maxDepth) {
        this.bids = []; // [{price, qty}] ordered desc
        this.asks = []; // [{price, qty}] ordered asc
        this.lastUpdateId = null;
    }

    applyDelta(updates, isBid) {
        const book = isBid ? this.bids : this.asks;
        const comparator = isBid ? (a, b) => b.price - a.price : (a, b) => a.price - b.price;

        for (let update of updates) {
            let [price, qty] = update;
            let index = binarySearch(book, price, comparator);
            
            if (qty === 0) {
                // Delete if exists
                if (index >= 0) book.splice(index, 1);
            } else {
                // Update or Insert
                if (index >= 0) {
                    book[index].qty = qty;
                } else {
                    // Insert keeping sort
                    book.splice(~index, 0, {price, qty});
                }
            }
        }
        
        // Trim fat
        if (book.length > CONFIG.max_depth) book.length = CONFIG.max_depth;
    }

    processMessage(payload) {
        if (this.lastUpdateId !== null && payload.u <= this.lastUpdateId) return; // Stale logic
        
        // Strict sequence checking logic here...
        
        let topBidBefore = this.bids[0]?.price;
        let topAskBefore = this.asks[0]?.price;

        this.applyDelta(payload.b, true);
        this.applyDelta(payload.a, false);
        this.lastUpdateId = payload.u;

        // Only alert the Arbitrage Engine if Top of book changed
        if (this.bids[0]?.price !== topBidBefore || this.asks[0]?.price !== topAskBefore) {
            EventBus.emit('TOP_OF_BOOK_CHANGED', this.getTopSpread());
        }
    }
}
```

## 25. Criterio final de excelencia
El manejador local de libros de órdenes refleja la realidad del mercado con cero tolerancia a la imprecisión y una velocidad implacable, comportándose como una memoria muscular del bot. Absorbe cientos de miles de alteraciones por segundo manteniéndose ligero, y provee el piso sólido y matemático para cualquier cálculo de Arbitraje.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Corrupción de memoria por problemas de Garbage Collector en alta intensidad (Javascript/Node.js). (Implementar con TypedArrays fijos o trasladar módulo a Rust/C++ FFI resuelve este problema crónico de latencia de GC).
- Dependencias: Websockets Multi-exchange, Normalización de datos.
- Próxima skill: Latency mapping & ping monitor (Skill 34).
