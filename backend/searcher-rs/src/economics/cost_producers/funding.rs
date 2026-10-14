//! §5 «Financiación» — proveedor, método, activo, capacidad disponible,
//! principal, premium y repayment exactos; comparar capital propio y
//! proveedores admisibles.
//!
//! ## El caso Aave que el prompt nombra
//!
//! > «para un adapter Aave EVM compatible, resuelve `FLASHLOAN_PREMIUM_TOTAL` y
//! > la semántica del método elegido desde la fuente autoritativa — **prohibido
//! > copiar 5 o 9 bps de documentos o fixtures**»
//!
//! `crate::financing::AAVE_FLASH_LOAN_FEE_BPS` es hoy un literal de `f64` con el
//! valor de un día concreto; este módulo no lo usa ni lo duplica. El premium
//! sale de la lectura del pool:
//!
//! ```text
//! Pool.FLASHLOAN_PREMIUM_TOTAL()      -> uint128, basis points (denominador 1e4)
//! Pool.FLASHLOAN_PREMIUM_TO_PROTOCOL()-> uint128, basis points (parte del premium)
//! premium   = floor(principal * premium_total_bps / 10_000)
//! repayment = principal + premium
//! ```
//!
//! Sin esa lectura el componente queda `PendingResolution` con la tarea exacta
//! [`ProviderReads::premium_read`] — **jamás** con un valor por defecto. Un cero
//! sólo entra como `ZeroAttested` si el propio venue lo acredita.
//!
//! ## Semántica del método (no intercambiable)
//!
//! | método | callback | base del premium |
//! |---|---|---|
//! | `AaveV3FlashLoan` | `executeOperation(assets, amounts, premiums, initiator, params)` | `FLASHLOAN_PREMIUM_TOTAL()` |
//! | `AaveV3FlashLoanSimple` | `executeOperation(asset, amount, premium, initiator, params)` | `FLASHLOAN_PREMIUM_TOTAL()` |
//! | `Erc3156FlashLoan` | `onFlashLoan(initiator, token, amount, fee, data)` | `IERC3156FlashLender.flashFee(token, amount)` |
//! | `BalancerV2FlashLoan` | `receiveFlashLoan(tokens, amounts, feeAmounts, userData)` | declaración del vault |
//! | `UniswapV2FlashSwap` | `uniswapV2Call` | tarifa de swap del pool (ver [`super::univ2`]) |
//!
//! El callback es parte de la semántica: `flashLoan` y `flashLoanSimple` NO son
//! intercambiables aunque compartan el premium.

use bigdecimal::BigDecimal;
use ethers::types::U256;
use serde::{Deserialize, Serialize};

use super::{
    AssetRef, ComponentCtx, CostComponent, CostError, CostState, Direction, Scope, Treatment, Unit,
    VenueAnchor,
};

pub const ADAPTER: &str = "funding_provider_adapter/1";

/// Categoría de bridge `financing`.
pub const KIND_FINANCING_PREMIUM: &str = "financing_premium";

/// Denominador de basis points usado por Aave (`PercentageMath.PERCENTAGE_FACTOR`).
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Método de financiación. La semántica NO es intercambiable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingMethod {
    /// Capital propio: sin proveedor y sin premium.
    OwnCapital,
    /// Aave V3 `flashLoan` (multi-activo).
    AaveV3FlashLoan,
    /// Aave V3 `flashLoanSimple` (un activo).
    AaveV3FlashLoanSimple,
    /// Balancer V2 `Vault.flashLoan`.
    BalancerV2FlashLoan,
    /// Flash swap de un par V2.
    UniswapV2FlashSwap,
    /// ERC-3156 `IERC3156FlashLender`.
    Erc3156FlashLoan,
}

impl FundingMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OwnCapital => "own_capital",
            Self::AaveV3FlashLoan => "aave_v3_flash_loan",
            Self::AaveV3FlashLoanSimple => "aave_v3_flash_loan_simple",
            Self::BalancerV2FlashLoan => "balancer_v2_flash_loan",
            Self::UniswapV2FlashSwap => "uniswap_v2_flash_swap",
            Self::Erc3156FlashLoan => "erc3156_flash_loan",
        }
    }
    /// Callback EXACTO que el ejecutor debe implementar para este método.
    pub fn callback(&self) -> &'static str {
        match self {
            Self::OwnCapital => "none",
            Self::AaveV3FlashLoan => {
                "executeOperation(address[] assets,uint256[] amounts,uint256[] premiums,address initiator,bytes params)"
            }
            Self::AaveV3FlashLoanSimple => {
                "executeOperation(address asset,uint256 amount,uint256 premium,address initiator,bytes params)"
            }
            Self::BalancerV2FlashLoan => {
                "receiveFlashLoan(address[] tokens,uint256[] amounts,uint256[] feeAmounts,bytes userData)"
            }
            Self::UniswapV2FlashSwap => "uniswapV2Call(address sender,uint256 amount0,uint256 amount1,bytes data)",
            Self::Erc3156FlashLoan => {
                "onFlashLoan(address initiator,address token,uint256 amount,uint256 fee,bytes data)"
            }
        }
    }
    /// ¿Este método cobra un premium que hay que resolver de la fuente?
    pub fn charges_premium(&self) -> bool {
        !matches!(self, Self::OwnCapital)
    }
    /// ¿El premium se expresa en basis points del principal?
    pub fn premium_in_bps(&self) -> bool {
        matches!(self, Self::AaveV3FlashLoan | Self::AaveV3FlashLoanSimple)
    }
}

