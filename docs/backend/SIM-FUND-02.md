# SIM-FUND-02 — el techo del fondeo: qué discriminé, y qué NO pude cerrar

**Orden:** t100 · **Perfil:** Backend · **Intento:** 1 · `130b5069-6330-4f3e-83ef-6ed8654146fd`
**SHA_BASE:** `main` = `327433332732b06975dccadc725940cbeaccd4ca` (medido con `git ls-remote`) · **Modo:** paper. Sin firma, sin broadcast, sin capital. **NO mergeé, NO desplegué, NO reinicié servicios.**

---

## 0. VEREDICTO CORTO

| | |
|---|---|
| **La causa NO quedó cerrada.** No hice caer la clase, y **no afirmo que el bug esté arreglado.** | |
| Lo que **sí** entregué | discriminación con evidencia, dos defectos reales corregidos, y la instrumentación que hace la clase **auto-diagnóstica** |

---

## 1. G1 — NO re-apliqué el fix viejo (y lo verifiqué)

`balance_slot` (`signer_funding.rs:270-280`) usa `ethers::abi::encode` y su test fija el vector **externo** `cast index address 0x1111…1111 9` → `233b1b49de63438bb1ac1a57ef81babcc52ccd4555c968bb144593ea539bbebc`. **El padding YA está arreglado.** El `706/706` del comentario es **pre-arreglo**. **No toqué `balance_slot`.** La misma cadena de razón sobrevivió a su propio arreglo describiendo **otra** causa — que es exactamente lo que esta orden vino a buscar.

## 2. G2 — la discriminación, con la evidencia que tengo (y la que NO tengo)

**Lo que MEDÍ, read-only, contra el anvil vivo:**

```
CONTROL  eth_blockNumber            -> {"result":"0x18eed00"}          (canal OK, bloque 26.140.928)
DECISIVO eth_call balanceOf(DAI, 0x1234…7890)
                                    -> 0x000000000000000000000000000000000000000000000000078e41566f0bc748
```

Ese resultado son **32 bytes bien formados y NO cero** (≈ 0,546 DAI). **⇒ la lectura FUNCIONA.** El `Eth_call` no revienta, no revierte, y devuelve un valor decodificable.

**Y `arbx_sim_funding_total` en el contenedor vivo:**
```
arbx_sim_funding_total{outcome="slot_unresolved"} 1633   (y 1642 en la lectura siguiente)
```
**SÓLO esa serie.** No hay `rpc_err`, ni `seeded_fresh`, ni `cache_hit`. (Y `balance_of` **se traga** los errores de `eth_call` como `Ok(None)` **sin contar** — por eso una lectura fallida era invisible y se veía igual que un slot equivocado.)

**De ahí, dos hechos firmes:**
1. **`write_balance` NO devolvió `Err`.** Si lo hubiera hecho, el `?` de `:137` habría propagado **su propio mensaje** (`funding_setstorage_rpc: …` / `anvil_setStorageAt_timeout`) y `ensure_funded` habría retornado ESE error. El `fail_reason` observado es `sim_signer_funding_slot_unresolved`, que sólo se produce al **agotarse el loop** (`:175-176`).
2. **La lectura no falla** (control + valor real). El `None` silencioso **no** es la explicación para DAI.

**⇒ Con los dos hechos, el loop se agota porque el centinela escrito en los 4 `CANDIDATE_SLOTS` (`[0,2,3,9]`) NO se reproduce en `balanceOf`.**

**QUÉ NO PUDE ESTABLECER, y lo digo claro:** no pude excluir concluyentemente **(a)** (`write_balance` → `Ok(false)` en los 4 slots) frente a **(b)** (la escritura se acepta pero el slot no es el que `balanceOf` lee). La evidencia **favorece (b)** — el saldo del firmante es **no cero**, y un centinela de desarrollo no debería tener DAI en un fork limpio — pero **no lo probé**. Para probarlo habría que escribir en el anvil vivo y leer de vuelta, y **mutar el estado del fork que usa producción no lo hago sin autorización**.

**Por eso no cierro G2 como cumplido.** La orden prohíbe reportar "cuál parece más probable", y eso es exactamente lo que tendría que hacer para marcarlo verde.

## 3. G3 — el `provider` y el fork: HIPÓTESIS, no hecho

**Hipótesis (a confirmar o refutar, NO establecida):** `SignerFunder.provider` (`Arc<Provider<Http>>` sobre `ANVIL_URL=http://anvil:8545`) y el `fork.acquire()` de `sim_engine` apuntan al **mismo** anvil. Medí la config (`ANVIL_URL=http://anvil:8545`, `SIM_BACKEND=anvil`, signer `0x1234567890123456789012345678901234567890`, el sentinela de desarrollo de `main.rs:57`) pero **no verifiqué si comparten INSTANCIA ni si el snapshot/revert envuelve la escritura**. Que `sim_engine.rs:95-113` documente que el fondeo debe ocurrir dentro de la ventana de snapshot es la razón de peso para tomar esto como **lo primero a medir**, no como una conclusión.

## 4. LO QUE SÍ CAMBIÉ — dos defectos reales, y la clase deja de mentir

`backend/sim-ctl/src/signer_funding.rs` (+`outcome::WRITE_REJECTED`, `VERIFY_MISMATCH`, `BALANCE_UNREADABLE`):

1. **`balance_of` ya no se traga el fallo en silencio.** Antes `Err(_) => Ok(None)` **sin contador**: una lectura fallida y un slot equivocado terminaban en el **mismo** `slot_unresolved`. Ahora la lectura ilegible tiene su propio outcome. **El comportamiento no cambia** (sigue `Ok(None)`): esto añade **visibilidad**, no un veredicto.
2. **El loop separa las dos causas físicas.** `Ok(false)` → `write_rejected` (causa **a**); write aceptado pero centinela no reproducido → `verify_mismatch` (causa **b**). **Eran el mismo string, y por eso la clase sobrevivió a su propio arreglo describiendo otra causa.**

