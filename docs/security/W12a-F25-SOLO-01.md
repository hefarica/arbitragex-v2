# W12a-F25-SOLO-01 — Extracción del subconjunto F25 (binding del callback)

- **run_id**: `arbx-entrega-20261008` · **dueño**: Security
- **Objeto**: extraer del bundle `PROPOSED-callback-binding-01.patch` **solo F25** y dejarlo **aterrizable**.
- **Línea base**: `contracts/src/FlashLoanExecutor.sol`, blob **`96be27838d149d3a3f1b74b5bd575c89d25017a9`** (el contrato **DESPLEGADO**, sin fixes).
- **Motor**: **fuzzer integrado de Foundry forge 1.7.1** (commit `4072e487`) en el VPS. `forge.exe` local **bloqueado por Smart App Control**; `echidna-test` **no instalado**. Todo el trabajo en **copias aisladas bajo `/tmp`**; `/opt/arbitragex-v2` **intacto**.
- **CERO mainnet, firma, broadcast, capital o rotación de claves.**

---

## 0. Veredicto

**El subconjunto F25 compila, no rompe el camino legítimo, y el binding funciona.** Es **aterrizable** — sujeto a revisión independiente.

**Pero F25 NO queda cerrado en esta tarea.** Regla de cierre, sin excepción: **F25 se cierra cuando haya bytecode distinto de `96be2783…` sirviendo en producción.** Un parche en una rama es un **borrador**. Hoy producción sigue sirviendo el código viejo.

**Aclaración de método, y es importante**: no se puede "copiar solo H8". **H8 aislado NO COMPILA**: su check usa `_pendingRequestHash` (declarado en **H4**), `_requestDigest(...)` (definido en **H13**), `error FL_RequestMismatch()` (**H2**) y el *set* del compromiso en la fase de solicitud (**H7**). Lo que se entrega es el **subconjunto F25 a nivel de LÍNEA** de esos hunks, con las líneas de F26/F27/F28/C **removidas**.

---

## 1. Mapa hunk → P0 de los 13 hunks (lo que habilita partir el resto)

| Hunk | Clase | Detalle |
|---|---|---|
| **H1** `@@ -28` | **F28** | `+ import PausableUpgradeable` |
| **H2** `@@ -52` | **MIXTO: F25 + C + F27** | `+ FL_RequestMismatch` (F25), `+ FL_BelowProfitFloor` (C), `+ FL_NoTreasury` (F27) |
| **H3** `@@ -72` | **F28** | `+ PausableUpgradeable` en la herencia |
| **H4** `@@ -132` | **MIXTO: F25 + F27 + C** | `+ _pendingRequestHash` (F25), `+ treasury` (F27), `+ minProfitFloor` (C) |
| **H5** `@@ -159` | **MIXTO: F27 + C + F28** | eventos `TreasuryUpdated`/`ProfitSwept` (F27), `MinProfitFloorUpdated` (C), `GuardianUpdated` (F28) |
| **H6** `@@ -171` | **F28** | `+ __PausableUpgradeable_init()` ← **el defecto que no compila** |
| **H7** `@@ -281` | **MIXTO: F28 + F25** | `+ whenNotPaused` (F28); set/clear del digest (F25) |
| **H8** `@@ -314` | **F25-PURO** ✅ | check del digest en el callback de **Aave** |
| **H9** `@@ -351` | **MIXTO: F25 + F26** | Layer 4 (digest) = F25; `+ balBeforeCallback` = F26 |
| **H10** `@@ -370` | **MIXTO: F26 + C** | `required = balBeforeCallback + amountOwed + minProfitFloor` |
| **H11** `@@ -383` | **F26** | `+ balBeforeCallback` en `_executeAndRepayAave` |
| **H12** `@@ -397` | **MIXTO: F26 + C** | gate delta-scoped + piso |
| **H13** `@@ -408` | **MIXTO: F25 + F27 + C + F28** | `_requestDigest`/`asset_candidate` (F25); `setTreasury`/`setMinProfitFloor`/`sweepProfit` (F27+C); `GUARDIAN_ROLE`/`pause`/`unpause`/`setGuardian` (F28) |

**Resumen: de 13 hunks, 1 es F25-puro (H8), 3 son F28 (H1/H3/H6), 1 es F26 (H11), y 8 son MIXTOS.** Los que traen F25 son **H2, H4, H7, H8, H9, H13** — y solo H8 lo trae sin contaminación.

## 2. Qué contiene el parche mínimo, y qué queda fuera

**Incluido (subconjunto F25 de H2/H4/H7/H8/H9/H13)** — 6 hunks, **+48/−1**, **1 archivo**:

1. `error FL_RequestMismatch();` (de H2; **sin** `FL_BelowProfitFloor` ni `FL_NoTreasury`)
2. `bytes32 private _pendingRequestHash;` (de H4; **sin** `treasury` ni `minProfitFloor`)
3. **Set** del digest en `requestFlashLoan`, en ambas ramas (de H7; **sin** `whenNotPaused`)
4. **Clear** del digest al volver de la llamada externa
5. **Check + consume** en `executeOperation` (**H8**, el único F25-puro)
6. **Check + consume** en `receiveFlashLoan` (de H9; **sin** `balBeforeCallback`)
7. Helpers `_requestDigest` + `asset_candidate` (de H13; **sin** `setTreasury`/`sweepProfit`/`GUARDIAN_ROLE`/`pause`)

**Excluido, nominalmente y con razón** (verificado por conteo = **0** en el archivo resultante):