/// Lecturas autoritativas del proveedor de financiación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderReads {
    pub provider: String,
    pub pool: String,
    pub chain_id: u64,
    pub method: FundingMethod,
    /// `Pool.FLASHLOAN_PREMIUM_TOTAL()` (BPS) o el fee del lender ERC-3156 ya
    /// normalizado a BPS. `None` = sin lectura → pendiente.
    pub premium_total_bps: Option<U256>,
    /// `Pool.FLASHLOAN_PREMIUM_TO_PROTOCOL()` (BPS dentro del premium).
    pub premium_to_protocol_bps: Option<U256>,
    /// Capacidad realmente disponible del activo en el proveedor.
    pub capacity_raw: Option<U256>,
    pub source: String,
    pub evidence_id: String,
}

impl ProviderReads {
    /// Lectura EXACTA que falta para resolver el premium. Es lo que viaja en la
    /// tarea de resolución.
    pub fn premium_read(&self) -> Option<String> {
        match self.method {
            FundingMethod::OwnCapital => None,
            FundingMethod::AaveV3FlashLoan | FundingMethod::AaveV3FlashLoanSimple => {
                Some(format!("{}.FLASHLOAN_PREMIUM_TOTAL()", self.pool))
            }
            FundingMethod::Erc3156FlashLoan => {
                Some(format!("{}.flashFee(token,amount)", self.pool))
            }
            FundingMethod::BalancerV2FlashLoan => {
                Some(format!("{}.flashLoan fee declaration (Vault)", self.pool))
            }
            FundingMethod::UniswapV2FlashSwap => {
                Some(format!("{}.swap fee terms (pair)", self.pool))
            }
        }
    }
    /// ¿El venue acredita explícitamente un premium CERO?
    ///
    /// Un cero sólo es admisible con lectura presente y evidencia citada.
    pub fn zero_attested(&self) -> bool {
        self.premium_total_bps == Some(U256::zero())
            && !self.evidence_id.trim().is_empty()
            && !self.source.trim().is_empty()
    }
}

/// Cotización de financiación resuelta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundingQuote {
    pub method: FundingMethod,
    pub provider: String,
    pub pool: String,
    pub asset: AssetRef,
    pub principal_raw: U256,
    pub premium_raw: U256,
    pub capacity_raw: Option<U256>,
    /// `principal + premium` — lo que hay que devolver exactamente.
    pub repayment_raw: U256,
    /// Parte del premium que va al protocolo (no al LP).
    pub protocol_share_raw: Option<U256>,
    pub premium_bps: Option<U256>,
    pub zero_attested: bool,
    pub source: String,
    pub evidence_id: String,
}

/// `premium = floor(principal × premium_total_bps / 10_000)`.
pub fn aave_premium_raw(principal: &U256, premium_total_bps: &U256) -> Result<U256, CostError> {
    super::proportion_floor(
        principal,
        premium_total_bps,
        &U256::from(BPS_DENOMINATOR),
        "funding:aave_premium",
    )
}

/// Parte del premium que retiene el protocolo (dentro del premium ya cobrado).
pub fn aave_protocol_share_raw(
    premium_raw: &U256,
    premium_to_protocol_bps: &U256,
) -> Result<U256, CostError> {
    let share = super::proportion_floor(
        premium_raw,
        premium_to_protocol_bps,
        &U256::from(BPS_DENOMINATOR),
        "funding:protocol_share",
    )?;
    if share > *premium_raw {
        return Err(CostError::InvalidRead {
            read: "funding:premium_to_protocol".into(),
            why: "protocol_share_exceeds_total_premium".into(),
        });
    }
    Ok(share)
}

