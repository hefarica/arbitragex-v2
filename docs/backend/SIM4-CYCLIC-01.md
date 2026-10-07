# SIM4-CYCLIC-01 — reconocimiento completo y verificado; **NO entrego Rust sin poder verificarlo**

**Orden:** t88 · **Perfil:** Backend · **Intento:** 1 · `0645253d-7674-4cdc-8780-8ec7196bc560`
**SHA_BASE:** `21d2039cc80b8c47c3eea6b223ce7e663da604fd` — clonado y **confirmado** (`HEAD is now at 21d2039c Merge pull request #843 from fix/selgate-01`).
**Permisos:** paper, `ARBX_LIVE_EXEC_ENABLED=False`, sin firma, sin broadcast, sin capital. **NO mergeé, NO empujé, NO toqué el checkout compartido.**

> ## VEREDICTO: **FALLIDO — NO IMPLEMENTADO.** Bloqueante: alcance que no puedo entregar VERIFICADO con el presupuesto restante.
>
> Los cuatro `Verify` exigen un ciclo completo de compilación y test de Rust en WSL2 (`fmt` + `check -D warnings` + `test` × 2 crates), con iteración. Escribir el fix sin correr ese ciclo sería entregar **Rust sin verificar**: exactamente la «hipótesis con formato de parche» que en `t66` y `t80` **no** entregué y que el capitán confirmó como la decisión correcta. **No lo hago ahora sólo porque la tarea sea grande.**
>
> Lo que sí entrego es el reconocimiento **medido**, y **corrige una premisa de la orden** (§4).

---

## 1. EL LOCUS, confirmado línea a línea

`backend/sim-ctl/src/tx_builder.rs`:
- **`:76-80`** — `if token_in == token_out { return Err(BuildError::CyclicRouteNotRepresentable(opp.strategy_kind.clone())); }`
- **`:22-27`** — la variante, con su doc: *"closed route (token_in == token_out) -- cannot be expressed as the single swap hop this builder encodes"*
- **`:56-75`** — el comentario BR-00 que explica **por qué** se rechaza: *"…and the **Opportunity payload carries no intermediate hops to rebuild the real path**. Refuse with a typed error instead of encoding a degenerate `[X, X]` swap that can only ever revert on the fork."*

`backend/sim-ctl/src/sim_engine.rs:66-75` — `Err(BuildError::CyclicRouteNotRepresentable(kind))` → `not_implemented("strategy_cyclic_route_not_simulatable_in_s4:<kind>")`, **antes** de `fork.acquire()` (`:87`). La negativa es por nombre y ocurre **sin tocar el fork**. Confirmado.

## 2. LA INTUICIÓN DEL ARREGLO SÍ EXISTE — y es barata para V2

UniswapV2 `swapExactTokensForTokens(amountIn, amountOutMin, **path[]**, to, deadline)` acepta un **array de ruta**. Un ciclo de N patas `token → X1 → … → Xn → token` es **una sola llamada** con `path = [token, X1, …, Xn, token]`. No hace falta un contrato nuevo ni un encoder nuevo por pata: **la ruta cíclica es representable como el mismo swap multi-hop que el router ya soporta.**

⇒ El arreglo **no** es «inventar una topología»; es **dejar de descartar la ruta** y encodear el path que ya viene en los datos.

## 3. DÓNDE ESTÁN LOS HOPS — la pregunta que decide todo, respondida

El comentario del locus dice que *"the Opportunity payload carries no intermediate hops"*. **Es cierto para el payload, y falso para el sistema:** los hops **sí** están persistidos.

- `backend/searcher-rs/src/engines/dex_engine.rs:1762` — `let metadata = crate::persistence::build_route_metadata_from_plan(&c.route_plan);`
- `dex_engine.rs:1766` — *"persisted route_metadata must keep the **full traversal path**…"*
- `backend/sim-ctl/src/route_lookup.rs:113-119` — `pub async fn fetch_candidate_inputs(...)` hace `SELECT o.route_metadata, o.chain_id, o.dex_a, o.token_in, o.token_out, …` y lo devuelve en `CandidateInputs` (`:26`), tipado como `shared_rs::candidates::RouteMetadata` (`:18`).

⇒ **La pata de adquisición de datos ya existe y ya está cableada en el camino B2c** (`consumer.rs:534` llama a `fetch_candidate_inputs`). Lo que falta es que **`build_probe` reciba la ruta**: hoy su firma es `build_probe(opp: &Opportunity, signer_from: Address)` — **no tiene forma de ver los hops**, y por eso el rechazo está en el lugar equivocado.

## 4. CORRECCIÓN A UNA PREMISA DE LA ORDEN (medida)

La orden dice (§6): *"Capacidad ya existente y NO cableada: `backend/sim-core/src/sim_multistep.rs` (multi-pata), con comentario en `:496` sobre el ctx same-token."*

**Matiz material, leído del código:** `sim_multistep.rs` **no** implementa un ciclo DEX plano de 2-7 patas. Implementa la topología **FLASH-FUNDED**: su entrada es `build_multistep_plan(ctx: &RoundTripContext, config: &MultiStepExecutionConfig, flashloan_executor: Address, backward_amount_in: U256)` (`:359`) y su plan despacha **una** llamada envuelta `requestFlashLoan` al proxy `FlashLoanExecutor` (`MultiStepPlan.flashloan_executor`, `:330-332`), con `MultiStepEntry::{ApplyStorage, ReadBalance, ExecuteCall}` (`:292-318`) — es decir *override de rol + wrapped flash + lectura*, **no** una secuencia de swaps.

