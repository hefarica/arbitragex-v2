//! §5 «V3 y concentrada» — fee tier REAL, liquidez y ticks completos o quoter
//! exacto, y **comisión protocolaria contabilizada según su relación con la
//! comisión total**.
//!
//! ## La relación que importa
//!
//! En Uniswap V3 el protocol fee **no es un cargo adicional**: es un CORTE de la
//! comisión que ya se descontó en `amount_out`. `UniswapV3Pool.swap` acumula
//! `protocolFees` sólo cuando el swap CRUZA al menos un tick inicializado:
//!
//! ```text
//! if (state.tick != slot0Start.tick) {
//!     uint128 protocolFeesOwed = (feeGrowthGlobal - feeGrowthInsideLast)
//!         * liquidity / protocolFeeDenominator;   // denominador del factory
//! }
//! ```
//!
//! Consecuencias contables, todas verificables:
//! * el swapper paga `fee/1e6` del input — **una sola vez**, dentro del quote;
//! * `protocolFee/1e6` de esa comisión va al protocolo, el resto al LP;
//! * **si no se cruza ningún tick, la comisión protocolaria del swap es CERO**
//!   aunque `protocolFee > 0`. Es un cero acreditado, no una ausencia.
//!
//! Por eso el tier se lee del pool (`fee()` es uint24 en centésimas de bip,
//! denominador 1e6) y nunca se asume. `crate::v3_fee_catalog` ya resuelve QUÉ
//! tier usar para cotizar (pool catalogado vs offered); este módulo consume ese
//! tier y produce la CONTABILIDAD de la comisión, que el catálogo no hace.

use ethers::types::U256;
use serde::{Deserialize, Serialize};

use super::{
    AssetRef, ComponentCtx, CostComponent, CostError, CostState, Direction, Scope, Treatment, Unit,
    VenueAnchor,
};

pub const ADAPTER: &str = "uniswap_v3_adapter/1";

/// Comisión que se queda el pool para los LPs (tras el corte protocolar).
pub const KIND_V3_LP_FEE: &str = "v3_lp_fee";
/// Corte protocolar de esa comisión. `Embedded`: sale DE la comisión, no se suma.
pub const KIND_V3_PROTOCOL_FEE: &str = "v3_protocol_fee";

/// Denominador del fee tier V3: centésimas de bip (`uint24`).
pub const V3_FEE_DENOMINATOR: u32 = 1_000_000;

/// Lecturas autoritativas de la comisión de un pool V3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V3FeeReads {
    /// `pool.fee()` — uint24, centésimas de bip.
    pub fee_raw: U256,
    /// `factory.protocolFee()` — numerador.
    pub protocol_fee_num_raw: U256,
    /// `factory.protocolFeeDenominator()` — denominador (0 en factories viejas).
    pub protocol_fee_den_raw: U256,
    /// ¿El swap evaluado CRUZA al menos un tick inicializado?
    pub crosses_initialized_tick: bool,
    pub source: String,
    pub evidence_id: String,
}

/// Términos de comisión V3 resueltos, con el corte protocolar separado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V3FeeTerms {
    pub fee_pips: u32,
    /// Corte protocolar EFECTIVO para ESTE swap (cero si no cruza ticks).
    pub protocol_pips: u32,
    /// Comisión que queda para los LPs.
    pub lp_pips: u32,
    pub crosses_initialized_tick: bool,
    pub source: String,
    pub evidence_id: String,
}

impl V3FeeTerms {
    /// Resuelve los términos desde las lecturas. Un `fee_raw` fuera de rango o
    /// un denominador cero NO se aproximan: son error.
    pub fn resolve(reads: &V3FeeReads) -> Result<Self, CostError> {
        if reads.fee_raw >= U256::from(V3_FEE_DENOMINATOR) {
            return Err(CostError::InvalidRead {
                read: format!("{}:fee()", reads.source),
                why: format!("fee_tier_out_of_range:{}", reads.fee_raw),
            });
        }
        let fee_pips = reads.fee_raw.as_u32();
        // `protocolFee` es uint8 en el factory (máx 255). El denominador por
        // defecto documentado es 100; una factory que exponga 0 no permite
        // calcular el corte → pendiente, jamás "0 asumido".
        let protocol_pips = if reads.protocol_fee_num_raw.is_zero() {
            0u32
        } else {
            if reads.protocol_fee_den_raw.is_zero() {
                return Err(CostError::ZeroDenominator {
                    denominator: format!("{}:protocolFeeDenominator()", reads.source),
                });
            }
            let num = reads.protocol_fee_num_raw;
            let den = reads.protocol_fee_den_raw;
            let cut = U256::from(fee_pips) * num / den;
            cut.as_u32()
        };
        // Sin cruce de tick inicializado, la comisión protocolar del swap es 0
        // por diseño del contrato (ver el `if` de `UniswapV3Pool.swap`).
        let protocol_pips = if reads.crosses_initialized_tick {
            protocol_pips
        } else {
            0
        };
        let lp_pips = fee_pips.saturating_sub(protocol_pips);
        Ok(Self {
            fee_pips,
            protocol_pips,
            lp_pips,
            crosses_initialized_tick: reads.crosses_initialized_tick,
            source: reads.source.clone(),
            evidence_id: reads.evidence_id.clone(),
        })
    }

