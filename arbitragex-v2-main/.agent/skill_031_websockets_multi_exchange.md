# SKILL 031 — WebSockets multi-exchange

## 1. Propósito superior
Establecer un canal de comunicación asíncrono, persistente y de latencia ultrabaja con todos los exchanges centralizados (Binance, OKX, Bybit, Kraken) y proveedores de infraestructura blockchain (Alchemy, Infura WSS). Esta skill reemplaza el ineficiente modelo de "Polling REST" por un flujo continuo (Data Firehose) impulsado por eventos, garantizando que el bot reaccione a cambios de precio, ejecución de órdenes y desconexiones en la misma fracción de milisegundo en la que el evento ocurre en el servidor remoto.

## 2. Nivel de conocimiento requerido
Experto en Arquitectura de Redes Asíncronas, Protocolo RFC 6455 (WebSockets), TCP Keep-Alive mechanics y Programación Reactiva (RxJS / Tokio Streams). Conocimiento profundo del ciclo de vida del socket, compresión de payloads on-the-fly (`permessage-deflate`), y estrategias de mitigación de congestión de buffers (Backpressure).

## 3. Capacidades principales
1. Mantenimiento de conexiones multiplexadas concurrentes (ej. 1 solo socket en Binance suscrito a 200 pares, en lugar de 200 sockets individuales).
2. Manejo heurístico de Ping/Pong frames para prevenir el cierre silencioso de conexiones (Half-open connections) por parte de firewalls intermedios o balanceadores de carga.
3. Auto-reconexión agresiva y resiliente con "Exponential Backoff" tras desconexiones forzosas o mantenimientos del exchange.
4. Autenticación dinámica de sockets privados (UserData Streams) mediante firmas HMAC renovables sin interrumpir el flujo del socket (Listen Key renewals).
5. Resincronización de Snapshot (State Recovery): Si el socket de OrderBook se cae por 2 segundos, no se confía en el stream de re-conexión; se fuerza la descarga de un snapshot fresco vía REST para evitar operar con libros corrompidos.
6. Decodificación de flujos comprimidos nativos (Ej. gzip de OKX WebSockets).
7. Distribución por colas de alta velocidad (Ring Buffers / Lock-free queues) para no bloquear el hilo de red mientras el HFT Engine procesa el mensaje.
8. Filtrado de ruido: Descartar "Heartbeat messages" o actualizaciones idénticas sin pasarlas a la capa matemática, ahorrando CPU.
9. Orquestación del límite de suscripciones: Si un exchange limita 50 pares por socket, la skill auto-instancia N sockets para cubrir 500 pares de forma transparente.
10. Gestión de latencia asimétrica: Registrar qué socket está entregando datos con lag para no mezclar precios rápidos de Bybit con precios viejos de un Binance socket degradado.

## 4. Entradas requeridas
- `endpoints`: URLs WSS de los exchanges.
- `subscription_topics`: Arrays de canales (ej. `trade`, `depth5`, `kline_1m`).
- `auth_credentials`: Claves API para streams privados (Opcional).

## 5. Salidas esperadas
- `event_stream`: Flujo continuo y limpio de JSONs/Binarios parseados dirigidos a los módulos de Ingesta.
- `connection_state`: Enum constante (`CONNECTING`, `OPEN`, `CLOSING`, `CLOSED`).
- `latency_metrics`: Medición del Round Trip Time (RTT) del socket.

## 6. Reglas inmutables
- Nunca procesar lógicas de negocio pesadas o cálculos dentro del evento `onMessage` del WebSocket. El Hilo de Red debe únicamente parsear, empujar a la cola (Queue) y retornar para procesar el siguiente mensaje, evitando estrangular el buffer TCP del sistema operativo.
- Un socket que no reciba un Ping, un Pong o un mensaje válido durante más de `N` segundos (generalmente 3-5s en HFT) DEBE ser considerado "Zombie", destruido y reconectado inmediatamente.
- Para flujos críticos de datos (Orderbooks), una desconexión implica la invalidación inmediata de todo el estado local en la memoria caché hasta la resincronización. No se opera "a ciegas".

