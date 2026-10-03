//! §5 — RESOLUCIÓN COMÚN DE COSTOS con productores específicos y trazables.
//!
//! Prompt `PROMPT_ARBITRAGEX_BUSQUEDA_AUTONOMA_FEES_264.md` §5 exige una
//! resolución común de costos con **productores específicos**, no una tarifa
//! universal por estrategia: la misma estrategia tiene costos distintos según
//! ruta, pools, tamaño, contrato, red, financiación y momento. §4 fija el
//! contrato numérico (enteros de unidades mínimas, precisión decimal explícita,
//! nada de `f64` para dinero) y el contrato `CostLine` del bridge Rhai.
//!
//! Este módulo es la capa de productores. NO sustituye a `CostLine` (el
//! contrato del bridge, que sigue siendo el vocabulario que el validador
//! entiende): lo **alimenta** agregando por categoría, sin colisiones entre
//! componentes ni duplicados por categoría.
//!
//! ## Frontera con el contrato existente
//!
//! `crate::rhai_agent_bridge::CostLine` es un contrato CERRADO del validador
//! (`economic_check`):
//!
//! * `kind` debe ser único por plan (`duplicate_cost_kind`);
//! * `treatment ∈ {external, embedded, not_applicable}`;
//! * `external`/`embedded` exigen `usd` decimal no negativo parseable;
//! * `not_applicable` exige `usd = None` **y** `reason` no vacío;
//! * `evidence_id` no vacío siempre;
//! * `atomic_quote` exige los kinds `gas`, `financing`, `execution_fees`.
//!
//! Por eso aquí un **componente** lleva un `kind` granular (identidad
//! económica exacta: `v2_lp_fee`, `v3_protocol_fee`, `aave_flash_premium`, …) y
//! las líneas se **agregan por categoría** de bridge (ver [`bridge_kind`]). Un
//! componente granular nunca colisiona; una categoría nunca se duplica.
//!
//! Este módulo NO edita `rhai_agent_bridge.rs` ni `snapshot_services.rs`
//! (zona caliente): sólo consume `CostLine` de forma aditiva.
//!
//! ## Los tres estados que no se confunden (§4)
//!
//! `Some(0)`, pérdida y ausencia son estados DISTINTOS:
//!
//! | estado | significado | efecto en la línea de bridge |
//! |---|---|---|
//! | [`CostState::Resolved`] | computado desde lecturas autoritativas | `external`/`embedded` con `usd` |
//! | [`CostState::ZeroAttested`] | la fuente autoritativa dice **exactamente cero** | `external`/`embedded` con `usd = 0` |
//! | [`CostState::Absent`] | aplicable pero **no computado** (falta el input) | `usd = None` → el bridge reporta `missing_or_invalid_cost` (honesto) |
//! | [`CostState::PendingResolution`] | la tarifa EXISTE y su valor se desconoce | idem + [`ResolutionTask`] con la llamada exacta |
//! | [`CostState::NotApplicable`] | probado inaplicable **con fundamento** | `not_applicable` + `reason` |
//!
//! Un `not_applicable` sin fundamento es un error de construcción
//! ([`CostError::NotApplicableNeedsGrounding`]) — no puede encubrir una tarifa
//! pendiente. Un fee desconocido SIEMPRE produce tarea de resolución.
//!
//! ## Lo que NO hace
//!
//! No inventa tarifas: ninguna función de este módulo devuelve una tarifa
//! cuando la lectura autoritativa falta. Las tablas de "valores documentados"
//! por fork existen sólo para **contrastar** la lectura real
//! ([`univ2::V2Fork::documented_terms`]) y para producir una tarea de
//! discrepancia; jamás se usan como valor productivo. Igual para Aave: el
//! premium sale de `FLASHLOAN_PREMIUM_TOTAL()` leído del pool, nunca de un
//! literal de 5 o 9 bps.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode};
use ethers::types::U256;
use serde::{Deserialize, Serialize};

use crate::rhai_agent_bridge::CostLine;

pub mod builder;
pub mod curve;
pub mod funding;
pub mod gas;
pub mod univ2;
pub mod univ3;
pub mod univ4;

#[cfg(test)]
mod tests;

// ─────────────────────────────────────────────────────────────────────────────
// Categorías de bridge (vocabulario del validador) y kinds granulares
// ─────────────────────────────────────────────────────────────────────────────

/// Categoría `gas` del bridge: coste de la transacción, `external`, se resta
/// UNA vez. Incluye base + priority + datos L1 + componente L2 + blobs, sin
/// doble conteo (ver [`gas`]).
pub const KIND_GAS: &str = "gas";
/// Categoría `financing` del bridge: principal/premium del proveedor de
/// capital. Con `profit_basis = retained_after_repayment` el bridge PROHÍBE
/// que sea `external` (`already_in_retained_spread`).
pub const KIND_FINANCING: &str = "financing";
/// Categoría `execution_fees` del bridge: comisiones de swap/curva YA
/// embebidas en las cotizaciones. El bridge PROHÍBE `external` cuando
/// `economic_kind = atomic_quote` (`quote_already_includes_swap_fees`).
pub const KIND_EXECUTION_FEES: &str = "execution_fees";
/// Categoría propia: pago al builder/relay, SEPARADO del priority fee.
pub const KIND_BUILDER_PAYMENT: &str = "builder_payment";
/// Categoría propia: coste de bridge/mensajería cross-domain.
pub const KIND_BRIDGE_COST: &str = "bridge_cost";
/// Categoría propia: comisiones de venue CEX/derivados.
pub const KIND_CEX_FEE: &str = "cex_trading_fee";
/// Categoría propia: comisiones de mint/redeem/unwrap/vault.
pub const KIND_MINT_REDEEM: &str = "mint_redeem_fee";

