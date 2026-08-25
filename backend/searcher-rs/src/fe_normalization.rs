//! F_e normalization prefilter — XLS-QB-06b (workbook #5 `05_QUOTE_BASE`).
//!
//! Pure math, no I/O — anchor prices and executable rates are supplied by
//! the consumer layer (G-PRICE price stream + quoters). Ids are the dense
//! token ids built by `pair_index::DenseIdBuilder` (QB-04); the module never
//! touches addresses or symbols directly (symbol is never a runtime key).
//!
//! Workbook rows implemented:
//! - r13 Fair rate from anchor Q: `r*_{A→B} = P_Q(A)/P_Q(B)` where `P_Q(x)`
//!   is the value of `x` measured in the quote anchor.
//! - r14 Normalized edge factor: `F_e(q) = r_e(q) · P_Q(dst)/P_Q(src)`.
//!   `F_e > 1` means "better than reference", NOT full arbitrage — this is a
//!   prefilter signal; PASS is decided by exact net-gate evaluation.
//! - r15 Directed edge preservation: Pair(A,B) ⇒ Edge A→B AND Edge B→A per
//!   pool/venue. The two directions normalize independently here; choosing a
//!   QUOTE never collapses or deletes the reverse direction.
//! - r23 QuoteState: per-chain anchor prices + reference freshness/version.
//! - r25 Version keys: `allowed_set_version`, `topology_version`,
//!   `state_block`, `quote_version` — consumers recompute whenever any key
//!   differs.
//!
//! Fail-honest (R8): `Ok(None)` = not computed (missing anchor price, no
//! executable rate, non-finite result) — never a fabricated rate and never
//! a silent 0. `Err` is reserved for structural misuse (id outside the
//! dense universe, self-edge, invalid price write).

use std::fmt;

/// r25 version keys. Any difference between the version a cached F_e was
/// computed under and the current one invalidates the cached value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateVersion {
    pub allowed_set_version: u64,
    pub topology_version: u64,
    pub state_block: u64,
    pub quote_version: u64,
}

impl StateVersion {
    pub const fn zeroed() -> Self {
        StateVersion {
            allowed_set_version: 0,
            topology_version: 0,
            state_block: 0,
            quote_version: 0,
        }
    }
}

impl Default for StateVersion {
    fn default() -> Self {
        Self::zeroed()
    }
}

/// Structural misuse of the normalization surface (fail-fast, not a data
/// condition — those are `Ok(None)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeError {
    /// Dense token id outside the universe built for this chain.
    TokenOutOfBounds { token: usize },
    /// src == dst: pools always pair two distinct tokens.
    SelfEdge { token: usize },
    /// `set_price` rejected a non-finite or non-positive anchor price.
    InvalidAnchorPrice,
}

impl fmt::Display for FeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeError::TokenOutOfBounds { token } => {
                write!(f, "token id {} outside dense universe", token)
            }
            FeError::SelfEdge { token } => {
                write!(f, "self-edge on token id {} (pairs need two tokens)", token)
            }
            FeError::InvalidAnchorPrice => {
                write!(f, "anchor price must be finite and > 0")
            }
        }
    }
}

impl std::error::Error for FeError {}

/// r14 result for one directed edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedEdge {
    /// `F_e(q) = r_e(q) · P_Q(dst)/P_Q(src)`. `> 1` beats the anchor
    /// reference (r14 note: not full arbitrage).
    pub f_e: f64,
    /// `ln(F_e)` — the per-edge contribution to the cycle log-alpha
    /// prefilter (08_HOPS direct spread form). Sign matches `f_e` vs 1.
    pub ln_alpha: f64,
}

impl NormalizedEdge {
    /// r14: strictly better than the anchor reference. This is a prefilter
    /// bit, not an arbitrage proof — exact net gates still decide.
    pub fn beats_reference(&self) -> bool {
        self.f_e > 1.0
    }
}

/// Pair prefilter form (15_IMPLEMENTATION_CONTRACT step 8): forward and
/// reverse executable rates normalized independently (r15).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairAlpha {
    pub forward: Option<NormalizedEdge>,
    pub reverse: Option<NormalizedEdge>,
}

impl PairAlpha {
    /// True only when BOTH directions are computed and at least one beats
    /// the reference — the cheapest pair-level "worth expanding" bit.
    pub fn any_beats_reference(&self) -> bool {
        self.forward.is_some_and(|e| e.beats_reference())
            || self.reverse.is_some_and(|e| e.beats_reference())
    }
}

