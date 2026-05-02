# SKILL 025 — Uniswap v3 concentrated liquidity

## 1. Propósito superior
Dominar la bestia matemática que representa Uniswap V3 (y clones como PancakeSwap V3, Sushi V3). A diferencia de los modelos V2, en V3 la liquidez está concentrada en rangos de precios limitados (Ticks). Esta skill permite simular, proyectar y ejecutar swaps y cálculos de slippage complejos saltando a través de "Tick Boundaries" activando y desactivando liquidez dinámica en tiempo real sin llamar al contrato.

## 2. Nivel de conocimiento requerido
Nivel PhD en Matemática de Protocolos y Arquitectura de Liquidación V3. Dominio del espacio Q64.96 (Números de punto fijo de 64 bits y fracción de 96 bits), matemática de Raíz Cuadrada (SqrtPriceX96), Tick Bitmaps cruzados por bitwise operators, y ecuaciones de Swap State (Ej. `computeSwapStep` de Uniswap).

## 3. Capacidades principales
1. Conversión fluida entre `TickIndex` (int24), Precio Flotante y `SqrtPriceX96`.
2. Extracción optimizada del estado global de la Pool: `slot0` (precio/tick actual), liquidez activa (L) y Bitmaps de Ticks inicializados.
3. Simulación iterativa de Swaps (`quoter` off-chain): Cruzar de un Tick al siguiente si el `amountIn` consume toda la liquidez del rango actual.
4. Activación/Desactivación de liquidez neta cruzando el límite de un Tick usando el mapa `ticks(tick_id).liquidityNet`.
5. Manejo implacable de la Aritmética Avanzada de Solidity on JS/Rust (`FullMath.mulDiv`, `TickMath.getSqrtRatioAtTick`).
6. Cálculo de los Fee Tiers múltiples (0.01%, 0.05%, 0.3%, 1%).
7. Optimización de lectura de Bitmaps (`tickBitmap`) agrupando y consultando palabras de 256 bits para predecir dónde está la próxima "pared" de liquidez sin escanear el contrato 1 a 1.
8. Estimación precisa de los costos de gas V3 (Saltar Ticks inicializados cuesta un gas extremadamente alto on-chain, lo cual destruye ganancias pequeñas).
9. Mapeo profundo del "Tick Spacing" dependiente del Fee Tier (Por ejemplo, el pool del 1% agrupa cada 200 ticks).
10. Modelado del efecto "JIT Liquidity" (Just In Time Liquidity), donde atacantes agregan/remueven liquidez en el mismo bloque para aplastar el arbitraje.

## 4. Entradas requeridas
- `slot0_data`: Estado actual del pool (`sqrtPriceX96`, `tick`, etc).
- `liquidity`: La liquidez global `L` en el tick actual.
- `tick_bitmap` / `tick_data_array`: Array o Map de la liquidez latente por delante del precio actual.
- `amount_in`: Monto a transar.
- `zero_for_one`: Dirección del swap (Token0 -> Token1 o viceversa).

## 5. Salidas esperadas
- `amount_out_calculated`: Valor preciso tras saltos de Tick y Fee deductions.
- `final_sqrt_price_x96`: Proyección matemática del precio exacto al concluir la orden.
- `ticks_crossed`: Entero indicando cuántas paredes de precio rompió la orden.
- `projected_gas_overhead`: Incremento de gasto de gas proporcional a los saltos computados.

## 6. Reglas inmutables
- Nunca emplear Float/Double matemático para aproximar la matemática Q64.96. Se requiere portar la librería `FullMath.sol` de Uniswap V3 al lenguaje local para garantizar truncamiento idéntico (Redondeo hacia arriba/abajo en divisiones).
- Para swaps masivos, es OBLIGATORIO calcular las adiciones/sustracciones de liquidez al cruzar Ticks inicializados. Evadir esto resultará en estimaciones asintóticamente desastrosas.
- Jamás usar el Smart Contract QuoterV2 en producción on-chain si no se puede pagar su inmenso gas. La simulación debe ocurrir off-chain.

## 7. Algoritmos o métodos que debe conocer
- Ecuaciones de Liquidez Concentrada: `L = Δy / Δ(√P)` o `L = Δx / Δ(1/√P)`.
- Algoritmos de iteración de Swap V3 (`while (state.amountSpecifiedRemaining > 0 && state.sqrtPriceX96 != sqrtPriceLimitX96)`).
- Manipulación de árboles de bits (Bitwise Shifts) para navegación del `tickBitmap`.

