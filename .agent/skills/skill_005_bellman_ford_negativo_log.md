# SKILL 005 — Bellman-Ford para arbitraje negativo-logarítmico

## 1. Propósito superior
Aplicar rigurosamente el algoritmo de Bellman-Ford sobre el grafo del ecosistema financiero para detectar ciclos de peso negativo absolutos. Dado que la conversión de tasas de mercado a logaritmos negativos convierte el arbitraje en un problema del "Camino Más Corto" (Shortest Path), esta skill garantiza matemáticamente que ninguna oportunidad estructural sea pasada por alto.

## 2. Nivel de conocimiento requerido
PhD/Máster en Algoritmia y Sistemas de Alta Frecuencia. Dominio de algoritmos de optimización de grafos, transformación logarítmica de divisas, relajación de aristas y mitigación de la complejidad asintótica `O(V*E)` en entornos de ultra-baja latencia.

## 3. Capacidades principales
1. Transformación on-the-fly de tasas bid/ask a logaritmos naturales negativos: `weight = -ln(rate)`.
2. Ejecución asíncrona y continua de la relajación de vértices `V-1` veces.
3. Iteración N-ésima para la detección certera y recuperación de vértices que componen el ciclo negativo.
4. Trazado inverso (Backtracking) desde el predecesor para reconstruir la ruta exacta del arbitraje.
5. Inserción dinámica de "Nodos Fantasma" (Dummy nodes) conectados a todo el grafo con peso 0 para detectar ciclos inconexos simultáneamente.
6. Algoritmo SPFA (Shortest Path Faster Algorithm) para optimizar el caso promedio de Bellman-Ford usando colas.
7. Precisión extrema de coma flotante para la acumulación de distancias para evitar derivas numéricas.
8. Adaptación a grafos multi-fuente (Single-Source Shortest Path modificado).
9. Descuento embebido de fees en el peso de la arista antes del logaritmo.
10. Aborto temprano (Early termination) si una iteración no produce relajaciones, ahorrando ciclos de CPU.

## 4. Entradas requeridas
- `vertices`: Lista `V` de todos los activos negociables.
- `edges`: Lista `E` de todos los pares direccionales con su `rate` (tasa), `fee`, y `liquidity`.
- `min_roi_bps`: Umbral de tolerancia de riesgo/ganancia (ej. 10 bps).

## 5. Salidas esperadas
- `negative_cycles_detected`: Array de secuencias de nodos (ej. `[A, B, C, A]`).
- `theoretical_roi`: Ganancia extraída tras reconvertir `exp(-total_weight)`.
- `execution_time_us`: Tiempo de resolución para monitor de rendimiento.

## 6. Reglas inmutables
- Toda tasa usada debe ser neta de fees proporcionales antes de aplicar el logaritmo (`Rate_neta = Rate_bruta * (1 - TakerFee)`).
- Nunca procesar aristas cuya liquidez disponible en USD sea inferior al monto mínimo del sistema.
- Se debe manejar la inestabilidad de coma flotante (Floating Point Underflow/Overflow).
- Todo ciclo detectado debe ser sometido a prueba de fuego contra liquidez real y costos fijos (gas) antes de la ejecución.

## 7. Algoritmos o métodos que debe conocer
- Bellman-Ford clásico `O(V*E)`.
- SPFA (Shortest Path Faster Algorithm) iterativo con detección de ciclos mediante contador de visitas `O(E)` promedio.
- Yen's Algorithm (si se requiere buscar el 2do o 3er mejor ciclo).
- Prevención de SLF (Small Label First) en SPFA para optimizar el ruteo de la cola.

## 8. Fórmulas críticas
- **Transformación de Tasa**: `W_{u,v} = -ln( Tasa_{u,v} * (1 - Fee_taker) )`
- **Condición de Relajación**: `if D[u] + W_{u,v} < D[v]: D[v] = D[u] + W_{u,v}; P[v] = u`
- **Condición de Arbitraje (Iteración V)**: `if D[u] + W_{u,v} < D[v]`, existe un ciclo negativo de arbitraje.
- **Retorno Esperado**: `ROI = exp(-Sum(W_{ciclo})) - 1`

## 9. Casos extremos
- Ciclos negativos compuestos únicamente por polvo de mercado (bajísimo volumen, altísimo spread).
- Tiempos de ejecución inaceptables en grafos masivos si no se usa SPFA.
- Valores de tasa iguales a 0 o negativos debido a bugs del feed (el `ln(0)` causa pánico del proceso).
- Ciclos en forma de "ocho" (dos ciclos superpuestos).

## 10. Validaciones obligatorias
- PRE: Validar que `Tasa > 0` y `1 - Fee > 0` para evitar logaritmos indefinidos.
- CÁLCULO: Validar contador de encolado en SPFA para romper si un nodo entra a la cola más de `V` veces.
- POST: Reconversión matemática obligatoria: Verificar `exp(-Sum_peso)` arroja > 1.0.

