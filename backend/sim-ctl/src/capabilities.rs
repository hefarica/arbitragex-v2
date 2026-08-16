//! GET /capabilities — G-SIM-1 FASE 1. Reports the REAL state of the
//! simulator: the single source of topological truth the readiness verifier
//! consumes. RULE 00 Zero-Mocks: every field derives from build reality or a
//! request-time env read; anything underivable is an honest null.

use axum::{http::StatusCode, response::IntoResponse, Json};

/// Route path served by the bin's `sim_router` and asserted by the tests below
/// — a shared const so the registration and the test cannot drift apart.
pub const CAPABILITIES_PATH: &str = "/capabilities";

/// Canonical env name of the v2 dispatch gate.
pub const SIMULATOR_V2_GATE_ENV: &str = "ARBX_USE_SIMULATOR_V2";

/// Pure derivation of the dispatch-gate state from a raw env value, so the
/// parsing rule is unit-testable without mutating process env (tests run
/// multi-threaded; env mutation would race).
///
/// PARITY: the canonical consumer of this gate is searcher-rs, which parses
/// the same env with `eq_ignore_ascii_case("true")`
/// (backend/searcher-rs/src/main.rs:359-361). sim-ctl must dispatch-enable on
/// exactly the same inputs — stricter or laxer parsing here would make this
/// report disagree with the consumer it describes.
fn v2_dispatch_enabled(raw: Option<&str>) -> bool {
    raw.is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

/// GET /capabilities response — the G-SIM-1 verifier contract.
///
/// RULE 00 derivation, field by field:
/// - `simulator_backend` / `modules` / `build.features`: relayed verbatim
///   from `simulator_v2::capabilities()` + `simulator_v2::BACKEND_TAG`. This
///   is a BUILD-REALITY claim backed by linkage: the call only compiles
///   because simulator-v2 is an unconditional path dependency of sim-ctl (no
///   cargo feature gate, not `optional`). Dropping the dependency breaks this
///   build rather than yielding a stale "v2". NOTE: this field reports what
///   the binary CONTAINS, not the runtime `SIM_BACKEND` (anvil|revm) switch
///   that chooses which backend serves /simulate.
/// - `build.sha`: `git rev-parse HEAD` of this checkout, captured at compile
///   time by `build.rs`; when git is unavailable (docker build without .git)
///   it falls back to the ambient `ARBX_BUILD_SHA` env var so CI/docker can
///   inject the SHA; when NEITHER exists it stays null — never fabricated.
/// - `build.revm_version` / `build.alloy_primitives_version`: the versions
///   RESOLVED for the workspace, scanned out of `../Cargo.lock` by `build.rs`
///   (the crates themselves expose no version const downstream code can
///   read). alloy-primitives resolves to multiple copies in the lockfile;
///   the disambiguation rule (the copy simulator-v2 itself links) lives in
///   `build.rs`. Unresolvable → null.
/// - `dispatch_gate`: `ARBX_USE_SIMULATOR_V2` read AT REQUEST TIME. The
///   canonical consumer of this env is searcher-rs (see the activation header
///   in simulator-v2/src/lib.rs); sim-ctl itself selects its /simulate
///   backend via SIM_BACKEND. Compose must pass ARBX_USE_SIMULATOR_V2 to
///   BOTH services for this view to be truthful — today it does NOT (honest
///   gap to fix in FASE 2, not to hide here).
/// - `fork_suite`: null — fed by the readiness_evidence registry (FASE 2),
///   wired in FASE 3.
///
/// Keys are snake_case to match the G-SIM-1 verifier contract exactly.
#[derive(Debug, serde::Serialize)]
struct CapabilitiesResponse {
    simulator_backend: &'static str,
    build: BuildCapabilities,
    modules: &'static [&'static str],
    dispatch_gate: DispatchGate,
    /// FASE 3 shape (documented, not yet emitted): `{"last_run_ts": ...,
    /// "last_run_green": ..., "block": ...}` from the readiness_evidence
    /// registry. Serialized as JSON null until then.
    fork_suite: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
struct BuildCapabilities {
    sha: Option<&'static str>,
    features: Vec<&'static str>,
    revm_version: Option<&'static str>,
    alloy_primitives_version: Option<&'static str>,
}

#[derive(Debug, serde::Serialize)]
struct DispatchGate {
    env: &'static str,
    active: bool,
}

/// Build the /capabilities payload from build reality + request-time env.
fn capabilities_report() -> CapabilitiesResponse {
    let caps = simulator_v2::capabilities();
    CapabilitiesResponse {
        simulator_backend: simulator_v2::BACKEND_TAG,
        build: BuildCapabilities {
            // Compile-time facts emitted by build.rs (see the field-by-field
            // doc above): real derivations, honest None when underivable.
            sha: option_env!("ARBX_BUILD_SHA"),
            features: caps.features.clone(),
            revm_version: option_env!("GSIM_REVM_VERSION"),
            alloy_primitives_version: option_env!("GSIM_ALLOY_PRIMITIVES_VERSION"),
        },
        modules: caps.modules,
        dispatch_gate: DispatchGate {
            env: SIMULATOR_V2_GATE_ENV,
            // Read at REQUEST time so the view tracks operator action without
            // a restart (canonical consumer: searcher-rs — see doc above).
            active: v2_dispatch_enabled(std::env::var(SIMULATOR_V2_GATE_ENV).ok().as_deref()),
        },
        fork_suite: serde_json::Value::Null,
    }
}

/// GET /capabilities — always 200: the payload IS the state (including honest
/// nulls); there is no error branch to hide behind.
pub async fn capabilities_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(capabilities_report()))
}

