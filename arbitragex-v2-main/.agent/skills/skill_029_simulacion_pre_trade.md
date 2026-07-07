# SKILL 029 — Simulación pre-trade (eth_call)

## 1. Propósito superior
Actuar como la muralla de cristal final e impenetrable antes de enviar una transacción on-chain (Mainnet/L2). Esta skill ejecuta un simulador de EVM local o vía RPC (Dry-Run / Shadow Execution) forzando que toda la ruta de arbitraje se ejecute lógicamente contra el estado congelado más reciente de la blockchain. Si el simulador predice un `Revert`, la oportunidad se descarta y nunca quema dinero en la red real (Preservación del 100% del Capital para Gas fees).

## 2. Nivel de conocimiento requerido
Experto en EVM Execution Trace, Manejo avanzado de JSON-RPC (`eth_call`, `eth_estimateGas`), y Simulaciones con State Overrides. Comprensión profunda de la memoria local, variables de estado temporales, decodificación de mensajes de error de Solidity (`Panic(uint256)`, `Error(string)`), e integración de frameworks locales como Foundry/Anvil o Hardhat Node.

## 3. Capacidades principales
1. Shadow Execution: Toma el Payload generado para el Smart Contract y lo ejecuta usando `eth_call` sobre el bloque actual.
2. Estimación quirúrgica de Gas: Invoca `eth_estimateGas` para calcular el Gas exacto a consumir, vital para descontar del Profit.
3. Extracción de Errores (Error Decoding): Si la simulación falla, decodifica el Custom Error hexadecimal o la cadena Revert (Ej. `"UniswapV2Router: EXPIRED"` o `"INSUFFICIENT_OUTPUT_AMOUNT"`) indicando exactamente qué matemática del bot está fallando.
4. "State Override" Simulation: Simular cómo se comportaría la transacción *si* el contrato del bot ya tuviese saldo suficiente, alterando temporalmente la memoria de la EVM sin ejecutar TX.
5. Predicción de Bloques Atrasados: Comprobación cruzada entre la lectura original y la simulación. Si la lectura es del bloque 100 y la simulación revierte en el 101, infiere que la oportunidad desapareció o alguien front-runeó el trade (Toxic Block).
6. Validación post-Trade (Profit check in-sim): Usar retornos de función que expongan el saldo (Return values in `eth_call`) para validar matemáticamente que la simulación ganó dólares en la práctica.
7. Balanceo de "Execution Nodes" privados: Redirige las simulaciones a nodos especiales (Ej. Tenderly, Alchemy Trace) en caso de fallos recurrentes oscuros que necesitan debug profundo.
8. Control Concurrente de Nonce: Evita problemas de simulación paralela al manejar la simulación como una llamada read-only, eludiendo bloqueos de cola.
9. Protección contra Over-Estimation: Reducir los buffers de Gas si la simulación es determinista y no estocástica, ahorrando fees extra pagados al validador.
10. Caché de simulaciones cortas: Si una segunda lógica trata de simular la misma ruta en menos de 100ms, saltar la capa de red y devolver la respuesta simulada previa (Debounce).

## 4. Entradas requeridas
- `signed_or_unsigned_payload`: El paquete de datos (`to`, `data`, `value`, `from`) listo para ejecutarse.
- `target_block`: Generalmente `"latest"` o `"pending"`.
- `gas_oracle_estimate`: Precio de Gas y Priority Fee base sugerido.
- `state_overrides`: (Opcional) Cambios dinámicos en saldos o ranuras de memoria para el RPC.

## 5. Salidas esperadas
- `simulation_success`: Booleano crítico.
- `exact_gas_used`: Cantidad de gas requerida.
- `estimated_net_profit`: Resultado devuelto por la función del contrato Proxy (si existe).
- `revert_reason`: Causa humana legible del fallo.

## 6. Reglas inmutables
- NUNCA broadcast a la red (transmitir un TX hash) si la simulación `eth_call` y `eth_estimateGas` revierte o falla. Esta es la garantía de protección contra desangre total de gas (Death by a thousand cuts).
- Toda simulación debe imponer límites de timeout estrictos (ej. < 150ms). Si un nodo tarda mucho simulando, la oportunidad pasará. Retornar "Time-out abort" y proteger el capital.
- Interpretar siempre la "Execution Reverted" sin motivo explícito como un Riesgo Crítico (Puede ser un Honeypot dinámico cambiando su comportamiento al detectar un `eth_call` vs `eth_sendRawTransaction`).

