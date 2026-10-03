//! §5 «Curve, weighted, bins, PMM y otros» — **invariante y versión reales**,
//! escalado, tasas, parámetros dinámicos, comisión variable, oráculo y rama de
//! inventario cuando apliquen.
//!
//! ## Lo que NO se puede heredar de Uniswap
//!
//! En Curve la comisión se cobra sobre el **output**, no sobre el input:
//!
//! ```text
//! dy_gross = xp[j] - y - 1                 # salida bruta del invariante
//! fee      = fee_rate * dy_gross / 1e10    # ¡sobre la SALIDA!
//! dy_net   = dy_gross - fee
//! admin    = fee * admin_fee / 1e10        # parte del admin dentro de la comisión
//! ```
//!
//! Aplicar la base de Uniswap V2/V3 (comisión sobre el input) a un pool Curve
//! da un número distinto — y equivocado. [`fee_on_output`] implementa la base
//! real y queda probado contra la base de input.
//!
//! ## Lo que cambia entre versiones
//!
//! * `FEE_DENOMINATOR = 1e10` en todas las StableSwap (no 1e4 ni 1e6).
//! * **StableSwap clásico**: el fee del contrato se aplica tal cual.
//! * **StableSwap-NG**: `_fee()` escala `self.fee * N_COINS / (4 * (N_COINS - 1))`
//!   antes de usarlo. Con N=2 el factor es 1/2; con N=3 es 3/8. Ignorarlo
//!   duplica la comisión de un pool de 2 monedas.
//! * **CryptoSwap**: la tarifa es DINÁMICA (`mid_fee`/`out_fee`/`fee_gamma`)
//!   y depende del desequilibrio `K`; no existe una tarifa constante que leer.
//!
//! La versión se RESUELVE de las lecturas ([`CurveInvariant::detect`]); si no
//! alcanza, se emite la tarea `exact_invariant_version_and_rates` (§9) en vez de
//! asumir una rama.

use bigdecimal::BigDecimal;
use ethers::types::U256;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::{
    AssetRef, ComponentCtx, CostComponent, CostError, CostState, Direction, Scope, Treatment, Unit,
    VenueAnchor,
};

pub const ADAPTER: &str = "curve_pool_adapter/1";

pub const KIND_CURVE_LP_FEE: &str = "curve_lp_fee";
pub const KIND_CURVE_ADMIN_FEE: &str = "curve_admin_fee";

/// Denominador de comisión de TODAS las StableSwap/CryptoSwap de Curve.
pub const FEE_DENOMINATOR: u64 = 10_000_000_000; // 1e10
/// Precisión 1e18 del espacio `xp` y de los parámetros cripto.
pub const PRECISION_1E18: u64 = 1_000_000_000_000_000_000;

/// Versión real del invariante del pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveInvariant {
    /// StableSwap clásico: `self.fee` se aplica tal cual.
    StableSwapClassic { n_coins: u8 },
    /// StableSwap-NG: `_fee()` escala por `N/(4*(N-1))`.
    StableSwapNg { n_coins: u8 },
    /// CryptoSwap: comisión dinámica mid/out según `K`.
    CryptoSwap {
        n_coins: u8,
        mid_fee_raw: U256,
        out_fee_raw: U256,
        fee_gamma_raw: U256,
    },
    /// No resuelta: no se asume ninguna rama.
    Unknown,
}

impl CurveInvariant {
    /// Detecta la versión a partir de qué getters responden.
    ///
    /// * `mid_fee`/`out_fee`/`fee_gamma` presentes → CryptoSwap.
    /// * `offpeg_fee_multiplier` presente → StableSwap-NG.
    /// * sólo `fee`/`A` → StableSwap clásico.
    ///
    /// Cualquier combinación ambigua queda `Unknown` a propósito.
    pub fn detect(
        n_coins: Option<u8>,
        has_fee: bool,
        has_offpeg_multiplier: bool,
        crypto: Option<(U256, U256, U256)>,
    ) -> Self {
        match (n_coins, crypto) {
            (Some(n), Some((mid, out, gamma))) if n >= 2 => Self::CryptoSwap {
                n_coins: n,
                mid_fee_raw: mid,
                out_fee_raw: out,
                fee_gamma_raw: gamma,
            },
            (Some(n), None) if n >= 2 && has_offpeg_multiplier => Self::StableSwapNg { n_coins: n },
            (Some(n), None) if n >= 2 && has_fee => Self::StableSwapClassic { n_coins: n },
            _ => Self::Unknown,
        }
    }

