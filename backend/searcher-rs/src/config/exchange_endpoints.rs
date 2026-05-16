//! Exchange endpoint configuration — sourced exclusively from environment variables.
//!
//! ## Doctrine: No-Hardcode
//!
//! Exchange base URLs are **B. URL operativa (CEX endpoint)** in Hector's
//! taxonomy. They are operational endpoints subject to change (geo-restrictions,
//! domain migrations, sandbox vs. production toggle) and MUST NOT be hardcoded
//! as silent fallbacks. The system must fail-fast on missing env rather than
//! silently connect to a potentially unintended production CEX endpoint.
//!
//! ## Usage
//!
//! Call `ExchangeEndpoints::from_env()` at worker boot. On missing env vars
//! the function returns `Err` — callers MUST propagate or abort the worker.
//!
//! For tests, construct `ExchangeEndpoints::for_test(binance, okx)` directly.
//!
//! ## Environment variables
//!
//! | Variable               | Example value                    | Notes                          |
//! |------------------------|----------------------------------|--------------------------------|
//! | `CEX_BINANCE_BASE_URL` | `https://api.binance.com`        | No trailing slash              |
//! | `CEX_OKX_BASE_URL`     | `https://www.okx.com`            | No trailing slash              |

/// Resolved CEX base URLs, sourced exclusively from environment variables.
///
/// Construct via [`ExchangeEndpoints::from_env`] in production code.
/// Use [`ExchangeEndpoints::for_test`] only inside `#[cfg(test)]` blocks.
#[derive(Debug, Clone)]
pub struct ExchangeEndpoints {
    /// Base URL for Binance REST API, e.g. `https://api.binance.com`.
    pub binance_base_url: String,
    /// Base URL for OKX REST API, e.g. `https://www.okx.com`.
    pub okx_base_url: String,
}

impl ExchangeEndpoints {
    /// Load endpoints from environment variables.
    ///
    /// Fails fast with a descriptive error if any required variable is absent
    /// or empty. There is NO silent fallback to a hardcoded value —
    /// doctrine: "POR EL HAZ DE LUZ SOLO PASA QUIEN ES VISTO".
    ///
    /// # Errors
    ///
    /// Returns `Err` if `CEX_BINANCE_BASE_URL` or `CEX_OKX_BASE_URL` are
    /// not set (or empty) in the process environment.
    pub fn from_env() -> anyhow::Result<Self> {
        let binance = load_env_url("CEX_BINANCE_BASE_URL")?;
        let okx = load_env_url("CEX_OKX_BASE_URL")?;
        Ok(Self {
            binance_base_url: binance,
            okx_base_url: okx,
        })
    }

    /// Construct directly from known values — **use only in `#[cfg(test)]`**.
    ///
    /// In tests, inject a wiremock / httptest base URL instead of the real
    /// CEX endpoint to avoid live network calls in unit tests.
    #[cfg(test)]
    pub fn for_test(binance_base_url: impl Into<String>, okx_base_url: impl Into<String>) -> Self {
        Self {
            binance_base_url: binance_base_url.into(),
            okx_base_url: okx_base_url.into(),
        }
    }
}

/// Load a URL from a named environment variable, failing fast if absent/empty.
fn load_env_url(var: &str) -> anyhow::Result<String> {
    match std::env::var(var) {
        Ok(v) if !v.trim().is_empty() => Ok(v.trim_end_matches('/').to_string()),
        Ok(_) => anyhow::bail!(
            "exchange endpoint config: env var `{var}` is set but empty. \
             Set it to the appropriate CEX base URL."
        ),
        Err(_) => anyhow::bail!(
            "exchange endpoint config: env var `{var}` is not set. \
             Add it to your .env file or deployment secrets. \
             See docs/governance/NO-HARDCODE-DOCTRINE.md §B."
        ),
    }
}
