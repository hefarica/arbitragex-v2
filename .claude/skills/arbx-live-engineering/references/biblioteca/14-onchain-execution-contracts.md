# 14. EJECUCIÓN ON-CHAIN AVANZADA — CONTRATO EJECUTOR, FLASH LOANS Y GAS GOLFING

CUÁNDO CARGAR ESTA REFERENCIA: escribir, auditar o modificar el contrato ejecutor on-chain; integrar flash loans (Aave V3, Balancer V2) o flash swaps (Uniswap V3); empaquetar calldata de rutas; optimizar gas del hot path del contrato; diseñar approvals (EIP-2612, Permit2); componer swaps con Multicall3 y manejar dust; blindar el contrato contra MEV (minOut, deadline, roles, pausa, rescate); planificar upgrades UUPS con compatibilidad de storage; evaluar impacto de EIP-7702 y EIP-4844 en el flujo de ejecución; escribir invariantes formales del contrato en Foundry; ejecutar el ciclo de despliegue inicial del contrato a una red (script Foundry, proxy ERC-1967 con initialize en el constructor, determinismo CREATE2, hardening post-despliegue con timelock, verificación en explorador, promoción testnet→mainnet).

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Contrato ejecutor minimalista | `execute(bytes hops, ...)` + custom errors + delta check | Sin custodia: principal entra y sale en la misma tx; profit se barre a treasury |
| Flash loan Aave V3 | `IPool.flashLoanSimple` + `executeOperation` | Devolver con `approve(pool, amount + premium)`; premium on-chain vía `FLASHLOAN_PREMIUM_TOTAL()` |
| Flash loan Balancer V2 | `IVault.flashLoan` + `receiveFlashLoan` | Fee cero; devolver `amounts[i] + feeAmounts[i]` a la Vault antes de salir del callback |
| Flash swap Uniswap V3 | `pool.swap` + `uniswapV3SwapCallback` | Pago al pool DENTRO del callback; validar `msg.sender` contra `factory.getPool(...)` |
| Dispatcher de callbacks | whitelist de `msg.sender` por callback | Nunca aceptar callbacks no solicitados; `initiator == address(this)` |
| Gas golfing | tabla EIP-2929/3529 + calldata packing | Medir con `forge test --gas-report`; packing ~25 B/hop vs ≥128 B/hop con structs ABI |
| Approvals | EIP-2612 `permit`, Permit2, approve exacto vs infinito | SafeERC20 obligatorio (tokens sin return bool); USDT exige approve(0) intermedio |
| Batching atómico | Multicall3 `aggregate3` | `allowFailure=false` para atomicidad; sweep de dust en la misma tx |
| MEV-resistencia | minOut de simulación + deadline + roles + pausa + rescate | minOut NUNCA hardcode (RULE 00); rescue ADMIN-only con evento |
| Upgradeabilidad | UUPS + `__gap` + `forge inspect storageLayout` | `_authorizeUpgrade` gated; timelock+multisig como UPGRADER |
| Despliegue inicial | `Deploy.s.sol` + `ERC1967Proxy(impl, initData)` + `forge script --broadcast --verify` | La impl nace con `_disableInitializers()`; `initialize` viaja como `_data` del constructor del proxy |
| Determinismo multi-chain | CREATE2: factory + salt + initcode idénticos | EOA + CREATE es nonce-dependiente → address distinta por chain; chainId NO entra en la derivación CREATE2 |
| EIP-7702 / EIP-4844 | type-0x04 / blob fees | `extcodesize>0` deja de implicar "no es EOA"; blob fee aparta costes DA de L2 del gas market |
| Invariantes formales | Foundry `invariant_*` + handlers | `fail_on_revert = true`; diff de `storageLayout` en CI para upgrades |

Extiende el núcleo §2 (contratos) sin repetirlo: el esqueleto UUPS completo del núcleo es el punto de partida; aquí lo comprimimos a su forma minimal de producción y bajamos al detalle de cada eje.

## 14.1 Contrato ejecutor canónico minimalista

Principios: (1) cero custodia — el contrato no retiene capital entre bloques; el principal del flash loan entra y sale en la misma transacción y el profit se transfiere a `treasury` dentro de `execute`; (2) pull payments — Aave hace pull del principal+premium vía allowance, ningún push de fondos a terceros queda pendiente; (3) un solo punto de entrada de operador; (4) verificación de solvencia por delta de balance, no por promesas de pools.

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

