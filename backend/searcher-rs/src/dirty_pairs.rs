//! Dirty-pair propagation — workbook QUOTEBASE-264 sheet
//! `09_RUNTIME_STRUCTURES` rows 9/10/13 (XLS-QB-05).
//!
//! Canonical pipeline steps 2–3 (00_MANUAL r18–19):
//! `event → Dirty object IDs` then `hot seed → adjacency/bitset → bounded
//! expansion`. This module is the exact middle of that path:
//!
//! * [`DirtyPairSet`] (09 r9) — bitset over the `C(N,2)` triangular buckets
//!   of [`crate::pair_index`], sized `ceil(C(N,2)/64)` words
//!   (04_INDEX_MATH r9), with a state version: **no duplicate rescans in the
//!   same state version**.
//! * [`PoolToPair`] (09 r10) — dense `Vec` fan-out so one pool event marks
//!   the EXACT pair dirty (parallel pools on one pair collapse to one dirty
//!   mark — 00_MANUAL r8).
//! * [`HotSeedQueue`] (09 r13) — bounded ring buffer of seed pair indices;
//!   push returns the evicted oldest seed when the ring wraps (the workbook
//!   offers "ring buffer OR binary heap if score ordered" — score-ordered
//!   eviction is a consumer choice, not invented here).
//! * [`DirtyPairEngine`] composes the three: `on_pool_event` fans a pool id
//!   out to its pair, marks it, and seeds the hot queue only on the FIRST
//!   mark per state version. Every outcome is honest
//!   ([`PoolEventOutcome`]) — unknown pools and degenerate pairs are
//!   reported, never silently dropped.
//!
//! Doctrine: N is DYNAMIC (`ceil(C(22,2)/64)×8 = 32` bytes is the workbook's
//! DEMO row, derived in tests only). `Dirty_Seeds` (01_CONFIG r8) is a
//! TELEMETRY metric (`dirty_queue.len`) — not a capacity; queue capacity is
//! caller-chosen. `Beam_K` bounds route EXPANSION, not this queue.
//!
//! Pure data structures, no I/O — the reserve-update hot path wires
//! `on_pool_event` as its follow-up consumer.

use crate::pair_index::{pair_count, pair_index, pair_unindex};

/// Bitset of dirty pair buckets over one state version (09 r9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyPairSet {
    words: Vec<u64>,
    n_tokens: usize,
    state_version: u64,
    dirty_count: usize,
}

impl DirtyPairSet {
    /// Bitset sized `ceil(C(n,2)/64)` words (04_INDEX_MATH r9). `n < 2`
    /// yields an empty (always-clean) set.
    pub fn new(n_tokens: usize) -> Self {
        let total = pair_count(n_tokens);
        Self {
            words: vec![0u64; total.div_ceil(64)],
            n_tokens,
            state_version: 0,
            dirty_count: 0,
        }
    }

    /// Set bucket `k` dirty. Returns `false` if it was ALREADY dirty in this
    /// state version (the caller must not re-enqueue — no duplicate rescans).
    /// Out-of-range `k` returns `false` unchanged (honest no-op).
    pub fn mark(&mut self, k: usize) -> bool {
        if k >= pair_count(self.n_tokens) {
            return false;
        }
        let (word, bit) = (k / 64, k % 64);
        if self.words[word] & (1 << bit) != 0 {
            false
        } else {
            self.words[word] |= 1 << bit;
            self.dirty_count += 1;
            true
        }
    }

    /// Test bucket `k` dirty. Out-of-range → `false`.
    pub fn is_dirty(&self, k: usize) -> bool {
        if k >= pair_count(self.n_tokens) {
            return false;
        }
        self.words[k / 64] & (1 << (k % 64)) != 0
    }

    /// Begin a new state version: every pair becomes eligible for re-marking
    /// (the rescan window resets; the seed QUEUE is consumer-drained and is
    /// deliberately not touched here).
    pub fn begin_state_version(&mut self) {
        self.words.iter_mut().for_each(|w| *w = 0);
        self.dirty_count = 0;
        self.state_version += 1;
    }

