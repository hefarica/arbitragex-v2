//! Monthly token-logo refresh (RU-TOKEN-REFRESH).
//!
//! Gap this closes: the 5-min reconciliation loop only retries tokens WITHOUT
//! a logo (`logo_url IS NULL` or TTL-expired `resolved_via = 'failed'`).
//! Tokens that already HAVE a logo are never re-verified — when TrustWallet
//! replaces an asset (re-brand) or removes it, the stored `logo_url` stays
//! frozen forever.
//!
//! This job walks EVERY logo-bearing token and HEAD-verifies its stored URL:
//!   - 2xx + Content-Length unchanged ... skip (`unchanged`); baseline kept.
//!   - 2xx + Content-Length changed ..... re-download to confirm, re-affirm
//!     `logo_url` + `last_seen_at` in PG, invalidate the Redis icon hot-cache
//!     key (`updated`). The URL itself is path-immutable on the TrustWallet
//!     CDN — what changed is the asset behind it, so the row is re-affirmed
//!     (idempotent UPDATE) and readers re-fetch after the cache DEL.
//!   - 404/410 ........................... the logo died: `logo_url = NULL` +
//!     `resolved_via = 'failed'` (`dead`). The reconciliation loop re-resolves
//!     it on a later tick (the 7-day retry TTL is long expired for rows this
//!     old, so `resolved_at` is intentionally NOT touched).
//!   - transport error / 403 / 5xx ........ `errors` — NEVER treated as dead.
//!     A GitHub rate-limit blip must not mass-NULL the logo table (fail-safe).
//!
//! ## Change detection (why Redis keeps the baseline)
//!
//! PG has no column for the last-seen logo size and the TrustWallet CDN URL
//! for an address is path-immutable (same URL, new bytes on re-brand). The
//! baseline `Content-Length` therefore lives in the Redis hash
//! `arbx:token-logo-refresh:sizes` (field `<chain_id>:<address>`, no TTL — it
//! must survive the monthly window). The first observation of a token stores
//! its baseline and counts as `unchanged`; every later run compares against
//! it. Redis being down degrades the job to a pure liveness check (404
//! detection still works) — fail-honest, never fatal.
//!
//! ## Rate limiting
//!
//! Probes run sequentially in batches of [`REFRESH_BATCH_SIZE`] with
//! [`BATCH_PAUSE`] between batches (rate-limit safe).
//!
//! ## Fail-safe
//!
//! No global transaction: each token's PG row is updated individually. If the
//! job dies mid-run, every logo verified so far is already persisted and the
//! rest keep serving their existing URLs.
//!
//! ## Doctrine
//!
//! Read-only with respect to capital: this job only verifies public CDN URLs
//! and writes PG/Redis metadata. No signer, no broadcast.

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::trustwallet::TrustWalletClient;

/// Tokens per probe batch before the inter-batch pause.
pub const REFRESH_BATCH_SIZE: usize = 50;
/// Pause between batches (rate-limit safety).
pub const BATCH_PAUSE: Duration = Duration::from_secs(2);
/// Default tick interval: 30 days.
pub const DEFAULT_LOGO_REFRESH_INTERVAL_SECS: u64 = 2_592_000;
/// Env var for the tick interval; `0` disables the job.
pub const ENV_LOGO_REFRESH_INTERVAL_SECS: &str = "LOGO_REFRESH_INTERVAL_SECS";
/// Redis hash: field `<chain_id>:<address>` -> last-seen Content-Length.
/// No TTL — the baseline must survive the monthly window.
const SIZES_HASH: &str = "arbx:token-logo-refresh:sizes";

// ---------------------------------------------------------------------------
// HTTP surface (prod: reqwest via TrustWalletClient; tests: scripted mocks)
// ---------------------------------------------------------------------------

/// HEAD probe result for one logo URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeadInfo {
    pub status: u16,
    pub content_length: Option<u64>,
}

