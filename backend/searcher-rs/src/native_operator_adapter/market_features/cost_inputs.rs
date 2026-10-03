//! MARKET-FEATURES-01 / tarea **F7** — los productores que faltaban del censo.
//!
//! # Por qué existe este archivo
//!
//! El censo del gate (`docs/market-features/check-feature-producers.ps1`) declaró
//! 21 claves `ABSENT`. De ellas, CINCO tienen lectores reales en producción y por
//! tanto valor económico; el resto son claves que nadie lee (ver el informe de
//! F7: producir aquéllas sería cobertura aparente). Las cinco con lector:
//!
//! | clave | lectores (medidos) | unidad que el LECTOR espera |
//! |---|---|---|
//! | `pool_fee` | `op_15_golden_section.rs:48`, `op_21_newton.rs:54`, `op_26_flash_loan.rs:60`, `op_32_multi_objective.rs:464` | **fracción** (`gamma = 1.0 − fee`, op_26:61) |
//! | `fee_bps` | `op_15:46`, `op_21:52`, `op_32:462` | **basis points** (`*bps / 10_000.0`) |
//! | `flash_premium` | `op_26:66` | **fracción** (`repayment = 1.0 + phi`, op_26:67) |
//! | `max_capital` | `op_19_simplex.rs:174` | **unidades mínimas (raw) del token0** — misma familia que `b[1+j] = liquidity_reserves[j].0` (op_19:188) |
//! | `break_even_target` | `op_21_newton.rs:118` | **[token0 numerary]** — unidad DECLARADA por el propio consumidor (op_21:123) |
//!
//! Las cinco unidades están por tanto LEÍDAS del consumidor, no supuestas. Ésa
//! es la razón por la que este módulo puede producir las cinco: una clave sin
//! unidad declarada por su lector no se produce (sería un proxy con un número
//! plausible).
//!
//! # Dos familias de fuente, ambas autoritativas
//!
//! 1. **Fee del pool** (`pool_fee` / `fee_bps`) — la fuente es el par
//!    `(fee_units, fee_denominator)` que el grafo ya leyó del despliegue
//!    (`agent_graph.rs:23-24`, poblado por `cartridge_boot.rs:1756`
//!    `v4_edge_protocol_and_fee`) y que **ya está en uso** como lectura
//!    autoritativa del fee embebido en `snapshot_services.rs:345`:
//!
//!    ```text
//!    let (Some(fee), Some(den)) = (edge.fee_units, edge.fee_denominator) else { … }
//!    fee_raw = amount_in_raw × fee_units / fee_denominator
//!    ```
//!
//!    Este módulo **reutiliza esa misma lectura** (no la duplica ni la
//!    reinterpreta): `PoolFeeRead` es exactamente ese par, y la fracción se
//!    deriva con la misma aritmética. `None` en cualquiera de los dos campos es
//!    el `missing_fee_units` honesto que el repo ya nombra
//!    (`snapshot_services.rs:345`, `cartridge_boot.rs:1623`): **no hay fee
//!    inferida**.
//!
//! 2. **Premium de financiación flash** (`flash_premium`) — la fuente
//!    autoritativa es la lectura ON-CHAIN del proveedor, jamás un literal. El
//!    prompt §5 nombra el caso Aave y el repo ya tiene DOS lectores reales:
//!
//!    * `shared-rs/src/flashloan_math.rs:165` →
//!      `call_word(provider, pool, "FLASHLOAN_PREMIUM_TOTAL()", &[], block_hash)`
//!    * `relays-client/src/plan_validation.rs:336` →
//!      `read_uint(provider, pool, "FLASHLOAN_PREMIUM_TOTAL()", &[], hash)`
//!
//!    Denominador 10_000 (`cost_producers/funding.rs:54`,
//!    `shared-rs/src/flashloan_math.rs:18`). El literal
//!    `financing::AAVE_FLASH_LOAN_FEE_BPS = 5.0`
//!    (`backend/searcher-rs/src/financing.rs:38`) es exactamente el valor que
//!    §5 PROHÍBE usar como fuente: este módulo no lo importa ni lo menciona como
//!    valor.
//!
//! # `flash_premium`: por qué es `BLOCKED_EXTERNAL` y no un número
//!
//! La lectura autoritativa exige `Provider<Http>` + `block_hash` + la dirección
//! del pool, y **hoy no está cableada al camino del intent**: el propio
//! `cartridge_boot.rs:2107` la declara pendiente («el premium real
//! (`FLASHLOAN_PREMIUM_TOTAL`) sigue pendiente»). Un productor puro y síncrono
//! —que es lo que este módulo es, y debe seguir siendo para ser testeable sin
//! arrancar `main.rs`— no puede hacer I/O.
//!
//! Por eso la clave se produce **sólo si la lectura llega como dato**
//! ([`PremiumRead`]) y, cuando no llega, el productor **omite la clave** y
//! publica el requisito exacto ([`flash_premium_requirement`]) con estado
//! `BLOCKED_EXTERNAL`. Lo que NUNCA ocurre es un `0.0`: un cero sólo se emite
//! con `zero_attested` (lectura presente = 0 **más** procedencia citada),
//! exactamente la semántica de `cost_producers::funding::ProviderReads::zero_attested`
//! (`funding.rs:154-158`). Un `0.0` sin esa acreditación sería «financiación
//! flash gratuita», la fabricación que el prompt §5 prohíbe de forma textual.
//!
//! # Doctrina §4 — `f64` y dinero
//!
//! `MarketState.features` es `HashMap<String, f64>` por diseño del math-engine
//! (`math-engine/src/operators/mod.rs:98`), así que una clave monetaria no puede
//! evitar el tipo. `max_capital` y `break_even_target` son MAGNITUDES, y la
//! limitación queda declarada en su contrato (`contract.rs`, `monetary: true`)
//! con la regla dura: **son entradas de un solver acotado, jamás contabilidad**.
//! Para neto, reparto o cualquier asiento, la única autoridad sigue siendo el
//! camino entero (`U256`/unidades mínimas) de `economics::cost_producers`.
//!
//! # Reuso cruzado verificado (`fix/cost-producers-01`)
//!
//! `git diff --numstat origin/main...origin/fix/cost-producers-01` = 141 líneas
//! en `cartridge_boot.rs` + 58 en `snapshot_services.rs`, y ese diff produce
//! `CostLine` (gas/financing/execution_fees) para el bridge Rhai — **no** produce
//! ninguna clave de `features`. No hay por tanto productor de fee de pool que
//! reusar en esa rama; la lectura que sí es autoritativa y compartida es el par
//! `(fee_units, fee_denominator)`, y es la que este archivo reutiliza.

