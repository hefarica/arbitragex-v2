# SKILL 022 — Gestión dinámica de RPCs

## 1. Propósito superior
Asegurar conectividad ininterrumpida, balanceada y de mínima latencia hacia los nodos de la blockchain. Dado que los proveedores públicos y privados (Infura, Alchemy, Ankr, LlamaRPC) sufren degradación de rendimiento, congestión, rate limits o caídas totales (Downtimes) constantemente, esta skill orquesta un enrutador inteligente de nodos (RPC Router) que conmuta milimétricamente el tráfico hacia el proveedor más saludable, garantizando que el bot nunca pierda un bloque por culpa de la infraestructura.

## 2. Nivel de conocimiento requerido
Arquitecto de Infraestructura HFT, Site Reliability Engineering (SRE), y Redes de Computadoras. Entendimiento profundo del protocolo HTTP/2, WebSockets, mecanismos de Load Balancing (Round Robin, Least Connections, Latency-Based Routing), Health-Checking asíncrono y métricas de red (Jitter, Packet Loss, Time-To-First-Byte).

## 3. Capacidades principales
1. Ponderación dinámica (Scoring) de todos los RPC endpoints basados en Latencia, Tasa de Error y Frescura del bloque (`eth_blockNumber`).
2. Implementación de Circuit Breakers independientes por cada proveedor RPC.
3. Balanceo de carga inteligente: Enviar lecturas pesadas (Multicalls) a nodos con alto límite de computo (Compute Units) y transacciones críticas a nodos especializados en Mempool/MEV (Flashbots).
4. Detección de "Silent Desync": Un nodo responde en 10ms pero con el estado de hace 50 bloques. La skill lo penaliza inmediatamente.
5. Suscripción cruzada de WebSockets (Subscribing to `newHeads` en 3 nodos distintos, tomando siempre el que notifique primero).
6. Retry Policies automáticos: Si `eth_estimateGas` falla por error transitorio de servidor, se reintenta atómicamente en el siguiente RPC de la lista.
7. Fallback escalonado: Primario (Pago/Dedicado) -> Secundario (Pago/Compartido) -> Terciario (Agregador Público gratuito).
8. Manejo integrado de Rate Limits (429 Too Many Requests) y Backoff Headers (`Retry-After`).
9. Rotación de llaves API para maximizar Free Tiers en redes secundarias.
10. Generación de telemetría de proveedores para optimización de presupuestos mensuales (saber qué nodo cobra mucho pero rinde poco).

## 4. Entradas requeridas
- `rpc_endpoints_config`: Lista JSON/YAML de proveedores, URLs, claves API, WebSocket URLs y límites de velocidad declarados.
- `network_requests`: Peticiones en bruto lanzadas por las skills (Ej. Lectura On-Chain, Simulador de Gas, Tx Broadcast).
- `expected_block_time`: Tiempo estimado por bloque de la cadena objetivo (Ej. 12s Ethereum, 0.25s Arbitrum).

## 5. Salidas esperadas
- `routed_response`: La respuesta cruda de la blockchain al solicitante.
- `rpc_health_matrix`: Estado actual de todos los nodos (Score 0-100, Latency ms, Block lag).
- `failover_events`: Logs de alerta si el sistema debió cambiar de ruta principal.

## 6. Reglas inmutables
- Nunca casar el bot HFT a un solo proveedor RPC (Single Point of Failure). Mínimo 3 proveedores paralelos.
- Si un RPC devuelve una respuesta HTTP 200 pero el bloque de la respuesta es menor a la altura máxima conocida (`Highest Known Block`), el payload DEBE ser descartado y marcado como "Stale", restando Score al proveedor.
- No gastar créditos pagos (Compute Units) de Alchemy/Infura para tareas banales si hay RPCs públicos rindiendo a latencias menores de 100ms.
- El tiempo total de "decisión de ruteo" debe ser < 1ms, para no añadir overhead a la petición nativa.