/// r23 QuoteState: per-chain anchor prices with freshness/version keys.
///
/// `anchor_price[id] = P_Q(id)` — value of the token measured in the quote
/// anchor (the anchor token itself prices at 1.0). `None` = no reference
/// price yet (fail-honest; the F_e for edges touching that token is not
/// computed, never fabricated).
#[derive(Debug, Clone)]
pub struct QuoteState {
    pub chain_id: u64,
    pub anchor_price: Vec<Option<f64>>,
    pub version: StateVersion,
}

impl QuoteState {
    /// New state over a dense universe of `n_tokens` ids, all prices unset.
    pub fn new(chain_id: u64, n_tokens: usize) -> Self {
        QuoteState {
            chain_id,
            anchor_price: vec![None; n_tokens],
            version: StateVersion::zeroed(),
        }
    }

    pub fn n_tokens(&self) -> usize {
        self.anchor_price.len()
    }

    /// P_Q(id). `None` = missing reference price (bounds-checked the same
    /// way as the normalization entry points).
    pub fn price_of(&self, id: usize) -> Result<Option<f64>, FeError> {
        match self.anchor_price.get(id) {
            Some(p) => Ok(*p),
            None => Err(FeError::TokenOutOfBounds { token: id }),
        }
    }

    /// Set `P_Q(id)`. Rejects non-finite / non-positive prices (a poisoned
    /// anchor would fabricate F_e values). Bumps `quote_version` only when
    /// the stored value actually changes. Returns the previous price.
    pub fn set_price(&mut self, id: usize, price_q: f64) -> Result<Option<f64>, FeError> {
        if !price_q.is_finite() || price_q <= 0.0 {
            return Err(FeError::InvalidAnchorPrice);
        }
        let slot = match self.anchor_price.get_mut(id) {
            Some(s) => s,
            None => return Err(FeError::TokenOutOfBounds { token: id }),
        };
        if *slot == Some(price_q) {
            return Ok(*slot); // no state transition, no version bump
        }
        let old = *slot;
        *slot = Some(price_q);
        self.version.quote_version = self.version.quote_version.wrapping_add(1);
        Ok(old)
    }

    /// r25: allowed-set changed (token allowlist edit) — invalidates cached
    /// normalizations that assumed the old set.
    pub fn bump_allowed_set_version(&mut self) {
        self.version.allowed_set_version = self.version.allowed_set_version.wrapping_add(1);
    }

    /// r25: topology changed (pools/venues added or removed).
    pub fn bump_topology_version(&mut self) {
        self.version.topology_version = self.version.topology_version.wrapping_add(1);
    }

    /// r25: prices observed at `block`. Monotonic — stale blocks are ignored
    /// (a late write must not roll freshness backwards).
    pub fn advance_block(&mut self, block: u64) {
        if block > self.version.state_block {
            self.version.state_block = block;
        }
    }

    /// Read-side anchor price usable for math: finite and `> 0`.
    fn usable_price(&self, id: usize) -> Option<f64> {
        match self.anchor_price.get(id) {
            Some(Some(p)) if p.is_finite() && *p > 0.0 => Some(*p),
            _ => None,
        }
    }

    fn check_edge(&self, src: usize, dst: usize) -> Result<(), FeError> {
        if src >= self.anchor_price.len() {
            return Err(FeError::TokenOutOfBounds { token: src });
        }
        if dst >= self.anchor_price.len() {
            return Err(FeError::TokenOutOfBounds { token: dst });
        }
        if src == dst {
            return Err(FeError::SelfEdge { token: src });
        }
        Ok(())
    }

    /// r13 fair (reference) rate `r*_{A→B} = P_Q(A)/P_Q(B)`.
    /// `Ok(None)` when either anchor price is missing/unusable.
    pub fn fair_rate(&self, src: usize, dst: usize) -> Result<Option<f64>, FeError> {
        self.check_edge(src, dst)?;
        let (Some(p_src), Some(p_dst)) = (self.usable_price(src), self.usable_price(dst)) else {
            return Ok(None); // missing anchor price → not computed (R8)
        };
        let r = p_src / p_dst;
        if r.is_finite() && r > 0.0 {
            Ok(Some(r))
        } else {
            Ok(None)
        }
    }

