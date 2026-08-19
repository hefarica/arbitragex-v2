//! cred_rotation — RunFullSyncCycle FASE 4: titular→fallback credential
//! rotation for CSV-backed credentials (rpc_http / rpc_ws).
//!
//! Contract (FASE 2 macro): for `rpc_http`/`rpc_ws` the credential value is a
//! CSV whose ORDER is the rotation priority — first entry = titular, the rest
//! = fallbacks. The value may come from `.env` or from the Redis projection
//! `arbx:svc_cred:rpc_http:chain:<id>` (FASE 3a/3b precedence: projection →
//! env); this module only ever sees the CSV string, so it works for both.
//!
//! Single-value credentials (bloxroute, titan, github_token) have no array to
//! rotate — their "rotation" is a re-PUT via the RunFullSyncCycle macro plus
//! the `arbx:svc_cred:reload` publish, out of scope here.
//!
//! Division of labor with `rpc_failover` (PIEZA B): the pool's circuit breaker
//! owns TRANSIENT failures (timeouts, connections, drift) — it re-probes with
//! its own backoff and is NOT reinvented here. This module owns CREDENTIAL
//! failures (401/403/429/quota-exceeded): a bad API key does not heal, the
//! only fix is advancing to the next entry.
//!
//! Fail-honest terminal (PIEZA C, RULE 00): when every entry in the CSV has
//! failed (wrap-around), the consumer MUST declare itself degraded with
//! reason `all_credentials_failed` for the duration of the backoff window —
//! never synthetic data, never a last-good-value. `rotate()` returns
//! `AllFailedCooldown` for exactly that case.
//!
//! Typical consumer loop:
//!
//! ```text
//! let (name, url) = current_entry(&csv, &rot)?;      // titular at boot
//! ... request against `url` fails with `err` ...
//! match rotate(&name, &csv, &rot, &err_text, now_ms) {
//!     RotationOutcome::Rotated { name, url, state } => { rot = state; use (name, url) }
//!     RotationOutcome::NotCredentialError           => { /* circuit breaker owns it */ }
//!     RotationOutcome::AllFailedCooldown            => { /* degrade: all_credentials_failed */ }
//!     RotationOutcome::Empty                        => { /* degrade: no parseable entries */ }
//! }
//! ... request succeeds → on_success(&mut rot) // titular retakes its place
//! ```

use tracing::{debug, info, warn};

use crate::metrics::{CREDENTIAL_ALL_FAILED_TOTAL, CREDENTIAL_ROTATION_TOTAL};

/// Backoff schedule before the titular is re-probed after a full cycle
/// failure: 1 min, 5 min, 15 min (capped).
pub const BACKOFF_SCHEDULE_MS: [u64; 3] = [60_000, 300_000, 900_000];

/// Degradation reason a consumer must report when the whole CSV failed.
pub const ALL_CREDENTIALS_FAILED: &str = "all_credentials_failed";

/// Rotation cursor over a credential CSV. Reset to `default()` when the CSV
/// changes (svc_cred reload / re-PUT) so the new titular anchors the cycle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RotationState {
    /// Index of the entry currently in use (0 = titular).
    pub current_index: usize,
    /// When `Some(t)`, every entry already failed and the array is cooling
    /// down until epoch-ms `t` before the titular is re-probed. `now < t`
    /// suppresses rotation (anti-thrash, R9); `on_success` clears it.
    pub degraded_until_ms: Option<u64>,
    /// Cumulative rotations (advances) since the last state reset.
    pub total_rotations: u64,
}

/// Result of a rotation step. The two degraded variants are the fail-honest
/// terminal states — the caller serves nothing synthetic (RULE 00).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationOutcome {
    /// Advanced to the next entry; use it and store the new state.
    Rotated {
        name: String,
        url: String,
        state: RotationState,
    },
    /// Not a credential-class error — the `rpc_failover` circuit breaker owns
    /// it; no credential action.
    NotCredentialError,
    /// Every entry already failed and the backoff window is still open —
    /// stay degraded with reason `all_credentials_failed`.
    AllFailedCooldown,
    /// CSV empty or nothing parseable — fail-honest degraded, no fabrication.
    Empty,
}

// ---------- error classification ----------

/// Classify an error string into a credential-rotation reason.
/// `Some("http_401" | "http_403" | "http_429" | "quota_exceeded")` when the
/// failure is credential-class (the API key is bad/throttled/exhausted and
/// will not heal by retrying the same entry).
pub fn credential_error_reason(error_code: &str) -> Option<&'static str> {
    let m = error_code.to_ascii_lowercase();
    if contains_bounded(&m, "401") {
        Some("http_401")
    } else if contains_bounded(&m, "403") {
        Some("http_403")
    } else if contains_bounded(&m, "429") {
        Some("http_429")
    } else if m.contains("quota") {
        Some("quota_exceeded")
    } else {
        None
    }
}

