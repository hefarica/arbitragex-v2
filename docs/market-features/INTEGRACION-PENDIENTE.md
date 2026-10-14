# REGISTRO DE INTEGRACIÓN PENDIENTE + ESTADO DE LAS RAMAS 01b/02

**Tarea:** F4-contrato (oleada 3 OMEGASEARCH-FEATURES)
**Rama:** `fix/omegasearch-features-01` · worktree `omegasearch-fees`
**Alcance de escritura de esta tarea:** `docs/market-features/` (único)
**Regla cumplida:** **ningún merge ejecutado**. Este documento sólo mide y recomienda.

---

## 0. Resumen ejecutable

| # | Conexión pendiente | Archivo:línea | Qué falta exactamente | Qué desbloquea |
|---|---|---|---|---|
| 1 | Productores → camino de `math_evidence` | `orchestrator.rs:705-721` | 4 piezas: el `features.extend(...)` **+ un símbolo resuelto desde la dirección** (el precio sí está; el símbolo NO) + el bus + `now_ns`. Ver §1.1: **no son 3 líneas** | las 5 `RegimeMetrics` → clasificación de régimen → **qué operadores se recomiendan** |
| 2 | `features` → `MarketState` de los OPERADORES nativos | `cartridge_boot.rs:1384` (`:1434`), llamada en `:2475` | ensanchar la firma de `v4_market_state_from_edges` para recibir las features + fijar el símbolo de un estado multi-arista | que los **operadores nativos** (no sólo el router) vean features |
| 3 | Quitar la fabricación en el consumidor de `volatility` | `canonical_strategy.rs:131` | `unwrap_or(0.0)` → patrón `if let Some(&vol)` (el de `:78`) | que el productor de `volatility` no se anule en su propio consumidor |
| 3b | Quitar la **fabricación monetaria** del precio de ETH | `canonical_strategy.rs:157` | `unwrap_or(2000.0)` → propagar ausencia o alimentar la clave desde una fuente real | que la estimación de gas en USD no use un precio inventado |
| 4 | Deuda de ausencia→constante en 8 operadores | ver §1.4 | decisión de producto: ¿`DATA_GAP` o default declarado? | honestidad de `pool_fee` / `flash_premium` / `max_capital` / `bayes_*` |
| 5 | Enganche del gate en CI | `.github/workflows/**` (no tocado) | un step `pwsh` + required check | que R1–R5 se apliquen en cada PR |

Los puntos 1, 2 y 5 están bloqueados por **zona caliente / alcance**. El 3 requiere cambio de
aritmética en `math-engine`. El 4 es deuda preexistente ajena a estas claves.

---

## 1. Las conexiones, una por una

### 1.1 [1] Productores → `math_evidence`

**Estado medido en este árbol.** `orchestrator.rs:705-707` ya alimenta features desde fuentes
vivas y tiene exactamente dos productores:

```rust
// backend/searcher-rs/src/orchestrator.rs:705
let mut features =
    crate::math_evidence::regime_features_from_redis(&mut math_redis, chain_id).await;   // :706-707
if let Some(hf) = hf_feature {                                                            // :708
    features.insert("health_factor".to_owned(), hf);                                       // :709
}
// ... el mapa se pasa a evaluate_math_evidence en :721
```

**Las cuatro piezas que faltan (no son tres líneas).** La conexión natural sería

```rust
features.extend(
    crate::native_operator_adapter::market_features::produce_from_global(
        bus, &symbol, onchain_price_usd, now_ns,
    ),
);
```

pero **cada argumento tiene un problema medido** en el alcance de `orchestrator.rs:705-721`:

| Argumento | Estado real en ese punto | Evidencia |
|---|---|---|
| `onchain_price_usd` | **disponible**: `math_evidence::build_market_state` ya normaliza el precio de cada pierna | `math_evidence.rs:186-189` (`normalized_price` → `price_matrix.push`) |
| `symbol` | **NO disponible.** El precio viaja con la **dirección** del token y la clave de par se construye **desde direcciones** (`canonical_pair_key(&in_lc, &out_lc)`), mientras el productor —y el propio `PriceBus`— están indexados por **símbolo** (`PriceView::price_usd(&self, symbol: &str)`, `price_bus.rs:462`) | `math_evidence.rs:188`; `price_bus.rs:462` |
| el bus | **NO está en alcance.** `price_bus_global` sólo existe en el target binario (`main.rs:137`) y no se resuelve en este bloque | `mod.rs:47-75` (nota de target de F2) |
| `now_ns` | **NO está en alcance**: hay que leer el reloj de pared del `PriceBus` (el contrato de reloj está en `mod.rs:205-215`) | `mod.rs:205-215` |