    /// r14 `F_e(q) = r_e(q) · P_Q(dst)/P_Q(src)`.
    ///
    /// `executable_rate` is the venue's real dst-per-src rate for this
    /// amount bucket (quoter output — pass `None` straight through when the
    /// quoter had none). `Ok(None)` = not computed (missing anchor, no
    /// executable rate, or non-finite result) — fail-honest, R8.
    pub fn normalized_edge(
        &self,
        src: usize,
        dst: usize,
        executable_rate: Option<f64>,
    ) -> Result<Option<NormalizedEdge>, FeError> {
        self.check_edge(src, dst)?;
        let (Some(p_src), Some(p_dst)) = (self.usable_price(src), self.usable_price(dst)) else {
            return Ok(None); // missing anchor price → not computed (R8)
        };
        let Some(rate) = executable_rate else {
            return Ok(None); // quoter had no rate → not computed (R8)
        };
        if !rate.is_finite() || rate <= 0.0 {
            return Ok(None);
        }
        let f_e = rate * p_dst / p_src;
        if !f_e.is_finite() || f_e <= 0.0 {
            return Ok(None); // overflow/underflow → not computed, never inf
        }
        Ok(Some(NormalizedEdge {
            f_e,
            ln_alpha: f_e.ln(),
        }))
    }

    /// Step-8 pair prefilter: forward and reverse normalized independently
    /// (r15 — never collapse the directions).
    pub fn pair_alpha(
        &self,
        a: usize,
        b: usize,
        rate_ab: Option<f64>,
        rate_ba: Option<f64>,
    ) -> Result<PairAlpha, FeError> {
        Ok(PairAlpha {
            forward: self.normalized_edge(a, b, rate_ab)?,
            reverse: self.normalized_edge(b, a, rate_ba)?,
        })
    }

