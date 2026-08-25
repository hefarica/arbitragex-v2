//! ARBX-QB-05-010 / XLS-QB-05c — `CycleIndex`: pair→cycles inverted index.
//!
//! World-layer W2 (registered in the XLS-QB-01 gap doc §4 from
//! `skills/arbitragex-ultra/world/graph-algorithms/` — the amms-rs
//! cycle-edge inverted-index pattern; NOT a workbook artifact, adopted
//! under the "best of both worlds" operator directive). Upgrade target of
//! the QB-05b scoped re-evaluation: a dirty PAIR learns which CYCLES it
//! belongs to, so re-evaluation walks `affected_cycles(dirty_pairs)` —
//! linear in the dirty set — instead of rescanning the cycle universe.
//!
//! Build once per cycle enumeration epoch (cycles are token-id sequences,
//! e.g. `[0, 1, 2]` = the loop 0→1→2→0). Every hop of a cycle — including
//! the closing wrap hop — indexes into the bucket of its unordered pair
//! via [`crate::pair_index::pair_index`]. Validation is fail-honest: a
//! degenerate cycle (fewer than 3 hops, repeated token, or a token outside
//! the universe) rejects the WHOLE build with the exact reason — a partial
//! index would silently under-report affected cycles.
//!
//! `slot` hop positions are cycle-order positions: cycle `[0, 1, 2]` has
//! hop 0 = (0,1), hop 1 = (1,2), hop 2 = (2,0).

use crate::pair_index::{pair_count, pair_index};

/// One indexed membership: cycle `cycle` contains the pair at hop `hop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleSlot {
    pub cycle: usize,
    pub hop: usize,
}

/// Inverted index over the cycle universe of one epoch.
#[derive(Debug, Clone)]
pub struct CycleIndex {
    n_tokens: usize,
    /// Token-id sequence per cycle (the re-evaluation payload).
    cycles: Vec<Vec<usize>>,
    /// Bucket per dense pair index → memberships (ascending pair order).
    slots: Vec<Vec<CycleSlot>>,
}

impl CycleIndex {
    /// Build from cycles given as token-id sequences. `Err` names the
    /// first offending cycle and the exact rejection reason.
    pub fn build(n_tokens: usize, cycles: Vec<Vec<usize>>) -> Result<Self, String> {
        for (cycle_id, cycle) in cycles.iter().enumerate() {
            if cycle.len() < 3 {
                return Err(format!(
                    "cycle {cycle_id}: {} hops < 3 minimum (degenerate loop)",
                    cycle.len()
                ));
            }
            for (hop, &t) in cycle.iter().enumerate() {
                if t >= n_tokens {
                    return Err(format!(
                        "cycle {cycle_id} hop {hop}: token {t} outside universe of {n_tokens}"
                    ));
                }
                if cycle[..hop].contains(&t) {
                    return Err(format!(
                        "cycle {cycle_id} hop {hop}: token {t} repeats (self-crossing loop)"
                    ));
                }
            }
        }
        let mut slots: Vec<Vec<CycleSlot>> =
            (0..pair_count(n_tokens)).map(|_| Vec::new()).collect();
        for (cycle_id, cycle) in cycles.iter().enumerate() {
            let len = cycle.len();
            for hop in 0..len {
                let a = cycle[hop];
                let b = cycle[(hop + 1) % len];
                let (i, j) = if a < b { (a, b) } else { (b, a) };
                let k = pair_index(i, j, n_tokens).ok_or_else(|| {
                    format!("cycle {cycle_id} hop {hop}: pair ({i},{j}) outside range")
                })?;
                slots[k].push(CycleSlot {
                    cycle: cycle_id,
                    hop,
                });
            }
        }
        Ok(Self {
            n_tokens,
            cycles,
            slots,
        })
    }

    /// Token universe the index was built for.
    pub fn n_tokens(&self) -> usize {
        self.n_tokens
    }

    /// Number of indexed cycles.
    pub fn n_cycles(&self) -> usize {
        self.cycles.len()
    }