/// HTTP probing surface used by the refresh loop.
///
/// The production implementation is [`TrustWalletClient`] (reqwest). Tests
/// provide scripted implementations — there are NO runtime mocks (RULE 00).
///
/// Generic-only trait: `async fn` in a trait is not dyn-compatible; callers
/// instantiate it statically (`run_logo_refresh::<TrustWalletClient>`). The
/// futures are always awaited in place by the generic probe loop (never
/// spawned across tasks), so no explicit `Send` bound is required — hence the
/// allow for the `async_fn_in_trait` lint.
#[allow(async_fn_in_trait)]
pub trait LogoHttp: Send + Sync {
    /// HEAD the exact stored logo URL (any host). `Err` = transport
    /// failure/timeout — the caller treats it as a probe error, never as dead.
    async fn head_logo(&self, url: &str) -> Result<HeadInfo>;

    /// Re-download (GET) the URL; returns the body byte count. `Err` on
    /// transport failure or non-2xx status.
    async fn download_logo(&self, url: &str) -> Result<u64>;
}

impl LogoHttp for TrustWalletClient {
    async fn head_logo(&self, url: &str) -> Result<HeadInfo> {
        let (status, content_length) = self.head_url(url).await?;
        Ok(HeadInfo {
            status,
            content_length,
        })
    }

    async fn download_logo(&self, url: &str) -> Result<u64> {
        self.download_url(url).await
    }
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

/// Counters for one refresh run. Invariant: `checked` == `updated` +
/// `unchanged` + `dead` + `errors` (every attempted probe lands in exactly one
/// bucket).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshStats {
    pub checked: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub dead: usize,
    pub errors: usize,
}

// ---------------------------------------------------------------------------
// Pure classification
// ---------------------------------------------------------------------------

/// Pure classification of one HEAD response (no I/O) — unit-testable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeadVerdict {
    /// 404/410 — the logo URL is gone.
    Dead,
    /// Non-2xx non-404 status (403 rate-limit, 429, 5xx) — leave the row alone.
    Unhealthy,
    /// 2xx; store/keep the baseline when `Some`.
    Unchanged { content_length: Option<u64> },
    /// 2xx with a Content-Length that differs from the baseline — caller must
    /// confirm with a re-download before acting.
    Changed { content_length: u64 },
}

/// Classify a HEAD response against the stored baseline.
fn classify_head(info: HeadInfo, baseline: Option<u64>) -> HeadVerdict {
    if info.status == 404 || info.status == 410 {
        return HeadVerdict::Dead;
    }
    if !(200..300).contains(&info.status) {
        // 403 = GitHub rate-limit, 5xx = CDN blip. Never kill a logo for these.
        return HeadVerdict::Unhealthy;
    }
    match info.content_length {
        None => HeadVerdict::Unchanged {
            content_length: None,
        },
        // No baseline yet = first observation: record it, do not "update".
        Some(len) if baseline.is_none_or(|b| b == len) => HeadVerdict::Unchanged {
            content_length: Some(len),
        },
        Some(len) => HeadVerdict::Changed {
            content_length: len,
        },
    }
}

// ---------------------------------------------------------------------------
// Core probe loop (I/O only through the LogoHttp trait — no DB, no Redis)
// ---------------------------------------------------------------------------

/// One logo-bearing token row selected for refresh.
#[derive(Clone, Debug)]
struct RefreshToken {
    chain_id: i32,
    /// Lowercase, as stored by the migration 034 CHECK constraint.
    address: String,
    logo_url: String,
}

/// Effect-level verdict after an optional re-download confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Dead,
    Unchanged { content_length: Option<u64> },
    Updated { content_length: u64 },
    Error,
}

/// Field key inside [`SIZES_HASH`] for one token.
fn baseline_field(chain_id: i32, address: &str) -> String {
    format!("{chain_id}:{address}")
}

/// Producer side of the api-server `token-icon.ts` reader contract:
/// `arbx:token-icons:{chainId}:{address}` (address lowercase). Invalidating it
/// forces the icon cascade to re-read PG, where the refreshed logo lives.
fn token_icon_key(chain_id: i32, address: &str) -> String {
    format!("arbx:token-icons:{chain_id}:{address}")
}