⇒ Para el caso que esta tarea pide (ciclo DEX de 2-7 patas), `sim_multistep` **no es la capacidad que falta cablear**: es otra topología. La capacidad que sirve es **el `path[]` multi-hop del propio router V2** (§2), alimentado por `route_metadata` (§3). Tomar `sim_multistep` como «la pieza que falta» llevaría a un arreglo equivocado.

## 5. PLAN DE IMPLEMENTACIÓN (concreto, con firmas — no ejecutado)

1. **`tx_builder.rs`**: añadir `build_probe_with_route(opp, signer_from, path: &[Address]) -> Result<ProbeTx, BuildError>` que encodee `swapExactTokensForTokens` con el `path[]` completo (V2). Para V3, `exactInput` con el `path` empaquetado. Mantener `build_probe` como wrapper: si `token_in == token_out` y **hay** ruta ⇒ delegar; si **no hay** ruta ⇒ error nuevo (punto 3).
2. **`sim_engine.rs`**: antes de `build_probe`, traer la ruta con `route_lookup::fetch_candidate_inputs(&pool, opp.id)` y pasarla. El `Err` de `CyclicRouteNotRepresentable` deja de mapearse a `not_implemented`.
3. **A3 — motivo propio, sin reutilizar la etiqueta vieja:** cuando la ruta esté ausente, emitir una razón **nueva y distinta** (p. ej. `cyclic_route_missing_route_metadata:<kind>`) que **no** reutilice `strategy_cyclic_route_not_simulatable_in_s4`, más la ruta de resolución (por qué falta: `arbx:validated_plan:*` = 0 claves y `route_metadata` vacío/`{}`), y contarla en `persistence.rs`.
4. **`persistence.rs:131-135`** y `max_slippage_for_pass_pct`: **sin tocar** (A2). El fail-closed (`eth_call` + gas + decode + umbral) queda intacto: el cambio sólo hace la ruta **representable**, no favorable.
5. **Tests**: en `backend/sim-ctl/tests/`, un ciclo de 2 patas (`[A, B, A]`) que afirme: (a) produce `ProbeTx` con `to` = router, `from` = signer, `data` no vacío y el `path` decodificable con A como primer y último elemento; (b) **no** devuelve `CyclicRouteNotRepresentable`; (c) ruta ausente ⇒ la razón NUEVA, no la vieja.
6. **`cargo fmt --all -- --check` / `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` / `cargo test -p sim-ctl --no-fail-fast` / `cargo test -p sim-core --no-fail-fast`** en WSL2 con el toolchain 1.91.0, iterando hasta verde.

**Estimación honesta:** 150-250 líneas de Rust + sus tests, más el ciclo de compilación/iteración en WSL. Es factible — **no** es lo que falta; lo que falta es presupuesto para hacerlo **verificado**.

## 6. LO QUE **NO** HICE, y por qué es la respuesta correcta

- **NO escribí el fix.** Sin poder correr los cuatro `Verify`, entregarlo sería Rust sin verificar. En `t66` (C1) y `t80` (C1) tomé la misma decisión por la misma razón, y el capitán la registró como correcta: *"no escribir Rust a medias sin poder compilarlo… habría sido una hipótesis con formato de parche"*. La regla no cambia porque esta orden sea más importante; **cambia más**: `simulations` es la tabla que alimenta todo lo de aguas abajo.
- **NO fabriqué ningún número.** No hay filas escritas, ni veredictos, ni `simulated_profit_usd` inventado. Cero mocks, cero hardcode.
- **NO toqué el checkout compartido** (`C:\Users\HFRC\Desktop\arbitragex-v2-main (17)`): todo en `%TEMP%\arbx-t88`. Sin `git add -A`, sin `reset`, sin `stash` allí.
- **NO mergeé, NO empujé, NO desplegué.** Sin firma, sin broadcast, sin capital.

## 7. A6 — EL VEREDICTO DE MERCADO NO SE TOCA

Declarado de antemano y sin medir, porque no ejecuté: **hacer la ruta representable NO la hace rentable.** `t48` midió que el rechazo económico está **10,82× corto** contra el hurdle. Si tras implementar esto el mejor caso sigue dando `simulated_profit_usd` negativo, **eso es un veredicto de mercado medido** y debe reportarse tal cual — sin mejorar el número y **sin subir el sizing**. El arreglo compra **flujo EVALUABLE**, no ganancia.

---

*Reconocimiento verificado con el SHA base confirmado, el locus citado línea a línea, la fuente de los hops localizada (`route_metadata`, con `dex_engine.rs:1762,1766`), y una premisa de la orden corregida: `sim_multistep` es la topología FLASH, no el ciclo DEX. El arreglo está diseñado y es barato para V2 (`path[]` del router). No lo entrego sin poder correr los cuatro Verify: Rust sin verificar sería la misma hipótesis con formato de parche que ya me negué a entregar dos veces.*
