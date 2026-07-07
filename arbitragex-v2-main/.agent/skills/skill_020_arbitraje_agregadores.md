# SKILL 020 — Arbitraje entre agregadores

## 1. Propósito superior
Explotar deficiencias en los algoritmos de enrutamiento (Smart Order Routers) de los principales agregadores DEX (1inch, Paraswap, Matcha/0x, KyberSwap, CowSwap). Convierte las asimetrías algorítmicas en beneficios capturando el spread entre la ruta que el Agregador A considera "óptima" y la ruta que el Agregador B ofrece, mediante operaciones de Flash-Swaps sin exposición direccional.

## 2. Nivel de conocimiento requerido
Experto en Arquitectura de DEX Aggregators, Call-data Manipulation, y Off-chain/On-chain pricing mechanics. Comprensión profunda de cómo 1inch utiliza el algoritmo "Pathfinder", el gas overhead de los subcontratos, y los mecanismos de "Positive Slippage" y "MEV protection" propios de cada protocolo.

## 3. Capacidades principales
1. Interrogación concurrente de alta frecuencia a las APIs/SDKs de múltiples agregadores para una misma cantidad de tokens de entrada.
2. Descubrimiento de Inversión de Tasas: Cuando 1inch paga 100.5 USDC por 100 USDT, y Matcha pide 100 USDT por 100.2 USDC.
3. Decodificación y limpieza de "Calldata" (Los agregadores empaquetan la TX con su propia comisión o trackers; la skill debe limpiarlo si va a armar un Smart Contract de Arbitraje atómico).
4. Estimación de "Gas Overhead": Los agregadores gastan más gas que un trade directo en Uniswap debido a su lógica de sub-ruteo. El beneficio debe superar el costo engrosado de la transacción.
5. Captura de "Positive Slippage": Entender qué agregador retiene el slippage positivo (como 1inch) y cuál lo devuelve al usuario (ej. CowSwap), ajustando el cálculo de ganancia real.
6. Bypass de APIs REST lentas: Utilizar directamente los contratos inteligentes de los agregadores (On-chain execution) o usar feeds dedicados (Websocket SDKs) cuando estén disponibles.
7. Ruteo de liquidez fragmentada: Enviar 60% del volumen a 1inch y 40% a Paraswap dentro del mismo contrato atómico para maximizar la ejecución contra otro agregador.
8. Gestión de Nonce y Autorizaciones ("Approve"): Optimizar gas ejecutando aprobaciones infinitas una sola vez para los contratos de los agregadores.
9. Uso de simuladores de gas en red local (Hardhat/Anvil fork) para predecir si el ruteo interno del agregador revertirá por colisiones de MEV.
10. Operaciones en redes de muy bajo gas (Polygon, Arbitrum) donde los ineficientes "Multi-hop" de los agregadores son muy rentables.

## 4. Entradas requeridas
- `token_in` / `token_out`: Par de activos objetivo (Generalmente pares estables o bluechips hiperlíquidos).
- `volume_usd_range`: Rango dinámico a consultar (e.g. $10k, $50k, $100k).
- `aggregator_apis`: Endpoints de cotización (Quotes) de 1inch, 0x API, Paraswap, etc.
- `gas_price`: Oracle de gas en tiempo real.

## 5. Salidas esperadas
- `aggregator_spread`: La diferencia matemática entre las mejores cotas.
- `atomic_calldata`: Datos hexadecimales preparados para inyectar a un contrato atómico multicall.
- `expected_net_profit`: ROI en moneda base tras gas de ruteadores complejos.
- `rejection_reason`: Motivo (ej. "Aggregator Gas overhead destroys profit").

## 6. Reglas inmutables
- Nunca ejecutar manualmente el trade usando las UI de los agregadores. Todo debe rutar atómicamente por el propio contrato `ArbitrageX` para garantizar el revert si la cota del agregador desaparece milisegundos antes del bloque.
- Se debe deducir obligatoriamente el impacto del "Gas Limit" provisto por el agregador; a menudo, sus rutas complejas consumen 500,000+ Gas, lo que invalida ganancias marginales en L1.
- No utilizar agregadores de tipo "Batch Auction" o "Intent-based" (Ej. CowSwap estándar o UniswapX) en patas que requieran inmediatez síncrona dentro del mismo bloque, salvo que se usen en arquitecturas especializadas.
- Descartar respuestas de APIs de agregadores que tarden > 200ms en responder la cotización (Cotización caduca).

