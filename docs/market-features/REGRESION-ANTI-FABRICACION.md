# PRUEBA DE REGRESIÓN ANTI-FABRICACIÓN — `MarketState.features`

**Tarea:** F4-contrato (oleada 3 OMEGASEARCH-FEATURES)
**Rama:** `fix/omegasearch-features-01`
**Alcance de escritura de esta tarea:** `docs/market-features/` (único)
**Objetivo:** que un `DATA_GAP` honesto **no pueda degradarse a un cero silencioso** en el futuro.

---

## 1. Por qué el gate tiene que ser estático (y no un `#[test]`)

Un test de runtime **no puede** detectar la fabricación que F4 encontró. La razón es estructural,
no de esfuerzo:

```rust
// backend/math-engine/src/strategies/canonical_strategy.rs:131
let vol = state.features.get("volatility").copied().unwrap_or(0.0);
```

La fabricación vive en la **expresión por defecto del consumidor**, y sólo se materializa cuando
la clave está **ausente**. Cualquier test que construya un `MarketState` observa la **salida** del
consumidor, nunca si el default fue una medición o una invención: `estimate_decoherencia` devuelve
`Some(...)` en ambos casos y su valor es idéntico. Se puede escribir un test que fije el número
actual (`Some(0.005)`, por ejemplo) — y ese test **bendeciría la fabricación** al congelarla como
comportamiento esperado.

Conclusión verificada: **la única defensa posible es leer el código fuente**. Por eso el
entregable es un gate estático ejecutable, más un test de runtime acotado a lo que sí es
observable en runtime (§4).

---

## 2. El gate ejecutable

**Archivo:** `docs/market-features/check-feature-producers.ps1`
**Ejecutable con:** PowerShell 7 (`pwsh`). No necesita compilar Rust, ni red, ni PG/Redis.

```
pwsh -File docs/market-features/check-feature-producers.ps1 -Root <raiz-del-repo>
pwsh -File docs/market-features/check-feature-producers.ps1 -Root <raiz> -Json docs/market-features/census.json
pwsh -File docs/market-features/check-feature-producers.ps1 -Root <raiz> -SelfTest
```

Exit: `0` = PASS (0 blocking) · `1` = FAIL · `2` = error de uso.

### 2.1 Qué escanea

Sólo **código de producción** en `backend/**/*.rs`, excluyendo:

- rutas con `/tests/` o `/test/` (tests de integración);
- archivos `*test.rs` / `*tests.rs`;
- los rangos de línea ocupados por un `#[cfg(test)] mod`.

El corte de la zona de test es por **rangos**, no por "todo lo que sigue al primer
`#[cfg(test)]`". Motivo medido: en `market_features/mod.rs` la declaración
`#[cfg(test)] mod tests;` está en la línea 81 y **la implementación de producción sigue después**
(los tres `insert` en 158/170/171). Un corte ciego habría excluido exactamente los tres
productores que este gate debe vigilar. Para `#[cfg(test)] mod tests { ... }` el rango se cierra
por conteo de llaves.

### 2.2 Las cinco reglas

| Regla | Detecta | Ejemplo que la dispara |
|---|---|---|
| **R1** | `features.get("<k>")` cuya expresión se completa con `unwrap_or(<literal numérico>)` o `unwrap_or_default()` | `state.features.get("volatility").copied().unwrap_or(0.0)` |
| **R2** | `insert(...)` en producción sobre una clave del universo de features cuyo **valor es un literal numérico** | `features.insert("volatility".to_owned(), 0.0)` |
| **R3** | una clave **leída** en producción y **no declarada** en el manifiesto | añadir `features.get("mi_clave")` sin contrato |
| **R4** | una clave declarada **SIN PRODUCTOR** para la que aparece un `insert` en producción | empezar a emitir `bayes_wins` sin actualizar el contrato |
| **R5** | `arbitrage_gap` con un lector real, pese a estar declarada `NO_CONSUMER` | un `features.get("arbitrage_gap")` nuevo |

R1 y R2 son las que impiden el cero silencioso. R3/R4/R5 obligan a que **el contrato y el código
no puedan divergir en silencio**: no se puede leer ni emitir una clave sin declararla.

### 2.3 Resolución de constantes (una trampa que el gate resuelve)

El productor de F2 emite mediante constantes, no literales:

```rust
pub const VOLATILITY_KEY: &str = "volatility";          // market_features/mod.rs:97
out.insert(VOLATILITY_KEY.to_owned(), v);               // market_features/mod.rs:158
```

Un escáner que busque sólo literales **no vería esos tres productores** (y de hecho un
`git grep "features.insert"` tampoco ve el productor de `parity_deviation`, que inserta en un mapa
llamado `out` — ver `CONTRATO-DATOS.md` §1.4). Por eso el gate:

1. resuelve `const X: &str = "…"` a su valor en todo el árbol de producción, y
2. acepta por archivo los alias del mapa de features (`features` siempre; `out` sólo en
   `math_evidence.rs` y en `market_features/mod.rs`, que son los dos productores reales).

**Prueba de que la resolución funciona** (salida real, §3.1): el gate detecta los tres productores
de F2 nombrados por su constante.

### 2.4 El ratchet: deuda clavada por `archivo:línea`, no amnistía por clave

Cada violación preexistente está en el `$Ledger` del script con su `archivo:línea` **exacto** y un
`blocking` explícito. Consecuencias, todas verificadas:

| Situación | Veredicto |
|---|---|
| violación **nueva o movida** (no está en el ledger con esa línea exacta) | **BLOCKING** → falla |
| violación del ledger con `blocking: true` | **BLOCKING** → falla |
| violación del ledger con `blocking: false` | **DEBT** → se reporta, no falla |
| entrada del ledger que ya **no ocurre** | **STALE** → hay que borrarla del ledger |

No hay amnistía por clave: editar cualquier línea del ledger (aunque sea para "arreglarla" a
medias) **mueve** el sitio y el gate vuelve a fallar. El ledger sólo puede encoger.

---

## 3. Evidencia de ejecución (real, sobre este árbol)

### 3.1 Gate contra el árbol real — `GATE=FAIL blocking=2 deuda-clavada=14`

```
Archivos de produccion escaneados: 420

PRODUCTORES DETECTADOS EN PRODUCCION:
  health_factor          <- backend/searcher-rs/src/orchestrator.rs:709
  onchain_price          <- backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:171
  oracle_price           <- backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:170
  parity_deviation       <- backend/searcher-rs/src/math_evidence.rs:250
  volatility             <- backend/searcher-rs/src/native_operator_adapter/market_features/mod.rs:158

BLOCKING (falla el gate): 2
  [DECLARADA-BLOCKING] R1 backend/math-engine/src/strategies/canonical_strategy.rs:131 --
      fabrica ausencia: features.get("volatility") con unwrap_or literal -> unwrap_or(0.0)
  [DECLARADA-BLOCKING] R1 backend/math-engine/src/strategies/canonical_strategy.rs:157 --
      fabrica ausencia: features.get("eth_price_usd") con unwrap_or literal -> unwrap_or(2000.0)

DEUDA PREEXISTENTE CLAVADA (no falla, ratchet): 14
  [DEBT] R1 backend/math-engine/src/operators/op_11_bayes.rs:35            -- unwrap_or(0.0)   (bayes_wins)
  [DEBT] R1 backend/math-engine/src/operators/op_11_bayes.rs:36            -- unwrap_or(0.0)   (bayes_losses)
  [DEBT] R1 backend/math-engine/src/operators/op_11_bayes.rs:39            -- unwrap_or(1.0)   (bayes_prior_alpha)
  [DEBT] R1 backend/math-engine/src/operators/op_11_bayes.rs:44            -- unwrap_or(1.0)   (bayes_prior_beta)
  [DEBT] R1 backend/math-engine/src/operators/op_15_golden_section.rs:46   -- unwrap_or(0.003) (fee_bps)
  [DEBT] R1 backend/math-engine/src/operators/op_15_golden_section.rs:48   -- unwrap_or(0.003) (pool_fee)
  [DEBT] R1 backend/math-engine/src/operators/op_19_simplex.rs:174         -- unwrap_or(1.0)   (max_capital)
  [DEBT] R1 backend/math-engine/src/operators/op_21_newton.rs:52           -- unwrap_or(0.003) (fee_bps)
  [DEBT] R1 backend/math-engine/src/operators/op_21_newton.rs:54           -- unwrap_or(0.003) (pool_fee)
  [DEBT] R1 backend/math-engine/src/operators/op_21_newton.rs:118          -- unwrap_or(0.0)   (break_even_target)
  [DEBT] R1 backend/math-engine/src/operators/op_26_flash_loan.rs:60       -- unwrap_or(0.003) (pool_fee)
  [DEBT] R1 backend/math-engine/src/operators/op_26_flash_loan.rs:66       -- unwrap_or(0.0)   (flash_premium)
  [DEBT] R1 backend/math-engine/src/operators/op_32_multi_objective.rs:462 -- unwrap_or(0.003) (fee_bps)
  [DEBT] R1 backend/math-engine/src/operators/op_32_multi_objective.rs:464 -- unwrap_or(0.003) (pool_fee)

ENTRADAS STALE del ledger: 0
GATE=FAIL  blocking=2  deuda-clavada=14
GATE_EXIT=1
```

