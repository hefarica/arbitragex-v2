//! ARBX-QB-07-006 (XLS-QB / REQ-QB-016): the canonical discovery-workload
//! builders shared by the `discovery_matrix` bench and this module's own
//! sanity tests — the workload whose p95 the `Discovery_SLA_ms` gate
//! (ARBX-QB-07-007) must judge.
//!
//! Workbook `10_LATENCY` contract (via `artifacts/quotebase_config.json`,
//! CANONICAL_WORKBOOK): N_Active_Chain=22, Avg_Active_Degree=6,
//! Avg_Parallel_Pools=2.5, Dirty_Seeds=4, Beam_K=4, Max_Hops=7/Min_Hops=2,
//! Discovery_SLA_ms=30 (p95 discovery/ranking).
//!
//! Why a lib module and not bench-local code: the workload builders are the
//! CONTRACT the gate measures — keeping them in-crate (ARBX-0012 fixture
//! precedent: the golden curve lives beside its unit test, the bench only
//! imports kernels) means the unit tests below EXECUTE the exact pipeline
//! the bench times, so harness correctness is provable on any machine that
//! can run `cargo test --lib` (the bench's formal timings still need a
//! release-profile run — CI/VPS; AppControl blocks release locally).
//!
//! The synthetic graph is deterministic (index arithmetic, no RNG): circulant
//! topology for exact degree control, parallel pools alternating 2/3 (avg
//! 2.5), every 23rd pool carrying a profitable rate so negative cycles exist
//! (the multihop pass does real work, and `cycles > 0` is assertable). All
//! magnitudes are labeled bench payload (RULE 00), never production
//! telemetry.

use std::collections::{BTreeSet, HashMap};
use std::hint::black_box;
use std::time::Instant;

use ethers::types::{Address, U256};

use crate::amount_buckets::AMOUNT_BUCKETS_CANONICAL;
use crate::dirty_pairs::{DirtyPairSet, HotSeedQueue};
use crate::net_bps_ranking::{rank_by_net_bps, RankedRoute, RouteNetEconomics};
use crate::pair_index::{pair_count, pair_index, pair_unindex};
use crate::route_discovery::graph_builder::TokenGraph;
use crate::route_discovery::multi_hop_search::find_profitable_cycles;
use crate::route_discovery::types::{RouteDirection, RouteEdge};
use crate::route_discovery::unique_route_finder::{find_routes, RouteFinderConfig};
use crate::route_intent::ProtocolType;
use crate::size_optimizer::{bucket_sweep_2leg_curve, golden_section_search_2leg};
use crate::strategy_hop_mask::admissible_hop_bounds;

/// Workbook base point (10_LATENCY / 01_CONFIG, CANONICAL_WORKBOOK).
pub const BASE_TOKENS: usize = 22; // N_Active_Chain
pub const BASE_DEGREE: usize = 6; // Avg_Active_Degree
pub const BASE_SEEDS: usize = 4; // Dirty_Seeds
pub const BASE_HOP: u8 = 3; // finder default max_depth (2- and 3-cycles)
pub const BASE_BUCKETS: usize = 32; // canonical amount_buckets mid
pub const BEAM_K: usize = 4; // Beam_K — refine finalists
pub const MAX_ROUTES: usize = 500; // anti-explosion cap (kernel default)

/// `1e18 * n` in wei — same helper semantics as the size_optimizer test
/// module (shared here so bench and tests cannot drift).
pub fn unit(n: u64) -> U256 {
    U256::from(10u128).pow(U256::from(18u32)) * U256::from(n)
}

pub fn token_addr(i: usize) -> Address {
    Address::from_low_u64_be(0x1000 + i as u64)
}

/// Distinct unordered pairs of the circulant topology: ring offsets
/// `1..=degree/2` on both sides (even degree), plus one antipodal chord per
/// token when the degree is odd (exact degree control, deterministic, no RNG).
pub fn circulant_pairs(n: usize, degree: usize) -> Vec<(usize, usize)> {
    assert!(
        n % 2 == 0,
        "antipodal chord (odd degree) needs an even token count"
    );
    let mut pairs = BTreeSet::new();
    let half = degree / 2;
    for i in 0..n {
        for o in 1..=half {
            let j = (i + o) % n;
            if i != j {
                pairs.insert((i.min(j), i.max(j)));
            }
        }
    }
    if degree % 2 == 1 {
        for i in 0..n / 2 {
            pairs.insert((i, i + n / 2));
        }
    }
    pairs.into_iter().collect()
}

