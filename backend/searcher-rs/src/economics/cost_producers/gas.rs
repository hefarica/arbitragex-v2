//! §5 «Gas» — estimación del payload REAL antes del envío y coste de receipt
//! DESPUÉS; base/priority, datos L1, componente L2, blobs y adicionales sólo
//! donde apliquen y **sin doble conteo**.
//!
//! ## Las dos fronteras temporales
//!
//! El prompt distingue dos medidas que no son la misma:
//!
//! * **antes del envío** — [`estimate_from_payload`]: gas intrínseco
//!   (21000 + coste de calldata + access list) + gas de ejecución, al precio
//!   efectivo del mercado de fees vigente. Es una ESTIMACIÓN.
//! * **después** — [`cost_from_receipt`]: `gasUsed × effectiveGasPrice` reales
//!   más el `l1Fee` que el receipt traiga. Es una MEDICIÓN.
//!
//! [`GasReconciliation`] conserva ambas y su varianza con signo: §5 exige
//! separar impacto determinista, tolerancia y **variación real quote↔ejecución**.
//!
//! ## Sin doble conteo
//!
//! * El **priority fee** ya está dentro del precio efectivo del gas
//!   (`min(maxFee, base+tip)`): NO es una línea aparte. La auditoría de
//!   [`super::CostResolution::double_count_audit`] lo detecta.
//! * **Blobs**: la comisión de blob (`blobBaseFee × blobGasUsed`, EIP-4844) es
//!   su propio término. Un componente L1 que ya la incluya debe declararlo en su
//!   `note` (`includes_blob_gas`) y entonces la línea de blob se elimina — la
//!   auditoría lo marca como doble conteo si conviven.
//! * **Datos L1 / componente L2**: son un ENUM, no flags acumulables. Es
//!   imposible sumar dos componentes L2 del mismo dominio por construcción.
//!
//! ## Coste intrínseco (EIP-2028 / EIP-2930)
//!
//! ```text
//! intrinsic = 21000 (+32000 si es creación)
//!           + 4 × bytes_cero + 16 × bytes_no_cero
//!           + 2400 × direcciones + 1900 × claves de access list
//! total     = intrinsic + execution_gas
//! ```
//!
//! Todo en `u64` con overflows controlados y saturación explícita al pasar a
//! `U256` (el coste económico vive en `U256`, jamás en `f64`).

use ethers::types::U256;
use serde::{Deserialize, Serialize};

use super::{
    AssetRef, ComponentCtx, CostComponent, CostError, CostState, Direction, Scope, Treatment, Unit,
    VenueAnchor,
};

pub const ADAPTER: &str = "evm_gas_adapter/1";

/// Categoría de bridge `gas`.
pub const KIND_GAS_EXECUTION: &str = "gas";
/// Coste de datos L1 de un rollup (OP-stack pre-Ecotone, o el estimado por el
/// nodo en Arbitrum Nitro).
pub const KIND_L2_DATA_FEE: &str = "l2_data_fee";
/// Comisión de blobs EIP-4844.
pub const KIND_BLOB_FEE: &str = "blob_fee";

pub const INTRINSIC_BASE: u64 = 21_000;
pub const TX_CREATE_EXTRA: u64 = 32_000;
pub const ACCESS_LIST_ADDRESS: u64 = 2_400;
pub const ACCESS_LIST_STORAGE_KEY: u64 = 1_900;
pub const CALLDATA_ZERO_BYTE: u64 = 4;
pub const CALLDATA_NONZERO_BYTE: u64 = 16;

/// Payload REAL del intent, tal como se va a enviar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    pub calldata: Vec<u8>,
    pub is_create: bool,
    /// `(dirección, claves)` de la access list EIP-2930.
    pub access_list: Vec<(String, Vec<U256>)>,
}