    /// Comisión total cobrada al swapper, en unidades mínimas del input.
    pub fn total_fee_raw(&self, amount_in: &U256) -> Result<U256, CostError> {
        super::proportion_floor(
            amount_in,
            &U256::from(self.fee_pips),
            &U256::from(V3_FEE_DENOMINATOR),
            "v3:fee",
        )
    }

    /// Corte protocolar, en unidades mínimas del input.
    pub fn protocol_fee_raw(&self, amount_in: &U256) -> Result<U256, CostError> {
        super::proportion_floor(
            amount_in,
            &U256::from(self.protocol_pips),
            &U256::from(V3_FEE_DENOMINATOR),
            "v3:protocol_fee",
        )
    }

    /// Comisión que queda al LP, en unidades mínimas del input.
    pub fn lp_fee_raw(&self, amount_in: &U256) -> Result<U256, CostError> {
        super::proportion_floor(
            amount_in,
            &U256::from(self.lp_pips),
            &U256::from(V3_FEE_DENOMINATOR),
            "v3:lp_fee",
        )
    }
}

/// Comisión LP de una pierna V3. `Embedded` en `amount_out`.
pub fn lp_fee_component(
    ctx: &ComponentCtx<'_>,
    terms: &V3FeeTerms,
    amount_in: &U256,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let raw = terms.lp_fee_raw(amount_in)?;
    let usd = ctx.value(&raw)?;
    Ok(CostComponent {
        kind: KIND_V3_LP_FEE.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: raw,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: Some(usd),
        payer: payer.to_owned(),
        beneficiary: Some("lp_positions".into()),
        source: format!("{}:fee()={}pips", terms.source, terms.fee_pips),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("{}:lp_share", terms.evidence_id),
        state: CostState::Resolved,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(format!(
            "fee tier {} pips de 1e6; incluida en amount_out, no se descuenta otra vez",
            terms.fee_pips
        )),
    })
}

/// Corte protocolar del swap. Siempre `Embedded`: es una PARTE de la comisión
/// ya descontada, no un cargo extra.
pub fn protocol_fee_component(
    ctx: &ComponentCtx<'_>,
    terms: &V3FeeTerms,
    amount_in: &U256,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let raw = terms.protocol_fee_raw(amount_in)?;
    let (state, usd, note) = if terms.protocol_pips == 0 {
        (
            CostState::ZeroAttested,
            Some(bigdecimal::BigDecimal::from(0)),
            if terms.crosses_initialized_tick {
                format!(
                    "protocolFee del factory es 0 para {}: corte protocolar exactamente cero",
                    terms.source
                )
            } else {
                format!(
                    "el swap no cruza ningún tick inicializado en {}: la comisión protocolar del swap es 0 por diseño del contrato",
                    terms.source
                )
            },
        )
    } else {
        (
            CostState::Resolved,
            Some(ctx.value(&raw)?),
            format!(
                "corte protocolar = {} de {} pips, tomado DE la comisión LP (no se suma)",
                terms.protocol_pips, terms.fee_pips
            ),
        )
    };
    Ok(CostComponent {
        kind: KIND_V3_PROTOCOL_FEE.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: raw,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd,
        payer: payer.to_owned(),
        beneficiary: Some("protocol_treasury".into()),
        source: format!("{}:protocolFee", terms.source),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("{}:protocol_share", terms.evidence_id),
        state,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(note),
    })
}

/// Tarea de resolución cuando NO se recorrieron los ticks y no hay quoter
/// exacto: §9 exige el recibo `full_tick_traversal_or_protocol_quoter`.
pub fn full_traversal_task(scope: &Scope, pool: &str) -> super::ResolutionTask {
    super::ResolutionTask {
        cost_kind: KIND_V3_LP_FEE.into(),
        scope: scope.clone(),
        receipt: Some("full_tick_traversal_or_protocol_quoter".into()),
        provider: pool.to_owned(),
        read: "factory.getPool(tokenIn,tokenOut,fee) + QuoterV2.quoteExactInputSingle".into(),
        why: "tick_traversal_incomplete_and_no_protocol_quoter".into(),
    }
}

/// Activo de la pierna V3.
pub fn leg_asset(chain_id: u64, token: &str, decimals: u8) -> AssetRef {
    AssetRef::new(chain_id, token, decimals)
}

/// Ancla EVM de la lectura de `slot0`/`liquidity`.
pub fn evm_anchor(chain_id: u64, block: u64) -> VenueAnchor {
    VenueAnchor::evm(chain_id, block, None)
}
