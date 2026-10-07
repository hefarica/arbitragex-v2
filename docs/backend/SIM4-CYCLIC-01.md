# SIM4-CYCLIC-01 — la ruta cíclica ya no es una negativa por nombre, y construye

**Orden:** t88 · **Perfil:** Backend · **Intento:** 2 · `d19358f5-dc09-44c5-a0db-853d332d813d`
**SHA_BASE:** `21d2039cc80b8c47c3eea6b223ce7e663da604fd` · **Modo:** paper, `ARBX_LIVE_EXEC_ENABLED=False`. Sin firma, sin broadcast, sin capital. **NO mergeé.**
**Toolchain:** WSL2, 1.91.0 (fijado por `rust-toolchain.toml`), cero instalaciones.

> **Nota de continuidad:** el intento 1 de esta orden está en el historial de este mismo archivo (`docs/backend/SIM4-CYCLIC-01.md`). Aquel entregó el reconocimiento y **no** el código, deliberadamente. Este intento tenía presupuesto fresco y **cerró el código**. Las dos correcciones de premisa del intento 1 (M2: `sim_multistep` es FLASH, no ciclo DEX; M3: los hops YA están en `route_metadata`) **gobiernan esta implementación** y fueron aceptadas por el capitán.

---

## 1. LAS CUATRO PUERTAS — corridas, en verde

| puerta | resultado |
|---|---|
| `cargo fmt --all -- --check` | **exit 0 — limpio** (`FMT_CLEAN`) |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **`Finished dev profile … in 2m 26s`** — cero errores, cero warnings |
| `cargo test -p sim-ctl --no-fail-fast` | **0 failed**: 52 passed/1 ignored (bin) · 8 · 1 · 8/1 ignored · 8 · 0 |
| `cargo test -p sim-core --no-fail-fast` | **0 failed**: 79 passed · 3 passed/1 ignored · 0 |

## 2. EL CAMBIO — `tx_builder.rs` + `sim_engine.rs`

**Lo que NO hizo falta:** ningún contrato nuevo, ningún encoder nuevo, ninguna dependencia. `encode_v2` ya emitía `swapExactTokensForTokens` con `address[] path` — **sólo tenía los 2 elementos cableados a `[token_in, token_out]`**. Un ciclo de N patas es la MISMA llamada con `path = [A, X1, …, Xn, A]`.

**`backend/sim-ctl/src/tx_builder.rs`:**
- Variante `CyclicRouteNotRepresentable` **reemplazada** por `CyclicRouteMissingPath` — su mensaje viejo afirmaba una imposibilidad topológica **falsa**.
- `build_probe(opp, signer_from)` intacto: delega con `&[]` (punto de entrada estable para quien no tiene ruta).
- **`build_probe_with_path(opp, signer_from, path)`** — la entrada nueva. Con `token_in == token_out` exige `path.len() >= 3` **y** que el path empiece en `token_in` y cierre en `token_out`; si no, `CyclicRouteMissingPath`.
- `encode_v2_path(path, …)` — mismo selector `0x38ed1739`, array completo.
- `encode_v3_exact_input(path, …)` — `exactInput` `0xc04b8d59` con el path empaquetado `token(20)‖fee(3)‖…`. El fee por pata es `DEFAULT_UNIV3_FEE`, **la misma simplificación declarada que el encoder single-hop ya hace** (probar 500/3000/10000 y elegir el mejor quote sigue diferido a S5).

**`backend/sim-ctl/src/sim_engine.rs`:** el `Err` pasa a mapearse a una familia de razones **PROPIA**:
```
cyclic_route_missing_route_metadata:<kind>
```
**Ya no se emite `strategy_cyclic_route_not_simulatable_in_s4`.** Reutilizarlo habría seguido publicando un veredicto falso sobre 50.496 filas.

## 3. A1 — PROBADO POR TEST, no por lectura

Cuatro tests nuevos, los cuatro **en verde**:

```
test tx_builder::tests::cyclic_two_leg_route_builds_executable_probe ... ok
test tx_builder::tests::cyclic_two_leg_route_builds_on_v3 ... ok
test tx_builder::tests::cyclic_route_without_path_has_its_own_reason ... ok
test tx_builder::tests::cyclic_route_with_inconsistent_path_is_refused ... ok
```

