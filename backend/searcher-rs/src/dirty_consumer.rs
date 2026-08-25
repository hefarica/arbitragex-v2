//! ARBX-QB-05-008 / XLS-QB-05b — consumer side of the dirty-pool signal
//! (writer side: `pool_sync_worker` + `dirty_signal.rs`).
//!
//! Per discovery tick the run_loop drains the Redis SET
//! `arbx:dirty_pools:<chain_id>` and replays each member through
//! [`DirtyPairEngine::on_pool_event`], so re-evaluation is seeded by the
//! pairs that actually moved instead of a global rebuild (workbook 08:
//! DirtySeed×Beam bound; 09 r9 "no duplicate rescans in same state
//! version").
//!
//! Transport: ONE `SMEMBERS` per tick, observe-phase — deliberately
//! NON-destructive. The writer's TTL is the set's leak backstop and the
//! engine's own `AlreadyDirty` outcome is the consumer's idempotency: a
//! member seen twice in one state version coalesces, it never re-enqueues.
//!
//! Pure logic: the Redis read sits behind [`DirtySource`] so this module
//! has zero transport dependencies and the drain semantics are testable
//! against an in-memory source. The production source is one
//! `SMEMBERS dirty_pools_key(chain_id)` call (see the worker wiring notes
//! in `WIRING.md`) — members arrive writer-normalized (lowercase), and a
//! member the consumer never registered surfaces as an `unknown_pool`
//! counter (fail-honest R8), never a silent drop.
//!
//! Re-evaluation scope is the CALLER's gate (ARBX-QB-05-009 knob,
//! default OFF — observe-only pattern of XLS-QB-03): with the knob off the
//! drain still marks pairs and seeds the bounded queue, and
//! [`DirtyDrain::dirty_seeds`] is the `Dirty_Seeds` telemetry metric
//! (09: a METRIC, not capacity).

use std::collections::HashMap;

use crate::dirty_pairs::{DirtyPairEngine, PoolEventOutcome};
use crate::dirty_signal;

/// One tick's replay of the dirty-pool signal through the engine.
///
/// The histogram is the R9 logging contract: per-item detail stays at
/// `debug!`, ONE summary line at `info!` per tick carries these counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrainStats {
    /// Members read from the signal set this tick.
    pub drained: usize,
    /// Writer marked a pool the consumer never registered (topology drift
    /// — pool sync knows a pool route discovery has not mapped yet).
    pub unknown_pool: usize,
    /// Registered pair degenerate for the current N (topology drift).
    pub invalid_pair: usize,
    /// Already dirty this state version — coalesced, not re-enqueued.
    pub already_dirty: usize,
    /// Newly dirty pairs seeded into the hot queue.
    pub seeded: usize,
    /// Seeds displaced by ring wrap while seeding (bounded queue, 09 r13).
    pub evicted: usize,
}

impl DrainStats {
    fn record(&mut self, outcome: PoolEventOutcome) {
        match outcome {
            PoolEventOutcome::UnknownPool => self.unknown_pool += 1,
            PoolEventOutcome::InvalidPair => self.invalid_pair += 1,
            PoolEventOutcome::AlreadyDirty { .. } => self.already_dirty += 1,
            PoolEventOutcome::Seeded { evicted, .. } => {
                self.seeded += 1;
                if evicted.is_some() {
                    self.evicted += 1;
                }
            }
        }
    }

    /// True when the tick replayed nothing at all (empty signal set).
    pub fn is_empty(&self) -> bool {
        self.drained == 0
    }
}

/// Transport boundary: where dirty-pool addresses come from. The production
/// read performs one `SMEMBERS arbx:dirty_pools:<chain_id>` per tick; tests
/// use an in-memory source (a boundary double for the transport, not
/// fabricated pipeline data).
pub trait DirtySource {
    fn drain(&mut self, chain_id: u64) -> Vec<String>;
}

