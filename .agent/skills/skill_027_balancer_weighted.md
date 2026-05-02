# SKILL 027 — Balancer weighted pools

## 1. Propósito superior
Incorporar al ecosistema del bot la capacidad de operar sobre el protocolo Balancer, el cual permite pools multidimensionales (hasta 8 tokens) con pesos asimétricos (ej. 80% WETH / 20% USDC). A diferencia de Uniswap o Curve, la matemática aquí se basa en el "Constant Mean Invariant". Esta skill dota al agente del poder de encontrar oportunidades raras en pools donde un activo es intencionalmente escaso y su impacto de precio diverge drásticamente del estándar de la industria.

## 2. Nivel de conocimiento requerido
Experto en Finanzas Cuantitativas DeFi. Dominio avanzado de logaritmos y potencias fraccionarias (`pow` con decimales) en un entorno de BigInt sin pérdida de precisión. Comprensión profunda de la arquitectura "Vault" de Balancer V2 (Separación entre la contabilidad global y la lógica del pool individual) y las matemáticas de pesos ponderados (Weighted Math).

## 3. Capacidades principales
1. Cálculo off-chain del Invariante Ponderado (Weighted Invariant `V`).
2. Aproximación matemática de exponenciación fraccionaria `(ReserveIn / ReserveOut) ^ (WeightOut / WeightIn)` usando algoritmos Taylor/Binomiales o aproximaciones de Newton especializadas.
3. Decodificación de transacciones `BatchSwap` nativas de Balancer V2.
4. Ruteo interno sobre el "Vault" de Balancer, moviendo tokens entre 5 pools distintos gastando Gas una sola vez gracias a que los tokens nunca abandonan el contrato del Vault hasta el salto final.
5. Inclusión de los "Swap Fees" que en Balancer son dinámicos y pueden ser alterados por el controlador de la pool.
6. Simulación matemática de extracción de liquidez (Exit Pool / Join Pool) para buscar arbitraje entre los tokens base y el BPT (Balancer Pool Token).
7. Mapeo estructural de los `Pool ID` (bytes32) frente a las direcciones de los tokens.
8. Adaptación al límite estricto de Balancer: El `amountIn` no puede exceder el 30% de la reserva del token.
9. Evaluación de Phantom BPTs (Tokens de pool pre-acuñados usados en pools anidados).
10. Cálculo exacto del "Spot Price" ponderado.

## 4. Entradas requeridas
- `vault_reserves`: Saldos actuales leídos desde el contrato Vault global.
- `pool_weights`: Los pesos fijos (normalized weights) de la pool, ej. 0.8 y 0.2.
- `swap_fee_percentage`: Comisión del pool.
- `amount_in` / `amount_out`: Cantidades de intercambio.

## 5. Salidas esperadas
- `amount_out_calculated`: Valor exacto post-swap.
- `spot_price`: Precio marginal en ese instante.
- `price_impact`: Deslizamiento porcentual.
- `batch_swap_steps`: Array con la ruta recomendada si abarca múltiples pools de Balancer.

## 6. Reglas inmutables
- Nunca superar el límite teórico máximo transable: Las pools de Balancer V2 revierten on-chain la operación si intentas intercambiar más del 30% de la reserva existente (`MAX_IN_RATIO`).
- Todos los cálculos de pesos (weights) deben estar normalizados a 10^18 (Ej. 20% = 0.2 * 10^18). La suma total de los pesos del pool siempre DEBE ser exactamente 1 (10^18).
- La arquitectura obliga a enviar la transacción al contrato `Vault`, no al contrato de la Pool directamente. Usar las direcciones equivocadas en el ruteo causa el fallo de la operación y el desperdicio de gas.

## 7. Algoritmos o métodos que debe conocer
- Constant Mean Market Maker (CMMM) `Π(R_t^W_t) = K`.
- Aproximación de exponenciales naturales para enteros grandes (`FixedPoint.sol` original de Balancer).
- Algoritmo de "Internal Balance" de Balancer (Dejar saldos pendientes en el Vault para operar más barato después).

