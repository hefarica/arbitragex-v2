# SKILL 003 — Álgebra lineal para rutas multi-leg

## 1. Propósito superior
Detectar y evaluar oportunidades de arbitraje complejas que involucren 3 o más activos/exchanges (ej. A -> B -> C -> A) mediante matrices de transformación, asegurando escalabilidad computacional en tiempo real en vez de usar bucles ineficientes. Convierte el mapeo del mercado en un problema matricial de álgebra lineal que puede ser resuelto casi instantáneamente usando operaciones vectorizadas.

## 2. Nivel de conocimiento requerido
Máster/PhD en Matemática Aplicada o Ciencias de la Computación. Especialización en operaciones matriciales de alta velocidad (BLAS/LAPACK), teoría espectral, transformación de grafos a matrices de adyacencia y cálculo tensorial aplicado a mercados financieros.

## 3. Capacidades principales
1. Representar la red global de pares de trading como una Matriz de Adyacencia Ponderada.
2. Calcular tasas de cambio cruzadas a través de multiplicación matricial.
3. Detectar ciclos de rentabilidad usando la traza de potencias de la matriz.
4. Escalar de arbitraje triangular (3-leg) a n-leg (poligonal) sin complejidad factorial.
5. Inclusión de matrices de costos (fees) como factores de descuento en las transformaciones lineales.
6. Soporte para operaciones vectorizadas (SIMD) para evaluar miles de rutas en paralelo.
7. Cálculo del determinante o valores propios para medir la estabilidad del ecosistema de precios.
8. Modelado de slippage a nivel matricial (aproximación de primer grado).
9. Extracción eficiente de sub-matrices para aislar clusters de liquidez (ej. DeFi en Solana, CEXes en Binance/Kraken).
10. Mapeo de activos sintéticos o bridged assets (ej. WETH vs ETH) como equivalencias identidad con peso de fee.

## 4. Entradas requeridas
- `tickers_snapshot`: Array unificado de todos los precios bid/ask del sistema.
- `fee_matrix`: Matriz NxN con los costos de transacción entre el activo i y el activo j.
- `liquidity_matrix`: Matriz NxN con la profundidad disponible en el nivel 1.

## 5. Salidas esperadas
- `profitable_paths`: Lista de vectores de rutas que muestran una transformación > 1.0 (antes de fees) o > `min_roi` (después de fees).
- `arbitrage_matrix`: Matriz resultante donde los elementos (i, j) indican el factor de conversión óptimo de i a j.
- `cycle_intensities`: Escalares que indican la fuerza de la oportunidad en sub-redes cerradas.

## 6. Reglas inmutables
- No usar bucles anidados tipo `for i in X: for j in Y: for k in Z` en producción para la búsqueda base; el core debe usar operaciones vectoriales de álgebra lineal.
- Los pesos de la matriz deben ser actualizados asincrónicamente, manteniendo los tensores inmutables durante cada ciclo de cómputo atómico.
- Los activos de distintas redes (USDC-ERC20 vs USDC-SPL) deben ser tratados como nodos distintos en la matriz, a menos que exista un puente explícito en la `fee_matrix`.

## 7. Algoritmos o métodos que debe conocer
- Multiplicación de Matrices de Adyacencia Ponderadas.
- Floyd-Warshall algebraico (Algebraic path problems en semianillos, como Min-Plus o Max-Times algebra).
- Operaciones BLAS de nivel 3 (GEMM - General Matrix Multiply).
- Sparse Matrix Operations (ya que la mayoría de los pares no tienen un mercado directo).

## 8. Fórmulas críticas
- **Conversión de Peso de Arista**: `W_ij = -log(Rate_ij * (1 - Fee_ij))` (Convierte multiplicaciones en sumas para usar álgebra lineal tradicional o detección de ciclos negativos).
- **Potencia de Matrices para n-legs**: `A^n` (Donde el elemento (i,i) de la diagonal de `A^n` representa el factor acumulado de un ciclo de n pasos desde i hasta i).
- **Condición de Arbitraje**: Si para algún `n`, `Trace(A^n) > N`, existe un ciclo rentable (bajo formulación multiplicativa).

## 9. Casos extremos
- Matrices dispersas extremadamente grandes (> 5000x5000) que causan memory thrashing.
- Disconexión de la red (particiones del grafo) donde la matriz tiene bloques sin cruce.
- Errores de redondeo de punto flotante en la multiplicación acumulativa que generan falsos positivos de `1.0000000001`.
- Activos atrapados con bid infinito y ask cero.

## 10. Validaciones obligatorias
- PRE: Validar simetría direccional (si A->B existe, no asume que B->A existe con la tasa inversa exacta, depende del bid/ask real).
- CÁLCULO: Mantener umbrales numéricos de épsilon (`1e-9`) para prevenir arbitrajes fantasma por error IEEE-754.
- POST: Validar las rutas detectadas matricialmente contra el calculador exacto del Order Book (Skill 1) para confirmar slippage real.

