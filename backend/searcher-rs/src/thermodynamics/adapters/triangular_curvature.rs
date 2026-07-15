use crate::engines::triangular_engine::TriangularEngine;
use crate::thermodynamics::liquidity_curvature::{EdgeObservation, LiquidityCurvature};
use crate::thermodynamics::PotentialGradient;
use std::sync::Arc;

pub struct TriangularCurvatureAdapter {
    #[allow(dead_code)]
    engine: Arc<TriangularEngine>,
}

impl TriangularCurvatureAdapter {
    pub fn new(engine: Arc<TriangularEngine>) -> Self {
        Self { engine }
    }
}

impl LiquidityCurvature for TriangularCurvatureAdapter {
    fn gradient(&self, _source_token: &str) -> Vec<PotentialGradient> {
        // TODO(scaffold): wire to TriangularEngine cycle detection
        Vec::new()
    }

    fn update_edge(&mut self, _edge: EdgeObservation) {
        // TODO(scaffold): refresh impacted cycles
    }
}
