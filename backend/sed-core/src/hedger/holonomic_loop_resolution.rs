//! Resolución de Lazos Holonómicos — La Joya de la Corona
//!
//! Implementa la búsqueda topológica de trayectorias de contorno cerrado
//! γ: ℒ₁ → ℒ₂ → ... → ℒₙ → ℒ₁  con  n ≥ 3.
//!
//! Si la integral de contorno de precios relativos ∮_γ (dp/p) ≠ 0,
//! el sistema extrae un TopologicalYield neto positivo (post-fricción)
//! y construye un BundlePosition<HolonomicLoopResolution> sellado.

use crate::allocator::LiquidityManifold;
use nalgebra::Vector2;
use std::collections::{HashMap, HashSet};

/// Trayectoria de contorno cerrado sobre N ≥ 3 variedades de liquidez.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedContourTrajectory {
    /// Variedades en orden de recorrido del ciclo
    pub manifolds: Vec<LiquidityManifold>,
    /// Puntos de transición (reservas en cada pool al entrar)
    pub transition_points: Vec<Vector2<f64>>,
    /// Precios relativos en cada arista: p_i = price_{i+1} / price_i
    pub relative_prices: Vec<f64>,
    /// Longitud geodésica total del contorno
    pub contour_length: f64,
    /// Cardinalidad del lazo (N)
    pub loop_cardinality: usize,
    /// Token de inicio (debe coincidir con token de fin)
    pub start_token: String,
}

impl ClosedContourTrajectory {
    pub const MIN_LOOP_SIZE: usize = 3;

    /// Verifica que el contorno sea cerrado topológicamente:
    /// punto inicial = punto final  y  token inicial = token final.
    ///
    /// OMEGA-8/M4 Fase 2: refactored to use slice pattern matching instead of
    /// `.first().unwrap()` / `.last().unwrap()`. The size guard remains as a
    /// fast-path, and the pattern match makes pre-dispatch panic-free
    /// (RULE 9 — no `unwrap`/`expect` in pre-dispatch).
    pub fn is_closed(&self) -> bool {
        if self.manifolds.len() < Self::MIN_LOOP_SIZE {
            return false;
        }
        match (self.manifolds.first(), self.manifolds.last()) {
            (Some(first), Some(last)) => first.token_pair.0 == last.token_pair.1,
            _ => false,
        }
    }

    /// Result-based variant of [`Self::is_closed`] that surfaces the exact
    /// reason a contour is not closed instead of collapsing to `bool`. Useful
    /// for the invariant checker pre-dispatch path which already maps to a
    /// typed `InvariantViolation`. RULE 8 (no silent errors).
    pub fn closure_check(&self) -> Result<(), ClosureError> {
        if self.manifolds.len() < Self::MIN_LOOP_SIZE {
            return Err(ClosureError::LoopTooShort {
                actual: self.manifolds.len(),
                min: Self::MIN_LOOP_SIZE,
            });
        }
        let first = self
            .manifolds
            .first()
            .ok_or(ClosureError::EmptyManifoldLoop)?;
        let last = self
            .manifolds
            .last()
            .ok_or(ClosureError::EmptyManifoldLoop)?;
        if first.token_pair.0 != last.token_pair.1 {
            return Err(ClosureError::InvalidLoopBoundary {
                first: first.token_pair.0.clone(),
                last: last.token_pair.1.clone(),
            });
        }
        Ok(())
    }

    /// Calcula la integral de contorno de log-precios relativos:
    /// ∮_γ (dp/p) = Σᵢ ln(p_{i+1} / p_i)
    ///
    /// Si el resultado es no nulo, indica una discrepancia de precios
    /// persistente (ineficiencia transitoria de ciclo cerrado explotable).
    pub fn contour_integral(&self) -> f64 {
        self.relative_prices.iter().map(|p| p.ln()).sum()
    }

    /// Producto de precios relativos alrededor del ciclo.
    /// Para un ciclo perfecto: Π p_i = 1.  Si ≠ 1, hay holonomía.
    pub fn price_product(&self) -> f64 {
        self.relative_prices.iter().product()
    }

    /// Verifica que no haya autointersección (pools repetidos en el lazo).
    pub fn is_simple(&self) -> bool {
        let mut seen = HashSet::new();
        for m in &self.manifolds {
            let key = format!("{}-{}", m.pool_address, m.token_pair.0);
            if !seen.insert(key) {
                return false;
            }
        }
        true
    }
}

