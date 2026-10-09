# DECIMALS-CYCLE-01 (t197) — el ciclo deja de defaultear `decimals` a 18

**Tarea:** t197 · **Alcance de escritura:** `docs/loop/` · **Código:** rama + PR **sin merge**
**Base medida:** `fix/perhop-reserves-01` HEAD **`858b943fd80c8b5e1606d220aa87d63b86f4ba15`** (árbol `622d32c0`); `origin/main` **`4902a47c16ef21b7683e5bf405ab49cd1c525b77`** al medir.
**Régimen:** paper, `ARBX_LIVE_EXEC_ENABLED=False`, capital expuesto 0, cero firma, cero broadcast, cero escritura al fork.

> **Regla de este documento:** cada número cita su artefacto. Lo que no se midió se declara como no medido.
> Lo que se cita de t187 y no reproduje yo, se marca **CITADO (no re-medido)**.

---

## 1. EL LOCUS — dónde el ciclo defaultea a 18

**`backend/searcher-rs/src/orchestrator.rs`, líneas 1472-1484 (858b943f) — el `decimals.map` del ciclo:**

```rust
1472:            {
1473:                let mut m = std::collections::HashMap::new();
1474:                for addr in &chosen.token_addresses {
1475:                    let lc = addr.to_lowercase();
1476:                    m.insert(
1477:                        lc,
1478:                        crate::engines::dex_engine::canonical_token_decimals_str(addr),
1479:                    );
1480:                }
1481:                if !m.is_empty() {
1482:                    chosen.decimals = shared_rs::candidates::DecimalsMap { map: m };
1483:                }
1484:            }
```

El valor sale de `canonical_token_decimals_str` (`backend/searcher-rs/src/engines/dex_engine.rs:890-895`),
que delega en la tabla canónica **hardcodeada** `canonical_token_decimals` (`dex_engine.rs:873-884`):

```rust
873: fn canonical_token_decimals(token: Option<Address>) -> u32 {
874:     let Some(addr) = token else { return 18 };
875:     match format!("0x{:040x}", addr).as_str() {
877:         USDC_MAINNET_LC => 6,
878:         USDT_MAINNET_LC => 6,
880:         "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => 8,   // WBTC
882:         _ => 18,                                             // <<< EL DEFAULT
883:     }
884: }
```

**La tabla tiene 3 entradas.** Todo lo demás —incluidos tokens con valor real ya medido en PostgreSQL y en el
catálogo de Redis— cae en `_ => 18`. Ese `18` es el "default vendido como dato" que t187 midió como **8 de 37
entradas del mapa discrepantes**, siempre 18.

En `origin/main` (4902a47c) el mismo bloque vive en **`orchestrator.rs:1635-1647`** con el texto **idéntico**
(verificado con `git show origin/main:… | sed -n '1635,1647p'`), y la tabla en `dex_engine.rs:1187`. El locus no
se movió de forma: solo cambió de número de línea.

## 2. POR QUÉ ESE 18 CONTAMINA EL VEREDICTO ECONÓMICO (mecanismo, con 3 citas)

1. `backend/shared-rs/src/candidates.rs:73-89` — `DecimalsMap { map: HashMap<String, u8> }`, claves en minúscula.
2. `backend/sim-ctl/src/route_lookup.rs:60-71` — `merge_decimals(rm, token_rows)` parte de las filas REALES de
   `tokens` y **superpone encima el mapa persistido**:
   ```rust
   67:    // Overlay persisted route_metadata decimals last: the row's own claim wins.
   68:    for (addr, decimals) in rm.map.iter() { merged.insert(addr.clone(), *decimals); }
   ```
   ⇒ **el 18 del searcher PISA el valor correcto** que sim-ctl ya había leído de PostgreSQL.
3. `backend/sim-ctl/src/sim_runner.rs:109-135` — `CandidateDecimalsProvider` es el `TokenDecimalsProvider` del
   encoder y se alimenta **del mapa del candidato**: *"sim-ctl does NOT need a PG connection to resolve decimals —
   it uses what the candidate already provides"*. Con `18` donde el token tiene 6, la escala es `10^18` en vez de
   `10^6` ⇒ **error de unidades de clase 1e12** (EURC) y 1e10 (los de 8) ⇒ un tamaño que no funciona.