/// Mapea un `kind` GRANULAR de componente a la categoría de bridge.
///
/// Devuelve `None` para un kind granular desconocido: el llamador debe
/// reportarlo (un coste sin categoría contable no entra al contrato).
pub fn bridge_kind(granular: &str) -> Option<&'static str> {
    Some(match granular {
        // ── gas: la transacción, una sola vez. Los desgloses L1/L2 y de blob
        //    pertenecen a la MISMA categoría `gas`: emitirlos como categorías
        //    propias los haría sumables aparte y el bridge los restaría dos
        //    veces (el audit `blob_fee_inside_l1_data_fee` vigila el caso).
        gas::KIND_GAS_EXECUTION => KIND_GAS,
        gas::KIND_L2_DATA_FEE => KIND_GAS,
        gas::KIND_BLOB_FEE => KIND_GAS,

        // ── financiación ─────────────────────────────────────────────────
        funding::KIND_FINANCING_PREMIUM => KIND_FINANCING,
        univ2::KIND_V2_FLASH_PREMIUM => KIND_FINANCING,

        // ── comisiones embebidas en el quote ─────────────────────────────
        univ2::KIND_V2_LP_FEE => KIND_EXECUTION_FEES,
        univ3::KIND_V3_LP_FEE => KIND_EXECUTION_FEES,
        univ3::KIND_V3_PROTOCOL_FEE => KIND_EXECUTION_FEES,
        univ4::KIND_V4_LP_FEE => KIND_EXECUTION_FEES,
        univ4::KIND_V4_PROTOCOL_FEE => KIND_EXECUTION_FEES,
        univ4::KIND_V4_HOOK_FEE => KIND_EXECUTION_FEES,
        curve::KIND_CURVE_LP_FEE => KIND_EXECUTION_FEES,
        curve::KIND_CURVE_ADMIN_FEE => KIND_EXECUTION_FEES,

        // ── categorías propias ───────────────────────────────────────────
        builder::KIND_BUILDER_BID => KIND_BUILDER_PAYMENT,

        // ── rebates: flujos con signo, categoría propia ───────────────────
        builder::KIND_BUILDER_REBATE => KIND_BUILDER_PAYMENT,
        univ4::KIND_V4_HOOK_REBATE => KIND_EXECUTION_FEES,
        _ => return None,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Identidad, ámbito, ancla y activo
// ─────────────────────────────────────────────────────────────────────────────

/// Ámbito contable de un componente (§4: «pierna o ámbito»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    /// Índice de pierna dentro del ledger del plan (`None` = ámbito de ruta).
    pub leg: Option<usize>,
    /// Identidad del venue: par V2, pool V3/V4, vault, relay, exchange.
    pub venue: String,
    /// Dominio: `evm:1`, `cex:binance`, `bridge:across`.
    pub domain: String,
}

impl Scope {
    pub fn route(domain: &str) -> Self {
        Self {
            leg: None,
            venue: "route".into(),
            domain: domain.into(),
        }
    }
    pub fn leg(index: usize, venue: &str, domain: &str) -> Self {
        Self {
            leg: Some(index),
            venue: venue.into(),
            domain: domain.into(),
        }
    }
    /// Clave estable para agrupar/telemetrizar sin perder la pierna.
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}",
            self.domain,
            self.venue,
            self.leg.map(|l| l.to_string()).unwrap_or_else(|| "-".into())
        )
    }
}

/// Activo de un componente económico: `native` para el token de gas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRef {
    pub chain_id: u64,
    pub token: String,
    pub decimals: u8,
}

impl AssetRef {
    pub fn new(chain_id: u64, token: &str, decimals: u8) -> Self {
        Self {
            chain_id,
            token: token.to_owned(),
            decimals,
        }
    }
    /// Token nativo de la cadena (el que paga el gas).
    pub fn native(chain_id: u64, decimals: u8) -> Self {
        Self::new(chain_id, "native", decimals)
    }
    pub fn is_native(&self) -> bool {
        self.token == "native"
    }
}

/// §13 — ancla de la lectura. Un timestamp de sincronización NO demuestra
/// mismo bloque: para EVM hace falta bloque o hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueAnchor {
    pub chain_id: u64,
    pub block_number: Option<u64>,
    pub block_hash: Option<String>,
    /// Momento local de observación (no es prueba de estado on-chain).
    pub observed_at_ms: u64,
    /// Tiempo del venue cuando el venue ES el reloj (CEX, subasta, oráculo).
    pub venue_time_ms: Option<u64>,
}

impl VenueAnchor {
    pub fn evm(chain_id: u64, block_number: u64, block_hash: Option<&str>) -> Self {
        Self {
            chain_id,
            block_number: Some(block_number),
            block_hash: block_hash.map(str::to_owned),
            observed_at_ms: 0,
            venue_time_ms: None,
        }
    }
    pub fn offchain(domain_chain_id: u64, venue_time_ms: u64) -> Self {
        Self {
            chain_id: domain_chain_id,
            block_number: None,
            block_hash: None,
            observed_at_ms: 0,
            venue_time_ms: Some(venue_time_ms),
        }
    }
    pub fn with_observed_at_ms(mut self, ms: u64) -> Self {
        self.observed_at_ms = ms;
        self
    }
    /// Hay ancla verificable (bloque, hash o tiempo del venue).
    pub fn is_anchored(&self) -> bool {
        self.block_number.is_some() || self.block_hash.is_some() || self.venue_time_ms.is_some()
    }
    pub fn describe(&self) -> String {
        match (self.block_number, &self.block_hash) {
            (Some(b), Some(h)) => format!("chain:{}:block:{b}:{}", self.chain_id, &h[..h.len().min(18)]),
            (Some(b), None) => format!("chain:{}:block:{b}", self.chain_id),
            (None, _) => match self.venue_time_ms {
                Some(t) => format!("chain:{}:venue_time_ms:{t}", self.chain_id),
                None => format!("chain:{}:unanchored", self.chain_id),
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tratamiento y estado
// ─────────────────────────────────────────────────────────────────────────────

/// Tratamiento contable. Vocabulario CERRADO del bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Treatment {
    /// Se descuenta UNA vez del neto.
    External,
    /// Ya está reflejado en el quote / delta de balances: se declara, no se resta.
    Embedded,
    /// Sin importe y con fundamento explícito.
    NotApplicable,
}

impl Treatment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::External => "external",
            Self::Embedded => "embedded",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// Estado de resolución. Cinco valores porque «cero acreditado», «ausente» y
/// «pendiente de resolver» son tres cosas distintas que no se pueden colapsar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostState {
    /// Computado desde lecturas autoritativas (el valor PUEDE ser 0).
    Resolved,
    /// La fuente autoritativa dice exactamente cero, con evidencia citada.
    ZeroAttested,
    /// Aplicable pero NO computado: falta el input. No es cero.
    Absent,
    /// La tarifa existe y su valor se desconoce: exige tarea de resolución.
    PendingResolution,
    /// Probado inaplicable para ESTE plan, con fundamento citado.
    NotApplicable,
}

impl CostState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::ZeroAttested => "zero_attested",
            Self::Absent => "absent",
            Self::PendingResolution => "pending_resolution",
            Self::NotApplicable => "not_applicable",
        }
    }
    /// ¿El componente tiene un importe computado (incluido el cero)?
    pub fn is_computed(&self) -> bool {
        matches!(self, Self::Resolved | Self::ZeroAttested)
    }
    /// ¿Bloquea el neto honesto?
    pub fn blocks_net(&self) -> bool {
        matches!(self, Self::Absent | Self::PendingResolution)
    }
}