impl Payload {
    pub fn call(calldata: Vec<u8>) -> Self {
        Self {
            calldata,
            is_create: false,
            access_list: Vec::new(),
        }
    }
    pub fn calldata_bytes(&self) -> usize {
        self.calldata.len()
    }
    pub fn zero_bytes(&self) -> u64 {
        self.calldata.iter().filter(|b| **b == 0).count() as u64
    }
    pub fn nonzero_bytes(&self) -> u64 {
        self.calldata.iter().filter(|b| **b != 0).count() as u64
    }
    /// Coste de calldata (EIP-2028): 4 por byte cero, 16 por byte no cero.
    pub fn calldata_gas(&self) -> u64 {
        self.zero_bytes()
            .saturating_mul(CALLDATA_ZERO_BYTE)
            .saturating_add(self.nonzero_bytes().saturating_mul(CALLDATA_NONZERO_BYTE))
    }
    /// Direcciones y claves de la access list.
    pub fn access_list_gas(&self) -> u64 {
        let keys: u64 = self
            .access_list
            .iter()
            .map(|(_, k)| k.len() as u64)
            .fold(0u64, |a, b| a.saturating_add(b));
        (self.access_list.len() as u64)
            .saturating_mul(ACCESS_LIST_ADDRESS)
            .saturating_add(keys.saturating_mul(ACCESS_LIST_STORAGE_KEY))
    }
}

/// Gas intrínseco exacto del payload (incluye calldata y access list).
pub fn intrinsic_gas(payload: &Payload) -> Result<u64, CostError> {
    let mut total = INTRINSIC_BASE;
    if payload.is_create {
        total = total
            .checked_add(TX_CREATE_EXTRA)
            .ok_or_else(|| CostError::Overflow {
                op: "gas:tx_create".into(),
            })?;
    }
    total = total
        .checked_add(payload.calldata_gas())
        .ok_or_else(|| CostError::Overflow {
            op: "gas:calldata".into(),
        })?;
    total = total
        .checked_add(payload.access_list_gas())
        .ok_or_else(|| CostError::Overflow {
            op: "gas:access_list".into(),
        })?;
    Ok(total)
}

/// Total estimado ANTES del envío: intrínseco + ejecución.
pub fn estimate_from_payload(
    payload: &Payload,
    execution_gas: u64,
) -> Result<u64, CostError> {
    intrinsic_gas(payload)?
        .checked_add(execution_gas)
        .ok_or_else(|| CostError::Overflow {
            op: "gas:total".into(),
        })
}

/// Mercado de fees del bloque en que se va a incluir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeMarket {
    pub base_fee_per_gas: U256,
    /// EIP-1559: `maxPriorityFeePerGas`.
    pub max_priority_fee_per_gas: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    /// Transacción legacy (pre-1559) o `eth_gasPrice`.
    pub legacy_gas_price: Option<U256>,
}

/// Precio efectivo del gas. EIP-1559: `min(maxFee, base + tip)`.
///
/// El priority fee vive AQUÍ: quien lo emita como línea propia lo duplica.
pub fn effective_gas_price(market: &FeeMarket) -> Result<U256, CostError> {
    match (
        market.max_fee_per_gas,
        market.max_priority_fee_per_gas,
        market.legacy_gas_price,
    ) {
        (Some(max_fee), Some(tip), _) => {
            let wanted = market
                .base_fee_per_gas
                .checked_add(tip)
                .ok_or_else(|| CostError::Overflow {
                    op: "gas:base_plus_tip".into(),
                })?;
            Ok(if wanted < max_fee { wanted } else { max_fee })
        }
        (None, None, Some(legacy)) => Ok(legacy),
        // Modo sin datos suficientes: NO se asume un precio.
        _ => Err(CostError::MissingRead {
            provider: "rpc".into(),
            read: "eth_maxPriorityFeePerGas + eth_getBlockByNumber(baseFeePerGas) | eth_gasPrice"
                .into(),
        }),
    }
}

/// Componente L1/L2 de datos, mutuamente excluyente por construcción.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum L2DataFee {
    /// Cadena L1 (o rollup sin coste de datos separable): no aplica.
    NotApplicable { reason: String },
    /// OP-stack pre-Ecotone: `l1BaseFee × (txDataGas + overhead) / l1FeeScalar`.
    OpStackPreEcotone {
        l1_base_fee: U256,
        overhead: U256,
        scalar: U256,
    },
    /// OP-stack Ecotone: el coste ya mezcla blobs y calldata con dos escalares.
    /// Se modela como LECTURA del nodo (`GasPriceOracle.getL1Fee`) — no se
    /// reimplementa una fórmula versionada a mano.
    OpStackEcotoneRead {
        /// Resultado de `GasPriceOracle.getL1Fee(serializedTx)`.
        l1_fee_wei: U256,
        /// `true` cuando ese importe YA incluye el coste de blobs.
        includes_blob_gas: bool,
    },
    /// Arbitrum Nitro: `NodeInterface.gasEstimateL1Component`.
    ArbitrumRead { l1_component_wei: U256 },
}

