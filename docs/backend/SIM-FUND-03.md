# SIM-FUND-03 — Sí, la mutación llega a la EJECUCIÓN. Y la causa raíz de t111 era MÍA

**run_id** arbx-entrega-20261007 · **phase_id** implementation · **SHA_BASE** `80e2f86c` (main medido)
**Rama** `fix/sim-fund-03` (PR propio, SIN merge) · **dueño** Backend
**Alcance** `backend/sim-ctl/src/signer_funding.rs` (1 archivo) · **NO se tocó** `consumer.rs` (W8)
**Prohibiciones respetadas**: sin merge, sin deploy, sin tocar main, sin reiniciar servicios.

---

## 0. INSTRUMENTO

`command -v cargo` → `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` · `GATE1=PASS`.
md5 idéntico WSL↔Windows **antes** de correr: `signer_funding.rs aff33a993bf8d4fc13286df3bf885c05` en ambos lados.

---

## 1. W1 — LA RESPUESTA: **SÍ**, EXISTE UN PRIMITIVO QUE LLEGA A LA EJECUCIÓN

Dos mediciones, ambas con el par antes/después medido **por `eth_call`**.

### 1.1 Una mutación producida por EJECUCIÓN llega a `eth_call`

`anvil_setBalance(signer, 10 ETH)` + `anvil_impersonateAccount(signer)` + `eth_sendTransaction` a `WETH.deposit()` con 1 ETH:

```
before_balanceOf = 0x000000000000000000000000000000000000000000000000000000174876e800   (1e11)
after_balanceOf  = 0x0000000000000000000000000000000000000000000000000de0b6caefdae800
```

**`after − before` = exactamente 1 ETH en WETH** (`0xde0b6b3a7640000`). La transacción se ejecutó (devolvió hash), el `deposit()` minteó, y `balanceOf` — por `eth_call` — lo vio. **Y el `eth_call` de `deposit()` solo pudo tener éxito porque la EVM veía el `anvil_setBalance`.**

### 1.2 `anvil_setStorageAt` TAMBIÉN llega — con el slot CORRECTO

```
== TEST con el slot CORRECTO (keyed por SIGNER) ==
  slot3 storage_tras_write=0x0000000000000000000000000000000000000000000000005eedf00d00000001
       balanceOf=0x0000000000000000000000000000000000000000000000005eedf00d00000001  <== EL CENTINELA REPRODUCE
       restaurado=0x…174876e800 (original=0x…174876e800)
```

**⇒ El centinela reproduce en `balanceOf` exactamente como el código espera.** `anvil_setStorageAt` funciona. **NO hay separación overlay/ejecución.**

---

## 2. RETRACCIÓN: LA CAUSA RAÍZ DE t111 ERA UN ERROR MÍO

**t111 afirmó**: *"`anvil_setStorageAt` escribe en un OVERLAY que `eth_getStorageAt` LEE y que `eth_call` NUNCA ve"*. **Es FALSO, y el error fue mío.**

**Qué pasó.** En t111 computé los slots con una función etiquetada `index(tok, s)` — **keyeada por la dirección del TOKEN**. Pero `balance_slot(slot, signer)` keyea por el **SIGNER**. Usé, en todos los experimentos de t111 y en la primera mitad de t112:

| | slot 3 |
|---|---|
| lo que usé (clave = **TOKEN** WETH) | `0x67aa9b7d2b6d14f3837d07b1073399a41e4104b1d98f169f02cc04f44f14f4b0` |
| el correcto (clave = **SIGNER**) | `0x961558ef95740fe5d8173078fa8d9fd6150201cd29befffef12f314fd45a2bfc` |

**Escribí el centinela en un slot que no tiene nada que ver con `balanceOf(signer)`.** Que `eth_getStorageAt` lo mostrara y `balanceOf` no cambiara **es el comportamiento CORRECTO de escribir en un slot ajeno** — no una separación de vistas. Interpreté como hallazgo lo que era la confirmación de mi propio error.

**Y el "control negativo" que creí independiente** (`anvil_setBalance` → `eth_getBalance` propaga; `anvil_setStorageAt` → `balanceOf` no) parecía separar lecturas de ejecución: en realidad separaba **una mutación en el sitio correcto de una mutación en el sitio equivocado**.

**El dato que debió delatarlo y no usé**: `eth_getStorageAt(slot3)` devolvía `0x…29755b74b022eef3d6` mientras `balanceOf` decía `0x…174876e800`. **Si ese fuera el slot del balance, tendrían que coincidir.** No coincidían → el slot estaba mal. Lo traté como "dos vistas" en vez de como "slot equivocado".

**Consecuencias que hay que declarar:**
1. **La causa raíz de t111 queda RETIRADA.** No hay separación overlay/ejecución.
2. **PR #859 está ABIERTO y SIN MERGEAR**, y su cambio (`write_invisible` / `write_not_persisted`) está construido sobre la premisa retirada: etiquetaría como "la EVM no lo ve" lo que en realidad es "escribiste en un slot que no es". **NO debe mergearse tal cual.** Este PR lo dice explícitamente.
3. **El hallazgo de t112 §1 se sostiene por sí mismo**: es una medición directa, no una inferencia.

---

## 3. W3 — ALCANCE REAL: **NO** INVALIDA LA SIMULACIÓN BASADA EN FORK