contract MinimalExecutor {
    using SafeERC20 for IERC20;

    error NotOperator();
    error NotAdmin();
    error Paused();
    error Expired(uint256 deadline, uint256 blockTimestamp);
    error Underwater(uint256 got, uint256 want); // delta < deuda + minProfit + gasBuffer
    error NoValueExpected();

    event Executed(
        bytes32 indexed routeHash,     // topic: filtra en PG/edge por ruta
        address indexed token,         // numeraire del ciclo
        uint256 amountIn,
        uint256 debt,                  // principal + premium del flash loan
        uint256 profit,
        uint256 gasUsed,
        uint64 blockNumber
    );
    event Paused(address indexed by);
    event Rescued(address indexed token, uint256 amount);

    address public immutable operator;   // EOA/bot del pipeline (whitelist de caller)
    address public immutable treasury;   // destino del profit (pull-out inmediato)
    bool private _paused;                // 1 SLOAD warm por execute

    constructor(address operator_, address treasury_) {
        operator = operator_;
        treasury = treasury_;
    }

    function execute(
        bytes calldata hops,        // rutas empaquetadas (§14.3): 25 B por hop
        uint256 amountIn,
        uint256 minProfit,          // derivado de simulación + slippage bps (§14.7)
        uint256 gasBuffer,          // estGasUsed * gasPrice calculado off-chain
        uint256 deadline,           // 1-2 bloques de holgura, nunca largo
        bytes32 routeHash           // idempotencia + trazabilidad (núcleo §1.3)
    ) external {
        uint256 gas0 = gasleft();
        if (msg.sender != operator) revert NotOperator();
        if (_paused) revert Paused();
        if (block.timestamp > deadline) revert Expired(deadline, block.timestamp);

        IERC20 token; uint256 before; uint256 debt;
        (token, debt, before) = _flashAndRoute(hops, amountIn); // §14.2

        uint256 delta = token.balanceOf(address(this)) - before;
        uint256 want = debt + minProfit + gasBuffer;
        if (delta < want) revert Underwater(delta, want);

        token.safeTransfer(treasury, delta - debt); // barre TODO el excedente
        emit Executed(routeHash, address(token), amountIn, debt,
                      delta - debt, gas0 - gasleft(), uint64(block.number));
    }

    function pause() external { if (msg.sender != operator) revert NotOperator(); _paused = true; emit Paused(msg.sender); }

    function rescue(address token) external {
        if (msg.sender != treasury) revert NotAdmin(); // ADMIN-only; idealmente detrás de timelock
        uint256 amt = IERC20(token).balanceOf(address(this));
        emit Rescued(token, amt);
        IERC20(token).safeTransfer(treasury, amt);
    }

    receive() external payable { revert NoValueExpected(); } // prohibición de value-stuck
}
```

Notas de diseño. El `gasBuffer` llega por calldata desde la simulación (revm/anvil del núcleo §1.2): estimar gas on-chain con `tx.gasprice` sólo es coherente cuando el numeraire del ciclo es WETH (el gas se paga en ETH). Si el numeraire es un stable u otro token, el costo de gas ya debe venir descontado dentro de `minProfit`; el contrato no puede "cobrarse" gas en un token ajeno. Custom errors en vez de strings: 4 bytes de selector vs 32+N bytes de string — menor bytecode de deploy y decodificación tipada off-chain; el ahorro de gas runtime es real pero menor (~50-100 gas por revert, en memoria/returndata), no lo sobredimensiones. Eventos: máx 3 `indexed` (más el topic de firma); indexa `routeHash` y `token`, no montos, para que el filtrado por índice log sea barato.

Diagrama del ciclo completo (el "terminus" matemático del núcleo §1):

```
bot(operator) ──execute()──▶ MinimalExecutor
   │ 1. IPool.flashLoanSimple(this, token, amountIn, hops, 0)
   │ 2. Aave transfiere principal y llama executeOperation(...)  ◀── dispatcher §14.2
   │ 3. hop1: poolV3.swap(...) → uniswapV3SwapCallback paga al pool
   │ 4. hop2..N: swaps sucesivos, cada minOut del hop viene empaquetado
   │ 5. approve(AAVE_POOL, principal + premium)      ← PULL payment
   │ 6. delta check: balanceDelta >= deuda + minProfit + gasBuffer
   └ 7. safeTransfer(treasury, delta - deuda) + emit Executed(...)
```

## 14.2 Flash loans reales: callbacks exactos y dispatcher seguro

El error clásico de producción: aceptar el callback de cualquiera. Cada callback DEBE validar que `msg.sender` es exactamente la pool esperada (derivada, no confundible) y que la operación fue iniciada por este contrato. Un callback no solicitado es vector de reentrancy y de ejecución forzada en estado adversario.

**Aave V3** (mainnet: PoolAddressesProvider `0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e`; resolver `IPoolAddressesProvider(provider).getPool()` en vez de hardcodear — RULE 00). Entrada: `IPool(pool).flashLoanSimple(receiverAddress, asset, amount, params, referralCode)` con `referralCode = 0`. El Pool transfiere `amount` y llama `executeOperation`. El premium NO se deduce del principal recibido: se debe aprobar `amount + premium`. Consulta el valor vigente con `IPool(pool).FLASHLOAN_PREMIUM_TOTAL()` en tiempo de build del bundle, no lo asumas (es parámetro de governance).

```solidity
function executeOperation(
    address asset,
    uint256 amount,
    uint256 premium,
    address initiator,
    bytes calldata params
) external returns (bool) {
    if (msg.sender != AAVE_POOL) revert NotPool();
    if (initiator != address(this)) revert NotPool(); // sólo flash loans propios
    _runHops(params);                                        // decodifica hops y ejecuta swaps
    IERC20(asset).forceApprove(AAVE_POOL, amount + premium); // SafeERC20: pull payment al final del ciclo
                                                             // (forceApprove resiste tokens sin retorno bool)
    return true;
}
```

**Balancer V2** (Vault canónica mainnet `0xBA12222222228d8Ba445958a75a0704d566BF2C8`). Entrada: `IVault(vault).flashLoan(recipient, tokens, amounts, userData)`. Fee cero en V2 — `feeAmounts[i]` llega en 0, pero se programa contra el valor recibido, no contra la esperanza. La Vault exige recibir `amounts[i] + feeAmounts[i]` de vuelta antes del retorno del callback.

```solidity
function receiveFlashLoan(
    IERC20[] memory tokens,
    uint256[] memory amounts,
    uint256[] memory feeAmounts,
    bytes memory userData
) external {
    if (msg.sender != BAL_VAULT) revert NotPool();
    _runHops(userData);
    for (uint256 i; i < tokens.length; ++i) {
        tokens[i].safeTransfer(BAL_VAULT, amounts[i] + feeAmounts[i]);
    }
}
```

Por fee cero, Balancer es la fuente TLS por defecto cuando la ruta no exige un activo exclusivo de Aave; el trade-off es que la Vault no presta "simple" por diseño multi-token y su callback es `memory` (más caro que `calldata` en decoding).

**Uniswap V3 flash swap** (Factory mainnet `0x1F98431c8aD98523631AE4a59f267346ea31F984`). Se pide prestado del propio par: la pool llama `uniswapV3SwapCallback` y el pago al pool ocurre DENTRO del callback (pull del contrato). Validación del emisor: derivar la pool esperada con `IUniswapV3Factory(factory).getPool(tokenA, tokenB, fee)` — cualquiera puede llamar el callback, sólo la pool legítima pasa el check.

```solidity
function _swapV3(address tokenIn, address tokenOut, uint24 fee, int256 amountSpecified, uint256 minOut) internal {
    IUniswapV3Pool(IUniswapV3Factory(UNIV3_FACTORY).getPool(tokenIn, tokenOut, fee)).swap({
        recipient: address(this),
        zeroForOne: tokenIn < tokenOut,
        amountSpecified: amountSpecified, // exact input: positivo
        sqrtPriceLimitX96: tokenIn < tokenOut ? MIN_SQRT_RATIO + 1 : MAX_SQRT_RATIO - 1, // "sin límite"
        data: abi.encode(tokenIn, tokenOut, fee, minOut)
    });
}