## 7. Algoritmos o métodos que debe conocer
- Decodificación ABI (`ethers.utils.defaultAbiCoder`).
- Algoritmos heurísticos de Multi-Armed Bandit (Para decidir a qué APIs de agregadores preguntar volumen con más frecuencia basándose en win-rates pasados).
- Smart Contract Composability (Llamar a un proxy de 1inch, recibir tokens, y en la misma función llamar a un proxy de 0x).

## 8. Fórmulas críticas
- **Ineficiencia entre Algoritmos (Algorithmic Inefficiency Spread)**: `Cotizacion_Salida_Agregador_A - Cotizacion_Entrada_Agregador_B`
- **Costo de Gas Combinado**: `(Gas_A_Overhead + Gas_B_Overhead + Proxy_Execution_Gas) * Base_Fee_GWEI`
- **Beneficio Neto**: `(Volumen_Out - Volumen_In) - Costo_Gas_USD`

## 9. Casos extremos
- Agregador A promete un retorno espectacular porque usa una pool abandonada en un DEX menor, pero al intentar ejecutar, la pool está vacía/manipulada y la transacción revierte (Trap Pool).
- El contrato del agregador actualizó su lógica ayer sin avisar en la documentación, rompiendo la decodificación del Calldata del bot de arbitraje.
- Latencia extrema del endpoint de 1inch en medio de volatilidad, causando que el bot opere con un precio obsoleto que es barrido por el Mempool.

## 10. Validaciones obligatorias
- PRE: Asegurar que el slippage_tolerance configurado al pedir la cota a las APIs es del `0.01%` a `0%` si el proxy atómico forzará un Revert (Para no darles margen de captura de MEV).
- CÁLCULO: Validar el tamaño máximo de retorno. Las APIs pueden devolver calldata de 10 Kilobytes si la ruta es demencial, lo que destruye la rentabilidad por puro costo de almacenamiento on-chain (calldata gas cost).
- POST: Validar con simulación estricta `eth_call` que el flujo cruza de A hacia B sin encallar por falta de "Approvals" en tiempo real.

## 11. Criterios de aprobación
- Cota A genera más tokens de los requeridos para satisfacer la cota B.
- El tiempo total de petición, proceso y simulación es `< 300ms`.
- El Gas Total proyectado es inferior al beneficio neto.

## 12. Criterios de rechazo
- El "Gas Overhead" reportado por la API del agregador es anormalmente alto (> 800,000 Gas).
- La oportunidad depende exclusivamente de una ruta hacia una pool no auditada o marcada como "Toxic" por el sistema central.

## 13. Riesgos que mitiga
- Riesgo de Opacidad Algorítmica: Aprovecha los márgenes ocultos y las ineficiencias matemáticas de los Pathfinder de terceros en lugar de tener que calcular manualmente rutas de 7 saltos en DEXes obscuros.
- Riesgo Direccional: El inventario empieza y termina en la misma moneda en la misma blockchain dentro del mismo bloque atómico.

## 14. Integración con otras skills
- Requiere Arbitraje DEX-DEX (Skill 13) como ejecutor final.
- Consume Simulación pre-trade (Skill 29).

