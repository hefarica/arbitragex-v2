use crate::thermodynamics::{PermittedCycle, PotentialGradient};
use async_trait::async_trait;

#[async_trait]
pub trait PotentialField: Send + Sync {
    async fn sample(&self, token_in: &str, token_out: &str) -> Option<PotentialGradient>;
    async fn reconcile(&self, cycle: &PermittedCycle);
}