/// Rendimiento topológico extraído de una holonomía de mercado.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologicalYield {
    /// Holonomía bruta: ∮_γ (dp/p)
    pub raw_holonomy: f64,
    /// Fricción termodinámica total de la red:
    pub network_friction: f64,
    /// Rendimiento neto: raw_holonomy − network_friction
    pub net_yield: f64,
    /// Factor de eficiencia: net_yield / |raw_holonomy|
    pub efficiency_factor: f64,
    /// IDs de las variedades involucradas
    pub manifold_ids: Vec<String>,
    /// Timestamp de resolución (ms desde epoch)
    pub resolved_at: u64,
    /// Costo de decoherencia del vacío (TLS)
    pub vacuum_decoherence_cost: f64,
}

impl TopologicalYield {
    pub const MINIMUM_VIABLE_YIELD: f64 = 1e-15;

    pub fn is_economically_viable(&self) -> bool {
        self.net_yield > Self::MINIMUM_VIABLE_YIELD
    }

    pub fn is_nontrivial_holonomy(&self) -> bool {
        self.raw_holonomy.abs() > 1e-12
    }

    pub fn from_holonomy_and_friction(
        raw_holonomy: f64,
        network_friction: f64,
        vacuum_decoherence_cost: f64,
        manifold_ids: Vec<String>,
    ) -> Self {
        let total_friction = network_friction + vacuum_decoherence_cost;
        let net = raw_holonomy - total_friction;
        let efficiency = if raw_holonomy.abs() > 1e-15 {
            net / raw_holonomy.abs()
        } else {
            0.0
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            raw_holonomy,
            network_friction: total_friction,
            net_yield: net,
            efficiency_factor: efficiency,
            manifold_ids,
            resolved_at: now,
            vacuum_decoherence_cost,
        }
    }
}

/// Grafo de manifolds donde nodos = tokens, aristas = pools.
#[derive(Debug, Clone)]
pub struct LiquidityGraph {
    pub edges: HashMap<String, Vec<LiquidityEdge>>,
    pub manifolds: HashMap<String, LiquidityManifold>,
}

#[derive(Debug, Clone)]
pub struct LiquidityEdge {
    pub to_token: String,
    pub pool_address: String,
    pub log_price: f64,
}

