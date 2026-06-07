// arbx-mev-ethics-gate: Typestate enforcement — JIT V3 inexpressible
use crate::mev_gate::{apply_mev_gate, GateVerdict};

let verdict = apply_mev_gate(
    route.depends_on_specific_pending_tx,
    route.worsens_user_outcome,
    route.protocol_explicitly_permits,
);

match verdict {
    GateVerdict::Permitted { reason } => {
        log::debug!("Route permitted: {}", reason);
    }
    GateVerdict::Prohibited { reason } => {
        log::warn!("Route rejected by MEV-ethics gate: {}", reason);
        return Ok(None);
    }
}
