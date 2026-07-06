# SKILL 034 — Latency mapping & ping monitor

## 1. Propósito superior
Mapear y monitorizar el "Campo de Batalla Físico" del Agente HFT. A diferencia del ping pasivo común de una aplicación web, esta skill ejecuta cálculos determinísticos de telemetría entre los servidores físicos de ArbitrageX (AWS/GCP/Local) y los nodos del exchange (APIs, RPCs, WebSockets). Garantiza que las estrategias basadas en milisegundos compensen sus cálculos (Lag Adjustment) con datos de latencia hiper-realistas y bloquea la ejecución de operaciones si el ruteo de red ha sufrido una perturbación o penalización asimétrica de BGP.

## 2. Nivel de conocimiento requerido
SRE (Site Reliability Engineer), Ingeniero de Redes de Baja Latencia, Experto en Optimización de Topologías HFT y Análisis Estadístico Estocástico (Distribuciones Normales vs Larga Cola en Paquetes de Red). Conocimiento de "Clock Drift" (PTP - Precision Time Protocol), ruteo intra-datacenter, direct-connects de AWS y medición de Round-Trip-Time (RTT) asimétrico.

## 3. Capacidades principales
1. Mapeo RTT Activo y Pasivo (Active Ping a endpoints REST explícitos, y Passive Ping extrayendo latencias midiendo el tiempo de recepción entre la emisión del exchange y la recepción del socket).
2. Cálculo de Offset de Latencia Asimétrica: Determinar si Bybit tarda 20ms en llegar al servidor, pero Binance tarda 50ms, inyectando un retraso (Delay) artificial de compensación a Bybit al enviar órdenes para "hacer match" simultáneo (Sincronización Cross-Exchange de Skill 12).
3. Construcción del EWMA (Exponential Weighted Moving Average) del Ping para evadir picos transitorios y suavizar la matriz de decisiones.
4. Generación de Alerta "Latency Spike" (Ej. Ping salta de 10ms a 250ms). Bloquea instantáneamente las ejecuciones HFT para no cazar oportunidades que matemáticamente ya pasaron.
5. Monitor de Desviación de Reloj (Clock Drift Monitor): Comprueba que el reloj del Servidor Local esté sincronizado (NTP/Chrony) en un margen de +/- 5ms contra los servidores del Exchange para evitar firmas rechazadas (Timestamp out of sync).
6. Trazabilidad regional (Heatmapping): Determinar qué exchange tiene menor latencia, útil para cambiar dinámicamente de Proxy si el nodo base experimenta caída regional de rutas.
7. Discriminación REST vs WS: Sabe que el Websocket tiene latencia distinta a la llamada REST y penaliza o aprueba el uso de uno u otro método dinámicamente según la saturación puntual.
8. Determinación de Jitter (Variabilidad del ping). Un ping constante de 50ms es operable, un ping que varía frenéticamente entre 10ms y 100ms es inoperable para HFT Atómico.
9. Watchdog de Interrupción de Nodos RPC (Bloqueos "Silent Drops").
10. Sincronización de Tiempo Maestro (Master Time Sync): Si el servidor local está adelantado 200ms al Exchange, ajusta todas las firmas HMAC internas agregando un offset sin cambiar la hora del Sistema Operativo.

## 4. Entradas requeridas
- `network_events`: Timestamps locales vs Timestamps del objeto JSON proveniente de los CEX/DEX.
- `ping_targets`: Lista de URLs (REST, WSS, RPC) para comprobación activa.
- `local_hardware_clock`: Timestamp de alta resolución de la CPU (Ej. `process.hrtime()` / `performance.now()`).

## 5. Salidas esperadas
- `latency_matrix`: Matriz 2D viva con los Pings, Jitter y Offsets por proveedor.
- `network_health_state`: Enum `GREEN`, `YELLOW` (Jitter Alto), `RED` (Latencia inoperable).
- `compensated_timestamp_offset`: Desviación matemática (Local Time vs Exchange Server Time).

## 6. Reglas inmutables
- Nunca confiar en `Date.now()` para mediciones de latencia interna (HFT) debido a las micro-correcciones de NTP del Sistema Operativo que hacen retroceder el tiempo; SIEMPRE usar relojes monotónicos (Monotonic Clocks como `performance.now()`).
- Si la matriz registra una caída en la conexión a Binance, NINGÚN arbitraje multileg que incluya Binance puede enviarse asumiendo un reintento o esperanza. Se desactiva la ruta dura.
- La evaluación de la oportunidad matemática (Ej. Arbitraje CEX-CEX) DEBE restar el RTT (ida y vuelta) al tiempo proyectado del Trade. Si la ineficiencia histórica del mercado para cerrarse es 50ms, y tu ping es 80ms, el arbitraje es Matemáticamente Inviable y se descarta.