pub struct SyntheticGraph {
    pub graph: TokenGraph,
    pub n_pairs: usize,
    pub n_pools: usize,
    pub avg_parallel: f64,
}

/// Deterministic synthetic universe: `n` tokens, exact `degree` topology,
/// parallel pools alternating 2/3 (workbook Avg_Parallel_Pools 2.5). Every
/// 23rd pool carries a profitable rate (`(1−fee)·rate > 1`) so negative
/// cycles exist and the multihop pass does real work; the rest are near-par
/// with an index-derived spread so `rank_parallel_pools` has real ties to
/// break.
pub fn build_graph(n: usize, degree: usize) -> SyntheticGraph {
    let pairs = circulant_pairs(n, degree);
    let mut edges: Vec<RouteEdge> = Vec::new();
    let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
    let mut pool_counter: u64 = 0;
    for (p, (a, b)) in pairs.iter().enumerate() {
        // Workbook Avg_Parallel_Pools = 2.5 → alternate 2 and 3 per pair.
        let parallel = 2 + (p % 2) as u64;
        for _ in 0..parallel {
            let pool = Address::from_low_u64_be(0x2000_0000 + pool_counter);
            let (ta, tb) = (token_addr(*a), token_addr(*b));
            let (t0, t1) = if ta < tb { (ta, tb) } else { (tb, ta) };
            let profitable = pool_counter % 23 == 5;
            let rate = if profitable {
                1.01
            } else {
                1.0 + 1e-4 * ((pool_counter.wrapping_mul(2_654_435_761) % 997) as f64 / 997.0)
            };
            let log_weight = -(0.997_f64 * rate).ln(); // MMBF −ln((1−fee)·rate)
            let liquidity = 50_000.0 + 1_000.0 * ((pool_counter % 11) as f64);
            for (tin, tout) in [(ta, tb), (tb, ta)] {
                let idx = edges.len();
                edges.push(RouteEdge {
                    chain_id: 1,
                    pool,
                    token_in: tin,
                    token_out: tout,
                    token0: t0,
                    token1: t1,
                    protocol: ProtocolType::V2,
                    fee_bps: Some(30),
                    liquidity_hint: Some(liquidity),
                    log_weight: Some(log_weight),
                    freshness_ts: 1_700_000_000,
                    blk: 1,
                    hot_token: *a < 4 || *b < 4, // first 4 tokens play the hub roles
                    direction: RouteDirection::from_in_token0(tin, t0),
                });
                adjacency.entry(tin).or_default().push(idx);
            }
            pool_counter += 1;
        }
    }
    let n_pools = pool_counter as usize;
    SyntheticGraph {
        graph: TokenGraph {
            edges,
            adjacency,
            dense: None, // find_routes/multi_hop_search read `adjacency` (the production path)
        },
        n_pairs: pairs.len(),
        n_pools,
        avg_parallel: n_pools as f64 / pairs.len().max(1) as f64,
    }
}