/// Basis points por unidad (denominador de Aave `PercentageMath.PERCENTAGE_FACTOR`).
///
/// Se **reexporta** desde el productor autoritativo de financiación para que el
/// repo tenga UN sólo denominador de bps en el camino de costes; duplicarlo
/// abriría la puerta a que los dos divergieran.
pub const BPS_DENOMINATOR: u64 = crate::economics::cost_producers::funding::BPS_DENOMINATOR;

/// Claves que produce este archivo. `const &str` (y no literales en el `insert`)
/// para que el gate anti-fabricación resuelva la clave desde su mapa de
/// constantes — el mismo mecanismo que ya usa `VOLATILITY_KEY`.
pub const POOL_FEE_KEY: &str = "pool_fee";
/// `fee_bps` — el MISMO dato que `pool_fee`, en la unidad que piden op_15/op_21/op_32.
pub const FEE_BPS_KEY: &str = "fee_bps";
/// `flash_premium` — fracción del principal (φ en `repayment = 1 + φ`).
pub const FLASH_PREMIUM_KEY: &str = "flash_premium";
/// `max_capital` — cupo del solver, en unidades mínimas del token0.
pub const MAX_CAPITAL_KEY: &str = "max_capital";
/// `break_even_target` — objetivo del solver, en unidades mínimas del token0.
pub const BREAK_EVEN_TARGET_KEY: &str = "break_even_target";

