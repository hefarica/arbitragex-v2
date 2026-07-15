use crate::thermodynamics::PotentialGradient;

pub trait LiquidityCurvature: Send + Sync {
    fn gradient(&self, source_token: &str) -> Vec<PotentialGradient>;
    fn update_edge(&mut self, edge: EdgeObservation);
}

#[derive(Debug, Clone)]
pub struct EdgeObservation {
    pub token_in: String,
    pub token_out: String,
    pub potential_delta_usd: f64,
    pub venue: String,
}