function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata data) external {
    (address tokenIn, address tokenOut, uint24 fee,) = abi.decode(data, (address, address, uint24, uint256));
    if (msg.sender != IUniswapV3Factory(UNIV3_FACTORY).getPool(tokenIn, tokenOut, fee)) revert NotPool();
    // El delta POSITIVO indica el token debido a la pool: token0 si amount0Delta>0, token1 si no.
    // token0/token1 se ordenan por dirección, NO por input/output del swap: pagar con "tokenIn"
    // un amount0Delta cuando tokenIn > tokenOut transfiere el token equivocado.
    (address token0, address token1) = tokenIn < tokenOut ? (tokenIn, tokenOut) : (tokenOut, tokenIn);
    if (amount0Delta > 0) IERC20(token0).safeTransfer(msg.sender, uint256(amount0Delta));
    else if (amount1Delta > 0) IERC20(token1).safeTransfer(msg.sender, uint256(amount1Delta));
    // el último hop del ciclo debe además verificar minOut aquí (o en execute): revertir burbujea y aborta todo
}
```

`MIN_SQRT_RATIO`/`MAX_SQRT_RATIO` son las constantes de TickMath (4295128740 y 1461446703485210103287273052203988822378723970341); ±1 porque el límite es exclusivo. Para Uniswap V2 el callback análogo es `uniswapV2Call(sender, amount0, amount1, data)` con fee 0.3% implícito en el par: mismo patrón de dispatcher, validando contra la Pair derivada por `CREATE2` de la factory.

**Dispatcher**: un solo contrato puede implementar los tres callbacks; cada uno con su guard de `msg.sender` y, en Aave, el check de `initiator`. Prohibido: lógica de negocio accesible sin guard (funciones `external` que muevan tokens fuera del ciclo), porque convierten el contrato en superficie de ataque ejecutable por cualquiera.

## 14.3 Gas golfing con costes verificados

Costes vigentes post-Cancun (EIP-2929 acceso frío/caliente desde Berlin, EIP-3529 reembolsos desde London, EIP-1153 storage transitorio desde Cancun). Verifícalos siempre con `forge test --gas-report` y `forge snapshot`; nunca optimices sin medición.

| Operación | Gas | Nota |
|---|---|---|
| SLOAD frío | 2 100 | primera lectura del slot en la tx |
| SLOAD caliente | 100 | lecturas 2..N del mismo slot |
| SSTORE 0 → ≠0 | 20 000 | escribir un slot nuevo (ej. `executedRoutes[hash] = true`) |
| SSTORE ≠0 → ≠0 | 2 900 (+2 100 si frío) | primera escritura en slot frío = 5 000 |
| SSTORE ≠0 → 0 | 2 900 + reembolso 4 800 | reset a cero |
| TSTORE / TLOAD | 100 | EIP-1153 (Cancun): storage transitorio, se borra al final de la tx |
| Cuenta fría (CALL, EXTCODESIZE, EXTCODECOPY, BALANCE a otra cuenta) | 2 600 | primer acceso al contrato — SSTORE siempre es a storage propio |
| Cuenta caliente | 100 | |
| CREATE | 32 000 base + 200/byte de código | |
| LOG | 375 + 375/topic + 8/byte | cada `indexed` extra cuesta |
| calldata | 16/byte ≠0, 4/byte =0 | el packing también abarata la tx (data gas) |
| Transacción | 21 000 base + calldata | |

Palancas ordenadas por ROI:

1. **`immutable`/`constant`**: `operator`, `treasury`, direcciones de pool/factory — van embebidas en el runtime bytecode; acceso ~0 gas (push de código), cero SLOAD. Todo lo conocido en deploy y constante de por vida debe ser `immutable`.
2. **Caching de storage en stack**: `uint256 x = slotVar;` y operar sobre `x`; cada reuso ahorra 100 gas warm. Y para state mutable entre llamadas, `_paused` leído una vez.
3. **TSTORE para locks**: un reentrancy-guard con SSTORE cuesta 2 900/20 000 por ciclo; con EIP-1153 (`assembly` `tstore`/`tload` desde solc 0.8.24; OZ 5 expone `ReentrancyGuardTransient`) cuesta 100. En el hot path del executor es la diferencia visible.
4. **Custom errors vs `require(string)`**: ahorra bytecode de deploy (200 gas/byte) y decodifica tipado off-chain; el ahorro runtime es acotado — no elijas arquitectura por esto.
5. **`unchecked { ... }` con prueba de límites**: contadores de loops con cota superior probada (`for (uint256 i; i < hops.length; ++i)` con `hops.length <= 5`): ~80 gas por iteración. Jamás en aritmética de montos sin cota.
6. **calldata vs memory**: decodificar `bytes calldata` directo y evitar copias `memory`; en callbacks Balancer (`memory` por interfaz) el decoding ya es inevitable — en las APIs propias usa `calldata`.
7. **Loops cortos**: 2-5 hops máximo (núcleo §1.2 ya acota el detector a 2-5); cada hop extra multiplica costes fríos de pools nuevas y ensancha la ventana de sandwich contra el bundle.
8. **Calldata packing de rutas**: budget de bits por hop:

| Campo | Bits | Rango/semántica |
|---|---|---|
| pool (address) | 160 | dirección de la pool/router del hop |
| zeroForOne | 1 | dirección del swap |
| feeTierIdx | 2 | índice 0-3 → 100/500/3000/10000 |
| shareBps | 16 | fracción del input para este hop |
| minOutBps | 16 | slippage relativo del hop |
| **total** | **195 → 25 B/hop** | vs ≥128 B/hop con `abi.encode` (32 B por campo ABI) |

El packing abarata dos veces: gas de calldata de la tx (16 gas/byte ≠0) y el copiado en `abi.decode`. Contrapartida: el decodificado es manual (Yul — §14.4) y más difícil de auditar; se justifica sólo en el hot path medido. Nota: los 195 bits son el mínimo teórico; el decodificador de §14.4 alinea a byte (200 bits = 25 B exactos) para que las máscaras caigan en fronteras de byte — mismo costo de calldata.

## 14.4 Yul inline assembly: casos justificados y memory-safety

Casos legítimos (los tres aparecen en librerías auditadas como OZ `Address`):

```solidity
// (a) Burbujear el revert original de una subllamada (patrón interno de OZ Address.functionCall)
assembly ("memory-safe") {
    if iszero(success) {
        returndatacopy(0, 0, returndatasize())
        revert(0, returndatasize())
    }
}