## 7. Algoritmos o métodos que debe conocer
- Decodificación ABI de `Error(string)` y Error codes (0x08c379a0).
- Local Forking Environments (Lanzar simuladores rápidos en RAM de ser necesario).
- Heurística de Profit/Loss en "Dry Runs".

## 8. Fórmulas críticas
- **Costo de Transacción Exacto (Tx_Fee)**: `Exact_Gas_Used * (Base_Fee + Priority_Fee)`
- **Condición Absoluta de Transmisión (Go/No-Go)**: `Profit_Proyectado_Simulado > (Exact_Gas_Used * Gas_Price) * Profit_Buffer_Margin`

## 9. Casos extremos
- Un contrato que intencionadamente detecta si está siendo llamado desde una EOA (Usuario) o si la lectura viene de `eth_call` (`tx.origin == address(0)`), devolviendo una simulación exitosa, pero fallando on-chain (Trampa Anti-Bot sofisticada).
- Nodos RPC públicos simulando la transacción, observando el resultado masivamente rentable, e inyectando esa ruta en su propio Front-Runner MEV robando la oportunidad al bot original antes de que emita la transacción (El simulador te delata).
- Reorganizaciones de Bloque (Reorgs) que validan una simulación contra un bloque, pero el bloque se desvanece de la red principal segundos después.

## 10. Validaciones obligatorias
- PRE: Validar que la cuenta EOA de origen (From) tiene fondos de red (ETH/MATIC) para cubrir el simulacro de ejecución, o simular sin origen fijo en nodos que lo permitan.
- CÁLCULO: Multiplicar `exact_gas_used` por 1.15 (Buffer de seguridad) porque el estado de la EVM es dinámico y un ligero cambio en la escritura (e.g. refunding zeros) puede cambiar sutilmente el gas on-chain.
- POST: Si falla, correr inmediatamente el Módulo de Decodificación de Error. Si falla por "Price Impact", debe realimentar (Feedback Loop) a la Skill de Optimización de Tamaño (Skill 2) para achicar la operación.

## 11. Criterios de aprobación
- `eth_estimateGas` completa su cálculo numérico sin lanzar Revert.
- El beneficio en USD resultante es superior al coste total en Gas calculado estáticamente.

## 12. Criterios de rechazo
- La red local/RPC retorna "Execution reverted: INSUFFICIENT_LIQUIDITY". (Alguien extrajo la liquidez o la simulación matemática local del bot calculó mal).
- Retorno "Execution reverted" genérico (sin String). Evadir la pool por riesgo de ser token scam.

## 13. Riesgos que mitiga
- Riesgo de Pérdida Silenciosa por Gas: Sin esta skill, un bot que encuentra arbitrajes de $10 USD pero requiere rutas que cuestan $15 USD de Gas en Ethereum se arruinará de forma sostenida a un ritmo de $5 por transacción hasta secar la cuenta.
- Front-Running indirecto: Saber si la transacción tiene posibilidades de sobrevivir a las fluctuaciones de red.

## 14. Integración con otras skills
- Es el paso definitivo antes de mandar a Arbitraje DEX-DEX (Skill 13) o Arbitraje Cross-Chain (Skill 19).
- Proveedor oficial de feedback para las funciones matemáticas (Si falla aquí, las mates de Skill 24/26 estuvieron mal).

