# SKILL 026 — Curve stable swap math

## 1. Propósito superior
Incorporar el invariante analítico de Curve Finance (El Santo Grial de la liquidez para stablecoins y activos envueltos/wrapped). A diferencia del producto constante, la matemática de Curve es una combinación no lineal extremadamente profunda entre "Suma Constante" y "Producto Constante". Esta skill dota al sistema de la capacidad para rutar operaciones inmensas (millones de dólares) con slippage ínfimo, ejecutando un simulador determinista de Curve (y Forks como Saddle, Ellipsis) en memoria.

## 2. Nivel de conocimiento requerido
PhD en Matemática Analítica y Algoritmos Numéricos Aplicados. Dominio absoluto del Método Iterativo de Newton-Raphson aplicado a raíces polinómicas multidimensionales. Entendimiento avanzado del "Amplification Coefficient" (A), `D` Invariant (Total Supply Virtual), Virtual Prices, y las implicaciones de gas de resolver bucles `while` on-chain (Solidty implementation of Curve).

## 3. Capacidades principales
1. Cálculo Iterativo off-chain del Invariante Global `D` (La métrica de salud y balance de un pool de Curve).
2. Cálculo de Salida (`get_dy`, `get_dy_underlying`) resolviendo el polinomio de 3er orden o superior (Pools de 2, 3 o 4 tokens como el `3pool`).
3. Modelado de depósitos y retiros balanceados/desbalanceados (Saber si añadir liquidez ayuda al peg del pool o lo daña, y cómo eso afecta los fees de recompensa).
4. Manejo adaptativo del "Ramp Up / Ramp Down" del parámetro de amplificación `A` (Si la DAO vota cambiar la curva, el sistema lee el timestamp y predice el `A` futuro al segundo).
5. Ajustes de Comisiones asimétricas (Admin fees vs LPs fees).
6. Cálculos de precisiones distintas (Pools que mezclan USDT con 6 decimales, WBTC con 8 y DAI con 18 decimales) escalando todo a 18 decimales uniformemente (`rates` adjustments).
7. Simulación de los "Metapools" (Pools acoplados a otros pools, ej. GUSD metapool unido a 3pool).
8. Conversión matemática entre Tokens Underlying (Token original) y Wrapped (cTokens, aTokens).
9. Mapeo de errores de tolerancia de Newton (Evita bucles infinitos en el hardware local que ocurren por oscilaciones en la convergencia).
10. Adaptación a la versión "Crypto V2" de Curve (AMM Dinámicos que mueven su propio Price Peg como el tricrypto pool).

## 4. Entradas requeridas
- `token_balances`: Saldos actuales del Smart Contract (Escalados o no escalados, dependiendo de la versión).
- `amplification_parameter`: Parámetro `A` que aplana la curva (Alta `A` = se comporta como suma constante, baja `A` = como Uniswap V2).
- `rates` / `multipliers`: Multiplicadores para igualar los decimales al estándar matemático.
- `amount_in` / `indexes`: Índices (i, j) indicando qué token entra y cuál sale.

## 5. Salidas esperadas
- `dy_calculated`: Tokens de salida proyectados en su cantidad neta descontando comisiones.
- `dy_gross`: Salida bruta antes de comisiones, para rastreo de spread general.
- `projected_D`: El valor del Invariante D tras la inyección (si es necesario para multicálculos).
- `convergence_iterations`: Número de iteraciones usadas por Newton-Raphson (para monitorizar salud algorítmica).

## 6. Reglas inmutables
- TODAS las operaciones de este motor deben programarse simulando desbordamientos y divisiones enteras de Solidity. El invariante D no puede resolverse con librerías de Float estandar, debe resolverse con BigInt puro iterativo hasta que la diferencia entre iteraciones `|D - prev_D| <= 1`.
- Si el bucle iterativo (Newton-Raphson) excede las 255 iteraciones, el cálculo debe abortarse inmediatamente devolviendo `MathError: Non-convergence` para proteger el Thread HFT.
- Nunca operar sobre un Metapool sin haber leído sincrónicamente el estado subyacente (Base Pool Virtual Price).

## 7. Algoritmos o métodos que debe conocer
- Newton's Method (Newton-Raphson) iterativo.
- Polynomial Root Finding para el Stableswap Invariant.
- Fórmulas de Vyper portadas a lenguajes estáticos.

