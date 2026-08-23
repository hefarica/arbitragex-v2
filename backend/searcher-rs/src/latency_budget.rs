//! Discovery latency budget — workbook QUOTEBASE-264 sheet `10_LATENCY`
//! (XLS-QB-07).
//!
//! The workbook's budget for **discovery/ranking PRE-simulation** (00_MANUAL
//! r13): remote RPC and simulation are explicitly OUT of this budget. Eight
//! canonical stages, each with a target and a `lat.*` telemetry key:
//!
//! | Stage (10_LATENCY r4–r11)        | Target | Key          |
//! |----------------------------------|-------:|--------------|
//! | Event decode + dirty marking     |  2 ms  | `lat.decode` |
//! | In-memory state update           |  3 ms  | `lat.state`  |
//! | Dirty-pair repricing             |  4 ms  | `lat.reprice`|
//! | Direct inefficiency              |  3 ms  | `lat.pair`   |
//! | Hot-seed expansion               |  7 ms  | `lat.expand` |
//! | Amount-aware refinement          |  5 ms  | `lat.refine` |
//! | Gas/risk/rank gates              |  3 ms  | `lat.gates`  |
//! | Queue/serialization              |  2 ms  | `lat.emit`   |
//!
//! `Target_Total_ms = 29` (r15) is the SUM of the row targets — derived by
//! [`target_total_ms`], never restated as a literal. The PASS gate compares
//! against `discovery_sla_ms` (01_CONFIG r20, knob — default 30 ms).
//!
//! Row 21: *"La cifra <30 ms NO puede garantizarse por Excel. Debe
//! demostrarse con benchmarks"* — this module is the INSTRUMENT (budget
//! table, recorder, p50/p95 and the PASS gate); the numbers become real
//! when the discovery hot path wires stage timings into it and benchmarks
//! run.
//!
//! Honesty rules: samples are caller-supplied microseconds (no clock inside —
//! deterministic, and the consumer chooses `Instant` vs test fixtures). Empty
//! = `None` (not computed); a stage that ran zero work in a cycle
//! contributes exactly `0` (R8 fail-honest: `None ≠ 0`). Percentiles are
//! nearest-rank (no interpolation — documented, exact integers).

/// The eight canonical stages of sheet 10_LATENCY r4–r11.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// r4 — Event decode + dirty marking. 2 ms. `lat.decode`.
    Decode,
    /// r5 — In-memory state update. 3 ms. `lat.state`.
    State,
    /// r6 — Dirty-pair repricing. 4 ms. `lat.reprice`.
    Reprice,
    /// r7 — Direct inefficiency. 3 ms. `lat.pair`.
    Pair,
    /// r8 — Hot-seed expansion. 7 ms. `lat.expand`.
    Expand,
    /// r9 — Amount-aware refinement. 5 ms. `lat.refine`.
    Refine,
    /// r10 — Gas/risk/rank gates. 3 ms. `lat.gates`.
    Gates,
    /// r11 — Queue/serialization. 2 ms. `lat.emit`.
    Emit,
}

impl Stage {
    /// Canonical enumeration order = sheet row order.
    pub const ALL: [Stage; 8] = [
        Stage::Decode,
        Stage::State,
        Stage::Reprice,
        Stage::Pair,
        Stage::Expand,
        Stage::Refine,
        Stage::Gates,
        Stage::Emit,
    ];

    /// Telemetry key (10_LATENCY "Telemetry key" column).
    pub fn key(self) -> &'static str {
        match self {
            Stage::Decode => "lat.decode",
            Stage::State => "lat.state",
            Stage::Reprice => "lat.reprice",
            Stage::Pair => "lat.pair",
            Stage::Expand => "lat.expand",
            Stage::Refine => "lat.refine",
            Stage::Gates => "lat.gates",
            Stage::Emit => "lat.emit",
        }
    }

    /// Row target in milliseconds (10_LATENCY "Target_ms" column).
    pub fn target_ms(self) -> u64 {
        match self {
            Stage::Decode => 2,
            Stage::State => 3,
            Stage::Reprice => 4,
            Stage::Pair => 3,
            Stage::Expand => 7,
            Stage::Refine => 5,
            Stage::Gates => 3,
            Stage::Emit => 2,
        }
    }

    fn idx(self) -> usize {
        Stage::ALL
            .iter()
            .position(|&s| s == self)
            .expect("member of ALL")
    }
}