impl Default for LiquidityGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl LiquidityGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            manifolds: HashMap::new(),
        }
    }

    pub fn add_pool(&mut self, manifold: &LiquidityManifold) {
        let token0 = manifold.token_pair.0.clone();
        let token1 = manifold.token_pair.1.clone();
        let addr = manifold.pool_address.clone();

        let price_0_to_1 = manifold.spot_price();
        let price_1_to_0 = 1.0 / price_0_to_1;

        self.edges
            .entry(token0.clone())
            .or_default()
            .push(LiquidityEdge {
                to_token: token1.clone(),
                pool_address: addr.clone(),
                log_price: price_0_to_1.ln(),
            });
        self.edges
            .entry(token1.clone())
            .or_default()
            .push(LiquidityEdge {
                to_token: token0.clone(),
                pool_address: addr.clone(),
                log_price: price_1_to_0.ln(),
            });

        self.manifolds.insert(addr.clone(), manifold.clone());
    }

    /// Contexto mutable para la búsqueda DFS de ciclos holonómicos.
    /// Agrupa los estados transitorios para reducir la aridad de la función recursiva.
    struct DfsContext<'a> {
        /// Conjunto de tokens visitados en el camino actual
        visited: &'a mut HashSet<String>,
        /// Camino de aristas recorrido hasta el momento
        path: &'a mut Vec<LiquidityEdge>,
        /// Camino de precios acumulados
        price_path: &'a mut Vec<f64>,
        /// Resultados acumulados de ciclos encontrados
        results: &'a mut Vec<ClosedContourTrajectory>,
    }

    /// Búsqueda de ciclos holonómicos mediante DFS con límite de profundidad.
    pub fn find_holonomic_cycles(
        &self,
        start_token: &str,
        max_depth: usize,
        min_holonomy: f64,
    ) -> Vec<ClosedContourTrajectory> {
        let mut results = Vec::new();
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        let mut price_path = Vec::new();

        let mut ctx = DfsContext {
            visited: &mut visited,
            path: &mut path,
            price_path: &mut price_path,
            results: &mut results,
        };

        self.dfs_cycles(
            start_token,
            start_token,
            max_depth,
            min_holonomy,
            &mut ctx,
        );
        results
    }

    fn dfs_cycles(
        &self,
        current: &str,
        start: &str,
        remaining_depth: usize,
        min_holonomy: f64,
        ctx: &mut DfsContext<'_>,
    ) {
        if remaining_depth == 0 {
            return;
        }

        if let Some(edges) = self.edges.get(current) {
            for edge in edges {
                if ctx.path.len() == 1 && edge.to_token == start {
                    continue;
                }

                if edge.to_token == start
                    && ctx.path.len() >= ClosedContourTrajectory::MIN_LOOP_SIZE - 1
                {
                    ctx.price_path.push(edge.log_price.exp());
                    let holonomy = ctx.price_path.iter().map(|p| p.ln()).sum::<f64>();

                    if holonomy.abs() > min_holonomy {
                        let mut manifolds = Vec::with_capacity(ctx.path.len() + 1);
                        let mut transitions = Vec::with_capacity(ctx.path.len() + 1);
                        let mut prices = Vec::with_capacity(ctx.path.len() + 1);

                        for e in ctx.path.iter() {
                            if let Some(m) = self.manifolds.get(&e.pool_address) {
                                manifolds.push(m.clone());
                                transitions.push(Vector2::new(m.token0_reserve, m.token1_reserve));
                                prices.push(e.log_price.exp());
                            }
                        }
                        if let Some(m) = self.manifolds.get(&edge.pool_address) {
                            manifolds.push(m.clone());
                            transitions.push(Vector2::new(m.token0_reserve, m.token1_reserve));
                            prices.push(edge.log_price.exp());
                        }

                        let contour_length = transitions
                            .windows(2)
                            .map(|w| {
                                let dx = w[1].x - w[0].x;
                                let dy = w[1].y - w[0].y;
                                (dx * dx + dy * dy).sqrt()
                            })
                            .sum();

                        ctx.results.push(ClosedContourTrajectory {
                            manifolds,
                            transition_points: transitions,
                            relative_prices: prices,
                            contour_length,
                            loop_cardinality: ctx.path.len() + 1,
                            start_token: start.to_string(),
                        });
                    }
                    ctx.price_path.pop();
                    continue;
                }

                if !ctx.visited.contains(&edge.to_token) {
                    ctx.visited.insert(edge.to_token.clone());
                    ctx.path.push(edge.clone());
                    ctx.price_path.push(edge.log_price.exp());

                    self.dfs_cycles(
                        &edge.to_token,
                        start,
                        remaining_depth - 1,
                        min_holonomy,
                        ctx,
                    );

                    ctx.path.pop();
                    ctx.price_path.pop();
                    ctx.visited.remove(&edge.to_token);
                }
            }
        }
    }
}

/// Verificador de invariante holonómica — 5 sub-condiciones formales.
pub struct HolonomicInvariantChecker;

