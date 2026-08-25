//! ARBX-QB-07-006 (XLS-QB / REQ-QB-016): the discovery-variables benchmark
//! matrix over the REAL discovery kernel — the workload whose p95 the
//! `Discovery_SLA_ms` gate (ARBX-QB-07-007) must judge.
//!
//! Workbook `10_LATENCY` contract (via `artifacts/quotebase_config.json`,
//! CANONICAL_WORKBOOK): N_Active_Chain=22, Avg_Active_Degree=6,
//! Avg_Parallel_Pools=2.5, Dirty_Seeds=4, Beam_K=4, Max_Hops=7/Min_Hops=2,
//! Discovery_SLA_ms=30 (p95 discovery/ranking). The acceptance criterion for
//! this task names the axes verbatim: "variables: pool_count / avg_degree /
//! dirty_pairs / enabled_strategies / hop / amount_buckets".
//!
//! Matrix design: one-at-a-time (OAT) around the workbook base point — each
//! axis is varied while the others sit at base. Every axis point measures the
//! SAME four-stage discovery tick (`discovery_workload::run_pass`, shared
//! with the in-crate sanity tests — one workload source, no drift):
//!
//! 1. **find** — [`find_routes`] bounded DFS; the stage split (Pair/Expand)
//!    comes from the kernel's own ARBX-0010 clocks, so `pair_ns + expand_ns`
//!    is the pass's exact find wall time.
//! 2. **multihop** — [`find_profitable_cycles`] under the runtime's exact
//!    bound mirror (the pass SHARES the finder's `min/max_depth` knob — "no
//!    new explosion surface"; a selected strategy intersects its HopMask
//!    with those bounds, the real QB-03 consumer path).
//! 3. **rank** — [`rank_by_net_bps`] over the emitted batch (ARBX-0009
//!    sheet-07 ordering) on a deterministic synthetic payload (labeled).
//! 4. **refine** — [`bucket_sweep_2leg_curve`] over the top `Beam_K`
//!    finalists on the byte-identical ARBX-0012 fixture curve, N = the
//!    `amount_buckets` axis value.
//!
//! Each run prints ONE JSON line per axis point to stdout
//! (PERFORMANCE_RESULTS.json §70 schema family — `percentiles_ms` is the
//! TOTAL tick, the SLA number; `stages_ms` splits it); progress goes to
//! stderr so stdout stays parseable. Percentiles use the live telemetry
//! kernel (`latency_budget::nearest_rank`) — a bench p95 and a wire p95 are
//! the same number-shape.
//!
//! Formal timings are release-profile (CI/VPS — AppControl blocks release
//! locally; ARBX-0012 precedent). Dev-profile runs locally still validate
//! harness correctness + JSON shape + RULE 00 sanity — `DM_SAMPLES` /
//! `DM_WARMUP` shrink the local pass count (the multihop stage is an
//! EXHAUSTIVE observe-only DFS when profitable cycles are sparse, so dev
//! profile makes it seconds per pass); every JSON row reports the ACTUAL
//! sample count, so a reduced run can never masquerade as formal.
//!
//! Run: `cargo bench --bench discovery_matrix`
//!
//! ARBX-QB-07-007 gate: the run ends with ONE summary JSON line (`gate` +
//! `resources`). PASS requires a formal run (release profile AND no
//! `DM_SAMPLES`/`DM_WARMUP` override) with the BASE point's p95 strictly
//! under Discovery_SLA_ms=30; a formal FAIL exits non-zero for CI. Dev runs
//! are `not_evaluated_dev_run` and exit 0. Resources: allocs/pass from an
//! UNtimed counting-allocator probe (the timed matrix pays one predicted
//! branch per alloc, never the fetch_add), CPU delta + peak RSS from
//! /proc (Linux — CI/VPS; null off-Linux, R8), and pairs/routes/quotes per
//! second from the run's own accumulated audit counters.

use searcher_rs::amount_buckets::AMOUNT_BUCKETS_CANONICAL;
use searcher_rs::discovery_workload::{
    build_graph, evaluate_sla, is_formal_run, mh_bounds, proc_cpu_seconds, rank_template, run_pass,
    seed_base_tokens, vm_hwm_mib, Fixture, Throughput, BASE_BUCKETS, BASE_DEGREE, BASE_HOP,
    BASE_SEEDS, BASE_TOKENS, DISCOVERY_SLA_MS, MAX_ROUTES,
};
use searcher_rs::latency_budget::nearest_rank;
use searcher_rs::route_discovery::unique_route_finder::RouteFinderConfig;
use searcher_rs::size_optimizer::golden_section_search_2leg;