**Y el caso negativo ya estaba especificado por el propio consumidor** (`sim_runner.rs:114-115`): *"Fail-honest:
returns `None` for tokens not in the map (the encoder will reject with `MissingDecimals`)"*. Es decir: **mapa sin
entrada ⇒ rechazo con reason explícito**, no una unidad inventada. La reparación no inventa semántica: alimenta el
camino que el consumidor ya documenta.

## 3. LOS 8 CASOS DE t187 — valor real, con las TRES fuentes citadas

`eth_call` **read-only** (`cast call <token> "decimals()(uint8)" --rpc-url https://ethereum-rpc.publicnode.com`,
ejecutado en el VPS; cero firma, cero broadcast, cero escritura — es la llamada on-chain que exige la aceptación):

| # | token | símbolo | on-chain `decimals()` | PG `tokens.decimals` | Redis `arbx:tokens:1:<addr>` | el ciclo ponía |
|---|---|---|---|---|---|---|
| 1 | `0x1abaea1f7c830bd89acc67ec4af516284b1bc33c` | EURC | **6** | 6 (`onchain_full`) | `{"decimals":6}` | 18 |
| 2 | `0xa1f410f13b6007fca76833ee7eb58478d47bc5ef` | RJV | **6** | 6 | `{"decimals":6}` | 18 |
| 3 | `0xac51066d7bec65dc4589368da368b212745d63e8` | ALICE | **6** | 6 | **sin fila** | 18 |
| 4 | `0x2b591e99afe9f32eaa6214f7b7629768c40eeb39` | HEX | **8** | 8 | `{"decimals":8}` | 18 |
| 5 | `0x72e4f9f808c49a2a61de9c5896298920dc4eeea9` | BITCOIN | **8** | 8 | `{"decimals":8}` | 18 |
| 6 | `0x14fee680690900ba0cccfc76ad70fd1b95d10e16` | $PAAL | **9** | 9 | `{"decimals":9}` | 18 |
| 7 | `0x95af4af910c28e8ece4512bfe46f1f33687424ce` | MANYU | **9** | 9 | `{"decimals":9}` | 18 |
| 8 | `0xa606d433971e9ee140e234daa7c94c476e10ead1` | CLAUS | **9** | 9 (`is_active=f`) | **sin fila** | 18 |

Artefactos: `cast call` (salida literal `6 6 6 8 8 9 9 9`), `psql SELECT address,symbol,decimals,is_active,resolved_via,chain_id
FROM tokens WHERE address LIKE …` (8 filas, las 8 con `resolved_via=onchain_full`), y
`redis-cli GET arbx:tokens:1:<addr>` para cada uno.

**Dato que decide el diseño:** el catálogo de Redis tiene **2503** claves `arbx:tokens:1:*`, pero **2 de los 8**
(ALICE y CLAUS) **no están** en él. Por eso el resolutor consulta **las dos fuentes** — Redis primero, PostgreSQL
(`tokens.decimals`) para el resto — y no solo Redis. Un resolutor que leyera solo Redis fallaría 2 de los 8 casos.

## 4. LA REPARACIÓN (rama + PR, sin merge)

`backend/searcher-rs/src/orchestrator.rs`
- **`CycleDecimals { map, unresolved }`** + **`build_cycle_decimals_map(addrs, redis_decimals, pg_decimals)`**:
  política **pura**, unit-testeable, sin I/O. Un token en Redis **o** en PG entra con su valor real; un token en
  **ninguna** de las dos va a `unresolved` y **no recibe entrada** (nunca 18).
- **`Orchestrator::resolve_cycle_decimals(&self, addrs, chain_id)`**: lee el catálogo Redis
  (`crate::reserves::get_token_meta`) y, para lo que Redis no responde (o falla), el provider PG
  (`self.ctx.token_decimals_provider`). Nunca sintetiza.
- **Call site (1472-1484) sustituido**: si `unresolved` está vacío se instala el mapa real; si no, **el mapa no se
  instala** y se emite `orchestrator.decimals_unresolved` con `reason =
  "decimals_not_in_redis_token_catalog_nor_pg_tokens_decimals"` y la lista de tokens sin unidad.
- **Nuevo campo de contexto** `token_decimals_provider`, tipado por la ruta de **lib**
  (`Option<Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>` — `crate::scanner` es módulo del BIN
  y no es alcanzable desde la lib; `lib.rs:182` re-exporta `sim_core::sim_encoder`).
- **Tests** (§5).