/// Dirty-seed start set derived through the REAL QB-05 structures (not
/// hand-picked pairs): pair `k` of the C(n,2) triangular index →
/// `DirtyPairSet::mark` + `HotSeedQueue` seed → drained once OUTSIDE timing
/// (seed resolution is O(seeds) upstream event-arrival work; the tick
/// receives its start set — the same boundary as the runtime consumer).
/// Empty set ⇒ cold full scan (every token starts), the axis zero point.
pub fn seed_base_tokens(n: usize, seeds: usize) -> Vec<Address> {
    if seeds == 0 {
        return Vec::new();
    }
    assert!(seeds <= pair_count(n), "seeds={} exceed C({},2)", seeds, n);
    let mut set = DirtyPairSet::new(n);
    let mut queue = HotSeedQueue::with_capacity(seeds);
    let mut marked = 0usize;
    for k in 0..seeds {
        let (i, j) = pair_unindex(k, n).expect("k < C(n,2) asserted above");
        let idx = pair_index(i, j, n).expect("pair_index∘pair_unindex round-trips");
        debug_assert_eq!(idx, k, "triangular index round-trip");
        if set.mark(idx) {
            marked += 1;
            queue.push(idx);
        }
    }
    assert_eq!(marked, seeds, "distinct seed pairs by construction");
    let mut starts: Vec<usize> = Vec::with_capacity(2 * seeds);
    while let Some(k) = queue.pop() {
        let (i, j) = pair_unindex(k, n).expect("seeded from valid indices");
        starts.push(i);
        starts.push(j);
    }
    starts.sort_unstable();
    starts.dedup();
    starts.into_iter().map(token_addr).collect()
}

/// Stage wall times of one discovery tick (ns). `find` is the kernel's own
/// Pair+Expand split (ARBX-0010 identity); the others are external clocks
/// over disjoint sub-intervals of `total` (same monotonic clock — the sum of
/// stages is assertably ≤ total).
#[derive(Debug, Clone, Copy, Default)]
pub struct StageNs {
    pub total: u64,
    pub find: u64,
    pub multihop: u64,
    pub rank: u64,
    pub refine: u64,
}

/// Honest counts of one pass (aux telemetry — caps are reported, never hidden).
#[derive(Debug, Clone, Default)]
pub struct PassAudit {
    pub routes: usize,
    pub capped: bool,
    pub pools_truncated: bool,
    pub cycles: usize,
    pub finalists: usize,
    /// Curve evaluations performed by the refine stage (RULE 00: counted
    /// from the sweeps' own `points.len()`, never assumed from N).
    pub quotes: usize,
}

pub struct Fixture {
    pub hop_a: Vec<(U256, U256)>,
    pub hop_b: Vec<(U256, U256)>,
    pub x_lo: U256,
    pub x_hi: U256,
}

impl Fixture {
    /// ARBX-0012 fixture twin: pool A buys deep, pool B sells back higher.
    /// The golden reference (the kernel's own answer on the same bracket)
    /// is computed LIVE by callers — never a hardcoded literal.
    pub fn canonical() -> Self {
        Self {
            hop_a: vec![(unit(1000), unit(2_000_000))],
            hop_b: vec![(unit(1_000_000), unit(600))],
            x_lo: U256::from(1u64),
            x_hi: unit(1000),
        }
    }
}

/// Precomputed ranking payload template for the full route cap: `(route_key,
/// target net_bps)` — `None` = `not_computable`. Built ONCE per run; each
/// pass rebuilds a fresh unsorted `Vec<RankedRoute>` from it OUTSIDE the
/// clock (the stage is the comparator, not the `format!` — and every pass
/// sorts a fresh unsorted batch, the real pipeline's cold-sort shape).
pub fn rank_template() -> Vec<(String, Option<f64>)> {
    (0..MAX_ROUTES)
        .map(|idx| {
            let key = format!("bench-{idx:06}");
            if idx % 7 == 3 {
                (key, None)
            } else {
                (key, Some(60.0 - (idx % 100) as f64 * 0.5))
            }
        })
        .collect()
}

/// Multi-hop bounds — the runtime's EXACT mirror (`route_discovery_worker`:
/// `mh_max_hops = finder.max_depth.clamp(2, 7)`, `mh_min_hops =
/// finder.min_depth.clamp(2, mh_max)`): the observe-only pass SHARES the
/// finder's depth knob ("no new explosion surface"), so the hop axis drives
/// both stages together. A selected strategy intersects its HopMask with
/// those shared bounds (the real QB-03 consumer path). `None` = empty
/// intersection ⇒ the pass is SKIPPED with a reason (R8) — the worker's
/// honest skip, never a silently-empty search.
pub fn mh_bounds(hop_max: u8, strategy: Option<&str>) -> Option<(u8, u8)> {
    let lo = 2u8; // finder.min_depth — canonical floor
    let hi = hop_max.clamp(2, 7); // finder.max_depth
    match strategy {
        None => Some((lo, hi)),
        Some(id) => admissible_hop_bounds(id, lo, hi),
    }
}