## 8. Fórmulas críticas
- **Stableswap Invariant General**: `A * n^n * sum(x_i) + D = A * D * n^n + D^(n+1) / (n^n * prod(x_i))`
- **Bucle Iterativo para calcular D (Simplified)**:
  `D_{t+1} = (Ann * S + D_t * P * n) * D_t / ((Ann - 1) * D_t + (n + 1) * P)` (Iterado hasta convergencia).
- **Cálculo del Valor Salida Y (get_y)**:
  Encontrar `y` tal que el nuevo producto y suma preserven el `D` original.

## 9. Casos extremos
- De-Peg colosal: El token A pierde su anclaje y cae un 90% (Ej. LUNA/UST). El pool se vacía completamente de token B, el Invariante colapsa, y Newton-Raphson rebota entre el infinito y cero provocando pánicos matemáticos (NaN/Overflow). El sistema debe truncar y aislar la falla de inmediato.
- Pool con liquidez absurdamente masiva (Miles de millones) que genera overflows en variables intermedias si el bot local fue implementado con u128 en lugar de u256.
- Alteraciones programadas en `A`: La simulación falla si asume un `A` estático cuando el contrato estaba en medio de un rampa temporal (`A` cambia cada segundo según el timestamp del bloque).

## 10. Validaciones obligatorias
- PRE: Chequear si el pool es un Curve V1 (Stableswap clásico) o Curve V2 (Crypto-pools con EMA oracles y peg dinámico). Aplicar el solver respectivo.
- CÁLCULO: Validar el Multiplicador de escala. (Si operas USDT, la entrada debe multiplicarse por `10^12` para calcular en el motor de Curve antes de enviarse al iterador).
- POST: Al terminar, el resultado bruto debe multiplicarse inversamente, restarse el fee (`fee * dy / 10**10` típicamente en Curve), y entregarse.

## 11. Criterios de aprobación
- La iteración converge en menos de 10 ciclos (Curve típicamente converge en 4 a 6 ciclos en estado normal).
- El error matemático (Tolerancia) es igual o menor a 1 Wei.

## 12. Criterios de rechazo
- El cálculo no converge tras el límite máximo de ciclos de seguridad.
- El saldo del token a vaciar (`y`) tras el swap indica que el pool de Curve quedará peligrosamente desequilibrado (Ej. Un pool 3pool quedando 98% USDT y 2% USDC/DAI). Oportunidad de alto riesgo direccional.

## 13. Riesgos que mitiga
- Riesgo de Opacidad "Black Box": Operar contra contratos Vyper muy complejos usando librerías externas lentas y confiar ciegamente. Si el bot portó la matemática, sabe exactamente a qué precio caerá el de-peg al centavo.
- Riesgo de Underpricing (Cálculos conservadores irreales): Asumir que Curve tiene el mismo slippage que UniswapV2 hace perder al fondo millones en volumen transable. La matemática de Curve prueba que el pool aguanta transacciones gigantes, liberando el Optimizador de Tamaño (Skill 2) para maximizar la ejecución.

## 14. Integración con otras skills
- Núcleo central operativo para el Arbitraje de Stablecoins (Skill 15).
- Agente esencial de consulta para el Ruteador de Agregadores (Skill 20) para verificar cota de 1inch contra la realidad.