/// Las cinco claves de F7, para censo y tests.
pub const COST_KEYS: &[&str] = &[
    POOL_FEE_KEY,
    FEE_BPS_KEY,
    FLASH_PREMIUM_KEY,
    MAX_CAPITAL_KEY,
    BREAK_EVEN_TARGET_KEY,
];

// ─────────────────────────────────────────────────────────────────────────────
// 1) Fee del pool — la MISMA lectura que ya consume el fee embebido
// ─────────────────────────────────────────────────────────────────────────────

/// El par `(fee_units, fee_denominator)` leído del despliegue.
///
/// Es la lectura que el grafo ya tiene (`agent_graph.rs:23-24`) y que
/// `snapshot_services.rs:345` consume para el fee embebido. Aquí se conserva el
/// par tal cual — **sin normalizar a una tarifa universal**: `30/10_000` (V2,
/// 30 bps) y `3000/1_000_000` (V3 fee tier 3000, 30 bps) son el MISMO fee por
/// caminos distintos, y convertir ambos a `0.003` antes de la lectura perdería
/// la identidad del tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolFeeRead {
    /// Numerador del fee (p. ej. `30` en un par V2, `3000` en un tier V3).
    pub fee_units: u32,
    /// Denominador del fee (`10_000` en V2, `1_000_000` en V3). Cero = ilegible.
    pub denominator: u32,
}

impl PoolFeeRead {
    pub fn new(fee_units: u32, denominator: u32) -> Self {
        Self {
            fee_units,
            denominator,
        }
    }

    /// Fracción que el pool retiene, en `[0, 1)`.
    ///
    /// `None` para un denominador nulo, un numerador mayor que el denominador
    /// (una «tarifa» por encima del 100 % no es una tarifa) o un valor no
    /// finito. Un fee **exactamente cero** es una lectura válida y se conserva
    /// como `Some(0.0)`: un pool sin comisión es un hecho, no una ausencia.
    pub fn fraction(&self) -> Option<f64> {
        if self.denominator == 0 || self.fee_units > self.denominator {
            return None;
        }
        let fraction = f64::from(self.fee_units) / f64::from(self.denominator);
        if fraction.is_finite() && (0.0..1.0).contains(&fraction) {
            Some(fraction)
        } else {
            None
        }
    }

    /// El mismo fee en basis points (×10⁴), la unidad de `fee_bps`.
    ///
    /// `fee_units/denominator × 10_000` — calculado desde el par, nunca desde
    /// una tabla de forks.
    pub fn bps(&self) -> Option<f64> {
        let fraction = self.fraction()?;
        let bps = fraction * f64::from(BPS_DENOMINATOR as u32);
        if bps.is_finite() && (0.0..f64::from(BPS_DENOMINATOR as u32)).contains(&bps) {
            Some(bps)
        } else {
            None
        }
    }
}

/// `(pool_fee, fee_bps)` **atómicos**: se emiten juntos o no se emite ninguno.
///
/// Atómico por la misma razón que el par oráculo/on-chain de `oracle_bias`: el
/// mismo hecho (el fee del pool) alimentando dos claves con dos unidades
/// distintas no puede quedar a medias sin que un operador lea una unidad y otro
/// lea nada. Y el par comparte la guarda del lector: si `fee_units` o
/// `fee_denominator` faltan, **no hay fee** — es el `missing_fee_units` que el
/// repo ya declara honesto (`snapshot_services.rs:345`).
pub fn pool_fee_pair(read: Option<&PoolFeeRead>) -> Option<(f64, f64)> {
    let read = read?;
    let fraction = read.fraction()?;
    let bps = read.bps()?;
    Some((fraction, bps))
}

