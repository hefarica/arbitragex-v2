//! §5 «V4 y hooks» — PoolKey, LP fee estática/dinámica, protocol fee, hook fees
//! y cambios de balance personalizados; simular el hook y contexto efectivos.
//!
//! ## Por qué V4 NO se contabiliza como V3
//!
//! En V3 la comisión protocolar es un CORTE de la comisión LP. En V4 es un cargo
//! **ADITIVO**: el swapper paga la suma
//!
//! ```text
//! fee_total_pips = lp_fee + protocol_fee + hook_fee      (denominador 1e6)
//! ```
//!
//! y `Pool.swap` revierte si esa suma excede el máximo representable. Tratar V4
//! como V3 (restar el corte de la comisión) subestima el coste del swapper y
//! rompe la igualdad entre quote y ejecución. Este módulo implementa la
//! semántica ADITIVA y la deja explícita en el código, no en un comentario.
//!
//! ## LP fee estática vs dinámica
//!
//! `PoolKey.fee` es uint24 en centésimas de bip. Si el bit 23
//! ([`DYNAMIC_FEE_FLAG`], `0x800000`) está puesto, el valor del key NO es una
//! tarifa: es una marca. La tarifa real la fija el hook en tiempo de swap
//! (`updateDynamicLPFee`) y sólo puede leerse del estado del hook o del propio
//! swap → sin esa lectura el componente queda `PendingResolution` con la tarea
//! `dynamic_parameters_and_hook_state` (§9).
//!
//! ## Cambios de balance personalizados
//!
//! `beforeSwap` devuelve un `BeforeSwapDelta` (int128 unspecified / int128
//! specified). Un delta positivo a favor del swapper es un **rebate**: un flujo
//! con SIGNO. El contrato `CostLine` sólo admite importes no negativos, así que
//! el rebate viaja en `CostResolution::rebates` y el cargo bruto en la línea —
//! nunca se resta dos veces ni se pierde el signo.

use ethers::types::U256;
use serde::{Deserialize, Serialize};

use super::{
    AssetRef, ComponentCtx, CostComponent, CostError, CostState, Direction, Scope, Treatment, Unit,
    VenueAnchor,
};

pub const ADAPTER: &str = "uniswap_v4_adapter/1";

pub const KIND_V4_LP_FEE: &str = "v4_lp_fee";
pub const KIND_V4_PROTOCOL_FEE: &str = "v4_protocol_fee";
pub const KIND_V4_HOOK_FEE: &str = "v4_hook_fee";
pub const KIND_V4_HOOK_REBATE: &str = "v4_hook_rebate";

/// Denominador de todas las tarifas V4: centésimas de bip (1e6).
pub const V4_FEE_DENOMINATOR: u32 = 1_000_000;
/// `LPFeeLibrary.DYNAMIC_FEE_FLAG` — bit 23 del campo `fee` del PoolKey.
pub const DYNAMIC_FEE_FLAG: u32 = 0x0080_0000;
/// Máscara del campo `fee` del PoolKey (uint24).
pub const POOL_KEY_FEE_MASK: u32 = 0x00FF_FFFF;
/// Máximo de `protocolFee` en pips, constante documentada de
/// `ProtocolFeeLibrary.MAX_PROTOCOL_FEE` (1000 = 0.1%).
pub const MAX_PROTOCOL_FEE_PIPS: u32 = 1_000;
/// Factor entre el carril uint8 y los pips: `ProtocolFeeLibrary` deriva
/// `getZeroForOneFee(self) = uint24(self & 0xFF) * 100`, de modo que el carril
/// guarda unidades de 100 centésimas de bip. Sin este factor, un protocolFee
/// de 0.05% se leería como 0.0005%.
pub const PROTOCOL_FEE_LANE_MULTIPLIER: u32 = 100;
/// Máximo valor de un carril uint8.
pub const PROTOCOL_FEE_LANE_MAX: u32 = 0xFF;