/// Probe ONE token: HEAD → classify → (if changed) re-download confirmation.
async fn probe_token<C: LogoHttp>(
    http: &C,
    token: &RefreshToken,
    baseline: Option<u64>,
) -> Verdict {
    let info = match http.head_logo(&token.logo_url).await {
        Ok(i) => i,
        Err(e) => {
            debug!(
                event = "logo_refresh.head_err",
                chain_id = token.chain_id,
                address = %token.address,
                err = %e
            );
            return Verdict::Error;
        }
    };
    match classify_head(info, baseline) {
        HeadVerdict::Dead => Verdict::Dead,
        HeadVerdict::Unhealthy => {
            warn!(
                event = "logo_refresh.unhealthy_status",
                chain_id = token.chain_id,
                address = %token.address,
                status = info.status,
                "treating as probe error (rate-limit/5xx must never kill a logo)"
            );
            Verdict::Error
        }
        HeadVerdict::Unchanged { content_length } => Verdict::Unchanged { content_length },
        HeadVerdict::Changed { content_length } => {
            match http.download_logo(&token.logo_url).await {
                Ok(n) if n > 0 => Verdict::Updated { content_length },
                Ok(_) => {
                    warn!(
                        event = "logo_refresh.empty_redownload",
                        chain_id = token.chain_id,
                        address = %token.address,
                        "re-download returned an empty body — treating as probe error"
                    );
                    Verdict::Error
                }
                Err(e) => {
                    debug!(
                        event = "logo_refresh.redownload_err",
                        chain_id = token.chain_id,
                        address = %token.address,
                        err = %e
                    );
                    Verdict::Error
                }
            }
        }
    }
}

/// Aggregate outcome of a full probe pass — consumed by the effect appliers.
#[derive(Default)]
struct ProbeOutcome {
    stats: RefreshStats,
    /// New baselines to persist (field -> Content-Length). First observations
    /// included; dead/error tokens deliberately absent.
    baselines: HashMap<String, u64>,
    /// `(chain_id, address)` of dead logos → PG NULL + Redis icon DEL.
    dead: Vec<(i32, String)>,
    /// `(chain_id, address, logo_url)` of changed logos → PG re-affirm +
    /// Redis icon DEL.
    updated: Vec<(i32, String, String)>,
}

/// Probe every token sequentially, pausing `pause` BETWEEN batches (never
/// after the last one). Pure with respect to PG/Redis — effects are applied by
/// the caller from the returned [`ProbeOutcome`].
async fn probe_all<C: LogoHttp>(
    http: &C,
    tokens: &[RefreshToken],
    baselines: &HashMap<String, u64>,
    batch_size: usize,
    pause: Duration,
) -> ProbeOutcome {
    let mut outcome = ProbeOutcome::default();
    let batch_size = batch_size.max(1); // defensive: `% 0` would panic
    for (i, token) in tokens.iter().enumerate() {
        if i > 0 && i % batch_size == 0 {
            tokio::time::sleep(pause).await;
        }
        let field = baseline_field(token.chain_id, &token.address);
        let baseline = baselines.get(&field).copied();
        outcome.stats.checked += 1;
        match probe_token(http, token, baseline).await {
            Verdict::Dead => {
                outcome.stats.dead += 1;
                outcome.dead.push((token.chain_id, token.address.clone()));
            }
            Verdict::Unchanged { content_length } => {
                outcome.stats.unchanged += 1;
                if let Some(len) = content_length {
                    outcome.baselines.insert(field, len);
                }
            }
            Verdict::Updated { content_length } => {
                outcome.stats.updated += 1;
                outcome.baselines.insert(field, content_length);
                outcome.updated.push((
                    token.chain_id,
                    token.address.clone(),
                    token.logo_url.clone(),
                ));
            }
            Verdict::Error => outcome.stats.errors += 1,
        }
    }
    outcome
}

// ---------------------------------------------------------------------------
// PG effects (per-token, idempotent, NO global transaction — fail-safe)
// ---------------------------------------------------------------------------