/// Bare router for this stateless route — used by the tests below with
/// `oneshot`; production mounts the same handler at [`CAPABILITIES_PATH`] in
/// the bin's full router.
#[cfg(test)]
fn capabilities_router() -> axum::Router {
    use axum::{routing::get, Router};
    Router::new().route(CAPABILITIES_PATH, get(capabilities_handler))
}

// ---------------------------------------------------------------------------
// Tests — G1 gate for GET /capabilities (G-SIM-1 FASE 1)
// ---------------------------------------------------------------------------

// Runs in CI via `cargo test --workspace --lib` (lib target). Local caveat:
// `cargo test` execution is blocked on this Windows host (AppControl os error
// 4551) — compiled locally, executed by CI.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;

    /// Null (underivable at build time) OR a git-object SHA (40-64 hex
    /// chars). Shape-only: never asserts a literal so the test holds on any
    /// build machine.
    fn assert_optional_sha(v: &serde_json::Value, field: &str) {
        match v.as_str() {
            None => assert!(v.is_null(), "{field} must be null or sha-shaped, got {v}"),
            Some(s) => assert!(
                (40..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit()),
                "{field} must be null or sha-shaped, got {s:?}"
            ),
        }
    }

    /// Null (unresolvable) OR a semver-shaped string (MAJOR.MINOR[.PATCH]
    /// with optional pre-release/build). Shape-only by the same rule.
    fn assert_optional_semver(v: &serde_json::Value, field: &str) {
        match v.as_str() {
            None => assert!(
                v.is_null(),
                "{field} must be null or semver-shaped, got {v}"
            ),
            Some(s) => assert!(
                s.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && s.chars()
                        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+'))
                    && s.contains('.'),
                "{field} must be null or semver-shaped, got {s:?}"
            ),
        }
    }

    /// GET /capabilities, G1 gate: 200, parseable JSON, contract shape, and
    /// values that MATCH what the build actually compiled (relayed from
    /// simulator_v2, never hand-written here).
    #[tokio::test]
    async fn capabilities_returns_200_and_matches_build_reality() {
        let app = capabilities_router();
        let req = Request::builder()
            .uri(CAPABILITIES_PATH)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Exact top-level contract consumed by the G-SIM-1 readiness verifier.
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "build",
                "dispatch_gate",
                "fork_suite",
                "modules",
                "simulator_backend"
            ],
            "top-level keys must match the verifier contract exactly"
        );

        // simulator_backend: valid enum value AND consistent with the crate
        // this binary actually links.
        let backend = json["simulator_backend"].as_str().unwrap();
        assert!(
            backend == "v1" || backend == "v2",
            "simulator_backend must be a valid enum value, got {backend:?}"
        );
        assert_eq!(
            backend,
            simulator_v2::BACKEND_TAG,
            "endpoint must relay the linked crate's tag, not a hand-written one"
        );

        // modules: non-empty AND faithful to simulator_v2's compile-time list.
        let modules = json["modules"].as_array().unwrap();
        assert!(!modules.is_empty(), "modules must be non-empty");
        let expected: Vec<&str> = simulator_v2::capabilities().modules.to_vec();
        let reported: Vec<&str> = modules.iter().map(|m| m.as_str().unwrap()).collect();
        assert_eq!(
            reported, expected,
            "endpoint must relay simulator_v2::capabilities().modules verbatim"
        );

        // build: sha/version fields are DERIVED facts (build.rs: git SHA +
        // Cargo.lock scan) — asserted shape-based (null OR correctly shaped),
        // never pinned to this machine's environment, so CI builds with or
        // without git history both stay valid.
        let build = &json["build"];
        assert_optional_sha(&build["sha"], "build.sha");
        assert_optional_semver(&build["revm_version"], "build.revm_version");
        assert_optional_semver(
            &build["alloy_primitives_version"],
            "build.alloy_primitives_version",
        );
        let reported_features: Vec<&str> = build["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap())
            .collect();
        assert_eq!(
            reported_features,
            simulator_v2::capabilities().features,
            "features must be relayed from the cfg!-derived list"
        );

        // dispatch_gate: canonical env name + active flag consistent with the
        // searcher-rs-parity parsing rule evaluated against the live process
        // env.
        let gate = &json["dispatch_gate"];
        assert_eq!(gate["env"].as_str().unwrap(), SIMULATOR_V2_GATE_ENV);
        assert_eq!(
            gate["active"].as_bool().unwrap(),
            v2_dispatch_enabled(std::env::var(SIMULATOR_V2_GATE_ENV).ok().as_deref()),
            "active must mirror the request-time env read"
        );

        // fork_suite: null until the readiness_evidence registry is wired (FASE 3).
        assert!(json["fork_suite"].is_null());
    }

    /// The dispatch-gate derivation matches searcher-rs's canonical parsing
    /// (`eq_ignore_ascii_case("true")`): case-insensitive "true" enables;
    /// anything else — including "yes", "1", "" — does not.
    #[test]
    fn v2_dispatch_gate_searcher_parity_rule() {
        assert!(v2_dispatch_enabled(Some("true")));
        assert!(v2_dispatch_enabled(Some("TRUE")));
        assert!(v2_dispatch_enabled(Some("True")));
        assert!(!v2_dispatch_enabled(None));
        assert!(!v2_dispatch_enabled(Some("false")));
        assert!(!v2_dispatch_enabled(Some("yes")));
        assert!(!v2_dispatch_enabled(Some("1")));
        assert!(!v2_dispatch_enabled(Some("")));
    }
}
