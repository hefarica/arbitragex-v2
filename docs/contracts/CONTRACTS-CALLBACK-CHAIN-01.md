# CONTRACTS-CALLBACK-CHAIN-01 — F25/F26/F27/F28 medidos sobre bytes anclados

**Carril:** Contracts (contratos, encoders y ejecución protocolaria) · perfil 2.0 · 2026-10-04
**Rev auditado:** `bceb31ef7c0fd6d4b50bcdee6fc8d33858334822` · corte previo `3f00b359…`
**Estado del carril:** `REQUIERE_CAMBIOS` — el fragmento **no** puede darse por cerrado.
**Entregables:** oráculo ejecutable, encoder verificado, patch aplicable, matriz adversarial especificada, plan de despliegue/recuperación con permisos separados.

---

## 0. Procedencia y qué NO es esta evidencia

El worktree local está en `fix/perhop-reserves-01` @ `858b943f`, con **970 rutas rastreadas modificadas, 322 sin seguimiento y 43 worktrees**. No es fuente de evidencia. Todo lo medido aquí viene de **bytes anclados al rev auditado**: `docs/contracts/anchored/` contiene esos bytes y el oráculo **rechaza** el análisis si el sha1 del blob git (`sha1("blob <len>\0" + bytes)`) no coincide con el del rev.

| Verdad | Medición |
|---|---|
| 7/7 archivos anclados VERIFIED | `node docs/contracts/CONTRACTS-ORACLE-01.mjs --repo .` §[1] |
| keccak verificado vs 3 vectores publicados + selector externo `0xf04f2707` | íd. §[2], `keccak/failure count = 0` |
| 8/8 mutantes invierten su sonda | íd. §[4] |
| `ORACLE VALID: anchors=true keccak=true mutants=true`, exit 0 | íd., salida completa |

**Fronteras declaradas (fail-honest):**

- `forge.exe` está **bloqueado por Windows AppControl** en este host (`Una directiva de Control de aplicaciones bloqueó este archivo`, el os error 4551 documentado en `CLAUDE.md` §36). Por eso **no hay** compilación, unit, fuzz, invariantes ni fork en este carril. Están **especificados** (§5) y **NO ejecutados**.
- No hubo red, firma, broadcast ni capital. El inventario de comandos de la sesión es: `workspace_diagnostico`, `git rev-parse/cat-file/ls-tree/archive/hash-object`, copia de archivos, `node <oráculo>`, `node <fixgen>`, `node <fixcheck>`, `forge --version` (bloqueado). Ninguno de ellos firma ni transmite.
- Un `PASS` de este carril significa **el constructo está presente en los bytes**. No significa que el bytecode desplegado se haya ejecutado.

---

## 1. La cadena compuesta: F25 → F26 → F27 son UN solo defecto

El informe entrante los lista como tres P0 independientes. Medidos sobre los bytes, **son eslabones de una misma cadena con un camino de custodia concreto**:

```text
atacante (sin rol, sin capital)
  └─ llama vault.flashLoan(address(FlashLoanExecutor), [asset], [amount], userData_arbitrario)
     sobre el Balancer V2 Vault REAL
        │
        ├─ el Vault transfiere `amount` de `asset` al wrapper y llama
        │  receiveFlashLoan(tokens, amounts, fees, userData_arbitrario)
        │     ├─ Layer 1 balancerVault != 0        PASS
        │     ├─ Layer 2 msg.sender == balancerVault PASS  ← ES el vault real
        │     ├─ Layer 3 flashLoanProvider != 0    PASS
        │     ├─ nonReentrant                      PASS
        │     └─ ✗ NO EXISTE Layer 4: nada liga este callback a una solicitud nuestra
        │
        ├─ asset.forceApprove(arbitrageExecutor, amount)      ← aprobación viva, tamaño del préstamo
        ├─ arbitrageExecutor.call(userData_arbitrario)        ← calldata elegido por el atacante
        │     └─ ArbitrageExecutor lo autentica como si fuéramos nosotros (onlyExecutor)
        │
        └─ if (asset.balanceOf(address(this)) < amountOwed) revert;   ← BALANCE TOTAL, no delta
              └─ si la reserva preexistente cubre el hueco, NO revierte: el atacante pagó
                 su repago con NUESTRO dinero                              [F26]
                 └─ y esa reserva existe porque el wrapper retiene el spread
                    sin ninguna función de salida ERC-20                  [F27]
```