## 7. Algoritmos o métodos que debe conocer
- Actor Model (Erlang/Rust) o Event-Loop optimization (Node.js).
- Algoritmo de Exponential Backoff con Jitter para reconexión masiva.
- Compresión zlib/deflate a nivel de paquete de red.

## 8. Fórmulas críticas
- **Exponential Backoff**: `Delay = Min(Max_Delay, Base_Delay * 2^Attempt) + Random(0, Jitter_Max)`
- **Umbral de Descarte Zombie**: `CurrentTime - LastMessageTime > Zombie_Threshold_MS`

## 9. Casos extremos
- Data Avalanche: Mercado en Flash Crash, Binance envía 10,000 actualizaciones por segundo. Si el bot las procesa en serie, el lag interno sube a 5 segundos (El bot "ve" el precio de hace 5s). Requiere muestreo (Sampling) o consolidación de profundidad antes del envío a la lógica matemática.
- Desconexión por límite de mantenimiento (24h limit): Binance Spot corta forzosamente los WebSockets cada 24 horas exactas. El bot debe anticiparse y abrir un Socket B en la hora 23:59, transferir el stream, y luego cerrar el Socket A (Zero-downtime rotation).
- Mensajes corruptos o desordenados (Out of order updates): Actualizaciones `u` (final) y `U` (inicio) que llegan fuera de secuencia destruyendo el Orderbook local.

## 10. Validaciones obligatorias
- PRE: Chequear los límites de conexión de IP estáticos del exchange. Algunos bloquean IPs que abren > 5 sockets por segundo.
- CÁLCULO: Validar el tamaño del Payload del socket antes de parsear `JSON.parse()`. JSONs excesivamente largos pueden ser ataques de vector o saturación, usar parseadores JSON seguros o librerías rápidas (ej. `simdjson`).
- POST: Verificador de Secuencia (Sequence Number Checker). Si se recibe el mensaje #10 y el anterior fue el #8, lanzar alerta de "Gap" y forzar resincronización.

## 11. Criterios de aprobación
- Conexión mantenida durante > 23 horas de forma estable sin degradación de memoria (Memory Leaks en handlers).
- El Ping/Pong (Heartbeat) confirma latencia menor a 50ms al endpoint.

## 12. Criterios de rechazo
- El WebSocket entra en bucle infinito de caídas y reconexiones (Crash loop) debido a credenciales expiradas o payload de suscripción malformado.
- La memoria RAM aumenta linealmente cada hora debido a listeners `onMessage` acumulados no liberados (Leak).

## 13. Riesgos que mitiga
- Retraso fatal de REST: La API REST normal requiere 1 handshake TLS por llamada (latencia > 100ms extra). WebSockets mantiene la tubería abierta, reduciendo el envío de datos a fracciones de milisegundo, indispensable para HFT.
- Rate Limits Masivos: Suscribirse a 500 símbolos vía REST costaría 500 créditos por segundo, ganando un Ban de IP de 3 días en segundos. WebSockets permite 500 símbolos consumiendo 0 créditos REST.

## 14. Integración con otras skills
- Provee los eventos primarios a Normalización de Datos (Skill 32).
- Mantiene activo el Ping Monitor (Skill 34).

