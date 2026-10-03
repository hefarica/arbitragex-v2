//! §5 «Swaps V2 y forks» — **tarifa y DENOMINADOR efectivos del deployment**,
//! reglas de cantidades y redondeo.
//!
//! El punto del prompt no es «V2 cobra 30 bps». Es que **no todos los forks
//! tienen la tarifa de Uniswap V2** y que la tarifa viaja como un PAR
//! (numerador, denominador) que el propio contrato usa en su `getAmountOut`:
//!
//! ```text
//! UniswapV2 / SushiSwap    getAmountOut: amountInWithFee = amountIn * 997
//!                                         denom = reserveIn * 1000 + amountInWithFee
//! PancakeSwap V2           getAmountOut: amountInWithFee = amountIn * 9975
//!                                         denom = reserveIn * 10000 + amountInWithFee
//! ```
//!
//! La forma general es la MISMA con `(numerator, denominator)` efectivos:
//!
//! ```text
//! amount_in_with_fee = amount_in * numerator
//! out                = amount_in_with_fee * reserve_out
//!                      / (reserve_in * denominator + amount_in_with_fee)
//! ```
//!
//! `crate::amm_math::v2_amount_out` implementa el caso `(10_000 - fee_bps,
//! 10_000)` y es el kernel de cotización existente (zona compartida: no se
//! toca). Este módulo produce el **par efectivo** desde la lectura
//! autoritativa del deployment y contabiliza la comisión retenida, que es lo
//! que el kernel no expone.
//!
//! Reglas fijadas aquí:
//! * la tarifa se LEE. [`V2Fork::documented_terms`] existe sólo para
//!   **contrastar** la lectura y generar una tarea de discrepancia; nunca se
//!   usa como valor productivo.
//! * el redondeo del contrato es **floor** en cada división; la comisión
//!   retenida es `amount_in - floor(amount_in * numerator / denominator)`,
//!   que es exactamente lo que el pool deja de recibir.
//! * la frontera de reserva se respeta: `amount_in > reserve_in` es un error
//!   de ámbito, no un cero.
//! * un token con comportamiento especial (fee-on-transfer / rebasing) NO se
//!   asume: produce tarea de resolución.

use ethers::types::U256;
use serde::{Deserialize, Serialize};

use super::{
    add_checked, div_floor, mul_checked, proportion_floor, sub_checked, AssetRef, ComponentCtx,
    CostComponent, CostError, CostState, Direction, Scope, Treatment, Unit, VenueAnchor,
};

pub const ADAPTER: &str = "uniswap_v2_fork_adapter/1";

/// Comisión retenida por el pool, ya dentro de `amount_out`.
pub const KIND_V2_LP_FEE: &str = "v2_lp_fee";
/// Premium de un flash swap V2: el pool cobra su propia tarifa de swap sobre el
/// importe prestado. Es financiación, no comisión de ruta (§5).
pub const KIND_V2_FLASH_PREMIUM: &str = "v2_flash_swap_premium";

/// Fork conocido. Sólo para CONTRASTAR la lectura del deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum V2Fork {
    UniswapV2,
    SushiSwap,
    PancakeSwapV2,
    /// Deployment V2-compatible sin tarifa documentada de fábrica.
    Unknown,
}

impl V2Fork {
    /// Par `(numerator, denominator)` DOCUMENTADO del fork.
    ///
    /// **No es un valor productivo.** Devuelve `None` para `Unknown` — un
    /// deployment sin tarifa documentada obliga a leer el contrato.
    pub fn documented_terms(&self) -> Option<(u64, u64)> {
        match self {
            Self::UniswapV2 | Self::SushiSwap => Some((997, 1000)),
            Self::PancakeSwapV2 => Some((9975, 10_000)),
            Self::Unknown => None,
        }
    }
}

/// Términos de comisión EFECTIVOS del deployment, con su procedencia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V2FeeTerms {
    pub numerator: U256,
    pub denominator: U256,
    /// Lectura de la que salieron (contrato.función).
    pub source: String,
    pub evidence_id: String,
    pub fork: V2Fork,
}

