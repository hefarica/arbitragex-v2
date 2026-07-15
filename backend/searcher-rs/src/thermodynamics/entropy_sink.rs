use crate::thermodynamics::{CycleStatus, DissipationMetrics, EntropyError, PermittedCycle, PotentialGradient, ThermodynamicCycle};
use uuid::Uuid;

pub struct EntropySink {
    min_eta: f64,
}

impl EntropySink {
    pub fn new(min_eta: f64) -> Self {
        Self { min_eta }
    }

    pub fn filter(&self, cycle: ThermodynamicCycle) -> Result<PermittedCycle, EntropyError> {
        let heat_out = cycle.impedance.gas_usd + cycle.impedance.decoherence_usd;
        let work_extracted = cycle.heat_in_usd - heat_out;
        if work_extracted <= 0.0 {
            return Err(EntropyError::NegativeWork);
        }
        let eta = work_extracted / cycle.heat_in_usd;
        if eta < self.min_eta {
            return Err(EntropyError::SecondLawViolation);
        }
        Ok(PermittedCycle {
            id: Uuid::new_v4().to_string(),
            chain_id: 1,
            detected_at: chrono::Utc::now(),
            eta,
            work_extracted_usd: work_extracted,
            heat_in_usd: cycle.heat_in_usd,
            heat_out_usd: heat_out,
            gradient: PotentialGradient {
                token_in: cycle.gradient.token_in.clone(),
                token_out: cycle.gradient.token_out.clone(),
                potential_delta_usd: cycle.gradient.potential_delta_usd,
                venue_in: cycle.gradient.venue_in.clone(),
                venue_out: cycle.gradient.venue_out.clone(),
            },
            dissipation: DissipationMetrics {
                gas_usd: cycle.impedance.gas_usd,
                fee_bps: cycle.impedance.fee_bps,
                latency_ms: cycle.impedance.latency_ms,
                decoherence_usd: cycle.impedance.decoherence_usd,
            },
            status: CycleStatus::Detected,
            rejection_reason: None,
        })
    }
}