## 15. Modelo de datos sugerido
```json
{
  "WebSocketManager": {
    "exchange": "binance_futures",
    "socket_id": "ws-bfut-001",
    "status": "OPEN",
    "active_subscriptions": 150,
    "uptime_seconds": 3600,
    "last_message_ms_ago": 12,
    "messages_per_second": 140,
    "reconnect_count_24h": 0
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Clase envoltorio genérica `WsManager` que expone métodos abstractos estandarizados `subscribe(topic)`, `unsubscribe(topic)` independientemente de los caprichos del API del exchange subyacente.

## 17. Logs obligatorios
- `[INFO] WSS Binance Spot Connected. Subscribing to 120 depth streams. Handshake RTT: 24ms.`
- `[WARN] WSS OKX Zombie detected. No ping/pong in 5000ms. Force closing socket and initializing failover.`
- `[CRITICAL] Max connection limits reached for Bybit WSS. Refusing to spawn more sockets. IP Ban protection engaged.`

## 18. Métricas obligatorias
- `wss_messages_processed_per_sec`.
- `wss_reconnect_events_counter`.
- `wss_last_message_latency_gauge`.

## 19. Tests unitarios
- Parseo de fragmentos: Simular una carga JSON partida por la mitad (Típico en TCP chunks grandes) y asegurar que el buffer espera el resto del frame antes de parsear y explotar.
- Ping Monitor Logic: Inyectar un tiempo simulado de 10 segundos sin actividad. El socket debe emitir el evento `close()` forzadamente.
- Limpiador de Listeners: Asegurar que al cerrar un socket, `removeAllListeners` destruya toda la basura referenciada (Garbage Collection test).

## 20. Tests de integración
- Levantar servidor Echo WSS local. Conectar bot, botarse el server local a propósito, ver cómo el backoff del bot reintenta a 1s, luego a 2s, luego a 4s de forma dócil y no agresiva.

## 21. Tests E2E
- El agente inicia. Conecta a Binance, Kraken, OKX y Bybit en paralelo, recibe un torrente de >5000 msg/sec, el monitor de sistema verifica que el CPU Thread de red no está ahogado y que los precios de 4 plataformas llegan a la memoria del bot en menos de 10ms.

## 22. Checklist de producción
- [ ] Incorporación de `PingTimeout` e `Intervals` precisos al milisegundo usando timers no bloqueantes de SO (`epoll`/`kqueue`).
- [ ] Optimizar tamaño del Buffer del socket a nivel kernel (sysctl params en linux) si el servidor va a absorber un throughput en gigabits.
- [ ] Activar compresión `permessage-deflate` solo si el CPU está libre, ya que descomprimir consume CPU pero ahorra Ancho de Banda; en VPS en el mismo datacenter (AWS Tokyo), desactivar compresión reduce la latencia por no requerir de-compress.

## 23. Ejemplo de configuración no hardcodeada
```yaml
websocket_manager:
  ping_interval_ms: 3000
  zombie_timeout_ms: 5000
  max_subscriptions_per_socket: 100
  compression_enabled: false # Disabled for minimal latency in co-located AWS instances
  auto_rotate_24h: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class ResilientSocket {
    constructor(url) {
        this.url = url;
        this.connect();
    }
    
    connect() {
        this.ws = new WebSocket(this.url, { perMessageDeflate: false });
        this.ws.on('open', this.onOpen.bind(this));
        this.ws.on('message', this.onMessage.bind(this));
        this.ws.on('close', this.onClose.bind(this));
        this.ws.on('ping', this.onPing.bind(this));
        
        this.resetWatchdog();
    }

    onMessage(data) {
        this.resetWatchdog();
        // Fast path: push to lock-free ring buffer or emit to worker thread
        messageBus.pushFast(data); 
    }

    resetWatchdog() {
        clearTimeout(this.zombieTimer);
        this.zombieTimer = setTimeout(() => {
            log.warn(`Socket ${this.url} is zombie. Reconnecting.`);
            this.ws.terminate(); // Hard kill
        }, CONFIG.zombie_timeout_ms);
    }
    
    onClose(error) {
        clearTimeout(this.zombieTimer);
        let backoff = calculateBackoff(this.reconnectAttempts++);
        setTimeout(() => this.connect(), backoff);
    }
}
```

## 25. Criterio final de excelencia
El manejador de WebSockets funge como un sistema nervioso central indestructible, soportando tormentas de datos y caídas de la red global sin que la inteligencia principal del bot sufra desajustes, rotando sockets de forma invisible y garantizando datos en tiempo absoluto.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Bloqueos de red a nivel ISP (Mitigados corriendo la instancia en AWS/GCP en la misma región que los servidores del exchange).
- Dependencias: Data Normalization (Skill 32).
- Próxima skill: Normalización de datos multi-fuente (Skill 32).
