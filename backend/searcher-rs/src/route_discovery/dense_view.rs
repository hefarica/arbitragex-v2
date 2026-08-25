//! ARBX-0019 — dense O(1) out-edge view (CSR) for the token graph.
//!
//! Workbook `09_RUNTIME_STRUCTURES` (REQ-QB-004): the evaluation hot path
//! reads out-edges per token visit; a CSR adjacency (per-token offsets +
//! contiguous `u32` edge slots keyed by dense token id) turns each read
//! into a direct slice access — no hashing, no per-token `Vec` pointer
//! chase, and (unlike the legacy `edges.iter().position(ptr::eq)` index
//! recovery) no O(E) scan per visited edge.
//!
//! Doctrine: the `HashMap<Address, Vec<usize>>` adjacency stays the SOURCE
//! OF TRUTH until equivalence is demonstrated at the final gate — this view
//! is BUILT FROM the same edge list (never mutated independently) and only
//! accelerates reads.
//!
//! Pure std (no ethers) — probe-compilable standalone.

/// CSR out-edge index: `offsets[id]..offsets[id + 1]` slices `edge_slots`
/// into the out-edges of dense token `id` (ascending edge order, matching
/// the source-of-truth adjacency's per-token insertion order).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DenseAdjacency {
    offsets: Vec<u32>,
    edge_slots: Vec<u32>,
}

impl DenseAdjacency {
    /// Build from `sources[i]` = dense source-token id of edge `i`, over a
    /// universe of `n_tokens` ids. Construction-time `assert!` (one bad id
    /// is a programmer error in OUR id assignment — fail loud at build,
    /// never a silently corrupted index).
    pub fn from_edge_sources(n_tokens: usize, sources: &[u32]) -> Self {
        let mut counts = vec![0u32; n_tokens];
        for &s in sources {
            let s = s as usize;
            assert!(
                s < n_tokens,
                "dense source id {} out of range {}",
                s,
                n_tokens
            );
            counts[s] += 1;
        }
        // Exclusive prefix sum → offsets (len n + 1).
        let mut offsets = Vec::with_capacity(n_tokens + 1);
        let mut acc = 0u32;
        offsets.push(0u32);
        for c in counts {
            acc += c;
            offsets.push(acc);
        }
        // Stable bucketing: per-token write cursor preserves ascending edge
        // order within each token's slice.
        let mut edge_slots = vec![0u32; sources.len()];
        let mut cursor = offsets[..n_tokens].to_vec();
        for (i, &s) in sources.iter().enumerate() {
            let s = s as usize;
            edge_slots[cursor[s] as usize] = i as u32;
            cursor[s] += 1;
        }
        Self {
            offsets,
            edge_slots,
        }
    }

    /// Out-edge indices of dense token `id` (empty slice for a token with
    /// no out-edges — `id` must be `< token_count`).
    pub fn out_edge_indices(&self, id: usize) -> &[u32] {
        assert!(id + 1 < self.offsets.len(), "dense id {} out of range", id);
        let lo = self.offsets[id] as usize;
        let hi = self.offsets[id + 1] as usize;
        &self.edge_slots[lo..hi]
    }

    /// Number of tokens the index was built for.
    pub fn token_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Number of edges indexed (== total slots).
    pub fn edge_count(&self) -> usize {
        self.edge_slots.len()
    }
}

/// Fixed-size bitset over dense token ids `0..n` (ARBX-0020, workbook
/// `09_RUNTIME_STRUCTURES` col G "Fallback when large/sparse": bitset rows
/// while the quadratic footprint fits the budget, CSR when it does not).
///
/// `n` must fit `u32` (the dense id space is `u32`-keyed by construction).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenBitSet {
    words: Vec<u64>,
    len: u32,
}

impl TokenBitSet {
    /// Empty bitset over `n` ids (all clear).
    pub fn new(n: u32) -> Self {
        Self {
            words: vec![0u64; n.div_ceil(64) as usize],
            len: n,
        }
    }

    /// Number of ids the set covers.
    pub fn len(&self) -> u32 {
        self.len
    }

    /// `true` when no bit is set (standard set semantics over the universe).
    pub fn is_empty(&self) -> bool {
        self.count_ones() == 0
    }

    /// Set bit `id`; out-of-range ids are a programmer error (same
    /// fail-loud contract as `DenseAdjacency`).
    pub fn set(&mut self, id: u32) {
        assert!(id < self.len, "bitset id {} out of range {}", id, self.len);
        self.words[(id / 64) as usize] |= 1u64 << (id % 64);
    }

    /// Clear bit `id` (out-of-range: programmer error).
    pub fn clear(&mut self, id: u32) {
        assert!(id < self.len, "bitset id {} out of range {}", id, self.len);
        self.words[(id / 64) as usize] &= !(1u64 << (id % 64));
    }

    /// Membership test — THE hot-path op this structure exists for.
    pub fn contains(&self, id: u32) -> bool {
        if id >= self.len {
            return false;
        }
        self.words[(id / 64) as usize] & (1u64 << (id % 64)) != 0
    }