impl L2DataFee {
    /// Importe en wei del componente de datos L1, o `None` si no aplica.
    pub fn wei(&self, tx_data_gas: &U256) -> Result<Option<U256>, CostError> {
        match self {
            Self::NotApplicable { .. } => Ok(None),
            Self::OpStackPreEcotone {
                l1_base_fee,
                overhead,
                scalar,
            } => {
                if scalar.is_zero() {
                    return Err(CostError::ZeroDenominator {
                        denominator: "gas:l1FeeScalar".into(),
                    });
                }
                let gas = tx_data_gas
                    .checked_add(*overhead)
                    .ok_or_else(|| CostError::Overflow {
                        op: "gas:l1_gas".into(),
                    })?;
                let fee = l1_base_fee
                    .checked_mul(gas)
                    .ok_or_else(|| CostError::Overflow {
                        op: "gas:l1_fee".into(),
                    })?;
                Ok(Some(fee / *scalar))
            }
            Self::OpStackEcotoneRead { l1_fee_wei, .. } => Ok(Some(*l1_fee_wei)),
            Self::ArbitrumRead { l1_component_wei } => Ok(Some(*l1_component_wei)),
        }
    }

    /// ¿El importe ya contiene la comisión de blobs?
    pub fn includes_blob_gas(&self) -> bool {
        match self {
            Self::OpStackEcotoneRead {
                includes_blob_gas, ..
            } => *includes_blob_gas,
            _ => false,
        }
    }

    pub fn note(&self) -> String {
        match self {
            Self::NotApplicable { reason } => format!("l1_data_fee_not_applicable:{reason}"),
            Self::OpStackPreEcotone { .. } => "op_stack_pre_ecotone".into(),
            Self::OpStackEcotoneRead {
                includes_blob_gas, ..
            } => {
                if *includes_blob_gas {
                    "l1_fee_read_from_node:includes_blob_gas".into()
                } else {
                    "l1_fee_read_from_node".into()
                }
            }
            Self::ArbitrumRead { .. } => "arbitrum_gasEstimateL1Component".into(),
        }
    }
}

/// Comisión de blobs EIP-4844: `blobBaseFee × blobGasUsed`.
pub fn blob_fee(blob_base_fee: &U256, blob_gas_used: &U256) -> Result<U256, CostError> {
    blob_base_fee
        .checked_mul(*blob_gas_used)
        .ok_or_else(|| CostError::Overflow {
            op: "gas:blob_fee".into(),
        })
}

/// Datos completos de gas de un intent para UN dominio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GasInputs {
    pub chain_id: u64,
    pub payload: Payload,
    pub execution_gas: u64,
    pub market: FeeMarket,
    pub l2_data_fee: L2DataFee,
    /// Gas de datos de la transacción serializada (`txDataGas`).
    pub tx_data_gas: U256,
    /// `(blobBaseFee, blobGasUsed)` cuando la transacción lleva blobs.
    pub blobs: Option<(U256, U256)>,
    pub source: String,
    pub evidence_id: String,
}

/// Coste de gas resuelto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasCost {
    pub gas_units: u64,
    pub effective_gas_price: U256,
    /// `gas_units × effective_gas_price` en wei.
    pub execution_wei: U256,
    pub l1_data_wei: Option<U256>,
    pub blob_wei: Option<U256>,
    pub total_wei: U256,
    pub measurement: Measurement,
}

/// De dónde viene la medida: estimación previa o receipt posterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measurement {
    /// Estimación del payload antes del envío.
    EstimatedBeforeSend,
    /// `gasUsed` real del receipt.
    ReceiptAfterExecution,
}

impl GasCost {
    pub fn total_wei_str(&self) -> String {
        self.total_wei.to_string()
    }
}