/// Unidad del entero `amount_raw`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    /// Unidades mínimas del token (wei, 1e-6, 1e-8 según decimals).
    MinUnits,
    /// Pips: millonésimas (denominador 1e6) — fee tiers V3/V4.
    Pips,
    /// Basis points: diezmilésimas (denominador 1e4).
    Bps,
}

impl Unit {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MinUnits => "min_units",
            Self::Pips => "pips_1e6",
            Self::Bps => "bps_1e4",
        }
    }
}

/// Dirección del flujo. Un rebate es un flujo con SIGNO, no un coste negativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// El pagador entrega el importe.
    Charge,
    /// El pagador RECIBE el importe (rebate/refund).
    Rebate,
}

// ─────────────────────────────────────────────────────────────────────────────
// Precio y valoración decimal
// ─────────────────────────────────────────────────────────────────────────────

/// Referencia de precio con procedencia explícita (§3: «precios consultados
/// sin validar procedencia, revisión y vigencia»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceRef {
    pub chain_id: u64,
    pub token: String,
    pub decimals: u8,
    /// Decimal exacto, jamás `f64`.
    pub usd: BigDecimal,
    pub revision: String,
    pub evidence_id: String,
    /// Productor real (`PriceBus`, `Chainlink`, …). Vacío = procedencia inválida.
    pub producer: String,
    pub observed_at_ms: u64,
    pub valid_until_ms: Option<u64>,
}

impl PriceRef {
    /// Valida procedencia, revisión, vigencia y coherencia de activo.
    pub fn check(
        &self,
        now_ms: u64,
        expected_revision: &str,
        expected: &AssetRef,
    ) -> Result<(), CostError> {
        if self.producer.trim().is_empty() {
            return Err(CostError::PriceProvenance {
                token: self.token.clone(),
                why: "missing_producer".into(),
            });
        }
        if self.evidence_id.trim().is_empty() {
            return Err(CostError::PriceProvenance {
                token: self.token.clone(),
                why: "missing_evidence_id".into(),
            });
        }
        if self.usd <= BigDecimal::from(0) {
            return Err(CostError::InvalidRead {
                read: format!("price:{}", self.token),
                why: "non_positive_price".into(),
            });
        }
        if self.chain_id != expected.chain_id || self.token != expected.token {
            return Err(CostError::PriceProvenance {
                token: self.token.clone(),
                why: format!(
                    "identity_mismatch:price_chain_{}_token_{}_vs_expected_chain_{}_token_{}",
                    self.chain_id, self.token, expected.chain_id, expected.token
                ),
            });
        }
        if self.decimals != expected.decimals {
            return Err(CostError::PriceProvenance {
                token: self.token.clone(),
                why: format!(
                    "decimals_mismatch:price_{}_vs_asset_{}",
                    self.decimals, expected.decimals
                ),
            });
        }
        if !expected_revision.is_empty() && self.revision != expected_revision {
            return Err(CostError::PriceRevisionMismatch {
                token: self.token.clone(),
                expected: expected_revision.to_owned(),
                actual: self.revision.clone(),
            });
        }
        if let Some(until) = self.valid_until_ms {
            if now_ms > until {
                return Err(CostError::StalePrice {
                    token: self.token.clone(),
                    observed_at_ms: self.observed_at_ms,
                    valid_until_ms: until,
                    now_ms,
                });
            }
        }
        Ok(())
    }

    /// Valor exacto de `amount` unidades mínimas de este token, en USD.
    ///
    /// `amount / 10^decimals * price`, todo en decimal: sin `f64`.
    pub fn value_min_units(&self, amount: &U256) -> Result<BigDecimal, CostError> {
        let scale = BigDecimal::from(10u64).powi(self.decimals as i64);
        let raw = BigDecimal::from_str(&amount.to_string())
            .map_err(|_| CostError::InvalidRead {
                read: format!("amount:{}", amount),
                why: "not_a_decimal".into(),
            })?;
        Ok(raw * &self.usd / scale)
    }