## 11. Criterios de aprobación
- La matriz resultante arroja un valor diagonal `(i, i) > 1.0 + min_roi_config`.
- La ruta matricial extraída tiene liquidez en la matriz de liquidez `L_ij > min_notional`.

## 12. Criterios de rechazo
- El cálculo se degrada a O(N^3) y excede el timeout de 5ms.
- La ruta implica cruzar un nodo que en la matriz de estado está marcado como "Suspendido" o "Mantenimiento".

## 13. Riesgos que mitiga
- Riesgo de "Time-to-Market": Un competidor encuentra el arbitraje triangular antes porque el toolkit usa bucles for ineficientes. El álgebra lineal procesa en paralelo a nivel de CPU/GPU.
- Riesgo de Oportunidades Invisibles: Perder oportunidades de 4 o 5 patas que los bots simples de 2 o 3 patas no pueden detectar a tiempo.

## 14. Integración con otras skills
- Es el preludio a Teoría de Grafos (Skill 4).
- Usa los modelos matemáticos exactos de Skill 1 para re-validar la ruta sugerida.
- La ejecución paralela puede correrse mediante Worker Orchestration (Skill 59).

## 15. Modelo de datos sugerido
```json
{
  "LinearAlgebraState": {
    "dimensions": 1200,
    "sparsity_ratio": 0.85,
    "last_compute_us": 450,
    "detected_cycles": [
      {
        "length": 4,
        "nodes": ["ETH", "USDT", "BTC", "ETH"],
        "matrix_factor": 1.0015
      }
    ]
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Módulo interno (idealmente Rust con ndarray o Python con NumPy/CuPy) expuesto vía gRPC interno de baja latencia al motor principal de NodeJS.

## 17. Logs obligatorios
- `[DEBUG] Matrix update completed. Dimensions N=1200. Compute time: 1.2ms.`
- `[INFO] Algebraic trace detected positive cycle at nodes [A, B, C]. Sending to strict validation.`

## 18. Métricas obligatorias
- `matrix_multiply_latency_us`
- `n_leg_opportunities_found`
- `false_positive_algebraic_rate` (Rutas que la matriz cree rentables pero el calculador estricto rechaza por slippage).

## 19. Tests unitarios
- Test de identidad: Matriz sin oportunidades debe tener toda su diagonal `A^n <= 1.0`.
- Inyección de oportunidad forzada en ruta 4-leg; la traza de la matriz a la 4ta potencia debe reflejarlo.
- Tolerancia a precisión: Manejo de flotantes cercanos a 1.0 sin causar triggers falsos.

## 20. Tests de integración
- Sincronización entre el actualizador de la Matriz y el feed de WebSockets (Skill 36) para asegurar que la matriz es siempre el estado actual.

## 21. Tests E2E
- El agente lee datos de mercado masivos, genera matrices gigantes, detecta oportunidad 4-leg en menos de 5ms y despacha a ejecución simulada con éxito.

## 22. Checklist de producción
- [ ] Implementación en lenguaje con soporte real SIMD o uso de librerías altamente optimizadas (BLAS).
- [ ] Separación de bid/ask en tasas direccionales (grafo dirigido).
- [ ] Función de extracción de ruta desde la matriz resultante para pasar la secuencia de operaciones al ejecutor.

## 23. Ejemplo de configuración no hardcodeada
```yaml
linear_algebra_engine:
  max_legs: 5
  epsilon_tolerance: 0.000001
  hardware_acceleration: "AVX512"
  min_liquidity_filter_usd: 100
```

## 24. Ejemplo de pseudocódigo
```python
import numpy as np

def detect_cycles_algebraic(adj_matrix_rates, max_legs=4):
    # adj_matrix_rates[i][j] = rate to convert asset i to asset j (including fees)
    current_power = np.copy(adj_matrix_rates)
    
    for n in range(2, max_legs + 1):
        current_power = current_power.dot(adj_matrix_rates)
        
        # Check diagonal for cycles > 1.0 + margin
        cycles = np.diagonal(current_power)
        viable_indices = np.where(cycles > 1.0005)[0] # 0.05% threshold
        
        if len(viable_indices) > 0:
            return extract_paths(current_power, viable_indices, n)
            
    return []
```

## 25. Criterio final de excelencia
El motor de álgebra lineal puede barrer el mercado entero de un gran CEX (e.g., Binance con miles de pares) resolviendo hasta 5 patas en menos de 2 milisegundos en CPU tradicional, superando drásticamente a competidores iterativos.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Costo de reconstruir la matriz entera ante un solo tick (mitigable con actualizaciones parciales / sparse updates).
- Dependencias: Data Normalization (Skill 32).
- Próxima skill: Teoría de grafos para detección de ciclos rentables (Skill 4).
