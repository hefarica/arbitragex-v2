use crate::thermodynamics::ImpedanceSnapshot;

pub trait ImpedanceTensor: Send + Sync {
    fn dissipate(&self, chain_id: u64, hop_count: usize) -> ImpedanceSnapshot;
}