// (b) Slice no alineado de calldata empaquetada (hop de 25 bytes; calldataload lee
//     32 bytes desde cualquier offset, los bits sobrantes se enmascaran al decodificar).
//     Layout byte-aligned del hop: [0..159] pool | [160..167] flags (zeroForOne +
//     feeTierIdx + pad) | [168..183] shareBps | [184..199] minOutBps = 200 bits = 25 B
uint256 word;
assembly { word := calldataload(add(hops.offset, mul(idx, 25))) }
address pool = address(uint160(word));
uint8 flags  = uint8(word >> 160);
uint16 bps   = uint16(word >> 168);
```

Otro caso razonable: leer el selector con `shr(224, calldataload(0))` en routers custom — aunque `msg.sig` ya lo hace en Solidity puro, así que casi nunca aplica. El revert manual con selector de custom error (`mstore` + `revert(0, 0x04)`) NO aporta en 0.8.x: el compilador ya emite custom errors eficientemente; sólo se justifica para reverts dinámicos construidos en runtime.

**Riesgos memory-safety**: el modelo de memoria de Solidity asume el free-memory pointer en `0x40`; un bloque assembly que escribe por encima de `0x40` sin actualizar el puntero, o que hace `msize`-dependencias, rompe los invariantes del optimizer y puede corromper structs en memoria de forma silenciosa. La anotación `assembly ("memory-safe")` (solc ≥ 0.8.13) promete al optimizer que tu bloque respeta el modelo (sólo escribe memoria derivada de `mload`/allocaciones propias); si mientes en la anotación, el miscompile es tuyo. **Cuándo NO usar Yul**: fuera del hot path medido (ahorros < 100 gas no justifican superficie de auditoría), cuando exista API Solidity equivalente (0.8.x ya cubre returndata con try/catch y `Address.functionCall`), y siempre que el auditor no pueda verificar el bloque a simple vista — el assembly no tipado es donde viven los bugs caros.

## 14.5 Approvals: permit, Permit2 y approve exacto vs infinito

- **EIP-2612 `permit`**: `permit(owner, spender, value, deadline, v, r, s)` sobre `DOMAIN_SEPARATOR` + `nonces(owner)`. Permite aprobar con firma off-chain y consumir la aprobación on-chain en la MISMA tx del swap (spread permit+transferFrom en una sola llamada): ahorra una tx al operador y reduce la ventana de allowance expuesto a cero bloques. Limitación: adopción por token (USDT mainnet NO lo implementa).
- **Permit2** (Uniswap, despliegue canónico `0x000000000022D473030F116dDEE9F6B43aC78BA3`, misma dirección multi-chain): interposito universal. `permit(owner, permitSingle, signature)` + `transferFrom(from, to, amount, token)`; consulta de estado con `allowance(user, token, spender) → (amount, expiration, nonce)`. Estructuras reales: `PermitDetails{token, amount(uint160), expiration(uint48), nonce(uint48)}` dentro de `PermitSingle{details, spender, sigDeadline}`; firma EIP-712 con domain name `Permit2`. Permite allowances con `expiration` corto y por monto exacto sin pagar una tx de approve por token — encaja con la doctrina de mínima exposición.
- **Approve exacto vs infinito**: infinito (`approve(spender, type(uint256).max)`) ahorra ~2 900 gas warm por ejecución y evita los 20 000 del 0→≠0 inicial; el costo es que el spender puede drenar en cualquier bloque futuro (historial de exploits por approvals huérfanas es abundante). Con flash loans el principal se aprueba en el mismo ciclo (approve puntual a la pool), por lo que el único approval persistente que el ejecutor necesita es ~ninguno: prefiere approve exacto por ejecución o Permit2 con expiración. Si operas con approve persistente: revocación automática al rotar operador y monitoreo de allowances (revoke.cash como referencia operativa).
- **Quirk USDT**: su `approve` no retorna `bool` y exige pasar por `approve(spender, 0)` antes de cambiar un allowance no-cero — `SafeERC20` (OZ) maneja el retorno ausente; la secuencia 0→valor debe hacerla tu flujo. En OZ 5, `SafeERC20.forceApprove` existe para este caso.
- **Fee-on-transfer y rebasing**: PROHIBIDO interactuar sin screening previo (defer a la skill `arbx-token-safety-screen`). Con fee-on-transfer el `balanceAfter < amount` esperado rompe cualquier contabilidad por montos; con rebasing, el balance muta sin tx y un delta check entre bloques miente. La regla del ejecutor: contabilidad SIEMPRE por delta de balance dentro de la misma tx (robusto a shortfall: simplemente revierte si el delta no cubre la deuda), y estos tokens fuera del universo salvo screening explícito.

## 14.6 Multicall/batching atómico y dust

**Multicall3** (despliegue canónico `0xcA11bde05977b3631167028862bE2a173976CA11`, misma dirección en decenas de chains):

```solidity
interface IMulticall3 {
    struct Call3 { address target; bool allowFailure; bytes callData; }
    struct Result { bool success; bytes returnData; }
    function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory returnData);
}
```

Con `allowFailure = false` en TODAS las llamadas, cualquier fallo revierte el batch completo — semántica atómica idéntica a la del ciclo in-contract, pero reutilizando un despliegue verificado en vez de lógica propia. Usos: (a) componer approval + swap + sweep de rescate de dust en una tx; (b) ejecutar pasos preparatorios (Permit2 permit + transferFrom + execute) sin ventanas intermedias. La atomicidad del BUNDLE (off-chain, núcleo §8.1) NO requiere Multicall3 — el bundle ya es atómico por block-builder — pero Multicall3 garantiza atomicidad también cuando la tx cae a mempool público (último recurso del núcleo §1.2).

**Dust**: toda ruta cíclica deja restos sub-mínimos de tokens intermedios. Política: (1) el contrato no debe acumularlos — value-stuck prohibido; (2) en el mismo `execute`, si un residual supera un umbral `dustBps` del input, se incluye como hop de sweep o se registra; (3) `rescue` ADMIN-only es la válvula para lo irreparable, con evento auditable. El invariante 14.10 verifica que `balance(executor) <= dust` tras cada handler.

## 14.7 MEV-resistencia del contrato

Enmarcado defensivo: las medidas siguientes protegen TU ejecución contra sandwich/frontrun de terceros. El uso ofensivo de estas técnicas contra otros está fuera de alcance y gobernado por `arbx-mev-ethics-gate`.

- **minOut/minProfit derivados de simulación**: cada hop lleva su `minOutBps` empaquetado y el ciclo su `minProfit` — ambos calculados off-chain contra el estado simulado (núcleo §1.2, revm/anvil) con el slippage del riesgo engine. Un minOut hardcode (RULE 00) es un bug de doble filo: demasiado bajo = regalas el sandwich; demasiado alto = la ruta revierte siempre y quemas gas del bundle.
- **Deadline**: `block.timestamp > deadline → revert`. La manipulación del timestamp por el builder es de segundos, no de bloques; con deadline de 1-2 bloques una inclusión tardía (bundle re-target o reorg) revierte en vez de ejecutar contra precios movidos.
- **Whitelist de caller**: `msg.sender == operator` (o `onlyRole(EXECUTOR_ROLE)` en la variante AccessControl del núcleo §2.1). Sin whitelist, cualquiera puede forzar `execute` en el peor estado visible (aunque con flash loans el atacante paga el gas del revert, la firma forzada consume tu nonce y contamina métricas/kill-switch).
- **Pausa de emergencia**: `_paused` (o OZ `PausableUpgradeable.pause()` con `__Pausable_init()` en la variante UUPS) — un SLOAD de 100 gas; ligada al kill-switch del control-plane: `emergencyPause` del núcleo §2.1 y el bot deben converger al mismo estado, con evento indexable para que el dashboard lo refleje.
- **Rescue withdraw ADMIN-only**: `rescue(token)` con `onlyRole(ADMIN_ROLE)`, evento `Rescued`, y en producción detrás de TimelockController — el rescate es la única salida legítima de fondos y debe ser ruidoso.
- **Prohibición de value-stuck**: `receive() revert` + ningún `payable` en el flujo normal; ningún código que deje tokens del ciclo sin barrer al final de `execute`.
- **Privacidad de ruta**: el bundle privado (Flashbots/MEV-Share, núcleo §8.1) es la defensa primaria contra copia de ruta; el contrato añade la segunda capa (minOut/deadline) para cuando la tx es visible.

## 14.8 Upgradeabilidad segura (UUPS)

- **`_authorizeUpgrade` gated**: en la implementación, `function _authorizeUpgrade(address newImplementation) internal override onlyRole(UPGRADER_ROLE)` — y el holder de `UPGRADER_ROLE` debe ser un `TimelockController` (OZ) propuesto por multisig (Safe 2/3), nunca una EOA del operador. El upgrade es el mayor riesgo de custody del sistema completo: la nueva implementación puede contener cualquier lógica sobre el storage heredado.
- **Storage layout append-only**: nunca reordenar, borrar ni cambiar tipos de variables existentes; sólo añadir al final, consumiendo el `__gap` correspondiente (`uint256[n] private __gap;` reducido en el espacio usado por las nuevas vars). Verificación mecánica en CI: `forge inspect ExecutorV2 storageLayout --json` y diff contra `ExecutorV1` — cualquier cambio en slot/offset de variables pre-existentes rompe el upgrade.
- **Initializers guarded**: constructor de la implementación con `_disableInitializers()` (evita el "uninitialized implementation" takeover); nuevas variables inicializadas con `reinitializer(n)` (n = versión), nunca reusando `initializer`.
- **Proxy ≠ storage de conocimiento sensible**: la clave del operador NUNCA vive en el contrato; el contrato sólo conoce addresses (`operator`, `treasury`) y umbrales numéricos — consistente con la política de §33.2 (ningún MCP con `PRIVATE_KEY`, Foundry siempre `PRIVATE_KEY=""`).

## 14.9 EIP-7702, EIP-4844 y EOF

**EIP-7702** (activo desde Pectra, mainnet 2025): transacciones type `0x04` con `authorization_list` de tuplas `(chain_id, nonce, address, y_parity, r, s)`; la EOA delegada ejecuta con el código del contrato designado (designación `0xef0100 || address`). Impacto defensivo en nuestros flows:
- `extcodesize(addr) > 0` YA NO implica "es contrato": una EOA 7702 tiene designación. `extcodesize == 0` tampoco implica EOA pura transaccionalmente. Toda heurística on-chain basada en "EOA vs contract" (incluido `tx.origin == msg.sender`) queda rota — usar roles/allowlists, que ya es nuestra práctica (§14.7).
- Firmas: el "owner" de una firma puede ser una EOA con código delegado; los flujos `permit`/Permit2 siguen válidos (la firma sigue siendo de la EOA), pero el análisis de replay debe considerar `chain_id = 0` en la autorización (válido cross-chain).
- Operativa: batch nativo (una firma, N calls vía delegación) reduce txs de gestión del operador, pero introduce un nuevo artefacto que rotar/revocar (la delegación persiste hasta revoke).

**EIP-4844** (activo desde Dencun, 2024; EIP-7691 elevó target/max blobs a 6/9 en Pectra): transacciones type `0x03` con sidecar de blobs, mercado de blob fee INDEPENDIENTE del gas market (consulta `eth_blobBaseFee` en el RPC; cada blob consume 131 072 de blob-gas; los blobs son inaccesibles a la EVM salvo su versioned hash vía opcode `BLOBHASH` y se podan tras ~18 días). Impacto: en L2 rollups, el fee por tx = costo DA (blobs) + ejecución L2 — la componente blob domina en horas valle y colapsa el costo de calldata pesada. Para rutas cross-L2 o pricing de opportunities en L2, el modelo de fees del evaluador (núcleo §1.2) debe leer el blob base fee en vez de extrapolar gas L1. El bundle de arbitrage mismo NO usa blobs (los blobs no son estado ejecutable): 4844 nos afecta por costos de L2, no por el vehículo de envío.

**EOF** (EVM Object Format, familia EIP-3540/663/7620): CANDIDATO, no hecho — fue retirado del alcance de Osaka y se evalúa como fork separado. Si activa: formato contenedor con código/datos separados, nuevas instrucciones de creación/retorno, y los opcodes de introspección legacy (`CODECOPY`/`EXTCODECOPY` sobre contratos EOF) dejan de comportarse igual — afectaría tooling de deploy (solc `--eof-version`, forge) y análisis de bytecode (heimdall disasm). NO diseñar nada contra EOF hoy; registrar como watch-item y re-verificar estado del hardfork antes de cualquier dependencia.

## 14.10 Invariantes formales del contrato y tests Foundry

| Invariante | Cómo se rompe | Test Foundry que lo protege |
|---|---|---|
| Solvencia post-ruta: `delta >= deuda + minProfit + gasBuffer` | pool maliciosa devuelve menos de lo simulado (short return) | fuzz `test_execute_revertsOnShortReturn` con mock pool que recorta 1 wei; `vm.expectRevert(Underwater.selector)` |
| No-custodia: `balance(executor, token) <= dust` tras cada `execute` | swap parcial deja residual de token intermedio | `invariant_noStuckBalances`: handler ejecuta rutas random; assert balances <= dust en cada paso |
| Reentrancy: callbacks no pueden reentrar `execute` | callback malicioso re-llama durante `receiveFlashLoan` | mock recipient que reintenta; con TSTORE guard: `vm.expectRevert`; invariant de profundidad (`reentrancyDepth == 0` entre handlers) |
| Pausa: `paused == true` ⇒ toda ejecución revierte | alguien ejecuta durante kill-switch | setUp pausa; `vm.prank(operator); vm.expectRevert(Paused.selector)` |
| Whitelist: caller sin rol/operador ⇒ revert | EOA arbitraria fuerza `execute` en estado adversario | `vm.prank(attacker); vm.expectRevert(NotOperator.selector)` |
| Idempotencia de ruta (si se habilita `executedRoutes`) | replay de la misma calldata en otro bloque | ejecutar 2× el mismo bundle; segundo `vm.expectRevert` |
| Dispatcher: callback desde dirección ≠ pool esperada ⇒ revert | tercero llama `executeOperation`/`uniswapV3SwapCallback` directamente | `vm.prank(attacker)` sobre cada callback; `vm.expectRevert(NotPool.selector)` |
| Upgrade gated: upgrade desde no-timelock revierte | atacante llama `upgradeToAndCall` en el proxy | `vm.prank(attacker); vm.expectRevert` en proxy UUPS |
| No value-stuck: `receive()` siempre revierte | alguien envía ETH para atrapar fondos | `vm.deal(attacker, 1 ether); (bool ok,) = addr.call{value: 1}(""); assertFalse(ok)` |

Esqueleto de invariantes (Foundry):

```solidity
contract ExecutorInvariants is Test {
    Handler handler;

    function setUp() public {
        // deploy executor + tokens mock + pools mock del universo screening
        handler = new Handler();
        targetContract(address(handler));       // handlers en vez de fuzzing directo al contrato
    }

    function invariant_noStuckBalances() public view {
        assertLe(handler.numeraire().balanceOf(address(handler.executor())), handler.dustTolerance());
    }
}

