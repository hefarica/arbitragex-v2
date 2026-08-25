//! QUOTEBASE-264 amount buckets (XLS-QB / ARBX-0008): the N-bucket sizing
//! surface consumed by the live `SizeOptimizer` curve.
//!
//! Split of concerns (mirrors `fe_normalization` ↔ worker wiring):
//! this module is the PURE bucket machinery — bounds validation, the sweep
//! over an injected per-amount evaluator, and the honest result types. The
//! motor side (`size_optimizer::bucket_sweep_2leg_curve`) resolves the LIVE
//! bracket (cap, reserves, fees) and injects `eval_2leg_profit` — the SAME
//! pure curve `golden_section_search_2leg` maximizes — so a bucket result
//! and a golden-section result are answers about one curve, not two.
//!
//! R8 honesty rules:
//! - an evaluator returning `None` at a probe SKIPS that point — a skipped
//!   bucket is never recorded as net 0 (no computed ≠ computed zero);
//! - `best` is `Some` only when a computed net is STRICTLY positive; an
//!   all-negative sweep is a real result (`best: None`), not an error;
//! - `buckets` stays the REQUESTED count so a short `points` list exposes
//!   the gap instead of hiding it.
//!
//! Bounds: N ∈ [8, 128] per the workbook's dynamic-N envelope (22 is the
//! reference point; the canonical benchmark matrix N = {8, 16, 22, 32, 64,
//! 128} lives here so producer and benchmark cannot drift apart).

use ethers::types::U256;

/// Smallest admissible bucket count (workbook envelope lower bound).
pub const AMOUNT_BUCKETS_MIN: usize = 8;

/// Largest admissible bucket count (workbook envelope upper bound).
pub const AMOUNT_BUCKETS_MAX: usize = 128;

/// Canonical benchmark matrix (ARBX-0012 axis). Strictly increasing, all
/// within `[AMOUNT_BUCKETS_MIN, AMOUNT_BUCKETS_MAX]`.
pub const AMOUNT_BUCKETS_CANONICAL: [usize; 6] = [8, 16, 22, 32, 64, 128];

/// Validate a requested bucket count against the workbook envelope.
/// Returns the same `n` on success so call sites can bind it in one step
/// (`let n = validate_amount_buckets(n)?;`). Trading-path input: every
/// caller-facing entry MUST go through this — a silently clamped or
/// wrap-around bucket count would misreport the sweep resolution.
pub fn validate_amount_buckets(n: usize) -> Result<usize, String> {
    if !(AMOUNT_BUCKETS_MIN..=AMOUNT_BUCKETS_MAX).contains(&n) {
        return Err(format!(
            "amount_buckets: n={} outside workbook envelope [{}, {}]",
            n, AMOUNT_BUCKETS_MIN, AMOUNT_BUCKETS_MAX
        ));
    }
    Ok(n)
}

/// One evaluated bucket: the probed `amount_in` and the motor's net at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BucketPoint {
    pub amount_in_wei: U256,
    pub net_wei: i128,
}

/// Result of a bucket sweep.
///
/// - `buckets` — the REQUESTED count (envelope-validated by the sweep);
/// - `points` — only the probes the evaluator answered (`None` probes are
///   skipped, never zero-filled);
/// - `best` — argmax over `points` with ties keeping the FIRST (lowest
///   amount — deterministic), `Some` only when strictly positive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketSweep {
    pub buckets: usize,
    pub points: Vec<BucketPoint>,
    pub best: Option<BucketPoint>,
}

