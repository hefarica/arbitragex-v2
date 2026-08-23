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
                    assert!(seen.insert(k), "collision at n={n} ({i},{j}) → {k}");
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
}