    /// Number of set bits.
    pub fn count_ones(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }
}

/// ARBX-0020 policy: build per-token destination bitset rows only while the
/// quadratic footprint (`T · ceil(T/64) · 8 B`) fits `MEMBERSHIP_BITSET_BUDGET`.
///
/// Derivation of the crossover (documented, not magic): with the 1 MiB
/// budget, `T·ceil(T/64)·8 ≤ 2^20` holds up to `T = 2880` (45 words/row,
/// 1,036,800 B); at `T = 2881` the 46th word pushes it to 1,060,208 B —
/// 18× the CSR itself (`(T+1+E)·4 B`, ≈ 57 KB at the workbook scale
/// T=2048 / E=12288). For large/sparse N the CSR IS the fallback, exactly
/// as the workbook's col-G prescribes.
pub const MEMBERSHIP_BITSET_BUDGET: usize = 1 << 20;

/// Bytes the per-token destination rows would cost for `n_tokens`.
pub fn membership_bitset_bytes(n_tokens: usize) -> usize {
    n_tokens
        .saturating_mul(n_tokens.div_ceil(64))
        .saturating_mul(8)
}

/// Whether the bitset membership view fits the budget for `n_tokens`.
pub fn membership_bitset_fits(n_tokens: usize) -> bool {
    membership_bitset_bytes(n_tokens) <= MEMBERSHIP_BITSET_BUDGET
}

/// Per-token destination-membership rows built from the same edge list as
/// [`DenseAdjacency`]: `rows[source_id].contains(dest_id)` ⇔ some edge runs
/// `source → dest`. `None` when the budget policy rejects N (the caller
/// keeps the CSR path — membership falls back to scanning the out-edges).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MembershipRows {
    rows: Vec<TokenBitSet>,
}

impl MembershipRows {
    /// Build rows over dense ids from `(sources[i], dests[i])` edge pairs.
    /// Returns `None` when `membership_bitset_fits(n_tokens)` is false.
    pub fn build(n_tokens: usize, sources: &[u32], dests: &[u32]) -> Option<Self> {
        if !membership_bitset_fits(n_tokens) {
            return None;
        }
        let mut rows = Vec::with_capacity(n_tokens);
        for _ in 0..n_tokens {
            rows.push(TokenBitSet::new(n_tokens as u32));
        }
        for (&s, &d) in sources.iter().zip(dests) {
            assert!(
                (s as usize) < n_tokens && (d as usize) < n_tokens,
                "dense id out of range {}",
                n_tokens
            );
            rows[s as usize].set(d);
        }
        Some(Self { rows })
    }

    /// `true` iff at least one edge runs `from → to` (both dense ids).
    /// Out-of-range ids are simply non-members (no assert: callers probe
    /// tokens that may predate the view; absence is the honest answer).
    pub fn has_edge(&self, from: u32, to: u32) -> bool {
        self.rows
            .get(from as usize)
            .map(|r| r.contains(to))
            .unwrap_or(false)
    }

    /// Out-neighbors of `from` as a bitset reference (for bulk iteration).
    pub fn row(&self, from: u32) -> Option<&TokenBitSet> {
        self.rows.get(from as usize)
    }

