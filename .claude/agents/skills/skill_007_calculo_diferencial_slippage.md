# SKILL 007 — Cálculo diferencial aplicado a slippage

## 1. Propósito superior
Proveer una medición analítica irrefutable del impacto en precio (Market Impact / Slippage) de inyectar volumen en un mercado, reemplazando simulaciones groseras o aproximaciones lineales por derivadas matemáticas exactas de la curva de liquidez subyacente. Permite al sistema comportarse de manera quirúrgica, tomando ganancias milimétricas sin alterar bruscamente el ecosistema.

## 2. Nivel de conocimiento requerido
PhD/Máster en Matemática Aplicada y Análisis Numérico. Dominio de Cálculo Vectorial, ecuaciones diferenciales estocásticas, modelos teóricos de provisión de liquidez en AMMs (Uniswap V3, Curve) y discretización iterativa de integrales de sumas de Riemann para Order Books de exchanges centralizados.

## 3. Capacidades principales
1. Diferenciación de curvas de liquidez continua (ej. Uniswap V2 `x*y=k`, Balancer, Curve stableswap).
2. Integración discreta de los niveles `Bid` y `Ask` (L2 Order Book) para CEXes.
3. Cálculo de la derivada primera (Slippage marginal) y la derivada segunda (Aceleración del impacto).
4. Determinación de "Paredes de Liquidez" (Liquidity Walls) donde la derivada segunda tiene una singularidad o salto abrupto.
5. Predicción del precio promedio de ejecución (VWAP - Volume Weighted Average Price) antes de lanzar la orden.
6. Aplicación de la fórmula de liquidez concentrada (Ticks de Uniswap V3) considerando el cruce de "Tick Boundaries".
7. Incorporación de "Slippage de Tiempo" (Erosión pasiva del Order book por otros bots durante el vuelo de red).
8. Modelado de impacto temporal y permanente basado en las ecuaciones de Almgren-Chriss (Market Impact Theory).
9. Optimización de la granularidad de las porciones de orden para algoritmos tipo TWAP/VWAP internos.
10. Ajuste del cálculo por la densidad fractal del spread.

## 4. Entradas requeridas
- `volume_to_execute`: Volumen propuesto de inyección de capital.
- `order_book_state`: Topografía L2 profunda del CEX o arreglo de Ticks activos en el DEX.
- `amm_formula_type`: Identificador matemático del protocolo (constant_product, stableswap, weighted, concentrated).
- `pool_params`: Reservas, factores de amplificación (A en Curve), o posiciones de liquidez (Uniswap V3).

## 5. Salidas esperadas
- `expected_execution_price`: Precio medio exacto o VWAP de toda la posición ejecutada.
- `marginal_price_impact_bps`: Cuántos puntos base se mueve el mercado por cada dólar extra inyectado (Derivada en `volume_to_execute`).
- `total_slippage_usd`: Costo total absorbido por el impacto.
- `max_safe_volume`: El volumen exacto antes de cruzar una barrera crítica de liquidez (salto de derivada).

## 6. Reglas inmutables
- Nunca asumir que el slippage aumenta de manera lineal con el volumen.
- Para CEXes, no usar jamás un "Average Slippage"; integrar nivel por nivel descontando el volumen de forma estricta.
- Para DEXes V3, es obligatorio contemplar la liquidez inactiva si la operación cruza el precio de frontera del tick actual.
- Abortar el cálculo si el volumen propuesto agota el `Order_book_state` completo.
- Retornar fallo matemático (`NaN` o Flag de error) si se detectan datos corruptos en el L2.

## 7. Algoritmos o métodos que debe conocer
- Sumas de Riemann para integración en Order Books escalonados.
- Newton-Raphson para resolver la ecuación no lineal de impacto de Curve (Stableswap invariant).
- Exponenciación y manipulación de Ticks en `sqrtPriceX96` para matemáticas V3 (Solidity standard portado a alta velocidad).
- Modelado de propagación bidireccional (Impacto cruzado de pares correlacionados).

## 8. Fórmulas críticas
- **VWAP (Discreto)**: `VWAP = (Σ P_i * V_i) / Volumen_Total` (Iterando hasta que `Σ V_i == Volumen_Total`).
- **Slippage BPS**: `Slippage_BPS = 10000 * |VWAP - Precio_Actual| / Precio_Actual`
- **Slippage Uniswap V2 Exacto**: `Delta_Y = Y_reserve - (k / (X_reserve + Delta_X_net_fee))`
- **Impacto Marginal**: `d(Precio) / d(Volumen)`

## 9. Casos extremos
- Orden que consume exactamente un nivel L2, activando un redondeo peligroso en el matcher del exchange.
- El volumen agota el Tick V3 actual y hay un "gap" de liquidez vacío hasta el siguiente Tick.
- Parámetro A (Amplification) de Curve alterándose dinámicamente por la DAO en el bloque actual.
- Order book invertido (Ask < Bid) provocado por latencia de red cruzada.

## 10. Validaciones obligatorias
- PRE: Validar que el L2 Book esté lógicamente sano (`Best Bid < Best Ask`).
- CÁLCULO: Validar precisión (No usar floats estándar para DEXes, usar enteros `u256` o librerías BigDecimal para evitar fallos catastróficos frente a contratos inteligentes).
- POST: Validar que `expected_execution_price` sea lógicamente peor que el precio marginal de Nivel 1.