impl V2FeeTerms {
    /// Construye los términos desde una LECTURA del deployment.
    ///
    /// Rechaza denominador cero, numerador cero y numerador > denominador (una
    /// tarifa ≥ 100% no es una tarifa: es un dato corrupto).
    pub fn from_read(
        numerator: U256,
        denominator: U256,
        fork: V2Fork,
        source: &str,
        evidence_id: &str,
    ) -> Result<Self, CostError> {
        if denominator.is_zero() {
            return Err(CostError::ZeroDenominator {
                denominator: format!("{source}:denominator"),
            });
        }
        if numerator.is_zero() {
            return Err(CostError::InvalidRead {
                read: format!("{source}:numerator"),
                why: "zero_numerator_invalid_fee".into(),
            });
        }
        if numerator > denominator {
            return Err(CostError::InvalidRead {
                read: format!("{source}:numerator"),
                why: "numerator_exceeds_denominator".into(),
            });
        }
        if source.trim().is_empty() || evidence_id.trim().is_empty() {
            return Err(CostError::InvalidRead {
                read: format!("{source}:provenance"),
                why: "fee_terms_without_provenance".into(),
            });
        }
        Ok(Self {
            numerator,
            denominator,
            source: source.to_owned(),
            evidence_id: evidence_id.to_owned(),
            fork,
        })
    }

    /// Contraste contra la tarifa documentada del fork. `None` = coherente (o
    /// fork sin tarifa documentada). `Some((lectura, documento))` = discrepancia
    /// que exige tarea: la LECTURA manda, la discrepancia se reporta.
    pub fn cross_check(&self) -> Option<((String, String), (u64, u64))> {
        let documented = self.fork.documented_terms()?;
        let matches = self.numerator == U256::from(documented.0)
            && self.denominator == U256::from(documented.1);
        if matches {
            None
        } else {
            Some((
                (self.numerator.to_string(), self.denominator.to_string()),
                documented,
            ))
        }
    }

    /// Tarea de resolución cuando la lectura contradice la documentación del
    /// fork: hay que confirmar el deployment real, no asumir la tarifa.
    pub fn disagreement_task(&self, scope: &Scope) -> Option<super::ResolutionTask> {
        let (read, documented) = self.cross_check()?;
        Some(super::ResolutionTask {
            cost_kind: KIND_V2_LP_FEE.into(),
            scope: scope.clone(),
            receipt: Some("exact_invariant_version_and_rates".into()),
            provider: self.source.clone(),
            read: format!(
                "confirm_deployment_fee_terms: read {}:{} vs documented {}:{}",
                read.0, read.1, documented.0, documented.1
            ),
            why: "deployment_fee_terms_differ_from_fork_documentation".into(),
        })
    }

    /// Fracción de comisión como decimal exacto (`(den - num) / den`).
    pub fn fee_fraction(&self) -> bigdecimal::BigDecimal {
        use bigdecimal::BigDecimal;
        use std::str::FromStr;
        let num =
            BigDecimal::from_str(&self.numerator.to_string()).unwrap_or_else(|_| BigDecimal::from(0));
        let den = BigDecimal::from_str(&self.denominator.to_string())
            .unwrap_or_else(|_| BigDecimal::from(1));
        (den.clone() - num) / den
    }
}

/// Resultado exacto de una cotización `getAmountOut` V2-compatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2Out {
    pub amount_out: U256,
    /// Input efectivamente aceptado por el pool (tras la comisión).
    pub amount_in_with_fee_effective: U256,
    /// Comisión retenida por el pool en el token de entrada.
    pub fee_raw: U256,
}

/// `getAmountOut` exacto, con el par efectivo del deployment.
///
/// Replica la aritmética entera del contrato: multiplicar primero, dividir
/// después, **floor**. Cada operación con overflow controlado (§4).
pub fn amount_out(
    terms: &V2FeeTerms,
    amount_in: &U256,
    reserve_in: &U256,
    reserve_out: &U256,
) -> Result<V2Out, CostError> {
    if amount_in.is_zero() {
        return Err(CostError::InvalidRead {
            read: "v2:amount_in".into(),
            why: "zero_amount_in".into(),
        });
    }
    if reserve_in.is_zero() || reserve_out.is_zero() {
        return Err(CostError::InvalidRead {
            read: "v2:reserves".into(),
            why: "zero_reserve".into(),
        });
    }
    if amount_in > reserve_in {
        return Err(CostError::ReserveBoundary {
            scope: terms.source.clone(),
            amount_in: amount_in.to_string(),
            reserve: reserve_in.to_string(),
        });
    }
    let amount_in_with_fee = mul_checked(amount_in, &terms.numerator, "v2:amount_in*numerator")?;
    let numerator = mul_checked(&amount_in_with_fee, reserve_out, "v2:numerator")?;
    let reserve_scaled = mul_checked(reserve_in, &terms.denominator, "v2:reserve_in*denominator")?;
    let denominator = add_checked(&reserve_scaled, &amount_in_with_fee, "v2:denominator")?;
    let out = div_floor(&numerator, &denominator, "v2:getAmountOut")?;
    let effective = div_floor(&amount_in_with_fee, &terms.denominator, "v2:amountInWithFee")?;
    let fee_raw = sub_checked(amount_in, &effective, "v2:fee")?;
    Ok(V2Out {
        amount_out: out,
        amount_in_with_fee_effective: effective,
        fee_raw,
    })
}

