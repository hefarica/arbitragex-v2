# W12-F25-CALLBACK-BINDING-01 — Veredicto sobre el parche de binding del callback

- **run_id**: `arbx-entrega-20261008` · **dueño**: Security
- **Objeto**: `docs/contracts/PROPOSED-callback-binding-01.patch`
- **Línea base**: `docs/contracts/anchored/FlashLoanExecutor.sol` — blob **`96be27838d149d3a3f1b74b5bd575c89d25017a9`** (el contrato **DESPLEGADO**, SIN fixes)
- **Motor**: **Foundry forge 1.7.1** (commit `4072e487`, fuzzer integrado). **`echidna-test`: NO instalado.** El `forge.exe` local está **bloqueado por Smart App Control** → el build/test corrió sobre **`forge` del VPS, en copias aisladas bajo `/tmp`**, sin tocar `/opt/arbitragex-v2`.
- **CERO valores de secreto.** CERO mainnet, CERO firma, CERO broadcast, CERO capital, CERO rotación de claves, CERO cambios de umbrales.

---

## 0. Veredicto

**El parche NO es aterrizable y F25 sigue ABIERTO.** Tres defectos medidos, en orden de gravedad:

1. **NO COMPILA tal como está** (error de compilador reproducible).
2. **Con la corrección de UNA línea, compila — pero ROMPE el camino legítimo**: el gate de repago exige **el doble del principal**, así que **todo flash loan legítimo revierte**. Reproducido por fuzz.
3. **Es un bundle de CUATRO P0s (F25+F26+F27+F28)**, no F25: **12 de sus 13 hunks mezclan alcance**, de modo que "aplicar SOLO F25" es imposible sin redactar un parche nuevo.

Lo único que **sí** queda probado es que **el binding de F25 funciona** (fuzz 300/300: un callback no solicitado revierte y el balance previo queda intacto).

**Regla de cierre respetada: F25 NO se declara cerrado por tener el parche en una rama.** Se cerraría cuando hubiera **bytecode distinto de `96be2783…` sirviendo en producción** — y no lo hay: el código desplegado sigue siendo el mismo. **Un parche no aterrizado es un borrador.**

---

## 1. Contabilidad hunk por hunk (13 hunks) — y lo que revela

El parche tiene **13 hunks, +159 / −4**. Cada uno, contra la línea base:

| Hunk | Línea base | Qué cambia | Fix |
|---|---|---|---|
| H1 `@@ -28` | imports | `+ import PausableUpgradeable` | **FIX E (F28)** |
| H2 `@@ -52` | errores | `+ FL_RequestMismatch`, `+ FL_BelowProfitFloor`, `+ FL_NoTreasury` | **F25 + C + F27** |
| H3 `@@ -72` | declaración del contrato | `+ PausableUpgradeable` en la herencia | **FIX E (F28)** |
| H4 `@@ -132` | storage | `+ _pendingRequestHash`, `+ treasury`, `+ minProfitFloor` | **F25 + F27 + C** |
| H5 `@@ -159` | eventos | `+ TreasuryUpdated/ProfitSwept/MinProfitFloorUpdated/GuardianUpdated` | **F27 + C + F28** |
| H6 `@@ -171` | `initialize` | `+ __PausableUpgradeable_init()` ← **EL ERROR** | **FIX E (F28)** |
| H7 `@@ -281` | `requestFlashLoan` | `+ whenNotPaused`; set/clear del digest (x2) | **F28 + F25** |
| H8 `@@ -314` | callback Aave | exige digest y lo consume; `revert FL_RequestMismatch` | **F25 (único hunk puro)** |
| H9 `@@ -351` | callback Balancer | **Layer 4**: digest + `revert FL_RequestMismatch`; `balBeforeCallback` | **F25 + F26** |
| H10 `@@ -370` | gate de repago (Balancer) | `required = balBeforeCallback + amountOwed + minProfitFloor` | **F26 + C** |
| H11 `@@ -383` | `_executeAndRepayAave` | `+ balBeforeCallback` | **F26** |
| H12 `@@ -397` | gate de repago (Aave) | delta + piso | **F26 + C** |
| H13 `@@ -408` | funciones nuevas | `_requestDigest`, `asset_candidate`; `setTreasury`/`setMinProfitFloor`/`sweepProfit`; `GUARDIAN_ROLE`/`pause`/`unpause`/`setGuardian` | **F25 + F27 + C + F28** |