## 11. Criterios de aprobación
- El slippage absoluto calculado permite mantener el ROI neto dentro del margen positivo exigido por la Skill 1.
- No se detectan anomalías matemáticas o divisiones por cero en curvas AMM atípicas.

## 12. Criterios de rechazo
- El Order Book es insuficiente para absorber el volumen mínimo.
- La ejecución provocará cruce de Ticks en DEX con saltos irracionales (Slippage en escalón insoportable).
- El error numérico del solver de Newton-Raphson (para Curve) excede la tolerancia.

## 13. Riesgos que mitiga
- Riesgo de Front-Running: Ordenar un gran volumen genera un impacto masivo que es presa fácil para bots MEV. El cálculo exacto limita el tamaño a zonas inofensivas.
- Ruina de Slippage (Slippage Ruin): El diferencial del CEX es 0.1%, se mandan $10,000 pero el libro solo aguantaba $5,000, los últimos $5k barren el mercado a -5%.

## 14. Integración con otras skills
- Provee la función de costo al motor de Optimización Convexa de Tamaño (Skill 2).
- Sus fórmulas matemáticas puras son empleadas por Matemática de Arbitraje Neto (Skill 1).
- Usa los fundamentos técnicos de los protocolos de AMM Mathematics (Skill 24).

## 15. Modelo de datos sugerido
```json
{
  "SlippageAnalytic": {
    "proposed_volume_usd": 1500.0,
    "amm_type": "uniswap_v3",
    "vwap_price": 3201.55,
    "marginal_impact_bps": 4,
    "total_slippage_usd": 0.60,
    "tick_crossing_detected": false
  }
}
```

## 16. Endpoints o interfaces sugeridas
- FFI (Foreign Function Interface) con Rust para invocar los cálculos de integración y derivadas matemáticas en microsegundos sin Garbage Collection penalizando Node.

## 17. Logs obligatorios
- `[DEBUG] Slippage calculation CEX: Iterated 4 levels to fill $1500. VWAP: $3201.55. Slippage: 4 bps.`
- `[WARN] DEX Slippage extreme: Filling $1000 in Curve Pool XYZ triggers massive A-factor imbalance. BPS: 250.`

## 18. Métricas obligatorias
- `slippage_calculation_latency_us`
- `levels_swept_average` (Cuantos niveles del L2 barren típicamente nuestras órdenes).
- `vwap_prediction_error` (Crucial: Delta entre el precio proyectado y el Fill Price reportado por el Exchange post-trade).

## 19. Tests unitarios
- Test CEX: Libro sintético con 3 niveles, inyectar orden que barra 2.5 niveles; comprobar que la matemática coincide al centavo.
- Test DEX V2: Calcular impacto de una inyección del 1% del pool y verificar usando producto constante estricto.
- Precisión BigDecimal: Testear diferencias sutiles entre float64 de JS y u256 en Solidity para un token con 18 decimales.

## 20. Tests de integración
- Conexión con reconstrucción de orden book asíncrona; disparar cálculo mientras el book se actualiza. Validar inmutabilidad temporal.

## 21. Tests E2E
- Ciclo de prueba en red testnet: Leer liquidez DEX, calcular VWAP con la skill, enviar operación, recibir recibo y medir diferencia < 0.01%.

## 22. Checklist de producción
- [ ] Incorporación de SDKs estandarizados en C/Rust de la matemática de Uniswap V3 (`TickMath`, `SqrtPriceMath`) re-implementados sin usar el EVM.
- [ ] Función de caída libre implementada: si el L2 está vacío, lanzar advertencia en vez de crashear el proceso.
- [ ] Almacenamiento estadístico continuo del `vwap_prediction_error`.

## 23. Ejemplo de configuración no hardcodeada
```yaml
differential_slippage:
  curve_newton_tolerance: 1e-18
  max_l2_levels_to_iterate: 50
  precision_mode: "bigint_u256_simulation"
  fallback_linear_bps_per_10k: 5
```

## 24. Ejemplo de pseudocódigo
```python
def calculate_cex_vwap(order_book_levels, target_volume, is_buy):
    total_cost = 0.0
    remaining_volume = target_volume
    levels = order_book_levels.asks if is_buy else order_book_levels.bids
    
    for level in levels:
        price, level_volume = level.price, level.volume
        
        if remaining_volume <= level_volume:
            total_cost += remaining_volume * price
            remaining_volume = 0
            break
            
        total_cost += level_volume * price
        remaining_volume -= level_volume
        
    if remaining_volume > 0:
        return MathReport(error="Insufficient liquidity in book", vwap=None)
        
    vwap = total_cost / target_volume
    return MathReport(success=True, vwap=vwap)

def d_slippage_d_volume_v2(reserve_x, target_volume_dx):
    # Marginal impact derivative approximation for x*y=k
    return target_volume_dx / (reserve_x ** 2)
```

## 25. Criterio final de excelencia
El motor diferencial clava el VWAP predictivo con un margen de error menor al 1% del propio slippage (es decir, si predice 10 bps de slippage, el real cae entre 9.9 y 10.1 bps) a través de todo tipo de exchanges y protocolos DeFi simultáneamente.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Latencia en el re-cálculo matemático ante cada micro-tick en libros profundos.
- Dependencias: AMM Mathematics, Order Book Reconstruction.
- Próxima skill: Teoría de colas para sistemas de ejecución (Skill 8).