## 8. Fórmulas críticas
- **Tick to SqrtPriceX96**: `√P = 1.0001^(tick / 2) * 2^96`
- **Cálculo de Salida en Tick estático (Exact Input)**: `Δy = L * (√P_current - √P_target)`
- **Condición de cruce de Tick**: Si `AmountIn_Calculado_Para_Llegar_Al_Borde < AmountIn_Total`, se consume esa cuota, el estado cruza el Tick, añade `liquidityNet`, y continúa el bucle con el remanente.

## 9. Casos extremos
- Agujeros de Liquidez (Liquidity Gaps): El bot busca vender ETH. Consumió la liquidez del Tick actual, busca en el Bitmap el siguiente Tick con liquidez y está un 25% más abajo (Crash local de liquidez). Si la simulación no anticipa este vacío, el trade se liquidará a precios abismales (Slippage Ruin).
- Desbordes aritméticos (Overflows) manejados por Uniswap en contratos (Require conditions) que en el simulador local provocan pánicos del intérprete y caídas del bot.
- Swaps de "Dust" (Cantidades tan ínfimas de wei que los redondeos devuelven AmountOut = 0), gastando el fee de gas por nada.

## 10. Validaciones obligatorias
- PRE: Validar que los datos del array de Ticks están frescos y provienen del mismo bloque que `slot0`.
- CÁLCULO: Mantener track exacto del "Fee Growth Inside" si se va a simular extracción de fees (Menos común en ruteo, crítico en LP management).
- POST: Si `ticks_crossed > 3`, advertir sobre posible fallo de gas on-chain debido al alto costo (aprox 10k gas por Tick cruzado).

## 11. Criterios de aprobación
- El bucle de `computeSwapStep` termina antes de alcanzar los límites de pánico (Slippage tolerance).
- Los resultados proyectados igualan la rentabilidad neta exigida.

## 12. Criterios de rechazo
- La iteración descubre una escasez severa de liquidez concentrada frente al tamaño de la orden (Price Impact masivo).
- La información del `tickBitmap` descargada es insuficiente para resolver el ciclo entero (El trade empuja el precio más allá del rango descargado del nodo RPC).

## 13. Riesgos que mitiga
- Riesgo de Cotización Ilusoria (Quoter Illusion): Muchos bots básicos asumen la profundidad actual del Tick como "Infinita" en V3 porque no entienden su matemática. Al intentar ejecutar, sus transacciones revierten por "Price Limit Reached". Esta skill modela la realidad matemática como una piedra.
- Optimización de L1 Gas: V3 en Ethereum Mainnet puede costar $30 o $150 de gas dependiendo de cuántos Ticks se crucen. Predecir esto off-chain evita pérdidas ocultas de ejecución.

## 14. Integración con otras skills
- Alimentado masivamente por Lectura On-Chain Multicall (Skill 21).
- Integrado profundamente en el Cálculo Diferencial de Slippage (Skill 7).