    /// Igual que [`Self::value_min_units`] desde un decimal ya escalado.
    pub fn value_token_amount(&self, token_amount: &BigDecimal) -> BigDecimal {
        token_amount * &self.usd
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Componente económico (§4)
// ─────────────────────────────────────────────────────────────────────────────

/// Un componente económico con TODOS los campos que §4 exige identificar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostComponent {
    /// Kind GRANULAR (identidad económica exacta, único dentro de su ámbito).
    pub kind: String,
    pub scope: Scope,
    pub asset: AssetRef,
    /// Entero de unidades mínimas con overflow controlado. Nunca `f64`.
    pub amount_raw: U256,
    pub unit: Unit,
    pub direction: Direction,
    /// Valoración decimal explícita. `None` = no valorado (≠ cero).
    pub usd: Option<BigDecimal>,
    /// Quién paga. Vacío = no declarado → error de construcción.
    pub payer: String,
    /// Quién lo recibe cuando corresponde (LP, protocolo, builder, tesorería).
    pub beneficiary: Option<String>,
    /// Fuente autoritativa: contrato + función + resultado.
    pub source: String,
    /// Versión del adapter que produjo la lectura.
    pub adapter_version: String,
    pub anchor: Option<VenueAnchor>,
    pub evidence_id: String,
    pub state: CostState,
    pub treatment: Treatment,
    /// §4: ¿ya está incluido en el quote / delta de balances?
    pub embedded_in_quote: bool,
    /// Fundamento obligatorio para `NotApplicable`; contexto para el resto.
    pub note: Option<String>,
}

impl CostComponent {
    /// Valida que el componente sea contable (§4) antes de entrar al agregado.
    pub fn validate(&self) -> Result<(), CostError> {
        for (field, value) in [
            ("kind", self.kind.as_str()),
            ("payer", self.payer.as_str()),
            ("source", self.source.as_str()),
            ("adapter_version", self.adapter_version.as_str()),
            ("evidence_id", self.evidence_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(CostError::IncompleteComponent {
                    kind: self.kind.clone(),
                    field: field.into(),
                });
            }
        }
        if self.state == CostState::NotApplicable
            && self.note.as_deref().is_none_or(|n| n.trim().is_empty())
        {
            return Err(CostError::NotApplicableNeedsGrounding {
                kind: self.kind.clone(),
            });
        }
        if self.treatment == Treatment::NotApplicable && self.usd.is_some() {
            return Err(CostError::InvalidRead {
                read: format!("component:{}", self.kind),
                why: "not_applicable_with_amount".into(),
            });
        }
        if self.treatment == Treatment::External && self.embedded_in_quote {
            return Err(CostError::InvalidRead {
                read: format!("component:{}", self.kind),
                why: "external_but_declared_embedded".into(),
            });
        }
        if self.state.is_computed() && self.usd.is_none() {
            return Err(CostError::IncompleteComponent {
                kind: self.kind.clone(),
                field: "usd (computed state requires a decimal valuation)".into(),
            });
        }
        if self.state.blocks_net() && self.usd.is_some() {
            return Err(CostError::InvalidRead {
                read: format!("component:{}", self.kind),
                why: "blocking_state_must_not_carry_an_amount".into(),
            });
        }
        if bridge_kind(&self.kind).is_none() {
            return Err(CostError::UnmappedKind {
                kind: self.kind.clone(),
            });
        }
        Ok(())
    }

    /// Importe USD con SIGNO (rebate negativo). `None` si no está valorado.
    pub fn signed_usd(&self) -> Option<BigDecimal> {
        self.usd.as_ref().map(|v| match self.direction {
            Direction::Charge => v.clone(),
            Direction::Rebate => -v.clone(),
        })
    }

    /// ¿Este componente pertenece al grupo `kind` y es restable?
    pub fn is_external(&self) -> bool {
        self.treatment == Treatment::External && self.state.is_computed()
    }
    pub fn is_embedded(&self) -> bool {
        self.treatment == Treatment::Embedded && self.state.is_computed()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Errores → tareas de resolución
// ─────────────────────────────────────────────────────────────────────────────

/// Tarea CONCRETA de resolución externa. §5: «Un fee desconocido debe generar
/// una tarea concreta de resolución».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionTask {
    pub cost_kind: String,
    pub scope: Scope,
    /// Receipt de §9 que la desbloquea, cuando existe uno canónico.
    pub receipt: Option<String>,
    pub provider: String,
    /// Llamada exacta a ejecutar (contrato.función).
    pub read: String,
    pub why: String,
}

/// Error de resolución. Todos los brazos nombran el dato exacto que falta —
/// ninguno se degrada silenciosamente a cero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostError {
    /// Falta una lectura autoritativa concreta.
    MissingRead { provider: String, read: String },
    /// Falta el precio del activo.
    MissingPrice { token: String },
    MissingAnchor { read: String },
    StalePrice {
        token: String,
        observed_at_ms: u64,
        valid_until_ms: u64,
        now_ms: u64,
    },
    PriceRevisionMismatch {
        token: String,
        expected: String,
        actual: String,
    },
    PriceProvenance {
        token: String,
        why: String,
    },
    InvalidRead {
        read: String,
        why: String,
    },
    Overflow {
        op: String,
    },
    ZeroDenominator {
        denominator: String,
    },
    /// El importe de entrada excede la reserva/liquidez del ámbito.
    ReserveBoundary {
        scope: String,
        amount_in: String,
        reserve: String,
    },
    /// Invariante/versión no identificada: hay que resolverla antes de cotizar.
    UnsupportedInvariant {
        provider: String,
        why: String,
    },
    MixedTreatment {
        kind: String,
    },
    IncompleteComponent {
        kind: String,
        field: String,
    },
    NotApplicableNeedsGrounding {
        kind: String,
    },
    UnmappedKind {
        kind: String,
    },
    /// Dos componentes del mismo kind intentan cubrir el mismo hecho.
    DuplicateComponent {
        kind: String,
        scope: String,
    },
}

impl CostError {
    /// Razón estable, sin espacios (apta para `repairs`/telemetría).
    pub fn reason(&self) -> String {
        match self {
            Self::MissingRead { provider, read } => format!("missing_read:{provider}.{read}"),
            Self::MissingPrice { token } => format!("missing_price:{token}"),
            Self::MissingAnchor { read } => format!("missing_anchor:{read}"),
            Self::StalePrice { token, .. } => format!("stale_price:{token}"),
            Self::PriceRevisionMismatch { token, .. } => format!("price_revision_mismatch:{token}"),
            Self::PriceProvenance { token, why } => format!("price_provenance:{token}:{why}"),
            Self::InvalidRead { read, why } => format!("invalid_read:{read}:{why}"),
            Self::Overflow { op } => format!("overflow:{op}"),
            Self::ZeroDenominator { denominator } => format!("zero_denominator:{denominator}"),
            Self::ReserveBoundary { scope, .. } => format!("reserve_boundary:{scope}"),
            Self::UnsupportedInvariant { provider, .. } => {
                format!("unsupported_invariant:{provider}")
            }
            Self::MixedTreatment { kind } => format!("mixed_treatment:{kind}"),
            Self::IncompleteComponent { kind, field } => {
                format!("incomplete_component:{kind}:{field}")
            }
            Self::NotApplicableNeedsGrounding { kind } => {
                format!("not_applicable_needs_grounding:{kind}")
            }
            Self::UnmappedKind { kind } => format!("unmapped_cost_kind:{kind}"),
            Self::DuplicateComponent { kind, scope } => {
                format!("duplicate_component:{kind}:{scope}")
            }
        }
    }