**El símbolo sí es resoluble, y el patrón ya existe** —lo que falta es traerlo a este alcance:

- `shared_rs::token_identity::TokenIdentity::symbol_for_addr(&self, addr) -> Option<&str>`
  (`backend/shared-rs/src/token_identity.rs:182`), documentado en `:16` como *«metadata feed for
  the symbol-keyed price»*.
- Ya se usa en la zona caliente con el mismo propósito: `cartridge_boot.rs:2009`
  (`let Some(symbol) = identity.symbol_for_addr(&token)`).
- Y el orquestador **ya construye** ese índice, en `orchestrator.rs:1791`
  (`crate::token_identity::index_for(&mut self.ctx.math_redis.clone(), chain_id, state)`,
  encadenado con `.with_token_identity(Some(identity_idx))` en `:1794`) — pero en **otro método**,
  no en el bloque de features.

**Corrección a una premisa de F2.** La matriz de F2 afirma que «`symbol` = el token base de la
ruta; `onchain_price_usd` = la entrada de `price_matrix` para ese par — **ambos ya calculados** en
`math_evidence::build_market_state`». Medido: el **precio** sí está calculado ahí; el **símbolo
no** (lo que hay son direcciones). La conexión requiere un paso de resolución
dirección→símbolo. Se reporta porque cambia el tamaño del trabajo: no es un renglón.

**Por qué no se hizo:** `orchestrator.rs` es zona caliente, y ahora además se sabe que no basta un
renglón.
**Qué desbloquea:** las 5 métricas de `RegimeRouter` (`volatility`, `oracle_bias`,
`health_factor`, `parity_deviation`, `arbitrage_gap`). Antes de esto, `oracle_bias` era
estructuralmente `None` y `volatility` caía siempre al proxy cross-par, con lo que
`RegimeRouter::classify` degeneraba a `["Neutral"]` (2 operadores recomendados). **No desbloquea a
los 9 operadores en `DATA_GAP`** — ver §2.

### 1.2 [2] `features` → el `MarketState` que reciben los OPERADORES nativos

Este es el punto que decide si las features llegan a los operadores, y es **estructuralmente
distinto** del anterior. Hay **dos** `MarketState` en juego:

| Camino | Construido en | `features` | Quién lo consume | Estado |
|---|---|---|---|---|
| Evidencia matemática | `math_evidence::build_market_state` | alimentado por `orchestrator.rs:705-709` | `RegimeRouter` (evidencia de régimen) | 2 productores vivos |
| **v4 / operadores nativos** | `cartridge_boot.rs:1384` `v4_market_state_from_edges` | **`HashMap::new()` en `:1434`** | `native_operator_adapter::evaluate_declared` → `registry.dispatch(id, state)` → **los 31 operadores** | **VACÍO** |

```rust
// backend/searcher-rs/src/cartridge_boot.rs (idéntico en la base y en origin/main)
fn v4_market_state_from_edges(                       // :1384
    edges: &[crate::agent_graph::Edge],
    block_number: u64,
    gas_price_gwei: f64,
) -> Option<std::sync::Arc<math_engine::MarketState>> {
    ...
    Some(std::sync::Arc::new(MarketState {
        price_matrix,
        pair_keys,
        liquidity_reserves,
        gas_price_gwei,
        block_timestamp: 0,                                   // :1432 honesto (no viaja)
        block_number,
        features: std::collections::HashMap::new(),           // :1434  <-- AQUÍ
    }))
}
```

**Qué falta exactamente**, en tres partes:

1. **La firma no tiene por dónde recibirlo.** `v4_market_state_from_edges(edges, block_number,
   gas_price_gwei)` no recibe ni bus, ni símbolo, ni features. Hay que ensancharla (p. ej. un
   parámetro `features: &HashMap<String,f64>` o `bus`+`symbol`) y construir el estado con ellas.
2. **El símbolo hay que decidirlo.** La función agrega **muchas** aristas (`e.token_in` /
   `e.token_out`) en **un** `MarketState` con **un** mapa de features; el productor
   (`produce_from_global`) toma **un** símbolo por llamada. Decidir a qué símbolo se refieren las
   features de un estado multi-arista es una **decisión de diseño**, no un renglón mecánico. Se
   declara como tal en vez de prometer una línea.