⇒ **La próxima medición contesta (a) vs (b) sin ambigüedad**, en vez de inferirlo.

## 5. G5 — fail-closed intacto

No se tocó `max_slippage_for_pass_pct` ni `sim_engine.rs:153`. Un fallo de fondeo sigue siendo un fallo **TIPADO** (`Err(String)` → `not_implemented`), **nunca un `passed` silencioso ni una sonda sin fondear**. Cero filas escritas, cero veredictos fabricados.

## 6. G4 — la clase NO cayó, y no la hago caer con esta orden

`sim_signer_funding_slot_unresolved` = **1731** en las últimas 24 h (y **628/702 = 89,5 %** en el corte post-deploy del ancla). **Sigue ahí.** Con las 4 puertas en verde y **sin deploy**, este cambio **no mueve la métrica**: lo único que cambia en producción hasta el deploy es **cero**.

**Corte temporal, no agregado:** confirmo la advertencia — en la tabla entera `strategy_cyclic_route_not_simulatable_in_s4:*` sigue mostrando **617.570 filas (99,9 %)** mientras su corte posterior al deploy es **0**. **El agregado miente.** Toda cifra de este informe es de corte.

## 7. G6 — las 4 puertas, con la instrumentación exigida

```
command -v cargo   -> /home/hfrc/.cargo/bin/cargo   CARGO_WHICH_EXIT=0
cargo --version    -> cargo 1.91.0 (ea2d97820 2025-10-10)   EXIT=0
md5 ANTES de correr (WSL == Windows): ffe55569471e8760e6c90c73aa60ab10
```

| puerta | exit code REAL | evidencia |
|---|---|---|
| `cargo fmt --all -- --check` | **FMT_EXIT=0** | sin diferencias |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **CHECK_EXIT=0** | ver §7.1 |
| `cargo test -p sim-ctl --no-fail-fast` | **T3_EXIT=0** | 0 failed (8 · **55**/1ign · 1 · 8/1ign · 9 · 0) |
| `cargo test -p sim-core --no-fail-fast` | **T4_EXIT=0** | 0 failed (79 · 3/1ign · 0) |

### 7.1 HALLAZGO SOBRE LA PROPIA INSTRUMENTACIÓN DE G6

G6 exige **la línea `Checking <crate>`** y **NO aceptar el `Finished`**. La corrí, y el `grep` dio **vacío** — con `CHECK_EXIT=0`. **Eso parecía el fallo que G6 anticipa** (cargo declarando `Fresh` sobre fuente modificada).

**No lo era, y lo verifiqué antes de reportarlo:**
```
grep -n "sim-ctl" /tmp/chk.log   ->  494:   Compiling sim-ctl v0.1.0 (/home/hfrc/arbx100/backend/sim-ctl)
```
En una compilación **completa desde cero**, cargo emite **`Compiling sim-ctl`**, no `Checking sim-ctl`. **El requisito literal habría dado un FALSO NEGATIVO sobre un build limpio.** Forcé un re-check con `touch` y ahí sí apareció:
```
1:  Checking sim-ctl v0.1.0 (/home/hfrc/arbx100/backend/sim-ctl)
2:  Finished `dev` profile ... in 1.36s        CHECK2_EXIT=0
```
**Seguí la INTENCIÓN de G6 —probar que el crate fue realmente procesado— y conseguí evidencia más fuerte que el chequeo literal:** el crate se **compiló** en el build limpio (línea 494) **y** un re-check forzado lo volvió a procesar con exit 0. **Regla corregida para el equipo:** exigir que el crate aparezca como `Compiling` **o** `Checking`, no sólo `Checking`.

## 8. G7 — relación con PR #848 (la cota de t97)

**PR #848** (`feat/simctl-bound-01`, base `main`, head `d1bcc2e9…`, **8 archivos**): `anvil_backend.rs`, `consumer.rs`, `persistence.rs`, **`sim_engine.rs`**, `simulator_backend.rs`, `tx_builder.rs`, `docs/backend/SIM4-CYCLIC-01.md`, `docs/sre/SIMCTL-BOUND-01.md`. **Estado OPEN, `mergedAt=null`.**

- **Comparte base conmigo: SÍ — ambos sobre `main`.** Mi rama `fix/sim-fund-02` se apoya en **`main` = `327433332732b06975dccadc725940cbeaccd4ca`**, y lo declaro.
- **NO duplica archivos en vuelo:** `#848` **no incluye `signer_funding.rs`** — mi único archivo de código. **Mi cambio no toca ninguno de sus 8.**
- **Riesgo declarado:** `#848` **sí toca `sim_engine.rs`**, que está en mi `inScope` pero **que NO modifiqué** precisamente por eso. Si t100 hubiera necesitado `sim_engine.rs`, había solapamiento.

---

*Discriminación hecha con evidencia read-only contra el anvil vivo: la lectura funciona y la escritura no yerra, así que el loop se agota porque el centinela no se reproduce en los 4 slots candidatos. NO cierro (a) vs (b) porque probarlo exigía mutar el fork de producción. Entrego dos defectos reales corregidos —el fallo de lectura silencioso y la fusión de dos causas físicas en un solo string— que hacen la clase auto-diagnóstica, más el hallazgo de que la instrumentación de G6, tomada al pie de la letra, da falso negativo. La clase NO cayó y no lo presento como si hubiera caído.*