    /// Sum of cycle lengths — total memberships across all buckets.
    pub fn total_slots(&self) -> usize {
        self.cycles.iter().map(|c| c.len()).sum()
    }

    /// Pairs that participate in at least one cycle (non-empty buckets).
    pub fn n_pairs_touched(&self) -> usize {
        self.slots.iter().filter(|b| !b.is_empty()).count()
    }

    /// Memberships of one unordered pair (empty slice = pair in no cycle).
    pub fn cycles_touching(&self, pair: usize) -> &[CycleSlot] {
        self.slots.get(pair).map(|b| b.as_slice()).unwrap_or(&[])
    }

    /// Token-id sequence of one cycle (`None` if out of range).
    pub fn cycle_tokens(&self, cycle: usize) -> Option<&[usize]> {
        self.cycles.get(cycle).map(|c| c.as_slice())
    }

    /// The scoped re-evaluation set: cycles containing ANY of the dirty
    /// pairs, each ONCE, in ascending cycle order — deterministic so
    /// observe-only logs and live re-evaluation see the same scope.
    pub fn affected_cycles<I: IntoIterator<Item = usize>>(&self, dirty_pairs: I) -> Vec<usize> {
        let mut hit = vec![false; self.cycles.len()];
        for k in dirty_pairs {
            for slot in self.cycles_touching(k) {
                hit[slot.cycle] = true;
            }
        }
        (0..self.cycles.len()).filter(|&c| hit[c]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pair_index::pair_index;

    fn k(a: usize, b: usize, n: usize) -> usize {
        pair_index(a.min(b), a.max(b), n).unwrap()
    }

    #[test]
    fn triangular_cycle_indexes_three_pairs() {
        let ix = CycleIndex::build(3, vec![vec![0, 1, 2]]).unwrap();
        assert_eq!(ix.n_cycles(), 1);
        assert_eq!(ix.total_slots(), 3);
        assert_eq!(ix.n_pairs_touched(), 3, "C(3,2)=3 — every pair touched");
        for (a, b, hop) in [(0, 1, 0), (1, 2, 1), (2, 0, 2)] {
            let bucket = ix.cycles_touching(k(a, b, 3));
            assert_eq!(
                bucket,
                &[CycleSlot { cycle: 0, hop }],
                "pair ({a},{b}) at hop {hop}"
            );
        }
    }

    #[test]
    fn shared_edge_fans_in_two_cycles() {
        // [0,1,2] and [0,1,3] share the edge (0,1).
        let ix = CycleIndex::build(4, vec![vec![0, 1, 2], vec![0, 1, 3]]).unwrap();
        let shared = ix.cycles_touching(k(0, 1, 4));
        assert_eq!(shared.len(), 2);
        assert_eq!(shared[0].cycle, 0);
        assert_eq!(shared[1].cycle, 1);
        // The non-shared edges belong to exactly one cycle each.
        assert_eq!(ix.cycles_touching(k(1, 2, 4)).len(), 1);
        assert_eq!(ix.cycles_touching(k(1, 3, 4)).len(), 1);
    }

    #[test]
    fn empty_cycle_universe_is_valid() {
        let ix = CycleIndex::build(8, vec![]).unwrap();
        assert_eq!(ix.n_cycles(), 0);
        assert_eq!(ix.total_slots(), 0);
        assert_eq!(ix.n_pairs_touched(), 0);
        assert!(ix.cycles_touching(0).is_empty());
        assert!(ix.affected_cycles([0, 1, 2]).is_empty());
    }

    #[test]
    fn degenerate_cycles_reject_with_exact_reason() {
        let err = CycleIndex::build(4, vec![vec![0, 1]]).unwrap_err();
        assert!(
            err.contains("cycle 0") && err.contains("< 3 minimum"),
            "{}",
            err
        );
        let err = CycleIndex::build(4, vec![vec![0, 1, 1]]).unwrap_err();
        assert!(err.contains("repeats"), "{}", err);
        let err = CycleIndex::build(3, vec![vec![0, 1, 5]]).unwrap_err();
        assert!(err.contains("outside universe"), "{}", err);
    }

    #[test]
    fn one_bad_cycle_rejects_the_whole_build() {
        // Partial index would under-report affected cycles — all-or-nothing.
        let err = CycleIndex::build(4, vec![vec![0, 1, 2], vec![2, 2, 3]]).unwrap_err();
        assert!(err.starts_with("cycle 1"), "{}", err);
    }

    #[test]
    fn wrap_hop_is_indexed() {
        // 4-cycle [0,1,2,3]: wrap hop 3 = (3,0).
        let ix = CycleIndex::build(4, vec![vec![0, 1, 2, 3]]).unwrap();
        let bucket = ix.cycles_touching(k(3, 0, 4));
        assert_eq!(bucket, &[CycleSlot { cycle: 0, hop: 3 }]);
    }

    #[test]
    fn cycle_tokens_roundtrip() {
        let cycles = vec![vec![2, 0, 1], vec![0, 3, 1, 2]];
        let ix = CycleIndex::build(4, cycles).unwrap();
        assert_eq!(ix.cycle_tokens(0), Some(&[2usize, 0, 1][..]));
        assert_eq!(ix.cycle_tokens(1), Some(&[0usize, 3, 1, 2][..]));
        assert_eq!(ix.cycle_tokens(2), None);
    }

    #[test]
    fn affected_cycles_dedupes_and_sorts() {
        let ix = CycleIndex::build(4, vec![vec![0, 1, 2], vec![0, 1, 3]]).unwrap();
        // Both dirty pairs hit cycle 0; only (0,1) hits cycle 1 → dedup.
        let affected = ix.affected_cycles([k(0, 1, 4), k(1, 2, 4)]);
        assert_eq!(affected, vec![0, 1]);
        // A pair in no cycle affects nothing.
        assert!(ix.affected_cycles([k(2, 3, 4)]).is_empty());
    }

    #[test]
    fn slots_invariant_over_a_dense_universe() {
        // 6 tokens, all C(6,3)=20 triangles + 3 squares → total_slots
        // must equal the sum of lengths exactly.
        let mut cycles = vec![];
        for a in 0..6 {
            for b in (a + 1)..6 {
                for c in (b + 1)..6 {
                    cycles.push(vec![a, b, c]);
                }
            }
        }
        for s in 0..3 {
            cycles.push(vec![s, (s + 1) % 3 + 3, (s + 2) % 3 + 3, s + 3]);
        }
        let ix = CycleIndex::build(6, cycles).unwrap();
        let expected: usize = (0..ix.n_cycles())
            .map(|c| ix.cycle_tokens(c).unwrap().len())
            .sum();
        assert_eq!(ix.total_slots(), expected);
        assert_eq!(ix.n_cycles(), 23);
        assert_eq!(expected, 72, "20 triangles ×3 hops + 3 squares ×4 hops");
    }

    #[test]
    fn composes_with_dirty_set_for_scoped_reeval() {
        // End-to-end scope: mark pairs in a DirtyPairSet, read the dirty
        // iterator, ask the index which cycles to re-evaluate.
        use crate::dirty_pairs::DirtyPairSet;
        let ix = CycleIndex::build(4, vec![vec![0, 1, 2], vec![0, 1, 3]]).unwrap();
        let mut dirty = DirtyPairSet::new(4);
        assert!(dirty.mark(k(0, 1, 4)));
        assert!(dirty.mark(k(1, 2, 4)));
        let affected = ix.affected_cycles(dirty.dirty_pairs());
        assert_eq!(affected, vec![0, 1], "both cycles are in scope");
        let tokens: Vec<_> = affected
            .iter()
            .map(|&c| ix.cycle_tokens(c).unwrap().to_vec())
            .collect();
        assert_eq!(tokens, vec![vec![0, 1, 2], vec![0, 1, 3]]);
    }
}