contract Handler is Test {
    function runRoute(uint256 seed) external {
        seed = bound(seed, 0, handler_routes_len - 1); // bound() de forge-std acota el input
        executor.execute(hops[seed], amountIn, minProfit, gasBuffer, block.timestamp + 24, routeHash[seed]);
    }
}
```

Configuración en `foundry.toml`:

```toml
[invariant]
runs = 128
depth = 64
fail_on_revert = true   # un revert del handler ES un hallazgo, no ruido
```

Cheatcodes útiles del flujo: `vm.prank` (caller forjado), `vm.expectRevert(bytes4(...))` (selector de custom error), `vm.warp`/`vm.roll` (deadline/bloque), `deal` (stdcheats: fundear tokens/ETH), `makeAddr` (identidades de prueba). Complemento de gates: los invariantes corren en CI junto al diff de `storageLayout` (§14.8) — ambos son bloqueantes para cualquier PR que toque `contracts/`.

## 14.11 Ciclo de despliegue inicial: del bytecode al proxy gobernado

§14.8 cubre el upgrade UUPS, pero el despliegue inicial quedaba implícito. Esta sección es la secuencia canónica de primera instalación del ejecutor en una red: script de deploy, determinismo de addresses, hardening de roles con timelock, verificación en explorador y promoción testnet→mainnet. En este repo el ciclo es planeamiento/scaffold: §32-§33 mantienen el modo audit/scaffold/shadow/read-only (Foundry con `PRIVATE_KEY=""`), así que el script se valida en simulación o fork y el broadcast real pertenece a la promoción gated del operador (§14.11.6).

### 14.11.1 Script de despliegue Foundry (patrón Deploy.s.sol)

El artefacto es implementación UUPS + proxy ERC-1967; `initialize` NO se llama a mano después del deploy: viaja como `_data` del constructor del proxy y se ejecuta como delegatecall durante la creación. En la OZ 5.6 pineada, el constructor `ERC1967Proxy(address implementation, bytes memory _data)` revierte con `ERC1967ProxyUninitialized` si `_data` está vacío — el proxy NUNCA nace sin inicializar, que es exactamente el invariante que queremos. (El guard es sobreescrible vía `_unsafeAllowUninitialized()` — nunca lo sobreescribas: un proxy sin inicializar es candidato a man-in-the-middle, como advierte el propio fuente de OZ.)

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script} from "forge-std/Script.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {Executor} from "src/Executor.sol";

contract DeployExecutor is Script {
    function run() external returns (address proxy) {
        vm.startBroadcast(); // cuenta de --account/--sender; impl + proxy en el MISMO broadcast

        Executor impl = new Executor(); // su constructor hace _disableInitializers() (§14.8)
        bytes memory initData = abi.encodeCall(Executor.initialize, ());
        proxy = address(new ERC1967Proxy(address(impl), initData)); // delegatecall a initialize en el constructor

        vm.stopBroadcast();
    }
}
```

