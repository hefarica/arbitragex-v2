# CONTRATO DE DATOS — `MarketState.features`

**Tarea:** F4-contrato (oleada 3 OMEGASEARCH-FEATURES)
**Rama:** `fix/omegasearch-features-01` · worktree `omegasearch-fees`
**Base medida:** `9202561e` (ancestro de `origin/main`, ver §5)
**Alcance de escritura de esta tarea:** `docs/market-features/` (único)
**Regla rectora:** RULE 00 / R8 — una ausencia es una ausencia. Un `DATA_GAP` honesto vale más
que un `0.0` fabricado.

---

## 0. El contrato del contenedor (medido, no citado)

```rust
// backend/math-engine/src/operators/mod.rs:61
pub struct MarketState {
    pub price_matrix: Vec<Vec<f64>>,        // :63
    pub pair_keys: Vec<String>,             // :88  (serde(default))
    pub liquidity_reserves: Vec<(f64,f64)>, // :90
    pub gas_price_gwei: f64,                // :92
    pub block_timestamp: u64,               // :94
    pub block_number: u64,                  // :96
    pub features: HashMap<String, f64>,     // :98
}
```

**Corrección al contrato citado en el encargo.** El encargo describía `MarketState` con 5 campos
(`price_matrix`, `liquidity_reserves`, `gas_price_gwei`, `block_timestamp`, `block_number`,
`features`). Medido en este árbol tiene **7**: falta `pair_keys: Vec<String>`
(`operators/mod.rs:88`), que es justamente la identidad de par introducida por FEATURES-01b y de
la que depende que `arbitrage_gap` no cruce pares distintos. Comando:
`git grep -n "pub struct MarketState" -- backend/` → `backend/math-engine/src/operators/mod.rs:61`.

### La limitación de `f64` para claves monetarias (doctrina §4)

`features` es `HashMap<String, f64>` **por diseño del math-engine** (`operators/mod.rs:98`), así
que una clave monetaria no puede evitar el tipo. La limitación se declara y se acota:

1. Las dos claves monetarias de este contrato (`oracle_price`, `onchain_price`) son **precios
   unitarios** (USD por token), **nunca importes**.
2. Su **único** consumidor las usa en un **ratio invariante de escala**:
   `|oracle − onchain| / onchain` (`regime_router.rs:201`), donde el redondeo de `f64` es
   irrelevante.
3. **Regla dura:** no se suman, no se netean, no se usan para contabilidad ni para sizing. Para
   cualquier magnitud contable la autoridad es el camino entero exacto (wei / `u256`), que este
   contrato no toca.
4. `monetary: true` queda registrado como dato ejecutable en
   `market_features/contract.rs` (campo `monetary`), verificado por el test
   `monetary_keys_are_declared_as_such_and_the_limitation_is_recorded`.

### El `HashMap` no distingue ausente de cero

Consecuencia operativa, y motivo de existir de F4: `features.get(k)` devuelve `None` si nadie
insertó `k`, y `Some(0.0)` si alguien midió exactamente cero. **Nada en el tipo impide que un
escritor inserte `0.0` cuando no tiene dato.** La única defensa es la disciplina del escritor, y
por eso el contrato se materializa como datos + un gate ejecutable (§4 de
`REGRESION-ANTI-FABRICACION.md`).

---

## 1. Contrato por clave — las 5 claves en juego

Todas las fuentes citadas abajo son **archivo:línea sobre este árbol**, verificadas con lectura
directa. `PRODUCED` = productor escrito y cableado al camino vivo.
`PRODUCED_PENDING_WIRE` = productor escrito, **conexión pendiente** por zona caliente (§3 de
`INTEGRACION-PENDIENTE.md`).

### 1.1 `volatility` — `PRODUCED_PENDING_WIRE`