## 7. Algoritmos o métodos que debe conocer
- Cálculo Monotónico del Tiempo (High-Resolution Timekeeping).
- Smoothing Metrics (Algoritmos de medias móviles ponderadas exponencialmente).
- Detección de Valores Atípicos (Z-Score aplicado al Jitter de red para filtrar spikes reales de errores locales).

## 8. Fórmulas críticas
- **Cálculo de Jitter**: `Jitter = (Jitter_antiguo * 15 + |RTT_Actual - RTT_Anterior|) / 16` (Fórmula simplificada de RFC 3550).
- **Compensación Cross-Exchange**: `Delay_Inyectado_en_Exchange_Rapido = Ping_Exchange_Lento - Ping_Exchange_Rapido`
- **Tolerancia Límite P99**: `if (Current_Latency > Average_Latency * 3) { Trigger Spike State }`

## 9. Casos extremos
- Intercepción BGP (Rutas de tráfico hackeadas o congestionadas a nivel de proveedor de internet Tier-1): El servidor en AWS Tokio, que usualmente está a 2ms de Binance Tokio, de repente envía el tráfico a través de Europa por un fallo de cable submarino, elevando la latencia a 280ms. El bot debe notarlo al instante y cesar todo fuego HFT.
- Sobrecarga (Throttling) por parte del Proveedor Cloud: La máquina virtual sufre limitaciones de CPU ("Steal Time" en AWS) pausando el proceso por 100ms aleatoriamente. El ping muestra spikes falsos. El sistema asume que la red colapsó (False Positive) protegiendo el capital.
- Desfase del Reloj (Clock Skew): El reloj interno del servidor se atrasa 2 segundos. Las API Keys son bloqueadas con error `Invalid Timestamp`. El Offset calcula la diferencia y suma/resta los ms automáticamente al firmar el payload para evadir la caída.

## 10. Validaciones obligatorias
- PRE: Correr test basal de pings al inicio de la aplicación para establecer la "Normalidad" estática de la red en ese datacenter.
- CÁLCULO: Validar el valor de "Receive Time". La marca de tiempo que adjunta Kraken en su orden L2 vs el `Time.now()` local al desempaquetarlo. Eso es el retraso "Half-Route" real (One-way latency).
- POST: Realimentar las predicciones (Skill 9 - Probabilidad Bayesiana) con los datos de Latencia. (A mayor latencia, menor probabilidad de que el spread siga existiendo).

## 11. Criterios de aprobación
- La medición activa e inactiva (Pasiva) produce datos estables en la matriz general con una varianza razonable.
- El módulo corre en background sin consumir más del 1% del tiempo de CPU.

## 12. Criterios de rechazo
- El desvío (Drift) temporal con el servidor remoto no es recuperable mediante un simple offset y supera los miles de milisegundos (Clock failure severo).
- Pérdida de paquetes (Packet loss) sostenida > 2% descubierta durante los pings continuos.

## 13. Riesgos que mitiga
- Error "Timestamp for this request is outside of the recvWindow" de Binance/OKX que arruina envíos de órdenes perfectas.
- HFT Sniper Trap: Competir asumiendo que tienes 5ms de latencia porque el servidor está en la misma ciudad, sin saber que el ruteo interno (VPC) o balanceo de carga estropea tu latencia a 85ms, haciéndote perder cada trade de front-running.

## 14. Integración con otras skills
- Proporciona el "Offset de Latencia" al Motor CEX-CEX (Skill 12) para que apriete el gatillo en distintos momentos logrando recepción simultánea.
- Controla los tiempos de timeout exigidos en WebSockets (Skill 31).