Ejecución: `forge script script/DeployExecutor.s.sol --rpc-url $RPC_URL --broadcast --verify`. `--verify` es un flag del propio script — verifica todo lo despachado en el run contra el explorador con la API key del entorno (`ETHERSCAN_API_KEY` o `--etherscan-api-key`), no un paso manual posterior. Sin `--broadcast`, el script corre en simulación local efímera: es el modo de validar la secuencia en este repo (§33.2, `arbx-simulation-mandatory`). Prohibido que el deploy script haga más que desplegar (grants de roles, unpause, funding): un script = una responsabilidad.

### 14.11.2 Determinismo: CREATE2 vs CREATE del EOA

- **CREATE2**: `address = keccak256(0xff ‖ factory ‖ salt ‖ keccak256(initCode))[12:]`. Misma address en todas las chains SI y sólo SI coinciden factory, salt e initcode; `chainId` NO entra en la derivación. Un byte de initcode distinto (código, versión de solc, optimizer, flags) cambia la address.
- **EOA + CREATE** (el `new Executor()` de §14.11.1): `address = keccak256(rlp(deployer, nonce))[12:]` — nonce-dependiente. Los nonces divergen entre chains con la primera tx incidental: NO es determinista cross-chain, y sincronizar nonces a mano es frágil por diseño.
- **Herramientas reales**: `Create2.deploy(uint256 amount, bytes32 salt, bytes memory bytecode)` de OZ (`utils/Create2.sol`, firma verificada en la 5.6 pineada) para factorías propias; el proxy canónico de deterministic-deployment (`0x4e59b44847b379578588920cA78FbF26c0B4956C`) es el patrón que logró la address única multi-chain de Multicall3 (§14.6) y Permit2 (§14.5) — pero ese proxy DEBE existir ya en la chain destino (no está en todas: en chains nuevas se replica con su despliegue canónico ya difundido antes de reutilizarlo). En tests, el cheatcode `vm.computeCreate2Address(bytes32 salt, bytes32 initCodeHash)` (y su overload `vm.computeCreate2Address(bytes32 salt, bytes32 initCodeHash, address deployer)`, ambos verificados en el `Vm` de forge-std) precalcula la address desde salt + hash de initcode.
- **Cuándo importa**: integraciones que registran o hardcodean la address del ejecutor (config del bot/control-plane), setup multi-chain donde una sola address simplifica config y monitoreo, y precomputación de destino (fondos enviados a la address antes de desplegar). En un solo chain no aporta nada: CREATE normal basta.

