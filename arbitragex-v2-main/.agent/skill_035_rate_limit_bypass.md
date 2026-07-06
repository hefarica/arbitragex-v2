# SKILL 035 — Rate limit bypass strategies

## 1. Propósito superior
Garantizar supervivencia de acceso a las APIs en un entorno intensivo (HFT - High Frequency Trading). Los exchanges centralizados (CEX) y proveedores de red imponen castigos severos (IP Bans de 3 a 30 días, bloqueos HTTP 429) por saturar sus endpoints. Esta skill organiza un gestor dinámico de cuotas, "Token Buckets" distribuidos, y enrutamiento inteligente (Proxies, Múltiples API Keys) para burlar la asfixia de límites teóricos manteniendo el bombardeo ininterrumpido necesario para el Arbitraje.

## 2. Nivel de conocimiento requerido
Experto en Ingeniería Inversa de Sistemas Rate-Limit (Leaky Bucket, Token Bucket, IP-based vs Account-based throttling), Arquitectura de Rotación de Proxies, HTTP Header Parsing (`X-MBX-USED-WEIGHT`), Manejo de Identidades Sintéticas (Multiple sub-accounts) y Patrones de Distribución de Carga Distribuida a lo largo de un clúster de bots.

## 3. Capacidades principales
1. Tracking local estricto ("Token Bucket Algorithm") simulando exactamente la contabilidad del servidor del Exchange (sabiendo que tienes 1199/1200 créditos usados sin hacer la petición final).
2. Manejo de Weights (Pesos): Distinguir entre peticiones baratas (Ping = 1 weight) y peticiones masivas (Orderbook 500 levels = 50 weights) deduciendo correctamente.
3. Rotación de Cuentas (API Key Pooling): Mantener un pool de 10+ subcuentas maestras y rotar las transacciones entre ellas (Round Robin/Least Used) si el Rate Limit es "Account-based".
4. Rotación de IP (Proxy Manager): Usar un pool de IPs dedicadas (AWS Elastic IPs o Proxies residenciales limpios) para evadir Rate Limits que son "IP-based".
5. Lectura dinámica de Headers (Feedback Loop): Interpretar en tiempo real los headers de cada respuesta HTTP del exchange que te dicen exactamente cuántos puntos te quedan, corrigiendo la matemática local.
6. Auto-Throttle adaptativo: Si la cuota restante llega al 90%, el sistema entra en "Modo Discreción" retrasando o suspendiendo tareas no críticas (Ej. Actualizar saldo REST) priorizando 100% las ejecuciones críticas de mercado.
7. Manejo automático del castigo pasivo `Retry-After`: Capturar silenciosamente el Error 429 (Too Many Requests), pausar la ruta específica por los segundos ordenados, y encolar las peticiones no críticas sin crashear el bot.
8. Prevención de ráfagas destructivas (Bursts): Usar colas en forma de túnel estrecho (Traffic Shaping) para que, si 50 módulos del bot piden algo a Binance en el mismo milisegundo, la petición se alise y no salte las alarmas de DDoS.
9. Orquestación Multi-Capa: CEX imponen límites por Segundo, por Minuto y por Día. La skill monitoriza los 3 vectores de tiempo tridimensionalmente.
10. Protección contra Shadowbans: Identifica si un exchange retrasa maliciosamente las peticiones (Ping de 500ms repentino) en lugar de dar un 429 explícito (Tarpitting detection).

## 4. Entradas requeridas
- `api_requests_queue`: Fila de peticiones entrantes generadas por todos los demás módulos.
- `rate_limit_rules`: JSON descriptivo de las reglas por exchange (Ej. `Binance: 1200 weight / minute`).
- `credentials_pool` / `proxies_pool`: Recursos paralelos a consumir para escalar horizontalmente.

## 5. Salidas esperadas
- `request_dispatcher`: Liberación de la promesa (`resolve/reject`) para enviar la petición de red con luz verde.
- `throttling_alert`: Bandera asíncrona enviada a todos los módulos: `"SLOW_DOWN_WARNING"`.
- `exhausted_resources_log`: Informe de APIs quemadas o pausadas temporalmente.