### 1.1 F25 — binding de solicitud del callback Balancer: **CONFIRMADO**

| Sonda | Resultado | Artefacto |
|---|---|---|
| C4 — 15 etiquetas de binding dentro del cuerpo de `receiveFlashLoan` (`FlashLoanExecutor.sol:341-378`, 1987 chars) | `found=[none]` | oráculo fila C4 |
| C4b — ¿el canal ABI puede transportar un binding? | `IFlashLoanProvider.flashLoan(receiver, asset, amount, params)` y el Vault `flashLoan(recipient, address[], uint256[], bytes)` **no llevan** initiator ni sesión; el único portador es `params`/`userData`, que nunca se registra ni compara | blob `16850392…` |
| C4c — lado **productor** | 6 slots de storage, **0 de tipo `bytes32`**; `keccak256(params)` aparece 1× y **solo** como argumento del evento `FlashLoanRequested` (un evento no es estado legible) | oráculo fila C4c |
| M2 / M8 — ¿las sondas pueden fallar al revés? | añadir un binding transitorio ⇒ C4 `ABSENT→PASS`; hacer que `requestFlashLoan` registre la identidad ⇒ C4c `ABSENT→PASS` | oráculo §[4] |

**Corrección al informe entrante.** Dice «Sin cambios Solidity relevantes» para F25. Es impreciso: en el rev **sí** hay un cambio Solidity relevante a la autenticación del callback — el guard `nonReentrant` con nota `WEB3-04 fix (2026-09-24)`. Está presente y es correcto; simplemente **no cubre** el vector de llamada no solicitada, porque el Vault real puede invocarlo legítimamente.

**Alcance honesto del impacto.** El atacante **no** puede robar por un selector whitelisteado: `ArbitrageExecutor` exige `onlyExecutor`, `approvedTokens`, `approvedRouters`, `approvedSelectors` (4 bytes) y revierte con `ZeroGrossProfit` si el balance de `tokenIn` no sube (líneas 260/361-384/442/445/479). El primitivo real es: **llamada forzada a `ArbitrageExecutor` con una aprobación del tamaño del préstamo viva y calldata elegido por un tercero**, más el gasto de reserva que habilita F26. No lo inflo a «drenaje arbitrario».

### 1.2 F26 — saldo previo expuesto al repago: **CONFIRMADO**

```solidity
// FlashLoanExecutor.sol:374  (Balancer)
if (asset.balanceOf(address(this)) < amountOwed) revert FL_RepaymentShortfall();
// FlashLoanExecutor.sol:400  (Aave)
if (IERC20(asset).balanceOf(address(this)) < amountToOwe) revert FL_RepaymentShortfall();
```

El predicado es de **solvencia total**, no del delta de la operación. Una ruta con `gross < premium` **no revierte** si la reserva cubre el hueco: el coste lo paga el capital propio.

**El delta correcto ya existe una capa más abajo** — `ArbitrageExecutor` lo hace bien (`balBeforePull`, líneas 319-323 y la identidad de retención 339). Es decir: el invariante está implementado en el executor y **no** se aplica en el wrapper que lo envuelve.

**Agravante de cobertura (fila C15 del oráculo):** el único test que espera `FL_RepaymentShortfall` (`FlashLoanRoundTrip.t.sol:352`) arranca el wrapper en **cero**; y `FlashLoanExecutor.t.sol:122` **acuña 10.000e18 DENTRO del wrapper** en `setUp` («Mint tokens to flash loan executor to cover repayment»). Ninguna prueba ejercita el camino de shortfall **con reserva preexistente** ⇒ la suite verde no detecta el defecto.

### 1.3 F27 — beneficio retenido sin salida a tesorería: **CONFIRMADO**

