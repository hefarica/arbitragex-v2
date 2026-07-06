# SKILL 028 — Flash loans mastery

## 1. Propósito superior
Proporcionar al agente liquidez infinita "libre de riesgo crediticio" usando Préstamos Relámpago (Flash Loans y Flash Swaps). Esta skill permite pedir prestado millones de dólares en criptoactivos, ejecutar el arbitraje atómico en el mercado, y devolver el préstamo más la comisión dentro del mismo bloque. Si el arbitraje falla, la transacción de solicitud de préstamo simplemente revierte (Revert) asegurando 100% de preservación de capital y erradicando el riesgo de liquidación o desbalance de cartera permanente.

## 2. Nivel de conocimiento requerido
Experto en EVM (Ethereum Virtual Machine) Execution Flow, Composability On-Chain y Arbitraje de Latencia 0. Comprensión absoluta de la delegación de transacciones (Callbacks, `executeOperation`, `uniswapV2Call`), análisis matemático de "Flash Loan Fees" (e.g. 0.05% en Aave V3, 0.3% en Uniswap V2 Flash Swaps, 0% en dYdX / MakerDAO), y optimización de gas para evitar el desangre de "Revert Gas Penalties".

## 3. Capacidades principales
1. Selección de Proveedor de Préstamo: Identifica automáticamente cuál protocolo (Aave, Balancer, Uniswap V2, MakerDAO, dYdX) ofrece el préstamo más barato para la moneda necesaria.
2. Inyección de Calldata Mágica: Empaqueta toda la lógica del Arbitraje (Los saltos del DEX A al DEX B) dentro del payload oscuro de inicialización del préstamo.
3. Tratamiento de Interfaces (Callbacks): Construcción de los listeners exigidos por los protocolos (e.g., `IFlashLoanSimpleReceiver` de Aave) dentro del Smart Contract Proxy del usuario.
4. Descuento matemático en Simulación: Integra inmediatamente el "Premium/Fee" del flash loan al cálculo matemático del margen neto para decidir la ejecución.
5. Emulación del Flash Swap de Uniswap: Toma prestado el activo B del pool pagando con el activo A *después* del trade (Pide prestado sin poner capital inicial y paga con los beneficios inmediatos).
6. Tolerancia a Cero-Liquidez: Si Aave no tiene 5 Millones de USDC libres en su pool en ese instante, el router migra al proveedor Balancer Flash Loans (Fee 0%).
7. Arbitraje de gran escala (Whale Arbitrage): Escalar los beneficios de operaciones marginales (ej. un gap de 0.05% de stablecoins en Curve) inyectando capital masivo ($10M) sin poseerlo.
8. Prevención de ataques de Re-entrancy (Re-entrada) que podrían explotar el contrato Proxy en el callback.
9. Codificación de transacciones atómicas "All-or-Nothing": Si el profit post-préstamo no paga la cuota (Premium), la función revierte con el error "Insufficient Profit to Repay".
10. Soporte Multicadena: Manejar direcciones y arquitecturas de Flash Loan en Arbitrum, Polygon, Optimism y BSC.

## 4. Entradas requeridas
- `borrow_asset`: Moneda a pedir prestada (ej. USDC, WETH).
- `borrow_amount`: Tamaño masivo del préstamo.
- `arbitrage_payload`: Los saltos encriptados en Hexadecimal (`bytes data`) a ejecutar con el dinero prestado.
- `flash_providers`: Lista de protocolos de liquidez disponibles en la red objetivo y sus costos base.

## 5. Salidas esperadas
- `optimal_flash_route`: Proveedor elegido (Ej. `AAVE_V3`).
- `encoded_flash_tx`: Calldata maestro que inicia la petición de préstamo desde la EOA (Externally Owned Account) hacia el contrato proxy.
- `premium_cost_usd`: Costo absoluto proyectado del uso del dinero prestado.

## 6. Reglas inmutables
- JAMÁS intentar un flash loan si el beneficio neto proyectado post-gas es menor al `Premium` (Fee del préstamo).
- Toda la ejecución debe ocurrir dentro de una única transacción (El mismo bloque).
- En el código del Smart Contract, la última línea de la función `callback` DEBE aprobar u otorgar permiso (Allowance) o devolver directamente (`transfer`) el dinero al originador del préstamo (Pool). Si no, el protocolo bloqueará la transacción.
- Evitar usar Aave (Fee 0.05% = 5 BPS) para arbitrajes ultrafinos (< 10 BPS) ya que el premium devora la ganancia; usar "Flash Swaps" de DEXes o Balancer Flash Loans (0% fee).

