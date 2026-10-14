# MARKET-FEATURES-01 — matriz de aceptación

Rama: `fix/omegasearch-features-01` · base `9202561e` (= `origin/main` menos los 2 commits de
PRICE-COVERAGE-01/02, que sólo tocan `cartridge_boot.rs`, `lib.rs` y `workers/price_worker.rs`)
Sobre: `98871072` (tarea C, OMEGASEARCH-FEES-01 — productores de coste) — no se perdió ni se mezcló.
Commit: `b3181136` (ver §6 para el SHA exacto verificado)

Módulo nuevo: `backend/searcher-rs/src/native_operator_adapter/market_features/` (5 archivos),
registrado desde `native_operator_adapter.rs:18` — **NO** desde `lib.rs`/`main.rs` (zona caliente).
El prefijo es `native_operator_adapter` porque ESE es el módulo que emite el `DATA_GAP` de los
operadores nativos (`native_operator_adapter.rs:72`, `:120`): el diagnóstico y el dato que lo
cierra quedan juntos.

---

## 1. Tabla de contratos — CLAVE | PRODUCTOR | FUENTE | UNIDAD | VENTANA | ESTADO

| CLAVE | PRODUCTOR (archivo:línea) | FUENTE AUTORITATIVA | UNIDAD | VENTANA / BLOQUE | ESTADO |
|---|---|---|---|---|---|
| `volatility` | `market_features::produce` → `realized_volatility.rs:173` (`realized_volatility`) vía `mod.rs:135` | observaciones reales del precio USD **fusionado** del `PriceBus` (`PriceView::price_usd`), auto-muestreadas 1 por llamada en `mod.rs:150` | adimensional — desv. est. **muestral** (n−1) de los retornos logarítmicos por observación, **no anualizada** | rodante: últimas 900 s (`DEFAULT_WINDOW_NS`), ≥ 8 muestras (`DEFAULT_MIN_SAMPLES`), span real ≥ 60 s (`DEFAULT_MIN_SPAN_NS`) | **IMPLEMENTED · TESTED** |
| `oracle_price` | `oracle_bias::fresh_anchor_usd` → `oracle_bias.rs:63`, emitido en `mod.rs:161` | ancla **Chainlink** del `PriceBus` (`PriceSnapshot.chainlink[SYM].answer`), escrita por `workers/price_worker.rs::fetch_chainlink` desde `latestRoundData()` de los agregadores de PG | USD por token (precio unitario) | ronda de `Anchor.updated_at`; fresca si ≤ 3900 s (volátil) / ≤ 90000 s (stable) | **IMPLEMENTED · TESTED** |
| `onchain_price` | `oracle_bias::oracle_bias_pair` → `oracle_bias.rs:93`, emitido en `mod.rs:162` | precio del manifold DEX desde las **reservas reales** del pool normalizadas por decimales en `math_evidence::build_market_state:186` (el mismo valor de `price_matrix`) | USD por token (precio unitario) | el bloque/tick del snapshot de reservas (mismo `block_number` del `MarketState`) | **IMPLEMENTED · TESTED** |
| `parity_deviation` | **YA PRODUCIDO — NO se reimplementa** `math_evidence::regime_features_from_redis:249-251` | Redis `arbx:token_prices:<chain>` (hash del PriceBus) | adimensional — `\|px − 1\|` de un ancla | último valor publicado del hash | **EXISTENTE (FEATURES-01a, en `main`)** |
| `health_factor` | **YA PRODUCIDO — NO se reimplementa** `orchestrator.rs:661-673` → insert `:708-710` | indexer **cacheado** del motor de liquidación | adimensional — ratio de salud | última indexación, sólo posiciones impactadas | **EXISTENTE (FEATURES-02, en `main`)** |
| `arbitrage_gap` | **NO ES UNA FEATURE — no se produce** | `RegimeRouter::analyze:153-186` lo **computa** desde `pair_keys` (FEATURES-01b) | adimensional — `max/min − 1` intra-par | el bloque del `MarketState` | **NO APLICA (sin lector)** |

Los contratos viven como **datos ejecutables** en `contract.rs:80` (`CONTRACTS`), no como prosa:
`contract.rs:190` declara `OWNED_KEYS`, y los tests fallan si `produce` emite una clave sin
contrato o si empieza a emitir una clave ajena.

---

## 2. Correcciones duras a las premisas de entrada (medidas, no supuestas)

