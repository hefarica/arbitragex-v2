# SKILL 051 — Orquestador de Smart Contracts Proxy (L1/L2)

## 1. Propósito superior
Proporcionar un "Chasis On-Chain" (Smart Contract) desplegado en cada red blockchain soportada (Ethereum, Arbitrum, Optimism, Base). Este contrato actúa como la mano de obra delegada del Agente Off-chain. En lugar de interactuar directamente desde una Wallet EOA (Externally Owned Account) con los DEXes, el bot llama a esta Bóveda Proxy, la cual ejecuta múltiples swaps atómicos, verifica balances, paga sobornos (MEV) y garantiza el Revert seguro si la rentabilidad esperada no se cumple al dedillo.

## 2. Nivel de conocimiento requerido
Ingeniero en Seguridad de Smart Contracts (Solidity/Yul), Especialista en Optimización de Gas L1/L2. Conocimiento profundo de "DelegateCalls", Estándar ERC20/ERC20-Permit, Interacción con Pools V2 (Constant Product) y V3 (Concentrated Liquidity), Protección contra Re-Entrancy, y Bypassing de controles Anti-Flashloan.

## 3. Capacidades principales
1. Ejecución Atómica Pura (All-or-Nothing): Encadena un préstamo Flashloan, 3 swaps en distintos DEXes (Uniswap -> Sushiswap -> Balancer) y la devolución del préstamo dentro de una sola transacción L1. Si la ganancia final es menor a la exigida, el contrato lanza un `revert()` destrozando toda la cadena y salvando el capital original.
2. Only-Owner / Whitelisted Caller: Contiene un modificador de seguridad (`modifier onlyBot`) que garantiza que NINGÚN hacker puede llamar a las funciones del contrato para drenar su liquidez o secuestrar los Flashloans.
3. Inyección Dinámica de Bytecode (Multicall): No está harcodeado para una ruta específica. Recibe un array de calldata y direcciones (`targets[]`, `data[]`) y hace un loop ejecutando cada comando ciegamente bajo delegación, convirtiéndose en una "Computadora de Ruteo" universal programable desde Node.js en tiempo real.
4. Auto-Liquidación de Beneficios (Sweep): Si la operación deja excedentes de tokens residuales en el contrato (Dust), la función los convierte de vuelta a WETH/USDC usando 1Inch/Paraswap o los almacena en su balance interno.
5. Control de Pagos al Minero (Coinbase Transfer): Capacidad de inyectar `block.coinbase.transfer(monto)` al final de la ejecución para pagar la porción correspondiente a los Validadores (Skill 50 - MEV Bribes).
6. Wrappers Nativos: Encapsula la molestia de interactuar con la moneda nativa (`msg.value`, ETH puro) y su versión ERC20 (`WETH`). Si necesita WETH, lo envuelve al vuelo (`WETH.deposit{value: X}()`).
7. Balance Checkpoints (Slippage Final): Guarda una "foto" del balance del activo base antes del trade (`balanceBefore = token.balanceOf(address(this))`). Tras los 5 swaps, verifica `balanceAfter`. Si `balanceAfter < balanceBefore + minProfit`, aborta la misión. Es el escudo anti-slippage definitivo.
8. Fallback Receiver (Protección de Drenaje): El contrato tiene capacidad de retirar fondos congelados accidentalmente hacia la Cold Wallet del fondo (Skill 43) en caso de emergencia manual (Emergency Withdraw).
9. Optimización Extrema en Yul (Assembly): Usa código de bajo nivel (Assembly en Solidity) para leer balances y omitir chequeos redundantes del compilador, ahorrando miles de unidades de Gas (esencial para ganar Gas Wars contra otros bots).
10. Detección de Honeypots Internos: Aunque la Skill 30 (Honeypot Detection) trabaja off-chain, el Smart Contract puede inyectar un "Dry Run" de venta minúscula para comprobar el impuesto (Transfer Tax) antes de meter $100,000 en el token, revirtiendo si el impuesto en vuelo resulta ser del 99%.

## 4. Entradas requeridas
- `targets`: Array de direcciones de Contratos DEX.
- `payloads`: Array de bytes (Calldata codificado con los comandos exactos de Swap).
- `values`: ETH/Matic a enviar a cada contrato si se requiere.
- `expectedProfit`: Umbral de beneficio mínimo neto exigido al contrato (Slippage guard).

