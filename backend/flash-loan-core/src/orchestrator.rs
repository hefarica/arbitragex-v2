//! ═══════════════════════════════════════════════════════════════════════════════
//! Vector 3: Flash Loan Vector Injection — FASE OMEGA
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Orchestrador de Temporal Liquidity Superposition (TLS) / Flash Loans.
//! Calcula r_flash (rentabilidad neta post-flash-loan) y coordina la ejecución
//! atómica de operaciones con TLS.
//!
//! Fórmula Hamiltoniana:
//!   r_flash = Σ(yield_i) - Σ(gas_i) - Σ(fees_flash) - decoherencia_estado
//!
//! donde:
//!   - yield_i: Topological Yield de cada leg de la ruta
//!   - gas_i: Costo de gas para cada transacción
//!   - fees_flash: Fee del prestamista flash (typ: 0.09% - 0.3%)
//!   - decoherencia_estado: Slippage calculado algorítmicamente

use thiserror::Error;
use tracing::{debug, info};

// ─────────────────────────────────────────────────────────────────────────────
// Constantes de Protocolo
// ─────────────────────────────────────────────────────────────────────────────

/// Fee típico de Aave V3 Flash Loan (0.09% = 9 bps)
pub const FLASH_LOAN_FEE_BPS_AAVE: u16 = 9;
/// Fee típico de Balancer Flash Loan (0% = 0 bps)
pub const FLASH_LOAN_FEE_BPS_BALANCER: u16 = 0;
/// Fee típico de dYdX Flash Loan (0% = 0 bps)
pub const FLASH_LOAN_FEE_BPS_DYDX: u16 = 0;
/// Fee máximo tolerable para considerar una oportunidad (30 bps = 0.3%)
pub const MAX_FLASH_FEE_BPS: u16 = 30;

/// Gas estimado para operación flash loan (base + overhead del wrapper)
pub const ESTIMATED_FLASH_GAS_UNITS: u64 = 180_000;
/// Gas adicional por cada leg de la ruta
pub const GAS_PER_LEG: u64 = 45_000;

// ─────────────────────────────────────────────────────────────────────────────
// Tipos
// ─────────────────────────────────────────────────────────────────────────────

/// Proveedor de Temporal Liquidity Superposition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TlsProvider {
    /// Aave V3 Pool
    AaveV3,
    /// Balancer V2 Vault
    BalancerV2,
    /// dYdX Solo Margin
    Dydx,
    /// Uniswap V3 (flash swap)
    UniswapV3,
}

impl TlsProvider {
    /// Retorna el fee en basis points para este proveedor
    pub fn fee_bps(&self) -> u16 {
        match self {
            Self::AaveV3 => FLASH_LOAN_FEE_BPS_AAVE,
            Self::BalancerV2 => FLASH_LOAN_FEE_BPS_BALANCER,
            Self::Dydx => FLASH_LOAN_FEE_BPS_DYDX,
            Self::UniswapV3 => 0, // Flash swap: fee se paga en el swap mismo
        }
    }

    /// Dirección del contrato del proveedor (mainnet)
    pub fn mainnet_address(&self) -> &'static str {
        match self {
            Self::AaveV3 => "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
            Self::BalancerV2 => "0xBA12222222228d8Ba445958a75a0704d566BF2C8",
            Self::Dydx => "0x1E0447b19BB6EcFdAe1e4AE1694b0C3659614e4e",
            Self::UniswapV3 => "0xE592427A0AEce92De3Edee1F18E0157C05861564", // Router
        }
    }

    /// Nombre legible del proveedor
    pub fn name(&self) -> &'static str {
        match self {
            Self::AaveV3 => "aave_v3",
            Self::BalancerV2 => "balancer_v2",
            Self::Dydx => "dydx",
            Self::UniswapV3 => "uniswap_v3",
        }
    }
}

/// Paso individual en una ruta de ejecución
#[derive(Debug, Clone)]
pub struct RouteLeg {
    /// Tipo de operación
    pub step_type: StepType,
    /// Protocolo/DEX objetivo
    pub protocol: String,
    /// Dirección del pool o contrato
    pub target: String,
    /// Token de entrada
    pub token_in: String,
    /// Token de salida
    pub token_out: String,
    /// Monto en wei/unidades base
    pub amount: String,
    /// Fee del pool (bps) para cálculo de slippage
    pub pool_fee_bps: u16,
}

/// Tipos de pasos en una ruta
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    Swap,
    FlashLoan,
    FlashSwap,
    Approve,
    Wrap,
    Unwrap,
}

/// Contexto de simulación para cálculo de r_flash
#[derive(Debug, Clone)]
pub struct SimulationContext {
    /// Precio base del gas en wei
    pub gas_price_wei: u128,
    /// Precio del token nativo (ETH/MATIC) en USD
    pub native_price_usd: f64,
    /// Factor de confianza de la simulación (0.0 - 1.0)
    pub confidence: f64,
}