**Resultado de la contabilidad: de 13 hunks, SOLO H8 (el callback de Aave) es F25-puro.** Los otros 12 entremezclan F25 con F26/F27/F28/C dentro del mismo hunk. **El contrato de esta tarea dice "SOLO F25… no se mezclan acá": el parche no permite cumplirlo.** Aplicarlo entero aterriza los cuatro P0s de una vez.

**Dato de identidad que cierra el triángulo**: la cabecera del parche declara `index 96be278..ea4daff` — es decir, transforma el blob **desplegado** `96be278` en `ea4daff`, que es **exactamente** el artefacto que el rescate dejó en `docs/contracts/proposed/FlashLoanExecutor.fixed.sol`. Medido en el VPS: `git apply --check` = **OK**, y el archivo parcheado da blob **`ea4daff747af8f25f8b00b9a5bc0d70a3093a7d0`**. El parche + la base desplegada == el fix rescatado, byte a byte.

**Defecto de path en el contrato**: el parche está en `docs/contracts/PROPOSED-callback-binding-01.patch`; `docs/contracts/proposed/PROPOSED-callback-binding-01.patch` **no existe**.

---

## 2. Defecto 1 — NO COMPILA

```
$ forge build --force
Compiling 102 files with Solc 0.8.24
Error: Compiler run failed:
Error (7576): Undeclared identifier.
   --> src/FlashLoanExecutor.sol:205:9:
205 |         __PausableUpgradeable_init();
```

**Causa raíz**: el OpenZeppelin upgradeable instalado es **5.1.0**, y en esa versión la función se llama **`__Pausable_init()`**, no `__PausableUpgradeable_init()`:

```
$ grep -n "__Pausable" lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol
56:    function __Pausable_init() internal onlyInitializing {
```

El parche **contradice su propio comentario**, que dice que no hace falta re-inicializar (el flag de Pausable vive en un slot ERC-7201 que lee 0 = no pausado). **Borrando esa única línea, compila** (`Compiling 102 files` + `Compiler run successful`).

---

## 3. Defecto 2 — el gate de repago exige el DOBLE del principal

`receiveFlashLoan` toma `balBeforeCallback` **dentro del callback**, es decir **después de que el Vault ya transfirió el préstamo** al contrato. Así, `balBeforeCallback` **ya incluye el principal**, y el gate queda:

```
required = balBeforeCallback + amountOwed + minProfitFloor
         = (previo + amount) + amount + floor
```

→ El contrato exige **`previo + 2·amount + floor`** cuando lo único que puede tener es `previo + amount + beneficio`. **Todo préstamo legítimo revierte salvo que la ruta devuelva al menos el principal, o sea un 100% de beneficio.**

**Reproducido por fuzz** (motor: fuzzer integrado de Foundry 1.7.1, `--fuzz-runs 300`):

```
[FAIL: FL_BelowProfitFloor(); counterexample: args=[4]] testFuzz_legitRequest_withZeroFloor_succeeds(uint256)
[FAIL: FL_BelowProfitFloor(); ...] testFuzz_profitFloor_isAtomic(uint256,uint256,uint256)
[PASS] testFuzz_unsolicitedCallback_revertsAndKeepsBalance(uint256,bytes32) (runs: 300)
```

Nota: `legitRequest_withZeroFloor` fija **`floor = 0`** y **aun así** revierte → el fallo **no** viene del piso de beneficio (FIX C) sino del **doble conteo del principal** (FIX B/F26). El comentario del FIX B dice "baseline taken BEFORE the executor call": el ejecutor es `ArbitrageExecutor`, pero el principal entra **antes** de esa llamada. La línea está bien escrita y mal ubicada.

---

## 4. Lo que SÍ queda probado — el binding de F25 funciona

