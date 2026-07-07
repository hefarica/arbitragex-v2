# SKILL 036 — Orquestador general de ejecución concurrente

## 1. Propósito superior
Actuar como el "Cerebro Director" o "Event Loop Maestro" del bot. Este orquestador gestiona el paralelismo real o simulado del sistema, garantizando que el escaneo de mercados, las simulaciones matemáticas, y las llamadas API de ejecución no se estorben entre sí. Coordina la arquitectura Multi-Threading (o multi-worker) para maximizar la utilización de la CPU (100% Core Usage) evitando el gran problema de los bots HFT en Javascript/Python: quedarse bloqueado calculando algo mientras el mercado cambia de precio.

## 2. Nivel de conocimiento requerido
Experto en Arquitectura de Software de Alta Concurrencia. Dominio del Actor Model (Ej. Erlang, Actix Rust), Event Loops, Worker Threads / Web Workers, IPC (Inter-Process Communication), uso eficiente de Shared Memory (SharedArrayBuffer/Atomics), Spinlocks vs Mutexes, y Promesas Asíncronas No Bloqueantes. Conocimiento sobre el "Context Switching Overhead" del Sistema Operativo.

## 3. Capacidades principales
1. Aislamiento de Red: Mantener todos los Sockets (Websockets/RPC) en un hilo (Thread) dedicado que únicamente parsee y ponga en cola.
2. Aislamiento Matemático: Asignar cálculos brutales (Newton-Raphson de Curve, Simulaciones EVM) a hilos de procesamiento (Worker Pool) para no detener la lectura de eventos.
3. Despachador de Tareas (Task Scheduler): Recibir eventos de "Top of Book Changed" y disparar asincrónicamente cientos de verificadores de grafos en paralelo.
4. Comunicación Lock-Free: Intercambiar datos entre hilos usando variables atómicas o Ring Buffers (LMAX Disruptor pattern) en lugar de JSON serialization (JSON.stringify/parse destruye la performance entre threads).
5. State Machine Management: Controlar en qué estado exacto está una operación de arbitraje (Detectado -> Calculado -> Simulado -> Emitido -> Confirmado).
6. Race Condition Prevention: Asegurar que dos hilos no traten de operar el mismo par con el mismo balance de inventario simultáneamente (Double Spend Risk).
7. Timeout & Kill Switch: Poder cancelar promesas o abortar hilos atascados (Zombie Threads).
8. Garbage Collection Evasion: Diseñar la circulación de objetos (Object Pooling) reutilizando estructuras de datos pre-asignadas en memoria en vez de instanciar un `{}` nuevo por cada tick, evadiendo micro-pausas del recolector de basura.
9. Orquestación SAGA: Enviar transacciones a CEX1 y CEX2 y manejar la lógica de rollback o avance de forma asíncrona pero correlacionada.
10. Priorización de CPU Affinity: Atar (Pin) el Hilo Crítico de Ejecución al Core 0 de la CPU (Hardware level optimization).

## 4. Entradas requeridas
- `system_events`: Torrente de mensajes del Bus de Eventos (EventBus).
- `worker_pool_config`: Número de núcleos lógicos disponibles en la máquina host.
- `arbitrage_signals`: Señales emitidas por los módulos matemáticos y forenses.

## 5. Salidas esperadas
- `executed_trades`: Operaciones consumadas y empaquetadas.
- `thread_telemetry`: Estado de salud de los hilos (CPU Load %, Queue Size).
- `system_state`: Estado general del orquestador (`RUNNING`, `HALTED`, `PANIC`).

## 6. Reglas inmutables
- Nunca ejecutar I/O de red en el Hilo de Cómputo Principal (Math Engine).
- Nunca ejecutar Matemáticas Pesadas (BigInt iterativo, Graph Search) en el Hilo de I/O de Red.
- El Orquestador debe utilizar estructuras Object Pool. Si un array es usado para Bids/Asks, se sobreescribe el existente, jamás se crea `new Array()` por cada mensaje (El GC (Garbage Collector) causará "Stop-the-World" pauses fatales).
- Cualquier error no capturado (Unhandled Rejection/Exception) en un Worker debe revivir al Worker inmediatamente, pero si ocurre 3 veces en < 1 segundo, el Orquestador debe colapsar todo el sistema (Fail-Fast) por seguridad estructural.