3. **El bus ya está disponible… en `origin/main`, no en esta base.** Ver §1.3. La llamada al
   constructor del estado vive en `build_and_register_intent_context` (`:2385` en esta base,
   `:2454` en `origin/main`) y el call site concreto es `:2475` en la base / `:2544` en
   `origin/main`.

**Por qué no se hizo:** `cartridge_boot.rs` es zona caliente (y la conexión no es de una línea).
**Qué desbloquea:** que `features` deje de nacer vacío **en el estado que despachan los
operadores**.

### 1.3 Hallazgo: la base asignada no contiene el commit que hace posible la conexión [2]

Medido sobre `origin/main` (= `6d38a7c6`, 2 commits por delante de la base `9202561e`):

```
git show origin/main:backend/searcher-rs/src/cartridge_boot.rs | Select-String price_bus_global
  1954: /// **PriceBus en proceso** (`price_bus_global::get()`, Binance WS bookTicker
  2588: let price_bus = crate::price_bus_global::get();
```

Y la llamada que construye el estado de los operadores está 44 líneas antes, **en la misma
función**:

```
2454: pub async fn build_and_register_intent_context(          // firma (6 parámetros, sin bus)
2544: v4_dispatch_state = v4_market_state_from_edges(&v4_edges, block, v4_intent_gas_gwei);
2588: let price_bus = crate::price_bus_global::get();
2589: let Some(bundle) = build_v4_intent_bundle( ... price_bus ... );
```

Consecuencias, todas verificadas:

- El bus que necesitan los tres productores (`PriceView::price_usd`, `PriceSnapshot.chainlink`)
  **ya se resuelve dentro de `build_and_register_intent_context`** en `origin/main`, gracias a
  **PRICE-COVERAGE-01 (#773)**. Subir esa resolución por encima de la línea 2544 la pone en el
  alcance del sitio de construcción del estado v4.
- Ese commit **no está en la base asignada** (`9202561e`). Medición directa sobre la base:
  `Select-String -Path backend/searcher-rs/src/cartridge_boot.rs -Pattern price_bus` →
  **0 coincidencias** (la cadena `price_bus` **no aparece en ninguna línea** de ese archivo en la
  base); el commit que la introduce es PRICE-COVERAGE-01 (`2d68094b`, #773), que toca
  `cartridge_boot.rs` + `lib.rs` y sólo existe en `origin/main`.
- **Recomendación derivada:** quien haga la conexión [2] debe partir de `origin/main` actual (o al
  menos de `2d68094b`), no de `9202561e`. Hacerlo sobre la base obliga a inventar el acceso al bus
  que ya existe aguas abajo.

**Corrección al dato de base del encargo.** El encargo dice «`origin/main` = `9202561e`». Medido:
`git rev-parse origin/main` → `6d38a7c6fce7ab3c7511e7a027adbdd85829d33b` (2026-10-03 04:15:13
−0500). `9202561e` **es ancestro** de `origin/main` (`git merge-base --is-ancestor 9202561e
origin/main` → exit 0) y está **2 commits atrás**. Los dos commits son
**PRICE-COVERAGE-01** (`2d68094b`, #773) y **PRICE-COVERAGE-02** (`6d38a7c6`, #774), y tocan
**sólo** `cartridge_boot.rs`, `lib.rs` y `workers/price_worker.rs` — los tres, zona caliente.

### 1.4 [3] y [4] — las fabricaciones que el gate deja rojas

**Bloqueantes y declaradas (2):**

| Sitio | Clave | Constante fabricada | Por qué es blocking y no deuda |
|---|---|---|---|
| `canonical_strategy.rs:131` | `volatility` | `0.0` | es una de las **tres claves que esta misión produce**; el consumidor anula el entregable |
| `canonical_strategy.rs:157` | `eth_price_usd` | `2000.0` | **fabricación monetaria**: valora el gas en USD con un precio de ETH inventado, y ese coste entra en la decisión de rentabilidad. Viola RULE 00 y la doctrina §4 |

```rust
// canonical_strategy.rs:154-162 — el segundo bloqueante
// gwei * gas_units * 1e-9 ETH * price_ETH_USD
let eth_price = state.features.get("eth_price_usd").copied().unwrap_or(2000.0);
let gas_units = base_gas * (1.0 + self.max_legs as f64 * 0.1);
let gas_eth = state.gas_price_gwei * gas_units * 1e-9;
Some(gas_eth * eth_price)
```

**Fix del bloqueante #3:** el patrón correcto ya existe 53 líneas antes en el mismo archivo
(`:78`: `if let Some(&vol) = … { if vol.is_finite() { … } }`). **Fix del bloqueante #3b:** propagar
la ausencia (`Option`) en vez de inventar un precio, o alimentar `eth_price_usd` desde una fuente
real. Ambos viven en `math-engine`, fuera del alcance de escritura de F4.

**Deuda clavada, no bloqueante (14):**

| Sitio | Clave | Constante que fabrica | Efecto |
|---|---|---|---|
| `op_11_bayes.rs:35` | `bayes_wins` | `0.0` | enmascarado aguas abajo por `wins+losses<1.0` → `reason_no_history`; el mecanismo es **implícito** |
| `op_11_bayes.rs:36` | `bayes_losses` | `0.0` | ídem |
| `op_11_bayes.rs:39` | `bayes_prior_alpha` | `1.0` | prior `Beta(1,1)`: **decisión de modelo**, no medición — pero default silencioso que oculta una config que no cargó |
| `op_11_bayes.rs:44` | `bayes_prior_beta` | `1.0` | ídem |
| `op_15_golden_section.rs:46` | `fee_bps` | `0.003` | computa con 30 bps inventados |
| `op_15_golden_section.rs:48` | `pool_fee` | `0.003` | ídem |
| `op_19_simplex.rs:174` | `max_capital` | `1.0` | computa con 1 USD de capital |
| `op_21_newton.rs:52` | `fee_bps` | `0.003` | ídem |
| `op_21_newton.rs:54` | `pool_fee` | `0.003` | ídem |
| `op_21_newton.rs:118` | `break_even_target` | `0.0` | umbral de break-even inventado en cero |
| `op_26_flash_loan.rs:60` | `pool_fee` | `0.003` | ídem |
| `op_26_flash_loan.rs:66` | `flash_premium` | `0.0` | computa un flash **gratis** |
| `op_32_multi_objective.rs:462` | `fee_bps` | `0.003` | ídem |
| `op_32_multi_objective.rs:464` | `pool_fee` | `0.003` | ídem |

Son **preexistentes y ajenas a las claves de esta misión**. No se tocan aquí por dos razones
medidas: (a) `math-engine` está fuera del alcance de escritura de F4; (b) arreglarlas **cambia la
aritmética de operadores vivos** (`pool_fee`/`fee_bps` alimentan a op_15/op_21/op_26/op_32), así
que exige tarea propia con evidencia numérica antes/después, no un cambio de paso.

### 1.5 [5] Enganche del gate en CI

El gate (`check-feature-producers.ps1`) es determinista, sin red y sin servicios, y corre en menos
de un segundo: es apto para un step bloqueante. **No se modificó ningún workflow**: `.github/**`
está fuera del alcance de F4 y `.github/npm-audit-allowlist.json` es zona caliente. Queda
especificado en `REGRESION-ANTI-FABRICACION.md` §5.

---

## 2. La conexión que falta para que los 9 operadores reciban datos

Pregunta del encargo: *«qué conexión falta para que los 9 operadores DATA_GAP puedan recibir
datos»*. Respuesta medida, y **corrige la premisa**:

**No falta una conexión de features.** Falta **otra cosa en cada subgrupo**, y para 6 de los 9 no
es una conexión en absoluto:

| Subgrupo | Operadores | Qué leen realmente | Qué falta para que reciban datos |
|---|---|---|---|
| 6 operadores **sin features** | 22, 16, 10, 5, 8, 13 | sólo `price_matrix` (guardas `price_matrix.is_empty()` en `op_22:26`, `op_16:23`, `op_10:25`, `op_05:27`, `op_08:28`, `op_13:22`) | **nada de features.** Su `DATA_GAP` nace en la admisión/despacho (`native_operator_adapter.rs:72/77/119`) o en su propia aritmética. Es **la línea de investigación siguiente**, no ésta. |
| 2 operadores de **coste/fee** | 21, 26 | `pool_fee` (+`flash_premium` en op_26); op_21 además exige `liquidity_reserves` no vacío (`op_21:87-88` → `no_reserves`) | **un productor de `pool_fee`/`flash_premium`** en el camino v4. Ni F2 ni el encargo lo cubren: es dominio de coste/fee y **hoy no existe productor** (`CONTRATO-DATOS.md` §2.1). |
| 1 operador de **histórico** | 11 | `bayes_wins`/`bayes_losses` | **su propio productor** (histórico de aciertos/fallos). No existe. |

**Y para los tres subgrupos, el bloqueo común es el mismo:** el `MarketState` que reciben viene de
`cartridge_boot.rs:1384` con `features: HashMap::new()` (`:1434`). Aunque existiera el productor de
`pool_fee`, **no llegaría a op_21/op_26** sin la conexión [2] (§1.2).

### 2.1 Qué NO es la causa (medido)

- **La admisión estructural no mira features.** `V4StructuralInputAdmission::validate`
  (`cartridge_boot.rs:1446-1471`) valida sólo: `price_matrix` no vacío, `pair_keys` alineado, gas
  finito y positivo, `block_number != 0`. Su propio doc-comment declara el límite: *«Un operador
  que consuma inputs no cubiertos por esta validación estructural reparte su propio DATA_GAP»*
  (`cartridge_boot.rs:1443-1444`). Por eso producir features **no cambia la admisión**.
- **El trío `capital_usd / costs.execution_fees / costs.financing` no son claves de features** y no
  son datos faltantes: son reparaciones de **doble conteo** del validador de QUOTE
  (`rhai_agent_bridge.rs:442-446`, `:486-490`, `:497-501`). Detalle en `CONTRATO-DATOS.md` §3.3.
  Ningún productor de features mueve ese contador.

---

## 3. Estado de las ramas 01b y 02 — medición y recomendación

**Ningún merge ejecutado** (orden explícita del encargo).

### 3.1 Medición de `git cherry` contra mi HEAD

```
$ git cherry HEAD <rama>          # '-' = el parche YA está en HEAD · '+' = no está
fix/features-01a-parity-01              -> - b421ffd6
fix/features-01c-volatility-feature     -> - e0e24e01
fix/features-01b-pair-grouping          -> + 3c593283   + b101695b
fix/features-02-health-factor-01        -> + bbbeb09a   + 06985034   + c042f7d1
tmp/revert-features-02-health-factor-01 -> + bbbeb09a   + 06985034   + c042f7d1
```

`9202561e` es ancestro de HEAD (`git merge-base --is-ancestor 9202561e HEAD` → exit 0), así que la
comparación es sobre la base correcta.

### 3.2 El `+` de 01b/02 es un artefacto de SHA, no contenido faltante (verificado)

Un `+` en `git cherry` significa «este *patch-id* no está en HEAD», **no** «este contenido falta».
Cuando el trabajo se mergeó por PR con otro SHA (squash/rebase), el patch-id cambia aunque el
contenido sea idéntico. Comprobación por **blobs** (`git diff HEAD:<f> <rama>:<f>` vacío ⇒
byte-idéntico):

**01b (`b101695b`, tip de rama = `3c593283` + rustfmt). Tocó 10 archivos:**

| Archivo | `git diff HEAD: <01b>:` |
|---|---|
| `matrix/topology_map.rs` | **IDÉNTICO** |
| `operators/mod.rs` | **IDÉNTICO** |
| `operators/op_11_bayes.rs` | **IDÉNTICO** |
| `operators/op_12_mle.rs` | **IDÉNTICO** |
| `operators/op_14_kl_divergence.rs` | **IDÉNTICO** |
| `operators/op_32_multi_objective.rs` | **IDÉNTICO** |
| `operators/op_32_nsga2/mod.rs` | **IDÉNTICO** |
| `operators/real_ops_tests.rs` | **IDÉNTICO** |
| `control/regime_router.rs` | difiere — y **HEAD es el superconjunto**: el diff es exactamente el bloque de FEATURES-01c (preferencia de `volatility` medida), que HEAD **tiene** (`regime_router.rs:107-111`) y el tip de 01b **no**, porque 01b ramificó antes de 01c |
| `math_evidence.rs` | difiere — HEAD es 242 líneas **más grande**; las dos funciones que 01b añadió existen en HEAD en `math_evidence.rs:52` (`normalized_price`) y `:85` (`canonical_pair_key`) |

Y **la feature de 01b está en HEAD, leída directamente**: agrupación del gap por par en
`regime_router.rs:137-186`, con `pair_keys` en el struct (`operators/mod.rs:88`).

**02 (`c042f7d1`, tip = neto de los 3 commits). Tocó 1 archivo:**

| Commit | Qué hace | Diffstat |
|---|---|---|
| `bbbeb09a` | alimenta `health_factor` desde el indexer cacheado | `orchestrator.rs` +40/−1 |
| `06985034` | re-añade `regime_features_from_redis` junto a `health_factor` («union de productores, no theirs-only») | `orchestrator.rs` +310/−1 |
| `c042f7d1` | **Revert** del anterior | `orchestrator.rs` −310/+1 |

Neto de los 3 = sólo el contenido de `bbbeb09a`. Y **está en HEAD, leído directamente**:
`orchestrator.rs:661-673` (cálculo del `health_factor` desde el indexer, publicando el **mínimo**)
y `:708-710` (`features.insert("health_factor", hf)`), con el comentario de la propia rama
(`:700-704`).

### 3.3 Recomendación

| Rama | Recomendación | Fundamento |
|---|---|---|
| `fix/features-01a-parity-01` | **Descartar** (ya en main) | `git cherry` → `-` |
| `fix/features-01c-volatility-feature` | **Descartar** (ya en main) | `git cherry` → `-` |
| `fix/features-01b-pair-grouping` | **Descartar como candidata a merge.** Nada que mergear: 8/10 archivos byte-idénticos y los otros 2 son superconjunto en HEAD. Su feature está viva en `regime_router.rs:137-186` | §3.2 |
| `fix/features-02-health-factor-01` | **Descartar como candidata a merge. PROHIBIDO cherry-pickear `c042f7d1`**: es un `Revert` que borraría las 310 líneas que `06985034` reintrodujo. Su contenido útil (`bbbeb09a`) ya está en HEAD | §3.2 + `git show --stat` de los 3 |
| `tmp/revert-features-02-health-factor-01` | **Descartar.** Rama temporal enredada; mismo contenido que 02 | `git cherry` idéntico |

**No se recomienda «re-hacer» nada**: el contenido de las 5 ramas está en main. **No se recomienda
mergear nada.** El único riesgo real es operativo: alguien que vea `+` en `git cherry` y
«complete» mergeando podría **revertir el productor de paridad** al traer `c042f7d1`.

**Borrado de ramas: NO ejecutado** (fuera del alcance de F4 y destructivo). Si el capitán lo
quiere, la secuencia segura es `git branch -d` (no `-D`) tras confirmar que ninguna tiene trabajo
sin publicar — y `fix/features-02-health-factor-01` **tiene** un `remotes/origin/` homónimo, así
que el borrado local no pierde el remoto.

---

## 4. Qué NO se pudo verificar (fail-honest)

| Afirmación | Estado |
|---|---|
| Telemetría en vivo del operador (`v4_edges_built: 2`, `top_repairs` sobre 58 evaluaciones) | **NO VERIFICABLE DESDE AQUÍ**: es telemetría del entorno del operador. Se toma como declaración, no como medición. Nada de §2 depende de ella: todo se midió en el código. |
| Que la conexión [1] baste para desbloquear a los 9 operadores | **FALSO, y así se reporta.** §2. |
| Que la conexión [1] sea de 3 líneas | **CORREGIDO.** El símbolo que exige el productor **no está** en el alcance de `orchestrator.rs:705-721`: ahí lo que viaja es la **dirección** del token (`math_evidence.rs:188`), mientras el `PriceBus` está indexado por **símbolo** (`price_bus.rs:462`). Requiere un paso dirección→símbolo (resoluble con `TokenIdentity::symbol_for_addr`, `token_identity.rs:182`) más el bus y `now_ns`. §1.1. |
| Que el wiring [2] sea mecánico | **NO es mecánico**: exige ensanchar una firma y decidir el símbolo de un estado multi-arista (§1.2). |
| Rama 01b/02 «faltan en main» | **CORREGIDO**: los SHAs no son ancestros, pero el **contenido sí está** (§3.2). |
| `origin/main = 9202561e` | **CORREGIDO**: `origin/main = 6d38a7c6`, 2 commits por delante (§1.3). |
| Ejecución real del test Rust especificado | **NO EJECUTADO** por F4 (vive en el crate de F2; F4 escribe sólo en `docs/market-features/`). Estado declarado: `SPECIFIED`. |
| Modificación de workflows de CI | **NO HECHA** (fuera de alcance). |