## 15. Modelo de datos sugerido
```json
{
  "NetworkTelemetry": {
    "target": "binance_spot_api",
    "rtt_latency_ms": 14.5,
    "jitter_ms": 1.2,
    "clock_offset_ms": -45,
    "packet_loss_pct": 0.0,
    "state": "HEALTHY",
    "p99_latency_ms": 22.0
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Background (Telemetry Worker) corriendo aisladamente y enviando actualizaciones por variables de estado atómicas (`AtomicI32`/`SharedArrayBuffer`) al Hilo Principal para no interferir en la latencia pura.

## 17. Logs obligatorios
- `[DEBUG] Telemetry Updated: Binance=14ms, Kraken=35ms. Master Offset=21ms. Clock Drift= -12ms.`
- `[WARN] Network Spike detected on OKX WSS connection (RTT > 150ms). Status changed to DEGRADED. Pausing dependent arb modules.`
- `[CRITICAL] PTP Clock offset exceeded 5000ms. Host system time is fatally out of sync. Correcting internally to avoid API Ban.`

## 18. Métricas obligatorias
- `network_rtt_latency_ms_per_exchange`.
- `clock_drift_correction_applied_ms`.
- `latency_spikes_detected_daily` (Analiza la fiabilidad del proveedor VPS donde está alojado el bot).

## 19. Tests unitarios
- One-Way Latency Extraction: Recibir un JSON mock con timestamp remoto `T=1000`, procesarlo localmente simulando reloj `T=1020`. La latencia inferida debe calcularse en `20ms`.
- Jitter Smoothing: Inyectar pings constantes de 10ms, seguido de un ping de 300ms. El EWMA debe atenuar el pico pero marcar el estado como Degradado (Yellow) temporalmente.
- Monotonic vs System Time: Probar que adelantar el reloj del sistema 1 hora con un mock del OS no afecta la métrica de latencia medida localmente.

## 20. Tests de integración
- Enviar 10 llamadas REST reales a los endpoints `/api/v3/ping` o `/api/v3/time` de los principales CEX y validar la recepción y cálculo del offset frente al reloj de Node.js o Rust local.

## 21. Tests E2E
- El bot principal despierta, solicita permiso de la Telemetría. La Telemetría le dice "Kraken 50ms, Binance 10ms". El bot identifica spread. Retrasa intencionalmente el envío a Binance por 40ms enviando Kraken primero. Las órdenes llegan al mismo milisegundo absoluto a ambos datacenters en continentes distintos y el spread se cobra limpio de "Leg Risk".

## 22. Checklist de producción
- [ ] Incorporar "Ping Frames" nativos de WebSocket en la capa de telemetría pasiva para mayor precisión vs HTTP.
- [ ] Aplicar "Nagle's Algorithm off" (`TCP_NODELAY`) obligatoriamente en todos los sockets del sistema operativo o node engine para evitar buffer artificial de 40ms nativos en Linux.
- [ ] Implementar la inyección del Offset al constructor de clientes API global: `Exchange_Time = Local_Time + Master_Offset`.

## 23. Ejemplo de configuración no hardcodeada
```yaml
telemetry_engine:
  ping_interval_seconds: 5
  jitter_acceptable_threshold_ms: 10
  latency_spike_timeout_seconds: 15
  enable_tcp_no_delay_override: true
```

## 24. Ejemplo de pseudocódigo
```javascript
class PingMonitor {
    constructor() {
        this.metrics = new Map();
        this.masterOffset = 0;
    }

    async pingRestEndpoint(name, url) {
        const start = performance.now();
        const startSystem = Date.now();
        
        try {
            const response = await fetch(url);
            const serverTime = parseInt(response.headers.get("x-mbx-time") || Date.now()); // Exchange specific
            const rtt = performance.now() - start;
            
            // Calculate one-way estimated delay (half RTT)
            const oneWay = rtt / 2;
            
            // Clock drift: Expected local time vs Server Time
            const clockDrift = serverTime - (startSystem + oneWay);

            this.updateEWMA(name, rtt, clockDrift);
            
        } catch (e) {
            this.handlePingFailure(name);
        }
    }

    updateEWMA(name, newRtt, drift) {
        let stats = this.metrics.get(name) || { rtt: newRtt, jitter: 0, offset: drift };
        
        // EWMA applied
        stats.jitter = stats.jitter * 0.9 + Math.abs(newRtt - stats.rtt) * 0.1;
        stats.rtt = stats.rtt * 0.8 + newRtt * 0.2;
        stats.offset = stats.offset * 0.9 + drift * 0.1; // Smooth clock sync
        
        if (stats.rtt > CONFIG.latency_spike_limit) {
            EventBus.emit('NETWORK_SPIKE', name);
        }
        
        this.metrics.set(name, stats);
    }
}
```

## 25. Criterio final de excelencia
El mapeador de red actúa como un radar militar avanzado de "Targeting". En un mundo donde todo el software matemático es idéntico, la victoria del Arbitraje la determina quién controla y manipula el tiempo absoluto de llegada de los paquetes físicos de red; esta skill es el arma principal para esa victoria.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Fluctuaciones imperceptibles (Micro-bursts) dentro de la red privada de un proveedor de Cloud que destruyen la métrica justo en el frame crítico de 10 milisegundos de la ejecución.
- Dependencias: Soporte de Alta Resolución Temporal del Sistema Operativo.
- Próxima skill: Rate limit bypass strategies (Skill 35).
