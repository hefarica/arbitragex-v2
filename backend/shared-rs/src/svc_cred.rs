//! svc_cred — RunFullSyncCycle FASE 3b: runtime credential resolution with
//! projection precedence.
//!
//! `arbx:svc_cred:<provider>:<scope>` is the PG→Redis projection of the
//! credentials store (`service_credentials`, migration 057): api-server SETs
//! it on every manual PUT / bulk row and re-hydrates the whole mirror at boot
//! (see api-server `credentials/projection.ts`). Runtime consumers resolve
//! credentials with this precedence:
//!
//!   projection (Redis)  →  legacy `.env` fallback   — never the reverse.
//!
//! Boot logs `event="cred.source" provider=… source=projection|env` so the
//! operator can see, per credential, where each consumer resolved it from.
//! Fail-honest: a Redis error or a malformed/empty projection falls through
//! to env silently at the resolution layer (the caller still logs the source).

use redis::aio::ConnectionManager;
use serde::Deserialize;

/// Mirror of the api-server projection JSON (`ProjectedCredential`).
#[derive(Debug, Clone, Deserialize)]
pub struct SvcCredProjection {
    pub provider: String,
    pub scope: String,
    pub secret_value: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub updated_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredSource {
    Projection,
    Env,
}

impl CredSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            CredSource::Projection => "projection",
            CredSource::Env => "env",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedCred {
    pub value: String,
    pub source: CredSource,
    /// Projection status ("valid"/"invalid"/…) when resolved from the mirror.
    pub status: Option<String>,
    /// Projection metadata (e.g. titan's `url`) when resolved from the mirror.
    pub metadata: Option<serde_json::Value>,
}

pub fn svc_cred_key(provider: &str, scope: &str) -> String {
    format!("arbx:svc_cred:{provider}:{scope}")
}

/// Pure resolution core (unit-testable): projection JSON first, env fallback.
/// An empty `secret_value` or a malformed projection falls through to env —
/// the projection never yields a value the operator did not persist.
pub fn resolve_from_parts(
    projection_raw: Option<&str>,
    env_value: Option<String>,
) -> Option<ResolvedCred> {
    if let Some(raw) = projection_raw {
        if let Ok(p) = serde_json::from_str::<SvcCredProjection>(raw) {
            if !p.secret_value.is_empty() {
                return Some(ResolvedCred {
                    value: p.secret_value,
                    source: CredSource::Projection,
                    status: p.status,
                    metadata: if p.metadata.is_null() {
                        None
                    } else {
                        Some(p.metadata)
                    },
                });
            }
        }
    }
    env_value
        .filter(|v| !v.is_empty())
        .map(|value| ResolvedCred {
            value,
            source: CredSource::Env,
            status: None,
            metadata: None,
        })
}

/// Redis-backed resolution (workspace redis). A transient Redis error simply
/// misses the projection and falls to env — callers log the resulting source.
pub async fn resolve(
    redis: &ConnectionManager,
    env_name: &str,
    provider: &str,
    scope: &str,
) -> Option<ResolvedCred> {
    let mut con = redis.clone();
    use redis::AsyncCommands;
    let raw: Option<String> = con.get(svc_cred_key(provider, scope)).await.ok();
    let env = std::env::var(env_name).ok().filter(|s| !s.is_empty());
    resolve_from_parts(raw.as_deref(), env)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection_json(secret: &str, status: &str) -> String {
        format!(
            r#"{{"provider":"alchemy_prices","scope":"global","secret_value":"{secret}","metadata":{{}},"status":"{status}","updated_at":"2026-08-17T00:00:00Z","updated_by":"admin"}}"#
        )
    }

    #[test]
    fn key_format() {
        assert_eq!(
            svc_cred_key("github_token", "global"),
            "arbx:svc_cred:github_token:global"
        );
        assert_eq!(
            svc_cred_key("rpc_http", "chain:1"),
            "arbx:svc_cred:rpc_http:chain:1"
        );
    }

    #[test]
    fn projection_wins_over_env() {
        let r = resolve_from_parts(
            Some(&projection_json("KEY123", "valid")),
            Some("ENVKEY".into()),
        )
        .unwrap();
        assert_eq!(r.value, "KEY123");
        assert_eq!(r.source, CredSource::Projection);
        assert_eq!(r.status.as_deref(), Some("valid"));
    }

    #[test]
    fn env_fallback_when_projection_missing() {
        let r = resolve_from_parts(None, Some("ENVKEY".into())).unwrap();
        assert_eq!(r.value, "ENVKEY");
        assert_eq!(r.source, CredSource::Env);
        assert!(r.status.is_none());
    }

    #[test]
    fn env_fallback_on_malformed_or_empty_projection() {
        // Malformed JSON → env.
        let r = resolve_from_parts(Some("not json"), Some("ENVKEY".into())).unwrap();
        assert_eq!(r.source, CredSource::Env);
        // Empty secret in the projection → env (never fabricate a value).
        let empty = projection_json("", "invalid");
        let r2 = resolve_from_parts(Some(&empty), Some("ENVKEY".into())).unwrap();
        assert_eq!(r2.source, CredSource::Env);
    }

    #[test]
    fn none_when_both_sources_empty() {
        assert!(resolve_from_parts(None, None).is_none());
        assert!(resolve_from_parts(None, Some("".into())).is_none());
        assert!(resolve_from_parts(Some(&projection_json("", "untested")), None).is_none());
    }

    #[test]
    fn source_strings() {
        assert_eq!(CredSource::Projection.as_str(), "projection");
        assert_eq!(CredSource::Env.as_str(), "env");
    }
}
