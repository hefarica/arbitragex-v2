# SIM-FUND-02c — Causa raíz: el anvil ESCRIBE en un overlay que su propia EVM no lee

**run_id** arbx-entrega-20261007 · **phase_id** implementation · **SHA_BASE** `9ccd1d04`
**Rama** `fix/sim-fund-02c` (PR propio, SIN merge) · **dueño** Backend
**Alcance** `backend/sim-ctl/src/signer_funding.rs` (1 archivo) · **NO se tocó** `consumer.rs` (F8), `sim_engine.rs`, `persistence.rs`
**Prohibiciones respetadas**: sin merge, sin deploy, sin tocar main, sin reiniciar servicios.

---

## 0. INSTRUMENTO

`command -v cargo` → `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` · `GATE1=PASS`.
md5 idéntico WSL↔Windows **antes** de correr: `signer_funding.rs eba7263ef026805b2f2c19cfcbcfc507` en ambos lados.
Toda comunicación con el anvil por `base64 -d | bash` (comillas anidadas de PowerShell manglean en silencio).

---

## 1. F1 — LOS TRES OUTCOMES NO SON TRES CAUSAS

La medición de t107 (2.195 eventos): `verify_mismatch` **1.428**, `balance_unreadable` **410**, `slot_unresolved` **357**, `write_rejected` **0 (serie ausente)**.

**La aritmética los ordena sola: 1.428 / 357 = 4,0000 EXACTO.**

`CANDIDATE_SLOTS` tiene exactamente 4 elementos (`signer_funding.rs:41`). **Cada token que agota el loop produce 4 intentos de slot, y los 4 producen un `verify_mismatch`.** El 4:1 exacto no es casualidad: es la firma de que **`verify_mismatch` y `slot_unresolved` son el MISMO suceso visto a dos granularidades** — por slot y por token.

Y `balance_unreadable` **no es un tercero independiente: está CONTENIDO en `verify_mismatch`.** El camino lo prueba leyendo el código:

```rust
Err(_) => { count(outcome::BALANCE_UNREADABLE); Ok(None) }   // balance_of
...
if verified == Some(SENTINEL_BALANCE) { … } else { count(outcome::VERIFY_MISMATCH); }  // el loop
```

`Ok(None)` hace que `verified == Some(SENTINEL)` sea falso ⇒ **cae al `else` y también cuenta `VERIFY_MISMATCH`.** Una lectura ilegible incrementa AMBOS contadores.

| relación | evidencia |
|---|---|
| `verify_mismatch` = 4 × `slot_unresolved` | 1428/357 = 4,0000 · `CANDIDATE_SLOTS: [u64; 4]` |
| `balance_unreadable` ⊆ `verify_mismatch` | 410 ≤ 1428, y el `else` del loop cuenta ambos |

**⇒ ORDENADOS: hay UNA causa raíz del techo (`slot_unresolved`: ningún slot cierra el par escrito/leído, para NINGÚN token). `verify_mismatch` es su síntoma por-slot; `balance_unreadable` es un síntoma anidado dentro de aquel.** Y como `seeded_fresh` **tampoco existe** como serie, **el firmante NUNCA se fondea: 0 éxitos sobre 2.195 eventos.**

---

## 2. F2 — LA VISIBILIDAD: RESUELTA, Y NO ERA ENTRE DOS PROVIDERS

`main.rs:661`: `SignerFunder::new(f.provider.clone())` donde `f` es el `ForkManager`, cuyo `provider: Arc<Provider<Http>>` apunta al mismo `ANVIL_URL`. **Mismo `Arc`, mismo anvil, mismo fork: la hipótesis "dos providers distintos" queda REFUTADA.**

**Pero la visibilidad SÍ era el problema — DENTRO del mismo anvil.** Ver §3.

---

## 3. F4 — EL ESBIRRO ACOTADO (autorizado, restaurado, declarado)

### 3.1 Por qué la vía read-only no alcanzó (F3 agotada)

