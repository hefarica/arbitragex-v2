//! ARBX-0012 (XLS-QB / REQ-QB-015): the N-bucket amount-sweep benchmark
//! matrix over the SAME 2-leg V2 curve the motor maximizes.
//!
//! Workbook `10_LATENCY` r9 pins the stage this axis feeds ("Amount-aware
//! refinement", input size = "top finalists / amount probe buckets") and r21
//! states the mandate outright: "<30 ms NO puede garantizarse por Excel.
//! Debe demostrarse con benchmarks." The canonical matrix N = {8, 16, 22,
//! 32, 64, 128} is imported from `amount_buckets` (single source — producer
//! and benchmark cannot drift apart), and the percentile statistic is the
//! live telemetry kernel (`latency_budget::nearest_rank`) so a bench p95 and
//! a wire p95 are the same number-shape.
//!
//! Fixtures are byte-identical to the unit-test golden fixture
//! (`bucket_sweep_2leg_curve_bounded_by_golden_and_envelope_enforced`):
//! pool A buys token_out deep, pool B sells it back higher, golden p* is
//! computed LIVE by the kernel (no hardcoded literal — cross-platform
//! proof), and every N's sweep is sanity-checked BEFORE its timings are
//! reported (RULE 00: a broken measurement is worse than no measurement).
//!
//! Each run prints ONE JSON line per N to stdout (`.ai-work/
//! PERFORMANCE_RESULTS.json` §70 schema — machine-transcribed, hand never
//! retypes numbers); progress and the golden reference go to stderr so
//! stdout stays parseable.
//!
//! Run: `cargo bench --bench amount_matrix`

use std::hint::black_box;
use std::time::Instant;

use ethers::types::U256;
use searcher_rs::amount_buckets::AMOUNT_BUCKETS_CANONICAL;
use searcher_rs::latency_budget::nearest_rank;
use searcher_rs::size_optimizer::{bucket_sweep_2leg_curve, golden_section_search_2leg};

/// Warmup sweeps per N (unmeasured — page faults, cache, branch predictors).
const WARMUP_PER_N: usize = 50;

/// Measured sweeps per N. Nearest-rank p99 at 400 samples = rank 396 —
/// exact, no interpolation, same as the wire.
const SAMPLES_PER_N: usize = 400;

/// `1e18 * n` in wei — same helper semantics as the size_optimizer test
/// module (benches do not see `#[cfg(test)]` helpers, so it lives here too).
fn unit(n: u64) -> U256 {
    U256::from(10u128).pow(U256::from(18u32)) * U256::from(n)
}

fn main() {
    // Golden fixture (unit-test twin): pool A buys deep, pool B sells back
    // higher; bracket [1 wei, min(cap, reserve_in_a)] = [1, 1000] units.
    let hop_a = vec![(unit(1000), unit(2_000_000))];
    let hop_b = vec![(unit(1_000_000), unit(600))];
    let x_lo = U256::from(1u64);
    let x_hi = unit(1000);
    let (fee_a, fee_b) = (30u32, 30u32);

    // Live golden reference — the kernel's own answer on the same bracket.
    let (_x_star, p_gs) = golden_section_search_2leg(x_lo, x_hi, &hop_a, &hop_b, fee_a, fee_b, 25);
    assert!(p_gs > 0, "fixture must be profitable, got {}", p_gs);

    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    eprintln!(
        "ARBX-0012 amount matrix · golden p* = {} net wei · {} samples/N (+{} warmup) · nearest-rank",
        p_gs, SAMPLES_PER_N, WARMUP_PER_N
    );

    for n in AMOUNT_BUCKETS_CANONICAL {
        for _ in 0..WARMUP_PER_N {
            let _ = black_box(bucket_sweep_2leg_curve(
                n, x_lo, x_hi, &hop_a, &hop_b, fee_a, fee_b,
            ));
        }

        let mut us: Vec<u64> = Vec::with_capacity(SAMPLES_PER_N);
        let mut last = None;
        for _ in 0..SAMPLES_PER_N {
            let t = Instant::now();
            let sweep = black_box(bucket_sweep_2leg_curve(
                n, x_lo, x_hi, &hop_a, &hop_b, fee_a, fee_b,
            ));
            let d = u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX);
            us.push(d);
            last = Some(sweep);
        }

        // Sanity BEFORE reporting (RULE 00): the reported timings must be
        // timings of a correct sweep, never of a degraded one.
        let sweep = last.expect("sampled at least one sweep");
        let sweep = sweep.expect("canonical N is envelope-admissible by construction");
        assert_eq!(sweep.buckets, n);
        assert_eq!(
            sweep.points.len(),
            n,
            "evaluator never returns None on this fixture"
        );
        let best = sweep.best.expect("profitable fixture: best must be Some");
        assert!(
            best.net_wei > 0 && best.net_wei <= p_gs,
            "grid argmax (N={}) must be in (0, golden={}], got {}",
            n,
            p_gs,
            best.net_wei
        );
        let ratio_pct = (best.net_wei as f64 / p_gs as f64) * 100.0;

        let pct = |p: u64| -> f64 {
            nearest_rank(&us, p).expect("400 samples, p in 1..=100 — rank always defined") as f64
                / 1000.0
        };

        let line = serde_json::json!({
            "run_id": format!("amount_matrix_N{}_{}", n, ts),
            "ts": ts,
            "matrix_axis": { "axis": "amount_buckets", "N": n },
            "samples": SAMPLES_PER_N,
            "percentiles_ms": {
                "p50": pct(50),
                "p90": pct(90),
                "p95": pct(95),
                "p99": pct(99),
                // nearest_rank(100) IS the max sample — same kernel, no second definition.
                "max": pct(100),
            },
            "cpu_mem": {},
            "env": "local-in-memory",
            "aux": {
                "best_net_wei": best.net_wei,
                "golden_net_wei": p_gs,
                "ratio_vs_golden_pct": (ratio_pct * 100.0).round() / 100.0,
            },
        });
        println!("{}", line);
        eprintln!(
            "N={:>3} · p50 {:.1}µs · p95 {:.1}µs · max {:.1}µs · grid/golden {:.2}%",
            n,
            pct(50) * 1000.0,
            pct(95) * 1000.0,
            pct(100) * 1000.0,
            ratio_pct
        );
    }

    eprintln!("done · 6 rows on stdout (transcribe into .ai-work/PERFORMANCE_RESULTS.json)");
}