- Camino de custodia: Vault → wrapper → `ArbitrageExecutor.executeArbitrageFlashFunded` devuelve `amountIn + profit` al wrapper → el wrapper repaga `amountOwed` al Vault → **el spread queda en el wrapper**.
- La propia suite lo declara: `FlashLoanRoundTrip.t.sol:219` → `assertEq(token.balanceOf(address(flashExec)), profit, "net profit retained by the borrower")`.
- Superficie del wrapper: `initialize, setReferralCode, setFlashLoanProvider, setBalancerVault, selectCheapestProvider, requestFlashLoan, executeOperation, receiveFlashLoan, _authorizeUpgrade` — **ninguna** función de salida ERC-20. `ArbitrageExecutor` **sí** tiene `emergencyWithdraw` (línea 594) y `withdrawETH`.
- No hay rol ni dirección de tesorería en el wrapper (`WalletTopology.COLD_TREASURY_ROLE` es otro contrato y no está cableado aquí).

**F27 no es cosmético: es lo que fabrica la reserva que F26 consume.**

### 1.4 F28 — pausa on-chain inmediata: **PARCIAL, con una contradicción medible**

| Hecho | Artefacto |
|---|---|
| `ArbitrageExecutor` **es** pausable (`whenNotPaused` en ambas entradas, `pause()` en línea 601) | `ArbitrageExecutor.sol:260/313/601` |
| `pause()` es `onlyRole(ADMIN_ROLE)`, **no** un guardián acotado | línea 601 |
| El wrapper **no** es pausable: `requestFlashLoan` y ambos callbacks no llevan `whenNotPaused` | fila C12 (M6 la invierte) |
| El retardo del timelock es **parámetro de constructor**, no literal (`AdminTimelock.sol` rechaza delay 0) ⇒ latencia de pausa en producción = **NO COMPUTADO**, no cero | línea del guard `AdminTimelock__ZeroMinDelay` |
| `AdminTimelock.t.sol:175` `testA9_Timelock_AdminCannotBypassDelay` **afirma que el admin NO puede saltarse el retardo** | test citado |

Conclusión: por la **única** vía de pausa existente, «pausa on-chain inmediata» **no es alcanzable** — y el repo tiene un test que exige exactamente que no lo sea. Se necesita un rol guardián acotado (FIX E).

### 1.5 Hallazgo nuevo de mi carril: 3 tests cuyo nombre no corresponde a su cuerpo (clase F16)

`FlashLoanExecutor.t.sol` — tests cuyo **nombre** reclama el callback Balancer pero cuyo **cuerpo** conduce `executeOperation` (el de Aave):

| Test | Línea | Qué conduce realmente |
|---|---|---|
| `testReceiveFlashLoan_Authorized` | 130 | `flashExec.executeOperation(...)` |
| `testReceiveFlashLoan_RejectsUnauthorized` | 183 | `flashExec.executeOperation(...)` |
| `testReceiveFlashLoan_RejectsInvalidInitiator` | 205 | `flashExec.executeOperation(...)` |

Balancer **no expone `initiator`**: un test llamado «RejectsInvalidInitiator» en la ruta Balancer describe cobertura que esa ruta **no puede tener**. Y la política del verificador exige **casos obligatorios por nombre** (F16): un nombre que miente es exactamente el falso verde que F16 pretendía cerrar.

*Nota de rigor:* la sonda resolvió indirección de profundidad 1 (`mockVault.triggerFlashLoan` es cómo los tests A9 sí alcanzan el callback) y descartó un cuarto falso positivo. El límite de profundidad está impreso en la evidencia y es auditable; el mutante M7 renombra los 3 y la sonda pasa a `PASS`.

**Lo que los tests A4 SÍ cubren** (y por eso las capas 1-2 pasan): «el sender no es el vault configurado» y «vault/provider sin setear». **Ningún** test conduce el **vault genuino configurado con `userData` del atacante** — que es justamente por qué F25 sobrevive a una suite verde.

### 1.6 Observación adicional (no es vulnerabilidad, es límite de capacidad)