/// Sweep `net_at` over an externally resolved probe grid.
///
/// The grid itself is the caller's (`size_optimizer::geom_probes` on the
/// live bracket) so this function stays pure and grid-agnostic. `probes`
/// must respect the workbook envelope — a probe list outside
/// `[AMOUNT_BUCKETS_MIN, AMOUNT_BUCKETS_MAX]` is an integration bug and
/// fails fast rather than sweeping a misreported resolution.
pub fn bucket_sweep_2leg(
    probes: &[U256],
    net_at: &mut dyn FnMut(&U256) -> Option<i128>,
) -> Result<BucketSweep, String> {
    validate_amount_buckets(probes.len())?;
    let mut points: Vec<BucketPoint> = Vec::with_capacity(probes.len());
    let mut best: Option<BucketPoint> = None;
    for amount in probes.iter() {
        // None ⇒ skipped (R8): no point is fabricated, no net is zero-filled.
        if let Some(net_wei) = net_at(amount) {
            let point = BucketPoint {
                amount_in_wei: *amount,
                net_wei,
            };
            let improves = match best {
                Some(b) => net_wei > b.net_wei,
                None => true,
            };
            if net_wei > 0 && improves {
                best = Some(point);
            }
            points.push(point);
        }
    }
    Ok(BucketSweep {
        buckets: probes.len(),
        points,
        best,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Positional format args only in probed files (edition-agnostic —
    // the CI-#460 lesson): asserts below use no inline captures.

    #[test]
    fn validate_rejects_outside_envelope() {
        assert!(validate_amount_buckets(0).is_err());
        assert!(validate_amount_buckets(7).is_err());
        assert!(validate_amount_buckets(129).is_err());
        assert_eq!(validate_amount_buckets(8), Ok(8));
        assert_eq!(validate_amount_buckets(22), Ok(22));
        assert_eq!(validate_amount_buckets(128), Ok(128));
        let err = validate_amount_buckets(7).unwrap_err();
        assert!(
            err.contains("[8, 128]"),
            "error should name the envelope, got: {}",
            err
        );
    }

    #[test]
    fn canonical_matrix_is_strictly_increasing_and_within_bounds() {
        assert_eq!(AMOUNT_BUCKETS_CANONICAL[0], AMOUNT_BUCKETS_MIN);
        assert_eq!(
            AMOUNT_BUCKETS_CANONICAL[AMOUNT_BUCKETS_CANONICAL.len() - 1],
            AMOUNT_BUCKETS_MAX
        );
        for pair in AMOUNT_BUCKETS_CANONICAL.windows(2) {
            assert!(pair[0] < pair[1], "matrix must strictly increase");
            assert!(validate_amount_buckets(pair[0]).is_ok());
            assert!(validate_amount_buckets(pair[1]).is_ok());
        }
    }

    #[test]
    fn sweep_skips_none_points_and_argmaxes_computed() {
        // Deterministic injectable curve over 8 probes, keyed by value:
        // v -> 11v mod 17 - 3 gives [8, -, 13, 7, -, 12, 6, 0] with None
        // at 2 and 5 (skipped) and argmax 13 at amount 3.
        let net_of = |v: u64| -> Option<i128> {
            match v {
                2 | 5 => None,
                _ => Some(((v * 11) % 17) as i128 - 3),
            }
        };
        let probes: Vec<U256> = (1u64..=8).map(U256::from).collect();
        let mut net_at = |x: &U256| -> Option<i128> { u64::try_from(*x).ok().and_then(net_of) };
        let sweep = bucket_sweep_2leg(&probes, &mut net_at).unwrap();
        assert_eq!(sweep.buckets, 8);
        assert_eq!(sweep.points.len(), 6, "2 None probes skipped");
        assert!(!sweep
            .points
            .iter()
            .any(|p| p.amount_in_wei == U256::from(2u64)));
        assert!(!sweep
            .points
            .iter()
            .any(|p| p.amount_in_wei == U256::from(5u64)));
        assert_eq!(sweep.best.unwrap().net_wei, 13);
        assert_eq!(sweep.best.unwrap().amount_in_wei, U256::from(3u64));
    }

    #[test]
    fn sweep_all_negative_yields_best_none_never_fake_zero() {
        let probes: Vec<U256> = (0u64..8).map(|i| U256::from(i + 1)).collect();
        let mut net_at = |_x: &U256| -> Option<i128> { Some(-7) };
        let sweep = bucket_sweep_2leg(&probes, &mut net_at).unwrap();
        assert_eq!(sweep.points.len(), 8);
        assert!(
            sweep.best.is_none(),
            "all-negative sweep: best None, not a fabricated 0 point"
        );
    }

    #[test]
    fn sweep_tie_keeps_lowest_amount_deterministic() {
        let probes: Vec<U256> = (0u64..8).map(|i| U256::from(i + 1)).collect();
        let mut net_at = |_x: &U256| -> Option<i128> { Some(5) };
        let sweep = bucket_sweep_2leg(&probes, &mut net_at).unwrap();
        assert_eq!(sweep.best.unwrap().amount_in_wei, U256::from(1u64));
    }

    #[test]
    fn sweep_rejects_probe_count_outside_envelope() {
        let short: Vec<U256> = (0u64..7).map(U256::from).collect();
        assert!(bucket_sweep_2leg(&short, &mut |_| None).is_err());
        let long: Vec<U256> = (0u64..129).map(U256::from).collect();
        assert!(bucket_sweep_2leg(&long, &mut |_| None).is_err());
    }
}