## 15. Modelo de datos sugerido
```json
{
  "CurveMathExecution": {
    "pool_type": "stableswap_3pool",
    "token_i_index": 1,
    "token_j_index": 2,
    "amount_in_normalized": "50000000000000000000000",
    "projected_dy": "49998001000000000000000",
    "solver_iterations": 4,
    "amplification_parameter": 2000,
    "converged": true
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Un módulo puro sin estado (Stateless Pure Module) con implementaciones en ensamblador de WebAssembly (WASM) si se corre en Node.js, para proteger el Event Loop de las intensivas iteraciones BigInt algebraicas.

## 17. Logs obligatorios
- `[DEBUG] Curve Math: Solving for D with A=2000. Converged in 5 iterations. D=150239010.`
- `[WARN] Curve Solver: Extreme De-peg detected on output token. Convergence struggled (25 iterations). Returning cautious result.`
- `[CRITICAL] Newton-Raphson failed to converge. Reverting calculation gracefully. Flagging pool state.`

## 18. Métricas obligatorias
- `curve_solver_convergence_iterations_avg`.
- `curve_math_latency_us` (Ideal < 10us por operación simple).
- `precision_mismatch_alerts` (Si una simulación difiere del blockchain, el bot lo reporta al equipo analítico para ajustar constantes).

## 19. Tests unitarios
- Solución Newton: Extraer el estado (Saldos y A) del `3pool` (DAI/USDC/USDT) de Ethereum en un bloque histórico famoso, y comparar la salida del solver nativo de Python/Rust con el emitido por el Smart Contract exacto hasta el último Wei.
- Dynamic A Ramp: Alimentar un timestamp futuro simulado. La función debe interpolar el valor de `A` entre `initial_A` y `future_A`.
- Metapool Abstraction: Intercambiar TUSD por USDC en el Metapool TUSD-3pool. La matemática debe desenvolver (Unwrap) el cálculo llamando al solver del base pool en cadena transparente.

## 20. Tests de integración
- Usar un Fork local de mainnet. Hacer un multicall a 100 pools de Curve, calcular 100 cotizaciones cruzadas usando el solver local de la Skill, confirmar con llamadas RPC simuladas `eth_call` y verificar exactitud 1:1.

## 21. Tests E2E
- El agente lee una divergencia de 0.2% en FRAX/USDC. Llama a Lectura On-chain para extraer balanzas. Inyecta los números al solver iterativo de Curve Math. Determina que puede ejecutar $2 Millones con slippage marginal. Envía el trade a Arbitraje DEX-DEX para su protección atómica. Resulta exitoso.

## 22. Checklist de producción
- [ ] Optimización `uint256` en las multiplicaciones: Si `amount * A * N` sobrepasa 256 bits, es mandatorio asegurar que las versiones portadas emulen los errores y mitigaciones implementadas por los matemáticos rusos de Curve en Vyper.
- [ ] Incorporación global de Rate Adjustments, especialmente con stablecoins que cobran interest pasivo (aTokens, cTokens) que mutan su saldo virtual bloque a bloque.
- [ ] Aborto silencioso (Silent fail) activado por defecto para proteger el runtime general.

## 23. Ejemplo de configuración no hardcodeada
```yaml
curve_math_engine:
  max_iterations_newton_raphson: 255
  fallback_to_rpc_simulation_on_fail: true
  enable_metapool_support: true
  wasm_hardware_acceleration: true
```

## 24. Ejemplo de pseudocódigo
```javascript
function get_y(i, j, x, balances, A) {
    // Solves polynomial for token j given new balance of token i (x)
    const N_COINS = BigInt(balances.length);
    const D = get_D(balances, A);
    const Ann = A * N_COINS;
    
    let c = D;
    let S_ = 0n;
    let _x = 0n;
    let y_prev = 0n;
    
    for (let _i = 0; _i < N_COINS; _i++) {
        if (_i === i) _x = x;
        else if (_i !== j) _x = balances[_i];
        else continue;
        
        S_ += _x;
        c = (c * D) / (_x * N_COINS);
    }
    
    c = (c * D) / (Ann * N_COINS);
    const b = S_ + (D / Ann);
    let y = D; // initial guess
    
    for (let iter = 0; iter < 255; iter++) {
        y_prev = y;
        y = ((y * y) + c) / ((y * 2n) + b - D);
        
        // Check convergence (diff <= 1)
        if (absDifference(y, y_prev) <= 1n) {
            return y;
        }
    }
    throw new Error("Did not converge");
}
```

## 25. Criterio final de excelencia
El motor de Curve es un prodigio analítico capaz de destripar ecuaciones cúbicas y cuárticas de la De-Fi pesada en microsegundos, asegurando el control y cálculo preciso sobre la liquidez de Stablecoins, protegiendo al agente autónomo de resbalar al operar grandes ballenas de capital.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Overflows crudos en ports a lenguajes no tipados estáticamente como JS sin validaciones. (Recomendado 100% Rust/WASM).
- Dependencias: Soporte U256 estricto, Skill de Stablecoins.
- Próxima skill: Balancer weighted pools (Skill 27).