/// `Target_Total_ms` (10_LATENCY r15) — SUM of the row targets, derived.
pub fn target_total_ms() -> u64 {
    Stage::ALL.iter().map(|s| s.target_ms()).sum()
}

/// Nearest-rank percentile over `samples` (sorted copy, 1-based rank
/// `ceil(p/100 · n)`). `None` when there are no samples (not computed).
fn nearest_rank(samples: &[u64], p: u64) -> Option<u64> {
    if samples.is_empty() || !(1..=100).contains(&p) {
        return None;
    }
    let mut s = samples.to_vec();
    s.sort_unstable();
    let rank = (p as usize * s.len()).div_ceil(100).min(s.len());
    Some(s[rank - 1])
}

/// Cycle-scoped recorder: stage samples accumulate per discovery cycle;
/// `end_cycle` closes the cycle and records its total.
#[derive(Debug, Clone)]
pub struct CycleRecorder {
    /// Microseconds per stage in the CURRENT cycle (0 = ran no work).
    current: [u64; 8],
    in_cycle: bool,
}

/// Bounded log of stage samples + cycle totals (10_LATENCY Actual columns).
#[derive(Debug, Clone)]
pub struct LatencyLog {
    per_stage: Vec<Vec<u64>>,
    totals: Vec<u64>,
    /// Ring window: only the last `window` samples per stage / cycle totals
    /// are retained (bounded memory — the telemetry window is a recorder
    /// configuration, not a workbook constant).
    window: usize,
    cycle: CycleRecorder,
}

impl LatencyLog {
    pub fn with_window(window: usize) -> Self {
        Self {
            per_stage: (0..8).map(|_| Vec::new()).collect(),
            totals: Vec::new(),
            window: window.max(1),
            cycle: CycleRecorder {
                current: [0; 8],
                in_cycle: false,
            },
        }
    }

    /// Start a discovery cycle (records outside a cycle are an honest `Err`
    /// — timing without a cycle boundary has no total to attribute).
    pub fn begin_cycle(&mut self) {
        self.cycle.current = [0; 8];
        self.cycle.in_cycle = true;
    }

    /// Record `micros` of work for `stage` in the current cycle. A stage may
    /// record MULTIPLE times per cycle (e.g. one decode per event) — samples
    /// accumulate into the cycle and each lands in the stage's history.
    pub fn record(&mut self, stage: Stage, micros: u64) -> Result<(), String> {
        if !self.cycle.in_cycle {
            return Err(format!(
                "record({}) outside a cycle — begin_cycle() first",
                stage.key()
            ));
        }
        let i = stage.idx();
        self.cycle.current[i] = self.cycle.current[i].saturating_add(micros);
        let hist = &mut self.per_stage[i];
        hist.push(micros);
        if hist.len() > self.window {
            hist.remove(0);
        }
        Ok(())
    }

    /// Close the cycle; returns its TOTAL microseconds (sum of per-stage
    /// accumulations — stages with no work contribute exactly 0). Pushes the
    /// total into the bounded history.
    pub fn end_cycle(&mut self) -> Option<u64> {
        if !self.cycle.in_cycle {
            return None;
        }
        self.cycle.in_cycle = false;
        let total: u64 = self.cycle.current.iter().sum();
        self.totals.push(total);
        if self.totals.len() > self.window {
            self.totals.remove(0);
        }
        Some(total)
    }

    /// `Actual_p50` for a stage (µs). `None` = no samples yet (R8).
    pub fn stage_p50_us(&self, stage: Stage) -> Option<u64> {
        nearest_rank(&self.per_stage[stage.idx()], 50)
    }

    /// `Actual_p95` for a stage (µs). `None` = no samples yet (R8).
    pub fn stage_p95_us(&self, stage: Stage) -> Option<u64> {
        nearest_rank(&self.per_stage[stage.idx()], 95)
    }

    /// `Headroom_p95` for a stage (µs, signed): target − actual p95.
    /// Negative = over budget (observed honestly). `None` = not computed.
    pub fn stage_headroom_p95_us(&self, stage: Stage) -> Option<i64> {
        self.stage_p95_us(stage)
            .map(|p95| stage.target_ms() as i64 * 1000 - p95 as i64)
    }

    /// `Actual_p50_Total` (µs) over completed cycles. `None` if none.
    pub fn total_p50_us(&self) -> Option<u64> {
        nearest_rank(&self.totals, 50)
    }