## 7. Algoritmos o métodos que debe conocer
- Weighted Least Connections Balancing.
- Exponential Weighted Moving Average (EWMA) para suavizar las métricas de latencia y no reaccionar exageradamente a un micro-spike.
- Ruteo por Contexto (Read vs Write segregation).
- Detección de particiones de red (Split-brain EVM nodes).

## 8. Fórmulas críticas
- **RPC Score**: `(Weight_Lat * (1/Latency)) + (Weight_Sync * (Highest_Block - Node_Block)) - (Penalty * Error_Count)`
- **Umbral de Desincronización (Desync)**: `|Node_Block - Highest_Known_Block| > Max_Tolerance`
- **Fallback Threshold**: `Latency > Config.max_acceptable_ping_ms`

## 9. Casos extremos
- Un hard-fork en la cadena genera 2 ramas. Alchemy sigue la Rama A, Infura sigue la Rama B. El gestor de RPCs recibe información caótica y contradictoria (Split-brain).
- El Creador de un RPC público inyecta silenciosamente retraso (Tarpit) o manipula datos de balances para atrapar bots (Malicious Node).
- Lluvia de Errores 429 simultáneos en todos los proveedores debido a un pico de mercado salvaje.

## 10. Validaciones obligatorias
- PRE: Chequear el Health Score del Nodo seleccionado en caché antes de enviar el Socket request.
- CÁLCULO: Validar si la petición es `eth_sendRawTransaction`. Si lo es, NUNCA usar nodos públicos estándar (riesgo de front-run), enrutar obligatoriamente a MEV Endpoints protegidos.
- POST: Actualizar EWMA de latencia con el tiempo exacto (`Time.now() - Start_Time`) una vez recibida la data válida.

## 11. Criterios de aprobación
- La petición JSON-RPC se completó devolviendo la respuesta esperada por el protocolo.
- El número de bloque (`eth_blockNumber`) devuelto es `>= Highest_Known_Block - 1`.

## 12. Criterios de rechazo
- El nodo responde en 500ms (Degradación). Baja Score y pasa la próxima petición al nodo B.
- HTTP 5XX / 429 Error. El nodo entra en "Cooldown" por N segundos.

## 13. Riesgos que mitiga
- Riesgo de Opacidad (Blindness): El bot de arbitraje se queda ciego en medio de un Flash Crash porque Infura colapsó, perdiendo oportunidades millonarias.
- Falsos Positivos de Arbitraje: Creer que hay un spread masivo entre DEX y CEX, cuando en realidad el RPC del DEX lleva estancado 5 minutos leyendo un bloque antiguo (Ghost spread).

## 14. Integración con otras skills
- Interfaz base absoluta para Lectura On-Chain (Skill 21).
- Proveedor de oráculos para Simulación Pre-trade On-Chain (Skill 29).