## 15. Modelo de datos sugerido
```json
{
  "SimulationPreTrade": {
    "target_contract": "0xProxyContract",
    "simulation_success": true,
    "gas_estimate": 410500,
    "projected_tx_fee_usd": 6.10,
    "net_profit_margin_usd": 28.40,
    "revert_reason": null,
    "latency_ms": 42
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Un servicio unificado que consolida peticiones simultáneas de `eth_call` y `eth_estimateGas` mediante Multicall o llamadas paralelas a Nodos Privados hiperrápidos (Alchemy, Erigon Local).

## 17. Logs obligatorios
- `[INFO] Simulation successful. Target: Uniswap/Curve route. Gas Required: 410k. Proceeding to TX Broadcast.`
- `[WARN] Simulation REVERTED on-chain. Reason: 'Slippage limit reached'. Feedback loop engaged to reduce size by 10%.`
- `[CRITICAL] eth_estimateGas failed with OutOfGas. Check proxy code for infinite loops or malicious tokens.`

## 18. Métricas obligatorias
- `simulation_success_ratio` (Una tasa >95% indica que las matemáticas del bot son perfectas y el bot no quema tiempo de simulador).
- `false_positive_revert_rate` (Simulación fallida por nodo desincronizado, no por matemática real).
- `simulation_latency_ms`.

## 19. Tests unitarios
- Revert Decoder: Inyectar el hash hexadecimal exacto equivalente a un error personalizado y validar que el string resultante mapea exactamente a la excepción conocida.
- Multiplier Buffer: Validar que un retorno de `100,000 gas` de la API arroja un límite operativo final de `115,000 gas` en la construcción local del TX.

## 20. Tests de integración
- Levantar un fork (Anvil). Tratar de ejecutar swap contra un pool falso vacío; forzar el `eth_call` y confirmar que el Catch local intercepta la falla, la clasifica y detiene el flujo general.

## 21. Tests E2E
- El bot halla un spread, arma calldata de Flash Loan, lo envía al simulador Pre-Trade. El simulador, usando un proveedor Flashbots, ejecuta, decodifica que ganó 1.2 ETH, aprueba la salida de la señal y desencadena la firma de clave privada.

## 22. Checklist de producción
- [ ] Incorporar suscripción al "Tenderly Simulator API" o nodo Local Geth/Reth para simulaciones pesadas si los límites de Alchemy (Compute Units) se están secando por volumen HFT masivo.
- [ ] Aplicar "State Overrides" si el bot simula un rebalanceo cross-chain donde los fondos aún no llegan físicamente pero quieres confirmar si el spread es válido (Fingir que se tiene saldo alterando el mapping en memoria).
- [ ] Uso exclusivo de RPCs Privados / MEV-Proof Nodes para la simulación, NUNCA usar Ankr o Binance RPC público para simulaciones ricas que delatan a las ballenas o a bots maliciosos.

## 23. Ejemplo de configuración no hardcodeada
```yaml
simulation_engine:
  gas_buffer_multiplier: 1.15
  require_simulate_before_broadcast: true
  max_simulation_latency_ms: 250
  decode_custom_errors: true
  rpc_simulation_node: "https://rpc.flashbots.net/fast"
```

## 24. Ejemplo de pseudocódigo
```javascript
async function simulateTradeSafety(transactionObject, rpcProvider) {
    try {
        // 1. Dry run the call to extract exact return values without mutating state
        const callResultHex = await rpcProvider.call(transactionObject);
        const projectedReturn = decodeProxyReturn(callResultHex);
        
        // 2. Exact gas estimation to calculate overhead cost
        const gasEstimate = await rpcProvider.estimateGas(transactionObject);
        const paddedGas = gasEstimate.mul(115).div(100);
        
        // 3. Mathematical validation of safety
        const gasCostUsd = calculateGasUsd(paddedGas);
        if (projectedReturn < gasCostUsd * MIN_PROFIT_MARGIN_MULTIPLIER) {
             throw new Error("PROFIT_EATEN_BY_GAS");
        }
        
        return { success: true, gas: paddedGas, profit: projectedReturn };
    } catch (error) {
        log.warn("Simulation failed locally, transaction aborted.", decodeRevertReason(error));
        return { success: false, reason: decodeRevertReason(error) };
    }
}
```

## 25. Criterio final de excelencia
El simulador sirve de escudo maestro (Firewall). Convierte las matemáticas teóricas de todas las skills en una prueba empírica "falsa" pero con validación de entorno real. Otorga invencibilidad al sistema bloqueando cualquier desperdicio económico por transacciones que fallarían irremediablemente.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Cambios de estado en el bloque "siguiente" (La simulación del bloque N es perfecta, pero en el bloque N+1 se liquida y falla).
- Dependencias: Gestión RPC Dinámica (Skill 22), Módulo MEV.
- Próxima skill: Detección de honeypots / scam tokens (Skill 30).