| Campo | Valor |
|---|---|
| **Clave exacta** | `volatility` (constante `VOLATILITY_KEY`, `market_features/mod.rs:97`) |
| **Unidad** | adimensional — desviación estándar **muestral (n−1)** de los retornos logarítmicos **por observación**. **NO anualizada.** |
| **Fuente autoritativa** | observaciones reales del precio USD **fusionado** del `PriceBus` (`PriceView::price_usd`); auto-muestreadas 1 por llamada en `mod.rs:152-154`. El `PriceBus` es la única fuente. |
| **Productor** | `market_features/mod.rs:158` (emisión) ← `realized_volatility::volatility_for` |
| **Ventana / anclaje** | rodante: últimas `DEFAULT_WINDOW_NS` (900 s), con ≥ `DEFAULT_MIN_SAMPLES` (8) muestras y span real ≥ `DEFAULT_MIN_SPAN_NS` (60 s) entre la primera y la última |
| **AUSENTE significa** | sin serie suficiente para un eje temporal real → la clave **NO se inserta** y `RegimeRouter::analyze` cae a su proxy cross-venue (`regime_router.rs:113-135`). Un `0.0` aquí afirmaría "mercado plano", que es una **medición**, no una ausencia. |
| **Consumidores** | `regime_router.rs:107` (honesto: `if let Some(&v)` + guarda `is_finite && >= 0`)<br>`canonical_strategy.rs:78` (honesto: `if let Some(&vol)` + guarda `is_finite`)<br>`canonical_strategy.rs:131` (**FABRICA**: `unwrap_or(0.0)` — ver §3.2) |
| **Monetaria** | no |

**Nota de arranque (honesta, no un bug):** el eje temporal se construye con una observación real
por llamada; durante los primeros ~60 s tras un reinicio la clave estará ausente y el router
conservará su proxy. Es el precio de no fabricar el eje.

### 1.2 `oracle_price` — `PRODUCED_PENDING_WIRE`

| Campo | Valor |
|---|---|
| **Clave exacta** | `oracle_price` (constante `ORACLE_PRICE_KEY`, `oracle_bias.rs:54`) |
| **Unidad** | USD por token (precio unitario) |
| **Fuente autoritativa** | ancla Chainlink del `PriceBus` (`PriceSnapshot.chainlink[SYM].answer`, ya ajustada por decimales por el escritor) ← `workers/price_worker.rs::fetch_chainlink` desde `latestRoundData()` de los agregadores configurados en PG |
| **Productor** | `market_features/mod.rs:170` (emisión) ← `oracle_bias::fresh_anchor_usd` (`oracle_bias.rs:63`) |
| **Ventana / anclaje** | la ronda del agregador referida por `Anchor.updated_at`, aceptada solo si es fresca: ≤ 3900 s para volátiles y ≤ 90000 s para stables (heartbeats reales de Chainlink, espejo de `PriceView::anchor_is_fresh`) |
| **AUSENTE significa** | sin ancla para el símbolo, o con edad mayor a su heartbeat: la clave **NO se inserta** y `oracle_bias` queda en `None` (`regime_router.rs:196-203`). **Nunca** un precio fabricado ni `0.0`. |
| **Consumidores** | `regime_router.rs:197` |
| **Monetaria** | **sí** → ver §0 |

Se rechaza un `answer` no finito o ≤ 0 (`oracle_bias.rs:74-76`): un cero de oráculo es un feed
roto, no un activo gratis.

### 1.3 `onchain_price` — `PRODUCED_PENDING_WIRE`

| Campo | Valor |
|---|---|
| **Clave exacta** | `onchain_price` (constante `ONCHAIN_PRICE_KEY`, `oracle_bias.rs:56`) |
| **Unidad** | USD por token (precio unitario) |
| **Fuente autoritativa** | precio del manifold de liquidez DEX derivado de las **reservas reales** del pool normalizado por los decimales de ambos tokens en `math_evidence::build_market_state` — **el mismo valor que ya viaja en `price_matrix`** |
| **Productor** | `market_features/mod.rs:171` (emisión) ← `oracle_bias::oracle_bias_pair` (`oracle_bias.rs:93`) |
| **Ventana / anclaje** | el bloque/tick del snapshot de reservas que alimentó `build_market_state` (el mismo `block_number` del `MarketState`) |
| **AUSENTE significa** | ninguna pata de pool con reservas y decimales conocidos → la clave **NO se inserta** y `oracle_bias` queda en `None` |
| **Consumidores** | `regime_router.rs:198` |
| **Monetaria** | **sí** → ver §0 |

**Atomicidad declarada:** `oracle_price` y `onchain_price` se emiten **juntos o no se emiten**
(`oracle_bias_pair` devuelve `Option<(f64,f64)>`, `oracle_bias.rs:99-105`). El único consumidor
exige ambos; media medición afirmaría un sesgo que nadie midió.

### 1.4 `parity_deviation` — `PRODUCED` (pre-existente, NO reimplementar)