/// Comisión retenida por el pool (sin necesidad de las reservas).
pub fn fee_taken_raw(terms: &V2FeeTerms, amount_in: &U256) -> Result<U256, CostError> {
    let effective = proportion_floor(
        amount_in,
        &terms.numerator,
        &terms.denominator,
        "v2:fee_effective_input",
    )?;
    sub_checked(amount_in, &effective, "v2:fee")
}

/// Componente económico de la comisión LP de una pierna V2.
///
/// `Treatment::Embedded`: la comisión ya está dentro de `amount_out` de la
/// cotización del ledger — se declara y **no se vuelve a restar**.
#[allow(clippy::too_many_arguments)]
pub fn lp_fee_component(
    ctx: &ComponentCtx<'_>,
    terms: &V2FeeTerms,
    amount_in: &U256,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let fee_raw = fee_taken_raw(terms, amount_in)?;
    let usd = ctx.value(&fee_raw)?;
    Ok(CostComponent {
        kind: KIND_V2_LP_FEE.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: fee_raw,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: Some(usd),
        payer: payer.to_owned(),
        beneficiary: Some("lp_pool".into()),
        source: terms.source.clone(),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: terms.evidence_id.clone(),
        state: CostState::Resolved,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(format!(
            "tarifa efectiva {}/{} ({}); incluida en amount_out, no se descuenta otra vez",
            terms.numerator, terms.denominator, terms.source
        )),
    })
}

/// Una comisión acreditada como CERO por la lectura del deployment (p. ej. un
/// fork con `numerator == denominator`). Se conserva como cero explícito: un
/// cero acreditado no es una ausencia.
pub fn zero_fee_component(
    ctx: &ComponentCtx<'_>,
    terms: &V2FeeTerms,
    payer: &str,
) -> CostComponent {
    CostComponent {
        kind: KIND_V2_LP_FEE.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: U256::zero(),
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: Some(bigdecimal::BigDecimal::from(0)),
        payer: payer.to_owned(),
        beneficiary: Some("lp_pool".into()),
        source: terms.source.clone(),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("{}:zero", terms.evidence_id),
        state: CostState::ZeroAttested,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(format!(
            "comisión exactamente cero según {} (numerador == denominador)",
            terms.source
        )),
    }
}

/// Premium de un flash swap V2 sobre el importe prestado.
///
/// El pool cobra su PROPIA tarifa de swap sobre el importe prestado; por eso el
/// premium se calcula con los términos efectivos de ESE pool y no con una
/// constante. `Treatment::External`: no está dentro de ninguna cotización.
pub fn flash_swap_premium_component(
    ctx: &ComponentCtx<'_>,
    terms: &V2FeeTerms,
    borrowed: &U256,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let premium_raw = fee_taken_raw(terms, borrowed)?;
    let usd = ctx.value(&premium_raw)?;
    Ok(CostComponent {
        kind: KIND_V2_FLASH_PREMIUM.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: premium_raw,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: Some(usd),
        payer: payer.to_owned(),
        beneficiary: Some("lp_pool".into()),
        source: format!("{}:flashSwap", terms.source),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("{}:flash_swap", terms.evidence_id),
        state: CostState::Resolved,
        treatment: Treatment::External,
        embedded_in_quote: false,
        note: Some("premium del flash swap = tarifa de swap del propio pool sobre el principal".into()),
    })
}

/// Comportamiento especial del token que la aritmética V2 NO modela.
///
/// §14: «Para tokens con comportamientos especiales, soporta y verifica el
/// comportamiento o declara incompatibilidad específica». Aquí se declara.
pub fn special_token_task(scope: &Scope, token: &str, observed: &str) -> super::ResolutionTask {
    super::ResolutionTask {
        cost_kind: KIND_V2_LP_FEE.into(),
        scope: scope.clone(),
        receipt: Some("exact_integrated_cost_curve".into()),
        provider: token.to_owned(),
        read: format!("token_transfer_behaviour({token})"),
        why: format!("transfer_delta_differs_from_amount_in:{observed}"),
    }
}

/// Activo de una pierna V2 (token de entrada del swap).
pub fn leg_asset(chain_id: u64, token: &str, decimals: u8) -> AssetRef {
    AssetRef::new(chain_id, token, decimals)
}

/// Ancla EVM de la lectura de reservas.
pub fn evm_anchor(chain_id: u64, block: u64) -> VenueAnchor {
    VenueAnchor::evm(chain_id, block, None)
}
