# FICHA REA-4 — Ejecución atómica, repago guardado y errores nombrados (simple-blind-arbitrage + aave-v3-core)

Fecha: 2026-10-23 · Autor: rea-analista-a (t4, attempt 8822d516) · Alimenta: PR5 ATOMIC-CAUSE-01 (t10, worktree `feat/atomic-cause-01`)
Skills cargadas: `arbx-reverse-engineering-fusile`, `arbx-flash-loan-discipline` (las 7 reglas se citan como R1–R7).

## 0. Fuentes fijadas (sha, jamás main) + licencias + estado allowlist (tal cual el gate)

| Fuente | SHA fijado | Fecha commit | Licencia EN el sha | rea_adopt (veredicto literal) |
|---|---|---|---|---|
| `flashbots/simple-blind-arbitrage` | `73c6c270b27059e6dc0469a6c08e0f7739e5d531` | 2023-09-07 (merge PR#7 "add-flashloan" = tip de `main`) | **MIT** (`LICENSE`, Copyright (c) 2023 Flashbots) | **BLOQUEADO** `fuera_de_allowlist`; clase permisiva, `admite_verbatim: si` con atribución. Enmienda registrada: **`CONFIRMED: allowlist amendment flashbots/simple-blind-arbitrage`** |
| `aave/aave-v3-core` | `782f51917056a53a2c228701058a6c3fb233684a` | 2024-09-03 ("chore: deprecate for 3.1 origin" = último estado v3.0, pin estable) | `LICENSE.md` = **BUSL-1.1** (Change Date 27-ene-2023 YA vencido, Change License MIT); SPDX headers MIXTOS al sha: `AGPL-3.0` en `flashloan/base/*`, `BUSL-1.1` en `mocks/*` | **BLOQUEADO** `fuera_de_allowlist`; clase copyleft fuerte de red, `modo_permitido: solo_vectores_numericos` (verbatim NO aunque el change date haya vencido — el gate clasifica y manda). Enmienda registrada: **`CONFIRMED: allowlist amendment aave/aave-v3-core`** |

Clones de lectura: `C:\Users\HFRC\Desktop\repos\rea-sources\{simple-blind-arbitrage,aave-v3-core}`. Consecuencia para PR5: se portan **semántica + números** (siempre libres); **cero verbatim** de cualquiera de las dos fuentes sin enmienda firmada por el operador.

## 1. Contrato primero

### 1.1 simple-blind-arbitrage@73c6c270

```solidity
// src/BlindBackrunFlashLoan.sol:8-36 [CONTRATO] — callback Balancer V2 (NO Aave)
interface IVault { function flashLoan(IFlashLoanRecipient, IERC20[] tokens, uint256[] amounts, bytes userData) external; }
interface IFlashLoanRecipient {
    // docstring del upstream (L21-23) [INVARIANTE textual]: "Before this call returns, the recipient
    // must have transferred `amounts` back or else the entire flash loan will revert."
    function receiveFlashLoan(IERC20[] tokens, uint256[] amounts, uint256[] feeAmounts, bytes userData) external;
}
// src/BlindBackrunLogic.sol:15-22 [CONTRATO/struct de segunda pata]
struct PairReserves { uint256 reserve0; uint256 reserve1; uint256 price; bool isWETHZero; }
// src/BlindBackrun.sol:8-17 [CONTRATO] — entry `executeArbitrage(firstPair, secondPair, pct) external onlyOwner`
```

### 1.2 aave-v3-core@782f5191

```solidity
// contracts/interfaces/IPool.sol [INVARIANTE textual]: "Allows smartcontracts to access the liquidity
// of the pool within one transaction, as long as the amount taken plus a fee is returned."
function flashLoanSimple(address receiverAddress, address asset, uint256 amount, bytes params, uint16 referralCode) external;
// contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol:13-20 [CONTRATO]
abstract contract FlashLoanSimpleReceiverBase is IFlashLoanSimpleReceiver {
  IPoolAddressesProvider public immutable ADDRESSES_PROVIDER;
  IPool public immutable override POOL;                 // [invariante R2: POOL immutabile, desde provider]
  constructor(IPoolAddressesProvider provider) { ... POOL = IPool(provider.getPool()); }
}
// mocks/flashloan/MockSimpleFlashLoanReceiver.sol:20-27 [BORDE/test-knobs del upstream]
function setFailExecutionTransfer(bool); function setAmountToApprove(uint256); function setSimulateEOA(bool);
```

## 2. Ejecución atómica de dos pools — `_executeArbitrage` (BlindBackrunLogic.sol:39-110), línea por línea

- **L44** `balanceBefore = WETH.balanceOf(address(this))` [FORMULA/orden: snapshot ANTES de mover nada].
- **L51-52** `getPairData` por pool: reservas + `isWETHZero = (token0 == WETH)` [contrato de segunda pata: la DIRECCIÓN vive en el orden de tokens, igual que FICHA REA-1 §6].
- **L58** `amountIn = getAmountIn(firstPairData, secondPairData)` — **forma cerrada on-chain** (§2.1) [FORMULA].
- **L62** `WETH.transfer(firstPairAddress, amountIn)` — fondea la pata 1 directamente al POOL [orden].
- **L66-80 (isWETHZero=true)**: `firstPair.swap(0, out1, secondPairAddress, "")` → la pata 1 entrega los tokens **DIRECTAMENTE a la dirección del pool 2** (chaining, sin custody intermedia); `secondPair.swap(out2, 0, address(this), "")` cierra a WETH [invariante: o ambas swaps o revert total]. Rama L81-97 espejo con amount0Out/amount1Out invertidos.
- **L99-100** `balanceAfter > balanceBefore` si no → `require(..., "Arbitrage failed")` **[INVARIANTE del repago guardado: el profit-floor se chequea con balance REAL post-swaps, ANTES de cualquier payout y (en la versión flash) ANTES del repay]**. Cumple R5 en espíritu (floor = 1 wei de ganancia; sin `minProfitFloor` configurable — gap).
- **L105-109** `WETH.withdraw(profit)` → ETH nativo; `profitToCoinbase = profit * pct / 100`; `block.coinbase.transfer(profitToCoinbase)`; `payable(tx.origin).transfer(profitToCaller)` [CONVENCIÓN-descartada: payout nativo + **tx.origin** — antipatrón R/lista de la skill; el destino paga en el propio asset y a config del operador].

### 2.1 Forma cerrada `getAmountIn` (L116-206) — segundo testigo para FICHA REA-2/t3

```
isWETHZero=false:                                       isWETHZero=true (espejo 0↔1):
presqrt     = 997²·rA0·rB1 / rB0 / rA1                  presqrt     = 997²·rA1·rB0 / rB1 / rA0
numerator   = (isqrt(presqrt) − 1000)·rB0·rA1           numerator   = (isqrt(presqrt) − 1000)·rB1·rA0
denominator = 997·rB0·1000 + 997²·rA0                   denominator = 997·rB1·1000 + 997²·rA1
amountIn    = numerator·1000 / denominator              (idem)
```
`isqrt` = Babilónico entero (L233-242); `getAmountOut` = V2 estándar fee 997/1000 floor (L249-258, idéntico a FICHA REA-1 §4.1). **[BORDE crítico]: si `isqrt(presqrt) ≤ 1000` (mercado no suficientemente cruzado), `(isqrt − 1000)` underflowea en Solidity 0.8 → `Panic(0x11)` ANÓNIMO** — el propio upstream produce el revert sin causa que ATOMIC-CAUSE-01 existe para eliminar. El destino debe chequear `presqrt ≥ 1000²` (o `isqrt ≥ 1000`) y revertir con error nombrado por causa ANTES de la resta.

### 2.2 `receiveFlashLoan` (BlindBackrunFlashLoan.sol:54-86) — el callback

- **L61-64** `require(msg.sender == address(vault), "FlashLoanRecipient: caller is not the vault")` [cumple R2 con string, no custom error; `vault` es `constant` con **address hardcodeada** — G7: NO viaja, el destino la recibe de config].
- **L67-71** decode `userData = (firstPair, secondPair, pct)`; **L75-79** `_executeArbitrage(secondPairAddress, firstPairAddress, pct)` — [ORDEN: en la versión flash la pata 1 es `secondPair`; invertirlo rompe la dirección de los swaps].
- **L82-85** repay = `WETH.transfer(address(vault), amounts[0])` — **PUSH** del principal, **IGNORA `feeAmounts`** (el propio comentario L73-74/L81 lo declara: se rompe si Balancer cobra fee) [viola R6: la fee no se lee del argumento; el destino DEBE usar `premiums[i]`/`feeAmounts[0]`].
- **Sin validación de initiator ni flag transient** (Balancer no pasa initiator) [viola R3: el port exige flag transient de sesión per skill].

## 3. Patrón de repago guardado — Aave (FlashLoanLogic.sol:182-269), línea por línea

- **L186-188** [INVARIANTE de orden, textual]: el flujo se ALTERA a *validation → user payload → cache → updateState → changeState → updateRates* "to protect against reentrance and rate manipulation within the user specified payload".
- **L190** `ValidationLogic.validateFlashloanSimple`: `RESERVE_PAUSED` / `RESERVE_INACTIVE` / `FLASHLOAN_DISABLED` ANTES de mover fondos (ValidationLogic.sol) [qué chequea antes de revertir: estado de la reserva].
- **L193** `totalPremium = amount.percentMul(params.flashLoanPremiumTotal)` [cumple R6: premium del parámetro del pool, no literal].
- **L194** `transferUnderlyingTo(receiver, amount)` — fondos primero.
- **L196-205** `require(receiver.executeOperation(...), Errors.INVALID_FLASHLOAN_EXECUTOR_RETURN)` — callback que no devuelve `true` → revert nombrado '13' [invariante del contrato del callback].
- **L226-252** `_handleFlashLoanRepayment`: `amountPlusPremium = amount + totalPremium`; repago **PULL**: `IERC20(asset).safeTransferFrom(receiver, aToken, amountPlusPremium)` — si el receiver no aprobó o su balance < amount+premium, `safeTransferFrom` revierte y **toda la tx revierte** (atomicidad por construcción del pull) [cumple R1].
- **L260-268** `emit FlashLoan(...)` recién al final [orden: evento sólo tras el repago].
- Mocks adversariales del upstream (MockSimpleFlashLoanReceiver): knobs `setAmountToApprove(<deuda)` y `setFailExecutionTransfer(true)` = el diseño del test negativo de atomicidad que PR5 debe espejar.

## 4. Errores nombrados por causa — comparativa de los tres estilos

| Estilo | Fuente | Ejemplo | Qué cruza la cadena | Veredicto para el destino |
|---|---|---|---|---|
| `require` con string | blind-arb | `"Arbitrage failed"` (L100), `"FlashLoanRecipient: caller is not the vault"` (L63), `"External call failed"` (L268), OZ 4.x `"Ownable: caller is not the owner"` | `Error(string)` selector `0x08c379a0` + ABI-encoded string | Mensaje legible pero SIN argumentos estructurados; el decode depende del string |
| string constantes numéricas | aave-v3-core | `INVALID_FLASHLOAN_EXECUTOR_RETURN = '13'` (Errors.sol L22), `'19'` premium inválido (L28), `'49'` params inconsistentes (L57), `'91'` flashloan disabled (L99) | `Error(string)` con "13" — compacto, causa nombrada SOLO en fuente | Causa nombrada barata en gas, opaca on-chain; exige tabla de decodificación |
| custom errors con args | **destino** (ArbitrageExecutor.sol L37-108, catálogo FASE1_BASE §2.5) | `SwapFailed` L50, `InsufficientProfit` L67, `AliasedTwoLegRoute` L108 | selector de 4 bytes + args ABI | **El que ATOMIC-CAUSE-01 conserva y extiende**: args estructurados decodificables por el sim/bindings |

Selectores verificados con `cast keccak` (foundry local): `Error(string)` = `0x08c379a0afcc32b1…` → selector `0x08c379a0`; `Panic(uint256)` = `0x4e487b71539e0164…` → `0x4e487b71` (underflow 0x11); `SwapFailed()` = `0x81ceff30f5d23d82…` → `0x81ceff30`.

**El patrón anónimo existe EN el upstream**: `BlindBackrunLogic.call()` L266-268 hace `(bool success, ) = _to.call{value:_value}(_data); require(success, "External call failed")` — **el returndata se descarta**, exactamente igual que `ArbitrageExecutor.sol:508-513` y `FlashLoanExecutor.sol:392/419` del destino. La causa raíz del callee muere dentro del wrapper. ATOMIC-CAUSE-01 = capturar ese returndata y bubbling con causa nombrada.

## 5. Vectores

### 5.1 Reverts esperados del upstream (con mensaje/selector)

| Trigger | Revert | Fuente ruta:linea | Tipo |
|---|---|---|---|
| caller ≠ owner en entry | `'Ownable: caller is not the owner'` — **asertado por el test** (`vm.expectRevert`, test L269) | BlindBackrun.sol:15 + blindBackrunMainnet.t.sol:268-272 | `Error(string)` `0x08c379a0` |
| profit ≤ 0 (balanceAfter ≤ balanceBefore) | `"Arbitrage failed"` | BlindBackrunLogic.sol:99-100 | `Error(string)` |
| callback invocado por no-vault | `"FlashLoanRecipient: caller is not the vault"` | BlindBackrunFlashLoan.sol:61-64 | `Error(string)` |
| owner-rescue call falla | `"External call failed"` (returndata descartado — el anti-patrón) | BlindBackrunLogic.sol:266-268 | `Error(string)` |
| callback Aave no devuelve true | `'13'` INVALID_FLASHLOAN_EXECUTOR_RETURN | FlashLoanLogic.sol:196-205 + Errors.sol:22 | `Error(string)` |
| receiver no aprueba/balance < amount+premium | revert del `safeTransferFrom` (ERC20/allowance) — tx completa revierte | FlashLoanLogic.sol:248-252 | `Error(string)` del token |
| mercado no cruzado: `isqrt(presqrt) ≤ 1000` | **`Panic(0x11)` anónimo** por underflow de `(isqrt − 1000)` | BlindBackrunLogic.sol:148/166 | `Panic(uint256)` `0x4e487b71` |
| reserva paused/inactive/flashloan-disabled | `'91'` / RESERVE_PAUSED / RESERVE_INACTIVE | ValidationLogic.sol (validateFlashLoanSimple) | `Error(string)` |

### 5.2 Vector numérico DERIVADO (el upstream lo loguea, no lo aserta — marcado DERIVADO)

Inputs = reservas fake del test (blindBackrunMainnet.t.sol L278-317): first `{r0: 17221979511711000000000, r1: 9022829950419911882261}`, second `{r0: 24221870080988000000000, r1: 29260889455340067009671}` (y su flipped). Re-implementación BigInt de §2.1 — **artefacto reproducible: `docs/rea/vectors_rea4_verify.mjs`**; salida observable de la corrida (el exit 0 solo dice que no falló, el efecto son estas líneas): `ok presqrt rama false = 2291978`, `ok isqrt(presqrt) = 1513`, `ok amountIn rama false (DERIVADO) = 2716778998396236163624`, `ok amountIn rama true (DERIVADO, simetria) = 2716778998396236163624`, `ok out1 pata 1 (DERIVADO) = 3976312652586`, `ok out2 pata 2 (DERIVADO) = 4115530729036751327899`, `ok profit = out2 - amountIn (DERIVADO) = 1398751730640515164275`. Nota de método: la primera versión del script generalizó las dos ramas con un solo mapeo de índices y FALLÓ (8 verificaciones en rojo); la transcripción literal por rama del fuente pasó — evidencia de que las ramas NO son un espejo simétrico de índices y de que el test falsa la lectura.

| Campo | Valor | Nota |
|---|---|---|
| `presqrt` (ambas ramas) | `2291978` | mismo mercado visto de los dos lados |
| `isqrt(presqrt)` | `1513` | > 1000 ⇒ no underflow en este dataset |
| `amountIn` (ambas ramas) | **`2716778998396236163624`** | vector DERIVADO (upstream: console.log L72/L79 sin assert) |
| trace flipped: out1 | `3976312652586` | getAmountOut pata 1 |
| trace flipped: out2 | `4115530729036751327899` | getAmountOut pata 2 |
| profit = out2 − amountIn | `1398751730640515164275` | > 0 ⇒ pasaría el require L100 |

**Simetría verificada**: ambas ramas de §2.1 (isWETHZero true/false con las reservas espejo del test) producen el MISMO `amountIn` — invariante de consistencia que el port de la forma cerrada (t3/t7) debe testear.

## 6. Mapa al destino (ATOMIC-CAUSE-01, sobre FASE1_BASE §2.4-2.5)

| Semántica del fuente | Destino | Acción PR5 |
|---|---|---|
| `(bool success, ) = _to.call(...)` descarta returndata (BlindBackrunLogic L266-268) | `ArbitrageExecutor.sol:508-513` `_boundedRouterCall` y `FlashLoanExecutor.sol:392-393/419-420` | mismo anti-patrón: capturar returndata y re-revertir con causa (`SwapFailed(bytes returndata)` o decode de `Error(string)`/custom error del callee) |
| require `balanceAfter > balanceBefore` con balance real ANTES de payouts (L99-100) | `InsufficientProfit` L67 ya existe | nombrar causa con args (actual, required) — el piso viene de config, no literal (R5) |
| `Panic(0x11)` anónimo del underflow `(isqrt − 1000)` (L148/166) | sizing on-chain si PORTA la forma cerrada | chequear `presqrt ≥ 1000²` ANTES de restar y revertir `NoTradableDislocation(presqrt)` (nombre a definir por t10) |
| invariante textual Balancer "must have transferred amounts back or else the entire flash loan will revert" (L21-23) + aave PULL `safeTransferFrom` (L248-252) | FlashLoanExecutor repago guardado existente | invariante INTACTA (o ambas patas o ninguna); lo que cambia es la VISIBILIDAD de la causa al fallar |
| callback guard `msg.sender == vault` string (L61-64) | guard existente del executor | mantener guard; si se renombra la causa, con custom error con args (caller) |
| knobs negativos del mock aave (`setAmountToApprove` < deuda) | tests forge del destino | PR5 debe testear: revert con returndata preservado, profit < floor con args correctos, callback falso caller, y (si porta forma cerrada) el borde underflow |
| catálogo Errors.sol numérico ('13','19','49','91') | NO se copia el estilo | referencia de "causa nombrada por fallo"; el destino ya tiene custom errors con args |

Bindings: si PR5 agrega decode de returndata, el catálogo a decodificar es ArbitrageExecutor.sol L37-108 + `Error(string)`/`Panic(uint256)` (selectores verificados §4) y los encoders están en `execute_arbitrage_encoder.rs` L55/62/69 (FASE1_BASE §2.5).

## 7. Qué se descarta a propósito

- **Address del Vault Balancer hardcodeada** (BlindBackrunFlashLoan.sol:39-40) y TODAS las addresses del test (routers/fábricas/WETH/USDC/USDT mainnet, test L43-55) — G7: dato del upstream, jamás config del destino.
- Payout en ETH nativo + `tx.origin` (L105-109) — antipatrón; el destino paga en asset/config del operador.
- `console.log` de foundry en producción (L45-104) — telemetría del destino.
- Estilo string/numérico de errores — el destino conserva custom errors con args.
- Código verbatim de AMBAS fuentes (gates §0): blind-arb sin enmienda, aave solo_vectores_numericos siempre según clasificación del gate.
- `execute/` JS del upstream (bundleExecutor.js, index.js) — fuera de alcance de ATOMIC-CAUSE-01.

## 8. Pendientes / no computado (fail-honest)

- La suite forge del upstream NO se ejecutó (test mainnet-fork + submodules; el vector asertado L269 se transcribe sin correr). Los vectores DERIVADOS de §5.2 se computaron con re-implementación BigInt independiente (sandbox node, exit 0), no con forge.
- Selector `0x118cdaa7` de `OwnableUnauthorizedAccount(address)` computado (cast) pero es del OZ MODERNO: el upstream usa OZ 4.x string; se lista como referencia del estilo nuevo, no como vector del sha.
- Los tests de pool del destino (`contracts/test/ArbitrageExecutor*.t.sol`) no se leyeron en esta ficha — son materia de t10 al implementar, no del análisis de fuente.
- BUSL change-date ya vencido: reportado como hecho de la licencia; la clasificación copyleft del gate PREVALECE sobre esa lectura (skill §0).