`backend/searcher-rs/src/scanner.rs` — el provider ya existía en el proceso (`run_chain` lo recibe desde
`chain_supervisor`); se **hila** hasta `build_orchestrator` (parámetro nuevo + 2 call sites) y de ahí al contexto.

`backend/searcher-rs/tests/{v2_shadow_replay,cartridge_shadow_replay,orchestrator_parallel_run}.rs` — los 3
literales de `OrchestratorContext` reciben `token_decimals_provider: None`.

**Diff auditado, archivo por archivo** (`git diff --numstat` entre el árbol base `858b943f` y el parcheado):

| fichero | líneas + | líneas − |
|---|---|---|
| `backend/searcher-rs/src/orchestrator.rs` | 325 | 15 |
| `backend/searcher-rs/src/scanner.rs` | 10 | 0 |
| `backend/searcher-rs/tests/v2_shadow_replay.rs` | 1 | 0 |
| `backend/searcher-rs/tests/cartridge_shadow_replay.rs` | 1 | 0 |
| `backend/searcher-rs/tests/orchestrator_parallel_run.rs` | 1 | 0 |
| **total (5 ficheros)** | **338** | **15** |

Ningún otro fichero se toca: `backend/shared-rs/`, `backend/sim-ctl/`, `backend/prioritization-spine/`, `docker/`,
`contracts/`, `frontend/` quedan **intactos**.

**Sin cambios en `shared-rs` ni en `sim-ctl`**: el consumidor ya da precedencia al claim de la fila y ya rechaza
con `MissingDecimals` cuando falta la entrada; lo que estaba mal era **el valor del claim**.

## 5. EL FALSIFICADOR (mutante que restaura el 18 en el locus)

Tres tests en `orchestrator.rs` (módulo de tests de la lib), que miden **las ENTRADAS del mapa**, token por token
—no el `token_in` de la oportunidad emitida— porque auditar `token_in` dio en t187 un **falso todo-OK
(discrepantes=0 contra 8 reales)**:

1. `decimals_cycle_t187_eight_cases_resolve_to_the_real_unit_not_18` — los 8 casos resuelven a **6/6/6/8/8/9/9/9**,
   ninguno a 18, y **en el mismo test** se afirma que la tabla canónica pre-fix **sigue devolviendo 18** para esos
   8 tokens (el defecto, reproducido: lo que cambió son las entradas del mapa).
2. `decimals_cycle_unknown_token_is_not_computed_never_18` — token sin fila en PG **y** sin entrada en Redis:
   **mapa vacío**, token en `unresolved`, **nunca 18**.
3. `decimals_cycle_source_precedence_and_route_order` — Redis manda sobre PG para el mismo token, PG respalda lo
   que Redis no tiene, y el orden de la ruta se preserva en `unresolved`.

**Mutante** (`mutate-decimals-cycle-01.py`): reemplaza el cuerpo del resolutor por el comportamiento **pre-fix**
(`canonical_token_decimals_str`, o sea el default 18). Mismo número de tests (no añade ni quita), distinta
implementación ⇒ si los tests siguen pasando con el mutante son vacuosos.

Corrida real (contenedor aislado `rust:1.91`; **tres árboles** desde `git archive` de `858b943f`: BASE sin parche,
FIX con parche, MUTANTE con el default 18 restaurado). Mismos comandos, mismo `CARGO_TARGET_DIR` con
re-fingerprint forzado:

| corrida | filtro `decimals_cycle` | suite **lib** | suite **bin** | `cargo check -Dwarnings` |
|---|---|---|---|---|
| BASE (sin parche) | `0 passed; 0 failed; 1426 filtered out` (exit 0 — **esos tests no existen**) | 1420 passed, **1 failed** | 1407 passed, **1 failed** | exit **0** |
| **FIX** | **`3 passed; 0 failed; 1426 filtered out`** (FOCAL_EXIT=**0**) | 1423 passed, **1 failed** | 1410 passed, **1 failed** | exit **0** |
| **MUTANTE** | **`0 passed; 3 failed; 0 ignored; 1426 filtered out`** (FOCAL_EXIT=**101**) | 1420 passed, **4 failed** | 1407 passed, **4 failed** | exit **0** |

**El mutante FALLA como debe, y por la razón correcta.** Literal de la corrida del mutante:

```
thread 'orchestrator::tests::decimals_cycle_t187_eight_cases_resolve_to_the_real_unit_not_18' panicked at
searcher-rs/src/orchestrator.rs:2687:13:
assertion `left == right` failed: EURC 0x1abaea1f7c830bd89acc67ec4af516284b1bc33c (redis+pg) debe entrar al
mapa con su unidad real 6

thread 'orchestrator::tests::decimals_cycle_unknown_token_is_not_computed_never_18' panicked at
searcher-rs/src/orchestrator.rs:2718:9

thread 'orchestrator::tests::decimals_cycle_source_precedence_and_route_order' panicked at
searcher-rs/src/orchestrator.rs:2754:9:
assertion `left == right` failed: Redis manda sobre PG
```

- **`CHECK_EXIT=0` en la corrida del mutante** ⇒ el mutante **compila** (`Finished dev profile … in 41.36s`): los 3
  fallos son fallos de **TEST**, no de compilación. Un mutante que no compila no probaría nada.
- **Mismo conteo de tests en las tres corridas** (lib `1420+1` = `1423+1` = `1420+4`; bin `1407+1` = `1410+1` =
  `1407+4`; **17 suites** en las tres): el mutante cambia una implementación, no añade ni quita tests.
- El filtro `decimals_cycle` sobre el árbol **sin** parche da `0 passed / 0 failed / 1426 filtered out` con
  **exit 0**: cero tests **no** es un PASS. Por eso el falsificador es el mutante y no una "corrida pre-fix" del
  mismo filtro.


## 6. VERIFICACIÓN (comandos del contrato, base `858b943f` + parche)

| comando del contrato | BASE (sin parche) | **FIX (con parche)** | MUTANTE |
|---|---|---|---|
| `cargo fmt --all -- --check` | exit **0** (0 ficheros con diff) | exit **0** (0 ficheros con diff) | exit 0 |
| `cargo fmt -p searcher-rs -- --check` | exit 0 | exit 0 | exit 0 |
| `RUSTFLAGS=-Dwarnings cargo check --manifest-path searcher-rs/Cargo.toml` | exit **0** | exit **0** | exit 0 |
| `cargo test --manifest-path searcher-rs/Cargo.toml --no-fail-fast` | exit **101** | exit **101** | exit 101 |

**La suite NO da 0 fallos — y no es por el parche: es PRE-EXISTENTE y está medido en el árbol SIN parche.** El
único fallo es el mismo test en los dos targets (lib y bin), y en las tres corridas:

```
route_discovery::route_discovery_worker::tests::xlang_golden_tick_contract
panicked at searcher-rs/src/route_discovery/route_discovery_worker.rs:2687:33:
golden full.json missing (No such file or directory (os error 2))
— run ARBX_REGEN_GOLDEN=1 cargo test -p searcher-rs xlang_golden
```

BASE `1420+1` / FIX `1423+1` (los 3 nuevos) / MUTANTE `1420+4` ⇒ el fallo pre-existente es **el mismo, aislado y
contable**. La fixture `full.json` **no está commiteada** (`git ls-tree -r --name-only HEAD | grep full.json` no
devuelve coincidencias) ⇒ falta por construcción en un árbol `git archive`, no por el parche. Quien quiera la
suite en verde necesita el fixture fuera del repo o `ARBX_REGEN_GOLDEN=1`; **eso no lo arreglo yo aquí y no lo
declaro arreglado**.

`cargo check` con `-Dwarnings` (el comando que el contrato exige que pase) da **exit 0 en los tres** árboles: el
parche compila con los warnings como errores.


Instrumento: **contenedor aislado `rust:1.91` en el VPS** con el árbol montado (`docker run --rm -v … -w /w/backend
rust:1.91`). Nada se instaló en el host del VPS; la única modificación del contenedor es
`rustup component add rustfmt` (la imagen no lo trae). Toolchain del contenedor **1.91.1** vs pin del repo
**1.91.0** (`rust-toolchain.toml`) — **declarado**: misma minor, patch distinto.

## 7. LO QUE ESTA REPARACIÓN **NO** REPARA (sin proximidad)

- **Las otras tres causas de t187** (CITADO, no re-medido por mí):
  - **índice V3 de Redis**: sigue sin **21** pools de `fee_tier` 30. Mis medidas propias del índice:
    `arbx:pool_index_v3:1:*` = **765** claves, `arbx:v3_slot0:1:*` = **434**, `arbx:pool_index:1:*` = **2033**.
    **No reproduje el diferencial exacto de 21**: se cita, no se hereda como medido.
  - **`fee_tier` NULL**: **2471 de 4280** filas de `pools` (mi query: `SELECT count(*) FROM pools` = 4280,
    `WHERE fee_tier IS NULL` = 2471). t187 citaba 2471 de 4279 ⇒ **coincide** (una fila nueva desde entonces).
    Ninguna de las dos se convierte en cero.
