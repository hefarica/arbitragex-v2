//! MARKET-FEATURES-01 — declarative contract for every `MarketState.features`
//! key in play, and the machine-checked record of WHO produces each one.
//!
//! ## Why this file exists
//!
//! `MarketState.features` is a `HashMap<String, f64>` (`math-engine/src/
//! operators/mod.rs:98`). A `HashMap` cannot tell "absent" from "zero", so the
//! only thing standing between a missing datum and a fabricated one is the
//! discipline of the writers. This module turns that discipline into data: each
//! key declares its unit, its authoritative source, the window it refers to,
//! and — the important one — what an ABSENT key means. A test
//! (`tests::every_owned_key_is_declared`) fails if `produce` can emit a key
//! with no contract, and (`tests::already_produced_keys_are_never_emitted_here`)
//! fails if this module starts re-producing a key somebody else already owns.
//!
//! ## What was measured before writing a single producer
//!
//! On the branch base (`9202561e` + the FEATURES merges already in `origin/main`)
//! the searcher's feature map is written in exactly three places, not one:
//!
//! | key | written by | line |
//! |---|---|---|
//! | `parity_deviation` | `math_evidence::regime_features_from_redis` ← Redis `arbx:token_prices:<chain>` | `math_evidence.rs:249-251` |
//! | `health_factor` | the liquidation indexer (cached), via the orchestrator | `orchestrator.rs:661-673`, inserted at `:708-710` |
//! | *(everything else)* | — | — |
//!
//! So the premise "only `health_factor` has a producer" is **false on this
//! base**: `parity_deviation` is already produced and reimplementing it here
//! would be duplication, not implementation. And `arbitrage_gap` is a third
//! case entirely — it has NO consumer, because FEATURES-01b made
//! `RegimeRouter::analyze` compute it internally from `pair_keys`
//! (`regime_router.rs:153-186`). Writing that key would be dead code.
//!
//! The genuinely unwritten keys are the three this module owns:
//! `volatility`, `oracle_price`, `onchain_price`.