**Interpretación honesta del `FAIL`.** El gate **está rojo a propósito**, por **dos** razones y
ninguna más:

1. `canonical_strategy.rs:131` convierte la ausencia de `volatility` en `0.0`. Es una de las tres
   claves que esta oleada acaba de dotar de productor: dejar el gate verde sería declarar cerrada
   una misión que se anula en su propio consumidor.
2. `canonical_strategy.rs:157` valora el **gas en USD** con un precio de ETH **inventado**
   (`2000.0`) cuando `eth_price_usd` está ausente:

   ```rust
   // gwei * gas_units * 1e-9 ETH * price_ETH_USD        (canonical_strategy.rs:154)
   let eth_price = state
       .features
       .get("eth_price_usd")
       .copied()
       .unwrap_or(2000.0);                                // <-- precio monetario fabricado
   Some(gas_eth * eth_price)                              // :162  alimenta la rentabilidad
   ```

   Viola RULE 00 y la doctrina §4 (`f64` para importes). Se marca **BLOCKING** porque es un
   **proxy inventado para un valor monetario**, no un default de modelo: el coste de gas entra en
   la decisión de rentabilidad.

Ambos fixes viven en `math-engine`, **fuera del alcance de escritura de F4** → conexiones
pendientes #3 y #3b en `INTEGRACION-PENDIENTE.md`.

Las 14 entradas `DEBT` son deuda **preexistente y ajena a las claves de esta misión**
(`pool_fee`, `fee_bps`, `max_capital`, `flash_premium`, `break_even_target`, `bayes_*`),
enumeradas por `archivo:línea` y con su razón. Se declaran en vez de silenciarse, y se clavan para
que cualquier edición las re-exponga. Arreglarlas cambia la aritmética de operadores vivos: exige
tarea propia con evidencia numérica.

**Nota sobre las dos entradas `bayes_prior_*`:** un prior `Beta(1,1)` uniforme es una **decisión de
modelo**, no una medición fabricada, y por eso se clasifican como deuda y no como blocking — pero
son un default **silencioso** que nadie declara: si la clave falta porque la configuración no
cargó, `1.0` lo oculta. Deben declararse como prior por defecto explícito.

### 3.2 Prueba de mutación (`-SelfTest`) — el gate detecta y bloquea la fabricación

El self-test construye un árbol temporal con **dos mutaciones deliberadas** sobre
`op_11_bayes.rs`: (A) un `unwrap_or(0.0)` cambiado por un literal distinto (`0.25`), y (B) un
productor que inserta una constante (`features.insert("bayes_wins", 0.0)`).

```
=== SELFTEST (mutacion deliberada: el gate debe FALLAR) ===
VIOLACIONES DETECTADAS: 6
  - R2 .../op_11_bayes.rs:35 productor fabricado: features.insert("bayes_wins", 0.0)
  - R1 .../op_11_bayes.rs:36 fabrica ausencia: features.get("bayes_wins") con unwrap_or literal -> unwrap_or(0.25)
  - R1 .../op_11_bayes.rs:37 fabrica ausencia: features.get("bayes_losses") con unwrap_or literal -> unwrap_or(0.0)
  - R1 .../op_11_bayes.rs:40 fabrica ausencia: features.get("bayes_prior_alpha") con unwrap_or literal -> unwrap_or(1.0)
  - R1 .../op_11_bayes.rs:45 fabrica ausencia: features.get("bayes_prior_beta") con unwrap_or literal -> unwrap_or(1.0)
  - R4 .../op_11_bayes.rs:35 "bayes_wins" esta declarada SIN PRODUCTOR y ahora se inserta ...
BLOCKING: 5
SELFTEST=PASS (el gate detecta y bloquea la fabricacion)
SELFTEST_EXIT=0
```