La pregunta era si `eth_call` contra este anvil ve el estado mutado. **Lo ve**: §1.1 y §1.2 lo miden por `eth_call` en ambos sentidos (mutación por ejecución y mutación directa). **No hay tal invalidación. La plataforma de simulación basada en fork no está estructuralmente limitada por esto.**

---

## 4. W4 — LA CONFIGURACIÓN ALTERNATIVA: EL FLAG EXISTE, Y LA PREMISA ES NULA

**El flag existe y se nombra con su valor exacto**: `--no-storage-caching` (anvil 1.7.1, `docker exec arbitragex-v2-anvil-1 anvil --help` → *"All storage slots are read entirely from the endpoint."*). Producción corre `anvil --fork-url "$ANVIL_FORK_URL" --host 0.0.0.0 --port 8545` — **sin él**.

**Se probó en un anvil AISLADO**, como exige W4: contenedor `arbx-anvil-iso-t112` sobre `arbitragex-v2_arbx-net`, misma imagen pinneada y mismo `ANVIL_FORK_URL`, con `--no-storage-caching`. **Contenedor eliminado tras la prueba** (`docker ps -a | grep -c arbx-anvil-iso` → **0**).

**Resultado**: `before` y `after` sin cambio. **PERO ESE TEST USÓ EL SLOT EQUIVOCADO** (el mismo error de §2), así que **no mide nada sobre el flag** — y lo declaro en vez de presentarlo como evidencia.

**Y la premisa de W4 queda NULA**: se diseñó para *eliminar la separación overlay/ejecución*, y **esa separación no existe** (§1.2). **No hay nada que eliminar. No se necesita ningún flag.** El anvil de producción, tal como está, ejecuta correctamente las escrituras en el slot correcto.

---

## 5. W5 — CADA EXPERIMENTO, RESTAURADO Y VERIFICADO

Todos los experimentos usaron **`evm_snapshot` + `evm_revert`** (el mecanismo nativo), con captura previa del valor y verificación posterior:

| experimento | restauración |
|---|---|
| §1.1 deposit (minó una transferencia real) | `evm_revert`; `final_balanceOf == before` ✓ |
| §1.2 barrido de 4 slots (4 escrituras) | `anvil_setStorageAt` al original por slot + `evm_revert`; cada slot verificado en su valor original ✓ |
| §4 anvil aislado | contenedor eliminado, 0 restantes ✓ |

**NO se dejó ninguna mutación.** A diferencia del primer barrido de t111 —donde dejé el centinela en un slot por una captura vacía y lo cacé leyendo mi propio log— aquí la restauración se verificó **en el mismo comando que la escribió**.

---

## 6. W2 — RE-FINANCIAR: EL MECANISMO YA FUNCIONA

`SENTINEL_BALANCE` reproduce en `balanceOf` por el camino que el código ya usa (§1.2). **No hay que cambiar de mecanismo.** El `return Ok(())` sigue exigiendo `verified == Some(SENTINEL_BALANCE)` y **no se tocó**.

**El cambio que SÍ se hizo ataca la causa de MI error, que es la que produjo un diagnóstico equivocado**: `balance_slot` estaba pinneado por valor contra el vector externo, pero **no fijaba QUIÉN es la clave**. Con el token como clave, cualquier slot distinto sigue dando un hash de 32 bytes plausible. El test nuevo fija el contrato con **CONTROL NEGATIVO**:

```rust
assert_eq!(balance_slot(3, signer), 0x961558ef…);         // el valor medido en el fork vivo
assert_ne!(balance_slot(3, signer), balance_slot(3, token)); // la clave es el SIGNER
```

---

## 7. LO QUE SIGUE ABIERTO — y ahora está mucho mejor planteado

**Si `anvil_setStorageAt` funciona en el slot correcto, ¿por qué producción falla el 100%?** Con WETH y el slot 3 **dentro** de `CANDIDATE_SLOTS`, la aritmética del código dice que debería funcionar. **No lo sé, y NO lo voy a inventar.**

Lo que esta tarea cambia es **dónde buscar**: ya no es una propiedad del anvil (§1), ni la derivación (§2, correcta), ni la lista de slots para los tokens mayoritarios. Hay que instrumentar **el `slot` concreto y el `token` concreto** de cada intento fallido en producción — medición que este alcance no permite sin desplegar, y desplegar está prohibido. **NO COMPUTADO, con la razón.**

---

## 8. W6 / W7 / W8

- **W6**: `grep -c 'passed=.true.'` sobre la evidencia viva de métricas → **0**. Ningún umbral tocado; el `return Ok(())` sigue exigiendo verificación real. El mejor caso sigue con `net_profit_usd` **negativo** (`-0.6538892865647935`) — veredicto de mercado medido, reportado tal cual, sin subir el sizing.
- **W7**: las 4 puertas **PASS** (`Checking sim-ctl v0.1.0`, 0 errores/warnings); md5 idéntico WSL↔Windows **antes** de correr; test nuevo con CONTROL NEGATIVO.
- **W8**: PR propio, SIN merge. `git status --porcelain` → solo `M backend/sim-ctl/src/signer_funding.rs`. **NO se tocó `consumer.rs`.** No se modificó `docker/` (el flag existe pero **no hace falta**, §4).