// ---- ARBX-QB-07-007: resource registration (bench-only, prod untouched) --
//
// Counting allocator: one well-predicted branch per allocation, DISABLED
// during the timed matrix (a `fetch_add` per alloc would inflate a 30 ms
// budget) and enabled only around the untimed alloc-probe passes below.
// `realloc` counts as one event (the reported number is a rate, not a
// ledger).
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

struct CountingAlloc;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static BENCH_ALLOC: CountingAlloc = CountingAlloc;

/// Untimed alloc-probe passes over the BASE point (counter ON, clock off) —
/// allocations per pass, kept OFF the SLA clocks by construction.
const ALLOC_PROBE_PASSES: usize = 16;

/// Warmup passes per point, formal default (unmeasured — page faults, caches,
/// branch predictors). `DM_WARMUP` overrides for local dev validation.
const WARMUP_DEFAULT: usize = 50;

/// Measured passes per point, formal default. Nearest-rank p99 at 400 samples
/// = rank 396 — exact, no interpolation, same as the wire (ARBX-0012
/// convention). Formal runs (CI/VPS release) use the default;
/// `DM_SAMPLES` overrides for local dev validation.
const SAMPLES_DEFAULT: usize = 400;

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

/// One axis point of the OAT matrix: the axis being varied + the full
/// (possibly base) configuration it runs with.
struct Point {
    axis: &'static str,
    tokens: usize,
    degree: usize,
    seeds: usize,
    strategy: Option<&'static str>,
    hop_max: u8,
    buckets: usize,
}

impl Point {
    fn base() -> Self {
        Self {
            axis: "base",
            tokens: BASE_TOKENS,
            degree: BASE_DEGREE,
            seeds: BASE_SEEDS,
            strategy: None,
            hop_max: BASE_HOP,
            buckets: BASE_BUCKETS,
        }
    }
}