/// Resuelve la financiación desde las lecturas autoritativas.
///
/// Errores posibles (todos generan tarea, ninguno produce un número inventado):
/// * `MissingRead` — falta `FLASHLOAN_PREMIUM_TOTAL()`/`flashFee()`/declaración.
/// * `InsufficientCapacity`-equivalente: `capacity < principal` → `InvalidRead`
///   con la razón `capacity_insufficient` (la ruta no es financiable así).
pub fn resolve(
    reads: &ProviderReads,
    asset: &AssetRef,
    principal: &U256,
) -> Result<FundingQuote, CostError> {
    if reads.source.trim().is_empty() || reads.evidence_id.trim().is_empty() {
        return Err(CostError::InvalidRead {
            read: format!("funding:{}", reads.provider),
            why: "provider_reads_without_provenance".into(),
        });
    }
    if reads.chain_id != asset.chain_id {
        return Err(CostError::InvalidRead {
            read: format!("funding:{}", reads.provider),
            why: "provider_chain_mismatch".into(),
        });
    }
    if principal.is_zero() {
        return Err(CostError::InvalidRead {
            read: "funding:principal".into(),
            why: "zero_principal".into(),
        });
    }

    if reads.method == FundingMethod::OwnCapital {
        return Ok(FundingQuote {
            method: reads.method,
            provider: reads.provider.clone(),
            pool: reads.pool.clone(),
            asset: asset.clone(),
            principal_raw: *principal,
            premium_raw: U256::zero(),
            capacity_raw: reads.capacity_raw,
            repayment_raw: *principal,
            protocol_share_raw: Some(U256::zero()),
            premium_bps: Some(U256::zero()),
            // Capital propio: el cero es estructural (no hay proveedor al que
            // pagar), no una tarifa leída.
            zero_attested: false,
            source: reads.source.clone(),
            evidence_id: reads.evidence_id.clone(),
        });
    }

    let Some(capacity) = reads.capacity_raw else {
        return Err(CostError::MissingRead {
            provider: reads.provider.clone(),
            read: format!("{}:availableLiquidity/asset capacity", reads.pool),
        });
    };
    if capacity < *principal {
        return Err(CostError::InvalidRead {
            read: format!("funding:{}:capacity", reads.provider),
            why: format!("capacity_insufficient:{capacity}<{principal}"),
        });
    }

    let Some(bps) = reads.premium_total_bps else {
        return Err(CostError::MissingRead {
            provider: reads.provider.clone(),
            read: reads
                .premium_read()
                .unwrap_or_else(|| "premium_read_unknown".into()),
        });
    };
    if bps > U256::from(BPS_DENOMINATOR) {
        return Err(CostError::InvalidRead {
            read: format!("funding:{}:premium", reads.provider),
            why: format!("premium_bps_above_10000:{bps}"),
        });
    }

    let premium_raw = if reads.method.premium_in_bps() {
        aave_premium_raw(principal, &bps)?
    } else if bps.is_zero() && reads.zero_attested() {
        // Métodos cuyo fee NO se expresa en bps (p. ej. un vault que por diseño
        // no cobra): sólo admisible cuando el venue lo ACREDITA explícitamente
        // (lectura = 0 + fuente + evidencia). Nunca se asume por el método.
        U256::zero()
    } else {
        // Para métodos cuyo fee no se expresa en BPS la lectura debe llegar ya
        // normalizada; se rechaza en vez de reinterpretarla.
        return Err(CostError::UnsupportedInvariant {
            provider: reads.provider.clone(),
            why: format!(
                "method_{}_premium_not_in_bps: supply the venue's own fee read",
                reads.method.as_str()
            ),
        });
    };
    let protocol_share_raw = match reads.premium_to_protocol_bps {
        Some(p) => Some(aave_protocol_share_raw(&premium_raw, &p)?),
        None => None,
    };
    let repayment_raw = super::add_checked(principal, &premium_raw, "funding:repayment")?;

    Ok(FundingQuote {
        method: reads.method,
        provider: reads.provider.clone(),
        pool: reads.pool.clone(),
        asset: asset.clone(),
        principal_raw: *principal,
        premium_raw,
        capacity_raw: Some(capacity),
        repayment_raw,
        protocol_share_raw,
        premium_bps: Some(bps),
        zero_attested: reads.zero_attested(),
        source: reads.source.clone(),
        evidence_id: reads.evidence_id.clone(),
    })
}