- **Segundo consumidor de la misma tabla, NO reparado aquí** — y **divergencia que introduzco y declaro**:
  `orchestrator.rs:2113-2126` (`row_token_decimals`, en `origin/main`) usa **la misma tabla canónica con default
  18** para convertir el ledger del sizing a unidades de token (`stamp_sized_figures`). Tras mi parche, el mapa
  dice 6 y ese camino sigue diciendo 18 para el mismo token: **los dos consumidores pueden discrepar** hasta que
  se repare con una fuente real (requiere hilar el provider hasta el sizing). Es una **divergencia declarada, con
  línea**, no un arreglo por proximidad. (En 858b943f ese helper no existe: es de main.)
- Otros dos call sites de `canonical_token_decimals_str` que **no** toco, con línea: `opportunity_emitter.rs:831`
  (sello del probe de la fila) y el probe interno de `dex_engine.rs` (251/294/781). Mismo defecto de clase, otro
  alcance.

## 8. RUTAS RETENIDAS POR TAREAS INALCANZABLES (hallazgo de harness)

Medido leyendo el estado del equipo (`team.json`, solo lectura):

| tarea | estado | `inScope` |
|---|---|---|
| **t124** | **cancelled** | `shared-rs/src/contracts.rs`, `backend/sim-ctl/tests/`, `docs/backend/` |
| **t128** | pending | `backend/sim-ctl/src/signer_funding.rs`, `…/sim_engine.rs`, `backend/sim-ctl/tests/`, **`docker/`**, **`docs/backend/`** |
| **t130** | pending | `backend/searcher-rs/src/strategy_label.rs`, `backend/prioritization-spine/src/config_aware.rs`, **`docs/backend/`** |
| **t134** | pending | `backend/shared-rs/src/contracts.rs`, **`backend/searcher-rs/src/`** (el árbol entero), `docs/backend/` |

**⇒ El PR NO PUEDE MERGEARSE, y por dos motivos independientes:**
1. `backend/searcher-rs/` está **retenido por t130** y `docs/backend/` por **t128**; ambas **pending** e
   inalcanzables porque su ancestro **t124 está cancelled** y *cancelled* cuenta como *unfinished* en el gate de
   dependencias ⇒ **el lock es un ladrillo permanente**: ninguna cancelación lo libera.
2. El contrato de esta tarea dice explícitamente `kind: implementation` con entregable = **patch y prueba, no el
   aterrizaje**, y su alcance de escritura es `docs/loop/`.

**No se disputa el lock: se elude** (el código viaja como rama + PR, no como escritura en el árbol compartido).

## 9. CERO CAMBIOS EN KNOBS / UMBRALES (query propia)

`redis-cli GET arbx:trading_config:1` (clave `arbx:trading_config:<chain_id>`, `shared-rs/src/trading_config.rs:31`),
leída en el runtime vivo:

```
capital_usd                  = 1000            ← intacto
min_profit_usd (target_net)  = 50              ← intacto
spread_sanity_mult (multiplicador) = 3         ← intacto
simulation_capital_usd       = None
updated_at                   = 2026-10-08T20:48:02.069Z   updated_by = admin   (no fui yo)
```

Los 34 campos están presentes. **Cero cambios a umbrales, knobs ni config**: mi parche no toca `shared-rs`, ni
configuración, ni `trading_config`, y el único fichero que escribo fuera de `docs/loop/` vive en la rama del PR.

## 10. INSTRUMENTOS CON DEFECTO PROPIO (declarados, no escondidos)