/// SELECT every logo-bearing token (chain_id, address, logo_url).
async fn fetch_logo_bearing_tokens(pool: &PgPool) -> Result<Vec<RefreshToken>> {
    let rows: Vec<(i32, String, String)> = sqlx::query_as(
        r#"
        SELECT chain_id, address, logo_url
          FROM tokens
         WHERE logo_url IS NOT NULL
         ORDER BY chain_id, address
        "#,
    )
    .fetch_all(pool)
    .await
    .context("logo_refresh: select logo-bearing tokens")?;
    Ok(rows
        .into_iter()
        .map(|(chain_id, address, logo_url)| RefreshToken {
            chain_id,
            address,
            logo_url,
        })
        .collect())
}

/// The logo died: NULL it and mark the row failed. `resolved_at` is NOT
/// touched — for monthly-old rows the 7-day retry TTL is already expired, so
/// the reconciliation loop picks the token up on its next tick.
async fn mark_logo_dead(pool: &PgPool, chain_id: i32, address: &str) -> Result<()> {
    sqlx::query(
        "UPDATE tokens SET logo_url = NULL, resolved_via = 'failed' \
         WHERE chain_id = $1 AND address = $2",
    )
    .bind(chain_id)
    .bind(address)
    .execute(pool)
    .await
    .context("logo_refresh: mark logo dead")?;
    Ok(())
}