/// Resuelve el coste de gas desde el payload ANTES del envío.
///
/// Si el `L2DataFee` ya incluye blobs, la comisión de blob NO se suma otra vez:
/// el `total_wei` sale de una sola rama.
pub fn cost_from_payload(inputs: &GasInputs) -> Result<GasCost, CostError> {
    let gas_units = estimate_from_payload(&inputs.payload, inputs.execution_gas)?;
    cost_from_units(inputs, gas_units, Measurement::EstimatedBeforeSend)
}

/// Resuelve el coste de gas desde el `gasUsed` REAL del receipt.
pub fn cost_from_receipt(inputs: &GasInputs, gas_used: u64) -> Result<GasCost, CostError> {
    cost_from_units(inputs, gas_used, Measurement::ReceiptAfterExecution)
}

fn cost_from_units(
    inputs: &GasInputs,
    gas_units: u64,
    measurement: Measurement,
) -> Result<GasCost, CostError> {
    let price = effective_gas_price(&inputs.market)?;
    let execution_wei = price
        .checked_mul(U256::from(gas_units))
        .ok_or_else(|| CostError::Overflow {
            op: "gas:execution_cost".into(),
        })?;
    let l1_data_wei = inputs.l2_data_fee.wei(&inputs.tx_data_gas)?;
    let includes_blobs = inputs.l2_data_fee.includes_blob_gas();
    let blob_wei = match (inputs.blobs, includes_blobs) {
        (Some((base, used)), false) => Some(blob_fee(&base, &used)?),
        // Ya incluido en el componente L1: contarlo aquí sería doble conteo.
        _ => None,
    };
    let mut total = execution_wei;
    if let Some(l1) = l1_data_wei {
        total = total.checked_add(l1).ok_or_else(|| CostError::Overflow {
            op: "gas:total_with_l1".into(),
        })?;
    }
    if let Some(blob) = blob_wei {
        total = total.checked_add(blob).ok_or_else(|| CostError::Overflow {
            op: "gas:total_with_blob".into(),
        })?;
    }
    Ok(GasCost {
        gas_units,
        effective_gas_price: price,
        execution_wei,
        l1_data_wei,
        blob_wei,
        total_wei: total,
        measurement,
    })
}

/// Componente `gas` del contrato de costos. `External`: se resta UNA vez.
pub fn gas_component(
    ctx: &ComponentCtx<'_>,
    cost: &GasCost,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let usd = ctx.value(&cost.total_wei)?;
    Ok(CostComponent {
        kind: KIND_GAS_EXECUTION.into(),
        scope: ctx.scope.clone(),
        asset: AssetRef::native(ctx.chain_id, ctx.asset.decimals),
        amount_raw: cost.total_wei,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: Some(usd),
        payer: payer.to_owned(),
        beneficiary: Some("block_producer".into()),
        source: format!(
            "gas:{}units×{}wei_per_gas(+l1:{}+blob:{})",
            cost.gas_units,
            cost.effective_gas_price,
            cost.l1_data_wei
                .map(|v| v.to_string())
                .unwrap_or_else(|| "na".into()),
            cost.blob_wei
                .map(|v| v.to_string())
                .unwrap_or_else(|| "na".into())
        ),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("gas:{:?}", cost.measurement),
        state: if cost.total_wei.is_zero() {
            CostState::ZeroAttested
        } else {
            CostState::Resolved
        },
        treatment: Treatment::External,
        embedded_in_quote: false,
        note: Some(format!(
            "priority fee INCLUIDO en el precio efectivo {}; los datos L1/L2 y blobs van dentro de esta misma línea",
            cost.effective_gas_price
        )),
    })
}