    /// Estado honesto que corresponde a este error.
    ///
    /// `MissingRead` = el importe mismo se desconoce → `PendingResolution`
    /// (exige tarea). Un fallo de PRECIO deja el importe conocido pero sin
    /// valorar → `Absent` (aplicable, no computado). Ni uno ni otro son cero.
    pub fn state(&self) -> CostState {
        match self {
            Self::NotApplicableNeedsGrounding { .. } | Self::UnmappedKind { .. } => {
                CostState::NotApplicable
            }
            Self::MissingPrice { .. }
            | Self::StalePrice { .. }
            | Self::PriceRevisionMismatch { .. }
            | Self::PriceProvenance { .. } => CostState::Absent,
            _ => CostState::PendingResolution,
        }
    }

    /// Tarea de resolución derivada, con la llamada EXACTA a ejecutar.
    pub fn task(&self, cost_kind: &str, scope: &Scope) -> ResolutionTask {
        let (provider, read, receipt): (String, String, Option<String>) = match self {
            Self::MissingRead { provider, read } => (
                provider.clone(),
                read.clone(),
                None,
            ),
            Self::MissingPrice { token } => (
                "PriceBus".into(),
                format!("price({token})"),
                Some("price_coverage".into()),
            ),
            Self::MissingAnchor { read } => (
                provider_of(read).into(),
                read.clone(),
                Some("independent_timestamps".into()),
            ),
            Self::StalePrice { token, .. } => (
                "PriceBus".into(),
                format!("refresh_price({token})"),
                Some("price_freshness".into()),
            ),
            Self::PriceRevisionMismatch { token, .. } => (
                "PriceBus".into(),
                format!("revision({token})"),
                Some("price_revision_binding".into()),
            ),
            Self::PriceProvenance { token, .. } => (
                "PriceBus".into(),
                format!("provenance({token})"),
                Some("price_provenance".into()),
            ),
            Self::ReserveBoundary { scope: s, .. } => (
                provider_of(s).into(),
                format!("reserves({s})"),
                Some("reserve_boundary_respected".into()),
            ),
            Self::UnsupportedInvariant { provider, .. } => (
                provider.clone(),
                "exact_invariant_version_and_rates()".into(),
                Some("exact_invariant_version_and_rates".into()),
            ),
            Self::InvalidRead { read, .. } => {
                (provider_of(read).into(), read.clone(), None)
            }
            _ => ("unknown".into(), "unresolved".into(), None),
        };
        ResolutionTask {
            cost_kind: cost_kind.to_owned(),
            scope: scope.clone(),
            receipt,
            provider,
            read,
            why: self.reason(),
        }
    }
}

fn provider_of(read: &str) -> &str {
    read.split([':', '.', '(']).next().unwrap_or(read)
}

impl fmt::Display for CostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason())
    }
}

impl std::error::Error for CostError {}

// ─────────────────────────────────────────────────────────────────────────────
// Agregado
// ─────────────────────────────────────────────────────────────────────────────

/// Identidad mínima para emitir un componente honesto cuando el productor
/// falla: el fallo NO borra el coste del contrato, lo marca pendiente.
#[derive(Debug, Clone)]
pub struct ComponentSeed {
    pub kind: String,
    pub scope: Scope,
    pub asset: AssetRef,
    pub treatment: Treatment,
    pub payer: String,
    pub source: String,
    pub adapter_version: String,
}

/// Hallazgo de doble conteo. §5/§14: fees embebidos, protocol fee, financing,
/// priority tip y builder NO se descuentan dos veces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoubleCountFinding {
    /// Identificador estable del riesgo detectado.
    pub code: String,
    pub kinds: Vec<String>,
    pub why: String,
    /// Qué hacer: el tratamiento correcto.
    pub action: String,
}

/// Flujo con signo, para rebates que el contrato `CostLine` (no negativo) no
/// puede representar (§5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedFlow {
    pub kind: String,
    pub scope: Scope,
    pub usd: BigDecimal,
    pub evidence_id: String,
    pub note: String,
}

/// Resultado completo de la resolución de costos de UN plan.
#[derive(Debug, Clone, Default)]
pub struct CostResolution {
    pub chain_id: u64,
    pub snapshot_id: String,
    pub price_revision: String,
    /// `before_financing` | `retained_after_repayment` (basis del plan).
    pub profit_basis: String,
    /// `atomic_quote` | `non_atomic_expected` | … (kind económico del plan).
    pub economic_kind: String,
    pub components: Vec<CostComponent>,
    pub tasks: Vec<ResolutionTask>,
    /// Flujos con signo (rebates) que el contrato no negativo no admite.
    pub rebates: Vec<SignedFlow>,
    /// Errores de construcción (no de dato): no deben llegar a producción.
    pub construction_errors: Vec<CostError>,
}

impl CostResolution {
    pub fn new(
        chain_id: u64,
        snapshot_id: &str,
        price_revision: &str,
        profit_basis: &str,
        economic_kind: &str,
    ) -> Self {
        Self {
            chain_id,
            snapshot_id: snapshot_id.to_owned(),
            price_revision: price_revision.to_owned(),
            profit_basis: profit_basis.to_owned(),
            economic_kind: economic_kind.to_owned(),
            ..Default::default()
        }
    }

    /// Añade un componente validado. Un componente inválido NO entra al
    /// agregado: se registra como error de construcción y como tarea.
    pub fn push(&mut self, component: CostComponent) {
        if let Err(e) = component.validate() {
            self.tasks.push(e.task(&component.kind, &component.scope));
            self.construction_errors.push(e);
            return;
        }
        if component.direction == Direction::Rebate {
            // Un rebate es un flujo con signo propio; se conserva aparte para
            // no violar el contrato no negativo de `CostLine`.
            self.rebates.push(SignedFlow {
                kind: component.kind.clone(),
                scope: component.scope.clone(),
                usd: -(component.usd.clone().unwrap_or_else(|| BigDecimal::from(0))),
                evidence_id: component.evidence_id.clone(),
                note: component
                    .note
                    .clone()
                    .unwrap_or_else(|| "rebate_flow".into()),
            });
        }
        self.components.push(component);
    }