### 14.11.3 Hardening post-despliegue (secuencia obligatoria)

Tras `initialize` (núcleo §2), `DEFAULT_ADMIN_ROLE` y `ADMIN_ROLE` caen en el EOA deployer y `EXECUTOR_ROLE` no lo tiene nadie. La secuencia de transferencia:

1. **Verificar el slot de implementación EIP-1967**: `cast storage $PROXY 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc` (forma legible: `cast implementation $PROXY --rpc-url $RPC`) debe devolver exactamente la impl desplegada. Si no coincide: ALTO — el proxy delega en otra cosa.
2. **Confirmar roles contra el archivo** (núcleo §2): `DEFAULT_ADMIN_ROLE` (bytes32(0)) ≠ `ADMIN_ROLE` (`keccak256("ADMIN_ROLE")`) ≠ `EXECUTOR_ROLE` (`keccak256("EXECUTOR_ROLE")`); check con `cast call $PROXY "hasRole(bytes32,address)(bool)" $(cast keccak "ADMIN_ROLE") $DEPLOYER`. En el esqueleto del núcleo `_authorizeUpgrade` es `ADMIN_ROLE`-gated; §14.8 exige que el holder efectivo sea timelock+multisig, nunca una EOA.
3. **Transferir admin a timelock**: desplegar `TimelockController(uint256 minDelay, address[] proposers, address[] executors, address admin)` (firma verificada en la OZ pineada) y `grantRole` de `DEFAULT_ADMIN_ROLE` y `ADMIN_ROLE` al timelock ANTES de revocar nada. Revocar `DEFAULT_ADMIN_ROLE` sin haberlo transferido primero es brick sin recuperación.
4. **Scheduling vía timelock**: todo cambio de gobernanza futuro sigue `schedule(target, value, payload, predecessor, salt, delay)` (queue) → esperar `delay >= minDelay` → `execute(target, value, payload, predecessor, salt)`; el id de operación es el hash de esos campos (idempotente, cancelable por proposers). El mismo schedule/revert se prueba en invariantes (§14.10, upgrade gated).
5. **Revocar el EOA deployer** de TODO rol sensible y verificar `hasRole == false` uno por uno. El EOA no queda ni admin, ni upgrader, ni executor: sin llaves de rey en hot-wallet.