`testFuzz_unsolicitedCallback_revertsAndKeepsBalance` — **PASS, 300/300 runs**: un callback que **no** corresponde a una petición de este contrato **revierte con `FL_RequestMismatch`** y **el balance previo del wrapper queda intacto** (`assertEq(balanceOf, balBefore)`). Es decir: **la Capa 4 del FIX A cierra el vector** que F25 describe (cualquiera puede abrir un flash loan en el Vault real con `recipient = este contrato`).

Ojo con la lectura: esto prueba que **ese fix funciona**, no que F25 esté cerrado — sigue sin estar desplegado.

---

## 5. Storage layout / UUPS — el parche NO desplaza slots

| Slot | ANTES (línea base desplegada) | DESPUÉS (parcheado) |
|---|---|---|
| 0 | `aavePool` | `aavePool` |
| 1 | `arbitrageExecutor` (off 0) + `referralCode` (off 20) | **idéntico** |
| 2 | `flashLoanProvider` | `flashLoanProvider` |
| 3 | `balancerVault` | `balancerVault` |
| 4 | `_reentrancyStatus` | `_reentrancyStatus` |
| 5 | — | `_pendingRequestHash` (**nuevo, apendado**) |
| 6 | — | `treasury` (**nuevo, apendado**) |
| 7 | — | `minProfitFloor` (**nuevo, apendado**) |

**Los slots 0–4 son idénticos** (mismos nombres, mismos slots, mismos offsets) y las tres variables nuevas van **después**. El requisito append-only de UUPS **se cumple**, y está **mostrado** con el layout real, no afirmado. (Medido con `forge inspect … storageLayout`; el one-liner de python del contrato falla porque el artefacto por defecto **no incluye `storageLayout`** — desviación declarada, no silenciosa.)

---

## 6. Test de desigualdad — y por qué F25 sigue ABIERTO

| Artefacto | sha256 del runtime | bytes |
|---|---|---|
| Bytecode compilado de la **línea base** | `f629c35ad45f9c6892c7f8cfaae0943e70e341a408b2e1da7000c667c2f33e45` | 7923 |
| Bytecode compilado del **parcheado** | `0f10457c957e618c4d46fb1f721a6bbe8598e34f6274dba325b36ea0dba868e5` | 9964 |

Los dos **difieren** — pero **eso no cierra F25**. El test que importa es contra el **bytecode desplegado**, y **producción sigue sirviendo el código viejo**: no hubo deploy, y **esta tarea no puede hacerlo** (cero mainnet, cero firma, cero broadcast). Por lo tanto:

> **F25 NO está cerrado. Se cerrará cuando haya bytecode distinto de `96be2783…` sirviendo en producción.** Hoy no lo hay.

---

## 7. Frontera declarada y falsedad a no repetir

- **F28 se enuncia al derecho**: certifica que **el binding QUEDÓ DESPLEGADO**, NO que `pendingRequest` esté ausente. Certificar su ausencia sería **firmar el agujero** — `pendingRequest = 0` en el contrato desplegado **no es una condición a certificar, ES el agujero**. Ese error ya se cometió y se corrigió; no se repite acá.
- **`#868` y `#870` son rescates documentales y NO Solidity productivo.** Este parche **sí** es funcional, y por eso es de otra categoría — pero **sigue sin aterrizar**, y con los defectos de §2 y §3 **no es candidato a aterrizar** en su forma actual.
- **F26, F27 y F28 NO se tocan acá**: son tareas propias con cadena dependiente. Este informe los **menciona** sólo porque el parche los arrastra.

## 8. Defectos de instrumento propios, declarados

1. Mi primera comparación de bytecode dio `DISTINTOS=True` **contra un artefacto VACÍO** (el `sha256` reportado, `e3b0c442…`, es el hash de la cadena vacía) porque el build parcheado había **fallado**. Aquella conclusión **no valía**; la de §6 es sobre artefactos reales.
2. El one-liner de `python3` del contrato para el layout **falla** (`KeyError: 'storageLayout'`) contra el artefacto por defecto; usé `forge inspect … storageLayout`. **Desviación declarada.**
3. El path del parche en el contrato (`docs/contracts/proposed/…`) **no existe**; el real es `docs/contracts/PROPOSED-callback-binding-01.patch`.