1. **Windows: Control de Aplicaciones bloquea la ejecución** de build scripts (`os error 4551` sobre
   `ring`'s build-script-build) ⇒ `cargo` no puede construir en Windows. Medido, no supuesto.
2. **WSL: sin linker** (`cc`/`gcc`/`clang` ausentes) y `sudo` pide contraseña ⇒ no se puede enlazar ahí.
3. **`CARGO_TARGET_DIR` compartido entre corridas con árboles distintos ⇒ falso positivo por mtime**:
   al cambiar de árbol (858b943f → main) con los mismos paths, cargo reutilizó artefactos del árbol anterior y
   produjo **45 errores fantasma** (`shared_rs::candidates::EconomicsBasis` "no existe" — existe en
   `origin/main:backend/shared-rs/src/candidates.rs:63`). Corregido forzando re-fingerprint (`touch` de los fuentes
   del workspace). **No reporto aquellos 45 errores como hallazgo sobre main**: eran mi instrumento.
4. **WSL borra `/tmp` entre invocaciones** (systemd-tmpfiles): los logs se perdieron en la primera pasada ⇒ logs a
   `$HOME/…` en las siguientes.
5. `rust:1.91` **no trae `rustfmt`** ⇒ `rustup component add rustfmt` dentro del contenedor.
6. El primer parche **no compilaba**: `E0425 cannot find value decimals_provider` en `scanner.rs` — `build_orchestrator`
   no recibía el provider. Corregido hilándolo (firma + 2 call sites). Se declara la primera pasada fallida.
7. **La copia inicial del árbol contenía WIP ajeno** (11 ficheros modificados sin commitear en `backend/searcher-rs`,
   p.ej. `v3_quote_provider.rs` +280/−61) y **no compilaba**. Se rehízo la verificación desde **`git archive HEAD`**
   (solo contenido commiteado) ⇒ la verificación corre sobre `858b943f` + mi parche, nada más.

## 11. LO QUE ESTE DOCUMENTO NO DECLARA

- **No** declara el PR mergeable (ver §8).
- **No** declara reparadas las otras causas de t187 (§7).
- **No** declara que el ciclo en producción esté ya corregido: el searcher desplegado es el de `77b42b3d`/`main`
  y esta reparación **no está desplegada**.
- **No** convierte en cero ninguna magnitud no medida: el diferencial de 21 pools V3 es **CITADO**, no re-medido.

---

## 12. ACTUALIZACIÓN t201 (DECIMALS-REBASE-01) — rebase sobre `main` + cierre de la divergencia

**Base nueva: `main` = `4077fea5dcca5d29212dc0d76e67f4fc20c66b9c`** (no `858b943f`), probado con
`git merge-base --is-ancestor 4077fea5dcca5d29212dc0d76e67f4fc20c66b9c HEAD`. El rebase fue **re-aplicación
mecánica** del transformador de t197 sobre un árbol `git archive origin/main -- backend`: **los 7 anclajes de t197
y los del fix-up casaron verbatim** ⇒ **cero archivos necesitaron resolución manual**. (Si un ancla no hubiera
casado, el script aborta sin escribir: no hay parche a medias.)

### 12.1 Los 3 tests del falsificador, RE-MEDIDOS sobre la base nueva

| corrida (base `main` 4077fea5) | filtro `decimals_cycle` | suite **lib** | suite **bin** | suites | `cargo check -Dwarnings` |
|---|---|---|---|---|---|
| BASE sin parche | `0 passed; 0 failed; 1717 filtered out` (exit 0: no existen) | 1708 passed, **1 failed**, 6 ignored | 1695 passed, **1 failed**, 6 ignored | 19 | exit 0 |
| **FIX** | **`ok. 3 passed; 0 failed; 1717 filtered out`** (FOCAL_EXIT=**0**) | 1713 passed, **1 failed**, 6 ignored | 1700 passed, **1 failed**, 6 ignored | 19 | exit **0** |
| **MUTANTE** | **`FAILED. 0 passed; 3 failed; 1717 filtered out`** (FOCAL_EXIT=**101**) | 1709 passed, **5 failed**, 6 ignored | 1696 passed, **5 failed**, 6 ignored | 19 | exit **0** (compila) |

- **Los números de t197 (1424 lib / 1411 bin / 17 suites) eran de OTRA base y NO se citan aquí como de esta:**
  sobre `main` son **1714 lib / 1701 bin / 19 suites** (FIX = MUTANTE = 1714 lib, 1701 bin ⇒ **mismo conteo
  total** en los dos lados; el mutante cambia una implementación, no añade ni quita tests).
- FIX `1713+1` vs BASE `1708+1` = **+5 passed** = los 3 tests de t197 **+ los 2 tests nuevos del cierre** (§12.2).
- `CHECK_EXIT=0` en la corrida del mutante ⇒ **compila**: los fallos son de TEST, no de compilación.

### 12.2 Cierre de la divergencia `row_token_decimals` (la deuda que t197 declaró)

En `main`, el ledger del sizing resolvía por la **tabla canónica con default 18** mientras el mapa del ciclo ya
resolvía por las fuentes reales: **el mismo token medido con dos unidades** (encoder 6, ledger 18). Ahora:

- `row_token_decimals(address, provider, chain_id)` resuelve por el **MISMO provider** que el mapa
  (`Option<Arc<dyn TokenDecimalsProvider + Send + Sync>`) y devuelve **`None`** — nunca 18 — cuando el provider no
  tiene fila o la dirección no parsea.
- `stamp_sized_figures(c, ledger, provider, chain_id)` recibe ese provider: **los dos fallbacks `1e18` fueron
  eliminados**. Sin unidad resuelta el notional queda **NO COMPUTADO** (`NaN`) con
  `orchestrator.sizing_units_unresolved` + `reason`, en vez de una escala inventada.
- `grep -n "1e18_f64" orchestrator.rs` = **sin coincidencias** tras el parche.
- El test `sized_sync_stamps_kernel_notional_and_output` tenía un bloque ("LEGACY ADDRESS SHAPE") que **afirmaba**
  el fallback 1e18 (`amount_in == 3.0`); se reescribió para afirmar el comportamiento nuevo (`is_nan()` y
  `assert_ne!(amount_in, 3.0)` porque *el fallback 1e18 ES la divergencia que t201 cierra*).
- Dos tests nuevos, **ambos `ok`** sobre la base nueva:
  - `row_decimals_resolves_through_the_provider_never_to_18` — resuelve por provider, y `None` (nunca 18) sin fila
    o sin provider; reproduce el defecto afirmando que la tabla canónica daría 18 para el token desconocido.
  - `sizing_ledger_and_cycle_map_agree_on_the_same_unit` — el MISMO token (EURC, 6 reales) por los DOS caminos: el
    mapa del ciclo y el ledger del sizing devuelven la misma unidad; sin provider, `NaN` (no 1e18).

### 12.3 El fallo pre-existente, re-medido en los dos lados ⇒ NO ATRIBUIDO

`route_discovery::route_discovery_worker::tests::xlang_golden_tick_contract` (`golden full.json missing`,
`route_discovery_worker.rs:2687:33`) aparece **2 veces en el árbol SIN parche y 2 veces CON parche** (una por
target: lib y bin) ⇒ **NO ATRIBUIDO al parche y NO declarado arreglado**.

**Y queda además EXPLICADO por medición — corrigiendo a t197, que lo dio por "fixture no commiteada":** la fixture
**sí está commiteada**, en `frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick/full.json`
(`git ls-tree -r origin/main --name-only` la lista; `git check-ignore` no la excluye), y `golden_fixture_dir()` la
busca en `CARGO_MANIFEST_DIR/../../frontend/…` — **fuera de `backend/`**. El árbol de verificación se construyó con
`git archive -- backend`, de modo que la fixture **no puede estar** ahí: el fallo es un **artefacto de alcance del
árbol**, no del código ni del parche. Lo confirma la vía independiente: en el mismo head (`d7a0f495`) el check de
CI **`Rust tests` = pass (1m20s)**. La afirmación de t197 era un artefacto de mi propio grep (`Select-Object
-First 15` truncó la lista y los aciertos de `.agents/skills/**` la taparon): **se corrige aquí**.

### 12.4 Lo que este PR NO puede hacer: aterrizar — con la razón MEDIDA, no heredada

El path lock de `backend/searcher-rs/` **ya no aplica**: `t130` y `t134`, que lo retenían, fueron **canceladas por
orden del operador**. Por tanto la incapacidad de aterrizar, si persiste, tiene que nombrarse con medición y no
heredarse. Lo medido está en el PR (estado de merge y checks); **la suite del propio proyecto queda en `exit 101`
por el fallo pre-existente de la fixture**, que es un gate de CI, no una razón de ruta.

### 12.5 Sin efectos sobre el dinero ni sobre el runtime

Cero firma, cero broadcast, cero escritura on-chain (la única llamada a mainnet sigue siendo el `eth_call`
read-only de `decimals()` de los 8 casos, medido en t197 y **base-independiente**: el contrato del token no cambia
con la base del repo). Knobs intactos por query propia sobre `arbx:trading_config:1`: **`capital_usd=1000`,
`min_profit_usd=50`, `spread_sanity_mult=3`**, 34 claves.
