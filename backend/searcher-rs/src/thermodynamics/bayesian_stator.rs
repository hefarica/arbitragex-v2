use crate::thermodynamics::{ImpedanceSnapshot, PotentialGradient, ThermodynamicCycle};

pub trait BayesianStator: Send + Sync {
    fn predict(
        &self,
        gradient: &PotentialGradient,
        impedance: &ImpedanceSnapshot,
    ) -> ThermodynamicCycle;
}