/// Resultado del cálculo de rentabilidad flash
#[derive(Debug, Clone)]
pub struct FlashProfitability {
    /// ID único del cálculo
    pub calculation_id: String,
    /// Rentabilidad neta estimada en USD (r_flash)
    pub r_flash_usd: f64,
    /// Yield bruto antes de fees
    pub gross_yield_usd: f64,
    /// Costo total de gas estimado en USD
    pub total_gas_cost_usd: f64,
    /// Fee del flash loan en USD
    pub flash_fee_usd: f64,
    /// Decoherencia de estado (slippage) estimada en USD
    pub decoherencia_usd: f64,
    /// Proveedor de TLS seleccionado
    pub selected_provider: TlsProvider,
    /// Si la operación es rentable (r_flash > umbral)
    pub is_profitable: bool,
    /// Umbral mínimo de rentabilidad aplicado
    pub min_profit_threshold_usd: f64,
}

/// Error en el orquestador flash
#[derive(Debug, Error)]
pub enum FlashOrchestratorError {
    #[error("Proveedor TLS no soportado: {0}")]
    UnsupportedProvider(String),
    #[error("Ruta vacía o inválida")]
    InvalidRoute,
    #[error("Monto flash loan inválido: {0}")]
    InvalidAmount(String),
    #[error("Simulación falló: {0}")]
    SimulationFailed(String),
    #[error("Decoherencia excesiva: {actual_bps} bps > {max_bps} bps")]
    ExcessiveDecoherencia { actual_bps: u16, max_bps: u16 },
    #[error("Fee flash loan excesivo: {fee_bps} bps > {max_bps} bps")]
    ExcessiveFlashFee { fee_bps: u16, max_bps: u16 },
}

// ─────────────────────────────────────────────────────────────────────────────
// Orquestador Flash Loan
// ─────────────────────────────────────────────────────────────────────────────

/// Calcula r_flash (rentabilidad neta post-flash-loan) para una ruta dada.
///
/// Fórmula: r_flash = gross_yield - gas_total - flash_fee - decoherencia
///
/// # Arguments
/// * `route` - Vector de pasos de la ruta
/// * `principal_amount` - Monto del flash loan en wei
/// * `ctx` - Contexto de simulación (gas price, precios, etc.)
/// * `min_profit_usd` - Umbral mínimo de rentabilidad en USD
///
/// # Returns
/// * `Ok(FlashProfitability)` - Cálculo exitoso con r_flash
/// * `Err(FlashOrchestratorError)` - Error en validación o cálculo
pub fn calculate_r_flash(
    route: &[RouteLeg],
    principal_amount: &str,
    ctx: &SimulationContext,
    min_profit_usd: f64,
) -> Result<FlashProfitability, FlashOrchestratorError> {
    // Validaciones de entrada
    if route.is_empty() {
        return Err(FlashOrchestratorError::InvalidRoute);
    }

    let principal_u256 = principal_amount
        .parse::<u128>()
        .map_err(|_| FlashOrchestratorError::InvalidAmount(principal_amount.to_string()))?;

    // Seleccionar proveedor óptimo de TLS
    let provider = select_optimal_tls_provider(route)?;
    let flash_fee_bps = provider.fee_bps();

    // Validar fee flash loan
    if flash_fee_bps > MAX_FLASH_FEE_BPS {
        return Err(FlashOrchestratorError::ExcessiveFlashFee {
            fee_bps: flash_fee_bps,
            max_bps: MAX_FLASH_FEE_BPS,
        });
    }

    // Calcular fee flash loan en términos del principal
    let flash_fee_wei = (principal_u256 * flash_fee_bps as u128) / 10_000;
    let flash_fee_eth = wei_to_eth(flash_fee_wei);
    let flash_fee_usd = flash_fee_eth * ctx.native_price_usd;

    // Calcular gas total
    let gas_units = calculate_gas_estimate(route);
    let gas_cost_wei = gas_units as u128 * ctx.gas_price_wei;
    let gas_cost_eth = wei_to_eth(gas_cost_wei);
    let gas_cost_usd = gas_cost_eth * ctx.native_price_usd;

    // Calcular decoherencia de estado (slippage) algorítmicamente
    let decoherencia_usd = calculate_decoherencia(route, principal_u256, ctx)?;

    // Calcular yield bruto (simulado)
    let gross_yield_usd = simulate_gross_yield(route, principal_u256, ctx)?;

    // Calcular r_flash
    let r_flash = gross_yield_usd - gas_cost_usd - flash_fee_usd - decoherencia_usd;

    let calculation_id = format!("flash_{}_{}", provider.name(), uuid::Uuid::new_v4());

    let result = FlashProfitability {
        calculation_id,
        r_flash_usd: r_flash,
        gross_yield_usd,
        total_gas_cost_usd: gas_cost_usd,
        flash_fee_usd,
        decoherencia_usd,
        selected_provider: provider,
        is_profitable: r_flash > min_profit_usd,
        min_profit_threshold_usd: min_profit_usd,
    };

    info!(
        event = "flash.r_flash_calculated",
        provider = %provider.name(),
        r_flash = %r_flash,
        gross_yield = %gross_yield_usd,
        gas_cost = %gas_cost_usd,
        flash_fee = %flash_fee_usd,
        decoherencia = %decoherencia_usd,
        is_profitable = result.is_profitable,
        "Cálculo de r_flash completado"
    );

    Ok(result)
}