    /// Escalado de `self.fee` que introduce StableSwap-NG.
    ///
    /// Devuelve `None` cuando la versión no está resuelta (no se adivina).
    pub fn fee_scaling_num_den(&self) -> Option<(u64, u64)> {
        match self {
            Self::StableSwapClassic { .. } => Some((1, 1)),
            Self::StableSwapNg { n_coins } => {
                let n = *n_coins as u64;
                if n < 2 {
                    return None;
                }
                Some((n, 4 * (n - 1)))
            }
            Self::CryptoSwap { .. } => None, // la tarifa se computa, no se escala
            Self::Unknown => None,
        }
    }

    /// Tarea de resolución cuando la versión no está resuelta.
    pub fn version_task(&self, scope: &Scope, pool: &str) -> Option<super::ResolutionTask> {
        if !matches!(self, Self::Unknown) {
            return None;
        }
        Some(super::ResolutionTask {
            cost_kind: KIND_CURVE_LP_FEE.into(),
            scope: scope.clone(),
            receipt: Some("exact_invariant_version_and_rates".into()),
            provider: pool.to_owned(),
            read: "pool.fee() / pool.A() / pool.offpeg_fee_multiplier() / pool.mid_fee() / pool.out_fee() / pool.fee_gamma() / pool.N_COINS()".into(),
            why: "curve_invariant_version_unresolved".into(),
        })
    }
}

/// Comisión sobre la SALIDA (base real de Curve).
///
/// `fee = floor(gross_output * fee_rate / 1e10)`. La comisión se cobra sobre lo
/// que sale del invariante, no sobre lo que entra.
pub fn fee_on_output(gross_output: &U256, fee_rate_raw: &U256) -> Result<U256, CostError> {
    super::proportion_floor(
        gross_output,
        fee_rate_raw,
        &U256::from(FEE_DENOMINATOR),
        "curve:fee_on_output",
    )
}

/// La base EQUIVOCADA (input), expuesta sólo para que un test pueda demostrar
/// que ambas difieren y que el motor no las confunde. NO usar en producción.
#[doc(hidden)]
pub fn fee_on_input_wrong_base(amount_in: &U256, fee_rate_raw: &U256) -> Result<U256, CostError> {
    super::proportion_floor(
        amount_in,
        fee_rate_raw,
        &U256::from(FEE_DENOMINATOR),
        "curve:fee_on_input",
    )
}

/// Aplica el escalado de versión a `self.fee` (StableSwap-NG).
pub fn scaled_fee_rate(
    invariant: &CurveInvariant,
    fee_raw: &U256,
) -> Result<U256, CostError> {
    let (num, den) = invariant
        .fee_scaling_num_den()
        .ok_or_else(|| CostError::UnsupportedInvariant {
            provider: "curve".into(),
            why: "fee_scaling_unresolved_for_version".into(),
        })?;
    if den == 0 {
        return Err(CostError::ZeroDenominator {
            denominator: "curve:fee_scaling".into(),
        });
    }
    super::proportion_floor(fee_raw, &U256::from(num), &U256::from(den), "curve:fee_scaling")
}

/// Reparto admin/LP dentro de la comisión ya cobrada.
///
/// `admin = floor(fee * admin_fee / 1e10)`; el LP recibe el resto. El total se
/// conserva exactamente (nunca se crea ni se pierde un mínimo).
pub fn admin_split(fee_raw: &U256, admin_fee_raw: &U256) -> Result<(U256, U256), CostError> {
    let admin = super::proportion_floor(
        fee_raw,
        admin_fee_raw,
        &U256::from(FEE_DENOMINATOR),
        "curve:admin_fee",
    )?;
    if admin > *fee_raw {
        return Err(CostError::InvalidRead {
            read: "curve:admin_fee".into(),
            why: "admin_share_exceeds_total_fee".into(),
        });
    }
    let lp = super::sub_checked(fee_raw, &admin, "curve:lp_fee")?;
    Ok((lp, admin))
}