### 14.11.4 Verificación en explorador

- **Implementación**: `forge verify-contract $IMPL src/Executor.sol:Executor --chain <chain>`; el constructor de la impl no toma argumentos (todo el estado va por `initialize`), así que no lleva `--constructor-args`; si una futura versión los tomara, los flags reales (verificados contra el `--help` del forge 1.7.2 del repo) son `--constructor-args <args...>` (forge los codifica según los tipos del constructor), `--constructor-args-path <file>` y `--guess-constructor-args` — `--constructor-args-by-name` NO existe en esta versión: es un flag apócrifo. El polling del GUID se hace con `forge verify-check <id>`.
- **Proxy**: el `ERC1967Proxy` puede verificarse como contrato propio (con forge: `--constructor-args $IMPL 0x<hex-de-initData>`), pero lo que realmente quieres es la proxy-verification del explorador: Etherscan expone "Verify Proxy Contract" (endpoint `module=contract&action=verifyproxycontract`, con la dirección de implementación esperada en el cuerpo) que liga el proxy a la fuente verificada de la impl para decodificar reads/writes/txs con el ABI real; Blockscout (explorador de §33) soporta el mismo patrón. forge NO tiene verificación de proxy integrada (verificado en el 1.7.2 del repo: `forge verify-contract --help` no expone ningún flag para ello — su `--no-proxy` es de proxy HTTP del sistema, ajeno a esto): la ligadura se hace llamando el endpoint `verifyproxycontract` del explorador (curl) o desde su UI. Nunca inventar flags (RULE 00).

### 14.11.5 Checklist de promoción de entorno (testnet→mainnet)

MISMA secuencia: mismo `Deploy.s.sol`, mismo salt (con CREATE2 → misma address en ambas redes, §14.11.2), misma secuencia de roles/timelock de §14.11.3, mismos checks post-deploy. Diferencias que SIEMPRE cambian — jamás se copian de test:
- RPC (`--rpc-url`) y chainId.
- Wallets: EOAs/Safe de test vs los de producción; nunca reutilizar la clave del deployer de test en mainnet.
- Allowances: nacen en cero en cada red — re-aprobar (§14.5); los approvals de test no migran.
- Direcciones de protocolos externos: Aave provider, Balancer Vault, factories de Uniswap difieren por red o no existen — resolver por red en el script (RULE 00, §14.2).
- Parámetros económicos: gasPrice/gasBuffer del evaluador, umbrales del risk engine (núcleo §1.2).

El estado on-chain (`executedRoutes`, `paused`, umbrales) nace limpio vía `initialize`: no existe "copiar estado" de test a mainnet. La promoción completa del stack (infra, smoke, SLOs) es la secuencia de ref. 22 §22.1-§22.2.

### 14.11.6 El despliegue NO habilita trading

Deploy ≠ activación. `executeArbitrage` es `onlyRole(EXECUTOR_ROLE)` (núcleo §2) y el `initialize` del despliegue NO otorga `EXECUTOR_ROLE` a nadie: el contrato nace inerte para ejecución — sin caller autorizado, con la pausa disponible como segundo enclavamiento (§14.7) y `rescue`/upgrade en manos del timelock (§14.8, §14.11.3). El grant de `EXECUTOR_ROLE` al bot y el unpause son un paso separado del operador, subordinado a CLAUDE.md §34 (terminus `relays-client` default-deny por env; mainnet soportada vía `ARBX_LIVE_EXEC_ENABLED=true` + `ARBX_LIVE_EXEC_CHAINS=1` — la variante `MainnetRefused` nunca existió, orden 2026-09-17) y a los gates `arbx-*`; §34.5 exige los gates con evidencia reproducible ANTES de tocar el terminus. Regla de PR: un deploy script que otorgue `EXECUTOR_ROLE` o haga unpause "de paso" se rechaza (§37 P-∅: un PR = un ID).

## GOBERNANZA

Todo lo anterior está subordinado a los gates `arbx-*` (`arbx-paper-trade-first`, `arbx-simulation-mandatory`, `arbx-risk-limits-enforcement`, `arbx-pre-execute-checklist`) y a CLAUDE.md §34: LIVE_MAINNET es gated, el terminus `relays-client` es default-deny, y ningún patrón de esta referencia autoriza por sí mismo un flip a live ni broadcast con capital real. Despliegue y firma de contratos siguen en modo audit/scaffold/shadow/read-only (§32-§33); desplegar el ejecutor NO habilita trading (§14.11.6); Foundry siempre con `PRIVATE_KEY=""`.