| Campo | Valor |
|---|---|
| **Clave exacta** | `parity_deviation` |
| **Unidad** | adimensional — `\|precio − 1\|` del stable más desviado |
| **Fuente autoritativa** | hash Redis `arbx:token_prices:<chain>` (publicado por el `PriceBus`) |
| **Productor** | `math_evidence.rs:250`, dentro de `regime_features_from_redis` (`:238-253`) |
| **Ventana / anclaje** | el último valor publicado del hash (TTL del `PriceBus`) |
| **AUSENTE significa** | ningún stable de `PARITY_STABLES` (`math_evidence.rs:257-259`) con precio parseable → el mapa va **vacío** y la métrica queda en `None` (`regime_router.rs:208`) |
| **Consumidores** | `regime_router.rs:208` |
| **Monetaria** | no |

**Trampa de medición registrada (importa para cualquier auditoría futura):** este productor
**no** aparece en un `git grep "features.insert"` porque el mapa local se llama `out`:
`out.insert("parity_deviation".to_owned(), dev)`. El encargo afirmaba que el único `insert` del
searcher era `health_factor`; eso es cierto **sólo** para el mapa literalmente llamado
`features`. Medición: `git grep -n "insert" -- backend/searcher-rs/src/math_evidence.rs` →
`:250`. Es exactamente el motivo por el que el gate de F4 resuelve constantes y no escanea sólo
literales.

### 1.5 `health_factor` — `PRODUCED` (pre-existente, NO reimplementar)

| Campo | Valor |
|---|---|
| **Clave exacta** | `health_factor` |
| **Unidad** | adimensional — ratio de salud (deuda / colateral ponderado); < 1 es liquidable |
| **Fuente autoritativa** | el indexer **cacheado** del motor de liquidación (`liquidation_engine.indexer`) |
| **Productor** | `orchestrator.rs:709` ← cálculo en `orchestrator.rs:661-673` |
| **Ventana / anclaje** | la última indexación de posiciones de lending, referida a las posiciones **impactadas** por este intent |
| **AUSENTE significa** | sin posiciones de lending impactadas o sin entrada en el indexer → `hf_feature = None` y la clave **NO se inserta**. **Nunca `1.0`** — eso afirmaría "todo sano", que es una aserción, no una ausencia. |
| **Consumidores** | `regime_router.rs:189` |
| **Monetaria** | no |

Publica el **mínimo**, no la media (`orchestrator.rs:659-673`): la posición más cerca de liquidar
es la significativa para el régimen.

### 1.6 `arbitrage_gap` — `NO_CONSUMER` (no es una feature)

No se produce, y **no debe producirse**: `RegimeRouter::analyze` lo **computa internamente**
desde `state.pair_keys` + `price_matrix` (`regime_router.rs:137-186`, FEATURES-01b), y **no
existe ningún lector** `features.get("arbitrage_gap")` en el repo. Medición:
`git grep -n 'features.get(' -- backend/` → la única aparición de la cadena es un doc-comment
(`market_features/mod.rs:30`) y la tabla de contrato (`contract.rs:179`). Insertarlo sería
**cobertura aparente**, no cobertura.

---

## 2. Tabla de verdad final — las 6 claves del encargo

| # | Clave | Productor (archivo:línea) | Estado | Evidencia |
|---|---|---|---|---|
| 1 | `parity_deviation` | `backend/searcher-rs/src/math_evidence.rs:250` | **PRODUCED** | lectura directa de `regime_features_from_redis` (`:238-253`); `#[cfg(test)]` en `:681` ⇒ la línea 250 es producción |
| 2 | `health_factor` | `backend/searcher-rs/src/orchestrator.rs:709` | **PRODUCED** | `git grep -n "features.insert" -- backend/` → única aparición en el searcher; cálculo en `:661-673` |
| 3 | `volatility` | `backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:158` | **PRODUCED_PENDING_WIRE** | emisión detectada por el gate resolviendo la const `VOLATILITY_KEY` (`mod.rs:97`) |
| 4 | `oracle_price` | `.../market_features/mod.rs:170` | **PRODUCED_PENDING_WIRE** | ídem, const `ORACLE_PRICE_KEY` (`oracle_bias.rs:54`) |
| 5 | `onchain_price` | `.../market_features/mod.rs:171` | **PRODUCED_PENDING_WIRE** | ídem, const `ONCHAIN_PRICE_KEY` (`oracle_bias.rs:56`) |
| 6 | `arbitrage_gap` | — (lo computa el router, no es feature) | **NO_CONSUMER** | `regime_router.rs:137-186`; sin `features.get` lector |