## 6. Reglas inmutables
- JAMÁS realizar una petición REST `fire-and-forget` suelta por fuera del Interceptor (Rate Limit Bypass Engine). Toda comunicación CEX debe estar subyugada a esta tubería para contabilidad perfecta.
- Si el "Token Bucket" local llega a un `Warning Level` predefinido (Ej. 95%), TODAS las operaciones de recolección de datos REST se pausan estricta y dolorosamente para reservar los escasos tickets restantes para la ejecución de "Cancel/Place Orders" puras en caso de atrapar un arbitraje masivo.
- Cero tolerancia al IP BAN. Acumular un error 418 o ban de 3 días en Binance es un fallo sistémico irrecuperable. Es mejor ralentizar el bot 5 segundos que estar offline 3 días.
- Diferenciar el Ruteo por Contexto: IP-Ban es salvado por un Proxy. Account-Ban es salvado por Sub-cuentas. El router debe saber la naturaleza de la limitación del exchange.

## 7. Algoritmos o métodos que debe conocer
- Token Bucket / Leaky Bucket Implementation (Data Structures).
- Round-Robin y Consistent Hashing para Sticky Sessions sobre un pool de sub-cuentas.
- Distributed Rate Limiting (Si se corren 5 instancias Docker del bot, deben compartir el estado límite mediante Redis / Shared Memory, no contar aisladamente).

## 8. Fórmulas críticas
- **Cálculo de Cuota Residual**: `Available = Max_Tokens - Sum(Weight_Used) + (Time_Passed_Secs * Refill_Rate)`
- **Condición de Disparo Ponderado**: `if (Available - Request_Weight >= Safety_Margin) { Emit() } else { Delay() }`
- **Límite de Riesgo de Asfixia**: `Utilized_Weight_1_Minute > Max_Weight * 0.90` (Trigger Alerta Amarilla).

## 9. Casos extremos
- Asincronía Engañosa (Race Condition masiva): 100 Promesas concurrentes disparan a la vez consultando el `Available Quota`. Como todas leen la variable al mismo tiempo (ej. 100 > 5), todas pasan, vacían el límite, llegan al servidor del Exchange y provocan un Ban en 1 milisegundo. Esto se previene aplicando deducciones atómicas (Mutex/Lock) pre-vuelo.
- "Weighted Endpoints" trampa: Un endpoint de API que usualmente cuesta 1 crédito, de repente en medio de un evento hiper volátil (Black Swan) el Exchange lo cambia dinámicamente a que cueste 50 créditos sin advertirlo. El bot envía 20 peticiones y causa Ban. Requiere actualización heurística leyendo Headers de respuesta en cada petición de forma reactiva.
- Caída de Websocket: Si Websocket se corta, el bot intenta pedir REST para resincronizar agresivamente. La lluvia de REST satura el límite de 1 Minuto y genera Ban. Se previene con Backoff.

## 10. Validaciones obligatorias
- PRE: Chequear dinámicamente el `Current_Weight`. Si la petición es de "Ejecución Crítica" (Orden de compra con Profit), permite vaciar el balde al máximo o tomar un salto proxy prioritario.
- CÁLCULO: Mantener temporizadores de limpieza (Sliding Windows vs Fixed Windows). Binance usa "Fixed Window" al minuto exacto, mientras otros CEX usan "Sliding Window". La matemática del bucket debe simular con exactitud al protocolo origen.
- POST: Leer obligatoriamente los Header de la respuesta (e.g. `X-MBX-USED-WEIGHT-1M`). Si el valor reportado por el servidor difiere del Local por más del 10%, re-sincronizar violentamente la memoria local con el servidor para acatar su palabra de Dios.

## 11. Criterios de aprobación
- La petición avanza de forma asincrónica si y solo si la cuota lo permite o el Multi-Proxy provee rotación fresca.
- Las penalizaciones `Retry-After` son absorbidas, acatadas al milisegundo local y no propician errores irrecuperables del Agente Supremo.