- **(a) La derivación es CORRECTA.** Reimplementé keccak256 y verifiqué mi propia implementación contra el vector EXTERNO que ya pincha el test: `keccak256(abi.encode(0x1111…1111, 9))` → `233b1b49de63438bb1ac1a57ef81babcc52ccd4555c968bb144593ea539bbebc` — **coincide exacto**. `balance_slot` computa lo que `cast index` computa.
- **(b) Los tokens son los CANÓNICOS.** Corte post-deploy (`simulated_at >= 2026-10-08T02:51:25Z`, join `simulations`→`opportunities`): WETH `0xc02aaa…c756cc2` **10.052**, USDC `0xa0b869…` **8.364**, DAI `0x6b17…` **1.839**, USDT `0xdac17f…` **93**. Sus slots de balance son **3, 9, 2** — **los tres están en `CANDIDATE_SLOTS = [0,2,3,9]`.** Si la lista fuera el problema, WETH habría funcionado. **REFUTADO.**
- **(c) `balance_unreadable` (410) es real pero MINORITARIO**: 1.018 lecturas SÍ devuelven palabra y aun así discrepan.

### 3.2 El experimento (mismos primitivos que producción)

`anvil_setStorageAt` + `balanceOf`, capturando el valor previo por `eth_getStorageAt` y **restaurándolo**:

```
== balanceOf(WETH, signer) de referencia ==
balanceOf=0x000000000000000000000000000000000000000000000000000000174876e800
---- slot 0x67aa9b7d… (slot 3 del mapping) ----
  storage DESPUES de escribir = 0x0000000000000000000000000000000000000000000000005eedf00d00000001
  balanceOf DESPUES           = 0x000000000000000000000000000000000000000000000000000000174876e800
```

**Para los CUATRO slots candidatos (0, 2, 3, 9), el mismo patrón: el storage queda con el centinela y `balanceOf` NO cambia.**

### 3.3 El test que lo cierra

Escribí un marcador inconfundible (`0x…deadbeef01`) en el **slot 5** — el `totalSupply` de WETH9:

```
WETH.totalSupply() ANTES  = 0x00000000000000000000000000000000000000000001cd33d1534d1924cd81f6
storage slot5 ANTES       = 0x0000000000000000000000000000000000000000000000000000000000000000
storage slot5 DESPUES     = 0x000000000000000000000000000000000000000000000000000000deadbeef01
WETH.totalSupply() DESPUES= 0x00000000000000000000000000000000000000000001cd33d1534d1924cd81f6
```

**Y el CONTROL que decide, sin ninguna escritura mía: `eth_getStorageAt(slot5)` devuelve CERO mientras `totalSupply()` — ejecutado por `eth_call` — devuelve un valor NO cero.**

Control negativo del instrumento: `anvil_setBalance(signer, 0x1234)` → `eth_getBalance` = **`0x1234`** ⇒ **las mutaciones locales SÍ llegan a las LECTURAS de estado.** El fallo es específico de la EJECUCIÓN.

### 3.4 La causa raíz

**En este anvil, `anvil_setStorageAt` escribe en un overlay que `eth_getStorageAt` lee y que `eth_call` — la EJECUCIÓN de la EVM — NUNCA ve.**

`balanceOf` se evalúa con `eth_call` (`signer_funding.rs:274`, `provider.call(&typed, None)`, `BlockId` por defecto = latest). El `eth_call` ejecuta contra el estado del FORK. La escritura vive en el overlay. **Son dos vistas distintas del mismo nodo.**

⇒ **El centinela no puede reproducirse NUNCA, para NINGÚN slot, NINGÚN token y NINGÚN firmante.** Eso explica el 100% de fallos, el `write_rejected = 0` (el nodo siempre acepta: escribe bien, en el overlay) y el 4:1 exacto.

### 3.5 RESTAURACIÓN Y DECLARACIÓN (F4-ii, F4-iii)

**Restauración verificada**, los cuatro slots en su valor original:

```
0x2a11cb67… -> 0x0000000000000000000000000000000000000000000000000000000000000000
0xde032e96… -> 0x0000000000000000000000000000000000000000000000000000000000000000
0x67aa9b7d… -> 0x00000000000000000000000000000000000000000000000029755b74b022eef3d6
0xa4adb76a… -> 0x0000000000000000000000000000000000000000000000000000000000000000
balanceOf_final = 0x…174876e800  (== referencia)
```

**Se declara, como exige F4-iii:** hubo UN fallo de mia parte — en el primer barrido mi captura de `$ORIG` salió vacía para el slot 0 y la restauración no se aplicó, dejando el centinela ahí. Lo detecté leyendo el propio log, **lo restauré (verificado: `antes=0x…5eedf00d00000001` → `despues=0x0000…0000`)** y el control final confirma los 4 slots limpios. Un segundo fallo (`invalid string length`) fue un hex de 66 dígitos en vez de 64, corregido y verificado.

---

## 4. F5 — EL ARREGLO: ATACA POR QUÉ ESTA CLASE SOBREVIVIÓ DOS VECES