// ─────────────────────────────────────────────────────────────────────────────
// 2) Premium de financiación flash — fuente autoritativa externa
// ─────────────────────────────────────────────────────────────────────────────

/// Lectura ON-CHAIN del premium del proveedor de financiación.
///
/// Se construye SÓLO desde una lectura real. Los campos de procedencia no son
/// decorativos: `cost_producers::funding::resolve` rechaza una lectura sin
/// `source`/`evidence_id` con `provider_reads_without_provenance`
/// (`funding.rs:222-227`), y aquí se aplica la misma guarda — sin procedencia no
/// hay premium, porque un bps sin origen es indistinguible de un literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PremiumRead<'a> {
    /// `Pool.FLASHLOAN_PREMIUM_TOTAL()` en basis points (denominador 10_000).
    pub premium_bps: u32,
    /// Proveedor (`aave_v3`, `balancer_v2`, `erc3156`, …).
    pub provider: &'a str,
    /// Contrato del pool/vault leído.
    pub pool: &'a str,
    /// Cadena de la lectura.
    pub chain_id: u64,
    /// Bloque/hash al que está anclada (§13: un timestamp no demuestra estado).
    pub block_ref: Option<&'a str>,
    /// Descripción de la fuente autoritativa.
    pub source: &'a str,
    /// Identificador de evidencia de la lectura.
    pub evidence_id: &'a str,
}

impl PremiumRead<'_> {
    /// Procedencia completa: proveedor, pool, fuente y evidencia no vacíos.
    pub fn has_provenance(&self) -> bool {
        !self.provider.trim().is_empty()
            && !self.pool.trim().is_empty()
            && !self.source.trim().is_empty()
            && !self.evidence_id.trim().is_empty()
    }

    /// ¿El venue ACREDITA un premium exactamente cero?
    ///
    /// Espejo de `funding::ProviderReads::zero_attested` (`funding.rs:154`): un
    /// cero sólo es admisible con la lectura presente (= 0) y la procedencia
    /// citada. Un cero sin acreditación sería «financiación gratuita».
    pub fn zero_attested(&self) -> bool {
        self.premium_bps == 0 && self.has_provenance()
    }

    /// Fracción del principal (φ), con la semántica exacta de op_26:67.
    ///
    /// `None` — es decir clave AUSENTE — cuando:
    /// * falta la procedencia (bps sin origen = literal encubierto);
    /// * los bps exceden el 100 % (`> 10_000`): una lectura imposible no se
    ///   reinterpreta, se rechaza (`flashloan_invalid_aave_premium`,
    ///   `flashloan_math.rs:17-20`);
    /// * los bps son cero **sin** acreditación.
    ///
    /// `Some(0.0)` sólo con `zero_attested()`. Ése es el único camino por el que
    /// un `0.0` puede llegar al mapa, y el test
    /// `flash_premium_is_never_zero_without_an_attested_read` lo barre entero.
    pub fn fraction(&self) -> Option<f64> {
        if !self.has_provenance() {
            return None;
        }
        if self.premium_bps > BPS_DENOMINATOR as u32 {
            return None;
        }
        if self.premium_bps == 0 && !self.zero_attested() {
            return None;
        }
        let fraction = f64::from(self.premium_bps) / f64::from(BPS_DENOMINATOR as u32);
        if fraction.is_finite() {
            Some(fraction)
        } else {
            None
        }
    }
}

/// Estado de una clave que no se puede computar por una dependencia EXTERNA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementStatus {
    /// La fuente es autoritativa y existe, pero vive fuera de este proceso:
    /// falta la lectura (RPC/ancla/proveedor) o el cableado.
    BlockedExternal,
    /// La unidad del slot no está declarada por ningún artefacto autoritativo:
    /// producir un número exigiría adivinar la escala.
    UnitUndeclared,
}