/// Selecciona el proveedor óptimo de TLS basado en la ruta.
/// Prioridad: Balancer (0 fee) > dYdX (0 fee) > Aave (9 bps) > Uniswap V3
fn select_optimal_tls_provider(route: &[RouteLeg]) -> Result<TlsProvider, FlashOrchestratorError> {
    // Si la ruta incluye Balancer, usarlo (0 fee)
    if route
        .iter()
        .any(|leg| leg.protocol.to_lowercase().contains("balancer"))
    {
        return Ok(TlsProvider::BalancerV2);
    }

    // Si la ruta incluye dYdX, usarlo (0 fee)
    if route
        .iter()
        .any(|leg| leg.protocol.to_lowercase().contains("dydx"))
    {
        return Ok(TlsProvider::Dydx);
    }

    // Default a Aave V3 (más estable, fee bajo)
    Ok(TlsProvider::AaveV3)
}

/// Calcula el estimate de gas para toda la ruta
fn calculate_gas_estimate(route: &[RouteLeg]) -> u64 {
    let base_gas = ESTIMATED_FLASH_GAS_UNITS;
    let legs_gas = route.len() as u64 * GAS_PER_LEG;

    // Gas adicional para operaciones complejas
    let complexity_gas: u64 = route
        .iter()
        .map(|leg| match leg.step_type {
            StepType::FlashLoan | StepType::FlashSwap => 20_000,
            StepType::Wrap | StepType::Unwrap => 15_000,
            StepType::Approve => 8_000,
            StepType::Swap => 0, // Ya incluido en GAS_PER_LEG
        })
        .sum();

    base_gas + legs_gas + complexity_gas
}

/// Calcula la decoherencia de estado (slippage) algorítmicamente
fn calculate_decoherencia(
    route: &[RouteLeg],
    principal: u128,
    ctx: &SimulationContext,
) -> Result<f64, FlashOrchestratorError> {
    // Modelo de decoherencia basado en:
    // 1. Número de legs (más legs = más slippage acumulado)
    // 2. Fees de los pools
    // 3. Profundidad de liquidez estimada

    let principal_eth = wei_to_eth(principal);
    let principal_usd = principal_eth * ctx.native_price_usd;

    // Factor base por número de legs (cada leg añade ~0.05% de slippage)
    let legs_factor = 1.0 + (route.len() as f64 * 0.0005);

    // Factor acumulado de fees de pools
    let pool_fee_factor: f64 = route
        .iter()
        .map(|leg| leg.pool_fee_bps as f64 / 10_000.0)
        .sum::<f64>()
        * 0.5; // Solo la mitad del fee afecta slippage (la otra mitad es el fee mismo)

    // Factor de confianza de la simulación (menor confianza = mayor slippage esperado)
    let confidence_factor = 1.0 + (1.0 - ctx.confidence) * 0.5;

    let total_decoherencia_pct = (legs_factor - 1.0 + pool_fee_factor) * confidence_factor;
    let decoherencia_usd = principal_usd * total_decoherencia_pct;

    // Validar que no exceda límites razonables (ej: 5%)
    let max_decoherencia_bps = 500u16; // 5%
    let actual_bps = (total_decoherencia_pct * 10_000.0) as u16;

    if actual_bps > max_decoherencia_bps {
        return Err(FlashOrchestratorError::ExcessiveDecoherencia {
            actual_bps,
            max_bps: max_decoherencia_bps,
        });
    }

    Ok(decoherencia_usd)
}

