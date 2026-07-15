use crate::thermodynamics::{
    bayesian_stator::BayesianStator, entropy_sink::EntropySink, impedance_tensor::ImpedanceTensor,
    liquidity_curvature::LiquidityCurvature, PermittedCycle,
};
use std::sync::Arc;

pub struct CarnotOrchestrator {
    curvature: Arc<dyn LiquidityCurvature>,
    stator: Arc<dyn BayesianStator>,
    impedance: Arc<dyn ImpedanceTensor>,
    sink: EntropySink,
    source_tokens: Vec<String>,
}

impl CarnotOrchestrator {
    pub fn new(
        curvature: Arc<dyn LiquidityCurvature>,
        stator: Arc<dyn BayesianStator>,
        impedance: Arc<dyn ImpedanceTensor>,
        sink: EntropySink,
        source_tokens: Vec<String>,
    ) -> Self {
        Self {
            curvature,
            stator,
            impedance,
            sink,
            source_tokens,
        }
    }

    pub fn cycle(&self) -> Vec<PermittedCycle> {
        let mut permitted = Vec::new();
        for source in &self.source_tokens {
            for gradient in self.curvature.gradient(source) {
                let impedance = self.impedance.dissipate(1, 2);
                let thermo = self.stator.predict(&gradient, &impedance);
                if let Ok(cycle) = self.sink.filter(thermo) {
                    permitted.push(cycle);
                }
            }
        }
        permitted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermodynamics::{
        liquidity_curvature::EdgeObservation, CycleStatus, ImpedanceSnapshot, PotentialGradient,
        ThermodynamicCycle,
    };

    struct FixedCurvature {
        gradients: Vec<PotentialGradient>,
    }

    impl LiquidityCurvature for FixedCurvature {
        fn gradient(&self, _source_token: &str) -> Vec<PotentialGradient> {
            self.gradients.clone()
        }
        fn update_edge(&mut self, _edge: EdgeObservation) {}
    }

    struct FixedStator;
    impl BayesianStator for FixedStator {
        fn predict(
            &self,
            gradient: &PotentialGradient,
            impedance: &ImpedanceSnapshot,
        ) -> ThermodynamicCycle {
            ThermodynamicCycle {
                gradient: gradient.clone(),
                heat_in_usd: gradient.potential_delta_usd,
                impedance: impedance.clone(),
            }
        }
    }

    struct FixedImpedance;
    impl ImpedanceTensor for FixedImpedance {
        fn dissipate(&self, _chain_id: u64, _hop_count: usize) -> ImpedanceSnapshot {
            ImpedanceSnapshot {
                gas_usd: 0.5,
                fee_bps: 30,
                latency_ms: 50,
                decoherence_usd: 0.1,
            }
        }
    }

    fn sample_gradient(delta: f64) -> PotentialGradient {
        PotentialGradient {
            token_in: "WETH".to_string(),
            token_out: "USDC".to_string(),
            potential_delta_usd: delta,
            venue_in: "uniswap_v3".to_string(),
            venue_out: "binance".to_string(),
        }
    }

    #[test]
    fn positive_work_cycle_is_permitted() {
        let curvature = Arc::new(FixedCurvature {
            gradients: vec![sample_gradient(10.0)],
        });
        let orchestrator = CarnotOrchestrator::new(
            curvature,
            Arc::new(FixedStator),
            Arc::new(FixedImpedance),
            EntropySink::new(0.01),
            vec!["WETH".to_string()],
        );
        let cycles = orchestrator.cycle();
        assert_eq!(cycles.len(), 1);
        assert!(cycles[0].work_extracted_usd > 0.0);
        assert!(matches!(cycles[0].status, CycleStatus::Detected));
    }

    #[test]
    fn negative_work_cycle_is_rejected() {
        let curvature = Arc::new(FixedCurvature {
            gradients: vec![sample_gradient(0.1)],
        });
        let orchestrator = CarnotOrchestrator::new(
            curvature,
            Arc::new(FixedStator),
            Arc::new(FixedImpedance),
            EntropySink::new(0.01),
            vec!["WETH".to_string()],
        );
        assert!(orchestrator.cycle().is_empty());
    }

    #[test]
    fn below_min_eta_is_rejected() {
        // heat_in=0.7, heat_out=0.6, eta=0.142, so min_eta=0.2 rejects
        let curvature = Arc::new(FixedCurvature {
            gradients: vec![sample_gradient(0.7)],
        });
        let orchestrator = CarnotOrchestrator::new(
            curvature,
            Arc::new(FixedStator),
            Arc::new(FixedImpedance),
            EntropySink::new(0.2),
            vec!["WETH".to_string()],
        );
        assert!(orchestrator.cycle().is_empty());
    }
}
