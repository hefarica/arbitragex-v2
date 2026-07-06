# SKILL 024 — AMM mathematics

## 1. Propósito superior
Encapsular y dominar la matemática pura de los Automated Market Makers (AMM) de generación V2 (Fórmula de producto constante `x * y = k`). Estandarizar la simulación hiper-rápida de swaps, impactos de precio y enrutamiento óptimo en miles de forks de Uniswap V2 (SushiSwap, PancakeSwap, TraderJoe, SpookySwap) mediante el cálculo directo de reservas sin llamar nunca al contrato inteligente real para cotizar.

## 2. Nivel de conocimiento requerido
Experto en Finanzas Descentralizadas (DeFi), Microeconomía Cuantitativa y Smart Contract Mechanics. Nivel Máster en derivación algebraica de la fórmula de producto constante, cálculo de "Amount Out" con descuento de Fee (997/1000), estimación de "Price Impact", y operaciones de coma flotante frente a Enteros Grandes (u256/BigInt) para exactitud al wey (10^-18).

## 3. Capacidades principales
1. Cálculo instantáneo in-memory de `getAmountOut` y `getAmountIn` (Cuántos tokens A se requieren para obtener X tokens B).
2. Manejo matemático del Fee dinámico por protocolo (ej. Uniswap V2 0.30%, PancakeSwap 0.25%, TraderJoe 0.30%).
3. Detección y corrección de "Divisiones Inseguras" o precisiones flotantes que arruinarían el tamaño del trade on-chain.
4. Mapeo de Reservas a Precio Spot marginal (`Reserve_B / Reserve_A`).
5. Cálculo del "Mid Price" y su deslizamiento post-swap.
6. Re-sincronización instantánea de constantes matemáticas cuando se emite el evento `Sync(reserve0, reserve1)` por WebSocket.
7. Discriminación lógica del orden del par (Token0 vs Token1) usando comparación lexicográfica de direcciones hexadecimales de Solidity.
8. Evaluación de rentabilidad transversal asumiendo un salto (hop) en el pool: `TokenIn -> TokenOut -> TokenIn`.
9. Filtro estricto contra Reservas Vacías (Pools con liquidez irrisoria que devuelven resultados matemáticos absurdos).
10. Generación de simulaciones inversas: ¿Cuánto volumen máximo admite la pool antes de desplazar el precio un 1%? (Slippage Boundary Check).

## 4. Entradas requeridas
- `reserve_in`: Cantidad del token de entrada almacenado en el Smart Contract.
- `reserve_out`: Cantidad del token de salida almacenado en el Smart Contract.
- `amount_in` / `amount_out`: La cantidad deseada de intercambio.
- `fee_tier`: El porcentaje de comisión deducido de la entrada por el protocolo (ej. 3 para 0.30%).

## 5. Salidas esperadas
- `amount_out_calculated`: Valor exacto en WEI esperado.
- `price_impact_pct`: El cambio porcentual en el precio spot causado por la inyección.
- `execution_price`: Relación `amount_out / amount_in`.
- `new_reserves_state`: Proyección de cómo quedarán las reservas después del swap (para cálculos multi-leg encadenados sin esperar el bloque).

## 6. Reglas inmutables
- TODA operación matemática debe hacerse usando la librería nativa de `BigInt` (Node.js) o `U256` (Rust) para emular milimétricamente el código de Solidity. Usar `float64` (`Number` en JS) está rotundamente prohibido por errores de redondeo letales (Dust rounding traps).
- Nunca consultar a `router.getAmountsOut` on-chain (RPC limit overhead). Descargar las reservas (Skill 21) y ejecutar el cálculo offline en el procesador local (CPU).
- El fee se descuenta SIEMPRE del `amount_in` ANTES de interactuar con la curva `k`, de la misma forma que lo dictamina el código núcleo de UniswapV2.