## 11. Criterios de aprobación
- La iteración final indica un ciclo negativo cuyo valor reconvertido excede el umbral de gas estimado.
- El ciclo detectado no involucra activos envueltos temporalmente ilíquidos (ej. un puente suspendido).

## 12. Criterios de rechazo
- El cálculo excede los 2 milisegundos en producción HFT.
- El ciclo detectado depende de un nodo que tiene `liquidez_efectiva < capital_minimo`.

## 13. Riesgos que mitiga
- Riesgo de Falsa Detección: Oportunidades visuales que al sumar los fees se vuelven pérdidas.
- Explosión de Complejidad: Algoritmos DFS ingenuos que evalúan billones de rutas. Bellman-Ford es polinómico, predecible y finito.

## 14. Integración con otras skills
- Es la "fuerza bruta matemática" complementaria a la detección heurística o algebraica (Skills 3 y 4).
- Pasa los ciclos al Simulador Pre-trade On-Chain (Skill 29) si involucra DEXes.

## 15. Modelo de datos sugerido
```json
{
  "BellmanFordResult": {
    "computation_id": "bf-9912",
    "vertices_count": 850,
    "edges_relaxed": 3400,
    "negative_cycle_found": true,
    "cycle_nodes": ["USDC", "WETH", "DAI", "USDC"],
    "implied_roi": 1.0021,
    "compute_latency_us": 850
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Demonio en Rust/Go compilado y vinculado vía WebAssembly o FFI a Node, disparado periódicamente o gatillado cuando el volatilidad global sube.

## 17. Logs obligatorios
- `[INFO] SPFA iteration triggered. V=850, E=3400. Cycle detected: [USDC->WETH->DAI->USDC].`
- `[DEBUG] Early termination of Bellman-Ford at iteration 4, no further relaxations.`

## 18. Métricas obligatorias
- `bellman_ford_latency_us`.
- `spfa_queue_operations_count` (Para monitorear degradación del caso peor de SPFA).
- `negative_cycles_filtered_by_liquidity`.

## 19. Tests unitarios
- Matriz 3x3 sintética con ciclo de ganancia: Debe retornar la secuencia exacta y el ROI preciso.
- Grafo sin ciclos rentables: Debe terminar en la iteración temprana sin errores.
- Prevención de `ln(0)`: Inyectar un rate de 0.0, debe filtrarlo o asignarle peso infinito.

## 20. Tests de integración
- Cargar un snapshot de memoria con 5,000 pares de Binance y ejecutar. Debe resolver en < 5ms.

## 21. Tests E2E
- El orquestador gatilla Bellman-Ford cada 100ms, detecta el ciclo, extrae el grafo y somete la operación al Risk Engine.

## 22. Checklist de producción
- [ ] Implementación de SPFA en lugar de Bellman-Ford estándar `V*E` para ganar velocidad de 10x-50x.
- [ ] Cola circular o Ring Buffer estático en SPFA para evitar Garbage Collection de arreglos.
- [ ] Uso de array plano 1D para los arreglos `Distance` y `Predecessor`.

## 23. Ejemplo de configuración no hardcodeada
```yaml
bellman_ford:
  algorithm_variant: "SPFA_SLF"
  precision_type: "f64"
  max_iterations_timeout_us: 2000
```

## 24. Ejemplo de pseudocódigo
```python
import math

def bellman_ford_arbitrage(vertices, edges, source_node):
    distances = {v: float('inf') for v in vertices}
    predecessor = {v: None for v in vertices}
    distances[source_node] = 0

    # SPFA optimization with queue
    queue = [source_node]
    in_queue = {v: False for v in vertices}
    in_queue[source_node] = True
    visit_count = {v: 0 for v in vertices}

    while queue:
        u = queue.pop(0)
        in_queue[u] = False

        for edge in get_out_edges(u, edges):
            v = edge.to_node
            weight = -math.log(edge.rate * (1 - edge.fee))

            if distances[u] + weight < distances[v]:
                distances[v] = distances[u] + weight
                predecessor[v] = u

                if not in_queue[v]:
                    queue.append(v)
                    in_queue[v] = True
                    visit_count[v] += 1
                    
                    if visit_count[v] > len(vertices):
                        return extract_cycle(v, predecessor)

    return None
```

## 25. Criterio final de excelencia
El motor resuelve topologías de 1,000 activos y 10,000 pares en 1 milisegundo mediante SPFA en Rust, filtrando matemáticamente el 100% del ruido del mercado sin requerir aproximaciones que pongan en riesgo el capital.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Degradación al peor caso `O(VE)` si el mercado oscila agresivamente (mitigado con timeout duro).
- Dependencias: Teoría de grafos, Ingesta de precios.
- Próxima skill: Optimización estocástica bajo incertidumbre (Skill 6).