## 7. Algoritmos o métodos que debe conocer
- LMAX Disruptor Pattern (Mechanical Sympathy).
- Wait-Free & Lock-Free Data Structures.
- Coroutines / Async-Await FSM (Finite State Machines).

## 8. Fórmulas críticas
- **Thread Count Óptimo**: `Workers = Num_Logical_Cores - 1 (dejando 1 libre para OS/Networking)`.
- **Latency Budget (Micro-arquitectura)**: `Latency_Network + Latency_IPC + Latency_Math < 5ms` total interno.

## 9. Casos extremos
- Inundación de Mensajes (Backpressure collapse): El mercado se vuelve loco, Binance manda 250,000 mensajes por segundo. El Worker Pool tiene un límite de procesamiento de 50,000/sec. La cola IPC se llena, colapsando la memoria RAM y creando un retraso masivo. El Orquestador debe activar "Tail Drop" (Descartar actualizaciones de pares no rentables automáticamente sin encolar).
- Deadlocks (Bloqueo Mutuo): Hilo A bloquea Recurso 1 y necesita Recurso 2. Hilo B bloquea Recurso 2 y necesita Recurso 1. El bot se congela para siempre sin emitir un error.
- GC Pause (Javascript/Go): El sistema pausado por 200ms recogiendo millones de strings JSON viejos. Causa la pérdida de toda sincronización temporal con la EVM.

## 10. Validaciones obligatorias
- PRE: Chequear que todos los canales IPC están abiertos y los Workers responden con un evento `"READY"`.
- CÁLCULO: Validar Locks en el inventario. Si Hilo A está analizando gastar $10,000 USDC en Arbitraje X, marcar esos $10,000 como "Reserved" usando variables atómicas para que el Hilo B no los contabilice para Arbitraje Y.
- POST: Al liberar el trade, liberar el "Reservation Lock" inmediatamente tras la confirmación o el fallo.

## 11. Criterios de aprobación
- La utilización del CPU es asimétrica y optimizada (100% en hilos matemáticos, <5% en hilo principal de orquestación).
- Latencia IPC es < 0.05ms entre hilos.

## 12. Criterios de rechazo
- El tamaño de las Colas de Mensajes (Queue Depth) supera 100 elementos (Significa que la matemática es demasiado lenta para procesar la realidad, latencia sistémica).
- El Event Loop Lag (En entornos de un solo hilo asincrónico como Node) supera los 15ms.

## 13. Riesgos que mitiga
- Riesgo de Asfixia por CPU (CPU Bound starvation): Ocurre cuando el bot no lee la respuesta HTTP del exchange porque "estaba ocupado" resolviendo el Teorema de Bellman-Ford. Desacoplar las tareas previene que el bot quede sordo y mudo temporalmente.
- Riesgo de Doble Gasto (Double Spending Local): Dos lógicas concurrentes tratando de arbitrar sobre la misma moneda porque la ejecución del Hilo A no descontó la billetera a tiempo.

## 14. Integración con otras skills
- Es la Placa Madre (Motherboard) de todas las Skills desde la 1 a la 35 y de la 41 a la 100.
- Interactúa críticamente con Rate Limit Bypass (Skill 35) distribuyendo el límite de tokens entre hilos.