## 7. Algoritmos o métodos que debe conocer
- Ecuación de Producto Constante `(x + dx) * (y - dy) = k`.
- Comparación lexicográfica de bytes20 `address(token0) < address(token1)`.
- Fórmulas de impacto en precio (Price Impact approximation vs exact formula).

## 8. Fórmulas críticas
- **Cálculo de AmountOut (UniswapV2 Core)**: 
  `AmountInWithFee = amountIn * (1000 - Fee)`
  `Numerator = AmountInWithFee * ReserveOut`
  `Denominator = (ReserveIn * 1000) + AmountInWithFee`
  `AmountOut = Numerator / Denominator` (División entera estricta de Solidity).
- **Cálculo de AmountIn (Inverso)**:
  `Numerator = ReserveIn * amountOut * 1000`
  `Denominator = (ReserveOut - amountOut) * (1000 - Fee)`
  `AmountIn = (Numerator / Denominator) + 1` (Añadir 1 wei para redondeo seguro de entrada).
- **Price Impact**: `1 - ( (ReserveOut - AmountOut) / ReserveOut ) / ( (ReserveIn + AmountIn) / ReserveIn )`

## 9. Casos extremos
- Un pool ilíquido (`reserve0 = 10`, `reserve1 = 1000`). El bot intenta operar con `amount_in = 500`. La fórmula matemática permite el swap pero agota casi todo el pool, resultando en un impacto de precio del 98% que destruye el capital.
- El token en el par sufre de un "Tax/Burn" nativo (Ej. SafeMoon transfiere un 10% menos de lo indicado). La matemática asume 100% de ingreso y el contrato revertirá por "K invariant failure".
- Divisiones por cero al procesar pools de tokens pausados (reservas vacías).

## 10. Validaciones obligatorias
- PRE: Validar rigurosamente que `reserveIn > 0` y `reserveOut > 0`.
- CÁLCULO: Validar `amountOut < reserveOut` (No se puede sacar más de lo que existe).
- POST: Si `price_impact_pct > Max_Slippage_Config` (ej. > 5%), marcar oportunidad como Inválida para protegerse del "Sandwich Risk" masivo de bots MEV.

## 11. Criterios de aprobación
- La matemática devuelve un `AmountOut` válido, sin divisiones por cero o underflows de enteros.
- La transacción simulada asegura la conservación local de la invariante `K`.

## 12. Criterios de rechazo
- La función lanza un fallo lógico `INSUFFICIENT_LIQUIDITY` (Intento de extraer más volumen del disponible).
- El token está detectado en la Blacklist de Tokens con Feeds asimétricos (Tax-on-transfer no contemplado).

## 13. Riesgos que mitiga
- Sobrecarga de RPC: Evita tener que preguntar a la Blockchain "¿Cuánto me das por 10 ETH?" 50,000 veces por segundo, usando en cambio 0 llamadas RPC (CPU Local Cache) para simulaciones en bucle for.
- Revert de Transacciones: Previene gastar gas en operaciones que inevitablemente iban a fallar por "Insufficient Output Amount" gracias al pronóstico preciso de la función.

## 14. Integración con otras skills
- Proporciona la lógica de conversión principal para el Simulador Pre-trade (Skill 29) y la Optimización de Tamaño (Skill 2).
- Nutre la matriz base del Álgebra Multileg (Skill 3).

