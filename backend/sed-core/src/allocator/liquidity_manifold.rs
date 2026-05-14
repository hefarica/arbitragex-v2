//! Variedad de liquidez CPMM: ℒₖ = { (x, y) ∈ ℝ²⁺ | x · y = k }
//!
//! Métrica hiperbólica inducida por la curva de producto constante.

use nalgebra::{DMatrix, Vector2};
use serde::{Deserialize, Serialize};

/// Variedad de liquidez con restricción hiperbólica x·y = k.
/// Representa un pool Uniswap V2 o equivalente CPMM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiquidityManifold {
    /// Producto constante k = x·y
    pub constant_product: f64,
    /// Reserva actual del token 0
    pub token0_reserve: f64,
    /// Reserva actual del token 1
    pub token1_reserve: f64,
    /// Dirección del pool (para trazabilidad)
    pub pool_address: String,
    /// Token pair (ej: "WETH/USDC")
    pub token_pair: (String, String),
    /// Chain ID
    pub chain_id: u64,
}

impl LiquidityManifold {
    /// Construye una variedad verificando que x·y = k (dentro de tolerancia)
    pub fn new(
        constant_product: f64,
        token0_reserve: f64,
        token1_reserve: f64,
        pool_address: String,
        token_pair: (String, String),
        chain_id: u64,
    ) -> Result<Self, ManifoldError> {
        let product = token0_reserve * token1_reserve;
        let rel_err = (product - constant_product).abs() / constant_product;
        if rel_err > 1e-9 {
            return Err(ManifoldError::HyperbolicViolation {
                expected: constant_product,
                actual: product,
                relative_error: rel_err,
            });
        }
        Ok(Self {
            constant_product,
            token0_reserve,
            token1_reserve,
            pool_address,
            token_pair,
            chain_id,
        })
    }

    /// Evalúa la métrica hiperbólica g en el punto actual
    /// g_ij = diag(1/x², 1/y²) para el CPMM
    pub fn metric_tensor(&self) -> DMatrix<f64> {
        let mut g = DMatrix::zeros(2, 2);
        g[(0, 0)] = 1.0 / (self.token0_reserve * self.token0_reserve);
        g[(1, 1)] = 1.0 / (self.token1_reserve * self.token1_reserve);
        g
    }

    /// Distancia geodésica inducida por la métrica hiperbólica
    pub fn geodesic_distance(&self, other: &LiquidityManifold) -> f64 {
        let dx = other.token0_reserve - self.token0_reserve;
        let dy = other.token1_reserve - self.token1_reserve;
        (dx * dx / (self.token0_reserve * self.token0_reserve)
            + dy * dy / (self.token1_reserve * self.token1_reserve))
        .sqrt()
    }

    /// Precio relativo implícito del pool: p = y/x
    pub fn spot_price(&self) -> f64 {
        self.token1_reserve / self.token0_reserve
    }

    /// Punto en la variedad como vector ℝ²
    pub fn as_vector(&self) -> Vector2<f64> {
        Vector2::new(self.token0_reserve, self.token1_reserve)
    }

    /// Calcula las nuevas reservas tras un swap de Δx (entrada de token0)
    pub fn swap_exact_token0_for_token1(
        &self,
        delta_token0_in: f64,
        fee_bps: u32,
    ) -> Result<(f64, f64), ManifoldError> {
        if delta_token0_in <= 0.0 {
            return Err(ManifoldError::NonPositiveInput);
        }
        let fee = fee_bps as f64 / 10_000.0;
        let amount_in_with_fee = delta_token0_in * (1.0 - fee);
        let new_token0 = self.token0_reserve + amount_in_with_fee;
        let new_token1 = self.constant_product / new_token0;
        let delta_token1_out = self.token1_reserve - new_token1;
        if delta_token1_out <= 0.0 {
            return Err(ManifoldError::InsufficientLiquidity);
        }
        Ok((new_token0, new_token1))
    }

    /// Calcula las nuevas reservas tras un swap de Δy (entrada de token1)
    pub fn swap_exact_token1_for_token0(
        &self,
        delta_token1_in: f64,
        fee_bps: u32,
    ) -> Result<(f64, f64), ManifoldError> {
        if delta_token1_in <= 0.0 {
            return Err(ManifoldError::NonPositiveInput);
        }
        let fee = fee_bps as f64 / 10_000.0;
        let amount_in_with_fee = delta_token1_in * (1.0 - fee);
        let new_token1 = self.token1_reserve + amount_in_with_fee;
        let new_token0 = self.constant_product / new_token1;
        let delta_token0_out = self.token0_reserve - new_token0;
        if delta_token0_out <= 0.0 {
            return Err(ManifoldError::InsufficientLiquidity);
        }
        Ok((new_token0, new_token1))
    }

    /// Genera una variedad actualizada con nuevas reservas, verificando x·y = k
    pub fn with_reserves(
        &self,
        token0: f64,
        token1: f64,
    ) -> Result<Self, ManifoldError> {
        Self::new(
            self.constant_product,
            token0,
            token1,
            self.pool_address.clone(),
            self.token_pair.clone(),
            self.chain_id,
        )
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ManifoldError {
    #[error("Hyperbolic violation: expected k={expected}, actual={actual}, rel_err={relative_error:e}")]
    HyperbolicViolation {
        expected: f64,
        actual: f64,
        relative_error: f64,
    },
    #[error("Non-positive input amount")]
    NonPositiveInput,
    #[error("Insufficient liquidity for output")]
    InsufficientLiquidity,
}