## 8. Fórmulas críticas
- **Spot Price Ponderado**: `SpotPrice_In_Out = (Reserve_In / Weight_In) / (Reserve_Out / Weight_Out) * (1 / (1 - SwapFee))`
- **Amount Out (OutGivenIn)**:
  `AmountOut = ReserveOut * (1 - (ReserveIn / (ReserveIn + AmountIn_Ajustado)) ^ (WeightIn / WeightOut))`
- **Amount In (InGivenOut)**:
  `AmountIn = ReserveIn * ((ReserveOut / (ReserveOut - AmountOut)) ^ (WeightOut / WeightIn) - 1)` ajustado por fee.

## 9. Casos extremos
- Pesos Extremos (Ej. Pool 98% / 2%): Estos pools son excelentes para listar nuevos tokens con poco capital, pero el deslizamiento de precio (slippage) del token menor (2%) es monumental. Un bot despistado operaría asumiendo comportamiento Uniswap y sufriría un "Slippage rekt".
- Fallo de precisión en potencias: Calcular `x^(0.2/0.8)` se traduce a `x^0.25`. Si el solver de BigInt calcula `x^0.2499999`, la diferencia de un decimal a gran escala arroja miles de dólares de divergencia, causando que el bot mande una transacción con un `minAmountOut` que la EVM jamás satisfará (Revert).
- Pausa del Protocolo (Recovery Mode): En emergencias, Balancer desactiva los swaps regulares y solo permite `ExitPool`. El bot debe leer este booleano.

## 10. Validaciones obligatorias
- PRE: Validar que `Amount_In <= Reserve_In * 0.3`. Si es mayor, truncar forzosamente la operación a ese límite.
- CÁLCULO: Validar la conversión matemática de pesos (`WeightOut / WeightIn`). En pools 50/50, el exponente es 1, y la fórmula colapsa graciosamente al producto constante normal de Uniswap V2 (x*y=k).
- POST: Incorporar comprobación del "Internal Balance". Si el Agente tiene saldos internos flotantes dentro del Vault, la transacción cuesta casi 0 Gas.

## 11. Criterios de aprobación
- La fórmula exponencial aproximada retorna un valor con desviación < 1 Wei frente al Smart Contract original.
- El pool está activo y no se encuentra en pausa por la gobernanza de Balancer.

## 12. Criterios de rechazo
- El cálculo exige extraer liquidez en un pool de pesos extremos que devora el margen matemático en puro impacto de precio.
- La ejecución propuesta excede el `MAX_IN_RATIO` de la arquitectura del Vault.

## 13. Riesgos que mitiga
- Riesgo de Pérdida de Gas por Reversiones Estructurales: Conocer de antemano la regla del 30% evita mandar peticiones masivas "whale-size" que siempre van a rebotar on-chain.
- Ampliación de Superficie de Ataque: Integra billones de dólares de liquidez profunda institucional alojada en Balancer (especialmente LSDs como wstETH, rETH) que los bots de arbitraje básicos de Uniswap no pueden tocar porque no saben calcular potencias no enteras.

## 14. Integración con otras skills
- Complementa la Matemática AMM básica (Skill 24).
- Alimenta el Ruteo Avanzado de Grafos (Skill 4).