/// Who produces a key TODAY, as measured on the tree — not as intended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// Produced by `native_operator_adapter::market_features` (this module).
    ThisModule,
    /// Already produced elsewhere. Reimplementing it here would be duplication
    /// of a live producer, so this module never emits it.
    Elsewhere(&'static str),
    /// No consumer reads this key, so producing it would be dead code that
    /// merely looks like coverage.
    NoConsumer(&'static str),
}

/// The full contract of one `MarketState.features` key.
#[derive(Debug, Clone, Copy)]
pub struct FeatureContract {
    /// Exact `HashMap` key.
    pub key: &'static str,
    /// Physical unit of the value in the `f64` slot.
    pub unit: &'static str,
    /// Authoritative source of the datum (the only writer allowed to feed it).
    pub source: &'static str,
    /// Temporal window / block the value refers to.
    pub window: &'static str,
    /// What an ABSENT key means. R8: absence is never encoded as `0.0`.
    pub absent_means: &'static str,
    /// True when the value is a monetary magnitude in USD.
    ///
    /// Doctrine §4 forbids `f64` for monetary amounts. `MarketState.features`
    /// is `HashMap<String, f64>` by design of the math-engine, so a monetary
    /// feature cannot avoid the type. The limitation is therefore recorded
    /// here and constrained by the contract: the two monetary keys this module
    /// owns are **unit prices** (USD per token), never amounts, and their only
    /// consumer uses them in a **scale-invariant ratio**
    /// (`|oracle − onchain| / onchain`, `regime_router.rs:201`). They must
    /// never be summed, netted, or used for accounting — for that the exact
    /// integer path (wei/`u256`) remains the only authority.
    pub monetary: bool,
    pub owner: Owner,
}

/// Every `MarketState.features` key this project currently has an opinion about,
/// with the evidence for that opinion.
pub const CONTRACTS: &[FeatureContract] = &[
    // ── Owned by this module ───────────────────────────────────────────────
    FeatureContract {
        key: "volatility",
        unit: "adimensional — desviación estándar MUESTRAL (n-1) de los retornos \
               logarítmicos por observación; NO anualizada",
        source: "observaciones reales del precio USD fusionado del PriceBus \
                 (Binance bookTicker × ancla del quote) muestreadas por este \
                 módulo; el PriceBus es la única fuente",
        window: "rodante — las últimas `window_ns` (default 900 s) de \
                 observaciones, requiriendo >= `min_samples` (default 8) y un \
                 span real >= `min_span_ns` (default 60 s) entre la primera y \
                 la última muestra",
        absent_means: "sin serie suficiente para un eje temporal real (< muestras \
                       mínimas, o span demasiado corto, o precios inválidos): \
                       `RegimeRouter::analyze` cae a su proxy cross-venue \
                       (regime_router.rs:115-135). Nunca 0.0 — un 0.0 afirmaría \
                       'mercado plano', que es una medición, no una ausencia",
        monetary: false,
        owner: Owner::ThisModule,
    },
    FeatureContract {
        key: "oracle_price",
        unit: "USD por token (precio unitario)",
        source: "ancla Chainlink del PriceBus (`PriceSnapshot.chainlink[SYM].answer`, \
                 ya ajustada por decimales), escrita por \
                 `workers/price_worker.rs::fetch_chainlink` desde \
                 `latestRoundData()` de los oráculos configurados en PG",
        window: "la ronda del agregador referida por `Anchor.updated_at`, \
                 aceptada solo si es fresca: <= 3900 s para volátiles y \
                 <= 90000 s para stables (heartbeats reales de Chainlink)",
        absent_means: "sin ancla para el símbolo, o con edad mayor al heartbeat \
                       de su clase: la clave NO se inserta y `oracle_bias` queda \
                       en `None` (regime_router.rs:196-203). Nunca un precio \
                       fabricado ni 0.0",
        monetary: true,
        owner: Owner::ThisModule,
    },
    FeatureContract {
        key: "onchain_price",
        unit: "USD por token (precio unitario)",
        source: "precio del manifold de liquidez DEX derivado de las reservas \
                 REALES del pool (`reserves_cache`) normalizado por los \
                 decimales de ambos tokens en \
                 `math_evidence::build_market_state` — el mismo valor que ya \
                 viaja en `price_matrix`",
        window: "el bloque/tick del snapshot de reservas que alimentó \
                 `build_market_state` (el mismo `block_number` del `MarketState`)",
        absent_means: "ninguna pata de pool con reservas y decimales conocidos: \
                       la clave NO se inserta y `oracle_bias` queda en `None`. \
                       `oracle_price` y `onchain_price` se emiten JUNTOS o no se \
                       emiten: el único consumidor exige ambos, y media medición \
                       afirmaría un sesgo que nadie midió",
        monetary: true,
        owner: Owner::ThisModule,
    },
    // ── Already produced elsewhere: reimplementing would be duplication ────
    FeatureContract {
        key: "parity_deviation",
        unit: "adimensional — |precio − 1| de un activo anclado",
        source: "YA PRODUCIDO — `math_evidence::regime_features_from_redis` \
                 (math_evidence.rs:249-251) leyendo el hash Redis \
                 `arbx:token_prices:<chain>` del PriceBus",
        window: "el último valor publicado del hash Redis (TTL del PriceBus)",
        absent_means: "ningún stable con precio parseable en el hash: el mapa va \
                       vacío y la métrica queda en `None` (FEATURES-01a)",
        monetary: false,
        owner: Owner::Elsewhere(
            "math_evidence::regime_features_from_redis (math_evidence.rs:249-251) \
             <- arbx:token_prices:<chain>",
        ),
    },
    FeatureContract {
        key: "health_factor",
        unit: "adimensional — ratio de salud (deuda/colateral ponderado); <1 es \
               liquidable",
        source: "YA PRODUCIDO — el indexer CACHEADO del motor de liquidación, \
                 leído en orchestrator.rs:661-673 e insertado en orchestrator.rs:708",
        window: "la última indexación de posiciones de lending del motor \
                 (cache), referida a las posiciones IMPACTADAS por este intent",
        absent_means: "sin posiciones de lending impactadas o sin entrada en el \
                       indexer: `hf_feature` queda en `None` y la clave NO se \
                       inserta. Nunca 1.0 — eso afirmaría 'todo sano', que es \
                       una aserción, no una ausencia",
        monetary: false,
        owner: Owner::Elsewhere(
            "orchestrator.rs:661-673 (indexer de liquidación) -> insert en orchestrator.rs:708-710",
        ),
    },
    // ── No consumer: producing it would be dead code ──────────────────────
    FeatureContract {
        key: "arbitrage_gap",
        unit: "adimensional — max/min − 1 entre venues del MISMO par",
        source: "NO ES UNA FEATURE: `RegimeRouter::analyze` lo COMPUTA \
                 internamente desde `state.pair_keys` + `price_matrix` \
                 (regime_router.rs:153-186, FEATURES-01b)",
        window: "el bloque del `MarketState` (mismo snapshot de precios)",
        absent_means: "ningún par con >=2 venues: la métrica queda en `None`. \
                       Insertarlo en `features` no tendría lector: ninguna línea \
                       del repo hace `features.get(\"arbitrage_gap\")`, así que \
                       producirlo sería cobertura aparente, no cobertura",
        monetary: false,
        owner: Owner::NoConsumer(
            "RegimeRouter::analyze computed it internally since FEATURES-01b \
             (regime_router.rs:153-186); no `features.get` reader exists",
        ),
    },
];

/// The keys this module is the authoritative producer of.
pub const OWNED_KEYS: &[&str] = &["volatility", "oracle_price", "onchain_price"];

/// Look up the contract of one key.
pub fn contract_for(key: &str) -> Option<&'static FeatureContract> {
    CONTRACTS.iter().find(|c| c.key == key)
}

/// Contracts this module owns.
pub fn owned_contracts() -> impl Iterator<Item = &'static FeatureContract> {
    CONTRACTS
        .iter()
        .filter(|c| matches!(c.owner, Owner::ThisModule))
}