    /// `Actual_p95_Total` (µs) over completed cycles. `None` if none.
    pub fn total_p95_us(&self) -> Option<u64> {
        nearest_rank(&self.totals, 95)
    }

    /// `PASS_p95` (r18): total p95 ≤ `sla_ms`. `None` if no completed
    /// cycles (not computed — never a fabricated PASS).
    pub fn pass_p95(&self, sla_ms: f64) -> Option<bool> {
        self.total_p95_us()
            .map(|p95| (p95 as f64) <= sla_ms * 1000.0)
    }

    /// Completed cycles retained in the window.
    pub fn cycle_count(&self) -> usize {
        self.totals.len()
    }

    /// Telemetry snapshot keyed by the workbook's `lat.*` keys — the shape
    /// the discovery hot path will publish. `None` fields = not computed.
    /// (The PASS gate is separate: [`LatencyLog::pass_p95`] with the SLA
    /// knob.)
    pub fn snapshot(&self) -> Vec<(&'static str, StageSnapshot)> {
        Stage::ALL
            .iter()
            .map(|&s| {
                (
                    s.key(),
                    StageSnapshot {
                        target_ms: s.target_ms(),
                        p50_us: self.stage_p50_us(s),
                        p95_us: self.stage_p95_us(s),
                        headroom_p95_us: self.stage_headroom_p95_us(s),
                    },
                )
            })
            .chain(std::iter::once((
                "lat.total",
                StageSnapshot {
                    target_ms: target_total_ms(),
                    p50_us: self.total_p50_us(),
                    p95_us: self.total_p95_us(),
                    headroom_p95_us: self
                        .total_p95_us()
                        .map(|p95| target_total_ms() as i64 * 1000 - p95 as i64),
                },
            )))
            .collect()
    }
}