    /// Absorbe el resultado de un productor. Un `Err` se convierte en un
    /// componente `PendingResolution`/`Absent` + su tarea — nunca en cero.
    pub fn absorb(&mut self, seed: ComponentSeed, result: Result<CostComponent, CostError>) {
        match result {
            Ok(c) => self.push(c),
            Err(e) => {
                self.tasks.push(e.task(&seed.kind, &seed.scope));
                let state = e.state();
                let component = CostComponent {
                    kind: seed.kind.clone(),
                    scope: seed.scope.clone(),
                    asset: seed.asset.clone(),
                    amount_raw: U256::zero(),
                    unit: Unit::MinUnits,
                    direction: Direction::Charge,
                    usd: None,
                    payer: seed.payer.clone(),
                    beneficiary: None,
                    source: format!("{}:unresolved", seed.source),
                    adapter_version: seed.adapter_version.clone(),
                    anchor: None,
                    evidence_id: format!("unresolved:{}", e.reason()),
                    state,
                    treatment: if state == CostState::NotApplicable {
                        Treatment::NotApplicable
                    } else {
                        seed.treatment
                    },
                    embedded_in_quote: seed.treatment == Treatment::Embedded,
                    note: Some(e.reason()),
                };
                // El componente sintético se registra a mano: `validate` exige
                // `usd` para estados computados, y este NO está computado.
                self.components.push(component);
            }
        }
    }

    /// Componentes que bloquean un neto honesto (ausentes o pendientes).
    pub fn blocking(&self) -> Vec<&CostComponent> {
        self.components.iter().filter(|c| c.state.blocks_net()).collect()
    }

    /// Suma de costes EXTERNOS con signo (los rebates externos restan).
    /// `None` mientras haya componentes que bloquean: no se inventa un neto.
    pub fn external_usd(&self) -> Option<BigDecimal> {
        if !self.blocking().is_empty() {
            return None;
        }
        let mut total = BigDecimal::from(0);
        for c in self.components.iter().filter(|c| c.is_external()) {
            total += c.signed_usd()?;
        }
        Some(total)
    }

    /// §5: «Muestra numéricamente fees embebidos cuando sean calculables, con
    /// “incluido; no se descuenta otra vez”». Sólo devuelve `Some` si TODOS los
    /// componentes embebidos están valorados.
    pub fn embedded_fee_usd(&self) -> Option<BigDecimal> {
        let embedded: Vec<&CostComponent> =
            self.components.iter().filter(|c| c.is_embedded()).collect();
        if embedded.is_empty() {
            return None;
        }
        let mut total = BigDecimal::from(0);
        for c in embedded {
            total += c.signed_usd()?;
        }
        Some(total)
    }

    /// Desglose por kind granular, para trazabilidad y para la card.
    pub fn per_kind_usd(&self) -> BTreeMap<String, BigDecimal> {
        let mut out: BTreeMap<String, BigDecimal> = BTreeMap::new();
        for c in &self.components {
            if let Some(v) = c.signed_usd() {
                *out.entry(c.kind.clone()).or_insert_with(|| BigDecimal::from(0)) += v;
            }
        }
        out
    }