## 15. Modelo de datos sugerido
```json
{
  "UniswapV3Simulation": {
    "pool_address": "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    "zero_for_one": true,
    "input_wei": "5000000000000000000",
    "output_wei": "16543000000",
    "projected_price_x96": "202356781293004561283",
    "ticks_crossed": 1,
    "gas_overhead_estimate": 45000,
    "price_impact_pct": 0.08
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Wrapper nativo de la clase `TickMath` y `SwapMath` portados a Typescript o Rust usando emuladores de aritmética U256 para latencia menor a 1ms.

## 17. Logs obligatorios
- `[DEBUG] V3 Simulator: Starting swap loop. AmountIn: 5 ETH. Current Tick: 204560.`
- `[INFO] V3 Simulation complete. Output: 16,543 USDC. Ticks crossed: 1. Impact: 0.08%.`
- `[WARN] V3 Liquidity Hole Detected. Next initialized tick is 2500 basis points away. Aborting simulation.`

## 18. Métricas obligatorias
- `v3_simulation_latency_us` (Crucial porque un bucle while mal optimizado demora).
- `average_ticks_crossed_per_trade`.
- `v3_math_accuracy_divergence_wei` (La diferencia entre el output predicho y el ejecutado on-chain. Debe ser 0 en un bloque aislado).

## 19. Tests unitarios
- Aritmética de Sqrt: Probar la función `getSqrtRatioAtTick(tick)` frente a los tests oficiales del repositorio original de Uniswap para certificar exactitud matemática.
- Mismo bloque, diferente trade: Someter la función de iteración `computeSwapStep` a 5 trades de tamaños crecientes en la misma pool sintética, validando que el gas simulado crece proporcionalmente.
- Boundary condition: Ordenar un swap que termina exactamente con el remanente = 0 en la frontera exacta del Tick. No debe arrojar errores de by-one ni cruzar al siguiente Tick innecesariamente.

## 20. Tests de integración
- Sincronizar un array estático de Bitmaps desde Ethereum Mainnet usando Alchemy, armar el objeto in-memory y lanzar una simulación de venta de 1000 ETH contra el pool de USDC, verificando el colapso predecible de precio.

## 21. Tests E2E
- El agente descubre arbitraje, carga los slots 0, bitmaps y ticks requeridos vía Multicall (Skill 23), simula el V3 Swap en milisegundos sin tocar `QuoterV2` del exchange, y encadena la salida al motor CEX para completar el arbitraje de Basis.

## 22. Checklist de producción
- [ ] Incorporación de SDKs de terceros para V3 (`@uniswap/v3-sdk`) únicamente como fallback, implementando el core crítico manualmente para velocidad.
- [ ] Descarga pre-cacheada (Warm-up caching) de Ticks adyacentes (+/- 10 Ticks) para los pares hiper-activos en el ciclo de inicio.
- [ ] Manejo de reversión atómica de estado en el simulador (Si falla un leg posterior, el estado del simulador debe ser reiniciado a la fotografía del bloque original).

## 23. Ejemplo de configuración no hardcodeada
```yaml
uniswap_v3_engine:
  tick_scan_limit_distance: 100    # How many ticks around current price to pre-load
  enable_jit_liquidity_defense: true
  max_loop_iterations_per_swap: 50
  precision_emulator: "rust_u256_bindings"
```

## 24. Ejemplo de pseudocódigo
```javascript
function simulateV3Swap(amountIn, stateCache, zeroForOne) {
    let state = {
        amountSpecifiedRemaining: BigInt(amountIn),
        amountCalculated: 0n,
        sqrtPriceX96: BigInt(stateCache.slot0.sqrtPriceX96),
        tick: stateCache.slot0.tick,
        liquidity: BigInt(stateCache.liquidity)
    };

    while (state.amountSpecifiedRemaining > 0n && state.sqrtPriceX96 !== SQRT_PRICE_LIMIT) {
        // Find next tick boundary
        let { nextTick, initialized } = TickBitmap.nextInitializedTickWithinOneWord(
            stateCache.tickBitmap, state.tick, stateCache.tickSpacing, zeroForOne
        );
        
        let sqrtPriceNextX96 = TickMath.getSqrtRatioAtTick(nextTick);

        // Compute step (how much is consumed vs generated in this tick range)
        let step = SwapMath.computeSwapStep(
            state.sqrtPriceX96, sqrtPriceNextX96, state.liquidity, state.amountSpecifiedRemaining, FEE_PIPS
        );

        state.amountSpecifiedRemaining -= step.amountIn;
        state.amountCalculated += step.amountOut;
        state.sqrtPriceX96 = step.sqrtPriceNextX96;

        // If we reached the next tick boundary, cross it
        if (state.sqrtPriceX96 === sqrtPriceNextX96) {
            if (initialized) {
                let liquidityNet = stateCache.ticks[nextTick].liquidityNet;
                if (zeroForOne) liquidityNet = -liquidityNet; // Reverse net if going down
                state.liquidity += liquidityNet;
            }
            state.tick = zeroForOne ? nextTick - 1 : nextTick;
        } else {
            state.tick = TickMath.getTickAtSqrtRatio(state.sqrtPriceX96);
        }
    }
    
    return state.amountCalculated;
}
```

## 25. Criterio final de excelencia
El motor Uniswap V3 debe ser capaz de modelar un pool altamente concentrado y fragmentado con 1000 posiciones superpuestas, navegando el book de órdenes virtuales con precisión al centavo, permitiendo extraer ganancias algorítmicas sin derrochar un wei en quoter requests of-chain.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Flash Liquidity Injection (Alguien añade liquidez masiva en el mismo bloque mediante Flashbots, destrozando el estado modelado off-chain).
- Dependencias: Soporte U256 estricto.
- Próxima skill: Curve stable swap math (Skill 26).
