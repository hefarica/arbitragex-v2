# SKILL 008 — Teoría de colas para sistemas de ejecución

## 1. Propósito superior
Garantizar que el sistema de ejecución de órdenes no colapse bajo ráfagas extremas de volatilidad de mercado. Aplica teoría matemática de colas (Queueing Theory) para modelar y gestionar las llegadas de señales de arbitraje, las peticiones HTTP/RPC, y las respuestas asíncronas. Evita el bloqueo de hilos, los timeouts paralelos y asegura un enrutamiento de red determinista incluso bajo condiciones de denegación de servicio (DDoS) del mercado.

## 2. Nivel de conocimiento requerido
Máster en Ingeniería de Software concurrente, Sistemas Distribuidos y Teoría de Colas (Modelos M/M/1, M/M/c). Experiencia profunda en Event Loops (Node.js/Rust Tokio), non-blocking I/O, backpressure, rate-limiting distribuido y orquestación de workers de baja latencia.

## 3. Capacidades principales
1. Implementación de colas de prioridad estricta: Las oportunidades de alto ROI neto puentean a las oportunidades de bajo ROI.
2. Manejo de Backpressure: Si la API del exchange se satura, el sistema descarta las señales nuevas (Load shedding) en lugar de encolarlas infinitamente.
3. Modelado de tasas de llegada (λ) y tasas de servicio (μ) para predecir la saturación del Worker Pool.
4. Orquestación asíncrona mediante un patrón de Reactor / Proactor.
5. Inserción dinámica de "Cooldowns" post-ejecución para evitar violar los Rate Limits (Skill 37).
6. Separación de colas: IO-bound (llamadas de red) vs CPU-bound (cálculo de grafos/criptografía de firmas en Web3).
7. Dead-letter queues (DLQ) para registrar ejecuciones fallidas sin bloquear la vía principal.
8. Gestión determinista de la cancelación de órdenes huérfanas encoladas.
9. Mecanismo de reintento exponencial pasivo (Exponential backoff) para fallos de red no financieros.
10. Sincronización de estado global libre de locks (Lock-free memory queues).

## 4. Entradas requeridas
- `incoming_signals`: Stream masivo de oportunidades rentables generadas por los motores matemáticos.
- `worker_status`: Disponibilidad en tiempo real del pool de ejecución (Hilos/Conexiones).
- `rate_limit_state`: Estado actual de los tokens/buckets de API permitidos por el exchange.
- `network_timeout_config`: Límite máximo de vida de una señal en la cola antes de ser descartada (Time-To-Live).

## 5. Salidas esperadas
- `execution_payloads`: Despacho de la orden al socket HTTP/WS final.
- `queue_metrics`: Largo de la cola, tiempo medio de espera, tasa de descarte.
- `rejection_events`: Eventos de tipo "Dropped by Load Shedding" o "TTL Expired".

## 6. Reglas inmutables
- Nunca procesar una señal de arbitraje cuyo tiempo en cola (TTL) haya excedido la ventana de frescura (ej. 200ms). Si envejece, se destruye.
- Nunca bloquear el hilo principal (Main Loop) esperando una respuesta de red.
- Las órdenes de cancelación (Cancel Orders) o recuperación de estado tienen prioridad infinita sobre la creación de nuevas órdenes (New Orders).
- Si `λ > μ` durante más de 1 segundo, activar la poda agresiva (Head/Tail drop) para preservar la memoria.

## 7. Algoritmos o métodos que debe conocer
- Token Bucket y Leaky Bucket para rate limiting asíncrono.
- Little's Law `L = λ * W` para monitoreo de estabilidad de la cola.
- Lock-free Ring Buffers (Disruptor pattern) para pasaje de mensajes entre hilos de CPU.
- Priority Queues basadas en Binary Heaps.

## 8. Fórmulas críticas
- **Little’s Law**: `Longitud_Cola = Tasa_Llegada * Tiempo_Medio_Espera`
- **Condición de Estabilidad**: `ρ = λ / (c * μ) < 1` (Donde c es el número de workers).
- **Cálculo de Descarte**: Si `Tiempo_Actual - Timestamp_Llegada > Max_TTL`, entonces `Drop()`.
- **Score de Prioridad**: `Priority = ROI_Neto_USD / Volatilidad_Riesgo`

## 9. Casos extremos
- Flash crash global: Miles de oportunidades de arbitraje disparadas en 50ms, superando el límite de rate del exchange por 100x.
- Memory Leak en la cola debido a señales bloqueadas sin timeout configurado.
- Inversión de prioridad (Priority Inversion): Señales de alta prioridad atascadas detrás de señales de baja prioridad porque el pool de workers se agotó.
- Partición de red: La red falla repentinamente y la cola se llena de peticiones en estado "Pending".

## 10. Validaciones obligatorias
- PRE: Validar que el payload encolado contiene un timestamp estricto de generación.
- CÁLCULO: Mantener un O(log n) o O(1) para inserción/extracción en la cola priorizada.
- POST: Al extraer de la cola, re-validar el TTL antes de enviarlo a red.