    /// Convierte la resolución al contrato `CostLine`, agregando por CATEGORÍA
    /// (una línea por categoría, sin duplicados) y preservando la granularidad
    /// en `reason`.
    pub fn to_cost_lines(&self) -> Result<Vec<CostLine>, CostError> {
        let mut by_category: BTreeMap<&'static str, Vec<&CostComponent>> = BTreeMap::new();
        for c in &self.components {
            let category = bridge_kind(&c.kind).ok_or_else(|| CostError::UnmappedKind {
                kind: c.kind.clone(),
            })?;
            by_category.entry(category).or_default().push(c);
        }

        let mut lines = Vec::with_capacity(by_category.len());
        for (category, members) in by_category {
            // §3: «Sustituye cualquier total de fees que se considere completo
            // porque una sola pierna aportó un importe. Cada suma completa exige
            // todos sus componentes aplicables resueltos.» Una categoría con
            // CUALQUIER miembro ausente/pendiente NO emite importe: emitirlo
            // convertiría un total incompleto en un número que parece medido.
            let blocking: Vec<&&CostComponent> =
                members.iter().filter(|c| c.state.blocks_net()).collect();
            if !blocking.is_empty() {
                lines.push(CostLine {
                    kind: category.to_owned(),
                    treatment: members
                        .first()
                        .map(|c| c.treatment.as_str())
                        .unwrap_or("external")
                        .to_owned(),
                    usd: None,
                    reason: Some(format!(
                        "incomplete_cost_sum: {}",
                        members
                            .iter()
                            .map(|c| format!(
                                "{}({})={}",
                                c.kind,
                                c.scope.key(),
                                c.state.as_str()
                            ))
                            .collect::<Vec<_>>()
                            .join(",")
                    )),
                    evidence_id: members
                        .iter()
                        .map(|c| c.evidence_id.as_str())
                        .collect::<Vec<_>>()
                        .join("+"),
                });
                continue;
            }
            let computed: Vec<&&CostComponent> =
                members.iter().filter(|c| c.state.is_computed()).collect();
            let not_applicable: Vec<&&CostComponent> = members
                .iter()
                .filter(|c| c.state == CostState::NotApplicable)
                .collect();

            if computed.is_empty() {
                // Sin ningún importe computado: `not_applicable` SOLO si TODOS
                // los miembros lo están y traen fundamento.
                if !not_applicable.is_empty() && not_applicable.len() == members.len() {
                    lines.push(CostLine {
                        kind: category.to_owned(),
                        treatment: Treatment::NotApplicable.as_str().into(),
                        usd: None,
                        reason: Some(
                            not_applicable
                                .iter()
                                .filter_map(|c| c.note.clone())
                                .collect::<Vec<_>>()
                                .join(" | "),
                        ),
                        evidence_id: not_applicable
                            .iter()
                            .map(|c| c.evidence_id.as_str())
                            .collect::<Vec<_>>()
                            .join("+"),
                    });
                    continue;
                }
                // Ausente/pendiente: la línea NO lleva importe. El bridge la
                // reporta como `missing_or_invalid_cost` — el estado honesto.
                lines.push(CostLine {
                    kind: category.to_owned(),
                    treatment: members
                        .first()
                        .map(|c| c.treatment.as_str())
                        .unwrap_or("external")
                        .to_owned(),
                    usd: None,
                    reason: Some(
                        members
                            .iter()
                            .map(|c| format!("{}={}", c.kind, c.state.as_str()))
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    evidence_id: members
                        .iter()
                        .map(|c| c.evidence_id.as_str())
                        .collect::<Vec<_>>()
                        .join("+"),
                });
                continue;
            }

            // Tratamientos mezclados dentro de una categoría: conflicto de
            // contrato, no se elige uno en silencio.
            let mut treatments: Vec<Treatment> = computed.iter().map(|c| c.treatment).collect();
            treatments.sort_by_key(|t| t.as_str());
            treatments.dedup();
            if treatments.len() > 1 {
                return Err(CostError::MixedTreatment {
                    kind: category.to_owned(),
                });
            }
            let treatment = treatments[0];
            let mut total = BigDecimal::from(0);
            let mut all_valued = true;
            for c in &computed {
                match c.signed_usd() {
                    Some(v) => total += v,
                    None => all_valued = false,
                }
            }
            let usd = if all_valued && total >= BigDecimal::from(0) {
                Some(plain(&total))
            } else if all_valued {
                // El neto del grupo es negativo (rebate > cargo). El contrato
                // no admite importes negativos: se declara 0 y el rebate viaja
                // como flujo con signo propio en `rebates`.
                Some("0".into())
            } else {
                None
            };
            lines.push(CostLine {
                kind: category.to_owned(),
                treatment: treatment.as_str().into(),
                usd,
                reason: Some(format!(
                    "{} | {}",
                    members
                        .iter()
                        .map(|c| format!(
                            "{}({})={}",
                            c.kind,
                            c.scope.key(),
                            c.signed_usd()
                                .map(|v| plain(&v))
                                .unwrap_or_else(|| "unvalued".into())
                        ))
                        .collect::<Vec<_>>()
                        .join(","),
                    if treatment == Treatment::Embedded {
                        "incluido; no se descuenta otra vez"
                    } else {
                        "externo; se descuenta una vez"
                    }
                )),
                evidence_id: members
                    .iter()
                    .map(|c| c.evidence_id.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
            });
        }
        Ok(lines)
    }

    /// Auditoría de doble conteo (§5/§14). Los hallazgos son DATOS, no errores:
    /// el llamador decide; el módulo no silencia ninguno.
    pub fn double_count_audit(&self) -> Vec<DoubleCountFinding> {
        let mut findings = Vec::new();
        let has = |k: &str| self.components.iter().any(|c| c.kind == k);
        let externals: Vec<&CostComponent> =
            self.components.iter().filter(|c| c.is_external()).collect();

        // 1. Builder pagado elevando el priority fee: ya está dentro de `gas`.
        if has(builder::KIND_BUILDER_BID) {
            let via_tip = self
                .components
                .iter()
                .any(|c| c.kind == builder::KIND_BUILDER_BID && c.embedded_in_quote);
            if via_tip && externals.iter().any(|c| c.kind == builder::KIND_BUILDER_BID) {
                findings.push(DoubleCountFinding {
                    code: "builder_paid_via_priority_fee_counted_twice".into(),
                    kinds: vec![KIND_GAS.into(), builder::KIND_BUILDER_BID.into()],
                    why: "el pago al builder se materializa elevando el priority fee, que ya está dentro de `gas`".into(),
                    action: "tratar el pago como embedded en gas; no volver a restarlo".into(),
                });
            }
        }

        // 2. Priority tip contado como línea propia además de dentro de gas.
        if self.components.iter().any(|c| c.kind == gas::KIND_GAS_EXECUTION)
            && externals
                .iter()
                .any(|c| c.kind == "priority_fee" || c.kind == "priority_tip")
        {
            findings.push(DoubleCountFinding {
                code: "priority_tip_double_counted".into(),
                kinds: vec![KIND_GAS.into(), "priority_tip".into()],
                why: "el priority fee es parte del precio efectivo del gas".into(),
                action: "dejar el tip SOLO dentro de `gas`; una línea propia lo duplica".into(),
            });
        }

        // 3. Blob fee + L1 data fee derivado del MISMO blob gas.
        let blob = self.components.iter().find(|c| c.kind == gas::KIND_BLOB_FEE);
        if let Some(blob) = blob {
            if let Some(l1) = self
                .components
                .iter()
                .find(|c| c.kind == gas::KIND_L2_DATA_FEE)
            {
                if l1
                    .note
                    .as_deref()
                    .is_some_and(|n| n.contains("includes_blob_gas"))
                {
                    findings.push(DoubleCountFinding {
                        code: "blob_fee_inside_l1_data_fee".into(),
                        kinds: vec![blob.kind.clone(), l1.kind.clone()],
                        why: "el componente L1 ya incluye el coste de blobs (Ecotone)".into(),
                        action: "eliminar la línea de blob independiente o el término de blob del L1, nunca ambos".into(),
                    });
                }
            }
        }

        // 4. Protocol fee V3 (es un CORTE de la comisión LP, no un cargo extra).
        if self
            .components
            .iter()
            .any(|c| c.kind == univ3::KIND_V3_PROTOCOL_FEE && c.treatment == Treatment::External)
        {
            findings.push(DoubleCountFinding {
                code: "v3_protocol_fee_is_a_cut_of_the_lp_fee".into(),
                kinds: vec![univ3::KIND_V3_PROTOCOL_FEE.into()],
                why: "en V3 el protocol fee se toma DE la comisión LP ya descontada en `amount_out`".into(),
                action: "marcarlo embedded; restarlo como externo lo descuenta dos veces".into(),
            });
        }

        // 5. Comisión de swap declarada externa en un quote atómico.
        if self.economic_kind == "atomic_quote"
            && self
                .components
                .iter()
                .any(|c| bridge_kind(&c.kind) == Some(KIND_EXECUTION_FEES)
                    && c.treatment == Treatment::External)
        {
            findings.push(DoubleCountFinding {
                code: "execution_fee_external_on_atomic_quote".into(),
                kinds: vec![KIND_EXECUTION_FEES.into()],
                why: "las comisiones de swap ya están dentro de `amount_out` del ledger".into(),
                action: "declararlas embedded; el bridge rechaza `execution_fees` external".into(),
            });
        }

        // 6. Financiación externa sobre un basis que ya la retuvo.
        if self.profit_basis == "retained_after_repayment"
            && self
                .components
                .iter()
                .any(|c| c.kind == funding::KIND_FINANCING_PREMIUM
                    && c.treatment == Treatment::External)
        {
            findings.push(DoubleCountFinding {
                code: "financing_inside_retained_spread".into(),
                kinds: vec![KIND_FINANCING.into()],
                why: "`profit_basis = retained_after_repayment` significa que el excedente ya devolvió el préstamo".into(),
                action: "no volver a restar el premium; el bridge lo rechaza".into(),
            });
        }

        // 7. Un embedded declarado TAMBIÉN como externo en otra categoría.
        for c in &self.components {
            if c.treatment == Treatment::External {
                if let Some(twin) = self.components.iter().find(|o| {
                    o.kind == c.kind
                        && o.treatment == Treatment::Embedded
                        && !std::ptr::eq(*o, c)
                }) {
                    findings.push(DoubleCountFinding {
                        code: "same_kind_both_embedded_and_external".into(),
                        kinds: vec![c.kind.clone()],
                        why: format!(
                            "{} aparece embedded ({}) y external ({})",
                            c.kind,
                            twin.scope.key(),
                            c.scope.key()
                        ),
                        action: "un solo tratamiento por kind; el bridge rechaza el duplicado".into(),
                    });
                }
            }
        }

        // 8. Rebate con signo perdido: un rebate positivo restado como coste
        //    inflaría el coste; sumado como ingreso sin su flujo, lo oculta.
        if !self.rebates.is_empty()
            && self
                .components
                .iter()
                .any(|c| c.direction == Direction::Rebate && c.usd.is_some() && !c.is_external())
        {
            findings.push(DoubleCountFinding {
                code: "rebate_without_signed_flow".into(),
                kinds: self.rebates.iter().map(|r| r.kind.clone()).collect(),
                why: "un rebate embebido no reduce ningún coste externo".into(),
                action: "registrarlo como flujo con signo (incoming) además del coste bruto".into(),
            });
        }

        findings
    }

    /// ¿Se puede cerrar el neto? Falso mientras haya bloqueos o errores.
    pub fn is_net_closable(&self) -> bool {
        self.construction_errors.is_empty() && self.blocking().is_empty()
    }
}

/// Formato plano (sin exponente) y recortado, apto para `Usd::parse` del bridge.
pub fn plain(v: &BigDecimal) -> String {
    v.normalized().to_plain_string()
}

/// Redondeo CONSERVADOR hacia abajo a `scale` decimales (nunca redondea a
/// favor del neto).
pub fn floor_scale(v: &BigDecimal, scale: i64) -> BigDecimal {
    v.with_scale_round(scale, RoundingMode::Floor)
}

// ─────────────────────────────────────────────────────────────────────────────
// Aritmética entera con overflow controlado (§4)
// ─────────────────────────────────────────────────────────────────────────────

pub fn mul_checked(a: &U256, b: &U256, op: &str) -> Result<U256, CostError> {
    a.checked_mul(*b).ok_or_else(|| CostError::Overflow { op: op.into() })
}

pub fn add_checked(a: &U256, b: &U256, op: &str) -> Result<U256, CostError> {
    a.checked_add(*b).ok_or_else(|| CostError::Overflow { op: op.into() })
}

pub fn sub_checked(a: &U256, b: &U256, op: &str) -> Result<U256, CostError> {
    a.checked_sub(*b).ok_or_else(|| CostError::Overflow { op: op.into() })
}

/// División entera con denominador no nulo explícito (floor, como la EVM).
pub fn div_floor(a: &U256, b: &U256, op: &str) -> Result<U256, CostError> {
    if b.is_zero() {
        return Err(CostError::ZeroDenominator {
            denominator: op.into(),
        });
    }
    Ok(*a / *b)
}

/// `amount * numerator / denominator` con floor, en un solo paso por cada
/// multiplicación (mismo orden que la EVM: multiplicar primero, dividir después).
pub fn proportion_floor(
    amount: &U256,
    numerator: &U256,
    denominator: &U256,
    op: &str,
) -> Result<U256, CostError> {
    let scaled = mul_checked(amount, numerator, op)?;
    div_floor(&scaled, denominator, op)
}

/// Lee un decimal entero no negativo desde `U256`.
pub fn u256_from_dec(s: &str) -> Result<U256, CostError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CostError::InvalidRead {
            read: format!("u256:{s}"),
            why: "not_a_nonnegative_integer".into(),
        });
    }
    U256::from_dec_str(s).map_err(|_| CostError::InvalidRead {
        read: format!("u256:{s}"),
        why: "out_of_range".into(),
    })
}