## 15. Modelo de datos sugerido
```json
{
  "RpcHealthMonitor": {
    "chain": "polygon",
    "primary_node": "alchemy_poly_mainnet",
    "nodes_status": [
      { "id": "alchemy", "ping_ms": 32, "block": 50123000, "score": 98 },
      { "id": "ankr_public", "ping_ms": 150, "block": 50122998, "score": 45, "cooldown": true },
      { "id": "local_erigon", "ping_ms": 2, "block": 50123000, "score": 100 }
    ],
    "total_requests_routed": 150045,
    "fallback_events": 12
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Un Interceptor / Middleware asíncrono para las librerías Ethers/Viem (Ej. Custom `FallbackProvider` supercargado).

## 17. Logs obligatorios
- `[DEBUG] Routed eth_call to Local_Erigon. Latency: 2ms.`
- `[WARN] Alchemy returned Stale Block (Lag: -3). Downgrading score from 98 to 65. Falling back to QuickNode.`
- `[CRITICAL] All Primary RPCs degraded for Polygon! Operating in degraded mode via public aggregators.`

## 18. Métricas obligatorias
- `rpc_pool_health_score_avg`.
- `requests_failed_over_count`.
- `latency_per_provider_histogram`.

## 19. Tests unitarios
- Score Penalties: Simular que un nodo responde con `status 429`, verificar que su score cae por debajo del umbral de activación instantáneamente.
- Stale Block Detection: Inyectar un JSON-RPC response con un `blockNumber` inferior al último registrado, validar que lanza la excepción interna y oculta el resultado al nivel superior.
- Read/Write Segregation: Intentar enviar `eth_sendRawTransaction` y verificar que el enrutador bloquea el nodo público y lo envía por Flashbots.

## 20. Tests de integración
- Levantar un servidor local express devolviendo JSON-RPC y simular latencias dinámicas (20ms, luego 500ms, luego error). Observar cómo el router salta de nodo sin cortar el flujo principal de promesas.

## 21. Tests E2E
- Desconectar físicamente (matar la conexión TCP) al nodo primario (Infura) en medio de un bucle de escaneo masivo del bot. El bot debe emitir un WARN y seguir funcionando con 0% de interrupción en la lectura, balanceando hacia el nodo secundario en el mismo tick.

## 22. Checklist de producción
- [ ] Incorporar `Flashbots` y `MEV-Share` en una lista blanca estricta (Whitelist) de endpoints de escritura (Write-only).
- [ ] Script de Watchdog que reinicie la conexión WebSocket (WSS) si no se reciben Pings/Heartbeats o mensajes de "newHeads" en 2x el Block Time normal (Ej. 24 segundos en Ethereum).
- [ ] Optimizar la caché de peticiones DNS para evitar latencia de resolución en los saltos de nodo.

## 23. Ejemplo de configuración no hardcodeada
```yaml
dynamic_rpc_manager:
  network: "arbitrum_one"
  expected_block_time_ms: 250
  max_acceptable_latency_ms: 100
  write_endpoints: 
    - "https://rpc.flashbots.net/arbitrum"
  read_endpoints:
    - url: "https://arb-mainnet.g.alchemy.com/v2/KEY"
      priority: 1
      type: "wss"
    - url: "https://rpc.ankr.com/arbitrum"
      priority: 2
      type: "http"
```

## 24. Ejemplo de pseudocódigo
```python
class DynamicRpcRouter:
    def __init__(self, endpoints):
        self.nodes = [RpcNode(cfg) for cfg in endpoints]
        self.highest_block = 0
        
    def get_best_node(self, is_write_operation=False):
        valid_nodes = [n for n in self.nodes if not n.is_cooldown and n.score > MIN_SCORE]
        if is_write_operation:
            return next(n for n in valid_nodes if n.is_mev_protected)
            
        # Sort by EWMA latency and freshness score
        valid_nodes.sort(key=lambda n: n.score, reverse=True)
        return valid_nodes[0]
        
    async def route_request(self, method, params):
        while True:
            node = self.get_best_node()
            try:
                start_time = time.now()
                response = await node.send(method, params)
                latency = time.now() - start_time
                
                # Check Stale Data
                if response.block_number < self.highest_block:
                    node.penalize("stale_data")
                    continue # Failover instantly
                    
                self.highest_block = max(self.highest_block, response.block_number)
                node.update_latency_ewma(latency)
                
                return response
                
            except RateLimitError:
                node.apply_cooldown(30) # 30 seconds HTTP 429
            except TimeoutError:
                node.penalize("timeout")
```

## 25. Criterio final de excelencia
El enrutador crea una capa de "Súper-Nodo" invulnerable que siempre ofrece respuestas en latencias de percentil 99 (P99) inferiores a 50ms, evadiendo fallos catastróficos de proveedores de nube de manera completamente invisible para la lógica de negocio.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Congestión masiva global en el protocolo HTTP base (Global backbone routing issues).
- Dependencias: Telemetría de Red.
- Próxima skill: Multicall avanzado (Skill 23).
