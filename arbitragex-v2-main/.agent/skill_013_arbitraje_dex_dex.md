# SKILL 013 — Arbitraje DEX-DEX

## 1. Propósito superior
Ejecutar oportunidades de arbitraje íntegramente on-chain entre Exchanges Descentralizados (DEXes) dentro de la misma blockchain o Layer 2. Aprovecha la atomicidad de la EVM (o SVM en Solana) para garantizar matemáticamente cero riesgo direccional: si el trade no es rentable o sufre slippage excesivo (Sandwich/Front-run), la transacción revierte (Revert) costando únicamente el gas base.

## 2. Nivel de conocimiento requerido
Experto en Blockchain Engineering, Solidity/Vyper, Arquitectura de Mempool, y DeFi Microstructure. Comprensión absoluta del Gas Optimization, ruteo on-chain en contratos inteligentes (Smart Contract Routing), EIP-1559, MEV (Miner Extractable Value), y simulaciones locales (`eth_call` sin estado).

## 3. Capacidades principales
1. Ruteo On-Chain: Llamada a un Smart Contract proxy personalizado (Arbitrage Contract) que consolida múltiples swaps en una sola TX atómica.
2. Estimación dinámica de Gas (`eth_estimateGas`) y empaquetamiento del Base Fee + Priority Fee bajo reglas de EIP-1559.
3. Simulación local asíncrona de la ejecución (`eth_call` o Anvil/Hardhat network fork) antes de enviar la TX real.
4. Prevención de MEV básico enviando transacciones a través de RPCs privados (Flashbots, MEV-Share, bloXroute).
5. Interacción con pools de Uniswap V2 (x*y=k), Uniswap V3 (concentrated liquidity), Curve, Balancer y SushiSwap simultáneamente.
6. Cálculo de "Slippage Tolerance" o `amountOutMinimum` derivado exactamente de la Matemática de Slippage (Skill 7).
7. Gestión de Nonce concurrente para evitar el bloqueo del Mempool local.
8. Codificación/Decodificación nativa de calldata ABI para interactuar directo con los routers, ahorrando gas de sub-contratos si es necesario.
9. Uso de multicall para leer reservas de liquidez de 100 pools en una sola petición RPC.
10. Detección y omisión de Tokens Tóxicos (Honeypots, fee-on-transfer, rebasing tokens) que rompen el arbitraje on-chain.

## 4. Entradas requeridas
- `onchain_route`: Secuencia de pools y direcciones de tokens (ej. `WETH -> USDC (UniV3) -> WETH (Sushiswap)`).
- `optimal_size`: Tamaño optimizado de la entrada en WEI.
- `gas_oracle_data`: Precios actuales de la red (Max Fee per Gas, Max Priority).
- `min_profit_wei`: La ganancia mínima innegociable configurada en el Smart Contract.

## 5. Salidas esperadas
- `tx_hash`: Identificador de la transacción inyectada a la red.
- `simulation_status`: Éxito o Fallo del Dry-Run (`eth_call`).
- `execution_receipt`: Log on-chain decodificado detallando el gas gastado real y el profit transferido a la wallet/contrato.

## 6. Reglas inmutables
- Toda transacción DEX-DEX debe ser atómica mediante un Smart Contract propio. Nunca realizar dos transacciones separadas desde una EOA (Externally Owned Account) para el trade.
- El contrato proxy debe implementar un chequeo estricto al final: `require(balanceFinal >= balanceInicial + minProfit, "Arb Failed");`
- Si la simulación local falla por slippage o falta de gas, la transacción JAMÁS se transmite a la red pública (Protección de quema de gas).
- Toda transacción importante en redes EVM públicas (Ethereum, Polygon, BSC) debe rutar por un Endpoint Protector MEV, no por el RPC público estándar.

## 7. Algoritmos o métodos que debe conocer
- Call / Delegatecall ABI encoding.
- State overrides en llamadas RPC para probar cambios sin esperar confirmaciones.
- Gestión de Mempool y reemplazo de transacciones (Bump fee transaction replacement / RBF).
- Matemáticas de Solidity u256 para evitar overflows y redondeos malignos.

## 8. Fórmulas críticas
- **Costo de Gas Total WEI**: `Gas_Limit * (Base_Fee + Priority_Fee)`
- **Condición de Reversión en Contrato**: `if (token.balanceOf(this) < minRequiredOut) revert();`
- **ROI On-Chain Neto**: `(Profit_Token * Token_USD_Price) - Gas_Total_USD`

## 9. Casos extremos
- Un competidor (Searcher) con más gas Priority ejecuta el mismo arbitraje milisegundos antes, causando que nuestra TX falle (Revert) costando $5 en gas.
- Picos locos de congestión de bloque (Base Fee salta 300% de un bloque a otro).
- Pools de tokens hackeados (Rug pulls) con reservas manipuladas que arrojan oportunidades de 10,000% pero revierten en ejecución (HoneyPot trap).
- Latencia del RPC causando que disparemos operaciones con el estado del bloque `N-2` en el bloque `N`.

## 10. Validaciones obligatorias
- PRE: Validar que el contrato inteligente tiene la "Allowance" requerida para mover los fondos, o dársela en la misma TX (Multicall).
- CÁLCULO: Validar la conversión matemática de decimales del token A y token B.
- POST: Validar que el `tx_hash` es indexado, monitorear el bloque por confirmación o reemplazo por "Dropped and Replaced".