`receiveFlashLoan` lee solo `tokens[0]/amounts[0]/feeAmounts[0]` y repaga únicamente ese par; el test `testReceiveFlashLoan_MultiElementArrayProcessesOnlyFirst` lo fija. Para una solicitud Balancer multi-token, el Vault exige el repago de **todas** las entradas antes de retornar ⇒ la tx revierte completa. **Seguro por revert**, sin pérdida de fondos, pero el wrapper anuncia una forma (arrays) que no puede servir. Documentado como límite, no como hallazgo de seguridad.

### 1.7 Linaje: qué ya estaba reportado (y qué es nuevo aquí)

Existe un handoff previo en este mismo directorio, `docs/contracts/CONTRACTS-FLASHLOAN-HANDOFF.md` (v1.0.0, 10-05, **no escrito por esta sesión**), cuyo **base es el worktree** `fix/perhop-reserves-01` @ `858b943f`. **No reclamo novedad sobre F25 y F26**: ese handoff ya los reportó.

| Este documento | Handoff previo | Delta declarado |
|---|---|---|
| F25 (sin binding de solicitud) | **C2 — BAJA (defense-in-depth)**: «`receiveFlashLoan` sin *session flag* transitorio» | **Subo la severidad a P0** con la composición: C2 no es defensa-en-profundidad, es el hueco de **autorización** que habilita una llamada forzada con aprobación viva; y F26 (su C1) es el combustible que la hace costar fondos. Aislados parecen MEDIA/BAJA; compuestos son una cadena. |
| F26 (saldo previo expuesto) | **C1 — MEDIA (doctrinal)**: «el callback no preserva saldo previo ante premium > profit» | Confirmado y ahora **anclado por hash** al rev auditado, con el **delta correcto ya presente una capa abajo** (`ArbitrageExecutor.balBeforePull`) como precedente interno, y con la **brecha de cobertura** medida (fila C15: el `setUp` acuña 10.000e18 dentro del wrapper y el único test de shortfall arranca en cero). |
| F27 (sin salida a tesorería) | no lo reporta | **Nuevo**: es el eslabón que **fabrica** la reserva que C1/F26 consume. |
| F28 (pausa) | no lo cuantifica | **Nuevo**: la única vía de pausa es admin-only y el repo tiene un test que **exige** que el admin no pueda saltarse el retardo ⇒ «inmediata» es inalcanzable sin rol acotado. |
| Arreglo + patch + oráculo + fixtures | el previo recomienda «next action for Planner» | **Nuevo**: 5 fixes, patch de 13 hunks que aplica byte-exacto sobre el blob auditado, oráculo con 8 mutantes, encoder verificado. |
| Base | worktree `858b943f` (sucio) | **Delta de base**: aquí es el blob del rev `bceb31ef`. El defecto existe en **ambos**; su lectura y la mía se corroboran, y la mía sobrevive a `git gc` del worktree. |

También corrobora su **C3 (adapters SKELETON)**: `BalancerFlashAdapter.sol` lleva `TODO(M12, audit 2026-05-10): wire this adapter into ArbitrageExecutor … is NOT yet invoked from the hot path`, y sus tests son `testTODO_UniV3FlashAdapter_NotImplemented` / `testTODO_DyDxFlashAdapter_NotImplemented`. Eso acota F39: la capacidad multi-proveedor está declarada, **no** homologada en el camino caliente.

---

## 2. El arreglo (5 correcciones, patch aplicable generado)

`docs/contracts/PROPOSED-callback-binding-01.patch` (272 líneas, cabeceras `a/contracts/src/...`, `index 96be2783..ea4daff7`) se **genera mecánicamente** desde los bytes anclados con `node docs/contracts/CONTRACTS-FIXGEN-01.mjs --repo .`: **16/16 ediciones**, cada una exigiendo **exactamente 1 coincidencia** antes de aplicar (una edición ambigua aborta sin escribir salida).