/// One-shot handoff source: the async transport read (SMEMBERS via the
/// worker's `ConnectionManager`) lands here, then the persistent drain
/// consumes it — the transport stays async-native while [`DirtyDrain`]
/// stays sync and testable. Members arrive ONCE; later drains see empty.
#[derive(Debug, Default)]
pub struct OnceSource {
    pending: Vec<String>,
}

impl OnceSource {
    pub fn new(members: Vec<String>) -> Self {
        Self { pending: members }
    }
}

impl DirtySource for OnceSource {
    fn drain(&mut self, chain_id: u64) -> Vec<String> {
        let _ = chain_id; // already chain-scoped by the key the caller read
        std::mem::take(&mut self.pending)
    }
}

/// Consumer-side composition: signal set → pool topology → dirty engine
/// → hot seeds. Owns the address→dense-pool-id map the writer's members
/// resolve against; the token pair `(i, j)` of each pool are
/// [`crate::pair_index::DenseIdBuilder`] ids assigned at boot.
pub struct DirtyDrain<S: DirtySource> {
    source: S,
    engine: DirtyPairEngine,
    /// Writer-normalized pool address → dense pool id (registration order).
    pool_by_addr: HashMap<String, usize>,
}

impl<S: DirtySource> DirtyDrain<S> {
    /// Universe of `n_tokens` dense ids, up to `n_pools` pools, bounded
    /// seed queue of `queue_capacity` (the `Dirty_Seeds` horizon).
    pub fn new(n_tokens: usize, n_pools: usize, queue_capacity: usize, source: S) -> Self {
        Self {
            source,
            engine: DirtyPairEngine::new(n_tokens, n_pools, queue_capacity),
            pool_by_addr: HashMap::new(),
        }
    }

    /// Boot-time topology registration. Dense pool ids are assigned in
    /// registration order from the pool universe; `Err` propagates the
    /// exact rejection (degenerate pair for the current N) — a pool that
    /// fails registration is NEVER silently mapped.
    pub fn register_pool(&mut self, pool_addr: &str, i: usize, j: usize) -> Result<(), String> {
        let pool_id = self.pool_by_addr.len();
        self.engine.register_pool(pool_id, i, j)?;
        self.pool_by_addr
            .insert(dirty_signal::normalize_member(pool_addr), pool_id);
        Ok(())
    }

    /// Drain one tick: replay every signaled pool through the engine.
    /// Members are re-normalized defensively — the writer already
    /// normalizes, but a mixed-case member still resolves instead of
    /// inflating `unknown_pool`.
    pub fn drain_tick(&mut self, chain_id: u64) -> DrainStats {
        let mut stats = DrainStats::default();
        for addr in self.source.drain(chain_id) {
            stats.drained += 1;
            match self
                .pool_by_addr
                .get(&dirty_signal::normalize_member(&addr))
            {
                None => stats.unknown_pool += 1,
                Some(&pool_id) => stats.record(self.engine.on_pool_event(pool_id)),
            }
        }
        stats
    }

    /// Swap the transport handoff (per-tick async read → [`OnceSource`])
    /// without losing engine state — dirty marks and the bounded seed queue
    /// belong to the state version, not to the transport.
    pub fn replace_source(&mut self, source: S) {
        self.source = source;
    }

    /// Advance the state version (per block/event window): dirty marks
    /// reset so the next version can re-mark the same pairs.
    pub fn begin_state_version(&mut self) {
        self.engine.begin_state_version();
    }

    /// Next hot seed (FIFO) — the re-evaluation scope. Callers gate this
    /// behind the re-eval knob (ARBX-QB-05-009); observe-only mode simply
    /// does not call it and the queue keeps its bounded ring semantics.
    pub fn pop_seed(&mut self) -> Option<usize> {
        self.engine.pop_seed()
    }

    /// `Dirty_Seeds` telemetry metric: current hot-queue length
    /// (workbook 09 — a metric, never a capacity).
    pub fn dirty_seeds(&self) -> usize {
        self.engine.dirty_len()
    }