## 7. Algoritmos o métodos que debe conocer
- Optimistic Transfers (El protocolo te envía el dinero primero y te hace un `require` de devolución al final del callstack).
- Smart Contract Callbacks (`receiveFlashLoan`, `uniswapV2Call`).
- ABI Packing para "State Passing" (Pasar parámetros complejos a través del campo `bytes data` para recuperar estado en el callback sin leer el Storage, salvando gas).

## 8. Fórmulas críticas
- **Costo de Préstamo (Premium)**: `Premium = Borrow_Amount * Flash_Fee_Pct`
- **Condición de Rentabilidad Absoluta**: `Profit_Gross > Gas_Total + Premium + Slippage`
- **Condición de Cierre (Smart Contract)**: `require(balanceOf(this) >= Borrow_Amount + Premium, "Not enough to repay");`

## 9. Casos extremos
- Intercepción de Flash Loan: Alguien monitorea la Mempool, clona el Flash Loan del bot, y lo envía con más gas (Front-running / Sandwiching). Se soluciona forzando el envío de este tipo de transacciones por Flashbots o RPC Privados.
- Contrato Proxy sin fondos extra: El arbitraje termina con $1 de pérdida técnica debido a un redondeo de Gas, el protocolo requiere la devolución íntegra y el contrato revierte costando $15 de Gas sin beneficio.
- Falsa liquidez: El protocolo marca que tiene $50M disponibles para prestar, pero la mayoría están utilizados como colateral por otros usuarios, haciendo fallar la operación al intentar tomarlos.

## 10. Validaciones obligatorias
- PRE: Validar dinámicamente el Flash Loan Fee rate. (Los protocolos como Aave V3 o Maker cambian las tarifas por gobernanza. Si el bot asume un fee falso, revierte o pierde dinero).
- CÁLCULO: Incorporar el Fee del préstamo a la Curva de Optimización Convexa (Skill 2). A mayor préstamo, más fee. El tamaño perfecto se detiene donde el impacto de mercado es igual al incremento del Premium.
- POST: Incorporar comprobación de retiro (`sweepToken`). Si el arbitraje produce 500 USDC de profit, el contrato debe estar configurado para barrer ese profit a la cold wallet del fondo.

## 11. Criterios de aprobación
- Existe un proveedor con Liquidez suficiente `> amount_borrow`.
- El Premium descontado permite un arbitraje rentable positivo neto `> Min_Profit_Target`.

## 12. Criterios de rechazo
- El gas requerido para abrir el Flash Loan y saltar entre contratos (+300,000 gas extra en promedio frente a trade directo) asfixia matemáticamente la ganancia.
- El token a pedir prestado no es soportado por el pool de Aave/Balancer en esa red.

## 13. Riesgos que mitiga
- Riesgo de Capital Inmovilizado: En lugar de tener $5 Millones aparcados en USDC ganando 5% anual, permite mantener un capital base pequeño ($10k para gas) y pedir $5M bajo demanda, elevando el ROI sobre capital propio al infinito.
- Riesgo Direccional Residual: Estás obligado a resolver el trade y devolver el balance a su estado original en el mismo bloque, lo que hace literalmente imposible quedarse "atrapado" comprando un activo que se desploma.

## 14. Integración con otras skills
- Aumenta logarítmicamente el poder destructivo del Arbitraje DEX-DEX (Skill 13) y de Stablecoins (Skill 15).
- Protegido fuertemente por Simulaciones on-chain pre-trade (Skill 29).

## 15. Modelo de datos sugerido
```json
{
  "FlashLoanExecution": {
    "provider": "aave_v3",
    "borrow_asset": "USDC",
    "borrow_amount": 5000000.0,
    "fee_bps": 5,
    "premium_cost_usd": 2500.0,
    "gross_arbitrage_profit": 3100.0,
    "net_profit_after_repay": 600.0,
    "gas_estimate": 450000,
    "status": "APPROVED_FLASH_PAYLOAD"
  }
}
```

## 16. Endpoints o interfaces sugeridas
- Generador Dinámico de Calldata (Payload Builder) que compone el string hexadecimal para inyectar en la función `flashLoan` del pool seleccionado.