/// One stage's budget line (Actual columns of 10_LATENCY).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageSnapshot {
    pub target_ms: u64,
    /// `Actual_p50` (µs) — `None` = no samples.
    pub p50_us: Option<u64>,
    /// `Actual_p95` (µs) — `None` = no samples.
    pub p95_us: Option<u64>,
    /// `Headroom_p95` (µs, signed; negative = over budget) — `None` if p95
    /// is not computed.
    pub headroom_p95_us: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 8-row stage table pinned exactly to 10_LATENCY r4–r11 (keys and
    /// targets), and Target_Total_ms = 29 DERIVED as the row sum (r15).
    #[test]
    fn stage_table_matches_workbook() {
        let expected: [(&str, u64); 8] = [
            ("lat.decode", 2),
            ("lat.state", 3),
            ("lat.reprice", 4),
            ("lat.pair", 3),
            ("lat.expand", 7),
            ("lat.refine", 5),
            ("lat.gates", 3),
            ("lat.emit", 2),
        ];
        for (&stage, &(key, target)) in Stage::ALL.iter().zip(expected.iter()) {
            assert_eq!(stage.key(), key);
            assert_eq!(stage.target_ms(), target);
        }
        assert_eq!(target_total_ms(), 29);
        assert_eq!(
            target_total_ms(),
            Stage::ALL.iter().map(|s| s.target_ms()).sum(),
            "total is always the derived sum"
        );
    }

    /// Nearest-rank percentile: exact integers, no interpolation. p50/p95 of
    /// 1..=100 → 50/95; single sample → itself; empty → None (not computed).
    #[test]
    fn nearest_rank_percentiles() {
        let samples: Vec<u64> = (1..=100).collect();
        assert_eq!(nearest_rank(&samples, 50), Some(50));
        assert_eq!(nearest_rank(&samples, 95), Some(95));
        assert_eq!(nearest_rank(&samples, 100), Some(100));
        assert_eq!(nearest_rank(&[42], 95), Some(42));
        assert_eq!(nearest_rank(&[], 95), None);
        // rank is clamped when p·n/100 rounds past n
        assert_eq!(nearest_rank(&[10, 20], 95), Some(20));
        // unsorted input is handled (sorts internally)
        assert_eq!(nearest_rank(&[30, 10, 20], 50), Some(20));
    }

    /// Full cycle: multiple records per stage accumulate; the cycle total is
    /// the stage-sum; p50/p95 and headroom land on both sides of the budget.
    #[test]
    fn cycle_accumulates_and_heads_into_budget() {
        let mut log = LatencyLog::with_window(64);
        log.begin_cycle();
        log.record(Stage::Decode, 1_500).expect("in cycle");
        log.record(Stage::Decode, 500)
            .expect("decode again — accumulates");
        log.record(Stage::Expand, 9_000).expect("in cycle"); // over its 7 ms
                                                             // Emit records NOTHING this cycle → contributes exactly 0.
        let total = log.end_cycle().expect("cycle closes");
        assert_eq!(total, 11_000, "2 ms decode + 9 ms expand + 0 emit");
        assert_eq!(log.total_p50_us(), Some(11_000), "one cycle → p50 = total");
        // Stage percentiles over individual samples [500, 1500]:
        // p50 rank = ceil(50·2/100) = 1 → 500; p95 rank = ceil(95·2/100) = 2 → 1500.
        assert_eq!(log.stage_p50_us(Stage::Decode), Some(500));
        assert_eq!(log.stage_p95_us(Stage::Decode), Some(1_500));
        assert_eq!(
            log.stage_p95_us(Stage::Emit),
            None,
            "no samples — None, not 0"
        );
        // Headroom signs: decode under (2 ms − 1.5 ms), expand over (7 − 9).
        assert_eq!(log.stage_headroom_p95_us(Stage::Decode), Some(500));
        assert_eq!(log.stage_headroom_p95_us(Stage::Expand), Some(-2_000));
    }

    /// PASS_p95 against the SLA knob semantics: ≤ passes, > fails, and no
    /// completed cycles → None (never a fabricated PASS).
    #[test]
    fn pass_p95_boundary_and_honest_absence() {
        let mut log = LatencyLog::with_window(8);
        assert_eq!(log.pass_p95(30.0), None, "no cycles → not computed");

        log.begin_cycle();
        log.record(Stage::Decode, 30_000).expect("in cycle"); // exactly 30 ms
        log.end_cycle().expect("closes");
        assert_eq!(log.pass_p95(30.0), Some(true), "exactly at SLA passes (≤)");

        log.begin_cycle();
        log.record(Stage::Decode, 30_001).expect("in cycle");
        log.end_cycle().expect("closes");
        assert_eq!(log.pass_p95(30.0), Some(false), "1 µs over fails");
    }

    /// Recording outside a cycle is an honest Err; end_cycle without a cycle
    /// is None.
    #[test]
    fn cycle_boundaries_enforced() {
        let mut log = LatencyLog::with_window(8);
        assert!(log.record(Stage::Decode, 1).is_err(), "no open cycle");
        assert_eq!(log.end_cycle(), None, "nothing to close");
        log.begin_cycle();
        assert!(log.record(Stage::Gates, 1).is_ok());
        assert_eq!(log.end_cycle(), Some(1));
        assert!(log.record(Stage::Gates, 1).is_err(), "cycle already closed");
    }

    /// Bounded window: only the last W stage samples / cycle totals are
    /// retained (bounded telemetry memory).
    #[test]
    fn window_is_bounded() {
        let mut log = LatencyLog::with_window(3);
        for i in 0..5u64 {
            log.begin_cycle();
            log.record(Stage::Decode, i).expect("in cycle");
            log.end_cycle().expect("closes");
        }
        assert_eq!(log.cycle_count(), 3, "only last 3 cycles retained");
        assert_eq!(log.total_p95_us(), Some(4), "over retained totals 2,3,4");
        assert_eq!(
            log.stage_p50_us(Stage::Decode),
            Some(3),
            "samples 2,3,4 → p50=3"
        );
    }

    /// The snapshot is keyed by the workbook's lat.* keys and carries the
    /// honest None for stages with no samples.
    #[test]
    fn snapshot_keys_and_absence() {
        let log = LatencyLog::with_window(8);
        let snap = log.snapshot();
        let keys: Vec<&str> = snap.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            vec![
                "lat.decode",
                "lat.state",
                "lat.reprice",
                "lat.pair",
                "lat.expand",
                "lat.refine",
                "lat.gates",
                "lat.emit",
                "lat.total",
            ]
        );
        for (_, s) in &snap {
            assert_eq!(s.p50_us, None, "empty log — nothing computed");
            assert_eq!(s.p95_us, None);
        }
        assert_eq!(
            snap[8].1.target_ms, 29,
            "lat.total carries the derived budget"
        );
    }
}