| Premisa recibida | Medición | Consecuencia |
|---|---|---|
| "las 5 features sin productor" | sólo **3** están sin productor: `volatility`, `oracle_price`, `onchain_price` | el alcance real es 3 productores, no 5 |
| "el único `features.insert` del searcher es `health_factor`" | hay **dos** productores vivos: `parity_deviation` (`math_evidence.rs:249-251`) y `health_factor` (`orchestrator.rs:708-710`) | reimplementar `parity_deviation` habría sido **duplicar** un productor vivo |
| `arbitrage_gap` como feature a producir | **no tiene lector**: `git grep 'features.get("arbitrage_gap")'` sobre `backend/` sólo devuelve el doc-comment de este módulo; el router lo computa desde `pair_keys` | producirla sería cobertura **aparente** |
| los 5 commits de las ramas 01b/02 "faltan en main" | sus **SHAs** no son ancestros de mi HEAD (se mergearon por PR con otro SHA), pero su **contenido sí está**: `pair_keys`+agrupación (`regime_router.rs`), preferencia de `volatility`, productor de `parity_deviation`, productor de `health_factor` | **no había nada que mergear ni reescribir**; se verificó contenido, no SHA |
| el contrato de `MarketState` citado | en este árbol tiene **7** campos, incluido `pair_keys: Vec<String>` (`operators/mod.rs:88`) | el contrato citado estaba desactualizado |

---

## 3. Por qué el módulo vive en `searcher-rs` y no en `math-engine`

Evidencia, no preferencia: `backend/math-engine/Cargo.toml` **no declara `shared-rs`** (ni `redis`
fuera de la feature `api`), así que ese crate no puede ver el `PriceBus` ni Redis — y las tres
fuentes autoritativas son exactamente el `PriceBus` (ancla Chainlink + precio fusionado) y las
reservas on-chain. El `MarketState` se construye además en
`searcher-rs/src/math_evidence.rs::build_market_state`.

Hallazgo de compilación que fijó la API (medido, no asumido): `price_bus_global` se declara en
`main.rs:137` — existe **sólo** en el target binario — mientras este módulo se compila en **ambos**
targets (`lib.rs:37` y `main.rs:200`). La primera versión del adaptador lo referenciaba como
`crate::price_bus_global::get()` y el build de la lib falló:

```
error[E0433]: failed to resolve: could not find `price_bus_global` in the crate root
   --> searcher-rs/src/native_operator_adapter/market_features/mod.rs:211
```

Por eso el `PriceBus` es un **parámetro** (`mod.rs:216`, `mod.rs:232`): el módulo queda agnóstico
de target y testeable sin arrancar `main.rs`.

---

## 4. Qué desbloquea realmente (y qué NO) — fail-honest

Dependencias de features de los 9 operadores que la telemetría en vivo reportó en `DATA_GAP`
(operadores 22, 21, 8, 11, 16, 13, 5, 26, 10), medidas leyendo cada `op_*.rs`:

| Operador | features que lee | ¿Lo cierra este módulo? |
|---|---|---|
| `op_22_monte_carlo` | **ninguna** | **NO** — su `DATA_GAP` no viene de una feature |
| `op_08_kalman` | **ninguna** | **NO** — ídem |
| `op_16_kelly` | **ninguna** | **NO** — ídem |
| `op_13_regression` | **ninguna** | **NO** — ídem |
| `op_05_pdmp` | **ninguna** | **NO** — ídem |
| `op_10_welford` | **ninguna** | **NO** — ídem |
| `op_21_newton` | `pool_fee` | NO aquí — dominio de la tarea C (`cost_producers`) |
| `op_26_flash_loan` | `flash_premium`, `pool_fee` | NO aquí — dominio de la tarea C |
| `op_11_bayes` | `bayes_wins`, `bayes_losses` | NO — requiere su propio productor |

**6 de los 9 operadores en `DATA_GAP` no leen NINGUNA clave de `features`.** Ningún productor de
features puede cerrar su gap: el receipt los distingue por `reason` —
`empty_input_admission_receipt` (`native_operator_adapter.rs:72`) o
`native_operator_returned_no_finite_value` (`:120`) — y esa es la línea de investigación siguiente,
no esta.

Lo que **sí** desbloquea este módulo es la **capa de régimen**, que es la que decide QUÉ operadores
se recomiendan: de las 5 `RegimeMetrics`, `oracle_bias` era estructuralmente `None` y `volatility`
caía siempre al proxy cross-par; con eso, `RegimeRouter::classify` degeneraba a `["Neutral"]`
(2 operadores). Las dos pruebas end-to-end de §5 lo miden sobre el router real.

---

## 5. Verificación reproducible (§6) — estados §15

