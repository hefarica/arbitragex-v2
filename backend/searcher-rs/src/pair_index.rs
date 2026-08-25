//! Dense pair indexing — workbook QUOTEBASE-264 sheet `04_INDEX_MATH`
//! (XLS-QB-04).
//!
//! Zero-based dense token ids `0..N` (assigned by the DenseIdBuilder step of
//! the workbook's 15-step contract — layer still pending, this module is the
//! exact index math it consumes). `PairIndex(i, j, N)` maps an unordered pair
//! `i ≠ j` to a bucket in `[0, C(N,2))` via the triangular form
//!
//! ```text
//! PairIndex = i(2N − i − 1)/2 + (j − i − 1)     with i = min, j = max
//! ```
//!
//! so a `Vec<PairBucket>` of size C(N,2) replaces a `HashMap<(Id,Id), _>`
//! lookup (workbook 09_RUNTIME_STRUCTURES row "PairBuckets": O(1) direct
//! index; the HashMap stays authoritative while ids are sparse).
//!
//! Doctrine (00_MANUAL): N is DYNAMIC — 22 tokens / 231 pairs / 462 directed
//! edges are the workbook's DEMO constants and appear only in tests as
//! fixtures, never in this logic.
//!
//! All arithmetic is checked integer math — an overflow returns `None`
//! (honest failure, never a wrapped index).

use std::collections::HashMap;

use ethers::types::Address;

/// `count` fresh empty buckets WITHOUT demanding `T: Clone` (`vec![Vec::new();
/// n]` would — the stored edge type is not cloneable in general).
fn empty_bucket_vec<T>(count: usize) -> Vec<Vec<T>> {
    (0..count).map(|_| Vec::new()).collect()
}

/// Canonical runtime token identity — workbook `03_CHAIN_REGISTRY` row 1:
/// `TokenKey = (CHAIN_ID, ADDRESS)`. The symbol is METADATA and never part
/// of the key (doctrine 00_MANUAL: the allowlist is dynamic; no algorithm
/// may depend on a symbol resolving to exactly one address).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TokenKey {
    pub chain_id: u64,
    pub address: Address,
}

/// Workbook `15_IMPLEMENTATION_CONTRACT` step 3 / `09_RUNTIME_STRUCTURES`
/// row "DenseIdMap": assigns zero-based dense ids `0..N` to [`TokenKey`]s
/// in deterministic FIRST-SEEN order. N is dynamic (00_MANUAL) — ids are
/// only meaningful within one builder epoch and never persisted.
///
/// Re-inserting an existing key is an idempotent upsert: the id is stable
/// and the `allowed` flag is refreshed (workbook 03 col `Allowed`).
#[derive(Clone, Debug)]
pub struct DenseIdBuilder {
    ids: HashMap<TokenKey, usize>,
    order: Vec<TokenKey>,
    allowed: Vec<bool>,
}

impl Default for DenseIdBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DenseIdBuilder {
    pub fn new() -> Self {
        Self {
            ids: HashMap::new(),
            order: Vec::new(),
            allowed: Vec::new(),
        }
    }

    /// Insert (or refresh) a token; returns its dense id.
    pub fn insert(&mut self, key: TokenKey, allowed: bool) -> usize {
        match self.ids.get(&key) {
            Some(&id) => {
                self.allowed[id] = allowed;
                id
            }
            None => {
                let id = self.order.len();
                self.ids.insert(key, id);
                self.order.push(key);
                self.allowed.push(allowed);
                id
            }
        }
    }

    /// Current universe size N.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Dense id of a known key — `None` if never inserted (honest, R8).
    pub fn id(&self, key: &TokenKey) -> Option<usize> {
        self.ids.get(key).copied()
    }

    /// Key that owns a dense id — `None` if out of range.
    pub fn key(&self, id: usize) -> Option<TokenKey> {
        self.order.get(id).copied()
    }

    /// Workbook 03 col `Allowed` for a dense id — `None` if out of range.
    pub fn allowed(&self, id: usize) -> Option<bool> {
        self.allowed.get(id).copied()
    }

    /// Update the `Allowed` flag; returns whether `id` was in range.
    pub fn set_allowed(&mut self, id: usize, allowed: bool) -> bool {
        match self.allowed.get_mut(id) {
            Some(slot) => {
                *slot = allowed;
                true
            }
            None => false,
        }
    }