/// One four-stage discovery tick: find (bounded DFS) → multihop (negative
/// cycles; SKIPPED with cycles=0 when the bounds intersection is empty —
/// the worker's honest skip, R8) → rank (sheet-07 Net_bps, ARBX-0009) →
/// refine (Beam_K bucket sweeps). Deterministic synthetic ranking payload
/// (labeled): net_bps spread with every 7th entry `not_computable` so
/// None-last ordering is exercised — this stage measures the comparator on
/// the batch, not real route economics (RULE 00: bench payload).
#[allow(clippy::too_many_arguments)]
pub fn run_pass(
    graph: &TokenGraph,
    cfg: &RouteFinderConfig,
    bounds: Option<(u8, u8)>,
    buckets: usize,
    fx: &Fixture,
    template: &[(String, Option<f64>)],
) -> (StageNs, PassAudit, Vec<i128>) {
    let t_total = Instant::now();

    // Stage 1 — bounded 2..=hop DFS (kernel Pair/Expand split).
    let outcome = black_box(find_routes(graph, 1, cfg));
    let find_ns = outcome.timings.pair_ns + outcome.timings.expand_ns;

    // Stage 2 — observe-only multi-hop negative-cycle pass under shared
    // bounds; an empty intersection skips the pass (0 cycles, ~0 ns).
    let t = Instant::now();
    let mh = black_box(match bounds {
        Some((lo, hi)) => find_profitable_cycles(graph, lo as usize, hi as usize, MAX_ROUTES),
        None => crate::route_discovery::multi_hop_search::MultiHopResult {
            cycles: Vec::new(),
            capped: false,
            dropped_for_cap: 0,
            v3_skipped: 0,
            noise_dropped: 0,
        },
    });
    let multihop_ns = u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX);

    // Stage 3 — sheet-07 Net_bps ranking of the emitted batch (ARBX-0009).
    let start = 10_000.0_f64;
    let mut ranked: Vec<RankedRoute> = template[..outcome.routes.len()]
        .iter()
        .map(|(route_key, target)| match target {
            None => RankedRoute {
                route_key: route_key.clone(),
                economics: RouteNetEconomics::not_computable(),
            },
            Some(target_bps) => RankedRoute {
                route_key: route_key.clone(),
                economics: RouteNetEconomics {
                    start_amount_usd: start,
                    // net_profit = gross − 17 (gas 10 + flash 5 + other 2) ⇒ net_bps = target
                    gross_over_input_usd: start * target_bps / 10_000.0 + 17.0,
                    gas_usd: 10.0,
                    flash_fee_usd: 5.0,
                    builder_tip_usd: 0.0,
                    other_cost_usd: 2.0,
                },
            },
        })
        .collect();
    let t = Instant::now();
    rank_by_net_bps(&mut ranked);
    let rank_ns = u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX);

    // Stage 4 — amount-aware refinement of the top Beam_K finalists over the
    // ARBX-0012 fixture curve, N = the amount_buckets axis value.
    let t = Instant::now();
    let finalists = ranked.len().min(BEAM_K);
    let mut best_nets = Vec::with_capacity(finalists);
    let mut quotes = 0usize;
    for _ in 0..finalists {
        let sweep =
            bucket_sweep_2leg_curve(buckets, fx.x_lo, fx.x_hi, &fx.hop_a, &fx.hop_b, 30, 30)
                .expect("canonical fixture is envelope-admissible by construction");
        quotes += sweep.points.len();
        best_nets.push(
            sweep
                .best
                .expect("profitable fixture: best is Some")
                .net_wei,
        );
    }
    let refine_ns = u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX);

    let total_ns = u64::try_from(t_total.elapsed().as_nanos()).unwrap_or(u64::MAX);
    (
        StageNs {
            total: total_ns,
            find: find_ns,
            multihop: multihop_ns,
            rank: rank_ns,
            refine: refine_ns,
        },
        PassAudit {
            routes: outcome.routes.len(),
            capped: outcome.capped,
            pools_truncated: outcome.pools_truncated,
            cycles: mh.cycles.len(),
            finalists,
            quotes,
        },
        best_nets,
    )
}