| Fix | Hallazgo | Qué hace | Cómo se comprueba sin compilador |
|---|---|---|---|
| **A** | F25 | `bytes32 private _pendingRequestHash` (append-only, tras `_reentrancyStatus`); `requestFlashLoan` graba `keccak256(abi.encode(chainid, this, provider, pool, asset, amount, keccak256(params)))` **antes** de salir y lo limpia después; ambos callbacks recomputan y comparan (**Layer 4**) y consumen. Sin `initiator`, la sesión transitoria es lo que la disciplina exige. | C4 y C4c `ABSENT→PASS` |
| **B** | F26 | Baseline `balBeforeCallback` **antes** de llamar al executor; el gate pasa a `balanceOf < balBeforeCallback + owed (+ floor)`. La reserva preexistente deja de poder cubrir una ruta perdedora; es la misma identidad delta que `ArbitrageExecutor` ya aplicaba. | C10 `FAIL→PASS` |
| **C** | F26 (regla FL-5) | `minProfitFloor` de operador + `setMinProfitFloor` + `FL_BelowProfitFloor`. Un suelo de ganancia on-chain **antes** del repago, que es lo que exige la disciplina. | C7 `ABSENT→PASS` |
| **D** | F27 | `treasury` de operador + `setTreasury` + `sweepProfit(asset, amount)` `onlyRole(DEFAULT_ADMIN_ROLE)` (en producción = timelock) + evento `ProfitSwept`. El rol de ejecución **no** puede mover fondos. | C11 `ABSENT→PASS` |
| **E** | F28 | `PausableUpgradeable` + `GUARDIAN_ROLE` acotado: `pause()` guardián, `unpause()` **admin**. `whenNotPaused` en `requestFlashLoan` (frena exposición nueva); los callbacks quedan llamables para que un préstamo ya emitido siempre pueda liquidar o revertir dentro de su propia tx. Almacenamiento namespaced ERC-7201 ⇒ un proxy ya inicializado lee «no pausado» y **no requiere re-inicialización**. | C12 `ABSENT→PASS` |

**Cross-check del arreglo:** `node docs/contracts/CONTRACTS-FIXCHECK-01.mjs --repo .` ⇒ **6/6 sondas marcadas pasan a PASS**.

**Lo que ese cross-check NO prueba (y hay que decir en la misma línea):** es un *cross-check por regex*, no un compilador ni una ejecución. **No** prueba que compile, **no** prueba que `test/StorageLayout.t.sol` siga clavando los slots 0..3, **no** prueba comportamiento bajo fuzz. Ese es el gate de forge, y forge está bloqueado en este host.

Comandos del gate real, a correr donde forge funcione:

```bash
cd contracts
forge build
forge test --match-contract 'FlashLoan(Executor|RoundTrip|Invariant)' -vvv
forge test --match-contract StorageLayout -vvv        # los slots 0..3 deben seguir clavados
forge test --match-contract ExecutorInvariant -vvv    # invariantes con presupuesto 256
forge test --match-contract FlashFundedFork --fork-url "$MAINNET_RPC_URL"
```

---

## 3. Contratos de API, encoders y ABIs (handoff a Backend/Integration)

Selectores **calculados** con el keccak propio del oráculo, cuya correctitud está probada contra 3 vectores publicados y contra el selector canónico externo de Balancer `receiveFlashLoan = 0xf04f2707`:

| Capacidad | Firma | Selector |
|---|---|---|
| Callback Balancer | `receiveFlashLoan(address[],uint256[],uint256[],bytes)` | `0xf04f2707` |
| Callback Aave (simple) | `executeOperation(address,uint256,uint256,address,bytes)` | `0x1b11d0ff` |
| Entrypoint proveedor | `flashLoan(address,address,uint256,bytes)` | `0x5cffe9de` |
| Entrypoint wrapper | `requestFlashLoan(address,uint256,bytes)` | `0x5107d61e` |

El oráculo emite además un ejemplo de calldata `receiveFlashLoan` real (388 bytes) con los offsets de cabeza verificados (`off_tokens=0x80`). **Identidad extremo a extremo exigida al consumidor:** el digest de solicitud liga `chainid + wrapper + provider + pool + asset + amount + keccak256(params)`; cualquier cambio de red, destinatario, activo, monto o payload **invalida** el binding por construcción. Cambiar la implementación detrás del proxy entre cotización y ejecución **no** invalida el digest (es address-based), así que el consumidor debe añadir su propia comprobación de implementación — declarado como pendiente.

---

## 4. Matriz adversarial a añadir (ESPECIFICADA, NO EJECUTADA)