    /// Dense pair bucket for two dense ids over the current N — plain
    /// [`pair_index`] (`None` on degenerate input).
    pub fn pair_bucket(&self, a: usize, b: usize) -> Option<usize> {
        pair_index(a, b, self.len())
    }

    /// Snapshot of `(TokenKey, id, allowed)` in dense-id order — the exact
    /// seed list a dense layer rebuilds from (09 col "Mutation frequency":
    /// on universe change).
    pub fn snapshot(&self) -> Vec<(TokenKey, usize, bool)> {
        self.order
            .iter()
            .zip(&self.allowed)
            .enumerate()
            .map(|(id, (k, a))| (*k, id, *a))
            .collect()
    }
}

/// Workbook `09_RUNTIME_STRUCTURES` row "PairBuckets": a dense `Vec` of
/// size C(N,2) giving O(1) pair→bucket access, replacing a
/// `HashMap<(Id, Id), _>` lookup on the hot path — while the HashMap path
/// stays authoritative until parity is demonstrated (XLS-QB-04
/// coexistence; the hot-path switch is ARBX-0019, gated on benchmark
/// evidence).
///
/// Parallel pools on the same unordered pair share ONE bucket; every pushed
/// edge is preserved (bijection edge↔bucket entry), so a dense rebuild can
/// never drop a pool.
pub struct PairBuckets<T> {
    n: usize,
    buckets: Vec<Vec<(usize, usize, T)>>,
}

impl<T> PairBuckets<T> {
    /// Allocates C(n,2) empty buckets; `n < 2` yields an empty (valid) layer.
    pub fn new(n: usize) -> Self {
        Self {
            n,
            buckets: empty_bucket_vec(pair_count(n)),
        }
    }

    /// Current universe size the layer was built for.
    pub fn n(&self) -> usize {
        self.n
    }

    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    /// Append one edge (e.g. a pool) to its pair's bucket. `false` (no-op)
    /// on degenerate ids: `a == b`, or either `>= n`.
    pub fn push(&mut self, a: usize, b: usize, value: T) -> bool {
        match pair_index(a, b, self.n) {
            Some(k) => {
                self.buckets[k].push((a, b, value));
                true
            }
            None => false,
        }
    }

    /// Entries sharing the unordered pair {a, b} — order-independent lookup.
    pub fn bucket(&self, a: usize, b: usize) -> Option<&[(usize, usize, T)]> {
        pair_index(a, b, self.n).map(|k| self.buckets[k].as_slice())
    }

    /// Mutable access to one pair's entries (reserve updates in place).
    pub fn bucket_mut(&mut self, a: usize, b: usize) -> Option<&mut Vec<(usize, usize, T)>> {
        match pair_index(a, b, self.n) {
            Some(k) => self.buckets.get_mut(k),
            None => None,
        }
    }

    /// Every non-empty bucket with its pair — dense-index (lexicographic
    /// pair) order.
    pub fn iter_pairs(&self) -> impl Iterator<Item = (usize, usize, &[(usize, usize, T)])> {
        let n = self.n;
        self.buckets
            .iter()
            .enumerate()
            .filter_map(move |(k, entries)| {
                if entries.is_empty() {
                    return None;
                }
                pair_unindex(k, n).map(|(a, b)| (a, b, entries.as_slice()))
            })
    }

    /// Total edges stored across all buckets.
    pub fn total_entries(&self) -> usize {
        self.buckets.iter().map(Vec::len).sum()
    }

    /// Full rebuild on universe change (09: "Mutation frequency: on
    /// universe change"): re-buckets every entry under the new N. Entries
    /// degenerate under the new N (ids `>= new_n`) are dropped and the
    /// count is RETURNED — visible, never silent (R8).
    pub fn rebuild(&mut self, new_n: usize) -> usize {
        let old = std::mem::replace(&mut self.buckets, empty_bucket_vec(pair_count(new_n)));
        self.n = new_n;
        let mut dropped = 0usize;
        for entries in old {
            for (a, b, v) in entries {
                if !self.push(a, b, v) {
                    dropped += 1;
                }
            }
        }
        dropped
    }
}