## 5. Salidas esperadas
- `transactionReceipt`: Hash inmutable en la blockchain validando la mutación de estado.
- `emitted_events`: Eventos de logs (`ArbitrageExecuted(profit, gasUsed)`).
- `revert_message`: Razón del fallo en caso de simulación / ejecución declinada.

## 6. Reglas inmutables
- El Smart Contract NUNCA debe retener inventario activo voluminoso de largo plazo en Capa 1 si no es estrictamente necesario. Debe ser "Stateless" en medida de lo posible para minimizar la superficie de ataque (Surface Attack Area) frente a hacks de contratos.
- La validación `require(msg.sender == owner)` DEBE ser el primer código OpCode ejecutado en cada función (Usando Custom Errors en Solidity >0.8.4 para abaratar Gas).
- Prohibición de Delegación Abierta (DelegateCall a Untrusted addresses): El contrato bot jamás debe hacer DelegateCall a un contrato que no esté matemáticamente auditado o que se extraiga del array de input sin whitelist, ya que un input malicioso podría ejecutar un SELFDESTRUCT y borrar el contrato.

## 7. Algoritmos o métodos que debe conocer
- EIP-1167 Minimal Proxy Contracts (Clones) si se necesita desplegar bóvedas secundarias.
- Yul/Assembly `staticcall` y `delegatecall`.
- Aritmética de punto fijo no-saturada (Unchecked blocks) de Solidity 0.8+ para ahorrar gas en bucles comprobados.

## 8. Fórmulas críticas
- **Bribe Computation en L1**: `uint256 bribe = (currentBalance - initialBalance) * bribePct / 10000; block.coinbase.transfer(bribe);`
- **Revert Condition**: `if (currentBalance < initialBalance + minProfit) revert ProfitSlippageError();`

## 9. Casos extremos
- Front-Running Fallido (Sandwich Target): Un bot MEV atacante logra meter su transacción ANTES que tu contrato. Mueve el precio de Uniswap a las nubes. Tu contrato intenta comprar. Como usas Calldata ciego, el DEX de Uniswap le da a tu contrato una cantidad miserable de tokens. Al final de la cadena, el Contrato evalúa: `Saldo Inicial: 1 ETH. Saldo Final: 0.95 ETH`. El Contrato detona el Custom Error `revert InsufficientProfit()`. El trade se anula. El Validador te cobra el costo de Gas de la reversión (Ej. $10), pero tú salvaste tu capital (Ej. $5,000).
- Fallo de Token Malicioso (Malicious Token Pause): Un administrador corrupto del token "PEPE_V2" pausa las transferencias on-chain (Pausable ERC20) justo mientras el Arbitraje va a la mitad. La llamada `transfer()` arroja error y revierte. El Contrato Orquestador propaga el Revert hacia arriba, limpiando el estado.
- Gas Griefing (Ataque de Gas): Llamar a un contrato ajeno (Ej. un DEX falso) que tiene un bucle infinito `while(true)` diseñado para quemar todo tu límite de Gas. Solución: El Orquestador L1 debe pasar un `gasLimit` estricto en el `.call{gas: maxGas}` interno a cada "Target".

## 10. Validaciones obligatorias
- PRE: Chequear que los balances de Allowance (Permisos ERC20) sean cero o exactos, usando `SafeERC20` de OpenZeppelin, pero preferiblemente limpiando el Approval (Set a 0) después de cada trade para evitar que un DEX hackeado luego te vacíe la billetera.
- CÁLCULO: En operaciones masivas, el uso de Memoria RAM EVM (MSTORE, MLOAD) incrementa cuadráticamente el costo del gas. Pasar arrays empacados o usar ensamblador para leer Offsets de memoria es obligatorio.
- POST: Disparar el evento `ArbitrageSuccess(uint256 profit)` para que los WebSockets de telemetría del Bot off-chain (Node/Rust) confirmen y re-evalúen inventarios sin necesidad de llamar la pesada API RPC.

## 11. Criterios de aprobación
- Un "Swap + Swap" completo entre Uniswap V2 y V3 cuesta < 250,000 unidades de Gas. Un costo superior a 400k Gas indica código ineficiente y hará que las estrategias sean no-rentables a nivel matemático puro.
- El contrato sobrevive al 100% de la Suite Testnet de Hardhat/Foundry incluyendo simulaciones de hackeo L1.