## 15. Modelo de datos sugerido
```json
{
  "BalancerWeightedExecution": {
    "pool_id": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
    "token_in": "WETH",
    "token_out": "BAL",
    "weight_in_normalized": 0.8,
    "weight_out_normalized": 0.2,
    "amount_in": "5000000000000000000",
    "projected_amount_out": "450000000000000000000",
    "calculated_slippage_pct": 2.5
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Un módulo matemático especializado en C MMM (`Constant Mean Market Maker`) preferiblemente implementado en C++ / Rust (vía FFI) o WebAssembly dado el altísimo costo computacional de aproximar series de Taylor para exponenciales de grandes números.

## 17. Logs obligatorios
- `[DEBUG] Balancer Math: Pool 80/20 WETH/BAL. Calculating outGivenIn for 5 WETH.`
- `[WARN] Balancer Math: amountIn exceeds MAX_IN_RATIO (30% of reserve). Truncating trade size.`
- `[CRITICAL] Approximation failed to converge during power calculation. Halting Balancer integration safely.`

## 18. Métricas obligatorias
- `balancer_math_latency_us`.
- `balancer_math_precision_divergence_wei`.
- `max_in_ratio_hits_count` (Para ajustar el tamaño inicial en el Skill 2 de Optimización).

## 19. Tests unitarios
- Power Function: Testear `pow(base, exp)` con `exp = 0.5` (raíz cuadrada), `exp = 1.0` (identidad) y `exp = 4.0` (pools 80/20). Comparar con los test vectors oficiales de `LogExpMath.sol`.
- MAX_IN_RATIO limit: Intentar meter 31 tokens en un pool de 100. El simulador local DEBE rechazar la petición o truncarla.
- Replicación de UniswapV2: Alimentar al motor de Balancer con un pool donde `weight_in = 0.5` y `weight_out = 0.5`, y comparar la salida contra el motor de Uniswap V2 (Deben ser idénticos tras ajuste de fees).

## 20. Tests de integración
- Descargar datos del `Vault` desde Mainnet vía Multicall, calcular salidas para 5 pools distintas asíncronamente y cotejar con el Smart Contract del `BalancerQueries` de forma estática en un test off-chain.

## 21. Tests E2E
- El bot halla gap de precios entre Uniswap V3 y Balancer (B-80BAL-20WETH), calcula la rentabilidad cruzada, dispara `BatchSwap` utilizando los balances internos (Internal Balances) y materializa el beneficio usando Flash Loans atómicos.

## 22. Checklist de producción
- [ ] Incorporación del contrato oficial `BalancerHelpers` o `BalancerQueries` si se permite la simulación `eth_call` sin restricciones de latencia (útil para fallback en caso de fallo del motor matemático interno).
- [ ] Codificación de bytes32 obligatoria para referenciar `Pool_IDs`. (No usar direcciones normales de 20 bytes).
- [ ] Tratamiento de fees del protocolo (Protocol Fees) que son un porcentaje que el contrato retiene aparte del Swap Fee de los LPs.

## 23. Ejemplo de configuración no hardcodeada
```yaml
balancer_engine:
  max_in_ratio_limit: 0.30  # Balancer V2 core rule
  max_out_ratio_limit: 0.30
  taylor_series_iterations: 15
  use_internal_balance_routing: true
```

## 24. Ejemplo de pseudocódigo
```javascript
function calcOutGivenIn(balanceIn, weightIn, balanceOut, weightOut, amountIn, fee) {
    if (amountIn > balanceIn * MAX_IN_RATIO) {
        throw new Error("MAX_IN_RATIO");
    }
    
    // Fee applies only to the input amount
    const amountInWithoutFee = amountIn * (1n - fee);
    const denominator = balanceIn + amountInWithoutFee;
    
    // Base = balanceIn / (balanceIn + amountInWithoutFee)
    const base = fixedPointDiv(balanceIn, denominator);
    
    // Exponent = weightIn / weightOut
    const exponent = fixedPointDiv(weightIn, weightOut);
    
    // Power calculation using Taylor series approximation or custom BigInt power
    const power = fixedPointPow(base, exponent);
    
    // AmountOut = balanceOut * (1 - power)
    return fixedPointMul(balanceOut, FIXED_1 - power);
}
```

## 25. Criterio final de excelencia
Al incluir Balancer con simulaciones matemáticas perfectas off-chain, el sistema no solo destraba billones en liquidez que otros bots evitan, sino que ejecuta Arbitrajes Ponderados en nanosegundos eludiendo los costosos revertos on-chain por fallas de límites proporcionales.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Precisión matemática en aproximaciones exponenciales. (Balancer Vault asume el error al alza a favor del LP, por lo que el bot perderá wei por errores minúsculos si no empata la biblioteca matemática perfecta).
- Dependencias: Soporte U256 estricto, Lectura On-Chain.
- Próxima skill: Flash loans mastery (Skill 28).