El de A1 construye un ciclo de 2 patas `[A, B, A]` y afirma sobre el **ProbeTx real**:
- `from` = signer, `to` ≠ 0, `data` no vacío, selector `0x38ed1739` (V2) / `0xc04b8d59` (V3);
- el array de ruta encodado lleva **A, B, A en ese orden** (localiza las palabras ABI de 32 bytes y verifica `first_a < first_b < last_a`);
- **A aparece DOS veces** — que es lo que hace que la ruta sea cerrada.

`cyclic_route_without_path_has_its_own_reason` prueba que sin ruta se emite la razón NUEVA, no la vieja. `cyclic_route_with_inconsistent_path_is_refused` prueba que un path que empieza bien pero **no cierra** se rechaza en vez de encodear un swap equivocado.

## 4. A2 — EL FAIL-CLOSED QUEDA INTACTO

`backend/sim-ctl/src/persistence.rs` **NO se tocó** (`persistence.rs:131-135` sin relajar) y **`max_slippage_for_pass_pct` no se tocó**. `passed=true` sigue exigiendo `eth_call` + gas + decode + umbral. El cambio sólo hace la ruta **REPRESENTABLE**; no toca el veredicto. **No se relajó ningún umbral.** Los dos archivos tocados son `tx_builder.rs` y `sim_engine.rs`.

## 5. ⚠️ EL HUECO DE ALCANCE — por qué la producción todavía no ejecuta ciclos

**Esto es lo que hay que leer antes de celebrar.**

Los cuatro gates están verdes y A1 está probado **a nivel de builder**. Pero `SimEngine` **no tiene `PgPool`** (sólo `fork`, `signer_from`, `timeout`, `max_slippage_for_pass_pct`, `funder`), así que `sim_engine.rs` **no puede leer `route_metadata` por sí mismo**. Y el único llamador de `simulate` es **`backend/sim-ctl/src/consumer.rs`, que está FUERA del `inScope` declarado**.

⇒ **Para que una oportunidad cíclica REAL ejecute contra el fork falta un paso que no puedo dar dentro del alcance autorizado:** en `consumer.rs`, leer `route_metadata.token_addresses` (con el lector que YA existe — `route_lookup::fetch_candidate_inputs`, que `consumer.rs:534` ya usa en el camino B2c), parsearlo a `Vec<Address>` y pasarlo a `tx_builder::build_probe_with_path`.

Hoy, en producción, el llamador sigue invocando `sim_engine` sin ruta ⇒ los ciclos siguen cayendo en `not_implemented`, **pero ahora con la razón HONESTA `cyclic_route_missing_route_metadata:<kind>`** en vez de la etiqueta falsa. **La negativa por nombre está eliminada; la ejecución end-to-end necesita ese call site.** No lo toqué porque `consumer.rs` está fuera de alcance y no amplío alcance por mi cuenta. **Es un cambio de una función.**

## 6. A3 — LA AUSENCIA, CON MOTIVO PROPIO Y RUTA DE RESOLUCIÓN

La ausencia de `arbx:validated_plan:{id}` **no se reutiliza como veredicto**. La razón nueva nombra el dato que falta (`route_metadata`), no una imposibilidad topológica, y el comentario en `sim_engine.rs` lleva la ruta de resolución escrita: el path ya lo persiste `searcher-rs` (`build_route_metadata_from_plan`, `dex_engine.rs:1762`) y ya lo lee `route_lookup.rs:113-119`.

## 7. A6 — FAIL-HONEST, DECLARADO SIN MEDIR

**Hacer la ruta representable NO la hace rentable.** `t48` midió que el rechazo económico está **10,82× corto** contra el hurdle. Cuando el hueco de §5 se cierre y los ciclos ejecuten de verdad, **si el mejor caso sigue dando `simulated_profit_usd` negativo, eso es un veredicto de MERCADO medido** y debe reportarse tal cual — **sin mejorar el número y sin subir el sizing**. Este cambio compra **flujo EVALUABLE, no ganancia**.

## 8. LO QUE NO HICE

Cero mocks, cero hardcode, **cero filas escritas**. **NO toqué el checkout compartido** (todo en `%TEMP%\arbx-t88`; sin `git add -A`/`reset`/`stash` ahí). **NO mergeé, NO empujé a `main`, NO desplegué.**

---

## 9. SIM4-CYCLIC-02 (t96) — N1 CERRADO: el call site ya está