    pub fn state_version(&self) -> u64 {
        self.state_version
    }

    /// Pairs currently dirty (the `Dirty_Seeds` telemetry metric, 01_CONFIG r8).
    pub fn dirty_count(&self) -> usize {
        self.dirty_count
    }

    /// Bitset size in bytes — `ceil(C(N,2)/64) × 8`, derived from N (04 r9).
    pub fn bitset_bytes(&self) -> usize {
        self.words.len() * 8
    }

    /// Iterate dirty buckets ascending (drain-scan helper for repricing).
    pub fn dirty_pairs(&self) -> impl Iterator<Item = usize> + '_ {
        let n = pair_count(self.n_tokens);
        (0..n).filter(move |&k| self.is_dirty(k))
    }
}

/// Bounded FIFO ring of seed pair indices (09 r13). Push to a full ring
/// evicts the OLDEST seed and returns it — bounded by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotSeedQueue {
    ring: std::collections::VecDeque<usize>,
    capacity: usize,
}

impl HotSeedQueue {
    /// Capacity 0 rejects everything (honest: an unconfigured queue holds
    /// no seeds — never silently unbounded).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ring: std::collections::VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push seed `k`. Returns the evicted oldest seed when the ring wraps.
    /// A capacity-0 queue returns `Some(k)` (the seed itself is rejected).
    pub fn push(&mut self, k: usize) -> Option<usize> {
        if self.capacity == 0 {
            return Some(k);
        }
        let evicted = if self.ring.len() == self.capacity {
            self.ring.pop_front()
        } else {
            None
        };
        self.ring.push_back(k);
        evicted
    }

    /// Pop the oldest seed (expansion consumes FIFO).
    pub fn pop(&mut self) -> Option<usize> {
        self.ring.pop_front()
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Dense pool-id → PairIndex fan-out (09 r10). Pool KEYS resolve to dense
/// ids at ingress (hash at ingress, never in the price loop — 09 r5);
/// this map is rebuilt on topology changes only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoolToPair {
    pairs: Vec<Option<usize>>,
}

impl PoolToPair {
    /// Dense pool ids `0..n_pools`.
    pub fn with_pool_count(n_pools: usize) -> Self {
        Self {
            pairs: vec![None; n_pools],
        }
    }

    /// Bind pool `pool_id` to the unordered token pair `{i, j}` over a
    /// universe of `n_tokens` dense ids. `Err` carries the exact rejection
    /// (degenerate pair / id out of range) — fail-honest, no silent unbind.
    pub fn register(
        &mut self,
        pool_id: usize,
        i: usize,
        j: usize,
        n_tokens: usize,
    ) -> Result<(), String> {
        let k = pair_index(i, j, n_tokens)
            .ok_or_else(|| format!("pair ({i},{j}) degenerate or out of range for N={n_tokens}"))?;
        if pool_id >= self.pairs.len() {
            self.pairs.resize(pool_id + 1, None);
        }
        self.pairs[pool_id] = Some(k);
        Ok(())
    }

    /// One event marks the exact pair dirty (09 r10). `None` = pool not
    /// registered in this topology (honest absence).
    pub fn fan_out(&self, pool_id: usize) -> Option<usize> {
        self.pairs.get(pool_id).copied().flatten()
    }
}

/// Honest outcome of one pool event through the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolEventOutcome {
    /// Pool id absent from the topology map — nothing marked (fail-honest:
    /// the caller observes the gap instead of a silent drop).
    UnknownPool,
    /// Registered pair is degenerate for the current N (topology drift).
    InvalidPair,
    /// Already dirty in this state version — NOT re-enqueued
    /// ("no duplicate rescans in same state version", 09 r9).
    AlreadyDirty { pair: usize },
    /// Newly dirty — seeded into the hot queue; `evicted` carries the oldest
    /// seed displaced when the ring wrapped (bounded, 09 r13).
    Seeded { pair: usize, evicted: Option<usize> },
}