## 15. Modelo de datos sugerido
```json
{
  "AmmMathResult": {
    "pool": "0xabc...",
    "amount_in_wei": "1000000000000000000",
    "amount_out_wei": "3154200000",
    "fee_applied_bps": 30,
    "price_impact_pct": 0.15,
    "projected_reserve_in": "1000010000000000000000",
    "projected_reserve_out": "3154188458000000",
    "is_valid": true
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Funciones puras (Pure Functions) in-memory construidas sobre BigInt en el lenguaje nativo, o mediante un sub-módulo compilado a WebAssembly para alta densidad matemática.

## 17. Logs obligatorios
- `[DEBUG] AMM V2 Math: 1 ETH -> USDC in UniswapV2. Output: $3100.25. Impact: 0.05%.`
- `[WARN] AMM V2 Math Error: INSUFFICIENT_LIQUIDITY in obscure pool (amount_in exceeds K depth). Trade rejected.`

## 18. Métricas obligatorias
- `amm_math_calculation_time_ns` (Nanosegundos. Esto se ejecuta billones de veces al día; debe ser imperceptible).
- `price_impact_average_executed`.
- `insufficient_liquidity_rejections`.

## 19. Tests unitarios
- Equivalencia de Uniswap: Compilar el contrato `UniswapV2Library.sol` y comparar el output `getAmountOut` del código C++/JS contra el output en EVM (Tienen que ser idénticos hasta el último dígito hexadecimal en >10,000 casos aleatorios).
- Overflow: Inyectar un número absurdamente grande `(2^256 - 1)`. Debe arrojar un Overflow Catching error en vez de dar resultados circulares.
- Zero Amount: Probar qué pasa si `amount_in = 0`. Debe devolver 0.

## 20. Tests de integración
- Cargar estado real de 50 pools desde la Blockchain a RAM y procesar todas las interacciones cruzadas en un ciclo cerrado, reajustando la reserva virtual tras cada "swap simulado".

## 21. Tests E2E
- Escanear una ruta de 3 patas (Triangular) en 3 forks de Uniswap V2 distintos (Sushi, Uni, ShibaSwap) asumiendo los diferentes fee tiers de cada uno. Calcular el resultado acumulado sin tocar la red, ejecutar on-chain y ver que la diferencia real vs esperada es exactamente de 0 weis (Salvo intervención externa).

## 22. Checklist de producción
- [ ] Uso exclusivo de `BigInt` sin ninguna coerción a Number en el proceso.
- [ ] Identificador de Fees inyectable por pool (Algunos clones de UniswapV2 ponen fees de 0.2% o 0.1%).
- [ ] Función combinada de `getAmountOut` con actualización de estado (`mutateReserves`) para algoritmos recursivos.

## 23. Ejemplo de configuración no hardcodeada
```yaml
amm_v2_engine:
  precision_library: "native_bigint"
  default_fee_multiplier: 997  # Unsiwap v2 default 0.3%
  max_acceptable_price_impact_pct: 3.0
  throw_on_insufficient_liquidity: true
```

## 24. Ejemplo de pseudocódigo
```javascript
function getAmountOut(amountIn, reserveIn, reserveOut, feeMultiplier = 997n) {
    if (amountIn <= 0n) throw new Error('INSUFFICIENT_INPUT_AMOUNT');
    if (reserveIn <= 0n || reserveOut <= 0n) throw new Error('INSUFFICIENT_LIQUIDITY');
    
    // Exact Solidity replication for Uniswap V2
    const amountInWithFee = amountIn * feeMultiplier;
    const numerator = amountInWithFee * reserveOut;
    const denominator = (reserveIn * 1000n) + amountInWithFee;
    
    return numerator / denominator; // Floor integer division
}

function calculatePriceImpact(amountIn, amountOut, reserveIn, reserveOut) {
    const spotPrice = Number(reserveOut) / Number(reserveIn);
    const executionPrice = Number(amountOut) / Number(amountIn);
    return ((spotPrice - executionPrice) / spotPrice) * 100.0;
}
```

## 25. Criterio final de excelencia
La skill simula cualquier cruce en el ecosistema AMM clásico a velocidades de memoria caché de CPU, prediciendo centavo a centavo el recibo de retorno sin generar un solo fallo de "K Invariant Revert" en el código del Smart Contract real.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Tokens con Fee-on-Transfer (Taxes) no advertidos o rebasing tokens. (Requiere un Honeypot Checker adjunto).
- Dependencias: Soporte de enteros grandes, Lectura On-Chain.
- Próxima skill: Uniswap v3 concentrated liquidity (Skill 25).