/// `PoolKey` leído del `PoolManager`. Sólo el `fee` participa del coste aquí;
/// las monedas y el `hooks` son la identidad del ámbito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolKey {
    pub currency0: String,
    pub currency1: String,
    pub fee_raw: u32,
    pub tick_spacing: i32,
    pub hooks: String,
}

impl PoolKey {
    /// ¿El key marca tarifa dinámica gestionada por el hook?
    pub fn is_dynamic_fee(&self) -> bool {
        self.fee_raw & DYNAMIC_FEE_FLAG != 0
    }
    /// Tarifa estática del key. `None` cuando el key es dinámico o el valor no
    /// cabe en uint24: en ambos casos no hay tarifa utilizable en el key.
    pub fn static_lp_fee_pips(&self) -> Option<u32> {
        if self.is_dynamic_fee() {
            return None;
        }
        if self.fee_raw > V4_FEE_DENOMINATOR {
            return None;
        }
        Some(self.fee_raw)
    }
    pub fn identity(&self) -> String {
        format!(
            "v4:{}:{}:{}:{}:{}",
            self.currency0, self.currency1, self.fee_raw, self.tick_spacing, self.hooks
        )
    }
}

/// Estado de comisiones de un pool V4 para UN swap concreto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4FeeInputs {
    pub key: PoolKey,
    /// `Pool.Slot0.protocolFee` del `PoolManager` (uint16 empaquetado).
    pub protocol_fee_packed: u16,
    /// Tarifa dinámica efectiva aportada por el hook, si el key la marca.
    pub dynamic_lp_fee_pips: Option<u32>,
    /// `IHooks.getHookFee(...)` en centésimas de bip para ESTE swap.
    pub hook_fee_pips: Option<u32>,
    /// Delta de `beforeSwap` a favor del swapper (rebate), en unidades mínimas.
    pub before_swap_rebate_raw: Option<U256>,
    pub source: String,
    pub evidence_id: String,
}

/// Términos de comisión V4 resueltos: ADITIVOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V4FeeTerms {
    pub lp_pips: u32,
    pub protocol_pips: u32,
    pub hook_pips: u32,
    pub dynamic_lp_fee: bool,
    pub source: String,
    pub evidence_id: String,
}