// ---- ARBX-QB-07-007: Discovery_SLA_ms gate + resource registration ----

/// Workbook `10_LATENCY` Discovery_SLA_ms — the p95 budget of the TOTAL
/// discovery tick at the BASE point, milliseconds (CANONICAL_WORKBOOK via
/// `artifacts/quotebase_config.json`). The workbook's own mandate: "<30 ms
/// NO puede garantizarse por Excel. Debe demostrarse con benchmarks".
pub const DISCOVERY_SLA_MS: f64 = 30.0;

/// Verdict of the Discovery_SLA gate. PASS requires BOTH a formal run
/// (release profile AND no `DM_SAMPLES`/`DM_WARMUP` shrink override) and a
/// measured base-point p95 strictly under the budget. A dev/reduced run is
/// never PASS — it cannot masquerade as formal (every row already reports
/// its real sample count). NaN p95 fails closed (`NaN < budget` is false).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlaVerdict {
    /// Release profile AND no sample-count overrides present.
    pub formal: bool,
    /// Measured base-point p95 of the TOTAL tick (ms).
    pub p95_ms: f64,
    /// Gate outcome — `true` ONLY on formal + strictly-under-budget runs.
    pub pass: bool,
}

/// Evaluate the gate against a measured base-point p95. Strict `<` per the
/// workbook contract (exactly 30.000 ms is over budget).
pub fn evaluate_sla(formal: bool, p95_ms: f64) -> SlaVerdict {
    SlaVerdict {
        formal,
        p95_ms,
        pass: formal && p95_ms < DISCOVERY_SLA_MS,
    }
}

/// Is this invocation a FORMAL gate run? Both honest conditions: the binary
/// was built without `debug_assertions` (bench/release profile — AppControl
/// blocks release locally, so formal timing is a CI/VPS artifact, ARBX-0012
/// precedent) AND neither `DM_SAMPLES` nor `DM_WARMUP` is present (a shrunk
/// run is dev validation, not the formal workload).
pub fn is_formal_run() -> bool {
    !cfg!(debug_assertions)
        && std::env::var_os("DM_SAMPLES").is_none()
        && std::env::var_os("DM_WARMUP").is_none()
}

/// CPU seconds (utime + stime) parsed from a `/proc/self/stat` line. The
/// comm field may contain spaces, so parsing starts after the LAST `)`;
/// utime/stime are fields 14/15 after it (11 tokens from state inclusive).
/// Clock: `USER_HZ = 100` (Linux default, what CI/VPS runners expose).
/// `None` on malformed input — absent file is a platform concern (Linux
/// only; a Windows dev run reports `null`, never a fabricated 0 — R8).
pub fn parse_proc_cpu_seconds(stat_line: &str) -> Option<f64> {
    let after = stat_line.rsplit_once(')')?.1;
    let mut fields = after.split_whitespace().skip(11);
    let utime: u64 = fields.next()?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    Some((utime + stime) as f64 / 100.0)
}

/// Peak resident set (MiB) parsed from a `/proc/self/status` body (`VmHWM:`
/// line, kB). `None` when absent/malformed (R8 — never a fabricated 0).
pub fn parse_vm_hwm_mib(status_body: &str) -> Option<f64> {
    let line = status_body.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb as f64 / 1024.0)
}

/// Live (utime+stime) CPU seconds of this process, or `None` off-Linux.
pub fn proc_cpu_seconds() -> Option<f64> {
    parse_proc_cpu_seconds(&std::fs::read_to_string("/proc/self/stat").ok()?)
}

/// Live peak RSS (MiB) of this process, or `None` off-Linux.
pub fn vm_hwm_mib() -> Option<f64> {
    parse_vm_hwm_mib(&std::fs::read_to_string("/proc/self/status").ok()?)
}