El §5 declaraba el hueco. **Ahora está cerrado, en el MISMO PR #846** (un solo ciclo de deploy, por la razón de N2: si N1 fuera en otro PR, el primer deploy sería un no-op garantizado sobre la métrica).

**`consumer.rs`** — en el camino legacy (`None =>`, tras `consumer.rs:426`), ANTES de despachar al backend:
```rust
let route_path: Vec<ethers::types::Address> =
    match route_lookup::fetch_candidate_inputs(&self.pool, opportunity.id).await {
        Ok(Some(inputs)) => inputs.route_metadata.token_addresses.iter()
            .filter_map(|s| s.parse::<ethers::types::Address>().ok()).collect(),
        Ok(None) => Vec::new(),
        Err(e) => { warn!(event = "sim_consumer.route_metadata_read_err", id = %id, error = %e); Vec::new() }
    };
let sim = match self.backend.simulate_with_route(&opportunity, &route_path).await { ... };
```
Es **el lector que YA existía** (`route_lookup::fetch_candidate_inputs`, el mismo que el camino B2c usa más abajo). **No es una capacidad nueva.**

**La ruta hasta el builder (3 archivos más):** `simulator_backend.rs` añade `simulate_with_route` **con cuerpo por defecto** (un backend que no se adapta hereda el comportamiento anterior **exacto** — por eso `revm_backend.rs` NO se toca); `anvil_backend.rs` la sobrescribe y la reenvía a `SimEngine::simulate_with_route` (nuevo en `sim_engine.rs`), que finalmente llama a `build_probe_with_path`.

**S2 — el fallo tipado ya estaba y sigue:** ruta ausente o que no cierra ⇒ `CyclicRouteMissingPath` ⇒ `cyclic_route_missing_route_metadata:<kind>`. **Nunca una sonda fabricada. Nunca un `passed` silencioso.** Si la lectura de `route_metadata` falla, se pasa ruta **VACÍA** (best-effort) y decide el mismo camino tipado.

**S3:** `strategy_cyclic_route_not_simulatable_in_s4` ya no se emite (desde t88) y esta rama lo confirma.

**Fallo de puerta corregido (honesto):** `-D warnings` cazó **`build_probe` is never used** — verdadero: al pasar `sim_engine` a `build_probe_with_path`, `build_probe` se quedó **sin llamador de producción** (sólo lo usan los tests). Se declaró con **`#[cfg(test)]`**, NO con `#[allow(dead_code)]`, que habría escondido el aviso detrás de una afirmación falsa.

**Las cuatro puertas, re-corridas sobre el código con `md5` verificado (`0fe96b71…` ambos lados):**

| puerta | resultado |
|---|---|
| `cargo fmt --all -- --check` | **exit 0 — limpio** |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **`Finished dev profile` in 11.91s** — 0 errores, 0 warnings |
| `cargo test -p sim-ctl --no-fail-fast` | **0 failed** (8 · 52/1ign · 1 · 8/1ign · 8 · 0) |
| `cargo test -p sim-core --no-fail-fast` | **0 failed** (79 · 3/1ign · 0) |

> **Nota de método, porque casi reporto basura:** una primera corrida de estas puertas dio todo verde pero era **inválida** — el `cp` de sincronización falló en silencio (`cp: -r not specified`), así que `cargo check` terminó en **3.13s** sobre código **sin cambios**. Se detectó por el tiempo y por el mensaje de `cp`, se re-sincronizó y **se verificó por `md5sum` en ambos lados** antes de volver a correr. Las cifras de arriba son de esa corrida verificada.

**S7 — N2, sin adornos:** este cambio **NO mueve** `simulations.passed` ni `simulated_profit_usd` en producción **por sí solo**. Eso requiere **mergear y desplegar**, y se mide en **t93**, no acá. Lo único que cambia en producción hasta el deploy es: **cero**, porque el contenedor sigue corriendo el código viejo.

**A6 vigente:** representable **≠** rentable (`t48`: 10,82× corto). Si tras el deploy el mejor caso sigue dando negativo, **es un veredicto de mercado medido** — sin mejorar el número y **sin subir el sizing**.

---

## 10. SIM4-CYCLIC-03 (t99) — F-01: el renombre había dejado fuera al CLASIFICADOR