| Excluido | Hunk de origen | P0 | Razón de exclusión |
|---|---|---|---|
| `import PausableUpgradeable` + herencia | H1, H3 | **F28** | Otra tarea; **no es F25** |
| `__PausableUpgradeable_init()` | H6 | **F28** | **Además NO EXISTE en OZ 5.1.0** (se llama `__Pausable_init()`) → **no compila**. Excluido y declarado como de F28 |
| `whenNotPaused` | H7 | **F28** | Frena la exposición nueva; no tiene que ver con el binding |
| `GUARDIAN_ROLE` / `pause` / `unpause` / `setGuardian` | H13, H5 | **F28** | Guardian/pause: tarea propia |
| `treasury` / `setTreasury` / `sweepProfit` / `FL_NoTreasury` / eventos | H4, H5, H13 | **F27** | Tesorería: tarea propia |
| `balBeforeCallback` (×2) + gate `required = balBefore + owed + floor` | H9, **H10**, H11, **H12** | **F26** | Repago por delta: tarea propia **y** el bundle lo tenía MAL (§4) |
| `minProfitFloor` / `setMinProfitFloor` / `FL_BelowProfitFloor` | H4, H5, H10, H12, H13 | **FIX C** | Piso de beneficio: no es F25 |

## 3. Evidencia medida

**Compilación REAL** (`out/` y `cache/` borrados antes; sin `Fresh` engañoso):

```
Compiling 103 files with Solc 0.8.24
Compiler run successful with warnings:
```

**Storage layout — append-only, mostrado** (`forge inspect … storageLayout`):

| Slot | ANTES (desplegado) | DESPUÉS (F25-solo) |
|---|---|---|
| 0 | `aavePool` | `aavePool` |
| 1 | `arbitrageExecutor` (off 0) + `referralCode` (off 20) | **idéntico** |
| 2 | `flashLoanProvider` | `flashLoanProvider` |
| 3 | `balancerVault` | `balancerVault` |
| 4 | `_reentrancyStatus` | `_reentrancyStatus` |
| 5 | — | **`_pendingRequestHash` (nuevo, apendado)** |

**Ningún slot desplazado.** (El bundle apendeaba 3 slots; esta versión solo 1 — más limpio.)

**Test de desigualdad (bytecode compilado)**:

| | sha256 del runtime | bytes |
|---|---|---|
| Línea base desplegada | `f629c35ad45f9c6892c7f8cfaae0943e70e341a408b2e1da7000c667c2f33e45` | 7923 |
| Subconjunto F25 | `7dbca72e77e32f2504e2a7f9816fd50ac0e5477b3b13968bf7834024d025c910` | 8332 |

Diferentes ✅ — pero esto es **bytecode compilado**, no **desplegado**: **F25 sigue ABIERTO**.

**El parche aplica sobre una copia prístina**: `git apply --check` = **OK**, `git apply` = **OK**, y el archivo resultante da blob **`355bef3d3ea090fc346e0f169aeca81fd019971e`**. El parche declara `index 96be278..355bef3`.

**Tests (fuzzer integrado de Foundry 1.7.1, `--fuzz-runs 300`) — 4 passed / 0 failed**:

```
[PASS] testFuzz_legitPath_completes(uint256)                          (runs: 300)   ★
[PASS] testFuzz_legitPath_withSurplus_completes(uint256,uint256)      (runs: 300)   ★
[PASS] testFuzz_unsolicitedCallback_revertsAndKeepsBalance(uint256,bytes32) (runs: 300)
[PASS] test_callbackCannotBeReplayed()                                (gas: 125760)
```

- **El binding**: un callback NO solicitado **revierte con `FL_RequestMismatch`** y el **balance previo del wrapper queda intacto** (300/300).
- ★ **EL CAMINO LEGÍTIMO COMPLETA** (300/300) — el criterio que el bundle violaba. Sin este test, "arreglar F25" y "romper la ejecución" son indistinguibles.
- Además: el compromiso **se consume**, así que el mismo callback **no se puede repetir**.

## 4. ★ Nota de diseño heredada — el arreglo de F26 (NO se implementa acá)

**`balanceBefore` pertenece a la fase de SOLICITUD, no al callback.** El binding de F25 ya persiste estado por petición (`_pendingRequestHash`); el saldo previo tiene que capturarse **en el mismo momento** y **viajar con la petición**. Tomarlo dentro del callback es lo que produce el **doble conteo**: para cuando el callback corre, el Vault ya transfirió el principal, así que `balBeforeCallback` lo **incluye**, y el gate queda `required = previo + 2·amount + floor` ⇒ **todo préstamo legítimo revierte** salvo 100 % de beneficio (medido por fuzz en t138 con `floor = 0`, lo que probó que el culpable era el doble conteo y no el piso).

**F25 y F26 están acoplados por construcción, y el binding es el lugar natural para ese dato.** Se documenta acá; **arreglar F26 es otra tarea.**

## 5. Límites

- **F25 NO queda cerrado**: hace falta bytecode distinto de `96be2783…` **sirviendo en producción**.
- **`#868` y `#870` son rescates documentales y NO Solidity productivo.** Este parche **sí** es funcional y por eso es de otra categoría — **candidato a aterrizar tras revisión independiente**.
- **Nada de F26/F27/F28/FIX C se toca acá** (verificado: 0 ocurrencias de `treasury`, `sweepProfit`, `minProfitFloor`, `GUARDIAN_ROLE`, `whenNotPaused`, `PausableUpgradeable`, `balBeforeCallback`, `FL_BelowProfitFloor` en el archivo resultante).
- **Cero mainnet, firma, broadcast, capital o rotación de claves.** `/opt/arbitragex-v2` **intacto** (verificado con `git status`).