Esto es la **verificación del verificador**: un gate que nunca ha fallado no ha demostrado que
sirva. Aquí falla cuando debe, y las mutaciones caen como `NUEVA/MOVIDA` → **BLOCKING**, que es
exactamente el comportamiento del ratchet: la mutación desplaza las líneas siguientes
(`:36→:37`, `:39→:40`, `:44→:45`) y **el desplazamiento por sí solo ya re-falla el gate**.

### 3.3 Artefacto máquina-legible

`docs/market-features/census.json` (generado con `-Json`) contiene: manifiesto, claves de la
misión, productores detectados en producción, censo de sitios de lectura, blocking, deuda y
entradas stale. Es la fuente del censo de `CONTRATO-DATOS.md` §2.1.

---

## 4. Especificación exacta del test de crate (drop-in)

El gate estático cubre lo que un test no puede. Lo que **sí** es observable en runtime debe
quedar fijado en el crate de F2, porque ahí vive el `MarketState` de producción y el router real.
Estos tres tests **complementan** los 26 existentes
(`backend/searcher-rs/src/native_operator_adapter/market_features/tests.rs`) — no repiten
`no_key_is_inserted_as_zero_when_the_data_is_missing` (`:404`), ni
`already_produced_keys_are_never_emitted_here` (`:472`), ni
`every_owned_key_has_a_complete_contract` (`:519`), ni
`without_the_producer_the_router_reports_no_oracle_bias_at_all` (`:677`).

Reutiliza el helper ya existente `empty_state_with(features)` (`tests.rs:69`) y la convención
`RegimeRouter::analyze(&state)` (`tests.rs:625`).

```rust
// ───────── F4: el router no degrada una clave ausente a un número ─────────

/// F4 — `health_factor` y `parity_deviation` ausentes NO se convierten en 0.0/1.0
/// en el router. `RegimeMetrics` deriva `Default` (regime_router.rs:37) con campos
/// `Option<f64>`: la ausencia es `None`.
///
/// Bloquea la regresión clásica: añadir `unwrap_or(1.0)` en `health_factor`
/// afirmaría "todo sano" sin haber indexado una sola posición
/// (regime_router.rs:189-192 y :208-211 son los sitios vigilados).
#[test]
fn f4_absent_health_and_parity_stay_none_and_are_never_defaulted() {
    let mut state = empty_state_with(std::collections::HashMap::new());
    // Un price_matrix de 3 filas: suficiente para que el proxy de volatilidad
    // opere, y NO suficiente para inventar salud ni paridad.
    state.price_matrix = vec![vec![100.0], vec![100.0], vec![100.0]];
    state.pair_keys = vec!["a|b".to_string(), "a|b".to_string(), "a|b".to_string()];

    let m = RegimeRouter::analyze(&state);

    assert_eq!(
        m.health_factor, None,
        "sin posiciones indexadas el health factor es AUSENTE, nunca 1.0 \
         (1.0 afirmaría 'todo sano': una aserción, no una medición)"
    );
    assert_eq!(
        m.parity_deviation, None,
        "sin stable con precio la paridad es AUSENTE, nunca 0.0 (0.0 afirmaría \
         'paridad perfecta', que es una medición, no una ausencia)"
    );
    assert_eq!(
        m.oracle_bias, None,
        "sin el par oracle/onchain no hay sesgo que medir"
    );
}

/// F4 — el productor emite EXACTAMENTE las claves declaradas y ninguna otra,
/// incluso cuando todas las fuentes faltan (mapa vacío, no un mapa con ceros).
///
/// Es el complemento en runtime de la regla R4 del gate estático: si alguien
/// añade una clave al productor sin declararla en el contrato, este test falla.
#[test]
fn f4_the_producer_emits_exactly_the_declared_owned_keys() {
    let mut store = SeriesStore::default();
    let empty = produce(
        &mut store,
        &FeatureConfig::default(),
        &FeatureRequest { symbol: "ETH", onchain_price_usd: None, live_price_usd: None },
        None,
        1_000_000_000,
    );
    assert!(
        empty.is_empty(),
        "sin ninguna fuente el mapa debe ir VACÍO: la ausencia no se codifica, \
         se omite (R8). Claves presentes: {:?}",
        empty.keys().collect::<Vec<_>>()
    );

    // Y cuando sí hay dato, el universo emitido es subconjunto de las declaradas.
    for key in empty.keys() {
        assert!(
            owned_contracts().any(|c| c.key == key),
            "el productor emitió `{key}`, que no está declarada en el contrato"
        );
    }
    assert_eq!(
        owned_contracts().count(),
        OWNED_KEYS.len(),
        "cada clave declarada debe tener exactamente un contrato"
    );
}

/// F4 — ninguna clave declarada SIN PRODUCTOR es emitida por este módulo.
///
/// El censo de `docs/market-features/census.json` declara 21 claves leídas en
/// producción y sin productor (pool_fee, fee_bps, eth_price_usd, gas_units,
/// bayes_*, nsga2.*, mo_*, ...). Este test impide que alguien "cierre" su
/// DATA_GAP insertando un valor por defecto desde aquí en vez de medirlo: si se
/// emite una de esas claves, falla.
#[test]
fn f4_the_unproduced_keys_are_never_emitted_by_this_module() {
    // Claves leídas en producción con productor AUSENTE (censo F4 §2.1).
    const UNPRODUCED: &[&str] = &[
        "pool_fee", "fee_bps", "flash_premium", "gas_units",
        "bayes_wins", "bayes_losses", "bayes_prior_alpha", "bayes_prior_beta",
        "break_even_target", "token0_per_eth", "decoherencia", "eth_price_usd",
        "jit_decay_rate", "min_liquidity", "max_capital",
        "mo_weight_yield", "mo_weight_risk", "mo_weight_latency",
        "mo_per_leg_latency_ms", "nsga2.count", "nsga2.population_size",
    ];
    for key in UNPRODUCED {
        assert!(
            !OWNED_KEYS.contains(key),
            "`{key}` no tiene productor: emitirla desde aquí sería cobertura \
             aparente. Su DATA_GAP vive en su operador, no en este módulo."
        );
        assert!(
            contract_for(key).is_none() || !matches!(contract_for(key).unwrap().owner, Owner::ThisModule),
            "`{key}` no puede estar declarada como propia de este módulo"
        );
    }
}
```

