# SKILL 004 — Teoría de grafos para detección de ciclos rentables

## 1. Propósito superior
Mapear dinámicamente todo el ecosistema de exchanges, DEXes y pares de trading como un grafo dirigido de alta frecuencia. Su objetivo es detectar ciclos cerrados (ej. A -> B -> C -> D -> A) donde el peso neto del recorrido indica una ganancia. Permite explorar topologías de mercado no evidentes que eluden a los bots que solo miran pares estáticos bidireccionales.

## 2. Nivel de conocimiento requerido
Máster/PhD en Ciencias de la Computación, especializado en Algoritmia de Grafos, Estructuras de Datos Avanzadas e Investigación de Operaciones. Conocimiento profundo de algoritmos de búsqueda (DFS, BFS adaptado, Yen's K-Shortest Paths, Tarjan para componentes fuertemente conexos) aplicados a topologías dinámicas.

## 3. Capacidades principales
1. Mantenimiento en memoria (in-memory) de un Grafo Dirigido Ponderado con miles de nodos y aristas.
2. Actualización atómica de pesos de aristas en < 100 microsegundos por cada evento de tick del mercado.
3. Detección de ciclos negativos (bajo modelo logarítmico) que representan oportunidades de arbitraje.
4. Identificación de componentes fuertemente conexos para aislar "islas de liquidez" operables.
5. Poda de grafos (Graph Pruning) para eliminar nodos ilíquidos o exchanges caídos sin recalcular toda la estructura.
6. Manejo de múltiples aristas entre los mismos nodos (Multigrafo), por ejemplo, ETH/USDC en Uniswap V2, V3, SushiSwap, y Binance.
7. Cálculo de K-caminos más cortos para proveer rutas alternativas de ejecución al motor de routing.
8. Gestión de puentes cross-chain como aristas con penalidad de tiempo asíncrono.
9. Uso de estructuras dispersas (Adjacency Lists) optimizadas para caché del CPU.
10. Sincronización libre de bloqueos (Lock-free data structures) para actualizaciones por workers concurrentes.

## 4. Entradas requeridas
- `market_events`: Stream de actualizaciones de precios y liquidez (Order book updates).
- `exchange_topology`: Lista de mercados activos, pausados o en mantenimiento.
- `fees_structure`: Estructura de comisiones de cada mercado/pool.
- `gas_edges`: Costo dinámico en USD convertido al peso del grafo para operaciones on-chain.

## 5. Salidas esperadas
- `cycle_list`: Array de ciclos encontrados, ordenados por rentabilidad teórica neta.
- `graph_health_score`: Métrica de integridad estructural (ej. % de nodos inalcanzables).
- `isolated_subgraphs`: Alerta sobre ecosistemas aislados donde un trade quedaría atrapado.

## 6. Reglas inmutables
- El grafo debe representar la realidad direccional: vender BTC por USDT no tiene el mismo costo/peso ni usa la misma profundidad que comprar BTC con USDT.
- Las aristas de puentes cross-chain o retiros de CEX deben incorporar un "Time Weight" (Riesgo de latencia).
- Los algoritmos de búsqueda deben tener un límite de profundidad (`max_depth` = 5 o 6) para evitar explosión combinatoria.
- Ninguna ruta se marca como viable si depende de un nodo (activo) cuya API o Blockchain está pausada/congestada.

## 7. Algoritmos o métodos que debe conocer
- Depth-First Search (DFS) acotado para búsqueda exhaustiva de ciclos pequeños.
- Johnson's Algorithm para All-Pairs Shortest Paths en grafos dispersos.
- Tarjan's Algorithm o Kosaraju's para identificar clusters de liquidez.
- Suurballe's Algorithm para rutas disjuntas (útil para dividir ejecuciones grandes).

## 8. Fórmulas críticas
- **Peso de la Arista (Multiplicativo a Aditivo)**: `W(u, v) = -ln(Rate(u, v) * (1 - Fee(u, v)))`
- **Condición de Ciclo Rentable**: La suma de los pesos de las aristas del ciclo `C` es estrictamente menor a cero: `Σ W(u, v) < 0` para `(u,v) ∈ C`.
- **Umbral de Activación**: `Σ W(u, v) < -ln(1 + min_net_roi_config)`.

## 9. Casos extremos
- Ciclos ilusorios generados por activos con volumen cero pero spread nominal atractivo.
- Nodos "Sink" (activos de los que es fácil comprar pero imposible vender por liquidez falsa).
- Latencia asimétrica: El tick de Binance llega en 50ms, el tick de DEX en Solana en 400ms. El grafo mezcla datos de distintos tiempos.
- Aristas duplicadas en multígrafos donde una ruta es barata pero ilíquida, y otra es cara pero muy líquida.

## 10. Validaciones obligatorias
- PRE: Validar que ninguna arista tenga un peso derivado de un timestamp mayor a 1 segundo.
- CÁLCULO: Validar heurística de poda (ej. no explorar si el spread inicial mata el 80% del profit requerido).
- POST: Cada ruta extraída del grafo debe ser enviada al Motor de Matemática de Arbitraje (Skill 1) para un chequeo de slippage exacto.

## 11. Criterios de aprobación
- El ciclo encontrado contiene activos iniciales y finales que coinciden con el wallet/balance base del operador.
- El peso negativo total supera el umbral de gas estimado para toda la ruta.

## 12. Criterios de rechazo
- El ciclo detectado depende de un nodo marcado como "Illiquid" o "Halted".
- El algoritmo excedió el límite de iteraciones (timeout forzado) previniendo bloqueo del hilo.

## 13. Riesgos que mitiga
- Riesgo Estructural: Operar en pares que matemáticamente dan ganancia pero que en el mundo real están aislados (ej. tokens pausados en un CEX).
- Bloqueo de Capital: Ejecutar el tramo 1 y darse cuenta de que el tramo 2 no existe porque no hay un camino transitable de regreso a la moneda base.

## 14. Integración con otras skills
- Es la estructura de datos subyacente para Bellman-Ford (Skill 5).
- Recibe inputs de Data Normalization (Skill 32).
- Pasa los ciclos crudos a la Optimización Convexa de Tamaño (Skill 2).

## 15. Modelo de datos sugerido
```json
{
  "GraphCycle": {
    "cycle_id": "uuid",
    "nodes": ["USDC", "WETH", "WBTC", "USDC"],
    "edges": ["uniswap_v3", "binance", "curve"],
    "total_log_weight": -0.0015,
    "theoretical_gross_roi": 0.001501
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en segundo plano (Worker) iterando sobre la estructura de grafo alojada en memoria compartida (Shared Memory) o Redis Graph (aunque memoria nativa Rust/C++ es preferida por latencia).

## 17. Logs obligatorios
- `[INFO] Cycle detected in subgraph: USDC -> WETH (UniV3) -> WBTC (Binance) -> USDC (Curve). Weight: -0.002.`
- `[WARN] Edge WETH -> WBTC on exchange X marked stale (age > 500ms). Pruned from graph.`

## 18. Métricas obligatorias
- `graph_node_count` y `graph_edge_count`.
- `cycle_detection_latency_us` (Microsegundos).
- `pruned_edges_per_second`.

## 19. Tests unitarios
- Crear un grafo sintético con 4 nodos y 1 oportunidad; el algoritmo debe encontrarla.
- Inyectar 10,000 nodos aleatorios sin oportunidades; el algoritmo debe retornar vacío sin crashear ni demorar > 10ms.
- Test de multígrafo: Dos aristas entre A y B, el algoritmo debe siempre preferir la de menor peso logarítmico condicionado a la liquidez.

## 20. Tests de integración
- Conexión del grafo con un feed simulado de precios a 500 msg/s; verificar que la estructura se mantiene coherente sin dataraces.

## 21. Tests E2E
- El agente carga el grafo con topología de Binance y Uniswap reales, detecta un ciclo y pasa el objeto exacto a la DB.

## 22. Checklist de producción
- [ ] Uso de listas de adyacencia planas en arrays pre-asignados (Zero-allocation arrays) para evitar pausas del Garbage Collector.
- [ ] Soporte para invalidación atómica de nodos (e.g. un exchange informa downtime).
- [ ] Timeout duro por ciclo de búsqueda.

## 23. Ejemplo de configuración no hardcodeada
```yaml
graph_engine:
  max_depth: 4
  pruning_threshold_bps: 50
  gc_pause_optimization: true
  max_edges_per_node: 10
```

## 24. Ejemplo de pseudocódigo
```python
def find_arbitrage_cycles(graph, start_node, max_depth, min_weight_threshold):
    cycles = []
    
    def dfs(current_node, visited, current_weight, path):
        if len(path) > max_depth:
            return
            
        for edge in graph.get_out_edges(current_node):
            if not edge.is_active or edge.liquidity < MIN_LIQ:
                continue
                
            new_weight = current_weight + edge.weight
            
            if edge.target == start_node and len(path) > 1:
                if new_weight < min_weight_threshold:
                    cycles.append((path + [edge.target], new_weight))
                continue
                
            if edge.target not in visited:
                dfs(edge.target, visited | {edge.target}, new_weight, path + [edge.target])
                
    dfs(start_node, {start_node}, 0.0, [start_node])
    return cycles
```

## 25. Criterio final de excelencia
El motor de grafos puede mantener 10,000 nodos y 50,000 aristas actualizándose a 1,000 Hz, detectando ciclos de hasta 4 patas en menos de 500 microsegundos en un solo core de CPU, sin pérdidas de memoria ni falsos positivos matemáticos.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Costo de latencia en la mutación concurrente del grafo.
- Dependencias: Álgebra lineal, Manejo de memoria eficiente.
- Próxima skill: Bellman-Ford para arbitraje negativo-logarítmico (Skill 5).