/// Tarifa dinámica de un pool CryptoSwap para el desequilibrio `K` (1e18).
///
/// Forma del reductor de `CurveCryptoSwap._fee()`:
///
/// ```text
/// f   = fee_gamma * 1e18 / (fee_gamma + 1e18 - K)
/// fee = (mid_fee * f + out_fee * (1e18 - f)) / 1e18
/// ```
///
/// Propiedades verificables (y probadas): con `K = 1e18` (pool balanceado) el
/// resultado tiende a `mid_fee`; con `K = 0` (pool totalmente desequilibrado)
/// tiende a `out_fee`. `mid_fee <= out_fee` es la relación documentada.
pub fn cryptoswap_dynamic_fee(
    mid_fee_raw: &U256,
    out_fee_raw: &U256,
    fee_gamma_raw: &U256,
    k_1e18: &U256,
) -> Result<BigDecimal, CostError> {
    if fee_gamma_raw.is_zero() {
        return Err(CostError::ZeroDenominator {
            denominator: "curve:fee_gamma".into(),
        });
    }
    let gamma = bd(fee_gamma_raw);
    let k = bd(k_1e18);
    let one = BigDecimal::from(PRECISION_1E18);
    let denom = &gamma + &one - &k;
    if denom <= BigDecimal::from(0) {
        return Err(CostError::InvalidRead {
            read: "curve:cryptoswap_fee".into(),
            why: "fee_gamma_plus_one_minus_k_not_positive".into(),
        });
    }
    let f = &gamma * &one / denom; // 1e18-escalado
    let mid = bd(mid_fee_raw);
    let out = bd(out_fee_raw);
    Ok((&mid * &f + &out * (&one - &f)) / &one)
}

/// `A` efectivo cuando hay un ramp en curso (lineal entre los extremos).
///
/// Devuelve `None` si la ventana es degenerada (`t_future <= t_initial`): no se
/// extrapola.
pub fn ramped_a(
    initial_a: u64,
    future_a: u64,
    initial_a_time: u64,
    future_a_time: u64,
    now: u64,
) -> Option<u64> {
    if future_a_time <= initial_a_time {
        return None;
    }
    if now <= initial_a_time {
        return Some(initial_a);
    }
    if now >= future_a_time {
        return Some(future_a);
    }
    let span = future_a_time - initial_a_time;
    let elapsed = now - initial_a_time;
    let delta = future_a as i128 - initial_a as i128;
    Some((initial_a as i128 + delta * elapsed as i128 / span as i128) as u64)
}

/// Multiplicador de comisión off-peg (StableSwap-NG / pools con `offpeg`).
///
/// `fee_eff = floor(fee * multiplier / 1e10)`. El multiplicador se LEE; la
/// decisión de aplicarlo pertenece a la condición off-peg del propio pool, que
/// llega aquí ya resuelta (`is_off_peg`).
pub fn offpeg_fee_rate(
    fee_raw: &U256,
    offpeg_multiplier_raw: &U256,
    is_off_peg: bool,
) -> Result<U256, CostError> {
    if !is_off_peg || offpeg_multiplier_raw.is_zero() {
        return Ok(*fee_raw);
    }
    super::proportion_floor(
        fee_raw,
        offpeg_multiplier_raw,
        &U256::from(FEE_DENOMINATOR),
        "curve:offpeg_fee",
    )
}

/// Rama de oráculo/inventario de un pool Curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleBranch {
    /// Oráculo leído y dentro del umbral: la rama spot es utilizable.
    OnPeg,
    /// Oráculo leído y fuera del umbral: la comisión/pricing deben recalcularse
    /// con la rama off-peg.
    OffPeg,
    /// Sin lectura de oráculo: NO se asume on-peg.
    Unknown,
}