/// Throughput registration from the run's OWN accumulated audit counters
/// (never assumed from configuration): additive accounting over the measured
/// passes; per-second rates are derived, and `per_sec` refuses to invent a
/// rate without wall time (`None`, R8).
#[derive(Debug, Clone, Copy, Default)]
pub struct Throughput {
    pub passes: u64,
    pub routes: u64,
    pub quotes: u64,
    pub pair_scans: u64,
    /// Sum of the per-pass TOTAL tick clocks (the tick-time denominator —
    /// between-pass overhead is excluded by construction).
    pub elapsed_ns: u64,
}

impl Throughput {
    /// Account one measured pass. `pairs` is the point's pair universe (a
    /// full-tick scan surface; seeded passes touch a subset — the counter
    /// stays the honest upper surface, never a fabricated exact touch count).
    pub fn record(&mut self, pairs: u64, audit: &PassAudit, pass_total_ns: u64) {
        self.passes += 1;
        self.routes += audit.routes as u64;
        self.quotes += audit.quotes as u64;
        self.pair_scans += pairs;
        self.elapsed_ns += pass_total_ns;
    }

    /// Derived per-second rate; `None` without wall time (R8).
    pub fn per_sec(count: u64, elapsed_ns: u64) -> Option<f64> {
        if elapsed_ns == 0 {
            return None;
        }
        Some(count as f64 / (elapsed_ns as f64 / 1e9))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Topology is exact: every token touches exactly `degree` distinct
    /// neighbors and the pair count is the closed-form `n·degree/2`.
    #[test]
    fn circulant_degree_is_exact() {
        for (n, d) in [(22, 6), (22, 3), (22, 9), (12, 6), (44, 6)] {
            let pairs = circulant_pairs(n, d);
            assert_eq!(pairs.len(), n * d / 2, "pair count (n={}, d={})", n, d);
            let mut degree = vec![0usize; n];
            for (a, b) in &pairs {
                degree[*a] += 1;
                degree[*b] += 1;
            }
            assert!(
                degree.iter().all(|k| *k == d),
                "exact degree (n={}, d={})",
                n,
                d
            );
        }
    }

    /// Parallel pools alternate 2/3 → the average sits inside (2.4, 2.6)
    /// (workbook Avg_Parallel_Pools = 2.5), and every pool yields both
    /// directed edges (out-degree consistency).
    #[test]
    fn parallel_pools_average_is_workbook_2_5() {
        let sg = build_graph(BASE_TOKENS, BASE_DEGREE);
        assert_eq!(
            sg.n_pools * 2,
            sg.graph.edges.len(),
            "two directed edges per pool"
        );
        assert!(
            sg.avg_parallel > 2.4 && sg.avg_parallel < 2.6,
            "avg parallel {} should straddle 2.5",
            sg.avg_parallel
        );
        for v in sg.graph.adjacency.values() {
            assert!(!v.is_empty());
        }
    }

    /// Dirty seeds flow through the REAL QB-05 structures and round-trip the
    /// triangular pair index; zero seeds ⇒ the cold full-scan empty start set.
    #[test]
    fn seeds_round_trip_and_starts() {
        assert!(seed_base_tokens(BASE_TOKENS, 0).is_empty());
        let starts = seed_base_tokens(BASE_TOKENS, BASE_SEEDS);
        // k=0..3 of the triangular index = row 0 pairs (0,1)..(0,4) → tokens
        // {0..=4} deduped-sorted (token 0 appears in all four pairs).
        let expected: Vec<Address> = (0..=BASE_SEEDS).map(token_addr).collect();
        assert_eq!(starts, expected, "first 4 pairs are (0,1)..(0,4)");
        // seeds beyond C(n,2) is a caller bug — fail fast, not silently.
        assert!(std::panic::catch_unwind(|| seed_base_tokens(4, 7)).is_err());
    }

    /// One full base-point tick satisfies every RULE 00 sanity the bench
    /// asserts before reporting: routes exist, negative cycles exist, every
    /// refine finalist's grid argmax lands in (0, golden], and the disjoint
    /// stage clocks sum to ≤ the total on one monotonic clock.
    #[test]
    fn pipeline_base_pass_sanity() {
        let sg = build_graph(BASE_TOKENS, BASE_DEGREE);
        let base_tokens = seed_base_tokens(BASE_TOKENS, BASE_SEEDS);
        let cfg = RouteFinderConfig {
            min_depth: 2,
            max_depth: BASE_HOP,
            max_pools_per_pair: 8,
            max_routes_per_tick: MAX_ROUTES,
            base_tokens,
            mode: "shadow".to_string(),
            hop_mask_strategy_id: None,
        };
        let fx = Fixture::canonical();
        let (_x, p_gs) =
            golden_section_search_2leg(fx.x_lo, fx.x_hi, &fx.hop_a, &fx.hop_b, 30, 30, 25);
        assert!(p_gs > 0, "fixture must be profitable");

        let (stages, audit, best_nets) = run_pass(
            &sg.graph,
            &cfg,
            mh_bounds(BASE_HOP, None),
            BASE_BUCKETS,
            &fx,
            &rank_template(),
        );

        assert!(
            audit.routes > 0,
            "parallel pools on every pair ⇒ cycles exist"
        );
        assert!(
            audit.cycles > 0,
            "profitable pools every 23rd ⇒ negative cycles exist"
        );
        assert!(audit.finalists > 0);
        assert!(audit.quotes > 0);
        for net in &best_nets {
            assert!(
                *net > 0 && *net <= p_gs,
                "argmax {} in (0, golden {}]",
                net,
                p_gs
            );
        }
        let sum = stages.find + stages.multihop + stages.rank + stages.refine;
        assert!(
            sum <= stages.total,
            "disjoint clocks: {} > {}",
            sum,
            stages.total
        );
    }

    /// The HopMask intersection path (workbook step 9, QB-03): bit `hops−2`
    /// of the mask gates each hop, so mask 2 (0b10) admits ONLY hop 3 (a
    /// single-tier strategy), mask 7 spans 2..=4, 31 spans 2..=6, and the
    /// mask-less ceiling stays (2, hop_max).
    #[test]
    fn hop_mask_bounds_intersection() {
        assert_eq!(mh_bounds(7, None), Some((2, 7)));
        assert_eq!(mh_bounds(3, None), Some((2, 3)));
        assert_eq!(mh_bounds(7, Some("MEV-01-016")), Some((3, 3))); // mask 0b10 → hop 3 only
        assert_eq!(mh_bounds(7, Some("MEV-08-001")), Some((2, 4))); // mask 0b111
        assert_eq!(mh_bounds(7, Some("MEV-07-001")), Some((2, 6))); // mask 0b11111
        assert_eq!(mh_bounds(3, Some("MEV-07-001")), Some((2, 3))); // clamped by shared depth
        assert_eq!(mh_bounds(2, Some("MEV-01-016")), None); // {3} ∩ [2,2] = ∅
    }

    /// A hop-3-only pass still finds 3-hop negative cycles, and the SAME
    /// strategy under a depth-2 knob is the worker's honest skip (R8): find
    /// still emits its routes, multihop contributes exactly zero cycles.
    #[test]
    fn hop3_only_strategy_runs_or_skips() {
        let sg = build_graph(BASE_TOKENS, BASE_DEGREE);
        let base_tokens = seed_base_tokens(BASE_TOKENS, BASE_SEEDS);
        let cfg_for = |max_depth: u8| RouteFinderConfig {
            min_depth: 2,
            max_depth,
            max_pools_per_pair: 8,
            max_routes_per_tick: MAX_ROUTES,
            base_tokens: base_tokens.clone(),
            mode: "shadow".to_string(),
            hop_mask_strategy_id: None,
        };
        let fx = Fixture::canonical();
        let template = rank_template();

        let (_, audit3, _) = run_pass(
            &sg.graph,
            &cfg_for(3),
            mh_bounds(3, Some("MEV-01-016")),
            8,
            &fx,
            &template,
        );
        assert!(audit3.routes > 0, "find stage runs regardless of bounds");
        assert!(
            audit3.cycles > 0,
            "hop-3-only pass finds 3-hop negative cycles"
        );

        let (_, audit_skip, _) = run_pass(
            &sg.graph,
            &cfg_for(2),
            mh_bounds(2, Some("MEV-01-016")),
            8,
            &fx,
            &template,
        );
        assert!(audit_skip.routes > 0, "find is bounds-independent");
        assert_eq!(
            audit_skip.cycles, 0,
            "empty intersection ⇒ honest skip (R8)"
        );
    }

    /// Gate truth table (ARBX-QB-07-007): PASS ONLY on a formal run with a
    /// measured p95 strictly under budget. A dev run cannot claim PASS even
    /// at 0.5 ms; exactly 30.000 ms is over budget; NaN fails closed.
    #[test]
    fn sla_truth_table() {
        assert!(!evaluate_sla(false, 0.5).pass, "dev run can never PASS");
        assert!(!evaluate_sla(false, 0.5).formal);
        assert!(evaluate_sla(true, 29.999).pass, "formal + under budget");
        assert!(
            !evaluate_sla(true, DISCOVERY_SLA_MS).pass,
            "strictly under budget — exactly 30 ms FAILS"
        );
        assert!(!evaluate_sla(true, 45.0).pass, "over budget");
        assert!(
            !evaluate_sla(true, f64::NAN).pass,
            "NaN p95 fails closed (NaN < budget is false)"
        );
        // The verdict carries the measurement it judged — never a rebased one.
        assert_eq!(evaluate_sla(true, 12.5).p95_ms, 12.5);
    }

    /// /proc parsers are exact on representative lines (spaces in comm,
    /// multi-line status bodies) and fail-honest on malformed input.
    #[test]
    fn proc_parsers_exact_and_honest() {
        // 10 filler fields (4..13) between state and utime; utime=250,
        // stime=350 clock ticks → 6.0 CPU-seconds at USER_HZ=100.
        let stat = "1234 (discovery_matrix) R 1 2 3 4 5 6 7 8 9 10 250 350 0 0 20 0";
        assert_eq!(parse_proc_cpu_seconds(stat), Some(6.0));
        // Comm with spaces still parses (skip past the LAST ')').
        let spaced = "99 (my bench) R 1 2 3 4 5 6 7 8 9 10 100 100 0 0";
        assert_eq!(parse_proc_cpu_seconds(spaced), Some(2.0));
        assert_eq!(parse_proc_cpu_seconds("garbage"), None);
        assert_eq!(parse_proc_cpu_seconds("1 (short) R 1 2"), None);

        let status = "Name:\tdiscovery_matrix\nVmPeak:\t 2048 kB\nVmHWM:\t  1536 kB\nThreads:\t8\n";
        assert_eq!(parse_vm_hwm_mib(status), Some(1.5));
        assert_eq!(parse_vm_hwm_mib("Name:\tx\n"), None);
        assert_eq!(parse_vm_hwm_mib("VmHWM:\tnot-a-number kB\n"), None);
    }

    /// Throughput accounting is additive and its derived rates match hand
    /// math; no wall time ⇒ no invented rate (R8).
    #[test]
    fn throughput_rates_honest() {
        let audit = PassAudit {
            routes: 10,
            quotes: 128,
            ..PassAudit::default()
        };
        let mut thr = Throughput::default();
        thr.record(66, &audit, 2_000_000_000); // 2 s per pass
        thr.record(66, &audit, 4_000_000_000);
        assert_eq!(thr.passes, 2);
        assert_eq!(thr.routes, 20);
        assert_eq!(thr.quotes, 256);
        assert_eq!(thr.pair_scans, 132);
        assert_eq!(thr.elapsed_ns, 6_000_000_000);
        assert_eq!(Throughput::per_sec(20, 6_000_000_000), Some(20.0 / 6.0));
        assert_eq!(Throughput::per_sec(0, 6_000_000_000), Some(0.0));
        assert_eq!(Throughput::per_sec(20, 0), None, "no wall time ⇒ None");
    }

    /// The canonical bucket ladder is importable from the workload module —
    /// the amount_buckets axis cannot drift from the kernel's own table.
    #[test]
    fn canonical_bucket_ladder_available() {
        assert!(AMOUNT_BUCKETS_CANONICAL.contains(&BASE_BUCKETS));
    }
}

// wdac-probe rev-2 (content-hash refresh — ARBX-0009 playbook)