/// Credential-class failure? (401/403/429/quota-exceeded).
pub fn is_credential_error(error_code: &str) -> bool {
    credential_error_reason(error_code).is_some()
}

/// Substring match where the needle is not embedded in a longer digit run —
/// "401" must not match "14013" (block numbers) or "4299".
fn contains_bounded(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    let nlen = n.len();
    if nlen == 0 || h.len() < nlen {
        return false;
    }
    h.windows(nlen).enumerate().any(|(i, w)| {
        if w != n {
            return false;
        }
        let before_ok = i == 0 || !h[i - 1].is_ascii_digit();
        let after_ok = i + nlen == h.len() || !h[i + nlen].is_ascii_digit();
        before_ok && after_ok
    })
}

// ---------- rotation core ----------

/// Should a credential rotation happen for this failure?
///
/// True iff the error is credential-class AND we are not inside the
/// wrap-around cooldown (during cooldown every entry is known-bad; churning
/// the cursor again would only burn the array — R9 log/rotation discipline).
pub fn should_rotate(state: &RotationState, error_code: &str, now_ms: u64) -> bool {
    if !is_credential_error(error_code) {
        return false;
    }
    match state.degraded_until_ms {
        Some(until) => now_ms >= until,
        None => true,
    }
}

/// The entry `state.current_index` points at (initial selection: the titular).
/// Clamps a stale index into range after the CSV shrank.
pub fn current_entry(csv: &str, state: &RotationState) -> Option<(String, String)> {
    let entries = crate::rpc_failover::parse_csv(csv).ok()?;
    if entries.is_empty() {
        return None;
    }
    let idx = state.current_index.min(entries.len() - 1);
    let (name, url) = &entries[idx];
    Some((name.clone(), url.clone()))
}

/// Advance to the next VALID entry (caller asserts the current one failed).
/// Wraps to index 0 when the last entry failed — that is the all-failed
/// signal; `rotate()` attaches the backoff window to it. `None` when nothing
/// parses (fail-honest: no fabricated entries).
pub fn next_entry(csv: &str, state: &RotationState) -> Option<(String, String, RotationState)> {
    let entries = crate::rpc_failover::parse_csv(csv).ok()?;
    advance(&entries, state)
}

fn advance(
    entries: &[(String, String)],
    state: &RotationState,
) -> Option<(String, String, RotationState)> {
    if entries.is_empty() {
        return None;
    }
    let len = entries.len();
    let next_index = (state.current_index + 1) % len;
    let (name, url) = &entries[next_index];
    Some((
        name.clone(),
        url.clone(),
        RotationState {
            current_index: next_index,
            degraded_until_ms: state.degraded_until_ms,
            total_rotations: state.total_rotations + 1,
        },
    ))
}

/// Backoff before the titular is re-probed, escalated per completed full
/// cycle: 1 min → 5 min → 15 min (capped).
pub fn backoff_ms(full_cycles_completed: u64) -> u64 {
    let idx = full_cycles_completed.saturating_sub(1) as usize;
    BACKOFF_SCHEDULE_MS[idx.min(BACKOFF_SCHEDULE_MS.len() - 1)]
}