Cada fila es una **regresión** que debe existir antes de dar el fix por bueno. Ninguna está ejecutada en este carril (forge bloqueado).

| # | Escenario | Resultado exigido | Estado hoy |
|---|---|---|---|
| R1 | `vault.flashLoan(executor, [asset], [amt], dataAtacante)` sobre el Vault real, sin solicitud en vuelo | revert `FL_RequestMismatch` | ausente ⇒ F25 |
| R2 | Igual que R1 pero **con** solicitud en vuelo y `userData` distinto | revert `FL_RequestMismatch` | ausente |
| R3 | Callback genuino de la solicitud legítima | `FlashLoanExecuted` + repago exacto | cubierto |
| R4 | Caller ≠ vault, vault sin setear, provider sin setear | `FL_UnauthorizedCaller` / `FL_BalancerVaultNotSet` / `FL_NoProviderConfigured` | cubierto (A4) |
| R5 | **Wrapper con reserva preexistente** + ruta con `gross < premium` | revert `FL_BelowProfitFloor`; **reserva intacta** | ausente ⇒ F26 |
| R6 | Wrapper con reserva + ruta con `gross == premium`, `minProfitFloor=0` | revierte (delta no cubre) | ausente |
| R7 | Reentrada desde token no estándar durante el callback | revert `FL_ReentrantCall`; binding consumido una sola vez | parcial |
| R8 | Doble callback en la misma tx (segundo intento tras consumir el digest) | revert `FL_RequestMismatch` | ausente |
| R9 | Fee del proveedor bumpeada por gobernanza | repago por el valor **del argumento**, nunca literal | cubierto (C8) |
| R10 | Token con `decimals` distintos y activos especiales (fee-on-transfer, rebasing) | rechazo fail-closed; sin consumo de reserva | parcial (executor sí, wrapper no) |
| R11 | `pause()` por guardián ⇒ `requestFlashLoan` revierte; `unpause()` por no-admin revierte | ambos reverts | ausente ⇒ F28 |
| R12 | Cambio de `implementation` detrás del proxy entre cotización y ejecución | invalidación declarada por el consumidor | ausente |
| R13 | Balancer multi-token (`tokens.length > 1`) | revert del Vault, sin pérdida | documentado §1.6 |

---

## 5. Plan de despliegue y recuperación con permisos separados

> **Frontera de autoridad:** este plan se **diseña** aquí. Firma y broadcast son acto del operador con alcance operativo autorizado (§34.3/§34.5). Nada de este documento concede capacidad de firma, broadcast ni manejo de capital a los agentes.

### 5.1 Permisos separados (invariante de despliegue)

| Rol | Quién | Puede | NO puede |
|---|---|---|---|
| `DEFAULT_ADMIN_ROLE` | **timelock** (post-handoff) | `setBalancerVault`, `setFlashLoanProvider`, `setReferralCode`, `setTreasury`, `setMinProfitFloor`, `unpause`, `setGuardian` | ejecutar rutas |
| `UPGRADER_ROLE` | timelock | `upgradeToAndCall` | mover fondos, ejecutar |
| `GUARDIAN_ROLE` (FIX E) | clave caliente de monitoreo | **solo `pause()`** | unpause, upgrade, sweep, ejecutar |
| `EXECUTOR_ROLE` | relays-client | `requestFlashLoan` | admin, upgrade, sweep, pause |

`DeployMainnetRoleCustody.t.sol` ya acredita que el deployer **no** retiene admin ni upgrader y que el timelock sí: el plan **conserva** ese contrato y sólo añade GUARDIAN como cuarto rol acotado.

### 5.2 Secuencia (reanudable, con verificación por paso)