/// C(n, 2) — number of unordered pairs over `n` dense ids. `0` for `n < 2`.
pub fn pair_count(n: usize) -> usize {
    n.checked_mul(n.saturating_sub(1)).map_or(0, |v| v / 2)
}

/// Normalize an unordered pair to `(min, max)`.
pub fn normalize_pair(i: usize, j: usize) -> (usize, usize) {
    if i <= j {
        (i, j)
    } else {
        (j, i)
    }
}

/// Row offset of dense id `a` — index of the first pair `(a, a+1)`:
/// `a(2N − a − 1)/2`. Rows shrink by one each step (row `a` holds the
/// `N − a − 1` pairs `(a, a+1)..(a, N-1)`), which is exactly the
/// lexicographic enumeration order of `(i, j)`.
fn row_offset(a: usize, n: usize) -> Option<usize> {
    // (2N − a − 1) is ≥ 1 whenever a ≤ N−2 (the only range callers use); the
    // product is always even (a odd ⇒ factor even), so the /2 is exact.
    let factor = n.checked_add(n)?.checked_sub(a)?.checked_sub(1)?;
    a.checked_mul(factor).map(|v| v / 2)
}

/// Triangular O(1) index of the unordered pair `{i, j}` over `n` dense ids —
/// workbook `04_INDEX_MATH` (worked example: `(3, 17, N=22) → 73`).
///
/// `None` on degenerate input: `n < 2`, `i == j`, or either id `≥ n`.
/// Order-independent: `pair_index(i, j, ..) == pair_index(j, i, ..)`.
pub fn pair_index(i: usize, j: usize, n: usize) -> Option<usize> {
    if n < 2 || i == j || i >= n || j >= n {
        return None;
    }
    let (a, b) = normalize_pair(i, j);
    row_offset(a, n)?.checked_add(b - a - 1)
}