## 12. Criterios de rechazo
- El sistema alcanza el Cap Duro local (Hard Limit) del 99%. Todas las operaciones no-HFT son rechazadas devolviendo error interno de límite local `Error("Local Rate Limit Firewall Triggered")`.
- Rotación masiva fallida: Todos los 10 proxies designados están en modo "Rate Limited" simultáneamente. (Situación de Apagón general).

## 13. Riesgos que mitiga
- IP Banning "Guillotine": Perder conexión por 3 días por culpa de un `for-loop` mal programado consultando precios de altcoins basura, impidiendo acceder al portafolio o desarmar posiciones con apalancamiento abierto (Riesgo absoluto de liquidación financiera del fondo).
- Latencia en colas pesadas: Las ejecuciones críticas quedan atascadas en colas detrás de miles de tareas asíncronas lentas e irrelevantes.

## 14. Integración con otras skills
- Interceptor pasivo para TODAS las operaciones API, pero esencial para Arbitraje Triangular (Skill 14) y CEX-CEX (Skill 12).
- Trabaja unificado con Multi-RPC Router (Skill 22) para evadir Rate Limits también de la blockchain.

## 15. Modelo de datos sugerido
```json
{
  "RateLimitManager": {
    "exchange": "binance",
    "window_type": "1_MINUTE_FIXED",
    "current_usage": 850,
    "max_allowed": 1200,
    "usage_pct": 70.8,
    "status": "HEALTHY",
    "active_proxy": "aws_ip_pool_3",
    "active_api_key_slot": 2,
    "throttled_queue_size": 15
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Singleton Orchestrator implementando el patrón Token Bucket con bloqueo asíncrono (Promises/Futures) e inyección de proxy y round-robin pool API Key Selector.

## 17. Logs obligatorios
- `[DEBUG] Request dispatched to OKX. Local Weight 45/1000. Header Weight confirmed 45/1000.`
- `[WARN] Binance 1M weight reached 90% (1080/1200). Activating THROW_NON_CRITICAL mode. Routing traffic to secondary API Pool.`
- `[CRITICAL] HTTP 429 received from Kraken. Retry-After 60s. Hard Pausing all REST loops.`

## 18. Métricas obligatorias
- `rate_limit_saturation_pct` (Vital monitorear esta métrica a nivel tablero general (Grafana/Dashboard) para saber si necesitas comprar más infraestructura/Proxies o el bot corre sobradamente).
- `proxy_rotation_events_count`.
- `rejected_tasks_due_to_throttle_count`.

## 19. Tests unitarios
- Sliding Window Emulation: Disparar 1199 eventos locales de "peso 1" usando un reloj mock (Mock Timer). Disparar el evento 1200 y debe ser rechazado. Avanzar el mock 60 segundos y debe permitir 1200 nuevamente de golpe.
- Dynamic Header Correction: Simular que el local tiene `500` consumidos, y el `Response Headers` reporta `800` (Fallo interno del servidor CEX o de cálculo propio). El bot debe ajustarse inmediatamente al valor `800` para protegerse de asimetrías.
- Priority Queue: Encolar 10 peticiones de baja prioridad `(Weight 1)` seguidas de 1 petición de altísima prioridad `(Ejecución Arbitraje, Weight 10)`. El encolador debe procesar primero la Crítica (Saltarse la fila) si la cuota escasea.

## 20. Tests de integración
- Cargar un pool de 3 IPs falsas y 3 API Keys Mockeadas. Simular 10,000 requests que el sistema repartirá balanceadamente (Load Balancing) entre las 9 combinaciones posibles simulando Round-Robin o saturación controlada, mitigando bloqueos individuales.

## 21. Tests E2E
- El agente inicia con la misión de scrapear violentamente históricos (Klines) para calcular modelos. Empieza a chupar toda la Data REST disponible. El "Rate Limit Manager" frena al Crawler al 90% y manda un flag de Warning. El agente recarga su pool de IPs y retoma sin detenerse y sin ser expulsado del exchange por el Firewall WAF (Web Application Firewall) de Cloudflare ni una vez en 24h.

## 22. Checklist de producción
- [ ] Incorporación de la regla `X-Forwarded-For` o Proxies reales (SOCKS5 o HTTP) para garantizar que los exchanges no compilen subcuentas rotantes como la misma cuenta si vienen de la misma IP pública ("IP-based Account Aggregation ban").
- [ ] Implementar un módulo distribuido en Redis (`INCR` atómico) si vas a lanzar Múltiples Servidores EC2 corriendo la misma estrategia (Swarm Bots), porque todos compartirán la misma API Key y se banearán entre sí.
- [ ] Reglas específicas por Endpoint. CEX imponen límites asimétricos (Cancelar orden = 1 weight, Consultar Orderbook completo = 50 weight, Solicitar reporte de cuenta = 20 weight). Todo debe estar harcodeado o cacheado dinámicamente según la documentación de cada CEX.

## 23. Ejemplo de configuración no hardcodeada
```yaml
rate_limit_engine:
  safety_margin_pct: 10
  throttle_non_critical_at_pct: 85
  enable_proxy_ip_rotation: true
  proxies:
    - "socks5://user:pass@ip1:port"
    - "socks5://user:pass@ip2:port"
  enable_api_key_rotation: false # Used when account limits apply, disabled if IP limits apply
  distributed_redis_tracking: false # False for single-node bots