## 17. Logs obligatorios
- `[INFO] Requesting Flash Loan: $1.5M USDC via Balancer Vault (0% fee). Executing Curve/UniV3 arb sequence.`
- `[WARN] Aave V3 Liquidity insufficient for $10M loan request. Failing over to UniswapV2 Flash Swap logic.`
- `[CRITICAL] Flash Loan execution Reverted internally. Insufficient profit generated to repay premium. Gas burned: $12.`

## 18. Métricas obligatorias
- `flash_loan_success_rate`.
- `total_volume_borrowed_usd`.
- `flash_premium_paid_usd` (Para reportes contables).
- `gas_burn_on_failed_flashloans` (Vital para vigilar la eficiencia del simulador local).

## 19. Tests unitarios
- Selector de Proveedor: Configurar oportunidad de 0.06% profit. El sistema DEBE descartar Aave (0.05% fee) porque 0.01% neto no cubre el gas, y debe buscar proveedor a 0% fee como Balancer o dYdX.
- Calldata Packing: Enviar un array de "saltos" (hops), empaquetar con `abi.encode` en TS, decodificar de forma simulada y validar que los datos no se corrompen al meterlos en la barriga del payload.
- Matemática del Premium: Validar redondeos matemáticos usando BigInt para asegurar que no se calcula un Wei de menos en el repago, causando revert masivo.

## 20. Tests de integración
- Levantar un fork (Hardhat/Anvil) del bloque actual de Mainnet. Pedir prestado a Aave V3 $1M de USDC, simular el arbitraje, y emitir el log del Callback exitoso comprobando que el `profit` es retenido.

## 21. Tests E2E
- El motor supremo identifica un de-peg menor en TUSD. Llama a esta skill solicitando $50 Millones en un Flash Loan de Aave V3. Empaqueta el proxy, inyecta la transacción atómica con `eth_call`, comprueba que es rentable, la envía por Flashbots RPC protegido y recibe una confirmación de un Profit neto de $4,500 en un solo bloque.

## 22. Checklist de producción
- [ ] Incorporación de Lógica Flash Swap en UniswapV2 (El método más barato de pedir prestado si la pata A del arbitraje ocurre en UniswapV2 mismo).
- [ ] Aislamiento de riesgos en el Callback: Poner mitigaciones contra manipuladores externos que llamen al `receiveFlashLoan` o `executeOperation` del proxy intentando extraer dinero (Restricciones `require(msg.sender == AavePoolAddress)`).
- [ ] Optimización `Approve`: Usar `approve` a 0 y luego a Amount exacto, o mejor, usar firmas EIP-2612 `permit` si el token lo permite.

## 23. Ejemplo de configuración no hardcodeada
```yaml
flash_loan_engine:
  providers:
    - id: "balancer_vault"
      fee_bps: 0
      address: "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
    - id: "aave_v3"
      fee_bps: 5
      address: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"
  max_gas_overhead_flash_loan: 200000
```

## 24. Ejemplo de pseudocódigo
```javascript
function selectFlashProvider(amountToBorrow, grossProfitPct, token) {
    const sortedProviders = getProvidersByFee(token);
    
    for (let provider of sortedProviders) {
        if (provider.availableLiquidity > amountToBorrow) {
            const netProfitPct = grossProfitPct - (provider.fee_bps / 10000);
            if (netProfitPct > MINIMUM_ARBITRAGE_NET_PCT) {
                return provider;
            }
        }
    }
    throw new Error("No profitable flash loan provider with enough liquidity");
}

function buildFlashPayload(provider, amount, arbCalldata) {
    // Encodes the proxy call into the format expected by Aave or Balancer
    if (provider.id === 'aave_v3') {
        const aaveInterface = new Interface(AAVE_POOL_ABI);
        return aaveInterface.encodeFunctionData("flashLoanSimple", [
            CONFIG.proxyContractAddress,
            tokenAddress,
            amount,
            arbCalldata, // Extra encoded bytes passed to the callback
            0 // Referral code
        ]);
    }
}
```

## 25. Criterio final de excelencia
El sistema convierte un agente de $1,000 en una ballena de mercado con poder de compra instantáneo de $100,000,000. Opera exclusivamente sin garantía, destruyendo micro-oportunidades inofensivas inyectándoles volumen titánico de la manera más elegante y eficiente posible en costos de red.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Front-running en la Mempool pública, por atacantes que copian tu payload atómico modificando la cuenta de destino (Se arregla combinando esto con RPC MEV protection).
- Dependencias: MEV Protection, Proxy Smart Contract.
- Próxima skill: Simulación pre-trade (eth_call) (Skill 29).