## 11. Criterios de aprobación
- La señal es despachada al socket en un tiempo `Wait_Time < Max_Latency_Config`.
- El uso de memoria de la cola se mantiene estable bajo estrés prolongado.

## 12. Criterios de rechazo
- El TTL de la señal expiró mientras esperaba en la cola (Dato Vencido).
- El Rate Limiter de red avisa que despachar la señal causaría un baneo (HTTP 429).

## 13. Riesgos que mitiga
- Riesgo de Baneo: Ser bloqueado por un CEX por inundar su endpoint con miles de órdenes ciegas.
- Operación Zombi: Ejecutar un arbitraje 2 segundos tarde porque el bot estaba "pensando" o "esperando" en una cola saturada (Pérdida garantizada).

## 14. Integración con otras skills
- Procesa el output final de Optimización Estocástica (Skill 6) y Risk Engine (Skill 41).
- Pasa peticiones a API Rate-limit intelligence (Skill 37).

## 15. Modelo de datos sugerido
```json
{
  "ExecutionQueueItem": {
    "signal_id": "uuid",
    "arrival_timestamp_us": 1698765432100123,
    "priority_score": 95.5,
    "payload": { ... },
    "ttl_us": 100000, 
    "status": "queued"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Módulo interno en memoria. Puede usar `std::sync::mpsc` en Rust o `worker_threads` + `SharedArrayBuffer` en Node.

## 17. Logs obligatorios
- `[DEBUG] Signal X enqueued. Priority: 95. Queue size: 3.`
- `[WARN] Load Shedding active. Dropped signal Y (TTL expired by 15ms).`
- `[INFO] Queue saturation detected (rho > 0.9). Backpressure engaged.`

## 18. Métricas obligatorias
- `queue_length_gauge`
- `signal_wait_time_us_histogram`
- `signals_dropped_rate`
- `worker_utilization_pct`

## 19. Tests unitarios
- Encolar 100 señales con el worker pausado. Reanudar. Verificar que extrae por orden de prioridad (ROI), no por orden de llegada (FIFO clásico).
- Test de TTL: Encolar señal, simular delay > TTL, intentar extraer. Debe retornar nulo y loguear drop.
- Test de Inversión de Prioridad: Cancel Order (Prioridad Infinita) debe saltar al frente de 1000 órdenes de compra.

## 20. Tests de integración
- Integrar con el Rate Limiter (Token Bucket). Si el bucket se vacía, la cola debe acumular, y luego podar agresivamente las órdenes viejas.

## 21. Tests E2E
- Bombardear el sistema con 10,000 oportunidades simultáneas generadas por test. El sistema debe ejecutar exactamente las N mejores oportunidades permitidas por la API, y descartar limpiamente las otras 9,900+ sin crashear.

## 22. Checklist de producción
- [ ] Implementar un Ring Buffer de tamaño fijo para evitar asignación dinámica de memoria (Zero-alloc).
- [ ] Monitoreo de latencia interno acoplado al módulo de Observabilidad (Skill 64).
- [ ] Separación física de hilos para lectura de Websocket vs envío de Orden HTTP REST.

## 23. Ejemplo de configuración no hardcodeada
```yaml
execution_queue:
  max_queue_size: 1024
  ttl_microseconds: 150000     # 150ms
  worker_threads: 4
  drop_strategy: "tail_drop_lowest_priority"
```

## 24. Ejemplo de pseudocódigo
```python
import time
import heapq

class ExecutionQueue:
    def __init__(self, ttl_us, rate_limiter):
        self.queue = [] # priority heap
        self.ttl_us = ttl_us
        self.rate_limiter = rate_limiter
        
    def enqueue(self, signal):
        # Priority is inverted for min-heap (lower number = higher priority)
        heapq.heappush(self.queue, (-signal.priority, signal))
        
    def process_next(self):
        while self.queue:
            _, signal = heapq.heappop(self.queue)
            
            # 1. Check TTL
            if time.time_us() - signal.arrival_timestamp_us > self.ttl_us:
                log.warn(f"Dropped signal {signal.id} due to TTL")
                continue
                
            # 2. Check Rate Limit
            if not self.rate_limiter.consume(1):
                # Backpressure: Rate limit hit. Put back or drop?
                # For HFT, we usually drop if the delay will kill the alpha
                log.warn(f"Dropped signal {signal.id} due to Rate Limit Backpressure")
                continue
                
            return execute_on_network(signal)
            
        return None
```

## 25. Criterio final de excelencia
El manejador de colas es inquebrantable; soporta ataques DDoS de señales internas, manteniendo el uso de CPU controlado, y garantiza que toda orden enviada a la red tiene < 2 milisegundos de latencia interna desde su detección hasta su despacho.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Pausas del Garbage Collector (Node/Python) bloqueando el hilo por más del TTL.
- Dependencias: API Rate Limit, Orchestration.
- Próxima skill: Probabilidad bayesiana para oportunidad real (Skill 9).