## 11. Criterios de aprobación
- La simulación local `eth_call` retorna un profit que excede holgadamente el costo del gas inyectado.
- La latencia hacia el endpoint del RPC constructor privado es menor a 150ms.

## 12. Criterios de rechazo
- El Gas Oracle indica que la red está bajo ataque/saturación (Gas exorbitante rompiendo el margen).
- Simulación local falla con error genérico (ej. "TransferHelper: TRANSFER_FAILED").

## 13. Riesgos que mitiga
- MEV (Miner Extractable Value) Sandwich Attacks: Un searcher ve nuestra orden, compra antes y vende después robando el beneficio, gracias a los RPCs privados.
- Revert Spam: Perder fondos en puros intentos fallidos de gas; la simulación y Atomicidad cortan el flujo de caja negativo.

## 14. Integración con otras skills
- Funciona junto con Lectura on-chain (Skill 21) y Flash Loans (Skill 28).
- Nutre de logs al sistema de Auditoría y Dashboard (Skills 57 y 81).

## 15. Modelo de datos sugerido
```json
{
  "DexArbitrageExecution": {
    "network": "arbitrum_one",
    "tx_hash": "0xabc123...",
    "gas_used": 350000,
    "effective_gas_price_gwei": 0.1,
    "profit_wei": "1500000000000000",
    "simulated_profit": "1520000000000000",
    "is_mev_protected": true
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Librería `ethers.js`, `viem` (TS) o `ethers-rs` (Rust) conectada a múltiples nodos RPC de pago en paralelo (Alchemy, Infura, QuickNode) para redundancia de Mempool.

## 17. Logs obligatorios
- `[INFO] Simulated DEX route successfully. Expected Profit: 0.05 ETH. Gas Cost: 0.01 ETH. Sending via Flashbots.`
- `[WARN] DEX tx reverted locally. Reason: "INSUFFICIENT_OUTPUT_AMOUNT". Block state shifted.`
- `[ERROR] TX Dropped from mempool. Nonce issue or gas underpriced.`

## 18. Métricas obligatorias
- `onchain_tx_success_rate` (Crucial para no desangrar el bot en costos de red).
- `average_gas_spent_usd`
- `rpc_simulation_latency_ms`

## 19. Tests unitarios
- Conversión de decimales: Validar que 1 USDC (6 decimales) a WETH (18 decimales) se procesa en `u256` correctamente.
- ABI Encoding: Testear que el payload generado para el Smart Contract es decodificable e idéntico a las firmas de Solidity.
- Chequeo de Profit estático: Validar la fórmula restando el gas consumido.

## 20. Tests de integración
- Forjar un bloque localmente (Fork mainnet en Anvil), inyectar un desbalance masivo en Uniswap, ejecutar la skill, y corroborar el incremento de balance en la wallet test.

## 21. Tests E2E
- Despliegue en red Arbitrum/Optimism (Bajo costo). Ejecutar un trade cíclico real (ej. `WETH -> USDC -> WETH`) y registrar el profit en la DB con validación post-blockchain de hash inmutable.

## 22. Checklist de producción
- [ ] Uso exclusivo de Flashbots/MEV-protect para Ethereum L1.
- [ ] Implementación de contrato proxy optimizado en ensamblador Yul/Assembly para ahorrar gas (10%-20% más eficiencia).
- [ ] Watcher de transacciones pendientes para re-lanzar con mayor gas si la red cambia de fee repentinamente (Transaction Resubmission).

## 23. Ejemplo de configuración no hardcodeada
```yaml
dex_execution:
  target_network_id: 42161 # Arbitrum One
  gas_buffer_multiplier: 1.15
  rpc_endpoints: 
    - "https://arb-mainnet.g.alchemy.com/v2/${API_KEY}"
  mev_endpoint: "https://rpc.flashbots.net"
  custom_proxy_address: "0x1234567890abcdef..."
```

## 24. Ejemplo de pseudocódigo
```javascript
async function executeDexArbitrage(route, optimalSize, gasConfig) {
    const minProfit = calculateMinProfit(route, gasConfig);
    const calldata = buildProxyCalldata(route, optimalSize, minProfit);
    
    const txObject = {
        to: CONFIG.proxy_contract,
        data: calldata,
        gasLimit: 400000,
        maxFeePerGas: gasConfig.baseFee * 1.2,
        maxPriorityFeePerGas: gasConfig.priorityFee
    };
    
    // Simulate Locally
    try {
        await provider.call(txObject);
    } catch (e) {
        log.warn("Simulation failed, aborting TX to save gas.", e.reason);
        return false;
    }
    
    // Send to MEV protected mempool
    const signedTx = await wallet.signTransaction(txObject);
    const receipt = await flashbotsProvider.sendRawTransaction(signedTx);
    
    return verifyReceipt(receipt);
}
```

## 25. Criterio final de excelencia
El sistema nunca pierde dinero por transacciones revertidas fallidas debido al slippage (porque la simulación y Atomicidad lo previenen), y el 95% de sus operaciones superan los bots básicos por gas optimization del Smart Contract proxy.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: "Uncle blocks" y reorgs en cadenas de L2 rápidas (Polygon/BSC) que pueden engañar el estado simulado.
- Dependencias: Risk Engine, Simulador Pre-trade.
- Próxima skill: Arbitraje triangular (Skill 14).