/// The asset behind the (path-immutable) URL changed: re-affirm the logo and
/// advance `last_seen_at` so observability sees the re-verification.
async fn reassert_logo(pool: &PgPool, chain_id: i32, address: &str, logo_url: &str) -> Result<()> {
    sqlx::query(
        "UPDATE tokens SET logo_url = $3, last_seen_at = NOW() \
         WHERE chain_id = $1 AND address = $2",
    )
    .bind(chain_id)
    .bind(address)
    .bind(logo_url)
    .execute(pool)
    .await
    .context("logo_refresh: reassert logo")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Redis effects
// ---------------------------------------------------------------------------

async fn open_redis(redis_url: &str) -> Result<redis::aio::MultiplexedConnection> {
    let client = redis::Client::open(redis_url).context("redis::Client::open")?;
    client
        .get_multiplexed_async_connection()
        .await
        .context("get_multiplexed_async_connection")
}

/// Load the `field -> Content-Length` baseline map. Empty on any failure
/// (degrades change detection to first-observation; liveness still works).
async fn load_baselines(conn: &mut redis::aio::MultiplexedConnection) -> HashMap<String, u64> {
    use redis::AsyncCommands;
    match conn.hgetall::<_, HashMap<String, String>>(SIZES_HASH).await {
        Ok(raw) => raw
            .into_iter()
            .filter_map(|(field, v)| v.parse::<u64>().ok().map(|len| (field, len)))
            .collect(),
        Err(e) => {
            warn!(event = "logo_refresh.baseline_load_err", err = %e);
            HashMap::new()
        }
    }
}

/// Apply Redis effects in ONE pipeline: DEL icon keys for changed/dead logos,
/// HDEL dead baselines, HMSET the new baselines. Best-effort — a failure only
/// delays invalidation until the icon TTL (≤30d) expires; logged, never fatal.
async fn apply_redis_effects(conn: &mut redis::aio::MultiplexedConnection, outcome: &ProbeOutcome) {
    if outcome.dead.is_empty() && outcome.updated.is_empty() && outcome.baselines.is_empty() {
        return;
    }
    let mut pipe = redis::pipe();
    for &(chain_id, ref address) in &outcome.dead {
        pipe.del(token_icon_key(chain_id, address)).ignore();
        pipe.hdel(SIZES_HASH, baseline_field(chain_id, address))
            .ignore();
    }
    for &(chain_id, ref address, _) in &outcome.updated {
        pipe.del(token_icon_key(chain_id, address)).ignore();
    }
    for (field, len) in &outcome.baselines {
        pipe.hset(SIZES_HASH, field, len).ignore();
    }
    if let Err(e) = pipe.query_async::<_, ()>(conn).await {
        warn!(
            event = "logo_refresh.redis_apply_err",
            err = %e,
            "icon invalidation deferred (cache TTL is the backstop)"
        );
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run one full logo-refresh pass over every logo-bearing token.
///
/// Effects are applied per-token (PG UPDATEs + Redis pipeline) with NO global
/// transaction: a mid-run crash leaves already-refreshed rows persisted and
/// the untouched ones serving their previous (still live) URLs.
///
/// Only a SELECT failure returns `Err` (nothing was probed); per-token write
/// failures are logged and skipped — the next monthly run re-covers them.
pub async fn run_logo_refresh<C: LogoHttp>(
    pool: &PgPool,
    http: &C,
    redis_url: &str,
) -> Result<RefreshStats> {
    let started = Instant::now();
    let tokens = fetch_logo_bearing_tokens(pool).await?;
    if tokens.is_empty() {
        let stats = RefreshStats::default();
        info!(
            event = "logo_refresh.done",
            checked = stats.checked,
            updated = stats.updated,
            unchanged = stats.unchanged,
            dead = stats.dead,
            errors = stats.errors,
            elapsed_ms = started.elapsed().as_millis() as u64,
        );
        return Ok(stats);
    }

    // Optional Redis tier: baselines + icon-key invalidation. Down Redis
    // degrades (no change detection / invalidation) but never fails the job.
    let mut redis_conn = match open_redis(redis_url).await {
        Ok(c) => Some(c),
        Err(e) => {
            warn!(
                event = "logo_refresh.redis_connect_err",
                err = %e,
                "continuing in liveness-only mode (no change detection / invalidation)"
            );
            None
        }
    };
    let baselines = match redis_conn.as_mut() {
        Some(conn) => load_baselines(conn).await,
        None => HashMap::new(),
    };

    let outcome = probe_all(http, &tokens, &baselines, REFRESH_BATCH_SIZE, BATCH_PAUSE).await;

    for &(chain_id, ref address) in &outcome.dead {
        if let Err(e) = mark_logo_dead(pool, chain_id, address).await {
            warn!(
                event = "logo_refresh.mark_dead_err",
                chain_id,
                address = %address,
                err = %e
            );
        }
    }
    for &(chain_id, ref address, ref logo_url) in &outcome.updated {
        if let Err(e) = reassert_logo(pool, chain_id, address, logo_url).await {
            warn!(
                event = "logo_refresh.reassert_err",
                chain_id,
                address = %address,
                err = %e
            );
        }
    }
    if let Some(conn) = redis_conn.as_mut() {
        apply_redis_effects(conn, &outcome).await;
    }

    let stats = outcome.stats;
    info!(
        event = "logo_refresh.done",
        checked = stats.checked,
        updated = stats.updated,
        unchanged = stats.unchanged,
        dead = stats.dead,
        errors = stats.errors,
        elapsed_ms = started.elapsed().as_millis() as u64,
    );
    Ok(stats)
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Parse the tick interval. Missing/invalid → [`DEFAULT_LOGO_REFRESH_INTERVAL_SECS`]
/// (30 days). `0` is passed through: the caller treats it as "disabled".
pub fn parse_logo_refresh_interval(raw: Option<&str>) -> u64 {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => DEFAULT_LOGO_REFRESH_INTERVAL_SECS,
        Some(v) => match v.parse::<u64>() {
            Ok(secs) => secs,
            Err(_) => {
                warn!(
                    event = "logo_refresh.bad_interval",
                    raw = %v,
                    default = DEFAULT_LOGO_REFRESH_INTERVAL_SECS,
                    "LOGO_REFRESH_INTERVAL_SECS is not a valid u64 — using the default"
                );
                DEFAULT_LOGO_REFRESH_INTERVAL_SECS
            }
        },
    }
}

/// Read [`ENV_LOGO_REFRESH_INTERVAL_SECS`] from the environment.
pub fn logo_refresh_interval_from_env() -> u64 {
    parse_logo_refresh_interval(
        std::env::var(ENV_LOGO_REFRESH_INTERVAL_SECS)
            .ok()
            .as_deref(),
    )
}

// ---------------------------------------------------------------------------
// Unit tests (pure — no network, no DB, no Redis, no env mutation)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Scripted HTTP double — implements [`LogoHttp`] with no network.
    /// `Err(&'static str)` simulates a transport failure (timeout).
    #[derive(Default)]
    struct MockLogoHttp {
        heads: HashMap<String, Result<HeadInfo, &'static str>>,
        downloads: HashMap<String, Result<u64, &'static str>>,
    }

    impl MockLogoHttp {
        fn head_ok(url: &str, len: Option<u64>) -> Self {
            let mut m = Self::default();
            m.heads.insert(
                url.to_string(),
                Ok(HeadInfo {
                    status: 200,
                    content_length: len,
                }),
            );
            m
        }
    }

    impl LogoHttp for MockLogoHttp {
        async fn head_logo(&self, url: &str) -> Result<HeadInfo> {
            match self.heads.get(url) {
                Some(Ok(info)) => Ok(*info),
                Some(Err(msg)) => Err(anyhow::anyhow!("transport failure: {msg}")),
                None => Err(anyhow::anyhow!("unscripted HEAD: {url}")),
            }
        }

        async fn download_logo(&self, url: &str) -> Result<u64> {
            match self.downloads.get(url) {
                Some(Ok(n)) => Ok(*n),
                Some(Err(msg)) => Err(anyhow::anyhow!("transport failure: {msg}")),
                None => Err(anyhow::anyhow!("unscripted GET: {url}")),
            }
        }
    }

    /// Distinct synthetic token: lowercase 40-hex address ending in digit `n`.
    fn token(chain_id: i32, n: u8, url: &str) -> RefreshToken {
        RefreshToken {
            chain_id,
            address: format!("0x{}{n}", "a".repeat(39)),
            logo_url: url.to_string(),
        }
    }

    fn head(status: u16, len: Option<u64>) -> HeadInfo {
        HeadInfo {
            status,
            content_length: len,
        }
    }

    // --- classify_head ---

    #[test]
    fn classify_dead_on_404_and_410() {
        assert_eq!(classify_head(head(404, None), Some(1)), HeadVerdict::Dead);
        assert_eq!(classify_head(head(410, None), None), HeadVerdict::Dead);
    }

    #[test]
    fn classify_unhealthy_on_rate_limit_and_5xx() {
        // A rate-limit or CDN blip must NEVER be classified as dead.
        assert_eq!(
            classify_head(head(403, None), Some(1)),
            HeadVerdict::Unhealthy
        );
        assert_eq!(classify_head(head(429, None), None), HeadVerdict::Unhealthy);
        assert_eq!(
            classify_head(head(500, Some(1)), Some(1)),
            HeadVerdict::Unhealthy
        );
    }

    #[test]
    fn classify_unchanged_when_no_header_or_first_observation_or_equal() {
        // No Content-Length header → cannot judge → unchanged, no baseline.
        assert_eq!(
            classify_head(head(200, None), Some(1)),
            HeadVerdict::Unchanged {
                content_length: None
            }
        );
        // First observation (no baseline) → unchanged, baseline recorded.
        assert_eq!(
            classify_head(head(200, Some(100)), None),
            HeadVerdict::Unchanged {
                content_length: Some(100)
            }
        );
        // Baseline matches → unchanged.
        assert_eq!(
            classify_head(head(200, Some(100)), Some(100)),
            HeadVerdict::Unchanged {
                content_length: Some(100)
            }
        );
    }

    #[test]
    fn classify_changed_when_length_differs() {
        assert_eq!(
            classify_head(head(200, Some(2048)), Some(1024)),
            HeadVerdict::Changed {
                content_length: 2048
            }
        );
    }

    // --- probe_all: stats + outcome lists ---

    #[tokio::test]
    async fn probe_all_counts_mixed_outcomes() {
        let mut mock = MockLogoHttp::default();
        // unchanged (baseline matches)
        mock.heads.insert(
            "https://cdn/a.png".into(),
            Ok(HeadInfo {
                status: 200,
                content_length: Some(100),
            }),
        );
        // updated (content-length differs, re-download confirms)
        mock.heads.insert(
            "https://cdn/b.png".into(),
            Ok(HeadInfo {
                status: 200,
                content_length: Some(2048),
            }),
        );
        mock.downloads.insert("https://cdn/b.png".into(), Ok(2048));
        // dead
        mock.heads.insert(
            "https://cdn/c.png".into(),
            Ok(HeadInfo {
                status: 404,
                content_length: None,
            }),
        );
        // timeout
        mock.heads
            .insert("https://cdn/d.png".into(), Err("timed out"));

        let tokens = vec![
            token(1, 1, "https://cdn/a.png"),
            token(1, 2, "https://cdn/b.png"),
            token(1, 3, "https://cdn/c.png"),
            token(137, 4, "https://cdn/d.png"),
        ];
        let mut baselines = HashMap::new();
        baselines.insert(baseline_field(1, &tokens[0].address), 100u64);
        baselines.insert(baseline_field(1, &tokens[1].address), 1024u64);

        let out = probe_all(&mock, &tokens, &baselines, 50, Duration::from_secs(2)).await;

        // Stats invariant: every probe lands in exactly one bucket.
        assert_eq!(
            out.stats,
            RefreshStats {
                checked: 4,
                updated: 1,
                unchanged: 1,
                dead: 1,
                errors: 1,
            }
        );
        // Dead list carries the 404 token.
        assert_eq!(out.dead, vec![(1, tokens[2].address.clone())]);
        // Updated list carries the changed token with its URL.
        assert_eq!(
            out.updated,
            vec![(1, tokens[1].address.clone(), "https://cdn/b.png".into())]
        );
        // Baselines: unchanged keeps 100, updated moves to 2048; dead/error absent.
        assert_eq!(
            out.baselines.get(&baseline_field(1, &tokens[0].address)),
            Some(&100)
        );
        assert_eq!(
            out.baselines.get(&baseline_field(1, &tokens[1].address)),
            Some(&2048)
        );
        assert!(!out
            .baselines
            .contains_key(&baseline_field(1, &tokens[2].address)));
        assert!(!out
            .baselines
            .contains_key(&baseline_field(137, &tokens[3].address)));
    }

    #[tokio::test]
    async fn probe_all_timeout_is_error_not_dead() {
        // Fail-safe: a network blip must not NULL any logo.
        let mut mock = MockLogoHttp::default();
        mock.heads
            .insert("https://cdn/x.png".into(), Err("timed out"));
        let tokens = vec![token(1, 1, "https://cdn/x.png")];

        let out = probe_all(&mock, &tokens, &HashMap::new(), 50, BATCH_PAUSE).await;
        assert_eq!(out.stats.errors, 1);
        assert_eq!(out.stats.dead, 0);
        assert!(out.dead.is_empty());
    }

    #[tokio::test]
    async fn probe_all_rate_limit_is_error_not_dead() {
        let mut mock = MockLogoHttp::default();
        mock.heads.insert(
            "https://cdn/x.png".into(),
            Ok(HeadInfo {
                status: 403,
                content_length: None,
            }),
        );
        let tokens = vec![token(1, 1, "https://cdn/x.png")];

        let out = probe_all(&mock, &tokens, &HashMap::new(), 50, BATCH_PAUSE).await;
        assert_eq!(out.stats.errors, 1);
        assert!(out.dead.is_empty());
    }

    #[tokio::test]
    async fn probe_all_changed_but_failed_redownload_is_error_without_baseline() {
        let mut mock = MockLogoHttp::default();
        mock.heads.insert(
            "https://cdn/x.png".into(),
            Ok(HeadInfo {
                status: 200,
                content_length: Some(2048),
            }),
        );
        mock.downloads
            .insert("https://cdn/x.png".into(), Err("timed out"));
        let tokens = vec![token(1, 1, "https://cdn/x.png")];
        let mut baselines = HashMap::new();
        baselines.insert(baseline_field(1, &tokens[0].address), 1024u64);

        let out = probe_all(&mock, &tokens, &baselines, 50, BATCH_PAUSE).await;
        assert_eq!(out.stats.errors, 1);
        assert_eq!(out.stats.updated, 0);
        // No new baseline is recorded → the change stays detectable next run.
        assert!(!out
            .baselines
            .contains_key(&baseline_field(1, &tokens[0].address)));
    }

    #[tokio::test]
    async fn probe_all_empty_redownload_body_is_error() {
        let mut mock = MockLogoHttp::default();
        mock.heads.insert(
            "https://cdn/x.png".into(),
            Ok(HeadInfo {
                status: 200,
                content_length: Some(2048),
            }),
        );
        mock.downloads.insert("https://cdn/x.png".into(), Ok(0));
        let tokens = vec![token(1, 1, "https://cdn/x.png")];
        let mut baselines = HashMap::new();
        baselines.insert(baseline_field(1, &tokens[0].address), 1024u64);

        let out = probe_all(&mock, &tokens, &baselines, 50, BATCH_PAUSE).await;
        assert_eq!(out.stats.errors, 1);
        assert_eq!(out.stats.updated, 0);
    }

    // --- batch pausing (paused tokio clock: sleeps auto-advance time) ---

    #[tokio::test(start_paused = true)]
    async fn probe_all_pauses_between_batches() {
        // 5 tokens / batch_size 2 → 3 batches → exactly 2 pauses of 2s.
        let mock = MockLogoHttp::head_ok("https://cdn/t.png", Some(10));
        let tokens: Vec<RefreshToken> = (1..=5).map(|n| token(1, n, "https://cdn/t.png")).collect();

        let t0 = tokio::time::Instant::now();
        let out = probe_all(&mock, &tokens, &HashMap::new(), 2, Duration::from_secs(2)).await;
        let elapsed = t0.elapsed();

        assert_eq!(out.stats.checked, 5);
        assert!(
            elapsed >= Duration::from_secs(4),
            "expected 2 inter-batch pauses, elapsed {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(6),
            "no pause expected after the LAST batch, elapsed {elapsed:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn probe_all_single_batch_has_no_pause() {
        let mock = MockLogoHttp::head_ok("https://cdn/t.png", Some(10));
        let tokens: Vec<RefreshToken> = (1..=5).map(|n| token(1, n, "https://cdn/t.png")).collect();

        let t0 = tokio::time::Instant::now();
        let out = probe_all(&mock, &tokens, &HashMap::new(), 50, Duration::from_secs(2)).await;
        let elapsed = t0.elapsed();

        assert_eq!(out.stats.checked, 5);
        assert!(
            elapsed < Duration::from_secs(2),
            "single batch must not pause, elapsed {elapsed:?}"
        );
    }

    // --- key contracts ---

    #[test]
    fn token_icon_key_matches_api_server_reader_contract() {
        // Must equal api-server routes/token-icon.ts `tokenIconKey`:
        // `arbx:token-icons:${chainId}:${address}` (address lowercase).
        assert_eq!(token_icon_key(1, "0xabc"), "arbx:token-icons:1:0xabc");
        assert_eq!(
            token_icon_key(137, "0xdef123"),
            "arbx:token-icons:137:0xdef123"
        );
    }

    // --- interval parsing ---

    #[test]
    fn parse_interval_defaults() {
        assert_eq!(parse_logo_refresh_interval(None), 2_592_000);
        assert_eq!(parse_logo_refresh_interval(Some("")), 2_592_000);
        assert_eq!(parse_logo_refresh_interval(Some("   ")), 2_592_000);
        assert_eq!(parse_logo_refresh_interval(Some("junk")), 2_592_000);
        assert_eq!(parse_logo_refresh_interval(Some("-5")), 2_592_000);
    }

    #[test]
    fn parse_interval_valid_values() {
        assert_eq!(parse_logo_refresh_interval(Some("0")), 0); // disabled
        assert_eq!(parse_logo_refresh_interval(Some("86400")), 86_400);
        assert_eq!(parse_logo_refresh_interval(Some(" 7200 ")), 7_200);
    }
}