## 12. Criterios de rechazo
- El contrato usa `tx.origin` para autenticación (Vulnerabilidad clásica de Phishing de contratos).
- El contrato no permite retirar "Airdrops" accidentalmente arrojados a su dirección (Fondos atascados).

## 13. Riesgos que mitiga
- Riesgo de Non-Atomicidad: Hacer arbitraje L1 lanzando una TX para Comprar y luego, cuando se confirma, lanzar otra TX para Vender. (El mercado se moverá entre el Bloque 1 y el Bloque 2, llevándote a la ruina). El Smart Contract hace que ambas cosas ocurran atómicamente en el "Mismo Milisegundo" (Mismo Bloque).
- Robo de MEV Bribes: Sin Smart Contract, no puedes pagarle al validador (`block.coinbase`) condicionado al éxito del trade, dependiendo así del frágil sistema de Gas tradicional (Caza ciega).

## 14. Integración con otras skills
- Brazo armado de la ejecución en On-Chain Arbitrage (Skill 13) y de Flash Loans (Skill 28).
- Funciona en sinergia absoluta con Private Tx Routing (Skill 50).

## 15. Modelo de datos sugerido
```solidity
// Struct internal in Smart Contract
struct ArbStep {
    address targetDex;
    bytes payload;
    uint256 msgValue;
    bool requiresApproval;
    address tokenToApprove;
}

struct ArbInstruction {
    ArbStep[] steps;
    address baseToken;
    uint256 expectedProfitMinimum;
    uint256 bribePercentage;
}
```

## 16. Endpoints o interfaces sugeridas
- Smart Contract desplegado en EVM (Ethereum, Arbitrum, BSC, Optimism).
- Función de entrada pública pero securizada: `executeArbitrage(ArbInstruction calldata instruction) external onlyBot;`

## 17. Logs obligatorios (Solidity Events)
- `event ArbitrageExecuted(address indexed token, uint256 profitNet, uint256 minerBribe);`
- `event EmergencyWithdrawal(address indexed token, uint256 amount);`
- `error InsufficientProfit(uint256 expected, uint256 actual); // Custom error`

## 18. Métricas obligatorias
- `gas_units_used_per_transaction` (Optimizar de 300k a 180k unidades significa miles de dólares salvados anualmente).
- `percentage_of_reverts_due_to_slippage`.