impl HolonomicInvariantChecker {
    pub fn verify(
        contour: &ClosedContourTrajectory,
        yield_data: &TopologicalYield,
        execution_time_ms: u64,
        cdc_half_life_ms: u64,
    ) -> Result<HolonomicInvariantReport, InvariantViolation> {
        let holonomy = contour.contour_integral();
        if holonomy.abs() < 1e-12 {
            return Err(InvariantViolation::TrivialHolonomy { value: holonomy });
        }
        if !yield_data.is_economically_viable() {
            return Err(InvariantViolation::NonViableYield {
                net_yield: yield_data.net_yield,
                threshold: TopologicalYield::MINIMUM_VIABLE_YIELD,
            });
        }
        if !contour.is_closed() {
            return Err(InvariantViolation::OpenContour);
        }
        if !contour.is_simple() {
            return Err(InvariantViolation::SelfIntersectingContour);
        }
        if execution_time_ms >= cdc_half_life_ms {
            return Err(InvariantViolation::ExecutionTooSlow {
                execution_time_ms,
                half_life_ms: cdc_half_life_ms,
            });
        }
        Ok(HolonomicInvariantReport {
            holonomy,
            net_yield: yield_data.net_yield,
            is_closed: true,
            is_simple: true,
            is_atomic: true,
            verification_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HolonomicInvariantReport {
    pub holonomy: f64,
    pub net_yield: f64,
    pub is_closed: bool,
    pub is_simple: bool,
    pub is_atomic: bool,
    pub verification_timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InvariantViolation {
    #[error("Holonomía trivial: {value}")]
    TrivialHolonomy { value: f64 },
    #[error("Rendimiento no viable: net_yield={net_yield} < threshold={threshold}")]
    NonViableYield { net_yield: f64, threshold: f64 },
    #[error("Contorno no cerrado")]
    OpenContour,
    #[error("Contorno con autointersección")]
    SelfIntersectingContour,
    #[error("Ejecución demasiado lenta: {execution_time_ms}ms >= half-life {half_life_ms}ms")]
    ExecutionTooSlow {
        execution_time_ms: u64,
        half_life_ms: u64,
    },
}

/// OMEGA-8/M4 Fase 2: typed errors for [`ClosedContourTrajectory::closure_check`].
/// Each variant maps 1-to-1 to a precise topological violation, replacing the
/// previous `.first().unwrap()` / `.last().unwrap()` pre-dispatch panic surface.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ClosureError {
    #[error("Manifold loop está vacío")]
    EmptyManifoldLoop,
    #[error("Loop demasiado corto: actual={actual} < min={min}")]
    LoopTooShort { actual: usize, min: usize },
    #[error("Frontera de loop inválida: first_in={first} != last_out={last}")]
    InvalidLoopBoundary { first: String, last: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::LiquidityManifold;

    fn pool(addr: &str, token0: &str, token1: &str, x: f64, y: f64) -> LiquidityManifold {
        LiquidityManifold::new(
            x * y,
            x,
            y,
            addr.to_string(),
            (token0.to_string(), token1.to_string()),
            1,
        )
        .unwrap()
    }

    #[test]
    fn contour_integral_detects_inefficiency_transitoria() {
        let m1 = pool("0xA", "WETH", "USDC", 1.0, 2000.0);
        let m2 = pool("0xB", "USDC", "DAI", 1000.0, 1000.0);
        let m3 = pool("0xC", "DAI", "WETH", 1900.0, 1.0);

        let contour = ClosedContourTrajectory {
            manifolds: vec![m1, m2, m3],
            transition_points: vec![
                Vector2::new(1.0, 2000.0),
                Vector2::new(1000.0, 1000.0),
                Vector2::new(1900.0, 1.0),
            ],
            relative_prices: vec![2000.0, 1.0, 1.0 / 1900.0],
            contour_length: 0.0,
            loop_cardinality: 3,
            start_token: "WETH".to_string(),
        };

        assert!(contour.is_closed());
        let holonomy = contour.contour_integral();
        assert!(holonomy.abs() > 1e-3, "Holonomy too small: {}", holonomy);
    }

    #[test]
    fn topological_yield_positive_after_friction() {
        let yield_data = TopologicalYield::from_holonomy_and_friction(
            0.05,
            0.01,
            0.005,
            vec!["0xA".to_string(), "0xB".to_string(), "0xC".to_string()],
        );
        assert!(yield_data.is_economically_viable());
        assert!((yield_data.net_yield - 0.035).abs() < 1e-9);
    }

    #[test]
    fn graph_finds_triangular_inefficiency() {
        let mut graph = LiquidityGraph::new();
        graph.add_pool(&pool("0xA", "WETH", "USDC", 1.0, 2000.0));
        graph.add_pool(&pool("0xB", "USDC", "DAI", 1000.0, 1000.0));
        graph.add_pool(&pool("0xC", "DAI", "WETH", 1900.0, 1.0));

        let cycles = graph.find_holonomic_cycles("WETH", 4, 1e-4);
        assert!(!cycles.is_empty(), "No cycles found");
        assert!(cycles[0].loop_cardinality >= 3);
        assert!(cycles[0].contour_integral().abs() > 1e-4);
    }

    #[test]
    fn invariant_checker_accepts_valid_cycle() {
        let contour = ClosedContourTrajectory {
            manifolds: vec![
                pool("0xA", "WETH", "USDC", 1.0, 2000.0),
                pool("0xB", "USDC", "DAI", 1000.0, 1000.0),
                pool("0xC", "DAI", "WETH", 1900.0, 1.0),
            ],
            transition_points: vec![
                Vector2::new(1.0, 2000.0),
                Vector2::new(1000.0, 1000.0),
                Vector2::new(1900.0, 1.0),
            ],
            relative_prices: vec![2000.0, 1.0, 1.0 / 1900.0],
            contour_length: 0.0,
            loop_cardinality: 3,
            start_token: "WETH".to_string(),
        };
        let yield_data = TopologicalYield::from_holonomy_and_friction(
            contour.contour_integral(),
            0.001,
            0.0005,
            vec!["0xA".to_string(), "0xB".to_string(), "0xC".to_string()],
        );
        let result = HolonomicInvariantChecker::verify(&contour, &yield_data, 100, 5000);
        assert!(result.is_ok());
    }

    /// OMEGA-8/M4 Fase 2: is_closed must NEVER panic, even on empty manifold
    /// vec. Previously a `first().unwrap()` would panic if the MIN_LOOP_SIZE
    /// guard was ever bypassed. The slice-pattern variant is panic-free.
    #[test]
    fn is_closed_returns_false_on_empty_manifold_vec() {
        let contour = ClosedContourTrajectory {
            manifolds: vec![],
            transition_points: vec![],
            relative_prices: vec![],
            contour_length: 0.0,
            loop_cardinality: 0,
            start_token: "WETH".to_string(),
        };
        assert!(!contour.is_closed());
    }

    /// OMEGA-8/M4 Fase 2: closure_check surfaces a typed error per failure
    /// mode (EmptyManifoldLoop, LoopTooShort, InvalidLoopBoundary). RULE 8
    /// requires explicit errors over silent falses.
    #[test]
    fn closure_check_empty_vec_returns_loop_too_short() {
        let contour = ClosedContourTrajectory {
            manifolds: vec![],
            transition_points: vec![],
            relative_prices: vec![],
            contour_length: 0.0,
            loop_cardinality: 0,
            start_token: "WETH".to_string(),
        };
        let err = contour.closure_check().expect_err("empty must error");
        assert!(matches!(err, ClosureError::LoopTooShort { actual: 0, .. }));
    }

    #[test]
    fn closure_check_too_short_returns_loop_too_short() {
        let contour = ClosedContourTrajectory {
            manifolds: vec![
                pool("0xA", "WETH", "USDC", 1.0, 2000.0),
                pool("0xB", "USDC", "WETH", 2000.0, 1.0),
            ],
            transition_points: vec![],
            relative_prices: vec![],
            contour_length: 0.0,
            loop_cardinality: 2,
            start_token: "WETH".to_string(),
        };
        let err = contour.closure_check().expect_err("len<3 must error");
        assert!(matches!(
            err,
            ClosureError::LoopTooShort { actual: 2, min: 3 }
        ));
    }

    #[test]
    fn closure_check_invalid_boundary_returns_invalid_loop_boundary() {
        let contour = ClosedContourTrajectory {
            manifolds: vec![
                pool("0xA", "WETH", "USDC", 1.0, 2000.0),
                pool("0xB", "USDC", "DAI", 1000.0, 1000.0),
                pool("0xC", "DAI", "USDT", 1.0, 1.0),
            ],
            transition_points: vec![],
            relative_prices: vec![],
            contour_length: 0.0,
            loop_cardinality: 3,
            start_token: "WETH".to_string(),
        };
        let err = contour.closure_check().expect_err("non-closing must error");
        assert!(matches!(err, ClosureError::InvalidLoopBoundary { .. }));
    }

    #[test]
    fn closure_check_valid_loop_is_ok() {
        let contour = ClosedContourTrajectory {
            manifolds: vec![
                pool("0xA", "WETH", "USDC", 1.0, 2000.0),
                pool("0xB", "USDC", "DAI", 1000.0, 1000.0),
                pool("0xC", "DAI", "WETH", 1900.0, 1.0),
            ],
            transition_points: vec![
                Vector2::new(1.0, 2000.0),
                Vector2::new(1000.0, 1000.0),
                Vector2::new(1900.0, 1.0),
            ],
            relative_prices: vec![2000.0, 1.0, 1.0 / 1900.0],
            contour_length: 0.0,
            loop_cardinality: 3,
            start_token: "WETH".to_string(),
        };
        assert!(contour.closure_check().is_ok());
        assert!(contour.is_closed());
    }

    #[test]
    fn invariant_checker_rejects_trivial_holonomy() {
        let contour = ClosedContourTrajectory {
            manifolds: vec![
                pool("0xA", "WETH", "USDC", 1.0, 2000.0),
                pool("0xB", "USDC", "DAI", 1000.0, 1000.0),
                pool("0xC", "DAI", "WETH", 2000.0, 1.0),
            ],
            transition_points: vec![
                Vector2::new(1.0, 2000.0),
                Vector2::new(1000.0, 1000.0),
                Vector2::new(2000.0, 1.0),
            ],
            relative_prices: vec![2000.0, 1.0, 1.0 / 2000.0],
            contour_length: 0.0,
            loop_cardinality: 3,
            start_token: "WETH".to_string(),
        };
        let yield_data = TopologicalYield::from_holonomy_and_friction(
            contour.contour_integral(),
            0.0,
            0.0,
            vec!["0xA".to_string()],
        );
        let result = HolonomicInvariantChecker::verify(&contour, &yield_data, 100, 5000);
        assert!(matches!(
            result,
            Err(InvariantViolation::TrivialHolonomy { .. })
        ));
    }
}