/// Componente `financing` del contrato, con principal, premium y repayment.
pub fn financing_component(
    ctx: &ComponentCtx<'_>,
    quote: &FundingQuote,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let usd = ctx.value(&quote.premium_raw)?;
    let (state, usd) = if quote.premium_raw.is_zero() {
        if quote.method == FundingMethod::OwnCapital {
            (CostState::NotApplicable, None::<BigDecimal>)
        } else if quote.zero_attested {
            (CostState::ZeroAttested, Some(BigDecimal::from(0)))
        } else {
            // Una financiación flash sin lectura acreditada de cero NO es cero.
            return Err(CostError::MissingRead {
                provider: quote.provider.clone(),
                read: format!("{}.FLASHLOAN_PREMIUM_TOTAL()", quote.pool),
            });
        }
    } else {
        (CostState::Resolved, Some(usd))
    };
    Ok(CostComponent {
        kind: KIND_FINANCING_PREMIUM.into(),
        scope: ctx.scope.clone(),
        asset: ctx.asset.clone(),
        amount_raw: quote.premium_raw,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd,
        payer: payer.to_owned(),
        beneficiary: Some(quote.provider.clone()),
        source: format!(
            "{}.{} [{}; principal={}; repayment={}]",
            quote.pool,
            quote.method.as_str(),
            quote.source,
            quote.principal_raw,
            quote.repayment_raw
        ),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: quote.evidence_id.clone(),
        state,
        treatment: if state == CostState::NotApplicable {
            Treatment::NotApplicable
        } else {
            Treatment::External
        },
        embedded_in_quote: false,
        note: Some(match state {
            CostState::NotApplicable => format!(
                "capital propio: sin proveedor y sin premium; repayment = principal ({})",
                quote.repayment_raw
            ),
            CostState::ZeroAttested => format!(
                "el venue acredita premium exactamente cero ({} bps leídos de {})",
                quote
                    .premium_bps
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "0".into()),
                quote.pool
            ),
            _ => format!(
                "premium {} bps LEÍDO de {}; repayment exacto = principal + premium = {} (el principal NO es ganancia)",
                quote
                    .premium_bps
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "?".into()),
                quote.pool,
                quote.repayment_raw
            ),
        }),
    })
}

/// Candidato de financiación comparable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundingCandidate {
    pub method: FundingMethod,
    pub provider: String,
    pub principal_raw: U256,
    pub premium_raw: U256,
    pub capacity_raw: Option<U256>,
    pub admissible: bool,
    pub reason: Option<String>,
}

/// Compara capital propio contra proveedores admisibles (§5).
///
/// NO fabrica una alternativa "flash más barata": devuelve lo que cada uno
/// cuesta realmente, con los no admisibles marcados y su razón.
pub fn candidates(quotes: &[FundingQuote], own_capital_raw: &U256) -> Vec<FundingCandidate> {
    let mut out: Vec<FundingCandidate> = quotes
        .iter()
        .map(|q| {
            let admissible = q
                .capacity_raw
                .map(|c| c >= q.principal_raw)
                .unwrap_or(false);
            FundingCandidate {
                method: q.method,
                provider: q.provider.clone(),
                principal_raw: q.principal_raw,
                premium_raw: q.premium_raw,
                capacity_raw: q.capacity_raw,
                admissible,
                reason: if admissible {
                    None
                } else {
                    Some("capacity_insufficient".into())
                },
            }
        })
        .collect();
    out.push(FundingCandidate {
        method: FundingMethod::OwnCapital,
        provider: "own_capital".into(),
        principal_raw: *own_capital_raw,
        premium_raw: U256::zero(),
        capacity_raw: Some(*own_capital_raw),
        admissible: *own_capital_raw >= quotes.first().map(|q| q.principal_raw).unwrap_or_default(),
        reason: if *own_capital_raw >= quotes.first().map(|q| q.principal_raw).unwrap_or_default() {
            None
        } else {
            Some("own_capital_insufficient".into())
        },
    });
    out
}

/// Candidato de menor premium entre los ADMISIBLES. `None` si ninguno lo es —
/// no se elige un candidato infactible para tener un ganador.
pub fn cheapest_admissible(cands: &[FundingCandidate]) -> Option<&FundingCandidate> {
    cands
        .iter()
        .filter(|c| c.admissible)
        .min_by(|a, b| a.premium_raw.cmp(&b.premium_raw))
}

/// Tarea de resolución del premium cuando la lectura falta.
pub fn premium_task(scope: &Scope, reads: &ProviderReads) -> super::ResolutionTask {
    super::ResolutionTask {
        cost_kind: KIND_FINANCING_PREMIUM.into(),
        scope: scope.clone(),
        receipt: None,
        provider: reads.provider.clone(),
        read: reads
            .premium_read()
            .unwrap_or_else(|| format!("{}.premium_read()", reads.pool)),
        why: "flashloan_premium_not_resolved_from_authoritative_source".into(),
    }
}

/// Ancla EVM de la lectura del pool del proveedor.
pub fn evm_anchor(chain_id: u64, block: u64) -> VenueAnchor {
    VenueAnchor::evm(chain_id, block, None)
}
