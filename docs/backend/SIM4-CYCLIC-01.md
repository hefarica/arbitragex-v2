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

*Negativa por nombre eliminada y probada por test en cuatro casos (V2, V3, sin ruta, ruta que no cierra); cuatro puertas en verde; fail-closed, `persistence.rs` y umbrales intactos. El hueco está declarado con nombre y apellido: falta un call site en `consumer.rs` —fuera de alcance— para que los ciclos ejecuten de verdad; hasta entonces la razón publicada es la honesta en vez de la falsa.*