## 15. Modelo de datos sugerido
```json
{
  "OrchestratorState": {
    "status": "RUNNING",
    "active_workers": 7,
    "event_loop_lag_ms": 1.2,
    "main_queue_depth": 0,
    "tasks_processed_per_sec": 8450,
    "memory_heap_used_mb": 142.5,
    "locked_resources": ["USDT_BINANCE", "ETH_ARB"]
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Singleton Orchestrator (Main Thread) que despacha comandos usando `Worker.postMessage` (con buffers transferibles para Zero-Copy `Transferable Objects`) o Channels en Rust.

## 17. Logs obligatorios
- `[DEBUG] Dispatching Bellman-Ford computation to Worker 3 (Pool Arbitrum).`
- `[WARN] Event Loop Lag detected: 25ms! Main thread is being blocked. Investigate sync JSON parsing.`
- `[CRITICAL] Worker 4 Panicked/Crashed. Reason: BigInt Overflow. Respawning worker and clearing its queue.`

## 18. Métricas obligatorias
- `ipc_transfer_latency_ns`.
- `event_loop_delay_histogram`.
- `worker_cpu_utilization_pct`.

## 19. Tests unitarios
- Zero-Copy Validation: Pasar un `SharedArrayBuffer` con 10,000 niveles de precio al Worker y validar que el Main Thread puede mutar la memoria y el Worker puede leerla instantáneamente sin pasar por un proceso de serialización.
- Resource Lock (Mutex): Lanzar 100 promesas simultáneas que intentan "Bloquear" y gastar $10 de un balance imaginario de $100. Asegurar que exactamente 10 promesas tienen éxito y 90 son rechazadas por "Falta de fondos/Lock engaged".
- Respawn logic: Matar un worker llamando `process.exit(1)` internamente. El orquestador debe detectarlo y levantar uno nuevo en menos de 100ms.

## 20. Tests de integración
- Inyectar 1 millón de ticks de precio por el Websocket simulado en 1 segundo. El sistema no debe crashear, debe descartar los obsoletos automáticamente, y la memoria RAM debe mantenerse en Flatline (Efectividad de Object Pooling).

## 21. Tests E2E
- Arrancar el Sistema Completo. Hilo de Red chupa datos de Binance y RPC. Hilo Matemático simula 5,000 cruces cruzados. Hilo de Ejecución arma el payload. El orquestador manda señal de Halt (Pausa). Todos los hilos deben frenar su procesamiento inmediatamente al recibir la señal Atómica de Parada.

## 22. Checklist de producción
- [ ] Uso exclusivo de Node.js `worker_threads` o Rust `std::thread / tokio::spawn`.
- [ ] Sustitución masiva de `JSON.parse` y `JSON.stringify` en transferencias inter-procesos por serialización binaria o Typed Arrays estáticos (`Float64Array`).
- [ ] Monitoreo constante de Garbage Collection events a través de flags nativos (`--trace-gc` en V8). Si el GC entra en acción pesada durante un arbitraje, se arruina el timing.

## 23. Ejemplo de configuración no hardcodeada
```yaml
orchestrator_engine:
  math_worker_threads: 4
  io_worker_threads: 2
  max_queue_depth_before_taildrop: 50
  enable_shared_array_buffers: true
  max_acceptable_event_loop_lag_ms: 15
```

## 24. Ejemplo de pseudocódigo
```javascript
// Main Thread
const buffer = new SharedArrayBuffer(1024 * 1024); // 1MB shared state
const orderbookState = new Float64Array(buffer);
const lock = new Int32Array(new SharedArrayBuffer(4)); // Spinlock

// Start math worker
const mathWorker = new Worker('./mathWorker.js', { workerData: { buffer, lock } });

// Network Socket onMessage (Runs in Main or IO Thread)
function onPriceUpdate(index, price, volume) {
    // Spinlock acquire
    while (Atomics.compareExchange(lock, 0, 0, 1) !== 0) { /* Wait */ }
    
    // Fast lock-free-ish update
    orderbookState[index * 2] = price;
    orderbookState[index * 2 + 1] = volume;
    
    // Spinlock release
    Atomics.store(lock, 0, 0);
    
    // Notify Math worker without passing massive arrays
    mathWorker.postMessage({ type: 'CALCULATE_NOW', indexChanged: index });
}

// Inside MathWorker.js
parentPort.on('message', (msg) => {
    if (msg.type === 'CALCULATE_NOW') {
         // Access shared memory directly without JSON cloning
         const price = orderbookState[msg.indexChanged * 2];
         runHeavyArbitrageMath(price);
    }
});
```

## 25. Criterio final de excelencia
El Orquestador es la obra de arte ingenieril que separa un "script de trading" aficionado de un "Motor Institucional HFT". Maneja millones de estados y gigabytes de I/O de red por hora manteniendo la utilización del procesador asimétricamente perfecta y asegurando latencia interna menor al medio milisegundo en cualquier ruta.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Bugs de Concurrencia (Data Races) si los Punteros y Mutex de memoria compartida no están implementados perfectamente (Suele llevar a valores espurios como un precio = 0).
- Dependencias: Soporte Multithreading a nivel Sistema Operativo.
- Próxima skill: Data lake & time-series storage (Skill 37).