**Lo que la causa raíz obliga a decir:** el arreglo REAL es de **configuración del anvil** (que su ejecución vea el overlay), y eso vive en `docker/` — **fuera del alcance de esta tarea**. La causa raíz queda **localizada y nombrada**, que es lo que el objetivo admite explícitamente cuando no es arreglable en alcance.

**Lo que SÍ se arregló en alcance, y ataca la causa de la ceguera:** el código afirmaba, en un comentario, *"The slot is not the one `balanceOf` reads"* — una afirmación sobre **dónde fue la escritura** que la medición de §3 demuestra **falsa**. Esa afirmación equivocada es lo que permitió que la clase sobreviviera a SIM-FUND-01b y a SIM-FUND-02 describiendo una causa nueva cada vez.

El loop ahora **lee de vuelta el slot que escribió** (`eth_getStorageAt`) y parte `verify_mismatch` en dos:

| outcome nuevo | significado |
|---|---|
| `write_invisible` | el overlay **TIENE** el centinela y la EVM igual no lo ve ⇒ la escritura llegó, el lector no la alcanza |
| `write_not_persisted` | el overlay **NO** tiene el centinela ⇒ la escritura no prendió en ese slot |

Si el read-back mismo falla, **no se inventa causa**: se mantiene `verify_mismatch`. **La próxima medición discrimina sin que nadie tenga que volver a montar el experimento a mano.**

---

## 5. F6 — FAIL-CLOSED INTACTO

No se tocó ningún umbral, ni `max_slippage_for_pass_pct`, ni el camino de `passed`. **NO se hizo que el fondeo devuelva `Ok(())` sin verificar** (F5 lo prohíbe): el `return Ok(())` sigue exigiendo `verified == Some(SENTINEL_BALANCE)`, y el fallo sigue siendo `Err("sim_signer_funding_slot_unresolved")` → fail-closed.

**Evidencia viva: NO existe ninguna serie `arbx_simulation_total{passed="true"}`.** Sólo `{passed="false",simulator="anvil"}` y `{passed="false",simulator="revm"}`.

Y no se mejoró ningún número: el mejor caso de la ventana sigue con `simulated_profit_usd` negativo (`net_profit_usd: -0.6538892865647935` en el mensaje real del stream) — **es un veredicto de mercado medido, se reporta tal cual.** No se subió el sizing.

---

## 6. F7 — LAS 4 PUERTAS

| puerta | resultado | evidencia |
|---|---|---|
| `command -v cargo` | **PASS** | `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` |
| `cargo fmt --all -- --check` | **PASS** | sin diff |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **PASS** | `Checking sim-ctl v0.1.0` + `Finished … in 17.07s`; `grep -cE '^(error\|warning)'` = **0** |
| `cargo test -p sim-ctl --no-fail-fast` | **PASS** | 0 fallos; test nuevo por nombre: `simfund02c_the_discriminator_labels_are_distinct ... ok` |
| `cargo test -p sim-core --no-fail-fast` | **PASS** | `79 passed; 0 failed` |

`Compiling` **O** `Checking` exigidos y presentes. md5 idéntico WSL↔Windows **antes** de correr. Exit code real por puerta.

El test nuevo lleva **CONTROL**: comprueba que los 9 labels no están vacíos y **ninguno está duplicado sobre el conjunto completo** — sin él, el test pasaría igual si alguien añadiera un tercer label idéntico a otro.

---

## 7. F8 — PR PROPIO

El arreglo vive en `signer_funding.rs`. **NO se tocó `consumer.rs`** (`git status --porcelain` → sólo `M backend/sim-ctl/src/signer_funding.rs`), así que **NO comparte archivo de código con #857** (que toca `consumer.rs`). PR propio, **sin mergear**.

---

## 8. NO COMPUTADO

- **Que la configuración alternativa del anvil (p. ej. desactivar la caché de storage del fork) elimine el problema.** No se probó: cambiarla exige tocar el contenedor, que está fuera de alcance y prohibido. **NO COMPUTADO.**
- **Por qué el `stateDiff` override de `eth_call` tampoco cambió el resultado.** Se probó y devolvió el mismo valor; si es por sintaxis del override o porque el slot no es el de `balanceOf` **no se discriminó**. **NO COMPUTADO.**
- **Cuál es el slot REAL de `balanceOf`** para estos tokens en este fork. No se localizó: el experimento de §3.3 demuestra que la pregunta es secundaria, porque **ninguna** escritura de storage llega a la ejecución.
