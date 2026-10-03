//! §5 «Builder/relay» — **pago adicional efectivamente propuesto, SEPARADO del
//! priority fee ya incluido en gas**; optimizar el bid dentro de la política
//! existente.
//!
//! ## La regla que evita el doble conteo
//!
//! Un pago al builder puede materializarse de dos maneras y sólo una es un
//! coste ADICIONAL:
//!
//! | modo | dónde vive el pago | tratamiento |
//! |---|---|---|
//! | elevando el priority fee de la tx | dentro de `effectiveGasPrice` | `Embedded` en `gas` |
//! | transferencia a `block.coinbase` | fuera de la tx | `External` |
//! | diferencia de coinbase del bundle | fuera de la tx | `External` |
//! | fee del relay | fuera de la tx | `External` |
//!
//! Declarar `External` un pago hecho por la vía del priority fee lo cuenta dos
//! veces: una dentro de `gas` y otra como línea propia. [`BuilderBid::treatment`]
//! deriva el tratamiento del MODO, no de una preferencia del llamador, y
//! [`super::CostResolution::double_count_audit`] lo vuelve a comprobar sobre la
//! resolución completa.
//!
//! ## Rebates
//!
//! Un refund (`mev_share`, `MEV-Share` hints, devolución del relay) es un flujo
//! con SIGNO. Se emite como [`Direction::Rebate`] y viaja en
//! `CostResolution::rebates`: el contrato `CostLine` sólo admite importes no
//! negativos y un rebate no puede convertirse en un coste negativo.

use ethers::types::U256;
use serde::{Deserialize, Serialize};

use super::{
    AssetRef, ComponentCtx, CostComponent, CostError, CostState, Direction, Scope, Treatment, Unit,
    VenueAnchor,
};

pub const ADAPTER: &str = "builder_relay_adapter/1";

pub const KIND_BUILDER_BID: &str = "builder_bid";
pub const KIND_BUILDER_REBATE: &str = "builder_rebate";

/// Cómo se materializa el pago al builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuilderPaymentMode {
    /// No hay pago al builder (ruta pública / sin bundle).
    None,
    /// El pago se hizo SUBIENDO el priority fee de la transacción.
    PriorityFeeTopUp,
    /// Transferencia explícita a `block.coinbase` dentro del bundle.
    SeparateCoinbaseTransfer,
    /// Diferencia de balance de coinbase del bundle (`coinbaseDiff`).
    BundleCoinbaseDiff,
    /// Fee cobrada por el relay.
    RelayFee,
}

impl BuilderPaymentMode {
    /// Tratamiento contable IMPUESTO por el modo.
    pub fn treatment(&self) -> Treatment {
        match self {
            Self::None => Treatment::NotApplicable,
            // Ya está dentro del precio efectivo del gas.
            Self::PriorityFeeTopUp => Treatment::Embedded,
            Self::SeparateCoinbaseTransfer | Self::BundleCoinbaseDiff | Self::RelayFee => {
                Treatment::External
            }
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PriorityFeeTopUp => "priority_fee_top_up",
            Self::SeparateCoinbaseTransfer => "separate_coinbase_transfer",
            Self::BundleCoinbaseDiff => "bundle_coinbase_diff",
            Self::RelayFee => "relay_fee",
        }
    }
}

/// Bid efectivamente propuesto, con su evidencia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderBid {
    pub relay: String,
    pub mode: BuilderPaymentMode,
    pub amount_wei: U256,
    /// Fuente de la lectura: `eth_sendBundle` payload, `flashbots_getBundleStats`,
    /// el propio `coinbaseDiff` simulado, etc.
    pub source: String,
    pub evidence_id: String,
}

impl BuilderBid {
    /// Un pago sin evidencia no es un pago: es un desconocido.
    pub fn validate(&self) -> Result<(), CostError> {
        if self.source.trim().is_empty() || self.evidence_id.trim().is_empty() {
            return Err(CostError::InvalidRead {
                read: format!("builder:{}", self.relay),
                why: "bid_without_provenance".into(),
            });
        }
        if self.mode != BuilderPaymentMode::None && self.amount_wei.is_zero() {
            return Err(CostError::MissingRead {
                provider: self.relay.clone(),
                read: "builder_payment_amount".into(),
            });
        }
        Ok(())
    }
}