/// Full rotation step (the consumer-facing API). `provider` is the name of
/// the entry whose credential just failed — used for logs and metrics only.
///
/// On wrap-around (last entry failed → cursor returns to the titular) this
/// sets the escalating backoff window, logs `credential.all_failed` and bumps
/// `arbx_credential_all_failed_total`; every advance bumps
/// `arbx_credential_rotation_total{provider}`.
pub fn rotate(
    provider: &str,
    csv: &str,
    state: &RotationState,
    error_code: &str,
    now_ms: u64,
) -> RotationOutcome {
    if !should_rotate(state, error_code, now_ms) {
        return if is_credential_error(error_code) {
            // Fail-honest terminal: every entry already failed; stay degraded.
            // debug (R9): a degraded consumer probing during the window would
            // otherwise emit one warn per probe for up to 15 minutes — the
            // `credential.wraparound` info already announced the window.
            debug!(
                event = "credential.rotation_suppressed",
                provider,
                reason = ALL_CREDENTIALS_FAILED,
                retry_after_ms = state
                    .degraded_until_ms
                    .map(|t| t.saturating_sub(now_ms))
                    .unwrap_or(0),
                "all entries known-bad — staying degraded until the backoff window lifts"
            );
            RotationOutcome::AllFailedCooldown
        } else {
            RotationOutcome::NotCredentialError
        };
    }

    let entries = match crate::rpc_failover::parse_csv(csv) {
        Ok(v) if !v.is_empty() => v,
        _ => {
            warn!(
                event = "credential.rotation_empty",
                provider,
                reason = ALL_CREDENTIALS_FAILED,
                "credential CSV has no parseable entries — fail-honest degraded"
            );
            return RotationOutcome::Empty;
        }
    };

    let len = entries.len();
    let wrapped = (state.current_index + 1) % len == 0;
    let (name, url, mut next) = match advance(&entries, state) {
        Some(v) => v,
        None => return RotationOutcome::Empty, // unreachable: entries non-empty
    };

    // Expired cooldown lifts with this rotation; a fresh wrap sets a new one.
    next.degraded_until_ms = None;
    if wrapped {
        let cycles = next.total_rotations / len as u64;
        next.degraded_until_ms = Some(now_ms + backoff_ms(cycles));
        record_all_failed(provider);
        info!(
            event = "credential.wraparound",
            provider,
            to = %name,
            backoff_ms = backoff_ms(cycles),
            "full cycle failed — titular re-probe scheduled with backoff"
        );
    }
    record_rotation(provider);
    info!(
        event = "credential.rotated",
        provider,
        from_index = state.current_index,
        to = %name,
        total_rotations = next.total_rotations,
        "credential rotation advanced"
    );
    RotationOutcome::Rotated {
        name,
        url,
        state: next,
    }
}

/// A successful call on the current entry: the cooldown lifts and the entry
/// keeps its place (a titular that passes its re-probe has retaken it).
pub fn on_success(state: &mut RotationState) {
    state.degraded_until_ms = None;
}

// ---------- metrics ----------

/// Bump `arbx_credential_rotation_total{provider}`.
pub fn record_rotation(provider: &str) {
    CREDENTIAL_ROTATION_TOTAL
        .with_label_values(&[provider])
        .inc();
}