/// The composed hot-path middle: pool event → dirty pair → hot seed
/// (00_MANUAL pipeline steps 2–3).
#[derive(Debug, Clone)]
pub struct DirtyPairEngine {
    pool_to_pair: PoolToPair,
    dirty: DirtyPairSet,
    hot: HotSeedQueue,
    n_tokens: usize,
}

impl DirtyPairEngine {
    /// Universe of `n_tokens` dense ids, dense pool ids `0..n_pools`,
    /// bounded seed queue of `queue_capacity`.
    pub fn new(n_tokens: usize, n_pools: usize, queue_capacity: usize) -> Self {
        Self {
            pool_to_pair: PoolToPair::with_pool_count(n_pools),
            dirty: DirtyPairSet::new(n_tokens),
            hot: HotSeedQueue::with_capacity(queue_capacity),
            n_tokens,
        }
    }

    /// Rebind a pool (topology change). `Err` propagates the exact rejection.
    pub fn register_pool(&mut self, pool_id: usize, i: usize, j: usize) -> Result<(), String> {
        self.pool_to_pair.register(pool_id, i, j, self.n_tokens)
    }

    /// Process one pool event: fan out → mark → seed. Idempotent per state
    /// version; honest about unknown pools and topology drift.
    pub fn on_pool_event(&mut self, pool_id: usize) -> PoolEventOutcome {
        let Some(k) = self.pool_to_pair.fan_out(pool_id) else {
            return PoolEventOutcome::UnknownPool;
        };
        // Defensive: a registered bucket must decode back into this universe
        // (pair_index validated it at register time; drift is reported).
        if pair_unindex(k, self.n_tokens).is_none() {
            return PoolEventOutcome::InvalidPair;
        }
        if !self.dirty.mark(k) {
            return PoolEventOutcome::AlreadyDirty { pair: k };
        }
        let evicted = self.hot.push(k);
        PoolEventOutcome::Seeded { pair: k, evicted }
    }

    /// Advance the state version (per block/event window): dirty marks reset
    /// so the next version can re-mark the same pairs.
    pub fn begin_state_version(&mut self) {
        self.dirty.begin_state_version();
    }

    /// Pop the oldest seed for bounded expansion (step 3).
    pub fn pop_seed(&mut self) -> Option<usize> {
        self.hot.pop()
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty.dirty_count()
    }

    pub fn dirty_len(&self) -> usize {
        self.hot.len()
    }