```

## 24. Ejemplo de pseudocódigo
```javascript
class TokenBucketRouter {
    constructor(config) {
        this.maxWeight = config.maxWeight;
        this.currentWeight = 0;
        this.proxyPool = config.proxies;
        this.queue = new PriorityQueue(); // Critical first, Analytics last
        
        // Reset mechanism depending on exchange rules (Fixed 1M usually)
        setInterval(() => this.resetBucket(), 60000);
    }

    async enqueueRequest(requestObj, priority = "NORMAL") {
        return new Promise((resolve, reject) => {
            this.queue.push({ requestObj, priority, resolve, reject });
            this.processQueue(); // Async spin
        });
    }

    async processQueue() {
        if (this.queue.isEmpty() || this.isProcessing) return;
        this.isProcessing = true;

        while (!this.queue.isEmpty()) {
            const req = this.queue.peek();
            
            // Check hard limits
            if (this.currentWeight + req.requestObj.weight >= this.maxWeight * 0.9) {
                if (req.priority !== "CRITICAL") {
                    log.warn("Rate limit approaching. Suspending non-critical request.");
                    break; // Wait for next tick to replenish
                }
            }

            const item = this.queue.pop();
            this.currentWeight += item.requestObj.weight; // Pre-deduct atomically

            try {
                // Execute using proxy rotation for IP masking
                const proxy = getNextProxy(this.proxyPool);
                const response = await dispatchNetworkCall(item.requestObj, proxy);
                
                // Real-time correction using HTTP Response Headers
                const serverWeight = response.headers['X-MBX-USED-WEIGHT-1M'];
                if (serverWeight && serverWeight > this.currentWeight) {
                    this.currentWeight = serverWeight; // Server is always right
                }
                
                item.resolve(response);
            } catch (error) {
                if (error.statusCode === 429) {
                    const penalty = error.headers['Retry-After'] || 60;
                    this.activateHardPause(penalty);
                    this.queue.pushHighPriority(item); // Re-queue
                    break; // Stop completely
                }
                item.reject(error);
            }
        }
        this.isProcessing = false;
    }
}
```

## 25. Criterio final de excelencia
El gestor del Rate Limit permite al bot moverse de manera hiper-ofensiva rozando milimétricamente la línea de "Ban" del servidor enemigo sin tocarla jamás. Comprime el máximo valor de cada IP y de cada milisegundo otorgando un "Free Pass" perpetuo a los sistemas críticos mientras desecha ruido no rentable ante la asfixia estructural, demostrando conocimiento absoluto de la guerra electrónica HFT.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Bloqueos manuales por administradores de Cloudflare al detectar comportamientos excesivamente maquinales a nivel TLS (Fingerprinting). (Mitigado alterando JA3 fingerprints / HTTP/2 Headers randomization).
- Dependencias: Soporte de Promesas, Sistema Multi-Hilo (Si es posible).
- Próxima skill: Orquestador general de ejecución concurrente (Skill 36).