1. **Pre**: `forge build` + suite completa verde **en el host con forge** (este carril no puede).
2. **Upgrade** (si aplica): `upgradeToAndCall(impl_fixed, "")` desde el timelock. Sin re-inicialización (Pausable usa slot namespaced; verificar con `test/StorageLayout.t.sol` que los slots 0..3 siguen clavados).
3. **Verificación post-upgrade** (sólo lectura): `_pendingRequestHash` inexistente en ABI ⇒ confirmar por selector/ABI que las funciones nuevas existen; `paused() == false`; `treasury == address(0)`; `minProfitFloor == 0`.
4. **Configuración**: `setTreasury(<multisig>)` → `setMinProfitFloor(<política>)` → `setGuardian(<hot key>)` → `setBalancerVault(<vault de la cadena>)` → `setFlashLoanProvider(<adapter>)`.
5. **Gate de activación**: aprobar `(router, selector)` en `ArbitrageExecutor.batchSetRouterSelectorApproval` — **sin esto TODA ruta revierte** con `AE_RouterSelectorNotApproved` (documentado en `setRouterSelectorApproval` y `DEPLOY.md §A5`). Es el paso que más se olvida.
6. **Ensayo**: R1/R5/R8 en **fork**, sin overrides de admisión, y declarado como laboratorio (§ el fork con estado inyectado no acredita producción).
7. **Reconciliación**: el evento `ProfitSwept` alimenta Ledger; `FlashLoanExecuted` no es beneficio.

### 5.3 Recuperación

| Situación | Acción | Límite honesto |
|---|---|---|
| Callback no solicitado observado | `pause()` por guardián (inmediato, sin timelock) | no revierte txs ya confirmadas |
| Ruta perdedora | `minProfitFloor > 0` la corta antes del repago | si ya ejecutó, el capital es irrecuperable |
| Beneficio atrapado | `sweepProfit(asset, amount)` desde el timelock | no puede tocar lo que la tx en curso debe |
| Regresión del upgrade | `upgradeToAndCall(impl_anterior, "")` desde el timelock | **un rollback técnico no deshace efectos on-chain ya finalizados**; conservar journal y conciliar |
| Kill-switch off-chain | ya probado (F32) | no sustituye la pausa on-chain |

---

## 6. Handoffs

| Consumidor | Qué recibe | Qué debe verificar |
|---|---|---|
| **Security** | 4 capas de auth vs 3 actuales; superficie de `GUARDIAN_ROLE` | que el guardián no pueda escalar a unpause/upgrade/sweep |
| **Ledger** | camino de custodia §1.3; `ProfitSwept` como evento de salida | que ningún `FlashLoanExecuted` se contabilice como beneficio; `REALIZADO` sólo con balance delta |
| **Release** | patch 272 líneas + fixgen determinista | aplicar **sobre el blob auditado**, no sobre `fix/perhop-reserves-01`; el patch **no** aplica al worktree actual y eso es esperado |
| **SRE** | pausa acotada, latencia del timelock = **NO COMPUTADO** | medir el delay real en el deploy y publicarlo |
| **Backend/Integration** | selectores de §3 y digest de binding | ligar plan→payload→sim→tx con el mismo digest |
| **Architect** | 4.º rol (GUARDIAN) y slot nuevo `_pendingRequestHash` | contrato de rol compartido y append-only de storage |
| **Reviewer** | oráculo + fixcheck + mutantes | reproducir con el comando exacto; un PASS del oráculo acredita los **bytes**, no el runtime |

---

## 7. Evidencia invalidada por cambios

Estos resultados quedan **obsoletos** si: cambia cualquier blob anclado (el oráculo pasa a `TAMPER` y sale con código ≠ 0), cambia `solc`/remappings, cambia el ABI de `IFlashLoanProvider` o del Vault, se introduce el binding por otra vía (entonces C4 pasa a PASS legítimamente), o se añade una salida ERC-20 al wrapper (C11). En ese caso **re-ejecutar** el oráculo: es barato y determinista — no reutilizar estos veredictos.

---

## 8. Comandos exactos de reproducción

```powershell
node docs/contracts/CONTRACTS-ORACLE-01.mjs   --repo .   # veredictos + mutantes + encoder + fixtures
node docs/contracts/CONTRACTS-FIXGEN-01.mjs   --repo .   # genera el contrato corregido (16/16 ediciones)
node docs/contracts/CONTRACTS-FIXCHECK-01.mjs --repo .   # 6/6 sondas cerradas por el arreglo
# patch: git diff --no-index anclado vs propuesto (ya emitido en PROPOSED-callback-binding-01.patch)
```