**Ninguna clave del encargo queda en `ABSENT`.** Las tres que estaban sin productor
(`volatility`, `oracle_price`, `onchain_price`) tienen productor escrito y testeado; lo que falta
es **la conexión**, no el productor.

**Ninguna clave del encargo queda en `BLOCKED_EXTERNAL`**: las tres fuentes (precio fusionado del
`PriceBus`, ancla Chainlink, reservas on-chain) están todas en proceso o en el intent. Lo que
bloquea la conexión es **la zona caliente de 11 archivos**, que es una restricción de proceso de
esta oleada, no un requisito externo irresoluble.

### 2.1 Censo completo: hay 26 claves leídas, no 6

El encargo habla de 6 claves. La medición sobre los **420 archivos de producción** (`backend/**/*.rs`
menos tests y bloques `#[cfg(test)]`) da **35 sitios de lectura y 26 claves distintas**. Las
**21 claves restantes no aparecen en el encargo y no tienen productor**:

| Clave | Lectores (archivo:línea) | Estado |
|---|---|---|
| `pool_fee` | `op_15_golden_section.rs:48`, `op_21_newton.rs:54`, `op_26_flash_loan.rs:60`, `op_32_multi_objective.rs:464` | **ABSENT** |
| `fee_bps` | `op_15_golden_section.rs:46`, `op_21_newton.rs:52`, `op_32_multi_objective.rs:462` | **ABSENT** (sólo hay inserts en tests: `real_ops_tests.rs:438`) |
| `flash_premium` | `op_26_flash_loan.rs:66` | **ABSENT** |
| `gas_units` | `op_15_golden_section.rs:114`, `op_21_newton.rs:111`, `op_26_flash_loan.rs:107` | **ABSENT** |
| `bayes_wins` / `bayes_losses` | `op_11_bayes.rs:35`, `:36` | **ABSENT** |
| `bayes_prior_alpha` / `bayes_prior_beta` | `op_11_bayes.rs:39`, `:44` | **ABSENT** |
| `break_even_target` | `op_21_newton.rs:118` | **ABSENT** |
| `token0_per_eth` | `op_26_flash_loan.rs:112` | **ABSENT** |
| `decoherencia` | `canonical_strategy.rs:86` | **ABSENT** |
| `eth_price_usd` | `canonical_strategy.rs:157` | **ABSENT** |
| `jit_decay_rate` / `min_liquidity` | `op_28_jit_liquidity.rs:58`, `:108` | **ABSENT** |
| `max_capital` | `op_19_simplex.rs:174` | **ABSENT** |
| `mo_weight_yield` / `mo_weight_risk` / `mo_weight_latency` | `op_32_multi_objective.rs:566`, `:567`, `:568` | **ABSENT** |
| `mo_per_leg_latency_ms` | `op_32_multi_objective.rs:626` | **ABSENT** |
| `nsga2.count` | `op_32_nsga2/mod.rs:78` | **ABSENT** (sólo hay inserts en tests, `:223-272`) |
| `nsga2.population_size` | `op_32_nsga2/mod.rs:85` | **ABSENT** |

**Cómo se obtuvo:** estos 35 sitios salieron de forma **iterativa**, y conviene registrar el
método porque dos veces el primer barrido fue incompleto. (a) Un `git grep 'features.get\('`
encuentra 25 sitios y **pierde las lecturas cuyo receptor está en una línea anterior** —
`op_11_bayes.rs:39` es `state` / `.features` / `.get("bayes_prior_alpha")` / `.copied()` /
`.unwrap_or(1.0)`, cinco líneas para una sola lectura. (b) Un escaneo que exige el receptor en la
misma línea pierde exactamente esas **siete** claves (`bayes_prior_alpha`, `bayes_prior_beta`,
`fee_bps`, `eth_price_usd`, `gas_units`, `break_even_target`, `token0_per_eth`). El gate resuelve
ambos con una ventana hacia atrás acotada, y **cada corrección subió el censo**: 20 → 26 claves.

Censo reproducible y versionado en `docs/market-features/census.json`, generado por
`docs/market-features/check-feature-producers.ps1 -Json`.

**Fail-honest sobre el censo:** las claves `mo_*` y `nsga2.*` son **parámetros de configuración
del propio operador op_32** (pesos, población, latencia por pierna), no mediciones de mercado.
Que estén ausentes no es un hueco de datos del mercado: es que nadie los configura. Se declaran
`ABSENT` porque ningún productor los inserta en producción — la razón exacta está en la columna
de lectores.