/// Componente separado del coste de datos L1, sólo cuando el llamador necesita
/// verlo desglosado. Marcado `Embedded` respecto de `gas` para que la
/// auditoría de doble conteo lo vea y NADIE lo sume dos veces.
pub fn l2_data_component(
    ctx: &ComponentCtx<'_>,
    cost: &GasCost,
    includes_blob_gas: bool,
    payer: &str,
) -> Result<CostComponent, CostError> {
    let wei = cost.l1_data_wei.unwrap_or_else(U256::zero);
    let usd = ctx.value(&wei)?;
    Ok(CostComponent {
        kind: KIND_L2_DATA_FEE.into(),
        scope: ctx.scope.clone(),
        asset: AssetRef::native(ctx.chain_id, ctx.asset.decimals),
        amount_raw: wei,
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: Some(usd),
        payer: payer.to_owned(),
        beneficiary: Some("l1_data_availability".into()),
        source: "gas:l1_data_component".into(),
        adapter_version: ADAPTER.into(),
        anchor: Some(ctx.anchor.clone()),
        evidence_id: format!("gas:l1_data:{includes_blob_gas}"),
        state: if wei.is_zero() {
            CostState::ZeroAttested
        } else {
            CostState::Resolved
        },
        treatment: Treatment::Embedded,
        embedded_in_quote: true,
        note: Some(if includes_blob_gas {
            "desglose de la línea `gas` (includes_blob_gas)".into()
        } else {
            "desglose de la línea `gas`".into()
        }),
    })
}

/// Componente de comisión de blobs, sólo cuando NO está dentro del L1 fee.
pub fn blob_component(
    ctx: &ComponentCtx<'_>,
    cost: &GasCost,
    payer: &str,
) -> Option<Result<CostComponent, CostError>> {
    let wei = cost.blob_wei?;
    let build = || -> Result<CostComponent, CostError> {
        let usd = ctx.value(&wei)?;
        Ok(CostComponent {
            kind: KIND_BLOB_FEE.into(),
            scope: ctx.scope.clone(),
            asset: AssetRef::native(ctx.chain_id, ctx.asset.decimals),
            amount_raw: wei,
            unit: Unit::MinUnits,
            direction: Direction::Charge,
            usd: Some(usd),
            payer: payer.to_owned(),
            beneficiary: Some("blob_space".into()),
            source: "gas:blobBaseFee×blobGasUsed".into(),
            adapter_version: ADAPTER.into(),
            anchor: Some(ctx.anchor.clone()),
            evidence_id: "gas:blob_fee".into(),
            state: CostState::Resolved,
            treatment: Treatment::Embedded,
            embedded_in_quote: true,
            note: Some("EIP-4844; ya incluido en la línea `gas`".into()),
        })
    };
    Some(build())
}

/// Diferencia con signo exacta, sin `f64` y sin desbordar `i128`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedWei {
    pub negative: bool,
    pub abs: U256,
}

impl SignedWei {
    pub fn between(actual: &U256, estimated: &U256) -> Self {
        if actual >= estimated {
            Self {
                negative: false,
                abs: *actual - *estimated,
            }
        } else {
            Self {
                negative: true,
                abs: *estimated - *actual,
            }
        }
    }
    pub fn is_zero(&self) -> bool {
        self.abs.is_zero()
    }
    pub fn describe(&self) -> String {
        format!(
            "{}{}",
            if self.negative { "-" } else { "+" },
            self.abs
        )
    }
}

/// Conciliación estimación ↔ receipt (§5: «variación real entre quote y
/// ejecución»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasReconciliation {
    pub estimated_wei: U256,
    pub actual_wei: Option<U256>,
    /// `actual - estimated` con signo; `None` mientras no haya receipt.
    pub variance: Option<SignedWei>,
    pub estimated_gas_units: u64,
    pub actual_gas_units: Option<u64>,
}

impl GasReconciliation {
    pub fn new(estimated: &GasCost, actual: Option<&GasCost>) -> Self {
        Self {
            estimated_wei: estimated.total_wei,
            actual_wei: actual.map(|a| a.total_wei),
            variance: actual.map(|a| SignedWei::between(&a.total_wei, &estimated.total_wei)),
            estimated_gas_units: estimated.gas_units,
            actual_gas_units: actual.map(|a| a.gas_units),
        }
    }
}

/// Ancla EVM de la lectura del mercado de fees.
pub fn evm_anchor(chain_id: u64, block: u64) -> VenueAnchor {
    VenueAnchor::evm(chain_id, block, None)
}

/// Activo nativo del dominio.
pub fn native_asset(chain_id: u64, decimals: u8) -> AssetRef {
    AssetRef::native(chain_id, decimals)
}

/// Scope de ruta para el gas (paga la ruta entera, no una pierna).
pub fn route_scope(chain_id: u64) -> Scope {
    Scope::route(&format!("evm:{chain_id}"))
}
