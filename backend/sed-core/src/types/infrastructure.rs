//! `InfrastructurePrerequisite` + 501-shaped `NotImplementedResponse`
//! (spec §4.3).
//!
//! Each SED module declares the upstream services it needs (e.g.
//! `mempool-node`, `anvil-node`, `flashbots-relay`). At gate time the
//! prerequisite verifies all required services are healthy; if any aren't,
//! the pipeline emits a 501-shaped response naming which services failed
//! and which sprint is supposed to make them available. The HTTP layer
//! consumes this struct directly to return `501 Not Implemented` to the
//! client with a structured body.

use serde::{Deserialize, Serialize};

/// Per-module infrastructure prerequisite.
///
/// Constructed once per pipeline gate (e.g., `G1`, `G2`, …) and consulted
/// before that gate's module runs. The `verify` method is async because
/// each `required_services` lookup is an HTTP probe to the service's
/// `/health` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructurePrerequisite {
    /// Service IDs in the canonical registry (must match the names in
    /// `monitoring/grafana/dashboards/*.json` and the
    /// `docs/governance/ROADMAP_FASES.md` mapping).
    pub required_services: Vec<String>,

    /// The minimum sprint at which these services are expected to be
    /// available. Used in the 501 response so operators can correlate the
    /// gap with their sprint plan.
    pub minimum_sprint: String,

    /// Cached fallback response used when verification fails. Pre-built so
    /// the error path is allocation-free.
    pub fallback_response: NotImplementedResponse,
}

/// 501 Not Implemented response body. Honors the repo-wide convention of
/// surfacing explicit missing-infrastructure deficits rather than silent
/// degradation. Matches the existing `[OK] [PARCIAL] [PENDIENTE] [BLOQUEADO]`
/// honesty discipline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotImplementedResponse {
    /// Services the pipeline requires but cannot reach.
    pub requires: Vec<String>,

    /// Sprint label (e.g., `"S2"`, `"S4"`). Frontends can render this in
    /// the operator UI to direct attention to the right plan item.
    pub sprint: String,

    /// Human-readable message. Format: `"SED module requires infrastructure
    /// not yet available in <sprint>"`.
    pub message: String,
}

impl InfrastructurePrerequisite {
    /// Verify all required services are healthy.
    ///
    /// Phase 1 stub: returns `Ok(())` only when `required_services` is
    /// empty; otherwise returns the cached `fallback_response`. The real
    /// health-probe integration lands in Phase 2 when this gate wires to
    /// the shared `HealthChecker`.
    ///
    /// # Errors
    ///
    /// - `NotImplementedResponse` describing which services failed health
    ///   and which sprint should supply them.
    pub async fn verify(&self) -> Result<(), NotImplementedResponse> {
        if self.required_services.is_empty() {
            return Ok(());
        }
        // Phase 2 will replace this with a real `HealthChecker` round-trip
        // per service. Stub: assume all listed services are unhealthy so
        // the pipeline degrades to 501 — fail-honest default.
        Err(self.fallback_response.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_prereq(services: Vec<&str>, sprint: &str) -> InfrastructurePrerequisite {
        let requires: Vec<String> = services.iter().map(|s| (*s).to_string()).collect();
        let sprint = sprint.to_string();
        InfrastructurePrerequisite {
            required_services: requires.clone(),
            minimum_sprint: sprint.clone(),
            fallback_response: NotImplementedResponse {
                requires,
                sprint: sprint.clone(),
                message: format!(
                    "SED module requires infrastructure not yet available in {sprint}"
                ),
            },
        }
    }

    #[tokio::test]
    async fn empty_requirements_pass() {
        let p = make_prereq(vec![], "S2");
        assert!(p.verify().await.is_ok());
    }

    #[tokio::test]
    async fn non_empty_requirements_return_501_shape() {
        let p = make_prereq(vec!["mempool-node", "ws-conn"], "S2");
        let err = p.verify().await.expect_err("must return 501");
        assert_eq!(err.requires, vec!["mempool-node", "ws-conn"]);
        assert_eq!(err.sprint, "S2");
        assert!(err.message.contains("S2"));
    }

    #[test]
    fn response_serialises_to_stable_json() {
        let r = NotImplementedResponse {
            requires: vec!["anvil-node".into()],
            sprint: "S4".into(),
            message: "SED module requires infrastructure not yet available in S4".into(),
        };
        let j = serde_json::to_string(&r).expect("serialise");
        assert!(j.contains("\"requires\""));
        assert!(j.contains("\"sprint\":\"S4\""));
    }
}