    /// Cycle log-alpha prefilter: sum of per-edge `ln F_e` around a closed
    /// walk (08_HOPS direct spread, normalized against the anchor). A cycle
    /// with ANY uncomputable edge has no alpha — `Ok(None)`, not a partial
    /// sum (R8). `alpha > 1 ⇔ ln_alpha > 0` is still only a prefilter bit.
    pub fn cycle_ln_alpha(
        &self,
        cycle: &[usize],
        rate_of: &dyn Fn(usize, usize) -> Option<f64>,
    ) -> Result<Option<f64>, FeError> {
        if cycle.len() < 2 {
            return Ok(None);
        }
        let mut sum = 0.0f64;
        for w in cycle.windows(2) {
            let edge = self.normalized_edge(w[0], w[1], rate_of(w[0], w[1]))?;
            let edge = match edge {
                Some(e) => e,
                None => return Ok(None), // one unpriced edge kills the cycle
            };
            sum += edge.ln_alpha;
        }
        if sum.is_finite() {
            Ok(Some(sum))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// USDC-anchor fixture: P_Q(USDC)=1, P_Q(WETH)=3000, P_Q(WBTC)=60000,
    /// LINK unset (missing reference — the honest branch).
    fn usdc_anchor_state() -> QuoteState {
        let mut s = QuoteState::new(1, 4);
        // ids: 0=USDC (anchor), 1=WETH, 2=WBTC, 3=LINK(no price)
        s.set_price(0, 1.0).unwrap();
        s.set_price(1, 3000.0).unwrap();
        s.set_price(2, 60000.0).unwrap();
        s
    }

    #[test]
    fn fair_rate_matches_workbook_form_r13() {
        let s = usdc_anchor_state();
        // r*_{WETH→USDC} = P_Q(WETH)/P_Q(USDC) = 3000/1
        let r = s.fair_rate(1, 0).unwrap().unwrap();
        assert!((r - 3000.0).abs() < 1e-9, "r* = {}", r);
        // r*_{USDC→WBTC} = 1/60000
        let r = s.fair_rate(0, 2).unwrap().unwrap();
        assert!((r - 1.0 / 60000.0).abs() < 1e-15, "r* = {}", r);
    }

    #[test]
    fn f_e_matches_workbook_form_r14() {
        let s = usdc_anchor_state();
        // WETH→USDC executable rate 3010 USDC per WETH:
        // F_e = 3010 · P_Q(USDC)/P_Q(WETH) = 3010/3000
        let e = s.normalized_edge(1, 0, Some(3010.0)).unwrap().unwrap();
        let expected = 3010.0 * 1.0 / 3000.0;
        assert!((e.f_e - expected).abs() < 1e-12, "F_e = {}", e.f_e);
        assert!(e.beats_reference(), "3010 > 3000 fair must beat reference");
    }

    #[test]
    fn f_e_equals_rate_over_fair_rate() {
        let s = usdc_anchor_state();
        let rate = 59000.0; // WBTC→WETH direction: WETH per WBTC
        let fair = s.fair_rate(2, 1).unwrap().unwrap();
        let e = s.normalized_edge(2, 1, Some(rate)).unwrap().unwrap();
        assert!((e.f_e - rate / fair).abs() < 1e-9, "F_e != rate/r*");
    }

    #[test]
    fn f_e_one_at_fair_rate_is_not_beats_reference() {
        let s = usdc_anchor_state();
        let fair = s.fair_rate(1, 0).unwrap().unwrap();
        let e = s.normalized_edge(1, 0, Some(fair)).unwrap().unwrap();
        assert!((e.f_e - 1.0).abs() < 1e-12, "F_e at fair rate = 1");
        assert!(!e.beats_reference(), "strictly > 1 is required (r14)");
        assert!(e.ln_alpha.abs() < 1e-12, "ln(1) = 0");
    }

    #[test]
    fn ln_alpha_sign_tracks_reference_comparison() {
        let s = usdc_anchor_state();
        let above = s.normalized_edge(1, 0, Some(3001.0)).unwrap().unwrap();
        let below = s.normalized_edge(1, 0, Some(2999.0)).unwrap().unwrap();
        assert!(above.ln_alpha > 0.0 && above.beats_reference());
        assert!(below.ln_alpha < 0.0 && !below.beats_reference());
    }

    #[test]
    fn missing_anchor_price_is_none_not_zero() {
        let s = usdc_anchor_state();
        // LINK (id 3) has no reference price → R8: not computed.
        let e = s.normalized_edge(3, 1, Some(1.0)).unwrap();
        assert_eq!(
            e, None,
            "missing anchor must be None, never 0 or fabricated"
        );
        assert_eq!(s.fair_rate(3, 1).unwrap(), None);
    }

    #[test]
    fn invalid_rate_collapses_to_none() {
        let s = usdc_anchor_state();
        for bad in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            let e = s.normalized_edge(1, 0, bad).unwrap();
            assert_eq!(e, None, "rate {:?} must be Ok(None)", bad);
        }
    }

    #[test]
    fn poisoned_anchor_price_read_is_guarded() {
        let mut s = usdc_anchor_state();
        // set_price would reject it, but the field is pub: defense-in-depth
        // on read (a 0 price would fabricate inf F_e).
        s.anchor_price[1] = Some(0.0);
        assert_eq!(s.normalized_edge(1, 0, Some(3000.0)).unwrap(), None);
        assert_eq!(s.fair_rate(1, 0).unwrap(), None);
    }

    #[test]
    fn structural_misuse_is_err() {
        let s = usdc_anchor_state();
        assert_eq!(
            s.normalized_edge(4, 0, Some(1.0)),
            Err(FeError::TokenOutOfBounds { token: 4 })
        );
        assert_eq!(
            s.normalized_edge(1, 4, Some(1.0)),
            Err(FeError::TokenOutOfBounds { token: 4 })
        );
        assert_eq!(s.fair_rate(1, 1), Err(FeError::SelfEdge { token: 1 }));
        let mut s2 = QuoteState::new(1, 1);
        assert_eq!(s2.set_price(0, 0.0), Err(FeError::InvalidAnchorPrice));
        assert_eq!(s2.set_price(0, f64::NAN), Err(FeError::InvalidAnchorPrice));
        assert_eq!(
            s2.set_price(9, 1.0),
            Err(FeError::TokenOutOfBounds { token: 9 })
        );
    }

    #[test]
    fn set_price_bumps_quote_version_only_on_change() {
        let mut s = QuoteState::new(1, 2);
        assert_eq!(s.version.quote_version, 0);
        s.set_price(0, 1.0).unwrap();
        assert_eq!(s.version.quote_version, 1);
        s.set_price(0, 1.0).unwrap(); // same value → no transition
        assert_eq!(s.version.quote_version, 1, "no-op write must not bump");
        s.set_price(0, 2.0).unwrap(); // anchor re-denominated
        assert_eq!(s.version.quote_version, 2);
        assert_eq!(s.price_of(0).unwrap(), Some(2.0));
    }

    #[test]
    fn version_keys_drive_invalidation() {
        let mut s = QuoteState::new(1, 2);
        let v0 = s.version;
        assert_eq!(s.version, v0);
        s.bump_allowed_set_version();
        assert_ne!(s.version, v0, "allowed-set edit invalidates");
        s.bump_topology_version();
        assert_ne!(s.version, v0);
        s.advance_block(100);
        assert_ne!(s.version, v0);
        s.advance_block(50); // stale block must NOT roll freshness back
        assert_eq!(s.version.state_block, 100);
        let v1 = s.version;
        s.advance_block(100); // re-observing same block is not a transition
        assert_eq!(s.version, v1);
    }

    #[test]
    fn directed_edges_normalize_independently_r15() {
        let s = usdc_anchor_state();
        // Asymmetric executable rates (fee venue): forward ≠ reverse.
        let pa = s
            .pair_alpha(1, 0, Some(3010.0), Some(1.0 / 3002.0))
            .unwrap();
        let fwd = pa.forward.unwrap();
        let rev = pa.reverse.unwrap();
        assert!(fwd.beats_reference(), "forward 3010 beats 3000 fair");
        assert!(!rev.beats_reference(), "reverse 1/3002 under 1/3000 fair");
        // Independent objects: recomputing one direction leaves the other.
        let again = s.normalized_edge(1, 0, Some(3010.0)).unwrap().unwrap();
        assert_eq!(again, fwd);
        // The missing-price direction is honest while the other still works.
        let pa = s.pair_alpha(1, 3, Some(1.0), Some(1.0)).unwrap();
        assert!(pa.forward.is_none() && pa.reverse.is_none());
        let pa = s.pair_alpha(0, 3, None, None).unwrap();
        assert!(!pa.any_beats_reference());
    }

    #[test]
    fn cycle_ln_alpha_sums_edges_and_fails_honest() {
        let s = usdc_anchor_state();
        // Triangle USDC→WETH→WBTC→USDC with executable rates (dst per src):
        // 1/3000 WETH per USDC · 0.05 WBTC per WETH · 59900 USDC per WBTC.
        let rates: &[((usize, usize), f64)] =
            &[((0, 1), 1.0 / 3000.0), ((1, 2), 0.05), ((2, 0), 59900.0)];
        let rate_of = |a: usize, b: usize| -> Option<f64> {
            rates
                .iter()
                .find(|((x, y), _)| *x == a && *y == b)
                .map(|(_, r)| *r)
        };
        let cycle = [0usize, 1, 2, 0];
        let la = s.cycle_ln_alpha(&cycle, &rate_of).unwrap().unwrap();
        let manual: f64 = [((0, 1), 1.0 / 3000.0), ((1, 2), 0.05), ((2, 0), 59900.0)]
            .iter()
            .map(|((a, b), r)| {
                let e = s.normalized_edge(*a, *b, Some(*r)).unwrap().unwrap();
                e.ln_alpha
            })
            .sum();
        assert!(
            (la - manual).abs() < 1e-12,
            "sum mismatch: {} vs {}",
            la,
            manual
        );
        // Cycle product: (1/3000)·0.05·59900 = 59900/60000 < 1 → loses.
        assert!(
            la < 0.0,
            "this triangle loses vs reference (59900<60000 leg)"
        );
        // One unpriced edge → the whole cycle is not computed.
        let hole = |a: usize, b: usize| -> Option<f64> {
            if (a, b) == (1, 2) {
                None
            } else {
                rate_of(a, b)
            }
        };
        assert_eq!(s.cycle_ln_alpha(&cycle, &hole).unwrap(), None);
        // Degenerate cycle (needs >= 2 nodes to walk).
        assert_eq!(s.cycle_ln_alpha(&[0], &rate_of).unwrap(), None);
    }

    #[test]
    fn per_chain_states_are_independent() {
        let mut a = QuoteState::new(1, 2);
        let mut b = QuoteState::new(137, 2);
        a.set_price(0, 1.0).unwrap();
        assert_eq!(b.price_of(0).unwrap(), None, "chain 137 untouched");
        assert_eq!(b.version, StateVersion::zeroed());
        assert_ne!(a.version, b.version);
        b.advance_block(42);
        assert_eq!(a.version.state_block, 0);
    }

    /// QB-06-007 caso 1 (05_QUOTE_BASE + 00_MANUAL TokenKey doctrine): the
    /// same display symbol on TWO different addresses (native vs bridged
    /// USDC) keeps TWO dense identities — identity is `(chain, address)`,
    /// never symbol (05 r24: no `HashMap<String symbol,…>` in the hot path).
    /// The symbol-keyed price stream assigns both the SAME quote price, but
    /// each dense id owns its own slot: repricing one never reprices the
    /// other, and no route through either collapses onto the other.
    #[test]
    fn same_symbol_different_addresses_never_collapse_identity() {
        use crate::pair_index::{DenseIdBuilder, TokenKey};
        use ethers::types::Address;
        use std::str::FromStr;

        let native = TokenKey {
            chain_id: 1,
            address: Address::from_str("0x1111111111111111111111111111111111111111")
                .expect("40 hex chars"),
        };
        let bridged = TokenKey {
            chain_id: 1,
            address: Address::from_str("0x2222222222222222222222222222222222222222")
                .expect("40 hex chars"),
        };
        let mut builder = DenseIdBuilder::new();
        let id_native = builder.insert(native, true);
        let id_bridged = builder.insert(bridged, true);
        assert_ne!(
            id_native, id_bridged,
            "identity is (chain, address) — the shared symbol must not merge them"
        );
        assert_eq!(builder.len(), 2);
        assert_eq!(builder.snapshot().len(), 2);

        // Both share the stream's symbol-keyed price — but each dense id
        // owns its slot: a reprice on one identity never reprices the other.
        let mut qs = QuoteState::new(1, builder.len());
        qs.set_price(id_native, 1.0).unwrap();
        qs.set_price(id_bridged, 1.0).unwrap();
        let v_shared = qs.version.quote_version;
        qs.set_price(id_native, 0.99).unwrap();
        assert_eq!(
            qs.price_of(id_bridged).unwrap(),
            Some(1.0),
            "bridged id untouched by the native reprice"
        );
        assert_eq!(qs.version.quote_version, v_shared + 1); // exactly one transition
    }

    /// QB-06-007 caso 2 (09_RUNTIME_STRUCTURES TokenRegistry + the worker
    /// glue): the same ADDRESS on two chains is TWO tokens — distinct
    /// `TokenKey`s, one builder+`QuoteState` PER chain (each prices from its
    /// own `arbx:token_prices:<chain>` stream). Extends
    /// `per_chain_states_are_independent` down to the dense-id contract the
    /// worker actually builds: ids are epoch-local, so `0` in chain 1 and `0`
    /// in chain 137 are different tokens, and nothing crosses.
    #[test]
    fn cross_chain_same_address_keeps_isolated_states() {
        use crate::pair_index::{DenseIdBuilder, TokenKey};
        use ethers::types::Address;
        use std::str::FromStr;

        let address =
            Address::from_str("0x3333333333333333333333333333333333333333").expect("40 hex chars");
        let key_eth = TokenKey {
            chain_id: 1,
            address,
        };
        let key_other = TokenKey {
            chain_id: 137,
            address, // SAME address, other chain ⇒ a different token
        };
        let mut b_eth = DenseIdBuilder::new();
        let mut b_other = DenseIdBuilder::new();
        let id_eth = b_eth.insert(key_eth, true);
        let id_other = b_other.insert(key_other, true);

        let mut qs_eth = QuoteState::new(1, b_eth.len());
        let mut qs_other = QuoteState::new(137, b_other.len());
        qs_eth.set_price(id_eth, 3000.0).unwrap();
        // Chain 137: its own stream never priced the token — UNSET, never
        // borrowed from chain 1 (R8).
        assert_eq!(qs_other.price_of(id_other).unwrap(), None);
        assert_eq!(qs_other.version, StateVersion::zeroed());
        assert_eq!(qs_eth.price_of(id_eth).unwrap(), Some(3000.0));
        // Epoch-local ids: both are 0 in their OWN builders — the contract
        // that lets each chain own its universe independently.
        assert_eq!(id_eth, 0);
        assert_eq!(id_other, 0);
    }
}