/// Componente del pago al builder, con el tratamiento que el MODO impone.
pub fn bid_component(ctx: &ComponentCtx<'_>, bid: &BuilderBid) -> Result<CostComponent, CostError> {
    bid.validate()?;
    let treatment = bid.mode.treatment();
    if treatment == Treatment::NotApplicable {
        return Ok(CostComponent {
            kind: KIND_BUILDER_BID.into(),
            scope: ctx.scope.clone(),
            asset: AssetRef::native(ctx.chain_id, ctx.asset.decimals),
            amount_raw: U256::zero(),
            unit: Unit::MinUnits,
            direction: Direction::Charge,
            usd: None,
            payer: "searcher".into(),
            beneficiary: None,
            source: bid.source.clone(),
            adapter_version: ADAPTER.into(),
            anchor: Some(ctx.anchor.clone()),
            evidence_id: bid.evidence_id.clone(),
            state: CostState::NotApplicable,
            treatment: Treatment::NotApplicable,
            embedded_in_quote: false,
            note: Some(format!(
                "modo {}: no se propuso pago adicional al builder",
                bid.mode.as_str()
            )),
        });
    }
    let usd = ctx.value(&bid.amount_wei)?;
    let embedded = treatment == Treatment::Embedded;
    Ok(CostComponent {
        kind: KIND_BUILDER_BID.into(),
        scope: ctx.scope.clone(),
        asset: AssetRef::native(ctx.chain_id, ctx.asset.decimals),
        amount_raw: bid.amount_wei,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: Some(usd),
        payer: "searcher".into(),
        beneficiary: Some(bid.relay.clone()),
        source: bid.source.clone(),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: bid.evidence_id.clone(),
        state: CostState::Resolved,
        treatment,
        embedded_in_quote: embedded,
        note: Some(match bid.mode {
            BuilderPaymentMode::PriorityFeeTopUp => {
                "pago por la vía del priority fee: YA está dentro de la línea `gas`; no se resta otra vez".into()
            }
            BuilderPaymentMode::SeparateCoinbaseTransfer => {
                "transferencia a block.coinbase: coste ADICIONAL a la línea `gas`".into()
            }
            BuilderPaymentMode::BundleCoinbaseDiff => {
                "diferencia de coinbase del bundle: coste ADICIONAL a la línea `gas`".into()
            }
            BuilderPaymentMode::RelayFee => {
                "fee del relay: coste ADICIONAL a la línea `gas`".into()
            }
            BuilderPaymentMode::None => unreachable!(),
        }),
    })
}

/// Rebate/refund del relay o de MEV-Share: flujo con signo propio.
pub fn rebate_component(
    ctx: &ComponentCtx<'_>,
    amount_wei: &U256,
    relay: &str,
    evidence_id: &str,
) -> Result<CostComponent, CostError> {
    if evidence_id.trim().is_empty() {
        return Err(CostError::InvalidRead {
            read: format!("builder_rebate:{relay}"),
            why: "rebate_without_evidence".into(),
        });
    }
    let usd = ctx.value(amount_wei)?;
    Ok(CostComponent {
        kind: KIND_BUILDER_REBATE.into(),
        scope: ctx.scope.clone(),
        asset: AssetRef::native(ctx.chain_id, ctx.asset.decimals),
        amount_raw: *amount_wei,
        unit: Unit::MinUnits,
        direction: Direction::Rebate,
        usd: Some(usd),
        payer: relay.to_owned(),
        beneficiary: Some("searcher".into()),
        source: format!("builder_rebate:{relay}"),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: evidence_id.to_owned(),
        state: CostState::Resolved,
        treatment: Treatment::Embedded,
        embedded_in_quote: false,
        note: Some("refund del relay: flujo con signo, no un coste negativo".into()),
    })
}

/// Tarea cuando el bundle se propone sin conocer el pago efectivo.
pub fn bid_task(scope: &Scope, relay: &str) -> super::ResolutionTask {
    super::ResolutionTask {
        cost_kind: KIND_BUILDER_BID.into(),
        scope: scope.clone(),
        receipt: None,
        provider: relay.to_owned(),
        read: "eth_sendBundle payload / flashbots_getBundleStats(bundleHash)".into(),
        why: "proposed_bid_amount_unknown".into(),
    }
}

/// Ancla del bloque objetivo del bundle.
pub fn evm_anchor(chain_id: u64, block: u64) -> VenueAnchor {
    VenueAnchor::evm(chain_id, block, None)
}