| Estado | Aplica a | Evidencia |
|---|---|---|
| `SPECIFIED` | contrato de cada clave (unidad / fuente / ventana / ausencia) | `contract.rs:80` (`CONTRACTS`), tests `every_owned_key_has_a_complete_contract`, `monetary_keys_are_declared_as_such_and_the_limitation_is_recorded` |
| `IMPLEMENTED` | 3 de 3 claves sin productor | 1.562 líneas añadidas, 6 archivos, commit `b3181136` |
| `TESTED` | 26/26 tests del módulo; 1569/1570 del suite `--lib` (el único fallo es preexistente y ajeno) | §6.2, §6.5 |
| `FORK_VERIFIED` | **NO** | no se ejecutó nada contra un fork |
| `LIVE_DATA_VERIFIED` | **NO** | todas las anclas/precios de las pruebas son **fixtures rotulados**; la única fuente real usada es la API pública del `PriceBus` en memoria |
| `EXECUTION_AUTHORIZED` | **NO** | el módulo no firma, no transmite, no activa capital |
| `SETTLED` / `RECONCILED` | **NO** | — |
| `BLOCKED_EXTERNAL` | **NO para el módulo** (compila y pasa tests enteramente con fixtures) · **SÍ para la conexión viva** | §7: el único punto de inserción es zona caliente y no se tocó |

### 5.1 Las cuatro aserciones que sostienen la doctrina R8

1. **Ausencia ≠ cero.** `no_key_is_inserted_as_zero_when_the_data_is_missing`: sin dato real el mapa
   sale **vacío** y ninguna de las tres claves aparece; además se afirma que no existe ningún `0.0`
   fabricado en el mapa.
2. **Cero real ≠ ausencia.** `a_flat_price_over_a_real_span_is_zero_and_that_is_not_absence`: una
   serie plana sobre 420 s reales da `Some(0.0)` — es una medición y se publica. Devolver `None`
   ahí sería la mentira espejo.
3. **La atomicidad del par.** `the_oracle_pair_is_atomic_and_never_half_emitted`: `oracle_price` y
   `onchain_price` se emiten **juntos o no se emiten**; media medición afirmaría un sesgo que nadie
   midió.
4. **El eje temporal es obligatorio.** `volatility_is_absent_when_the_span_is_too_short`: 8 muestras
   dentro de 7 ms se rechazan aunque sobren muestras — aritmética real sobre un eje sin significado.

Además, el valor de referencia de la volatilidad se calculó **fuera del código** (Python
`statistics.stdev`, contrastado con un `ddof=1` explícito: `0.031694824469859523`) y se congela como
constante en `tests.rs:40`, de modo que el test no puede pasar re-derivando su propia respuesta.

### 5.2 Limitación de `f64` en claves monetarias (doctrina §4)

`oracle_price` y `onchain_price` son **USD por token**: magnitudes monetarias. `MarketState.features`
es `HashMap<String, f64>` **por diseño del `math-engine`** (`operators/mod.rs:98`), así que una
feature monetaria no puede evitar el tipo. La limitación queda acotada por contrato
(`contract.rs:39-52`, campo `monetary`) y por test:

* son **precios unitarios**, nunca importes — no se suman, no se netean, no se usan para
  contabilidad;
* su **único** consumidor los usa en un **ratio invariante de escala**
  (`|oracle − onchain| / onchain`, `regime_router.rs:201`), donde el redondeo de `f64` es
  irrelevante;
* para cualquier magnitud contable la autoridad sigue siendo el camino entero exacto (wei/`u256`),
  que este módulo no toca.

---

## 6. Comandos de verificación (WSL Ubuntu, sin Windows AppControl)

Los build scripts recién compilados están bloqueados en Windows por **AppControl** (`os error 4551`)
— es la limitación ambiental ya documentada en `CLAUDE.md` §36.4 y en
`docs/verification/COST-PRODUCERS-01-matrix.md` §4.1, con precedente en
`docs/verification/MEV-01-001-implementacion-auditoria.md`. Se verificó en **WSL2 Ubuntu**
(`rustc`/`cargo` 1.91.0 — la versión que fija `rust-toolchain.toml`), con correspondencia exacta de
fuentes vía `git archive` del HEAD commiteado:

```bash
git archive --format=tar.gz -o src.tar.gz HEAD backend rust-toolchain.toml
tar -xzf src.tar.gz -C ~/arbx-verify-features
cd ~/arbx-verify-features/backend
```