    /// Number of rows (== token universe size).
    pub fn token_count(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn csr_prefix_sums_and_per_token_order() {
        // sources: edge0←t0, edge1←t1, edge2←t0, edge3←t2, edge4←t0
        let csr = DenseAdjacency::from_edge_sources(3, &[0, 1, 0, 2, 0]);
        assert_eq!(csr.token_count(), 3);
        assert_eq!(csr.edge_count(), 5);
        assert_eq!(csr.out_edge_indices(0), &[0, 2, 4], "ascending order kept");
        assert_eq!(csr.out_edge_indices(1), &[1]);
        assert_eq!(csr.out_edge_indices(2), &[3]);
    }

    #[test]
    fn csr_roundtrips_every_edge_exactly_once() {
        let sources = vec![3u32, 1, 3, 0, 1, 1, 2, 0, 3, 3];
        let csr = DenseAdjacency::from_edge_sources(4, &sources);
        let mut seen = Vec::new();
        for id in 0..csr.token_count() {
            seen.extend_from_slice(csr.out_edge_indices(id));
        }
        seen.sort_unstable();
        let expected: Vec<u32> = (0..sources.len() as u32).collect();
        assert_eq!(
            seen, expected,
            "bijection: every edge in exactly one bucket"
        );
    }

    #[test]
    fn csr_empty_and_dense_single_token() {
        let empty = DenseAdjacency::from_edge_sources(0, &[]);
        assert_eq!(empty.token_count(), 0);
        assert_eq!(empty.edge_count(), 0);

        let no_edges = DenseAdjacency::from_edge_sources(5, &[]);
        assert_eq!(no_edges.token_count(), 5);
        assert!(no_edges.out_edge_indices(4).is_empty());

        let one = DenseAdjacency::from_edge_sources(1, &[0, 0, 0]);
        assert_eq!(one.out_edge_indices(0), &[0, 1, 2]);
    }

    /// Differential vs a HashMap stand-in built the same way — the same
    /// equivalence the TokenGraph integration pins against the REAL
    /// `HashMap<Address, Vec<usize>>` adjacency (graph_builder tests).
    #[test]
    fn csr_matches_hashmap_standin() {
        let n = 32usize;
        // Deterministic deg-6-ish pattern (workbook Avg_Active_Degree=6).
        let mut sources = Vec::new();
        for e in 0..(n * 3) {
            sources.push(((e * 7 + e / 3) % n) as u32);
        }
        let mut hm: HashMap<u32, Vec<u32>> = HashMap::new();
        for (i, &s) in sources.iter().enumerate() {
            hm.entry(s).or_default().push(i as u32);
        }
        let csr = DenseAdjacency::from_edge_sources(n, &sources);
        for id in 0..n as u32 {
            let empty: Vec<u32> = Vec::new();
            let hm_view: &[u32] = hm.get(&id).map(|v| &v[..]).unwrap_or(&empty);
            assert_eq!(
                csr.out_edge_indices(id as usize),
                hm_view,
                "token {id}: dense view == HashMap view (order included)"
            );
        }
    }

    /// ARBX-0020: bitset membership vs the CSR iteration — same edge list,
    /// two structures, one truth.
    #[test]
    fn membership_rows_match_csr_iteration_exactly() {
        let n = 40usize;
        // Deterministic edges: e leaves (e*7 % n) and lands ((e*13 + 5) % n).
        let mut sources = Vec::new();
        let mut dests = Vec::new();
        for e in 0..(n * 2) {
            let s = (e * 7) % n;
            let d = (e * 13 + 5) % n;
            if s == d {
                continue; // no self-loops, mirroring synth_graph
            }
            sources.push(s as u32);
            dests.push(d as u32);
        }
        let csr = DenseAdjacency::from_edge_sources(n, &sources);
        let rows = MembershipRows::build(n, &sources, &dests).expect("n=40 fits the bitset budget");
        assert_eq!(rows.token_count(), n);
        for s in 0..n {
            for d in 0..n {
                let via_csr = csr
                    .out_edge_indices(s)
                    .iter()
                    .any(|&e| dests[e as usize] == d as u32);
                assert_eq!(
                    rows.has_edge(s as u32, d as u32),
                    via_csr,
                    "pair {s}->{d}: bitset membership == CSR scan"
                );
            }
        }
        // Out-of-range probes answer absent (no panic — callers may probe
        // tokens outside the view's universe).
        assert!(!rows.has_edge(n as u32, 0));
        assert!(!rows.has_edge(0, n as u32));
    }

    #[test]
    fn membership_budget_policy_flips_on_n() {
        // Below the crossover: rows build. Above: None (CSR is the fallback).
        let tiny = 1_000usize; // 1000·16·8 = 128,000 B — fits
        let big = 3_000usize; // 3000·47·8 = 1,128,000 B — exceeds 1 MiB
        assert!(
            membership_bitset_bytes(tiny) <= MEMBERSHIP_BITSET_BUDGET,
            "tiny N fits by construction"
        );
        assert!(
            membership_bitset_bytes(big) > MEMBERSHIP_BITSET_BUDGET,
            "N=3000 exceeds the 1 MiB budget (crossover T=2880→2881)"
        );
        assert!(membership_bitset_fits(tiny));
        assert!(!membership_bitset_fits(big));
        assert!(
            MembershipRows::build(big, &[], &[]).is_none(),
            "over-budget N → None → CSR fallback path"
        );
        // Zero edges but budget-fit N still yields empty rows (universe kept).
        let empty = MembershipRows::build(tiny, &[], &[]).expect("fits");
        assert_eq!(empty.token_count(), tiny);
        assert!(empty.row(0).expect("row exists").is_empty());
        // Exact crossover: 2880 fits (45 words), 2881 does not (46th word).
        assert!(membership_bitset_fits(2_880));
        assert!(!membership_bitset_fits(2_881));
    }

    #[test]
    fn token_bitset_boundaries_and_counts() {
        let mut bs = TokenBitSet::new(130); // spans 3 words (64+64+2)
        assert!(bs.is_empty());
        for id in [0u32, 1, 63, 64, 65, 127, 128, 129] {
            bs.set(id);
        }
        assert_eq!(bs.count_ones(), 8);
        for id in [0u32, 63, 64, 65, 129] {
            assert!(bs.contains(id));
        }
        assert!(!bs.contains(2));
        assert!(!bs.contains(130), "universe is 0..130");
        assert!(!bs.contains(u32::MAX));
        bs.clear(64);
        assert!(!bs.contains(64));
        assert_eq!(bs.count_ones(), 7);
        // Idempotent set / clearing a clear bit is a no-op.
        bs.set(0);
        bs.clear(63);
        bs.clear(63);
        assert_eq!(bs.count_ones(), 6);

        let zero = TokenBitSet::new(0);
        assert!(zero.is_empty());
        assert!(!zero.contains(0));
    }
}