impl V4FeeTerms {
    /// Resuelve los términos con la semántica aditiva de V4.
    ///
    /// * `protocolFee` empaquetado: `zeroForOne` en el byte bajo, `oneForZero`
    ///   en el alto. `ProtocolFeeLibrary` convierte cada carril uint8 a pips con
    ///   `× 100` y el máximo documentado es 1000 pips (0.1%): un carril mayor
    ///   que `MAX_PROTOCOL_FEE_PIPS / 100` es un dato corrupto, no una tarifa.
    /// * key dinámico sin lectura de la tarifa efectiva → `MissingRead` con la
    ///   llamada exacta (jamás se usa el valor del key como tarifa).
    /// * hook sin lectura de su fee → `MissingRead` (`getHookFee`).
    pub fn resolve(inputs: &V4FeeInputs, zero_for_one: bool) -> Result<Self, CostError> {
        let dynamic = inputs.key.is_dynamic_fee();
        let lp_pips = if dynamic {
            let fee = inputs.dynamic_lp_fee_pips.ok_or_else(|| CostError::MissingRead {
                provider: inputs.key.identity(),
                read: format!(
                    "dynamic_lp_fee({}) [LPFeeLibrary.DYNAMIC_FEE_FLAG en PoolKey.fee=0x{:06x}]",
                    inputs.key.hooks, inputs.key.fee_raw
                ),
            })?;
            if fee > V4_FEE_DENOMINATOR {
                return Err(CostError::InvalidRead {
                    read: format!("{}:dynamic_lp_fee", inputs.source),
                    why: format!("above_1e6:{fee}"),
                });
            }
            fee
        } else {
            inputs
                .key
                .static_lp_fee_pips()
                .ok_or_else(|| CostError::InvalidRead {
                    read: format!("{}:PoolKey.fee", inputs.source),
                    why: format!("not_a_static_fee:0x{:06x}", inputs.key.fee_raw),
                })?
        };
        if inputs.key.fee_raw & POOL_KEY_FEE_MASK != inputs.key.fee_raw {
            return Err(CostError::InvalidRead {
                read: format!("{}:PoolKey.fee", inputs.source),
                why: "fee_field_out_of_uint24".into(),
            });
        }

        let lane = if zero_for_one {
            (inputs.protocol_fee_packed & 0x00FF) as u32
        } else {
            ((inputs.protocol_fee_packed >> 8) & 0x00FF) as u32
        };
        if lane > PROTOCOL_FEE_LANE_MAX {
            return Err(CostError::InvalidRead {
                read: format!("{}:protocolFee", inputs.source),
                why: format!("lane_out_of_uint8:{lane}"),
            });
        }
        // Carril uint8 → pips, con el factor documentado de ProtocolFeeLibrary.
        let protocol_pips = lane * PROTOCOL_FEE_LANE_MULTIPLIER;
        if protocol_pips > MAX_PROTOCOL_FEE_PIPS {
            return Err(CostError::InvalidRead {
                read: format!("{}:protocolFee", inputs.source),
                why: format!(
                    "protocol_fee_above_max:{protocol_pips}pips>{}pips",
                    MAX_PROTOCOL_FEE_PIPS
                ),
            });
        }

        let hook_pips = if inputs.key.hooks == "0x0000000000000000000000000000000000000000" {
            // Sin hook desplegado no hay hook fee: cero por identidad del pool.
            0
        } else {
            inputs.hook_fee_pips.ok_or_else(|| CostError::MissingRead {
                provider: inputs.key.hooks.clone(),
                read: "IHooks.getHookFee(...)".into(),
            })?
        };
        if hook_pips > V4_FEE_DENOMINATOR {
            return Err(CostError::InvalidRead {
                read: format!("{}:getHookFee", inputs.source),
                why: format!("above_1e6:{hook_pips}"),
            });
        }

        if lp_pips + protocol_pips + hook_pips > V4_FEE_DENOMINATOR {
            return Err(CostError::InvalidRead {
                read: format!("{}:total_fee", inputs.source),
                why: format!(
                    "additive_fee_exceeds_1e6:{}",
                    lp_pips + protocol_pips + hook_pips
                ),
            });
        }

        Ok(Self {
            lp_pips,
            protocol_pips,
            hook_pips,
            dynamic_lp_fee: dynamic,
            source: inputs.source.clone(),
            evidence_id: inputs.evidence_id.clone(),
        })
    }

    /// Comisión TOTAL que paga el swapper (aditiva). Ésta es la diferencia
    /// contable con V3.
    pub fn total_pips(&self) -> u32 {
        self.lp_pips + self.protocol_pips + self.hook_pips
    }

    pub fn raw_for_pips(&self, amount_in: &U256, pips: u32) -> Result<U256, CostError> {
        super::proportion_floor(
            amount_in,
            &U256::from(pips),
            &U256::from(V4_FEE_DENOMINATOR),
            "v4:fee",
        )
    }
}

fn fee_component(
    ctx: &ComponentCtx<'_>,
    kind: &str,
    pips: u32,
    amount_in: &U256,
    payer: &str,
    beneficiary: &str,
    terms: &V4FeeTerms,
    note: String,
) -> Result<CostComponent, CostError> {
    let raw = terms.raw_for_pips(amount_in, pips)?;
    let (state, usd) = if pips == 0 {
        (CostState::ZeroAttested, Some(bigdecimal::BigDecimal::from(0)))
    } else {
        (CostState::Resolved, Some(ctx.value(&raw)?))
    };
    Ok(CostComponent {
        kind: kind.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: raw,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd,
        payer: payer.to_owned(),
        beneficiary: Some(beneficiary.to_owned()),
        source: terms.source.clone(),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("{}:{}", terms.evidence_id, kind),
        state,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(format!(
            "{note}; V4 aditivo: lp={} protocol={} hook={} de 1e6",
            terms.lp_pips, terms.protocol_pips, terms.hook_pips
        )),
    })
}