impl RequirementStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BlockedExternal => "BLOCKED_EXTERNAL",
            Self::UnitUndeclared => "UNIT_UNDECLARED",
        }
    }
}

/// Requisito EXACTO para desbloquear una clave. Es un dato, no un log: quien
/// reciba esto sabe qué llamar, contra qué, y qué falta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRequirement {
    pub key: &'static str,
    pub status: RequirementStatus,
    /// Contrato/función exactos a invocar.
    pub call: String,
    /// Red/dominio de la lectura.
    pub network: String,
    /// Qué falta HOY para poder emitir la clave.
    pub missing: String,
    /// Sitios del repo que YA hacen la lectura (evidencia reutilizable).
    pub existing_readers: Vec<&'static str>,
}

/// Requisito externo exacto de `flash_premium`.
///
/// Devuelve el contrato, el método, el denominador, la red y lo que falta, con
/// los dos lectores que ya existen en el repo. Es lo que se entrega cuando el
/// premium NO se pudo resolver — nunca un `0.0` ni un `not_applicable` sin
/// fundamento (prohibidos por el prompt §5).
pub fn flash_premium_requirement(chain_id: u64) -> ExternalRequirement {
    ExternalRequirement {
        key: FLASH_PREMIUM_KEY,
        status: RequirementStatus::BlockedExternal,
        call: format!(
            "AaveV3 Pool.FLASHLOAN_PREMIUM_TOTAL() -> uint128 BPS (denominador {BPS_DENOMINATOR}); \
             metodo/callback segun el proveedor: flashLoan/flashLoanSimple => \
             executeOperation(...premiums...), ERC-3156 => flashFee(token,amount)"
        ),
        network: format!(
            "evm:{chain_id} — la lectura debe ir anclada a un bloque (eth_call con blockHash, \
             EIP-1898) o no es reproducible"
        ),
        missing: "un Provider<Http> para esa cadena en el proceso del searcher + la direccion del \
                  pool (via FLE.aavePool() o PoolAddressesProvider) + el sitio de cableado, que hoy \
                  es zona caliente: cartridge_boot.rs:2107 declara la lectura pendiente"
            .to_string(),
        existing_readers: vec![
            "backend/shared-rs/src/flashloan_math.rs:165",
            "backend/relays-client/src/plan_validation.rs:336",
        ],
    }
}

/// Fracción del premium desde la lectura autoritativa, u omisión honesta.
///
/// `None` ⇒ la clave NO se inserta (R8: `None = no computado`).
pub fn flash_premium_fraction(read: Option<&PremiumRead<'_>>) -> Option<f64> {
    read.and_then(PremiumRead::fraction)
}

// ─────────────────────────────────────────────────────────────────────────────
// 3) Cupo y objetivo del solver — magnitud configurada × escala del numerario
// ─────────────────────────────────────────────────────────────────────────────

/// Escala del token NUMERARIO de la ruta (token0): precio y decimales REALES.
///
/// Los dos campos son necesarios y ninguno es opcional: el precio convierte USD
/// → cantidad de token, y los decimales convierten cantidad humana → unidades
/// mínimas. Sin uno de los dos, la conversión no existe y la clave se omite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenScale {
    /// Precio USD del token (precio unitario, no un importe).
    pub usd_price: f64,
    /// Decimales ERC-20 del token, leídos del contrato (`decimals()`).
    pub decimals: u8,
}

impl TokenScale {
    pub fn new(usd_price: f64, decimals: u8) -> Self {
        Self {
            usd_price,
            decimals,
        }
    }

    /// ¿La escala es utilizable? Precio finito y positivo, decimales en rango ERC-20.
    pub fn is_usable(&self) -> bool {
        self.usd_price.is_finite() && self.usd_price > 0.0 && self.decimals <= 77
    }
}