    pub fn state_version(&self) -> u64 {
        self.dirty.state_version()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 04_INDEX_MATH r9, DERIVED from N (demo row N=22 → 32 bytes; the
    /// module itself never holds the constant).
    #[test]
    fn bitset_bytes_derive_from_n() {
        assert_eq!(DirtyPairSet::new(22).bitset_bytes(), 32); // ceil(231/64)=4 words
        assert_eq!(DirtyPairSet::new(2).bitset_bytes(), 8); // C=1 → 1 word
        assert_eq!(DirtyPairSet::new(64).bitset_bytes(), 256); // C=2016 → 32 words
        assert_eq!(DirtyPairSet::new(0).bitset_bytes(), 0); // C=0 → no words
    }

    /// Mark/dedupe within one state version; re-mark allowed after the
    /// version advances (09 r9 "no duplicate rescans in same state version").
    #[test]
    fn mark_dedupes_within_version_only() {
        let mut s = DirtyPairSet::new(22);
        assert!(s.mark(73), "first mark is new");
        assert!(!s.mark(73), "second mark dedupes");
        assert_eq!(s.dirty_count(), 1);
        s.begin_state_version();
        assert_eq!(s.state_version(), 1);
        assert!(!s.is_dirty(73), "clean after version bump");
        assert!(s.mark(73), "re-markable in the new version");
    }

    /// Out-of-range buckets are honest no-ops (never a wrapped word write).
    #[test]
    fn out_of_range_bucket_is_noop() {
        let mut s = DirtyPairSet::new(5); // C=10
        assert!(!s.mark(10));
        assert!(!s.is_dirty(10));
        assert_eq!(s.dirty_count(), 0);
    }

    /// Ring eviction order: full ring pushes out the OLDEST seed (09 r13
    /// bounded queue).
    #[test]
    fn ring_evicts_oldest_when_full() {
        let mut q = HotSeedQueue::with_capacity(2);
        assert_eq!(q.push(1), None);
        assert_eq!(q.push(2), None);
        assert_eq!(q.push(3), Some(1), "oldest evicted on wrap");
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(3));
        assert_eq!(q.pop(), None);
        // Capacity 0 rejects the seed honestly (returns it back).
        let mut z = HotSeedQueue::with_capacity(0);
        assert_eq!(z.push(9), Some(9));
        assert!(z.is_empty());
    }

    /// PoolToPair fan-out: exact pair per pool; parallel pools on the SAME
    /// pair collapse (00_MANUAL r8); unknown pool is honest `None`.
    #[test]
    fn pool_to_pair_fan_out_exact() {
        let mut m = PoolToPair::with_pool_count(4);
        m.register(0, 3, 17, 22).expect("valid pair");
        m.register(1, 17, 3, 22).expect("same pair reversed");
        assert_eq!(m.fan_out(0), Some(73));
        assert_eq!(m.fan_out(1), Some(73), "order-independent");
        assert_eq!(m.fan_out(2), None, "unregistered pool");
        assert!(m.register(2, 5, 5, 22).is_err(), "i == j rejected");
        assert!(m.register(2, 0, 22, 22).is_err(), "j >= N rejected");
    }

    /// Engine happy path: event → newly seeded; duplicate event → AlreadyDirty
    /// with NO second queue entry; second pool on the same pair also
    /// AlreadyDirty (parallel pools → one dirty mark).
    #[test]
    fn engine_marks_and_dedupes() {
        let mut e = DirtyPairEngine::new(22, 3, 8);
        e.register_pool(0, 3, 17).expect("valid");
        e.register_pool(1, 17, 3)
            .expect("same pair, reversed order");
        e.register_pool(2, 0, 1).expect("valid");
        assert_eq!(
            e.on_pool_event(0),
            PoolEventOutcome::Seeded {
                pair: 73,
                evicted: None
            }
        );
        assert_eq!(
            e.on_pool_event(0),
            PoolEventOutcome::AlreadyDirty { pair: 73 }
        );
        assert_eq!(
            e.on_pool_event(1),
            PoolEventOutcome::AlreadyDirty { pair: 73 },
            "parallel pool collapses to one dirty mark"
        );
        assert_eq!(
            e.on_pool_event(2),
            PoolEventOutcome::Seeded {
                pair: 0,
                evicted: None
            }
        );
        assert_eq!(e.dirty_count(), 2);
        assert_eq!(e.dirty_len(), 2, "queue holds exactly the new seeds");
        assert_eq!(e.pop_seed(), Some(73), "FIFO");
        assert_eq!(e.pop_seed(), Some(0));
        assert_eq!(e.pop_seed(), None);
    }

    /// Unknown pool ids and version-scoped re-seeding.
    #[test]
    fn engine_unknown_pool_and_version_reset() {
        let mut e = DirtyPairEngine::new(22, 1, 4);
        assert_eq!(e.on_pool_event(99), PoolEventOutcome::UnknownPool);
        e.register_pool(0, 0, 1).expect("valid");
        assert_eq!(
            e.on_pool_event(0),
            PoolEventOutcome::Seeded {
                pair: 0,
                evicted: None
            }
        );
        e.begin_state_version();
        assert_eq!(
            e.on_pool_event(0),
            PoolEventOutcome::Seeded {
                pair: 0,
                evicted: None
            },
            "re-seedable next version; prior seed still queued (consumer-drained, ring not full)"
        );
        assert_eq!(e.dirty_len(), 2, "same seed queued once per version");
    }

    /// Bounded engine: queue capacity 2 with 3 distinct pairs — the third
    /// seed evicts the first, and the eviction is OBSERVED (never silent).
    #[test]
    fn engine_ring_eviction_observed() {
        let mut e = DirtyPairEngine::new(22, 3, 2);
        e.register_pool(0, 0, 1).expect("valid");
        e.register_pool(1, 0, 2).expect("valid");
        e.register_pool(2, 0, 3).expect("valid");
        assert_eq!(
            e.on_pool_event(0),
            PoolEventOutcome::Seeded {
                pair: 0,
                evicted: None
            }
        );
        assert_eq!(
            e.on_pool_event(1),
            PoolEventOutcome::Seeded {
                pair: 1,
                evicted: None
            }
        );
        assert_eq!(
            e.on_pool_event(2),
            PoolEventOutcome::Seeded {
                pair: 2,
                evicted: Some(0)
            },
            "bounded ring wraps oldest out"
        );
        assert_eq!(e.dirty_count(), 3, "bitset keeps ALL dirty marks");
        assert_eq!(e.dirty_len(), 2, "queue stays bounded");
    }

    /// Dirty-scan helper iterates ascending buckets and inverts cleanly.
    #[test]
    fn dirty_scan_ascending_and_invertible() {
        let mut s = DirtyPairSet::new(6); // C=15
        for k in [14usize, 3, 7] {
            s.mark(k);
        }
        assert_eq!(s.dirty_pairs().collect::<Vec<_>>(), vec![3, 7, 14]);
        for k in s.dirty_pairs() {
            let (i, j) = pair_unindex(k, 6).expect("bucket in range");
            assert_eq!(pair_index(i, j, 6), Some(k), "roundtrip");
        }
    }

    /// QB-05-007 — replay de eventos REALES de mainnet. El fixture es una
    /// grabación read-only (§77) de la ventana Redis del VPS (~80s, 40
    /// muestras × 2s): `arbx:pool_reserves:1:*` / `arbx:v3_slot0:1:*` +
    /// `pool_index*` para el mapeo par→pools (provenance completa dentro
    /// del JSON). Valida la semántica de propagación contra churn real:
    /// la inmensa mayoría de los refreshes NO mueven reservas (solo blk/ts
    /// avanzan) y el motor debe suprimirlos exactamente como hace
    /// `v2_changed`/`v3_changed`. Census esperado de la ventana grabada
    /// (2026-08-24): 1576 eventos = 197 arrivals + 77 value-changes +
    /// 1302 unchanged refreshes; 72 eventos en 9 pools sin pool_index.
    mod qb05_007_replay {
        use super::super::*;
        use crate::dirty_signal::{v2_changed, v3_changed};
        use std::collections::{HashMap, HashSet};

        const FIXTURE: &str = include_str!("dirty_trace.fixture.json");

        fn fixture() -> serde_json::Value {
            serde_json::from_str(FIXTURE).expect("fixture parsea")
        }

        /// (pool_addr, [sym_a, sym_b]) — topología real del pool_index.
        fn pools() -> Vec<(String, [String; 2])> {
            fixture()["pools"]
                .as_array()
                .expect("pools array")
                .iter()
                .map(|p| {
                    (
                        p["addr"].as_str().expect("addr").to_string(),
                        [
                            p["pair"][0].as_str().expect("sym0").to_string(),
                            p["pair"][1].as_str().expect("sym1").to_string(),
                        ],
                    )
                })
                .collect()
        }

        /// Evento real decodificado del fixture.
        struct RealEvent {
            pool: String,
            is_v2: bool,
            prev: Option<[String; 2]>,
            next: [String; 2],
            value_changed: bool,
        }

        fn events() -> Vec<RealEvent> {
            fixture()["events"]
                .as_array()
                .expect("events array")
                .iter()
                .map(|e| RealEvent {
                    pool: e["pool"].as_str().expect("pool").to_string(),
                    is_v2: e["kind"].as_str().expect("kind") == "v2",
                    prev: e["prev"].as_array().map(|p| {
                        [
                            p[0].as_str().expect("prev0").to_string(),
                            p[1].as_str().expect("prev1").to_string(),
                        ]
                    }),
                    next: [
                        e["next"][0].as_str().expect("next0").to_string(),
                        e["next"][1].as_str().expect("next1").to_string(),
                    ],
                    value_changed: e["value_changed"].as_bool().expect("value_changed"),
                })
                .collect()
        }

        /// Anti-stale: el census declarado en la provenance del fixture debe
        /// ser EXACTAMENTE el recomputado desde los eventos (un hand-edit de
        /// cualquiera de los dos lados rompe aquí).
        #[test]
        fn census_matches_provenance() {
            let f = fixture();
            let ev = events();
            let arrivals = ev.iter().filter(|e| e.prev.is_none()).count();
            let value_changes = ev
                .iter()
                .filter(|e| e.prev.is_some() && e.value_changed)
                .count();
            let unchanged = ev
                .iter()
                .filter(|e| e.prev.is_some() && !e.value_changed)
                .count();
            let c = &f["provenance"]["census"];
            assert_eq!(c["events"].as_u64().unwrap() as usize, ev.len());
            assert_eq!(c["arrivals"].as_u64().unwrap() as usize, arrivals);
            assert_eq!(c["value_changes"].as_u64().unwrap() as usize, value_changes);
            assert_eq!(
                c["unchanged_refreshes"].as_u64().unwrap() as usize,
                unchanged
            );
            // El fenómeno central del replay: la mayoría de refreshes no mueve
            // valores (solo blk/ts del payload crudo avanzan).
            assert!(
                unchanged > value_changes,
                "census real esperado: unchanged ({unchanged}) > value_changes ({value_changes})"
            );
        }

        /// Los detectores de cambio acuerdan con el churn real: arrival y
        /// value-change señalan; refresh sin cambio NO señala (1302/1379
        /// suppressed — la semántica que evita re-evals inútiles).
        #[test]
        fn change_detectors_agree_with_real_churn() {
            for ev in events() {
                let RealEvent {
                    pool,
                    is_v2,
                    prev,
                    next,
                    value_changed,
                } = ev;
                let prev_ref = prev.as_ref().map(|p| (p[0].as_str(), p[1].as_str()));
                let changed = if is_v2 {
                    match prev_ref {
                        None => v2_changed(None, &next[0], &next[1]),
                        Some((p0, p1)) => v2_changed(Some((p0, p1)), &next[0], &next[1]),
                    }
                } else {
                    match prev_ref {
                        None => v3_changed(None, &next[0], &next[1]),
                        Some((p0, p1)) => v3_changed(Some((p0, p1)), &next[0], &next[1]),
                    }
                };
                let expect = prev.is_none() || value_changed;
                assert_eq!(
                    changed, expect,
                    "detector discrepa con el evento real {pool} (prev={prev:?})"
                );
                // Identical refresh (next contra next) nunca señala.
                let identical = if is_v2 {
                    v2_changed(Some((&next[0], &next[1])), &next[0], &next[1])
                } else {
                    v3_changed(Some((&next[0], &next[1])), &next[0], &next[1])
                };
                assert!(!identical, "refresh idéntico no puede señalar ({pool})");
            }
        }

        /// Replay completo por el motor: topología real (pool_index), eventos
        /// en orden temporal. Solo los pools que MOVIERON valores (o llegaron)
        /// alimentan `on_pool_event`; los unchanged refreshes jamás se
        /// encolan. Duplicados del mismo par → AlreadyDirty; dirty set ==
        /// pares únicos tocados (≪ universo — jamás rebuild global); seeds
        /// FIFO == dirty; eventos de pools sin pool_index quedan contados
        /// (UnknownPool honesto, ya cubierto unit-level arriba).
        #[test]
        fn full_replay_marks_only_moved_pairs() {
            let f = fixture();
            let tokens: Vec<String> = f["tokens"]
                .as_array()
                .expect("tokens")
                .iter()
                .map(|t| t.as_str().expect("tok").to_string())
                .collect();
            let tid: HashMap<&str, usize> = tokens
                .iter()
                .enumerate()
                .map(|(i, t)| (t.as_str(), i))
                .collect();
            let n = tokens.len();
            let universe_pairs = n * (n - 1) / 2;

            let reg = pools();
            let pool_id: HashMap<&str, usize> = reg
                .iter()
                .enumerate()
                .map(|(i, (addr, _))| (addr.as_str(), i))
                .collect();

            let mut engine = DirtyPairEngine::new(n, reg.len(), reg.len().max(1));
            for (id, (_, [a, b])) in reg.iter().enumerate() {
                engine
                    .register_pool(id, tid[a.as_str()], tid[b.as_str()])
                    .expect("par real del pool_index cabe en el universo");
            }

            let mut expected_pairs: HashSet<usize> = HashSet::new();
            let mut seed_order: Vec<usize> = Vec::new();
            let mut already_dirty_seen = 0usize;
            let mut unmapped_feed = 0usize;
            let mut unchanged_seen = 0usize;

            for ev in events() {
                let feeds = ev.prev.is_none() || ev.value_changed;
                if !feeds {
                    unchanged_seen += 1;
                    continue;
                }
                let Some(&pid) = pool_id.get(ev.pool.as_str()) else {
                    unmapped_feed += 1;
                    continue; // UnknownPool honesto: contado, no descartado.
                };
                match engine.on_pool_event(pid) {
                    PoolEventOutcome::Seeded { pair, .. } => {
                        if expected_pairs.insert(pair) {
                            seed_order.push(pair);
                        }
                    }
                    PoolEventOutcome::AlreadyDirty { .. } => already_dirty_seen += 1,
                    other => panic!("evento real no esperado: {other:?} ({})", ev.pool),
                }
            }

            assert!(
                unchanged_seen > 0,
                "la ventana real contiene unchanged refreshes"
            );
            assert!(
                unmapped_feed > 0,
                "la ventana real contiene pools sin pool_index"
            );
            assert!(
                already_dirty_seen > 0,
                "la ventana real contiene re-marks dentro de la versión"
            );
            // Dirty set == pares únicos tocados, calculado INDEPENDIENTENTE.
            assert_eq!(engine.dirty_count(), expected_pairs.len());
            // Jamás rebuild global: solo los pares que movieron.
            assert!(engine.dirty_count() < universe_pairs);
            assert!(
                engine.dirty_count() <= reg.len(),
                "pares dirty ≤ pools registrados"
            );
            // Seeds FIFO == exactamente el orden de primera marca.
            let drained: Vec<usize> = std::iter::from_fn(|| engine.pop_seed()).collect();
            assert_eq!(drained, seed_order);
            assert_eq!(engine.pop_seed(), None, "cola agotada");
        }

        /// Versión nueva re-habilita el mark: el mismo evento real re-alimenta
        /// → Seeded otra vez (la ventana de 2s coalesce, el bloque siguiente
        /// re-muerve el mismo par).
        #[test]
        fn state_version_reset_remarks_real_pair() {
            let f = fixture();
            let tokens: Vec<&str> = f["tokens"]
                .as_array()
                .expect("tokens")
                .iter()
                .map(|t| t.as_str().expect("tok"))
                .collect();
            let tid: HashMap<&str, usize> =
                tokens.iter().enumerate().map(|(i, t)| (*t, i)).collect();
            let reg = pools();
            let first_moved = events()
                .into_iter()
                .find(|ev| {
                    (ev.prev.is_none() || ev.value_changed)
                        && reg.iter().any(|(a, _)| a == &ev.pool)
                })
                .expect("la ventana real tiene al menos un evento alimentable");
            let pid = reg
                .iter()
                .position(|(a, _)| a == &first_moved.pool)
                .expect("registrado");
            let [a, b] = reg[pid].1.clone();
            let mut engine = DirtyPairEngine::new(tokens.len(), reg.len(), reg.len().max(1));
            engine
                .register_pool(pid, tid[a.as_str()], tid[b.as_str()])
                .expect("par real");
            assert!(matches!(
                engine.on_pool_event(pid),
                PoolEventOutcome::Seeded { .. }
            ));
            assert!(matches!(
                engine.on_pool_event(pid),
                PoolEventOutcome::AlreadyDirty { .. }
            ));
            engine.begin_state_version();
            assert!(
                matches!(engine.on_pool_event(pid), PoolEventOutcome::Seeded { .. }),
                "versión nueva re-marks el par que volvió a mover"
            );
        }
    }
}