Hallazgo **F-01 [MEDIA]** de la verificación independiente **t91**, y es un daño **que yo mismo introduje** en t88: renombré la familia del gap cíclico de `strategy_cyclic_route_not_simulatable_*` a `cyclic_route_missing_route_metadata:<kind>` **y no actualicé el clasificador**.

**El hueco, medido en el código:** `persistence.rs:130-179` `is_sim_capability_gap` reconocía `strategy_cyclic_route_not_simulatable` (`:135`) pero **no la familia nueva**, y **no tiene catch-all**. El diff de #846 tenía **0 menciones** de `is_sim_capability_gap`.

**Por qué importaba aunque hoy su impacto sea 0:** un gap de CAPACIDAD habría pasado a clasificarse como fallo de CALIDAD y la oportunidad se habría **flipeado a `rejected`** — exactamente lo que el comentario **SIMWIRE-02** prohíbe, y **peor que la negativa anterior**, porque el rechazo no deja rastro de la causa. El impacto era 0 sólo porque el arreglo todavía no producía rutas simulables: **se activaba en el mismo instante en que el call site empezara a funcionar.** Mergear sin este cierre era cambiar una pared por otra.

**El fix** (`persistence.rs`, +14): una cláusula nueva
```rust
|| fail_reason.starts_with("cyclic_route_missing_route_metadata")
```
**Las DOS familias quedan reconocidas** — la vieja por las filas históricas, la nueva por lo que produce el arreglo — con el comentario que explica por qué existen ambas.

**F2 — el test que MUESTRA la inversión**, en el módulo `simwire02_classifier_tests`:
```
test persistence::simwire02_classifier_tests::sim4_cyclic_renamed_gap_family_is_still_a_gap ... ok
```
Afirma que la familia NUEVA (4 variantes por kind) **y** la vieja son gaps, y cierra con un **CONTROL**: `v3_quote_unavailable`, `single_pool_no_spread`, `non_positive_profit`, `safety_below_threshold`, `simulation_failed`, `score_below_min` **NO** son gaps. Sin ese control, el test pasaría igual si el clasificador devolviera `true` para todo — es decir, si hubiera dejado de distinguir. **Un test que no puede fallar no prueba nada.**

**F5 — puntería corregida por el capitán:** el fail-closed REAL **no** es `persistence.rs:131-135` (eso es el clasificador); es **`sim_engine.rs:153`**: `passed = slippage_pct.is_some_and(|s| s <= self.max_slippage_for_pass_pct)`. **No se tocó.** Los archivos de este cierre son `persistence.rs` y este documento: `sim_engine.rs`, `tx_builder.rs`, el trait, `anvil_backend.rs` y `consumer.rs` quedan **intactos**.

**F6 — puertas y la regla ★ aplicada otra vez:** md5 idéntico WSL↔Windows **antes** de correr (`fc1ac48e…`), `touch` forzado, y **`Checking sim-ctl v0.1.0` confirmado en la salida** (7.16 s) antes de creerle al `Finished`. Gate 1 encontró **una** diferencia (una línea en blanco de más que introduje al insertar el test): corregí **sólo eso** y re-corrí **sólo esa puerta** — que pasó a `FMT_CLEAN`.

| puerta | resultado |
|---|---|
| `cargo fmt --all -- --check` | **FMT_CLEAN** |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **`Checking sim-ctl v0.1.0` … `Finished` in 7.16s** — 0/0 |
| `cargo test -p sim-ctl --no-fail-fast` | **0 failed** (8 · 53/1ign · 1 · 8/1ign · 9 · 0) |
| `cargo test -p sim-core --no-fail-fast` | **0 failed** (79 · 3/1ign · 0) |

**F7 — N2:** este cierre **no mueve la métrica**; **la habilita sin romperla**. Hasta el deploy, `simulations.passed` sigue en 0 y `simulated_profit_usd` en NULL. Se mide en t93.

---

*Negativa por nombre eliminada y probada por test en cuatro casos (V2, V3, sin ruta, ruta que no cierra); el call site de N1 cerrado en el mismo PR; y el clasificador de capability-gap devuelto a reconocer la familia renombrada, con control que prueba que sigue distinguiendo. Un solo PR, un solo ciclo de deploy; cuatro puertas en verde sobre código verificado por `md5`; fail-closed real (`sim_engine.rs:153`) y umbrales intactos. Lo que queda es desplegar y medir en t93.*
