//! Superposición Temporal de Liquidez (TLS)
//!
//! Para recorrer un lazo holonómico de N ≥ 3 variedades de manera atómica,
//! el sistema requiere energía de activación que no posee en su estado base.
//! La TLS modela la capacidad de tomar prestada energía del vacío de la red
//! (Liquidity Providers / Flash Loans) al inicio de la trayectoria y devolverla
//! (State Collapse) al final del lazo en el mismo ciclo temporal.
//!
//! El costo de esta superposición —Vacuum Decoherence Cost— se deduce del
//! TopologicalYield ANTES de que el candidato pase por el GateManager.

use super::{ClosedContourTrajectory, TopologicalYield};

/// Estado de superposición temporal de liquidez.
/// Representa capital "borrowed" que existe simultáneamente en múltiples
/// puntos del lazo holonómico durante la ventana de ejecución atómica.
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalLiquiditySuperposition {
    /// Capital total tomado prestado del vacío (en unidades del token base)
    pub borrowed_capital: f64,
    /// Token base del préstamo (ej: WETH, USDC)
    pub base_token: String,
    /// Duración de la superposición (ms) — debe ser < half_life del CDC
    pub superposition_duration_ms: u64,
    /// Costo de decoherencia del vacío (flash_loan_fee + gas + slippage)
    pub vacuum_decoherence_cost: f64,
    /// Manifolds donde el capital está superpuesto
    pub superposed_manifolds: Vec<String>,
    /// Estado de la superposición
    pub state: SuperpositionState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuperpositionState {
    /// Superposición activa: capital borrowed, listo para ejecutar lazo
    Coherent,
    /// Superposición colapsada: capital devuelto, yield realizado
    Collapsed { net_yield_realized: f64 },
    /// Superposición decoherente: fallo en ejecución, capital perdido parcialmente
    Decohered { loss: f64 },
}

impl TemporalLiquiditySuperposition {
    /// Factor de fee típico de flash loan (ej: Aave = 0.09%, DyDx = 0.0%)
    pub const DEFAULT_FLASH_LOAN_FEE_BPS: u32 = 9;
    /// Gas estimado por transacción en el lazo (en unidades de capital)
    pub const DEFAULT_GAS_COST_FACTOR: f64 = 0.0001; // 0.01%

    /// Inicializa una superposición para un lazo holonómico dado.
    pub fn for_contour(
        contour: &ClosedContourTrajectory,
        base_token: &str,
        flash_loan_fee_bps: u32,
    ) -> Result<Self, TlsError> {
        if contour.loop_cardinality < ClosedContourTrajectory::MIN_LOOP_SIZE {
            return Err(TlsError::ContourTooShort);
        }

        // Capital requerido = máximo de liquidez comprometida en el lazo
        let borrowed = contour.manifolds.iter()
            .map(|m| m.token0_reserve.max(m.token1_reserve))
            .fold(0.0, f64::max);

        // Costo de decoherencia = flash_loan_fee + gas*N + slippage_buffer
        let flash_fee = borrowed * (flash_loan_fee_bps as f64 / 10_000.0);
        let gas_cost = borrowed * Self::DEFAULT_GAS_COST_FACTOR * contour.loop_cardinality as f64;
        let slippage_buffer = borrowed * 0.001 * contour.loop_cardinality as f64; // 0.1% por hop
        let vacuum_cost = flash_fee + gas_cost + slippage_buffer;

        let manifold_ids: Vec<String> = contour.manifolds.iter()
            .map(|m| m.pool_address.clone())
            .collect();

        Ok(Self {
            borrowed_capital: borrowed,
            base_token: base_token.to_string(),
            superposition_duration_ms: 0, // Se establece al ejecutar
            vacuum_decoherence_cost: vacuum_cost,
            superposed_manifolds: manifold_ids,
            state: SuperpositionState::Coherent,
        })
    }

    /// Colapsa la superposición al final del lazo holonómico.
    /// Devuelve el capital + deduce el VacuumDecoherenceCost.
    pub fn collapse(
        &mut self,
        raw_yield: f64,
        execution_time_ms: u64,
    ) -> Result<f64, TlsError> {
        if !matches!(self.state, SuperpositionState::Coherent) {
            return Err(TlsError::AlreadyCollapsed);
        }

        self.superposition_duration_ms = execution_time_ms;
        let net = raw_yield - self.vacuum_decoherence_cost;

        if net > 0.0 {
            self.state = SuperpositionState::Collapsed {
                net_yield_realized: net,
            };
            Ok(net)
        } else {
            self.state = SuperpositionState::Decohered {
                loss: self.vacuum_decoherence_cost - raw_yield,
            };
            Err(TlsError::DecoherenceLoss {
                raw_yield,
                vacuum_cost: self.vacuum_decoherence_cost,
                loss: self.vacuum_decoherence_cost - raw_yield,
            })
        }
    }

    /// Verifica que la superposición sea económicamente viable ANTES de ejecutar.
    pub fn is_viable_given_expected_yield(&self, expected_raw_yield: f64) -> bool {
        expected_raw_yield > self.vacuum_decoherence_cost * 1.5 // Margen de seguridad 50%
    }
}

/// Costo de decoherencia del vacío — desglose detallado para métricas.
#[derive(Debug, Clone, PartialEq)]
pub struct VacuumDecoherenceCost {
    /// Fee del proveedor de flash loan
    pub flash_loan_fee: f64,
    /// Costo de gas estimado para N transacciones
    pub gas_cost: f64,
    /// Slippage esperado por N hops
    pub slippage_cost: f64,
    /// Costo de oportunidad (capital locked durante la superposición)
    pub opportunity_cost: f64,
    /// Costo total = suma de los anteriores
    pub total_cost: f64,
}

impl VacuumDecoherenceCost {
    /// Calcula el costo de decoherencia para un lazo holonómico dado.
    pub fn for_contour(
        contour: &ClosedContourTrajectory,
        borrowed_capital: f64,
        flash_loan_fee_bps: u32,
        gas_price_gwei: f64,
    ) -> Self {
        let n = contour.loop_cardinality as f64;
        let flash = borrowed_capital * (flash_loan_fee_bps as f64 / 10_000.0);
        let gas = borrowed_capital * 0.0001 * n * (gas_price_gwei / 20.0);
        let slippage = borrowed_capital * 0.001 * n;
        let opportunity = borrowed_capital * 0.00005 * n; // 0.005% por hop
        let total = flash + gas + slippage + opportunity;

        Self {
            flash_loan_fee: flash,
            gas_cost: gas,
            slippage_cost: slippage,
            opportunity_cost: opportunity,
            total_cost: total,
        }
    }
}

/// Motor de superposición temporal — orquesta borrow→execute→repay.
pub struct TemporalLiquidityEngine {
    /// Flash loan provider (ej: "aave-v3", "dyDx", "balancer")
    pub provider: String,
    /// Fee en basis points
    pub flash_loan_fee_bps: u32,
    /// Gas price estimado (gwei)
    pub gas_price_gwei: f64,
}

impl TemporalLiquidityEngine {
    pub fn new(provider: &str, flash_loan_fee_bps: u32, gas_price_gwei: f64) -> Self {
        Self {
            provider: provider.to_string(),
            flash_loan_fee_bps,
            gas_price_gwei,
        }
    }

    /// Ejecuta el ciclo completo TLS para un lazo holonómico.
    /// En Paper-Shadow Mode, los swaps son simulados (no on-chain).
    pub fn execute_holonomic_loop(
        &self,
        contour: &ClosedContourTrajectory,
    ) -> Result<TlsExecutionResult, TlsError> {
        // 1. Inicializar superposición
        let base_token = &contour.start_token;
        let mut tls = TemporalLiquiditySuperposition::for_contour(
            contour, base_token, self.flash_loan_fee_bps,
        )?;

        // 2. Calcular VacuumDecoherenceCost detallado
        let vacuum = VacuumDecoherenceCost::for_contour(
            contour, tls.borrowed_capital, self.flash_loan_fee_bps, self.gas_price_gwei,
        );

        // 3. Calcular holonomía bruta del lazo
        let raw_holonomy = contour.contour_integral();
        let raw_yield = raw_holonomy.abs() * tls.borrowed_capital; // Escalar a capital

        // 4. Verificar viabilidad PRE-ejecución (GateManager check)
        if !tls.is_viable_given_expected_yield(raw_yield) {
            return Err(TlsError::ExpectedYieldTooLow {
                raw_yield,
                vacuum_cost: vacuum.total_cost,
            });
        }

        // 5. Simular ejecución atómica (en Paper-Shadow: sin firma real)
        let execution_time_ms = 250u64; // Target: < 500ms atómico

        // 6. Colapsar superposición
        let net_yield = tls.collapse(raw_yield, execution_time_ms)?;

        // 7. Construir resultado
        let yield_data = TopologicalYield::from_holonomy_and_friction(
            raw_holonomy,
            vacuum.gas_cost + vacuum.slippage_cost + vacuum.opportunity_cost,
            vacuum.flash_loan_fee,
            tls.superposed_manifolds.clone(),
        );

        Ok(TlsExecutionResult {
            tls,
            vacuum_cost: vacuum,
            raw_yield,
            net_yield,
            topological_yield: yield_data,
            execution_time_ms,
        })
    }

    /// Construye un BundlePosition<HolonomicLoopResolution> sellado
    /// a partir de un resultado TLS exitoso.
    pub fn to_bundle_position(
        &self,
        result: &TlsExecutionResult,
        contour: &ClosedContourTrajectory,
    ) -> Result<crate::types::bundle_position::BundlePosition<crate::types::bundle_position::HolonomicLoopResolution>, crate::types::errors::TopologyValidationError> {
        let canonical_contour = contour.to_canonical();
        let canonical_yield = result.topological_yield.to_canonical();
        crate::types::bundle_position::BundlePosition::new_holonomic_loop(
            contour.start_token.clone(),
            (contour.start_token.clone(), contour.start_token.clone()),
            result.tls.borrowed_capital,
            result.net_yield,
            &canonical_contour,
            &canonical_yield,
        )
    }
}

/// Resultado de la ejecución TLS de un lazo holonómico.
#[derive(Debug, Clone, PartialEq)]
pub struct TlsExecutionResult {
    /// Estado final de la superposición
    pub tls: TemporalLiquiditySuperposition,
    /// Costo de decoherencia desglosado
    pub vacuum_cost: VacuumDecoherenceCost,
    /// Yield bruto antes de deducciones
    pub raw_yield: f64,
    /// Yield neto post-VacuumDecoherenceCost
    pub net_yield: f64,
    /// Rendimiento topológico formal
    pub topological_yield: TopologicalYield,
    /// Tiempo de ejecución atómica (ms)
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TlsError {
    #[error("Contour too short: need >= 3 manifolds")]
    ContourTooShort,
    #[error("Superposición ya colapsada")]
    AlreadyCollapsed,
    #[error("Decoherence loss: raw_yield={raw_yield}, vacuum_cost={vacuum_cost}, loss={loss}")]
    DecoherenceLoss { raw_yield: f64, vacuum_cost: f64, loss: f64 },
    #[error("Expected yield too low: raw={raw_yield}, vacuum={vacuum_cost}")]
    ExpectedYieldTooLow { raw_yield: f64, vacuum_cost: f64 },
    #[error("Insufficient liquidity for borrow amount: {0}")]
    InsufficientLiquidity(f64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::LiquidityManifold;

    fn pool(addr: &str, token0: &str, token1: &str, x: f64, y: f64) -> LiquidityManifold {
        LiquidityManifold::new(x * y, x, y, addr.to_string(),
            (token0.to_string(), token1.to_string()), 1).unwrap()
    }

    fn test_contour() -> ClosedContourTrajectory {
        ClosedContourTrajectory {
            manifolds: vec![
                pool("0xA", "WETH", "USDC", 1.0, 2000.0),
                pool("0xB", "USDC", "DAI", 1000.0, 1000.0),
                pool("0xC", "DAI", "WETH", 1900.0, 1.0),
            ],
            transition_points: vec![
                nalgebra::Vector2::new(1.0, 2000.0),
                nalgebra::Vector2::new(1000.0, 1000.0),
                nalgebra::Vector2::new(1900.0, 1.0),
            ],
            relative_prices: vec![2000.0, 1.0, 1.0/1900.0],
            contour_length: 0.0,
            loop_cardinality: 3,
            start_token: "WETH".to_string(),
        }
    }

    #[test]
    fn tls_initializes_for_contour() {
        let contour = test_contour();
        let tls = TemporalLiquiditySuperposition::for_contour(&contour, "WETH", 9).unwrap();
        assert!(tls.borrowed_capital > 0.0);
        assert!(tls.vacuum_decoherence_cost > 0.0);
        assert_eq!(tls.superposed_manifolds.len(), 3);
        assert!(matches!(tls.state, SuperpositionState::Coherent));
    }

    #[test]
    fn tls_rejects_short_contour() {
        let short = ClosedContourTrajectory {
            manifolds: vec![pool("0xA", "WETH", "USDC", 1.0, 2000.0)],
            transition_points: vec![nalgebra::Vector2::new(1.0, 2000.0)],
            relative_prices: vec![2000.0],
            contour_length: 0.0,
            loop_cardinality: 1,
            start_token: "WETH".to_string(),
        };
        let result = TemporalLiquiditySuperposition::for_contour(&short, "WETH", 9);
        assert!(matches!(result, Err(TlsError::ContourTooShort)));
    }

    #[test]
    fn tls_collapses_with_positive_yield() {
        let contour = test_contour();
        let mut tls = TemporalLiquiditySuperposition::for_contour(&contour, "WETH", 9).unwrap();
        let raw_yield = 1000.0; // Holonomía escalar alta
        let net = tls.collapse(raw_yield, 250).unwrap();
        assert!(net > 0.0);
        assert!(net < raw_yield);
        assert!(matches!(tls.state, SuperpositionState::Collapsed { .. }));
    }

    #[test]
    fn tls_decoheres_with_negative_yield() {
        let contour = test_contour();
        let mut tls = TemporalLiquiditySuperposition::for_contour(&contour, "WETH", 9).unwrap();
        let raw_yield = 0.001; // Muy bajo
        let result = tls.collapse(raw_yield, 250);
        assert!(result.is_err());
        assert!(matches!(tls.state, SuperpositionState::Decohered { .. }));
    }

    #[test]
    fn engine_executes_holonomic_loop() {
        let contour = test_contour();
        let engine = TemporalLiquidityEngine::new("aave-v3", 9, 20.0);
        let result = engine.execute_holonomic_loop(&contour);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        let exec = result.unwrap();
        assert!(exec.net_yield > 0.0);
        assert!(exec.execution_time_ms <= 500);
        assert!(exec.vacuum_cost.total_cost > 0.0);
    }

    #[test]
    fn vacuum_cost_breakdown_sums_correctly() {
        let contour = test_contour();
        let vacuum = VacuumDecoherenceCost::for_contour(&contour, 10000.0, 9, 20.0);
        let expected_total = vacuum.flash_loan_fee + vacuum.gas_cost
            + vacuum.slippage_cost + vacuum.opportunity_cost;
        assert!((vacuum.total_cost - expected_total).abs() < 1e-9);
    }

    #[test]
    fn viability_check_rejects_marginal_yield() {
        let contour = test_contour();
        let tls = TemporalLiquiditySuperposition::for_contour(&contour, "WETH", 9).unwrap();
        let marginal = tls.vacuum_decoherence_cost * 1.2;
        assert!(!tls.is_viable_given_expected_yield(marginal));
        let safe = tls.vacuum_decoherence_cost * 2.0;
        assert!(tls.is_viable_given_expected_yield(safe));
    }
}