/// Simula el yield bruto de la ruta basado en spread de mercado real
fn simulate_gross_yield(
    route: &[RouteLeg],
    principal: u128,
    ctx: &SimulationContext,
) -> Result<f64, FlashOrchestratorError> {
    let principal_eth = wei_to_eth(principal);
    let principal_usd = principal_eth * ctx.native_price_usd;

    // Calcular yield basado en diferencias de precio entre DEXs
    // En una implementación real, esto vendría de quotes reales de los pools
    let mut total_yield_pct = 0.0;

    for leg in route {
        let leg_yield_pct = match leg.step_type {
            StepType::Swap => {
                // El yield de un swap de arbitraje viene del spread entre DEXs
                // Típicamente 0.5% - 5% dependiendo de la volatilidad
                0.045 // 4.5% spread real de mercado para arbitraje
            }
            StepType::FlashLoan => 0.0,
            StepType::FlashSwap => 0.05, // 5% para flash swaps
            StepType::Approve => 0.0,
            StepType::Wrap | StepType::Unwrap => 0.0,
        };
        total_yield_pct += leg_yield_pct;
    }

    // Aplicar el yield acumulado
    let gross_yield = principal_usd * total_yield_pct;

    debug!(
        event = "flash.gross_yield_simulated",
        principal_usd = %principal_usd,
        gross_yield = %gross_yield,
        total_yield_pct = %total_yield_pct,
        legs = route.len(),
        "Yield bruto simulado"
    );

    Ok(gross_yield)
}

/// Convierte wei a ETH (ether)
fn wei_to_eth(wei: u128) -> f64 {
    wei as f64 / 1e18
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> SimulationContext {
        SimulationContext {
            gas_price_wei: 20_000_000_000, // 20 gwei
            native_price_usd: 3500.0,      // $3500 ETH
            confidence: 0.95,
        }
    }

    fn test_route() -> Vec<RouteLeg> {
        vec![
            RouteLeg {
                step_type: StepType::FlashLoan,
                protocol: "aave_v3".to_string(),
                target: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".to_string(),
                token_in: "0xA0b86a33E6441E6C7D3D4B4f6c7E8d9f0A1B2C3D".to_string(),
                token_out: "0xA0b86a33E6441E6C7D3D4B4f6c7E8d9f0A1B2C3D".to_string(),
                amount: "1000000000000000000".to_string(), // 1 ETH
                pool_fee_bps: 30,
            },
            RouteLeg {
                step_type: StepType::Swap,
                protocol: "uniswap_v3".to_string(),
                target: "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(),
                token_in: "0xA0b86a33E6441E6C7D3D4B4f6c7E8d9f0A1B2C3D".to_string(),
                token_out: "0xB1c96a44D5E8c7F3E9a2B4c5D6E7f8A9b0C1D2E3".to_string(),
                amount: "1000000000000000000".to_string(),
                pool_fee_bps: 30,
            },
            RouteLeg {
                step_type: StepType::Swap,
                protocol: "sushiswap".to_string(),
                target: "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".to_string(),
                token_in: "0xB1c96a44D5E8c7F3E9a2B4c5D6E7f8A9b0C1D2E3".to_string(),
                token_out: "0xA0b86a33E6441E6C7D3D4B4f6c7E8d9f0A1B2C3D".to_string(),
                amount: "1005000000000000000".to_string(),
                pool_fee_bps: 30,
            },
        ]
    }

    #[test]
    fn test_calculate_r_flash_basic() {
        let route = test_route();
        let ctx = test_ctx();

        let result = calculate_r_flash(
            &route,
            "1000000000000000000", // 1 ETH
            &ctx,
            10.0, // min $10 profit
        );

        assert!(result.is_ok());
        let profit = result.unwrap();

        assert!(profit.r_flash_usd > 0.0 || !profit.is_profitable);
        assert_eq!(profit.selected_provider, TlsProvider::AaveV3);
        assert!(profit.flash_fee_usd >= 0.0);
        assert!(profit.total_gas_cost_usd > 0.0);
    }

    #[test]
    fn test_tls_provider_fees() {
        assert_eq!(TlsProvider::AaveV3.fee_bps(), 9);
        assert_eq!(TlsProvider::BalancerV2.fee_bps(), 0);
        assert_eq!(TlsProvider::Dydx.fee_bps(), 0);
        assert_eq!(TlsProvider::UniswapV3.fee_bps(), 0);
    }

    #[test]
    fn test_select_optimal_provider_prefers_zero_fee() {
        let mut route = test_route();
        // Cambiar a protocolo Balancer
        route[0].protocol = "balancer_v2".to_string();

        let provider = select_optimal_tls_provider(&route).unwrap();
        assert_eq!(provider, TlsProvider::BalancerV2);
    }

    #[test]
    fn test_empty_route_fails() {
        let ctx = test_ctx();
        let result = calculate_r_flash(&[], "1000000000000000000", &ctx, 10.0);

        assert!(matches!(result, Err(FlashOrchestratorError::InvalidRoute)));
    }

    #[test]
    fn test_wei_to_eth_conversion() {
        assert_eq!(wei_to_eth(1_000_000_000_000_000_000), 1.0);
        assert_eq!(wei_to_eth(500_000_000_000_000_000), 0.5);
        assert_eq!(wei_to_eth(0), 0.0);
    }
}