## 15. Modelo de datos sugerido
```json
{
  "AggregatorArbitrage": {
    "token_in": "WETH",
    "volume_in": 10.0,
    "buy_leg": { "aggregator": "matcha", "expected_out": 31000.5 },
    "sell_leg": { "aggregator": "1inch", "expected_out": 10.005 },
    "gross_profit_token": 0.005,
    "total_gas_cost_usd": 8.50,
    "net_profit_usd": 6.50,
    "calldata_size_bytes": 1024
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Pool de peticiones HTTP Keep-Alive asíncronas para disparar `/quote` o `/swap` hacia los principales SDKs.

## 17. Logs obligatorios
- `[INFO] Aggregator Spread Detected: Matcha outpaces 1inch by 0.15% on 100 ETH -> USDC. Extracting calldata for atomic execution.`
- `[WARN] Rejecting 0x API quote. Calldata gas overhead (950k) consumes 120% of the mathematical profit.`
- `[ERROR] Aggregator execution reverted locally. Reason: Intermediate Dex Slippage hit.`

## 18. Métricas obligatorias
- `aggregator_api_latency_ms`.
- `aggregator_spread_occurrences`.
- `calldata_gas_efficiency` (Mide si el ruteador del agregador es un derrochador de gas o es magro).

## 19. Tests unitarios
- Calldata Parsing: Inyectar un JSON complejo de 1inch, verificar que extrae correctamente el campo `to`, `data`, `value`.
- Profit Evaluator: Confirmar que descuenta correctamente el costo del `calldata` (16 gas por byte no nulo, 4 gas por byte cero en Ethereum).
- Slippage overrides: Validar que el bot inyecta slippage 0 a los agregadores (El slippage lo controla el proxy atómico final).

## 20. Tests de integración
- Bucle de "Poll" continuo contra 3 APIs reales (1inch, Paraswap, Odos) durante 5 minutos para monitorizar la tasa de límites superados (HTTP 429) e implementar backoff pasivo.

## 21. Tests E2E
- El agente rastrea L2s baratas (Polygon/Base), encuentra asimetría entre Odos y KyberSwap para MATIC/USDC, extrae calldata de ambos, consolida en proxy Smart Contract, simula con éxito, inyecta TX, y recaba profit.

## 22. Checklist de producción
- [ ] Incorporación de SDKs en lenguajes rápidos (Rust/Go) o bindings nativos para evitar parsear strings JSON pesados (1inch manda respuestas inmensas).
- [ ] Función de Blacklist estático: Evitar ciertos agregadores o pools tóxicos configurables vía dashboard.
- [ ] Arquitectura de Smart Contract Proxy debe tener `payable` y funciones de `sweep()` para recoger sobrantes por slippage positivo.

## 23. Ejemplo de configuración no hardcodeada
```yaml
aggregator_arb:
  enabled_aggregators: ["1inch", "paraswap", "matcha", "odos"]
  min_net_profit_usd: 5.0
  max_acceptable_gas_limit: 750000
  quote_concurrency_timeout_ms: 150
```

## 24. Ejemplo de pseudocódigo
```javascript
async function findAggregatorArbitrage(amountIn, tokenIn, tokenMid) {
    const quotePromises = aggregators.map(agg => agg.getQuote(tokenIn, tokenMid, amountIn));
    
    // Race or Promise.allSettled with strict 150ms timeout
    const quotesMid = await Promise.allSettledWithTimeout(quotePromises, 150);
    
    // Find aggregator giving max amount of tokenMid
    const bestMid = findMaxOutput(quotesMid);
    
    if (!bestMid) return false;
    
    // Ask all aggregators how much tokenIn they give back for that tokenMid amount
    const reversePromises = aggregators.map(agg => agg.getQuote(tokenMid, tokenIn, bestMid.amountOut));
    const quotesEnd = await Promise.allSettledWithTimeout(reversePromises, 150);
    
    // Find aggregator giving max amount of tokenIn back
    const bestEnd = findMaxOutput(quotesEnd);
    
    if (bestEnd.amountOut > amountIn) {
        const grossProfit = bestEnd.amountOut - amountIn;
        const totalGasCost = bestMid.gasEstimate + bestEnd.gasEstimate + PROXY_OVERHEAD_GAS;
        
        if (grossProfit > convertGasToToken(totalGasCost)) {
            return buildAtomicCalldata(bestMid, bestEnd);
        }
    }
    return false;
}
```

## 25. Criterio final de excelencia
El sistema convierte los motores de búsqueda de billones de dólares de los agregadores en sus "empleados gratuitos", extrayendo las ineficiencias cruzadas de sus algoritmos de Machine Learning antes de que el mercado las consolide, ganando la carrera de forma determinista y libre de riesgo direccional.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: API Rate Limits muy agresivos por parte de los agregadores que obliguen a usar rotadores de IP o cuentas enterprise de pago.
- Dependencias: Smart Contract Proxy, Multi-API Orchestration.
- Próxima skill: Lectura on-chain con RPCs reales (Skill 21).