/// Determina la rama comparando el oráculo con el spot, en puntos base.
pub fn oracle_branch(
    price_oracle_1e18: Option<&U256>,
    spot_1e18: &U256,
    threshold_bps: u64,
) -> OracleBranch {
    let Some(oracle) = price_oracle_1e18 else {
        return OracleBranch::Unknown;
    };
    if spot_1e18.is_zero() {
        return OracleBranch::Unknown;
    }
    let diff = if oracle > spot_1e18 {
        *oracle - *spot_1e18
    } else {
        *spot_1e18 - *oracle
    };
    let scaled = diff.saturating_mul(U256::from(10_000u64)) / *spot_1e18;
    if scaled > U256::from(threshold_bps) {
        OracleBranch::OffPeg
    } else {
        OracleBranch::OnPeg
    }
}

/// Tarea de resolución del oráculo/rama de inventario (§9:
/// `oracle_round_and_inventory_branch`).
pub fn oracle_task(scope: &Scope, pool: &str) -> super::ResolutionTask {
    super::ResolutionTask {
        cost_kind: KIND_CURVE_LP_FEE.into(),
        scope: scope.clone(),
        receipt: Some("oracle_round_and_inventory_branch".into()),
        provider: pool.to_owned(),
        read: "pool.price_oracle() / pool.last_prices() / pool.price_scale()".into(),
        why: "oracle_branch_unresolved".into(),
    }
}

/// Comisión LP de Curve como componente embebido en el output cotizado.
pub fn lp_fee_component(
    ctx: &ComponentCtx<'_>,
    fee_raw: &U256,
    gross_output: &U256,
    payer: &str,
    pool: &str,
) -> Result<CostComponent, CostError> {
    let raw = fee_on_output(gross_output, fee_raw)?;
    let (state, usd) = if raw.is_zero() {
        (
            CostState::ZeroAttested,
            Some(BigDecimal::from(0)),
        )
    } else {
        (CostState::Resolved, Some(ctx.value(&raw)?))
    };
    Ok(CostComponent {
        kind: KIND_CURVE_LP_FEE.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: raw,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd,
        payer: payer.to_owned(),
        beneficiary: Some("lp_pool".into()),
        source: format!("{pool}:fee_on_output(fee={fee_raw}/1e10,admin_fee=split)"),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("{pool}:fee"),
        state,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(
            "comisión de Curve sobre la SALIDA del invariante; incluida en el quote, no se descuenta otra vez".into(),
        ),
    })
}

/// Parte del admin dentro de la comisión. `Embedded`: sale DE la comisión.
pub fn admin_fee_component(
    ctx: &ComponentCtx<'_>,
    admin_fee_raw: &U256,
    gross_output: &U256,
    fee_rate_raw: &U256,
    payer: &str,
    pool: &str,
) -> Result<CostComponent, CostError> {
    let fee = fee_on_output(gross_output, fee_rate_raw)?;
    let (_, admin) = admin_split(&fee, admin_fee_raw)?;
    let (state, usd) = if admin.is_zero() {
        (CostState::ZeroAttested, Some(BigDecimal::from(0)))
    } else {
        (CostState::Resolved, Some(ctx.value(&admin)?))
    };
    Ok(CostComponent {
        kind: KIND_CURVE_ADMIN_FEE.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: admin,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd,
        payer: payer.to_owned(),
        beneficiary: Some("curve_admin_treasury".into()),
        source: format!("{pool}:admin_fee"),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("{pool}:admin_fee:{admin_fee_raw}"),
        state,
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(
            "parte del admin DENTRO de la comisión ya cobrada; no es un cargo adicional".into(),
        ),
    })
}

fn bd(v: &U256) -> BigDecimal {
    BigDecimal::from_str(&v.to_string()).unwrap_or_else(|_| BigDecimal::from(0))
}

/// Activo de la pierna Curve (token de entrada/salida del swap).
pub fn leg_asset(chain_id: u64, token: &str, decimals: u8) -> AssetRef {
    AssetRef::new(chain_id, token, decimals)
}

/// Ancla EVM de la lectura del pool.
pub fn evm_anchor(chain_id: u64, block: u64) -> VenueAnchor {
    VenueAnchor::evm(chain_id, block, None)
}
