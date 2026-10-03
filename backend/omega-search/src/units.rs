//! Exact monetary units.
//!
//! Mandate (PROMPT §4, mission rule 2): token amounts travel as **integers of
//! minimal units** with controlled overflow, and money/price keeps explicit
//! decimal precision. `f64` is forbidden for monetary amounts — this module
//! deliberately exposes no float conversion at all.
//!
//! Rounding here is *protocol-favouring floor* (`div_euclid`), which is the
//! direction Uniswap-V2-style implementations round: the pool never pays out a
//! base unit it did not receive. Overflow is an explicit `None`, never a wrap
//! and never a saturating clamp that would fabricate a value.

use core::fmt;

/// Denominator for basis points.
pub const BPS_DENOM: i128 = 10_000;

/// An integer count of a token's minimal units (`wei`, `1e-6`, …).
///
/// The token identity and its `decimals` are contextual (carried by the graph
/// edge / adapter); `Raw` is only ever compared against `Raw` of the *same*
/// token. Mixing denominations is a caller bug, not something `Raw` can
/// paper over by "just adding".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Raw(i128);

impl Raw {
    pub const ZERO: Raw = Raw(0);

    pub const fn new(v: i128) -> Self {
        Raw(v)
    }

    pub const fn get(self) -> i128 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// A *computed* strictly positive amount (not "missing").
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// A *computed* strictly negative amount — a real loss, not "missing".
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn is_negative_or_zero(self) -> bool {
        self.0 <= 0
    }

    pub fn checked_add(self, other: Raw) -> Option<Raw> {
        self.0.checked_add(other.0).map(Raw)
    }

    pub fn checked_sub(self, other: Raw) -> Option<Raw> {
        self.0.checked_sub(other.0).map(Raw)
    }

    /// `self * k` with controlled overflow.
    pub fn checked_mul_int(self, k: i128) -> Option<Raw> {
        self.0.checked_mul(k).map(Raw)
    }

    /// `floor(self * num / den)` with a checked intermediate product.
    ///
    /// `None` when `den == 0` or the product overflows `i128` — the caller must
    /// treat that as an *unresolved* amount, never as zero.
    pub fn mul_div_floor(self, num: i128, den: i128) -> Option<Raw> {
        if den == 0 {
            return None;
        }
        let product = self.0.checked_mul(num)?;
        Some(Raw(product.div_euclid(den)))
    }

    /// `floor(self / den)` with `den == 0` rejected.
    pub fn div_floor(self, den: i128) -> Option<Raw> {
        if den == 0 {
            return None;
        }
        Some(Raw(self.0.div_euclid(den)))
    }

    pub fn max(self, other: Raw) -> Raw {
        if self.0 >= other.0 {
            self
        } else {
            other
        }
    }

    pub fn min(self, other: Raw) -> Raw {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }
}

impl fmt::Display for Raw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Basis points (`1 bps = 1/10_000`), bounded to `[0, 10_000]`.
///
/// `Default` is zero bps — the free case — so a cost model built with
/// `..Default::default()` never invents a fee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Bps(u32);

impl Bps {
    pub const ZERO: Bps = Bps(0);
    /// 100 % — the only value that zeroes an amount out.
    pub const FULL: Bps = Bps(10_000);

    /// `None` for anything above 10 000 bps: a 120 % fee is not a fee.
    pub const fn new(v: u32) -> Option<Self> {
        if v <= 10_000 {
            Some(Bps(v))
        } else {
            None
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    /// Amount retained after this fee: `floor(amount * (10_000 − bps) / 10_000)`.
    pub fn apply_floor(self, amount: Raw) -> Option<Raw> {
        amount.mul_div_floor(BPS_DENOM - self.0 as i128, BPS_DENOM)
    }

    /// This fee expressed as an amount: `floor(amount * bps / 10_000)`.
    pub fn of_floor(self, amount: Raw) -> Option<Raw> {
        amount.mul_div_floor(self.0 as i128, BPS_DENOM)
    }
}

impl fmt::Display for Bps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}bps", self.0)
    }
}

/// Decimal precision of a token, carried explicitly instead of implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Decimals(u8);

impl Decimals {
    /// Token decimals are `0..=255`; the EVM's practical ceiling is 18 but the
    /// type does not hardcode a protocol constant.
    pub const fn new(d: u8) -> Self {
        Decimals(d)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Render `Raw` for **display only** using integer arithmetic.
///
/// This is the single sanctioned direction of conversion: engine → human. The
/// inverse (parsing a decimal string back into `Raw`) is deliberately absent
/// from the engine, because PROMPT §4 forbids display values returning to the
/// engine as economic data.
pub fn display_units(amount: Raw, decimals: Decimals) -> String {
    let d = decimals.get() as u32;
    let negative = amount.is_negative();
    let magnitude = amount.get().unsigned_abs();
    if d == 0 {
        return format!("{}{}", if negative { "-" } else { "" }, magnitude);
    }
    let scale = 10u128.pow(d);
    let whole = magnitude / scale;
    let frac = magnitude % scale;
    format!(
        "{}{}.{:0width$}",
        if negative { "-" } else { "" },
        whole,
        frac,
        width = d as usize
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn fee_application_floors_toward_the_pool() {
        // 3 bps of 1 unit cannot pay the pool a whole unit: floor keeps 1 for
        // the trader? No — it floors the *retained* amount, so a 1-unit trade
        // through a 3 bps fee yields floor(1 * 9997 / 10000) = 0.
        let fee = Bps::new(3).unwrap();
        assert_eq!(fee.apply_floor(Raw::new(1)).unwrap(), Raw::ZERO);
        assert_eq!(fee.apply_floor(Raw::new(10_000)).unwrap(), Raw::new(9997));
        assert_eq!(fee.apply_floor(Raw::new(20_000)).unwrap(), Raw::new(19_994));
    }

    #[test]
    fn overhead_is_none_never_a_wrapped_or_zeroed_value() {
        let huge = Raw::new(i128::MAX);
        assert!(huge.checked_mul_int(2).is_none());
        assert!(huge.mul_div_floor(10_000, 1).is_none());
        assert!(Raw::new(5).mul_div_floor(3, 0).is_none());
        assert!(Raw::new(5).div_floor(0).is_none());
    }

    #[test]
    fn three_states_are_distinct() {
        let zero = Raw::ZERO;
        let loss = Raw::new(-7);
        assert!(zero.is_zero() && !zero.is_negative() && !zero.is_positive());
        assert!(loss.is_negative() && !loss.is_zero());
        assert!(Raw::new(7).is_positive());
    }

    #[test]
    fn bps_is_bounded_and_display_is_integer_only() {
        assert_eq!(Bps::new(10_001), None);
        assert_eq!(Bps::new(10_000).unwrap(), Bps::FULL);
        assert_eq!(Bps::FULL.apply_floor(Raw::new(999)).unwrap(), Raw::ZERO);
        assert_eq!(display_units(Raw::new(1_234_567), Decimals::new(6)), "1.234567");
        assert_eq!(display_units(Raw::new(-1_500_000), Decimals::new(6)), "-1.500000");
        assert_eq!(display_units(Raw::new(42), Decimals::new(0)), "42");
    }
}