/// Convierte una magnitud USD al cupo del solver, en **unidades mínimas del
/// numerario** (la familia que op_19 y op_21 declaran).
///
/// `min_units = usd / precio_usd × 10^decimales`
///
/// Es una CONVERSIÓN de un valor real (el cupo configurado por el operador) con
/// una escala real (precio y decimales del numerario), no una estimación: no hay
/// constante, ni media, ni proxy. Si falta el valor o la escala, `None` — la
/// clave se omite.
///
/// Rechaza un resultado no finito, no positivo o fuera del rango de `u128`
/// (un cupo que no cabe en el espacio de un saldo on-chain es una lectura
/// incoherente, no un cupo grande).
pub fn numeraire_min_units(usd: Option<f64>, scale: Option<&TokenScale>) -> Option<f64> {
    let usd = usd?;
    let scale = scale?;
    if !usd.is_finite() || usd <= 0.0 || !scale.is_usable() {
        return None;
    }
    let units = usd / scale.usd_price * 10f64.powi(i32::from(scale.decimals));
    if units.is_finite() && units > 0.0 && units <= u128::MAX as f64 {
        Some(units)
    } else {
        None
    }
}

/// Todo el contexto de coste/financiación/capital de UN candidato.
///
/// Cada campo es `Option` porque cada uno es una lectura que puede faltar, y
/// cada `None` es un hueco DECLARADO (la clave no se emite), nunca un valor por
/// defecto. `Default` = «no hay ninguna lectura», que produce exactamente cero
/// claves nuevas.
#[derive(Debug, Clone, Copy, Default)]
pub struct CostInputs<'a> {
    /// Fee REAL del pool de la ruta (`agent_graph::Edge::{fee_units,fee_denominator}`).
    pub pool_fee: Option<PoolFeeRead>,
    /// Premium REAL del proveedor de financiación (lectura on-chain).
    pub flash_premium: Option<PremiumRead<'a>>,
    /// Capital/cupo configurado por el operador (`trading_config.capital_usd`), USD.
    pub capital_usd: Option<f64>,
    /// Escala del numerario para expresar el cupo en unidades mínimas.
    pub numeraire: Option<TokenScale>,
    /// Objetivo de beneficio neto configurado (`trading_config.min_profit_usd`), USD.
    pub min_profit_usd: Option<f64>,
}

impl CostInputs<'_> {
    /// El cupo del solver en unidades mínimas del numerario, si existe.
    pub fn max_capital_min_units(&self) -> Option<f64> {
        numeraire_min_units(self.capital_usd, self.numeraire.as_ref())
    }

    /// El objetivo del solver en unidades mínimas del numerario, si existe.
    ///
    /// El slot declara `[token0 numerary]` en `op_21_newton.rs:123`, la misma
    /// familia que `max_capital` (op_19:184/188). AVISO MEDIDO, declarado y no
    /// silenciado: el encabezado de op_21 usa `f'(x) = r1·γ·r0/(r0+γ·x)² − 1`
    /// (`op_21_newton.rs:7`) mientras el código aplica el factor `price`
    /// (`op_21_newton.rs:137`), y el término `gas` se construye como
    /// `gwei·units·1e-9·price` (op_21:115) — dimensionalmente coherente sólo si
    /// el numerario ES el token de gas. Esa incoherencia es del consumidor
    /// (math-engine) y queda como seguimiento; lo que este productor garantiza
    /// es que el VALOR sea el objetivo configurado, en la familia que el
    /// consumidor declara, o nada.
    pub fn break_even_target_min_units(&self) -> Option<f64> {
        numeraire_min_units(self.min_profit_usd, self.numeraire.as_ref())
    }

    /// Requisito externo de las claves que no se pudieron resolver, si alguna.
    ///
    /// No es un log: es el contrato de resolución que viaja al operador.
    pub fn external_requirements(&self, chain_id: u64) -> Vec<ExternalRequirement> {
        let mut out = Vec::new();
        if self.flash_premium.is_none() {
            out.push(flash_premium_requirement(chain_id));
        }
        out
    }
}