fn main() {
    let fx = Fixture::canonical();
    // Live golden reference — the kernel's own answer on the same bracket.
    let (_x_star, p_gs) =
        golden_section_search_2leg(fx.x_lo, fx.x_hi, &fx.hop_a, &fx.hop_b, 30, 30, 25);
    assert!(p_gs > 0, "fixture must be profitable, got {}", p_gs);

    // OAT matrix around the workbook base point — the AC's six axes verbatim.
    let mut points: Vec<Point> = vec![Point::base()];
    for t in [12usize, 22, 44, 88] {
        let mut p = Point::base();
        p.axis = "pool_count";
        p.tokens = t;
        points.push(p);
    }
    for d in [3usize, 6, 9, 12] {
        let mut p = Point::base();
        p.axis = "avg_degree";
        p.degree = d;
        points.push(p);
    }
    for s in [0usize, 4, 16, 64] {
        let mut p = Point::base();
        p.axis = "dirty_pairs";
        p.seeds = s;
        points.push(p);
    }
    // enabled_strategies: mask-less + one id per distinct mask shape
    // (63 = all 2..=7, 31 = 2..=6, 7 = 2..=4, 2 = 0b10 → hop 3 ONLY — the
    // real table's ids; bit `hops−2` gates the hop). Run at the workbook
    // Max_Hops CEILING so the masks differentiate:
    // at the production default max_depth=3 the wider masks clamp to (2,3)
    // — that case is the base point, which this axis overlaps as a
    // cross-consistency check (same config, two axes, two measurements).
    for id in [
        None,
        Some("MEV-01-001"),
        Some("MEV-07-001"),
        Some("MEV-08-001"),
        Some("MEV-01-016"),
    ] {
        let mut p = Point::base();
        p.axis = "enabled_strategies";
        p.strategy = id;
        p.hop_max = 7;
        points.push(p);
    }
    for h in [2u8, 3, 4, 5, 6, 7] {
        let mut p = Point::base();
        p.axis = "hop";
        p.hop_max = h;
        points.push(p);
    }
    for b in AMOUNT_BUCKETS_CANONICAL {
        let mut p = Point::base();
        p.axis = "amount_buckets";
        p.buckets = b;
        points.push(p);
    }

    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let samples = env_usize("DM_SAMPLES", SAMPLES_DEFAULT);
    let warmup = env_usize("DM_WARMUP", WARMUP_DEFAULT);
    let template = rank_template();
    // ARBX-QB-07-007 registration: CPU snapshot before the matrix (Linux only
    // — `None` off-Linux, reported as null, never a fabricated 0 — R8), and
    // the base-point p95 the gate will judge.
    let cpu_start = proc_cpu_seconds();
    let formal = is_formal_run();
    let mut base_p95_ms: Option<f64> = None;
    let mut thr = Throughput::default();
    eprintln!(
        "ARBX-QB-07-006 discovery matrix · {} OAT points over base (tokens={} degree={} seeds={} hop={} buckets={}) · golden p* = {} net wei · {} samples (+{} warmup) · nearest-rank",
        points.len(),
        BASE_TOKENS,
        BASE_DEGREE,
        BASE_SEEDS,
        BASE_HOP,
        BASE_BUCKETS,
        p_gs,
        samples,
        warmup
    );

    for point in &points {
        let sg = build_graph(point.tokens, point.degree);
        let base_tokens = seed_base_tokens(point.tokens, point.seeds);
        let cfg = RouteFinderConfig {
            min_depth: 2,
            max_depth: point.hop_max,
            max_pools_per_pair: 8,
            max_routes_per_tick: MAX_ROUTES,
            base_tokens,
            mode: "shadow".to_string(),
            hop_mask_strategy_id: None, // the main finder serves every coarse strategy (QB-03 contract)
        };
        let bounds = mh_bounds(point.hop_max, point.strategy);

        for _ in 0..warmup {
            let _ = run_pass(&sg.graph, &cfg, bounds, point.buckets, &fx, &template);
        }

        let mut tot: Vec<u64> = Vec::with_capacity(samples);
        let mut fin: Vec<u64> = Vec::with_capacity(samples);
        let mut mho: Vec<u64> = Vec::with_capacity(samples);
        let mut rnk: Vec<u64> = Vec::with_capacity(samples);
        let mut refi: Vec<u64> = Vec::with_capacity(samples);
        for _ in 0..samples {
            let (stages, audit, _) =
                run_pass(&sg.graph, &cfg, bounds, point.buckets, &fx, &template);
            thr.record(sg.n_pairs as u64, &audit, stages.total);
            tot.push(stages.total);
            fin.push(stages.find);
            mho.push(stages.multihop);
            rnk.push(stages.rank);
            refi.push(stages.refine);
        }

        // Sanity BEFORE reporting (RULE 00): the reported timings must be
        // timings of a correct pass, never of a degraded one.
        let (stages, audit, best_nets) =
            run_pass(&sg.graph, &cfg, bounds, point.buckets, &fx, &template);
        assert!(
            audit.routes > 0,
            "parallel pools on every pair ⇒ cycles exist"
        );
        assert!(
            audit.cycles > 0,
            "profitable pools every 23rd ⇒ negative cycles exist"
        );
        assert!(
            audit.finalists > 0,
            "routes > 0 ⇒ at least one refine finalist"
        );
        for net in &best_nets {
            assert!(
                *net > 0 && *net <= p_gs,
                "grid argmax must be in (0, golden={}], got {}",
                p_gs,
                net
            );
        }
        let stages_sum = stages.find + stages.multihop + stages.rank + stages.refine;
        assert!(
            stages_sum <= stages.total,
            "disjoint stage clocks on one monotonic clock: sum {} > total {}",
            stages_sum,
            stages.total
        );

        let pct_ms = |v: &[u64], p: u64| -> f64 {
            nearest_rank(v, p).expect("samples ≥ 1, p in 1..=100 — rank always defined") as f64
                / 1e6
        };
        let pct_obj = |v: &[u64]| -> serde_json::Value {
            serde_json::json!({
                "p50": pct_ms(v, 50),
                "p90": pct_ms(v, 90),
                "p95": pct_ms(v, 95),
                "p99": pct_ms(v, 99),
            })
        };
        if point.axis == "base" {
            // The SLA judges the workbook BASE point only — the 29 axis
            // points are sensitivity exploration, not the contract.
            base_p95_ms = Some(pct_ms(&tot, 95));
        }

        let strategy_label = point.strategy.unwrap_or("none");
        let value = match point.axis {
            "pool_count" => sg.n_pools.to_string(),
            "avg_degree" => point.degree.to_string(),
            "dirty_pairs" => point.seeds.to_string(),
            "enabled_strategies" => strategy_label.to_string(),
            "hop" => point.hop_max.to_string(),
            "amount_buckets" => point.buckets.to_string(),
            _ => "base".to_string(),
        };
        let line = serde_json::json!({
            "run_id": format!("discovery_matrix_{}_{}_{}", point.axis, value, ts),
            "ts": ts,
            "matrix_axis": {
                "axis": point.axis,
                "value": value,
                "tokens": point.tokens,
                "degree": point.degree,
                "pairs": sg.n_pairs,
                "pools": sg.n_pools,
                "avg_parallel_pools": sg.avg_parallel,
                "seeds": point.seeds,
                "strategy": strategy_label,
                // null = empty mask∩depth intersection ⇒ the pass was the
                // worker's honest skip (R8) — reported, never hidden.
                "multihop_bounds": bounds,
                "hop_max": point.hop_max,
                "buckets": point.buckets,
            },
            "samples": samples,
            "percentiles_ms": pct_obj(&tot), // TOTAL tick — the SLA number
            "stages_ms": {
                "find": pct_obj(&fin),
                "multihop": pct_obj(&mho),
                "rank": pct_obj(&rnk),
                "refine": pct_obj(&refi),
            },
            "aux": {
                "edges": sg.graph.edges.len(),
                "routes": audit.routes,
                "capped": audit.capped,
                "pools_truncated": audit.pools_truncated,
                "cycles": audit.cycles,
                "finalists": audit.finalists,
                "quotes": audit.quotes,
            },
        });
        println!("{}", line);
        eprintln!(
            "  [{}] value={} pools={} routes={} cycles={} p95_total={:.3}ms (find {:.3} · mh {:.3} · rank {:.3} · refine {:.3})",
            point.axis,
            value,
            sg.n_pools,
            audit.routes,
            audit.cycles,
            pct_ms(&tot, 95),
            pct_ms(&fin, 95),
            pct_ms(&mho, 95),
            pct_ms(&rnk, 95),
            pct_ms(&refi, 95),
        );
    }

    // ---- ARBX-QB-07-007: gate verdict + resource registration -----------
    // Alloc probe over the BASE point: counter ON, clock OFF (the SLA clocks
    // above never paid the fetch_add — only this probe's branch, untimed).
    let sg = build_graph(BASE_TOKENS, BASE_DEGREE);
    let cfg = RouteFinderConfig {
        min_depth: 2,
        max_depth: BASE_HOP,
        max_pools_per_pair: 8,
        max_routes_per_tick: MAX_ROUTES,
        base_tokens: seed_base_tokens(BASE_TOKENS, BASE_SEEDS),
        mode: "shadow".to_string(),
        hop_mask_strategy_id: None,
    };
    let bounds = mh_bounds(BASE_HOP, None);
    ALLOCS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for _ in 0..ALLOC_PROBE_PASSES {
        let _ = run_pass(&sg.graph, &cfg, bounds, BASE_BUCKETS, &fx, &template);
    }
    COUNTING.store(false, Ordering::Relaxed);
    let allocs_per_pass = ALLOCS.load(Ordering::Relaxed) as f64 / ALLOC_PROBE_PASSES as f64;

    let cpu_delta = match (cpu_start, proc_cpu_seconds()) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None, // off-Linux / unreadable → null, never a fabricated 0 (R8)
    };
    // Fail-closed by construction: an unmeasured base p95 (impossible with
    // samples ≥ 1) would be NaN, and `NaN < budget` is false → FAIL.
    let verdict = evaluate_sla(formal, base_p95_ms.unwrap_or(f64::NAN));
    let summary = serde_json::json!({
        "run_id": format!("discovery_gate_{}", ts),
        "ts": ts,
        "gate": {
            "budget_ms": DISCOVERY_SLA_MS,
            "formal": verdict.formal,
            "p95_ms": verdict.p95_ms,
            "pass": verdict.pass,
            "verdict": if !verdict.formal {
                "not_evaluated_dev_run"
            } else if verdict.pass {
                "PASS"
            } else {
                "FAIL"
            },
        },
        "resources": {
            "cpu_seconds": cpu_delta,
            "peak_rss_mib": vm_hwm_mib(),
            "allocs_per_pass": allocs_per_pass,
            "alloc_probe_passes": ALLOC_PROBE_PASSES,
            "throughput": {
                "passes": thr.passes,
                "pairs_per_sec": Throughput::per_sec(thr.pair_scans, thr.elapsed_ns),
                "routes_per_sec": Throughput::per_sec(thr.routes, thr.elapsed_ns),
                "quotes_per_sec": Throughput::per_sec(thr.quotes, thr.elapsed_ns),
                "measured_tick_ms_total": thr.elapsed_ns as f64 / 1e6,
            },
        },
    });
    println!("{}", summary);
    eprintln!(
        "GATE {} formal={} base_p95={:.3}ms budget={}ms (dev/prof-shrunk runs are never PASS)",
        if !verdict.formal {
            "NOT_EVALUATED"
        } else if verdict.pass {
            "PASS"
        } else {
            "FAIL"
        },
        verdict.formal,
        verdict.p95_ms,
        DISCOVERY_SLA_MS,
    );
    // A formal FAIL is a hard gate — non-zero exit so CI surfaces it. Dev
    // runs exit 0 (validation tooling, not gate invocations).
    if verdict.formal && !verdict.pass {
        std::process::exit(1);
    }
}

// wdac-probe rev-3 (content-hash refresh — ARBX-0009 playbook)