/// Inverse of [`pair_index`]: bucket `k` → `(i, j)` with `i < j`.
/// Binary search over row offsets — exact integer math, `O(log n)`.
///
/// `None` when `k >= pair_count(n)` (or `n < 2`).
pub fn pair_unindex(k: usize, n: usize) -> Option<(usize, usize)> {
    if n < 2 || k >= pair_count(n) {
        return None;
    }
    // Largest row a ≤ n−2 whose offset ≤ k (rows are strictly increasing).
    let (mut lo, mut hi) = (0usize, n - 2);
    while lo < hi {
        let mid = lo + (hi - lo + 1).div_ceil(2);
        if row_offset(mid, n)? <= k {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let a = lo;
    Some((a, a + 1 + (k - row_offset(a, n)?)))
}

/// `N!/[h(N−h)!]` — unique directed cycles up to rotation on a COMPLETE
/// token graph (workbook `04_INDEX_MATH` rows 19–24, formula column).
///
/// THEORETICAL CEILING for prefilter/beam sizing only (the sheet itself marks
/// hops ≥4 "NO" for runtime enumeration; global enumeration explodes — dirty
/// seeds + beam are the runtime path). `None` outside the canonical
/// `h ∈ 2..=7` or when `n < h`.
pub fn unique_cycles_ceiling(n: usize, h: u8) -> Option<u128> {
    if !(2..=7).contains(&h) || n < h as usize {
        return None;
    }
    let mut acc: u128 = 1;
    for k in 0..h as u128 {
        acc = acc.checked_mul(n as u128 - k)?;
    }
    acc.checked_div(h as u128)
}

// ---- ARBX-0028: CROSS-CHAIN GENERALIZATION (workbook 04 rows 27–31) ----
//
// The cross-chain rows generalize the single-chain combinatorics to a
// per-chain vector `chain_sizes` — every quantity is DERIVED from the input
// (no constants; the demo N=22 lives only in tests). All arithmetic is
// `checked` (u128 saturation → `None`, the honest overflow answer).

/// `Σ_c C(N_c, 2)` — within-chain unordered pairs (workbook r28). `None` on
/// overflow. Chains with fewer than 2 tokens contribute 0; the empty slice
/// is a valid degenerate universe (→ 0).
pub fn within_chain_pairs(chain_sizes: &[usize]) -> Option<u128> {
    let mut acc: u128 = 0;
    for &n in chain_sizes {
        if n < 2 {
            continue; // C(0,2) = C(1,2) = 0 — no underflow, no fabrication
        }
        let n = n as u128;
        acc = acc.checked_add(n.checked_mul(n - 1)?.checked_div(2)?)?;
    }
    Some(acc)
}

/// `Σ_c N_c(N_c − 1)` — within-chain directed token edges before pool
/// multiplicity (workbook r29). `None` on overflow; N_c < 2 contributes 0.
pub fn within_chain_directed_edges(chain_sizes: &[usize]) -> Option<u128> {
    let mut acc: u128 = 0;
    for &n in chain_sizes {
        if n < 2 {
            continue;
        }
        let n = n as u128;
        acc = acc.checked_add(n.checked_mul(n - 1)?)?;
    }
    Some(acc)
}

/// ARBX-0028 cross-chain cycle ceiling (workbook rows 19–24 generalized by
/// the r28/r29 Σ_c convention).
///
/// `within_chain` = `Σ_c unique_cycles_ceiling(N_c, h)` — the ceiling for
/// cycles that never leave their chain. This is the ONLY part derivable from
/// token counts alone.
///
/// `bridge_gated = true` encodes workbook r30 verbatim: bridge/domain edges
/// are ONLY the supported token-domain copies and bridges — NEVER assume
/// complete cross-chain connectivity. A bridge-using cycle ceiling requires
/// the supported-bridge topology (which chains, which token copies), and
/// this function deliberately refuses to fabricate one from `N_c` alone:
/// the struct makes "cross-chain is topology-gated" a TYPE-LEVEL fact a
/// caller cannot accidentally blur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossChainCeiling {
    /// `Σ_c N_c!/[h(N_c−h)!]` — provable from the token counts alone.
    pub within_chain: u128,
    /// Always `true`: bridge-using cycles are gated by the SUPPORTED bridge
    /// set (r30) and have no N-only ceiling. A future bridge registry must
    /// supply the topology to extend this bound — nothing else may.
    pub bridge_gated: bool,
}

/// Per-chain cycle ceilings summed (r19–24 × the r28/r29 Σ_c convention).
/// A chain with `N_c < h` has exactly ZERO h-cycles — it contributes 0
/// (the R8 "Some(0) = computed and exactly zero", never a silent undercount).
/// `None` only for misuse or overflow: `h` outside the canonical `2..=7`, or
/// a u128 overflow — the honest refusal, never a partial sum that
/// under-reports.
pub fn cross_chain_cycles_ceiling(chain_sizes: &[usize], h: u8) -> Option<CrossChainCeiling> {
    if !(2..=7).contains(&h) {
        return None;
    }
    let mut acc: u128 = 0;
    for &n in chain_sizes {
        let part = if n < h as usize {
            0
        } else {
            unique_cycles_ceiling(n, h)?
        };
        acc = acc.checked_add(part)?;
    }
    Some(CrossChainCeiling {
        within_chain: acc,
        bridge_gated: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Workbook 04_INDEX_MATH worked example row 15: (i=3, j=17, N=22) → 73,
    /// order-independent in both argument orders.
    #[test]
    fn worked_example_workbook() {
        assert_eq!(pair_index(3, 17, 22), Some(73));
        assert_eq!(pair_index(17, 3, 22), Some(73));
    }

    /// Injectivity + exact range for every N up to 24 (covers the workbook's
    /// demo N=22): all C(N,2) pairs map to distinct buckets in [0, C(N,2)).
    #[test]
    fn injective_and_full_range_n_2_to_24() {
        for n in 2..=24usize {
            let total = pair_count(n);
            let mut seen = std::collections::HashSet::with_capacity(total);
            let (mut min_k, mut max_k) = (usize::MAX, 0usize);
            for i in 0..n {
                for j in (i + 1)..n {
                    let k = pair_index(i, j, n).expect("valid pair must index");
                    // Positional args: edition-agnostic assert message (implicit
                    // format captures are 2021-only; probes compile this file
                    // under older editions and `non_fmt_panics` fires there).
                    assert!(seen.insert(k), "collision at n={} ({},{}) → {}", n, i, j, k);
                    min_k = min_k.min(k);
                    max_k = max_k.max(k);
                }
            }
            assert_eq!(seen.len(), total, "n={n} pair count");
            assert_eq!((min_k, max_k), (0, total - 1), "n={n} range");
        }
    }

    /// The triangular form IS the lexicographic enumeration order: the k-th
    /// pair in ascending (i, j) order gets bucket k.
    #[test]
    fn lexicographic_order_pinned() {
        let n = 9usize;
        let mut k = 0usize;
        for i in 0..n {
            for j in (i + 1)..n {
                assert_eq!(pair_index(i, j, n), Some(k), "lex order at ({i},{j})");
                k += 1;
            }
        }
    }

    /// `pair_unindex` is the exact inverse of `pair_index` for every pair
    /// (round-trip both ways, N up to 24).
    #[test]
    fn decode_roundtrip_exhaustive() {
        for n in 2..=24usize {
            for i in 0..n {
                for j in (i + 1)..n {
                    let k = pair_index(i, j, n).expect("valid pair");
                    assert_eq!(pair_unindex(k, n), Some((i, j)), "n={n} k={k}");
                }
            }
            assert_eq!(pair_unindex(pair_count(n), n), None, "k == C(n,2) rejected");
        }
    }

    /// Degenerate inputs are `None` — honest rejection, never a wrapped index.
    #[test]
    fn rejects_degenerate_input() {
        assert_eq!(pair_index(0, 0, 5), None, "i == j");
        assert_eq!(pair_index(5, 0, 5), None, "i >= n");
        assert_eq!(pair_index(0, 5, 5), None, "j >= n");
        assert_eq!(pair_index(0, 1, 1), None, "n < 2");
        assert_eq!(pair_index(0, 1, 0), None, "n == 0");
        assert_eq!(pair_unindex(0, 1), None, "unindex n < 2");
    }

    /// Doctrine check: the workbook's DEMO constants (N=22 → 231 pairs / 462
    /// directed edges) are DERIVED from N here — they live in tests as
    /// fixtures, never in the module's logic.
    #[test]
    fn workbook_demo_constants_derive_from_n() {
        assert_eq!(pair_count(22), 231);
        assert_eq!(22usize * 21, 462); // directed edges N(N−1)
        assert_eq!(pair_index(0, 1, 22), Some(0));
        assert_eq!(pair_index(21, 20, 22), Some(230)); // last bucket = C(22,2)−1
    }

    /// Workbook 04_INDEX_MATH rows 19–24: the complete-graph cycle ceiling
    /// N!/[h(N−h)!] for N=22 across every canonical hop count.
    #[test]
    fn cycles_ceiling_matches_workbook_n22() {
        let expected: [(u8, u128); 6] = [
            (2, 231),
            (3, 3_080),
            (4, 43_890),
            (5, 632_016),
            (6, 8_953_560),
            (7, 122_791_680),
        ];
        for (h, want) in expected {
            assert_eq!(unique_cycles_ceiling(22, h), Some(want), "h={h}");
        }
        // Outside the canonical hop range / n < h → None (honest ceiling).
        assert_eq!(unique_cycles_ceiling(22, 1), None);
        assert_eq!(unique_cycles_ceiling(22, 8), None);
        assert_eq!(unique_cycles_ceiling(3, 4), None);
        assert_eq!(unique_cycles_ceiling(0, 2), None);
    }

    // ---- ARBX-0028: cross-chain generalization (workbook rows 27–31) ----

    /// Property (r28/r29): the Σ_c sums equal a brute-force recount over
    /// every chain (pairs via the double-loop count, edges via N(N−1) per
    /// chain) across deterministic pseudo-random chain vectors.
    #[test]
    fn cross_chain_sums_match_bruteforce() {
        let mut seed = 0x5EED_u64;
        let mut next = || {
            // LCG (Knuth): deterministic, no rand dep (Cero Dependencias).
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            seed
        };
        for _case in 0..200 {
            let n_chains = 1 + (next() % 5) as usize;
            let chain_sizes: Vec<usize> = (0..n_chains).map(|_| (next() % 27) as usize).collect();

            let mut brute_pairs = 0u128;
            let mut brute_edges = 0u128;
            for &n in &chain_sizes {
                if n < 2 {
                    continue; // C(0,2) = C(1,2) = 0 — nothing to count
                }
                let nf = n as u128;
                brute_pairs += nf * (nf - 1) / 2;
                brute_edges += nf * (nf - 1);
            }

            assert_eq!(
                within_chain_pairs(&chain_sizes),
                Some(brute_pairs),
                "chain_sizes={chain_sizes:?}"
            );
            assert_eq!(
                within_chain_directed_edges(&chain_sizes),
                Some(brute_edges),
                "chain_sizes={chain_sizes:?}"
            );
        }
    }

    /// Property (r19–24 × Σ_c): the cross-chain ceiling equals the per-chain
    /// ceilings summed, with `N_c < h` chains contributing an exact 0 — and
    /// the r30 doctrine is structurally pinned (`bridge_gated` always true).
    #[test]
    fn cross_chain_ceiling_equals_sum_of_per_chain() {
        for h in 2..=7u8 {
            for &extra in &[0usize, 1, 2, 3, (h - 1) as usize, 22, 26] {
                let chain_sizes = [22usize, extra, 4];
                let mut want = 0u128;
                for &n in &chain_sizes {
                    want += if n < h as usize {
                        0
                    } else {
                        unique_cycles_ceiling(n, h).expect("n >= h in range")
                    };
                }
                let got =
                    cross_chain_cycles_ceiling(&chain_sizes, h).expect("canonical h computes");
                assert_eq!(got.within_chain, want, "h={h} chains={chain_sizes:?}");
                assert!(got.bridge_gated, "r30 is a type-level invariant");
            }
        }
        // Misuse → honest None (never a partial undercount).
        assert_eq!(cross_chain_cycles_ceiling(&[22, 4], 1), None);
        assert_eq!(cross_chain_cycles_ceiling(&[22, 4], 8), None);
        // Degenerate universes.
        let empty = cross_chain_cycles_ceiling(&[], 3).expect("empty is valid");
        assert_eq!(empty.within_chain, 0);
        assert_eq!(within_chain_pairs(&[]), Some(0));
        assert_eq!(within_chain_directed_edges(&[]), Some(0));
        assert_eq!(within_chain_pairs(&[0, 1, 0]), Some(0));
    }

    /// Workbook demo generalized: a single N=22 chain reproduces rows 19–24
    /// exactly; adding a second chain of the same size doubles them (Σ_c).
    #[test]
    fn cross_chain_workbook_example_doubles() {
        for (h, want22) in [
            (2u8, 231u128),
            (3, 3_080),
            (4, 43_890),
            (5, 632_016),
            (6, 8_953_560),
            (7, 122_791_680),
        ] {
            assert_eq!(
                cross_chain_cycles_ceiling(&[22], h).map(|c| c.within_chain),
                Some(want22),
                "single chain h={h}"
            );
            assert_eq!(
                cross_chain_cycles_ceiling(&[22, 22], h).map(|c| c.within_chain),
                Some(want22 * 2),
                "two chains h={h}"
            );
        }
        assert_eq!(within_chain_pairs(&[22]), Some(231));
        assert_eq!(within_chain_pairs(&[22, 22]), Some(462));
        assert_eq!(within_chain_directed_edges(&[22]), Some(462));
    }

    /// Overflow is a checked `None` — never a wrapped (fabricated) magnitude.
    /// u128 headroom math: one chain at `usize::MAX` still fits
    /// (`N(N−1) < u128::MAX`), so the overflow cases need sums of near-max
    /// chains (2 for edges, 3 for pairs); the h=7 ceiling overflows on its
    /// own falling factorial long before the sum.
    #[test]
    fn cross_chain_overflow_is_none() {
        // Single near-max chain: fits (documents the u128 headroom).
        assert!(within_chain_pairs(&[usize::MAX]).is_some());
        assert!(within_chain_directed_edges(&[usize::MAX]).is_some());
        // Sums past u128::MAX → checked None.
        assert_eq!(within_chain_directed_edges(&[usize::MAX; 2]), None);
        assert_eq!(within_chain_pairs(&[usize::MAX; 3]), None);
        // The h=7 ceiling overflows internally at N ≈ 2^62 (N^7 ≫ 2^128).
        assert!(cross_chain_cycles_ceiling(&[usize::MAX / 4], 7).is_none());
        let many = vec![usize::MAX / 4; 8];
        assert!(cross_chain_cycles_ceiling(&many, 7).is_none());
    }

    // ---- DenseIdBuilder (workbook 15_CONTRACT step 3 / 09 DenseIdMap) ----

    fn tkey(chain_id: u64, byte: u8) -> TokenKey {
        TokenKey {
            chain_id,
            address: Address::from([byte; 20]),
        }
    }

    /// Dense ids are first-seen order; re-insert is an idempotent upsert
    /// (stable id, refreshed `allowed`); unknown keys are `None` (honest).
    #[test]
    fn dense_ids_first_seen_and_idempotent() {
        let mut b = DenseIdBuilder::new();
        assert!(b.is_empty());
        let k0 = tkey(1, 0xAA);
        let k1 = tkey(137, 0xBB); // same address, DIFFERENT chain → distinct key
        let k2 = tkey(1, 0xCC);
        assert_eq!(b.insert(k0, true), 0);
        assert_eq!(b.insert(k1, false), 1);
        assert_eq!(b.insert(k2, true), 2);
        assert_eq!(b.len(), 3);
        // Upsert: same key returns the same id, flag refreshed.
        assert_eq!(b.insert(k1, true), 1);
        assert_eq!(b.len(), 3);
        assert_eq!(b.allowed(1), Some(true));
        // Unknown key (never inserted) → None; no dense id fabricated.
        assert_eq!(b.id(&tkey(999, 0xAA)), None);
        assert_eq!(b.key(3), None);
        assert_eq!(b.allowed(3), None);
    }

    /// `key`/`id` round-trip over the whole universe; `snapshot` carries
    /// `(TokenKey, id, allowed)` in dense order.
    #[test]
    fn dense_key_id_roundtrip_and_snapshot() {
        let mut b = DenseIdBuilder::new();
        let keys = [tkey(1, 0x01), tkey(1, 0x02), tkey(10, 0x03), tkey(1, 0x04)];
        for (i, k) in keys.iter().enumerate() {
            assert_eq!(b.insert(*k, i % 2 == 0), i);
        }
        for (i, k) in keys.iter().enumerate() {
            assert_eq!(b.id(k), Some(i));
            assert_eq!(b.key(i), Some(*k));
        }
        let snap = b.snapshot();
        assert_eq!(snap.len(), 4);
        for (i, (k, id, allowed)) in snap.iter().enumerate() {
            assert_eq!(*id, i);
            assert_eq!(*k, keys[i]);
            assert_eq!(*allowed, i % 2 == 0);
        }
        // set_allowed flips and reports range honestly.
        assert!(b.set_allowed(2, true));
        assert_eq!(b.allowed(2), Some(true));
        assert!(!b.set_allowed(9, true));
    }

    /// `pair_bucket` is plain `pair_index` over the live N — degenerate
    /// pairs are `None` exactly as in the pure function.
    #[test]
    fn builder_pair_bucket_matches_pure_index() {
        let mut b = DenseIdBuilder::new();
        for byte in 0u8..22 {
            b.insert(tkey(1, byte), true);
        }
        assert_eq!(b.pair_bucket(3, 17), pair_index(3, 17, 22));
        assert_eq!(b.pair_bucket(3, 17), Some(73)); // workbook worked example
        assert_eq!(b.pair_bucket(17, 3), Some(73)); // order-independent
        assert_eq!(b.pair_bucket(5, 5), None);
        assert_eq!(b.pair_bucket(0, 22), None);
    }

    // ---- PairBuckets (workbook 09_RUNTIME_STRUCTURES) ----

    /// Parallel pools on the same pair share ONE bucket and EVERY pushed
    /// edge survives lookup (bijection edge↔bucket entry); degenerate
    /// pushes are rejected no-ops.
    #[test]
    fn buckets_preserve_parallel_edges() {
        let mut pb: PairBuckets<&'static str> = PairBuckets::new(8);
        assert_eq!(pb.bucket_count(), pair_count(8));
        assert!(pb.push(2, 5, "univ2"));
        assert!(pb.push(5, 2, "univ3")); // same unordered pair
        assert!(pb.push(2, 5, "sushi")); // third parallel pool
        assert!(pb.push(0, 1, "edge"));
        assert_eq!(pb.total_entries(), 4);
        let bucket = pb.bucket(2, 5).expect("pair 2-5 exists");
        assert_eq!(bucket.len(), 3);
        // The two direction pushes normalize to the same stored pair.
        assert_eq!(bucket.iter().filter(|e| e.2 == "univ3").count(), 1);
        assert_eq!(
            pb.bucket(5, 2).unwrap().len(),
            3,
            "lookup is order-independent"
        );
        // Degenerate: rejected, nothing stored, total unchanged.
        assert!(!pb.push(4, 4, "self"));
        assert!(!pb.push(0, 8, "oob"));
        assert_eq!(pb.total_entries(), 4);
        assert!(pb.bucket(4, 4).is_none());
        assert!(pb.bucket(0, 8).is_none());
        // An untouched pair has an empty (but present) bucket.
        assert_eq!(pb.bucket(0, 7), Some(&[][..]));
    }

    /// `iter_pairs` yields exactly the non-empty buckets in lexicographic
    /// pair order with their edges — nothing dropped, nothing duplicated.
    #[test]
    fn iter_pairs_is_complete_and_ordered() {
        let mut pb: PairBuckets<u32> = PairBuckets::new(6);
        pb.push(4, 4, 0); // rejected
        pb.push(0, 2, 10);
        pb.push(0, 1, 20);
        pb.push(0, 2, 11);
        pb.push(3, 1, 30); // reversed order on purpose
        let seen: Vec<(usize, usize, usize)> = pb
            .iter_pairs()
            .map(|(a, b, entries)| (a, b, entries.len()))
            .collect();
        assert_eq!(seen, vec![(0, 1, 1), (0, 2, 2), (1, 3, 1)]);
        assert_eq!(pb.total_entries(), 4);
    }

    /// Universe change → full `rebuild` re-buckets every entry under the
    /// new N and REPORTS entries that became degenerate (R8 visible drop).
    #[test]
    fn rebuild_rebuckets_and_reports_drops() {
        let mut pb: PairBuckets<u32> = PairBuckets::new(5);
        pb.push(0, 1, 1);
        pb.push(3, 4, 2);
        pb.push(2, 4, 3);
        assert_eq!(pb.total_entries(), 3);
        // Grow: nothing can become degenerate, all survive.
        let dropped = pb.rebuild(8);
        assert_eq!(dropped, 0);
        assert_eq!(pb.n(), 8);
        assert_eq!(pb.bucket_count(), pair_count(8));
        assert_eq!(pb.total_entries(), 3);
        assert_eq!(pb.bucket(3, 4).map(|e| e.len()), Some(1));
        // Shrink below an existing id: those entries DROP, count returned.
        let dropped = pb.rebuild(3);
        assert_eq!(dropped, 2); // (3,4) and (2,4) out of range now
        assert_eq!(pb.total_entries(), 1);
        assert_eq!(pb.bucket(0, 1).map(|e| e.len()), Some(1));
    }

    /// N < 2 is a valid empty layer (no buckets, no panic) — honest-empty.
    #[test]
    fn buckets_below_minimum_n() {
        let mut pb: PairBuckets<u8> = PairBuckets::new(1);
        assert_eq!(pb.bucket_count(), 0);
        assert!(!pb.push(0, 1, 9));
        assert_eq!(pb.total_entries(), 0);
        assert_eq!(pb.rebuild(4), 0);
        assert_eq!(pb.bucket_count(), pair_count(4));
    }

    /// QB-04-009: injectivity + exact range for the larger universes the
    /// benchmark matrix sweeps (N=8..128) — mirrors the Python twin
    /// (`scratchpad/qb04_dense_twin.py`, 24/24 PASS 2026-08-23).
    #[test]
    fn injective_and_full_range_n_32_64_128() {
        for n in [32usize, 64, 128] {
            let total = pair_count(n);
            let mut seen = std::collections::HashSet::with_capacity(total);
            let (mut min_k, mut max_k) = (usize::MAX, 0usize);
            for i in 0..n {
                for j in (i + 1)..n {
                    let k = pair_index(i, j, n).expect("valid pair must index");
                    // Positional args: edition-agnostic assert message (implicit
                    // format captures are 2021-only; probes compile this file
                    // under older editions and `non_fmt_panics` fires there).
                    assert!(seen.insert(k), "collision at n={} ({},{}) → {}", n, i, j, k);
                    min_k = min_k.min(k);
                    max_k = max_k.max(k);
                }
            }
            assert_eq!(seen.len(), total, "n={n} pair count");
            assert_eq!((min_k, max_k), (0, total - 1), "n={n} range");
        }
    }
}