### 6.1 `cargo check -p searcher-rs --lib`

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 50s
CHECK_EXIT=0
```

### 6.2 `cargo test -p searcher-rs --lib native_operator_adapter::market_features`

```
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 1549 filtered out; finished in 0.00s
TEST_EXIT=0
```

### 6.3 `rustfmt --edition 2021 --check` sobre los 6 archivos

```
FMT_EXIT=0
```

### 6.4 `cargo clippy -p searcher-rs --lib --all-targets`

Dos avisos emitidos en la primera pasada y **corregidos** antes del commit final
(`excessive_precision` en la constante de referencia → literal al mínimo de dígitos que identifica
el mismo `f64`; `explicit_auto_deref` en `symbol: *sym` → `symbol: sym`, que además confirma que la
coerción de campo de struct funciona).

```
0        # ocurrencias de 'market_features' en la salida de clippy
CLIPPY_HITS_ABOVE (0 = clean)
```

**Nota de método:** la constante de referencia **no** se "redondeó para contentar al linter": ambas
escrituras son el **mismo** `f64` (es exactamente la condición que dispara
`clippy::excessive_precision`), y el decimal de 17 dígitos que salió de Python queda en el
doc-comment inmediatamente encima, como provenance.

### 6.5 Suite completa `cargo test -p searcher-rs --lib` (regresión)

```
test result: FAILED. 1569 passed; 1 failed; 5 ignored; 0 measured; 0 filtered out
SUITE_EXIT=101
```

El único fallo es `route_discovery::route_discovery_worker::tests::xlang_golden_tick_contract`
(«golden full.json missing»). Es **preexistente y ajeno**:

* el fixture **no está versionado**: `git ls-files 'backend/searcher-rs/tests/golden/*'` → 0 archivos;
* **no existe en el árbol**: `find . -name full.json` → 0 resultados;
* este cambio no toca `route_discovery/**` — el diff son 6 archivos y ninguno está en esa ruta
  (`route_discovery/route_discovery_worker.rs` es además zona caliente y no se modificó);
* el mismo fallo, con el mismo motivo exacto, está documentado en la verificación de la tarea C
  sobre esta misma base (`COST-PRODUCERS-01-matrix.md` §4.2, 1543 passed / 1 failed).

**Contraste cuantitativo que cierra el círculo:** 1569 − 1543 = **26** = exactamente los tests que
añade este módulo. La suite no perdió ni un test y ganó los 26 nuevos.

---

## 7. Integración pendiente (declarada, con archivo:línea — NO ocultada)

1. **La conexión de una línea.** El módulo se compila y se prueba, pero **no está cableado** al
   camino vivo: todos los puntos de inserción posibles están en la zona caliente. Conexión exacta:

   ```
   backend/searcher-rs/src/orchestrator.rs:705
     existe:  let mut features =
                  crate::math_evidence::regime_features_from_redis(&mut math_redis, chain_id).await;
     añadir:  features.extend(crate::native_operator_adapter::market_features::produce_from_global(
                  crate::price_bus_global::get().as_deref(), &symbol, onchain_price_usd, now_ns));
   ```

   `symbol` = el token base de la ruta; `onchain_price_usd` = la entrada de `price_matrix` para ese
   par — **ambos ya calculados** en `math_evidence::build_market_state` (`math_evidence.rs:186-189`).
   Alternativa equivalente: hacerlo dentro de `regime_features_from_redis`, cuya firma necesitaría
   entonces el símbolo y el precio on-chain. Ninguno de los dos archivos se tocó (ambos son
   zona caliente).

2. **Contrato de reloj.** `now_ns` debe ser el instante actual en el mismo reloj de pared que usa el
   `PriceBus`: `PriceView::price_usd` usa su propio reloj interno mientras la frescura del ancla se
   juzga contra el `now_ns` inyectado. En producción coinciden; documentado en `mod.rs:216-233`.

3. **Semántica del congelamiento (deliberada).** Este módulo lee `Anchor.answer` en crudo. El
   `DivergenceFrozen` del bus sigue gobernando `price_usd` para toda decisión de trading
   (`price_bus.rs:446-458`) y **no se elude**: el sesgo oracle es justamente la magnitud contra la
   que se define el congelamiento, y una vez cableado lo hará observable en el régimen.

4. **`volatility` no tiene muestras al arranque.** El eje temporal se construye con una observación
   real por llamada; durante los primeros ~60 s tras un reinicio la clave estará **ausente**
   (correcto: ausencia, no un cero) y el router conservará su proxy hasta que haya span real. No es
   un bug: es el precio de no fabricar el eje.