    /// Pairs currently marked dirty in this state version.
    pub fn dirty_count(&self) -> usize {
        self.engine.dirty_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory transport double: scripted batches, one per drain call.
    struct ScriptedSource {
        batches: Vec<Vec<String>>,
        chain_seen: Vec<u64>,
    }

    impl ScriptedSource {
        fn new(batches: Vec<Vec<&str>>) -> Self {
            Self {
                batches: batches
                    .into_iter()
                    .map(|b| b.into_iter().map(String::from).collect())
                    .collect(),
                chain_seen: Vec::new(),
            }
        }
    }

    impl DirtySource for ScriptedSource {
        fn drain(&mut self, chain_id: u64) -> Vec<String> {
            self.chain_seen.push(chain_id);
            self.batches.pop().unwrap_or_default()
        }
    }

    /// 3 tokens (pairs 0..2), 2 pools, unbounded-enough queue of 4.
    fn fixture(batches: Vec<Vec<&str>>) -> DirtyDrain<ScriptedSource> {
        let mut d = DirtyDrain::new(3, 2, 4, ScriptedSource::new(batches));
        d.register_pool("0xAAAa", 0, 1)
            .expect("pair (0,1) valid for N=3");
        d.register_pool("0xBBBB", 1, 2)
            .expect("pair (1,2) valid for N=3");
        d
    }

    /// Reversed so `fixture(vec![batch1, batch2])` drains batch1 FIRST.
    fn batches(first: Vec<&str>) -> Vec<Vec<&str>> {
        vec![first]
    }

    #[test]
    fn empty_signal_drains_nothing() {
        let mut d = fixture(vec![]);
        let s = d.drain_tick(1);
        assert_eq!(
            s,
            DrainStats {
                drained: 0,
                unknown_pool: 0,
                invalid_pair: 0,
                already_dirty: 0,
                seeded: 0,
                evicted: 0
            }
        );
        assert!(s.is_empty());
        assert_eq!(d.dirty_seeds(), 0);
    }

    #[test]
    fn first_event_seeds() {
        let mut d = fixture(batches(vec!["0xaaaa"]));
        let s = d.drain_tick(1);
        assert_eq!(s.drained, 1);
        assert_eq!(s.seeded, 1);
        assert_eq!(s.unknown_pool, 0);
        assert_eq!(d.dirty_seeds(), 1);
    }

    #[test]
    fn unknown_member_is_counted_not_dropped() {
        let mut d = fixture(batches(vec!["0xdeadbeef"]));
        let s = d.drain_tick(1);
        assert_eq!(s.drained, 1);
        assert_eq!(s.unknown_pool, 1, "fail-honest: topology drift observed");
        assert_eq!(s.seeded, 0);
    }

    #[test]
    fn defensive_renormalization_resolves_mixed_case() {
        // Registered as "0xAAAA", signaled by the writer as lowercase —
        // already the contract, but a mixed-case member must resolve too.
        let mut d = fixture(batches(vec!["0xAaAa"]));
        let s = d.drain_tick(1);
        assert_eq!(s.seeded, 1);
        assert_eq!(s.unknown_pool, 0);
    }

    #[test]
    fn re_mark_within_version_coalesces() {
        let mut d = fixture(batches(vec!["0xaaaa", "0xaaaa"]));
        let s = d.drain_tick(1);
        assert_eq!(s.drained, 2);
        assert_eq!(s.seeded, 1, "first observation seeds");
        assert_eq!(s.already_dirty, 1, "second coalesces (09 r9)");
        assert_eq!(d.dirty_seeds(), 1, "queue never holds a duplicate");
    }

    #[test]
    fn distinct_pools_seed_their_pairs() {
        let mut d = fixture(batches(vec!["0xaaaa", "0xbbbb"]));
        let s = d.drain_tick(1);
        assert_eq!(s.seeded, 2);
        assert_eq!(d.dirty_count(), 2);
        assert_eq!(d.dirty_seeds(), 2);
    }

    #[test]
    fn version_bump_rearms_marking() {
        let mut d = DirtyDrain::new(3, 1, 4, ScriptedSource::new(vec![]));
        d.register_pool("0xAAAA", 0, 1).unwrap();
        // ScriptedSource pops from the END, so this yields one batch per
        // drain call, both containing the same pool address.
        d.source.batches = vec![vec!["0xaaaa".into()], vec!["0xaaaa".into()]];
        let first = d.drain_tick(1);
        assert_eq!(first.seeded, 1);
        assert_eq!(d.dirty_count(), 1);
        d.begin_state_version();
        assert_eq!(d.dirty_count(), 0, "clean slate after the bump");
        let second = d.drain_tick(1);
        assert_eq!(second.seeded, 1, "same pool re-seeds in the new version");
        assert_eq!(second.already_dirty, 0);
    }

    #[test]
    fn pop_seed_is_fifo() {
        let mut d = DirtyDrain::new(3, 2, 4, ScriptedSource::new(vec![]));
        d.register_pool("0xAAAA", 0, 1).unwrap();
        d.register_pool("0xBBBB", 1, 2).unwrap();
        d.source.batches = vec![vec!["0xaaaa".into(), "0xbbbb".into()]];
        let _ = d.drain_tick(1);
        let first = d.pop_seed().expect("seed queued");
        let second = d.pop_seed().expect("second seed queued");
        assert_eq!((first, second), (0, 2), "pair(0,1)=0 precedes pair(1,2)=2");
        assert!(d.pop_seed().is_none(), "queue drained");
        assert_eq!(d.dirty_seeds(), 0);
    }

    #[test]
    fn bounded_ring_eviction_is_counted() {
        // Queue capacity 1: the second distinct seed wraps the ring.
        let mut d = DirtyDrain::new(3, 2, 1, ScriptedSource::new(vec![]));
        d.register_pool("0xAAAA", 0, 1).unwrap();
        d.register_pool("0xBBBB", 1, 2).unwrap();
        d.source.batches = vec![vec!["0xaaaa".into(), "0xbbbb".into()]];
        let s = d.drain_tick(1);
        assert_eq!(s.seeded, 2);
        assert_eq!(s.evicted, 1, "oldest seed displaced (09 r13)");
        assert_eq!(d.dirty_seeds(), 1, "ring holds capacity");
    }

    #[test]
    fn chain_id_reaches_the_transport() {
        let mut d = fixture(batches(vec!["0xaaaa"]));
        d.drain_tick(11155111);
        assert_eq!(d.source.chain_seen, vec![11155111], "key is chain-scoped");
    }

    #[test]
    fn once_source_hands_off_once_then_empty() {
        let mut s = OnceSource::new(vec!["0xaaaa".into(), "0xbbbb".into()]);
        assert_eq!(s.drain(1).len(), 2, "first drain yields the members");
        assert!(s.drain(1).is_empty(), "second drain is empty");
    }

    #[test]
    fn replace_source_preserves_engine_state() {
        let mut d = DirtyDrain::new(3, 1, 4, OnceSource::new(vec!["0xaaaa".into()]));
        d.register_pool("0xAAAA", 0, 1).unwrap();
        let first = d.drain_tick(1);
        assert_eq!(first.seeded, 1);
        assert_eq!(d.dirty_count(), 1, "mark lives in the engine");
        // Next tick: new transport read, SAME drain — the mark coalesces.
        d.replace_source(OnceSource::new(vec!["0xaaaa".into()]));
        let second = d.drain_tick(1);
        assert_eq!(second.drained, 1);
        assert_eq!(second.already_dirty, 1, "state survived the handoff swap");
        assert_eq!(second.seeded, 0);
    }
}