/// Comisión LP (estática o la dinámica efectiva) del swap V4.
pub fn lp_fee_component(
    ctx: &ComponentCtx<'_>,
    terms: &V4FeeTerms,
    amount_in: &U256,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let note = if terms.dynamic_lp_fee {
        "LP fee DINÁMICA leída del hook (el PoolKey sólo lleva la marca DYNAMIC_FEE_FLAG)"
    } else {
        "LP fee estática del PoolKey"
    };
    fee_component(
        ctx,
        KIND_V4_LP_FEE,
        terms.lp_pips,
        amount_in,
        payer,
        "lp_positions",
        terms,
        note.into(),
    )
}

/// Comisión protocolar V4. **ADITIVA**: se suma a la comisión LP, no la corta.
pub fn protocol_fee_component(
    ctx: &ComponentCtx<'_>,
    terms: &V4FeeTerms,
    amount_in: &U256,
    payer: &str,
) -> Result<CostComponent, CostError> {
    fee_component(
        ctx,
        KIND_V4_PROTOCOL_FEE,
        terms.protocol_pips,
        amount_in,
        payer,
        "protocol_treasury",
        terms,
        "protocolFee del carril del swap (aditivo en V4; NO es un corte de la comisión LP)".into(),
    )
}

/// Hook fee: cargo del hook sobre el swap. Cero acreditado si no hay hook o el
/// hook devuelve 0 con evidencia.
pub fn hook_fee_component(
    ctx: &ComponentCtx<'_>,
    terms: &V4FeeTerms,
    amount_in: &U256,
    payer: &str,
) -> Result<CostComponent, CostError> {
    fee_component(
        ctx,
        KIND_V4_HOOK_FEE,
        terms.hook_pips,
        amount_in,
        payer,
        "hook",
        terms,
        "getHookFee() del hook para ESTE swap".into(),
    )
}

/// Cambio de balance personalizado del hook a favor del swapper: flujo con
/// SIGNO. Se emite como `Rebate` para que `CostResolution` lo conserve en
/// `rebates` sin violar el contrato no negativo.
pub fn hook_rebate_component(
    ctx: &ComponentCtx<'_>,
    rebate_raw: &U256,
    source: &str,
    evidence_id: &str,
) -> Result<CostComponent, CostError> {
    let usd = ctx.value(rebate_raw)?;
    Ok(CostComponent {
        kind: KIND_V4_HOOK_REBATE.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: *rebate_raw,
        unit: Unit::MinUnits,
        direction: Direction::Rebate,
        usd: Some(usd),
        payer: ctx.scope.venue.clone(),
        beneficiary: Some("swapper".into()),
        source: source.to_owned(),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: evidence_id.to_owned(),
        state: CostState::Resolved,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some("BeforeSwapDelta a favor del swapper: flujo con signo, no un coste negativo".into()),
    })
}

/// Tarea de resolución para el contexto del hook (§9:
/// `dynamic_parameters_and_hook_state`). El hook cambia la ejecución: sin su
/// estado real no hay coste ni delta verificable.
pub fn hook_state_task(scope: &Scope, hooks: &str, why: &str) -> super::ResolutionTask {
    super::ResolutionTask {
        cost_kind: KIND_V4_HOOK_FEE.into(),
        scope: scope.clone(),
        receipt: Some("dynamic_parameters_and_hook_state".into()),
        provider: hooks.to_owned(),
        read: "IHooks.getHookFee / beforeSwap / afterSwap (state + return data)".into(),
        why: why.to_owned(),
    }
}

/// Activo de la pierna V4.
pub fn leg_asset(chain_id: u64, token: &str, decimals: u8) -> AssetRef {
    AssetRef::new(chain_id, token, decimals)
}

/// Ancla EVM de la lectura del `PoolManager`.
pub fn evm_anchor(chain_id: u64, block: u64) -> VenueAnchor {
    VenueAnchor::evm(chain_id, block, None)
}