---

## 3. Hallazgos duros del contrato

### 3.1 El `DATA_GAP` de los 9 operadores NO se cierra con features

El encargo asume que producir las features desbloquea a los 9 operadores de la telemetría
(`22, 21, 8, 11, 16, 13, 5, 26, 10`). Medición, operador por operador:

| Operador | Archivo | Lee de `features` | Guarda de entrada | ¿Lo cierra un productor de features? |
|---|---|---|---|---|
| `op_22_monte_carlo` | `op_22_monte_carlo.rs:26` | **—** | `price_matrix.is_empty()` | **NO** |
| `op_16_kelly` | `op_16_kelly.rs:23` | **—** | `price_matrix.is_empty()` | **NO** |
| `op_10_welford` | `op_10_welford.rs:25` | **—** | `price_matrix.is_empty()` | **NO** |
| `op_05_pdmp` | `op_05_pdmp.rs:27` | **—** | `price_matrix.is_empty()` | **NO** |
| `op_08_kalman` | `op_08_kalman.rs:28` | **—** | `price_matrix.is_empty()` | **NO** |
| `op_13_regression` | `op_13_regression.rs:22` | **—** | `price_matrix.is_empty()` | **NO** |
| `op_21_newton` | `op_21_newton.rs:54` | `pool_fee` (fallback) | `liquidity_reserves.is_empty()` → `no_reserves` | **NO** (dominio de coste/fee) |
| `op_26_flash_loan` | `op_26_flash_loan.rs:60,66` | `pool_fee`, `flash_premium` | retornos tempranos | **NO** (dominio de coste/fee) |
| `op_11_bayes` | `op_11_bayes.rs:35,36` | `bayes_wins`, `bayes_losses` | `wins+losses < 1.0` → `reason_no_history` | **NO** (requiere su propio productor) |

**6 de los 9 operadores no leen ninguna clave de `features`.** Ningún productor de features puede
cerrar su gap, y el productor que F2 escribió no lo pretende: su consumidor es la **capa de
régimen** (`regime_router.rs`), que decide qué operadores se recomiendan.

### 3.2 El consumidor de la clave recién producida fabrica un `0.0`

En `backend/math-engine/src/strategies/canonical_strategy.rs` conviven los dos patrones:

```rust
// :78  CORRECTO — la ausencia no contribuye, y el valor se usa tal cual.
if let Some(&vol) = state.features.get("volatility") {
    if vol.is_finite() { z += vol * 0.5; }
}

// :131  FABRICA — la ausencia se convierte en "mercado plano".
fn estimate_decoherencia(&self, state: &MarketState) -> Option<f64> {
    let vol = state.features.get("volatility").copied().unwrap_or(0.0);   // <-- aquí
    ...
    Some((vol * 0.01 + liq_factor * 0.005 + gas_factor * 0.001).min(0.5))
}
```

Por qué importa para **esta** misión: `volatility` es una de las tres claves que F2 acaba de
dotar de productor. Una vez cableada, cuando la clave esté **ausente** (los primeros ~60 s tras
un reinicio, o cualquier tick sin serie suficiente) `estimate_decoherencia` no propaga la
ausencia: inventa una volatilidad de `0.0` y devuelve `Some(...)` **incondicionalmente**, es
decir un valor de slippage/decoherencia construido sobre un cero fabricado. Es exactamente la
degradación que F4 existe para impedir, y ocurre en el **consumidor** de la clave, no en el
productor.

Además `decoherencia` (la clave que ese mismo módulo lee en `:86` para penalizar) **no tiene
productor**: la penalización nunca dispara mientras la estimación se calcula con `vol = 0.0`
fabricado.

**Estado:** declarado `BLOCKING` en el ledger del gate (`check-feature-producers.ps1`). El fix
vive en `math-engine`, **fuera del alcance de escritura de F4** → registrado como conexión
pendiente #3 en `INTEGRACION-PENDIENTE.md`. No se tocó.

### 3.3 El trío de la telemetría es un contrato de QUOTE, no un hueco de features

El encargo reporta `capital_usd / costs.execution_fees / costs.financing` = 58 sobre la misma
población. Medido: esas tres etiquetas **no son claves de `MarketState.features`**. Se producen
en el validador de **quote** de cartuchos:

| Etiqueta | Sitio | Razón emitida | Significado real |
|---|---|---|---|
| `capital_usd` | `rhai_agent_bridge.rs:442-446` | `capital_differs_from_ledger_input` | el capital declarado del quote no coincide con el input del ledger |
| `costs.financing` | `rhai_agent_bridge.rs:486-490` | `already_in_retained_spread` | coste de financiación contado **dos veces** (`profit_basis == "retained_after_repayment"` + `treatment == "external"`) |
| `costs.execution_fees` | `rhai_agent_bridge.rs:497-501` | `quote_already_includes_swap_fees` | fees de ejecución contadas dos veces (`economic_kind == "atomic_quote"` + `treatment == "external"`) |

Son **reparaciones de doble conteo / consistencia**, no datos faltantes. Población distinta de la
de los 9 operadores (que nacen en `native_operator_adapter.rs:72/77/119`). Consecuencia: **ningún
productor de features mueve ese contador**, y arreglarlo no es un problema de features sino del
contrato de costes del quote.

---

## 4. Qué NO se pudo verificar (fail-honest)

| Afirmación del encargo | Estado de verificación |
|---|---|
| Telemetría en vivo del operador (`pertinent: 269, negative: 269, positive: 0, v4_edges_built: 2`; `top_repairs` sobre 58 evaluaciones) | **NO VERIFICABLE DESDE AQUÍ.** Es telemetría del entorno del operador; no hay acceso a ese runtime, a sus logs ni a su PG/Redis. Se toma como **declaración del operador**, no como medición propia. Todo lo que este documento afirma sobre los 9 operadores se midió **en el código**, no en esa telemetría. |
| `origin/main = 9202561e` | **CORREGIDO.** `git rev-parse origin/main` → `6d38a7c6fce7ab3c7511e7a027adbdd85829d33b` (2026-10-03 04:15:13 −0500). `9202561e` **es ancestro** de `origin/main` (`git merge-base --is-ancestor 9202561e origin/main` → exit 0) y está **2 commits atrás**: `2d68094b` PRICE-COVERAGE-01 (#773) y `6d38a7c6` PRICE-COVERAGE-02 (#774), que tocan sólo `cartridge_boot.rs`, `lib.rs` y `workers/price_worker.rs`. La base asignada es correcta como base, pero **no es el tip actual**. |
| Informe de F1 (medición) | **NO RECIBIDO** en esta sesión. Las mediciones que F1 debía aportar se rehicieron aquí de forma independiente sobre el código. El informe de F2 sí llegó y su matriz (`docs/verification/MARKET-FEATURES-01-matrix.md`) se contrastó línea por línea contra el árbol: sus afirmaciones sobre `math_evidence.rs:249-251`, `orchestrator.rs:708-710`, la ausencia de lector de `arbitrage_gap` y la no-duplicación de `parity_deviation` **se confirmaron**. |
| (de F2) «`symbol` y `onchain_price_usd` **ya están calculados** en `build_market_state`» | **PARCIALMENTE FALSO.** El **precio** sí (`math_evidence.rs:186-189`: `normalized_price` → `price_matrix.push`). El **símbolo NO**: lo que viaja es la dirección del token y la clave de par se construye desde direcciones (`canonical_pair_key(&in_lc, &out_lc)`, `math_evidence.rs:188`), mientras el `PriceBus` está indexado por símbolo (`PriceView::price_usd(&self, symbol: &str)`, `price_bus.rs:462`). Resoluble con `TokenIdentity::symbol_for_addr` (`token_identity.rs:182`), pero es un paso adicional. Consecuencia: la conexión del productor **no es un renglón** (`INTEGRACION-PENDIENTE.md` §1.1). |
| Validación en runtime (fork, VPS, datos vivos) | **NO.** Ninguna clave de este contrato se verificó contra un fork ni contra producción. Todas las anclas y precios de los tests de F2 son fixtures rotulados (`LIVE_DATA_VERIFIED: NO` en su matriz). |

---

## 5. Referencias cruzadas

- `REGRESION-ANTI-FABRICACION.md` — el gate ejecutable + la especificación del test de crate.
- `INTEGRACION-PENDIENTE.md` — las 5 conexiones que no se pudieron hacer, y el estado 01b/02.
- `census.json` — censo máquina-legible generado por el gate.
- F2: `docs/verification/MARKET-FEATURES-01-matrix.md` (matriz de aceptación del productor).