## 19. Tests unitarios (Foundry / Hardhat)
- Profit Verification Test: Inicializar el contrato con 1 ETH. Simular 2 llamadas (Swaps falsos) que dejan el balance final en 0.99 ETH. Enviar instrucción con `expectedProfitMinimum = 0.05 ETH`. El contrato DEBE revertir indicando `InsufficientProfit`.
- OnlyOwner Security Check: Intentar llamar a `executeArbitrage()` desde una cuenta "Imposter" (Cuenta #2). El Smart Contract debe escupir `Revert: Not Authorized` en menos de 100 Gas (Rechazo Inmediato) sin leer calldatas pesados.
- Bribe Calculation Math Test: Simular ganancia de 10 ETH. Parámetro bribe = 5000 (50%). El saldo de `block.coinbase` (Minero) debe aumentar exactamente 5 ETH, y el contrato debe quedarse con 5 ETH.

## 20. Tests de integración
- Desplegar el contrato en una "Mainnet Fork" local (Anvil / Ganache). Simular una cuenta inyectando 1 Millón de DAI. El Bot (Node.js) calcula un array de bytes para llamar a Uniswap V3 y Curve. Se manda la transacción al Forcado. El Forcado muta los estados reales de los DEX y se verifica que el DAI del contrato subió a 1 Millón + 5,000 de beneficio.

## 21. Tests E2E
- El fondo HFRC despliega el contrato en Arbitrum pagando el costo único ($5). El Orquestador local identifica la dirección y alimenta la Skill 45 (Llavero) con su address. Al captar una divergencia en USDC, el bot codifica el `executeArbitrage(steps...)` y lo dispara por RPC Privado (Skill 50). El contrato en Arbitrum despierta. Verifica que quien llama es la Skill 45. Recorre el Array. Pide un préstamo flash en Aave (Skill 28), inunda el pool de Sushiswap, vende en GMX, paga el Flash Loan, evalúa que su inventario tiene $50 dólares más de USDC que hace un milisegundo. Despacha el Validador-Bribe de $25 a la Coinbase, y emite el evento al Log. El Orquestador apaga la alarma de fuego on-chain y actualiza contabilidad (Skill 38) asimilando el triunfo.

## 22. Checklist de producción
- [ ] Minimización de ABI Coding en tiempo de ejecución de EVM. Pre-empacar (ABI.encode) toda la instrucción off-chain. El contrato L1 debe hacer cero cálculos complejos de concatenación de bytes, solo extraer punteros y enrutar `.call`.
- [ ] Desplegar versiones aisladas ("Silos") para grandes verticales. A veces mezclar lógicas Flashloan DODO con Flashloan AAVE V3 revienta el límite de tamaño de contrato de 24KB (Spurious Dragon limit). Usar arquitecturas Modulares o Diamond Proxy (EIP-2535) si es muy gordo.
- [ ] Retención de Fallbacks de Receive ETH. El contrato debe implementar `receive() external payable {}` vacío, de lo contrario los préstamos o DEXes nativos que regresan ETH puro fallarán brutalmente rebotando la transacción L1.

## 23. Ejemplo de configuración no hardcodeada
- No aplica (Código inmutable On-Chain en Solidity). Su configuración vive en las variables de inicialización (Constructor args) tales como: `address_owner`, `address_emergency_recovery`.

## 24. Ejemplo de pseudocódigo (Solidity Yul Mixto)
```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract ArbitrageProxy {
    address public immutable botOwner;

    error Unauthorized();
    error SlippageProtectionTriggered(uint256 expected, uint256 actual);

    constructor(address _bot) {
        botOwner = _bot;
    }

    modifier onlyBot() {
        if(msg.sender != botOwner) revert Unauthorized();
        _;
    }

    receive() external payable {}

    function executeAtomic(
        address baseToken,
        address[] calldata targets,
        bytes[] calldata payloads,
        uint256 minProfitExpected,
        uint256 minerBribeBps
    ) external onlyBot {
        uint256 balanceBefore = IERC20(baseToken).balanceOf(address(this));
        
        // Loop over blind execution payloads (Pre-calculated offchain)
        for(uint256 i = 0; i < targets.length; i++) {
            (bool success, ) = targets[i].call(payloads[i]);
            require(success, "DEX_ROUTING_FAILED");
        }

        uint256 balanceAfter = IERC20(baseToken).balanceOf(address(this));
        
        // Slippage / Anti-Sandwich Check
        if (balanceAfter < balanceBefore + minProfitExpected) {
            revert SlippageProtectionTriggered(balanceBefore + minProfitExpected, balanceAfter);
        }

        // Bribe the Validator (MEV integration)
        uint256 profit = balanceAfter - balanceBefore;
        if (minerBribeBps > 0) {
            uint256 bribeAmount = (profit * minerBribeBps) / 10000;
            block.coinbase.transfer(bribeAmount); // Pay the block proposer
        }
    }
    
    function emergencyWithdraw(address token, uint256 amount, address to) external onlyBot {
        IERC20(token).transfer(to, amount);
    }
}
```

## 25. Criterio final de excelencia
El Smart Contract Proxy L1 es la armadura de combate (Mech-Suit) del algoritmo. Sin él, el bot combate desnudo (EOA directa), atado a los límites y latencias del Exchange tradicional. Con este contrato desplegado, el Agente Supremo se transforma en un Ciudadano Nativo L1, con el poder de orquestar transacciones atómicas, pagar mercenarios (Validadores) y asegurar su propio capital con la invulnerabilidad matemática y criptográfica de la Capa 1.

---
ESTADO DE LA SKILL:
- Completitud: 100%
- Lista para implementación: Sí
- Riesgo residual: Vulnerabilidades en el compilador de Solidity (Casi nulas usando versiones altamente estables ej. 0.8.20 y evitando ensamblador excesivamente complejo sin tests fuzzing).
- Dependencias: Blockchain L1/L2 desplegada, MEV Rounter.
- Próxima skill: Liquidation Engine Tracker (Aave/Compound) (Skill 52).