/// Decimal exacto desde `U256` (sin pasar por `f64`).
pub fn u256_to_bigdecimal(v: &U256) -> BigDecimal {
    BigDecimal::from_str(&v.to_string()).unwrap_or_else(|_| BigDecimal::from(0))
}

/// `10^decimals` como decimal exacto.
pub fn ten_pow(decimals: u8) -> BigDecimal {
    BigDecimal::from(10u64).powi(decimals as i64)
}

// ─────────────────────────────────────────────────────────────────────────────
// Contexto de emisión
// ─────────────────────────────────────────────────────────────────────────────

/// Contexto común de un ámbito valorado. Agrupa lo que TODO componente
/// necesita: identidad del venue, activo, precio con procedencia, revisión
/// exigida y ancla del estado leído.
pub struct ComponentCtx<'a> {
    pub chain_id: u64,
    pub scope: Scope,
    pub asset: AssetRef,
    pub price: &'a PriceRef,
    pub price_revision: &'a str,
    pub now_ms: u64,
    pub anchor: &'a VenueAnchor,
}

impl<'a> ComponentCtx<'a> {
    /// Valida el precio (procedencia, revisión, vigencia, identidad) sin valorar.
    pub fn check_price(&self) -> Result<(), CostError> {
        if !self.anchor.is_anchored() {
            return Err(CostError::MissingAnchor {
                read: format!("anchor:{}", self.scope.key()),
            });
        }
        if self.anchor.chain_id != self.chain_id {
            return Err(CostError::InvalidRead {
                read: format!("anchor:{}", self.scope.key()),
                why: "anchor_chain_mismatch".into(),
            });
        }
        self.price
            .check(self.now_ms, self.price_revision, &self.asset)
    }

    /// Valora `amount` unidades mínimas del activo del ámbito.
    pub fn value(&self, amount: &U256) -> Result<BigDecimal, CostError> {
        self.check_price()?;
        self.price.value_min_units(amount)
    }
}