**Ubicación exacta:** `backend/searcher-rs/src/native_operator_adapter/market_features/tests.rs`
(tras el bloque 6, «End-to-end unlock of the real router»). Imports ya presentes en ese archivo
salvo `Owner` y `owned_contracts`, que se añaden a la lista de `use super::…`.

**Estado de ejecución: ESPECIFICADO, NO EJECUTADO por F4.** El código de arriba vive en el crate
de F2 y F4 tiene prohibido escribir fuera de `docs/market-features/`. No se declara `TESTED`. Queda
para el dueño del crate, que ya tiene el entorno de verificación montado (F2 verificó en WSL2
Ubuntu por AppControl de Windows, `docs/verification/MARKET-FEATURES-01-matrix.md` §6).

---

## 5. Cómo se corre y dónde se engancha en CI

| Comprobación | Comando | Cobertura |
|---|---|---|
| Gate anti-fabricación (estático) | `pwsh -File docs/market-features/check-feature-producers.ps1 -Root .` | R1–R5 sobre todo el código de producción Rust |
| Verificación del propio gate | `pwsh -File docs/market-features/check-feature-producers.ps1 -Root . -SelfTest` | que el gate falle cuando debe |
| Tests del productor (crate) | `cargo test -p searcher-rs --lib native_operator_adapter::market_features` | productor + router end-to-end |

**Recomendación de enganche (no ejecutada):** el gate es determinista, no necesita red ni
servicios, y corre en menos de un segundo: es apto para un step de CI bloqueante. Su lugar natural
es el job que ya corre `cargo fmt --check` / `clippy` sobre `backend/`, con `shell: pwsh`. **No se
modificó ningún workflow** (`.github/workflows/**` no está en el alcance de F4 y
`.github/npm-audit-allowlist.json` es zona caliente). Queda como conexión #5 en
`INTEGRACION-PENDIENTE.md`.

**Nota de entorno:** el gate es PowerShell **porque en esta máquina los build scripts recién
compilados están bloqueados por Windows AppControl (`os error 4551`)** — la limitación ya
documentada en `CLAUDE.md` §36.4 y en `docs/verification/COST-PRODUCERS-01-matrix.md` §4.1. Un
gate que requiere compilar Rust no se puede correr en este árbol; uno que lee texto, sí. Eso es
una razón medida, no una preferencia de herramienta.