/// Bump `arbx_credential_all_failed_total{provider}` + the terminal warn log.
pub fn record_all_failed(provider: &str) {
    CREDENTIAL_ALL_FAILED_TOTAL
        .with_label_values(&[provider])
        .inc();
    warn!(
        event = "credential.all_failed",
        provider,
        reason = ALL_CREDENTIALS_FAILED,
        "every entry in the credential CSV failed — consumer MUST degrade honestly \
         (no synthetic data, no last-good-value)"
    );
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "titular=https://a.example/v2/K1,fallback1=https://b.example,K2typo=,publicnode=https://ethereum-rpc.publicnode.com";

    // --- classification ---

    #[test]
    fn credential_errors_classify() {
        assert_eq!(
            credential_error_reason("HTTP status client error (401 Unauthorized)"),
            Some("http_401")
        );
        assert_eq!(credential_error_reason("403 forbidden"), Some("http_403"));
        assert_eq!(
            credential_error_reason("HTTP status client error (429 Too Many Requests)"),
            Some("http_429")
        );
        assert_eq!(
            credential_error_reason("monthly quota exceeded for key"),
            Some("quota_exceeded")
        );
        assert_eq!(
            credential_error_reason("QUOTA EXCEEDED"),
            Some("quota_exceeded")
        );
    }

    #[test]
    fn non_credential_errors_do_not_classify() {
        for e in [
            "timeout",
            "connection refused",
            "decode error: unexpected payload",
            "block 14013 behind tip", // embedded 401 — digit-bounded must reject
            "error 4299 while syncing", // embedded 429
            "provider 24031 lagging",
        ] {
            assert!(
                !is_credential_error(e),
                "false credential classification for {e:?}"
            );
        }
    }

    // --- should_rotate ---

    #[test]
    fn should_rotate_true_for_credential_errors_when_no_cooldown() {
        for e in ["401", "403", "429", "quota"] {
            assert!(should_rotate(&RotationState::default(), e, 1_000), "e={e}");
        }
    }

    #[test]
    fn should_rotate_false_for_transport_errors() {
        assert!(!should_rotate(&RotationState::default(), "timeout", 1_000));
        assert!(!should_rotate(
            &RotationState::default(),
            "connection reset",
            1_000
        ));
    }

    #[test]
    fn should_rotate_gated_by_cooldown_window() {
        let st = RotationState {
            degraded_until_ms: Some(10_000),
            ..RotationState::default()
        };
        assert!(!should_rotate(&st, "401", 9_999), "inside cooldown");
        assert!(should_rotate(&st, "401", 10_000), "at the boundary");
        assert!(should_rotate(&st, "401", 10_001), "after cooldown");
    }

    // --- next_entry / wrap-around ---

    #[test]
    fn next_entry_advances_in_order() {
        let csv = "a=https://a,b=https://b,c=https://c";
        let (n, u, s) = next_entry(csv, &RotationState::default()).unwrap();
        assert_eq!((n.as_str(), u.as_str()), ("b", "https://b"));
        assert_eq!(s.current_index, 1);
        assert_eq!(s.total_rotations, 1);

        let (n, _, s) = next_entry(csv, &s).unwrap();
        assert_eq!(n, "c");
        assert_eq!(s.current_index, 2);
        assert_eq!(s.total_rotations, 2);
    }

    #[test]
    fn next_entry_wraps_to_titular_after_last() {
        let csv = "a=https://a,b=https://b,c=https://c";
        let st = RotationState {
            current_index: 2,
            total_rotations: 2,
            ..RotationState::default()
        };
        let (n, u, s) = next_entry(csv, &st).unwrap();
        assert_eq!((n.as_str(), u.as_str()), ("a", "https://a"));
        assert_eq!(s.current_index, 0);
        assert_eq!(s.total_rotations, 3);
    }

    #[test]
    fn next_entry_none_on_empty_or_unparseable_csv() {
        assert!(next_entry("", &RotationState::default()).is_none());
        assert!(next_entry("  , , ", &RotationState::default()).is_none());
        assert!(next_entry("a=,=x,ftp://nope", &RotationState::default()).is_none());
    }

    #[test]
    fn next_entry_skips_malformed_tokens_like_the_pool_parser() {
        // same parse_csv semantics as HttpRpcPool: invalid tokens are dropped,
        // valid ones keep their order (FASE 2 rotation priority).
        let (n, _, _) = next_entry(CSV, &RotationState::default()).unwrap();
        assert_eq!(n, "fallback1");
    }

    #[test]
    fn current_entry_defaults_to_titular() {
        let (n, u) = current_entry(CSV, &RotationState::default()).unwrap();
        assert_eq!(
            (n.as_str(), u.as_str()),
            ("titular", "https://a.example/v2/K1")
        );
    }

    #[test]
    fn current_entry_clamps_stale_index() {
        let st = RotationState {
            current_index: 99,
            ..RotationState::default()
        };
        let (n, _) = current_entry("a=https://a", &st).unwrap();
        assert_eq!(n, "a");
    }

    // --- backoff ---

    #[test]
    fn backoff_escalates_1m_5m_15m_then_caps() {
        assert_eq!(backoff_ms(1), 60_000);
        assert_eq!(backoff_ms(2), 300_000);
        assert_eq!(backoff_ms(3), 900_000);
        assert_eq!(backoff_ms(7), 900_000);
    }

    // --- rotate orchestration (PIEZA A + C) ---

    fn three_provider_csv() -> &'static str {
        "a=https://a,b=https://b,c=https://c"
    }

    #[test]
    fn rotate_full_cycle_then_cooldown_then_recovers() {
        let csv = three_provider_csv();
        let now = 1_000_000u64;
        let mut st = RotationState::default();

        // a fails 401 → rotate to b
        match rotate("a", csv, &st, "401", now) {
            RotationOutcome::Rotated { name, state, .. } => {
                assert_eq!(name, "b");
                st = state;
            }
            other => panic!("expected Rotated, got {other:?}"),
        }
        // b fails 403 → rotate to c
        match rotate("b", csv, &st, "403", now) {
            RotationOutcome::Rotated { name, state, .. } => {
                assert_eq!(name, "c");
                st = state;
            }
            other => panic!("expected Rotated, got {other:?}"),
        }
        // c fails 429 → wrap-around: titular re-probe scheduled at now+60s
        match rotate("c", csv, &st, "429", now) {
            RotationOutcome::Rotated { name, state, .. } => {
                assert_eq!(name, "a");
                assert_eq!(state.degraded_until_ms, Some(now + 60_000));
                st = state;
            }
            other => panic!("expected Rotated(wrap), got {other:?}"),
        }
        // titular fails again INSIDE the cooldown → fail-honest terminal
        assert_eq!(
            rotate("a", csv, &st, "401", now + 1_000),
            RotationOutcome::AllFailedCooldown
        );

        // cooldown expires → rotation resumes from the titular
        match rotate("a", csv, &st, "401", now + 60_001) {
            RotationOutcome::Rotated { name, state, .. } => {
                assert_eq!(name, "b");
                assert_eq!(state.degraded_until_ms, None, "expired cooldown lifted");
                st = state;
            }
            other => panic!("expected Rotated after cooldown, got {other:?}"),
        }

        // current entry starts succeeding → cooldown stays clear
        on_success(&mut st);
        assert_eq!(st.degraded_until_ms, None);
    }

    #[test]
    fn rotate_backoff_escalates_across_cycles() {
        let csv = three_provider_csv();
        let mut st = RotationState::default();
        let t0 = 5_000_000u64;
        // three failures → first wrap (1 min backoff)
        for (p, i) in [("a", 0u64), ("b", 1), ("c", 2)] {
            match rotate(p, csv, &st, "401", t0 + i) {
                RotationOutcome::Rotated { state, .. } => st = state,
                other => panic!("expected Rotated, got {other:?}"),
            }
        }
        assert_eq!(st.degraded_until_ms, Some(t0 + 2 + 60_000));
        let after_first = st.degraded_until_ms.unwrap();
        // next full cycle: 3 more rotations (cooldown respected between cycles)
        for (p, dt) in [("a", 0u64), ("b", 1), ("c", 2)] {
            match rotate(p, csv, &st, "401", after_first + dt) {
                RotationOutcome::Rotated { state, .. } => st = state,
                other => panic!("expected Rotated, got {other:?}"),
            }
        }
        assert_eq!(
            st.degraded_until_ms,
            Some(after_first + 2 + 300_000),
            "second cycle → 5 min"
        );
    }

    #[test]
    fn rotate_ignores_non_credential_errors() {
        let csv = three_provider_csv();
        assert_eq!(
            rotate("a", csv, &RotationState::default(), "timeout", 1_000),
            RotationOutcome::NotCredentialError
        );
    }

    #[test]
    fn rotate_empty_csv_is_fail_honest() {
        assert_eq!(
            rotate("a", "", &RotationState::default(), "401", 1_000),
            RotationOutcome::Empty
        );
    }

    #[test]
    fn rotate_single_entry_csv_cooldown_every_failure() {
        // pool-of-one: every failure is a wrap-around (re-probe with backoff)
        let csv = "solo=https://solo";
        let now = 42_000u64;
        match rotate("solo", csv, &RotationState::default(), "401", now) {
            RotationOutcome::Rotated { name, state, .. } => {
                assert_eq!(name, "solo");
                assert_eq!(state.degraded_until_ms, Some(now + 60_000));
            }
            other => panic!("expected Rotated, got {other:?}"),
        }
    }

    // --- metric emission ---

    #[test]
    fn record_rotation_increments_counter() {
        let p = "mt-rotation-test";
        let before = CREDENTIAL_ROTATION_TOTAL.with_label_values(&[p]).get();
        record_rotation(p);
        record_rotation(p);
        assert_eq!(
            CREDENTIAL_ROTATION_TOTAL.with_label_values(&[p]).get(),
            before + 2
        );
    }

    #[test]
    fn record_all_failed_increments_counter() {
        let p = "mt-allfailed-test";
        let before = CREDENTIAL_ALL_FAILED_TOTAL.with_label_values(&[p]).get();
        record_all_failed(p);
        assert_eq!(
            CREDENTIAL_ALL_FAILED_TOTAL.with_label_values(&[p]).get(),
            before + 1
        );
    }

    #[test]
    fn rotate_emits_metrics_per_advance_and_per_wrap() {
        let csv = three_provider_csv();
        let p_rot = "mt-wrap-rot";
        let p_fail = "mt-wrap-allfailed";
        let rot_before = CREDENTIAL_ROTATION_TOTAL.with_label_values(&[p_rot]).get();
        let fail_before = CREDENTIAL_ALL_FAILED_TOTAL
            .with_label_values(&[p_fail])
            .get();

        let mut st = RotationState::default();
        for provider in ["mt-wrap-rot", "mt-wrap-rot", "mt-wrap-allfailed"] {
            match rotate(provider, csv, &st, "401", 1_000) {
                RotationOutcome::Rotated { state, .. } => st = state,
                other => panic!("expected Rotated, got {other:?}"),
            }
        }
        // 3 advances on p_rot for the first two + the wrap advance counted
        // under its own provider label
        assert_eq!(
            CREDENTIAL_ROTATION_TOTAL.with_label_values(&[p_rot]).get(),
            rot_before + 2
        );
        assert_eq!(
            CREDENTIAL_ROTATION_TOTAL.with_label_values(&[p_fail]).get(),
            1
        );
        assert_eq!(
            CREDENTIAL_ALL_FAILED_TOTAL
                .with_label_values(&[p_fail])
                .get(),
            fail_before + 1,
            "wrap-around bumps the all-failed metric exactly once"
        );
    }
}
