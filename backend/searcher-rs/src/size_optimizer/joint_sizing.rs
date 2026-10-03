// JOINT-SIZING-01 (§6) — joint optimization of route, size and financing.
//
// Prompt §6: "Optimiza conjuntamente ruta, cantidad y financiación. Determina
// el dominio de tamaño con balances, disponibilidad de préstamo, liquidez,
// límites y capacidad ejecutable. Utiliza una exploración inicial multiescala y
// refina intervalos prometedores, fronteras, saltos de ticks/bins y
// discontinuidades de fees. Redondea a unidades mínimas y vuelve a cotizar. No
// presupongas unimodalidad ni uses búsqueda ternaria o Brent como prueba de
// óptimo global sin sus hipótesis."
//
// This module is the SIZING MOTOR of that sentence. It is deliberately
// self-contained (no RPC of its own, no orchestrator wiring): every external
// fact — a size limit, a quote, a provider's premium, a pool's reserves —
// enters through an explicit, injectable seam, so the same code runs against
// on-chain QuoterV2 (production), the local CPMM kernel, or a test closure.
//
// ── What each prompt clause maps to, in code ────────────────────────────────
//
//  §6 "dominio de tamaño"  → [`SizeLimit`] / [`SizeDomain`]. Every bound carries
//                            its PROVENANCE. A bound that could not be resolved
//                            is [`Bound::Absent`] with its reason — never 0,
//                            never an invented range. A domain with NO known
//                            upper bound is refused as a whole
//                            ([`DomainReject::NoUpperBound`]): searching an
//                            unbounded range is exactly the invented range the
//                            prompt forbids.
//
//  §6 "exploración multiescala + refinamiento de intervalos, fronteras,
//      discontinuidades"   → [`multiscale_search`]. A log-spaced coarse grid,
//                            then recursive refinement of the best-bounded
//                            intervals, plus EXPLICIT boundary probes for
//                            tick/bin edges. Refinement never subdivides an
//                            interval whose endpoints are not both computed: a
//                            quote gap is a discontinuity, not a smooth valley.
//
//  §6 "no presupongas unimodalidad" → the search reports
//                            [`SearchReport::local_maxima`] and
//                            [`SearchReport::unimodal_observed`] but never
//                            relies on them; the optional vertex candidate is
//                            only ever used as ONE candidate among others, and
//                            is re-quoted like every other. The test module
//                            pins the hypothesis-violating case that breaks
//                            golden-section (see `golden_section_misses_...`).
//
//  §6/§4 "redondea a unidades mínimas y vuelve a cotizar" → [`RoundingRule`] /
//                            [`finalize_rounded`]. The published figure is the
//                            RE-QUOTE's figure at the ROUNDED size. If the
//                            rounded size cannot be quoted, the published
//                            result is ABSENT — the ideal's number is never
//                            substituted for it.
//
//  §4 "montos como enteros de unidades mínimas … PROHIBIDO f64 en importes
//      monetarios"          → amounts are `U256` base units; USD is
//                            `rust_decimal::Decimal` with checked arithmetic.
//                            The ONLY f64 in this module is the dimensionless
//                            grid ratio of [`geometric_grid_exact`], which
//                            never carries a monetary value.
//
//  §6/§5 "compara capital propio y proveedores admisibles (principal, premium,
//      repayment exactos)"  → [`compare_financing`]. A premium that could not
//                            be read is ABSENT (with a concrete resolution
//                            task), a credited zero stays a credited zero, and
//                            the borrowed principal is structurally NOT income:
//                            net = proceeds − repayment − costs.
//
//  §6 "split routes y pools compartidos" → [`SharedPoolSplit::evaluate`]
//                            composes the paths SEQUENTIALLY over one mutable
//                            pool state and publishes the JOINT output; the
//                            independent-sum figure is reported alongside as
//                            the error that summing would have made.
//
//  §6 "ambas orientaciones económicas … protocolos mixtos" → [`RouteVariant`]
//                            carries the orientation and the change vs the
//                            baseline; [`optimize_joint`] searches all variants
//                            and reports which one won, plus the best
//                            diagnostic (even when negative), the best feasible
//                            and the best over target as SEPARATE states.
//
// ── States (prompt §15) ─────────────────────────────────────────────────────
//
// SPECIFIED   — this file's doc block + the typed contracts below.
// IMPLEMENTED — the motor is complete and total: no `todo!`, no stub branch.
// TESTED      — `size_optimizer::joint_sizing::tests` (offline, deterministic).
// LIVE_DATA_VERIFIED — ONE test (`real_quoter_v2_tick_crossing_...`) quotes the
//               real mainnet QuoterV2 through the repo's aggregate3 path; it is
//               `#[ignore]`d because CI has no RPC, and its result is reported
//               by the task that ran it, or not at all.
// NOT claimed: EXECUTION_AUTHORIZED / SETTLED / RECONCILED — nothing here
// signs, sends or settles; a quote is not a realized gain (§4).
//
// NOT WIRED YET: no orchestrator / cartridge / worker row consumes this module.
// [`optimize_joint`] is the entry point and takes its pricing through the
// [`CycleQuoter`] seam, so wiring it is a matter of implementing that trait over
// `StateProjector` + the existing kernels at the call site — which lives in
// files this task must not edit (`orchestrator.rs`, `scanner.rs`,
// `cartridge_boot.rs`). Integration is therefore an explicitly PENDING item,
// reported as such rather than half-done here.
//
// That pending wiring is also why this module carries `#![allow(dead_code)]`
// (the repo's per-module allowance convention): `main.rs` declares
// `mod size_optimizer` privately, so until a call site consumes
// `optimize_joint` every item here is unreachable FROM THE BINARY and the
// workspace `clippy -- -D warnings` gate would fail on it. Remove this line in
// the same change that wires the entry point.
#![allow(dead_code)]

use ethers::types::U256;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;

use crate::amm_math::{mul_div, v2_amount_out};

// The parent module's two lossy helpers are used for the DIMENSIONLESS grid
// ratio only (see `geometric_grid_exact`). Re-using them keeps ONE grid-math
// implementation in the file instead of two that can drift apart.
use super::{f64_to_u256_clamped, u256_to_f64_lossy};

/// Denominator of the rational that carries a *continuous* (unrounded) optimum.
/// 10^9 base units is far finer than any token's minimum unit, so the rounding
/// step — not this constant — is what decides the representable size.
pub const IDEAL_PRECISION_DEN: u64 = 1_000_000_000;

/// The basis string every search report carries. It is deliberately verbose:
/// a reader must be able to tell "best found in the evaluated scope" from
/// "global optimum", which is the distinction prompt §6 demands.
pub const JOINT_SEARCH_BASIS: &str = "multiscale coarse grid + interval refinement + boundary probes; \
     publishes best-found-in-evaluated-scope; no unimodality assumed; no ternary/Brent global claim";

// ===========================================================================
// 1. Money: denominations, exact decimal conversion, signed amounts
// ===========================================================================

/// Largest `decimals` accepted: `10^decimals` and the USD conversions below
/// must survive the `Decimal` (96-bit mantissa, 28–29 digits) round-trip.
pub const MAX_TOKEN_DECIMALS: u8 = 28;

/// Digits a `Decimal` holds without loss. An amount with more digits is
/// ABSENT (never silently truncated, never rounded behind the caller's back).
const DECIMAL_DIGITS: usize = 28;

/// A token's minimum-unit denomination.
///
/// `decimals` is a property of the TOKEN, not of the price: it fixes how many
/// base units one whole token is. Every amount in this module is an integer
/// count of base units; conversion to USD goes through `Decimal` and is
/// checked at every step (prompt §4: "Dinero y precios deben conservar
/// precisión decimal/racional explícita").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenDenomination {
    decimals: u8,
}

impl TokenDenomination {
    /// `None` outside `[0, MAX_TOKEN_DECIMALS]` — an out-of-range
    /// denomination would silently overflow the Decimal arithmetic.
    pub fn new(decimals: u8) -> Option<Self> {
        if decimals > MAX_TOKEN_DECIMALS {
            None
        } else {
            Some(Self { decimals })
        }
    }

    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Base units in one whole token (`10^decimals`).
    pub fn scale(&self) -> U256 {
        U256::from(10u64).pow(U256::from(self.decimals as u64))
    }

    /// USD → base units, FLOORED (never over-states an affordable amount).
    ///
    /// Returns `None` when the price is non-positive, the amount is negative,
    /// or the intermediate value would leave `Decimal`'s exact range — i.e. the
    /// honest ABSENT instead of a wrong number.
    pub fn usd_to_base_units(&self, usd: Decimal, price_usd: Decimal) -> Option<U256> {
        if price_usd <= Decimal::ZERO || usd < Decimal::ZERO {
            return None;
        }
        let tokens = usd.checked_div(price_usd)?;
        let units = tokens.checked_mul(decimal_pow10(self.decimals)?)?;
        decimal_to_u256(units.trunc())
    }

    /// Base units → USD, exact within `Decimal` (prompt §4). `None` when the
    /// amount carries more digits than `Decimal` can hold exactly.
    pub fn base_units_to_usd(&self, amount: U256, price_usd: Decimal) -> Option<Decimal> {
        if price_usd < Decimal::ZERO {
            return None;
        }
        let amount_dec = u256_to_decimal(amount)?;
        let scale = decimal_pow10(self.decimals)?;
        amount_dec.checked_div(scale)?.checked_mul(price_usd)
    }
}

/// `10^n` as an exact `Decimal` for `n <= MAX_TOKEN_DECIMALS`.
fn decimal_pow10(n: u8) -> Option<Decimal> {
    if n > MAX_TOKEN_DECIMALS {
        return None;
    }
    let mut acc = Decimal::ONE;
    for _ in 0..n {
        acc = acc.checked_mul(Decimal::from(10u32))?;
    }
    Some(acc)
}

/// `U256` → `Decimal` only when the value fits the mantissa EXACTLY.
fn u256_to_decimal(v: U256) -> Option<Decimal> {
    let s = v.to_string();
    if s.len() > DECIMAL_DIGITS {
        return None;
    }
    Decimal::from_str(&s).ok()
}

/// Integral `Decimal` → `U256`. Non-integral or negative input is refused
/// (the caller decides the rounding direction, this helper never guesses).
fn decimal_to_u256(d: Decimal) -> Option<U256> {
    if d.is_sign_negative() {
        return None;
    }
    let truncated = d.trunc();
    U256::from_dec_str(&truncated.to_string()).ok()
}

/// An amount with a sign, kept in base units. Used for deltas that can go
/// either way (rounding drift, cycle surplus) without pulling `f64` in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedAmount {
    pub negative: bool,
    pub magnitude: U256,
}

impl SignedAmount {
    pub fn positive(magnitude: U256) -> Self {
        Self {
            negative: false,
            magnitude,
        }
    }

    pub fn zero() -> Self {
        Self::positive(U256::zero())
    }

    /// `after − before`, exactly.
    pub fn from_delta(before: U256, after: U256) -> Self {
        if after >= before {
            Self::positive(after - before)
        } else {
            Self {
                negative: true,
                magnitude: before - after,
            }
        }
    }

    pub fn is_zero(&self) -> bool {
        self.magnitude.is_zero()
    }

    /// Signed USD value. `None` when the amount leaves `Decimal`'s exact range.
    pub fn to_signed_usd(
        &self,
        denomination: TokenDenomination,
        price_usd: Decimal,
    ) -> Option<Decimal> {
        let usd = denomination.base_units_to_usd(self.magnitude, price_usd)?;
        if self.negative && !usd.is_zero() {
            Some(-usd)
        } else {
            Some(usd)
        }
    }
}

// ===========================================================================
// 2. The honest size domain (prompt §6)
// ===========================================================================

/// The provenance of ONE bound.
///
/// `Known { value: 0 }` is a CREDITED zero (the source really answered "zero");
/// `Absent` means the bound was not resolved. They are different states and the
/// type keeps them apart (prompt §4/§7, R8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    Known { value: U256, source: String },
    Absent { reason: String },
}

impl Bound {
    pub fn known(value: U256, source: impl Into<String>) -> Self {
        Self::Known {
            value,
            source: source.into(),
        }
    }

    pub fn absent(reason: impl Into<String>) -> Self {
        Self::Absent {
            reason: reason.into(),
        }
    }

    pub fn value(&self) -> Option<U256> {
        match self {
            Self::Known { value, .. } => Some(*value),
            Self::Absent { .. } => None,
        }
    }

    pub fn source(&self) -> Option<&str> {
        match self {
            Self::Known { source, .. } => Some(source.as_str()),
            Self::Absent { .. } => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Absent { reason } => Some(reason.as_str()),
            Self::Known { .. } => None,
        }
    }
}

/// Which kind of fact produced an upper bound. The list is the prompt's own:
/// "balances, disponibilidad de préstamo, liquidez, límites y capacidad
/// ejecutable".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SizeLimitKind {
    /// Balance the engine actually holds for the route's input token.
    OwnBalance,
    /// Principal a financing provider can make available for the asset.
    BorrowCapacity,
    /// Depth of a leg's in-reserve: beyond it the price has run away.
    PoolLiquidity,
    /// A protocol-level ceiling (max swap, tick-range edge, vault cap).
    ProtocolLimit,
    /// What the executor can actually move atomically (gas/calldata bound).
    ExecutableCapacity,
    /// Size at which the configured slippage budget is exhausted.
    SlippageBudget,
    /// Operator-authorized capital for this strategy/route.
    AuthorizedCapital,
    /// A venue-imposed lot/tick grid expressed as a maximum admissible size.
    LotGrid,
}

impl SizeLimitKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OwnBalance => "own_balance",
            Self::BorrowCapacity => "borrow_capacity",
            Self::PoolLiquidity => "pool_liquidity",
            Self::ProtocolLimit => "protocol_limit",
            Self::ExecutableCapacity => "executable_capacity",
            Self::SlippageBudget => "slippage_budget",
            Self::AuthorizedCapital => "authorized_capital",
            Self::LotGrid => "lot_grid",
        }
    }
}

/// One candidate upper bound plus where it came from and what it scopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeLimit {
    pub kind: SizeLimitKind,
    /// The leg/pool/asset this bound applies to (free-form but explicit).
    pub scope: String,
    pub bound: Bound,
}

impl SizeLimit {
    pub fn new(kind: SizeLimitKind, scope: impl Into<String>, bound: Bound) -> Self {
        Self {
            kind,
            scope: scope.into(),
            bound,
        }
    }

    pub fn known(
        kind: SizeLimitKind,
        scope: impl Into<String>,
        value: U256,
        source: impl Into<String>,
    ) -> Self {
        Self::new(kind, scope, Bound::known(value, source))
    }

    pub fn absent(
        kind: SizeLimitKind,
        scope: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::new(kind, scope, Bound::absent(reason))
    }
}

/// Why a domain could not be established. Each variant is a DIFFERENT reason to
/// stop, so a metric can tell them apart instead of counting one bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainReject {
    /// The lower bound itself was zero — no searchable size exists.
    ZeroLower,
    /// Not one upper bound was resolved. Searching an unbounded range would be
    /// an invented range (prompt §6), so the domain is refused and every
    /// unresolved limit is carried as evidence.
    NoUpperBound {
        absent: Vec<(SizeLimitKind, String)>,
    },
    /// Every known bound sits below the minimum tradable size.
    UpperBelowLower {
        lower: U256,
        upper: U256,
        binding: SizeLimitKind,
    },
}

impl DomainReject {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZeroLower => "domain_zero_lower",
            Self::NoUpperBound { .. } => "domain_no_upper_bound",
            Self::UpperBelowLower { .. } => "domain_upper_below_lower",
        }
    }

    /// Human-readable detail, including the unresolved limits verbatim.
    pub fn detail(&self) -> String {
        match self {
            Self::ZeroLower => "lower bound is zero".to_string(),
            Self::NoUpperBound { absent } => {
                let parts: Vec<String> = absent
                    .iter()
                    .map(|(k, r)| format!("{}={}", k.as_str(), r))
                    .collect();
                format!("no upper bound resolved; absent: [{}]", parts.join(", "))
            }
            Self::UpperBelowLower {
                lower,
                upper,
                binding,
            } => format!(
                "upper {upper} (binding {}) is below lower {lower}",
                binding.as_str()
            ),
        }
    }
}

/// The feasible size range, with the provenance of every input to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeDomain {
    lower: U256,
    upper: U256,
    binding: SizeLimitKind,
    limits: Vec<SizeLimit>,
}

impl SizeDomain {
    /// Resolve `[lower, upper]` from the candidate limits.
    ///
    /// `upper` is the MINIMUM of the KNOWN bounds — the binding constraint.
    /// Absent bounds do not constrain, but they are kept in `limits` so a card
    /// can state what was NOT known rather than implying it was fine.
    pub fn resolve(limits: Vec<SizeLimit>, lower: U256) -> Result<Self, DomainReject> {
        if lower.is_zero() {
            return Err(DomainReject::ZeroLower);
        }
        let mut best: Option<(SizeLimitKind, U256)> = None;
        for limit in &limits {
            if let Bound::Known { value, .. } = &limit.bound {
                let better = match best {
                    Some((_, current)) => *value < current,
                    None => true,
                };
                if better {
                    best = Some((limit.kind, *value));
                }
            }
        }
        let Some((binding, upper)) = best else {
            return Err(DomainReject::NoUpperBound {
                absent: limits
                    .iter()
                    .filter_map(|l| match &l.bound {
                        Bound::Absent { reason } => Some((l.kind, reason.clone())),
                        Bound::Known { .. } => None,
                    })
                    .collect(),
            });
        };
        if upper < lower {
            return Err(DomainReject::UpperBelowLower {
                lower,
                upper,
                binding,
            });
        }
        Ok(Self {
            lower,
            upper,
            binding,
            limits,
        })
    }

    pub fn lower(&self) -> U256 {
        self.lower
    }

    pub fn upper(&self) -> U256 {
        self.upper
    }

    /// The constraint that actually decided `upper`.
    pub fn binding(&self) -> SizeLimitKind {
        self.binding
    }

    pub fn limits(&self) -> &[SizeLimit] {
        &self.limits
    }

    /// Every limit that could NOT be resolved, with its reason (R8: reported,
    /// never treated as zero and never treated as "no constraint").
    pub fn absent_limits(&self) -> Vec<(SizeLimitKind, &str, &str)> {
        self.limits
            .iter()
            .filter_map(|l| match &l.bound {
                Bound::Absent { reason } => Some((l.kind, l.scope.as_str(), reason.as_str())),
                Bound::Known { .. } => None,
            })
            .collect()
    }

    pub fn describe(&self) -> String {
        format!(
            "size_domain[{},{}] binding={} known={} absent={}",
            self.lower,
            self.upper,
            self.binding.as_str(),
            self.limits
                .iter()
                .filter(|l| matches!(l.bound, Bound::Known { .. }))
                .count(),
            self.absent_limits().len()
        )
    }
}

// ===========================================================================
// 3. Rounding to minimum units (prompt §6/§4)
// ===========================================================================

/// An exact non-negative rational `num/den` — the shape a CONTINUOUS search
/// returns before the amount is forced onto a representable grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalAmount {
    pub num: U256,
    pub den: U256,
}

impl RationalAmount {
    pub fn new(num: U256, den: U256) -> Option<Self> {
        if den.is_zero() {
            None
        } else {
            Some(Self { num, den })
        }
    }

    /// An amount that is already an exact integer of base units.
    pub fn integer(value: U256) -> Self {
        Self {
            num: value,
            den: U256::one(),
        }
    }

    /// `floor(num/den)`.
    pub fn floor(&self) -> Option<U256> {
        if self.den.is_zero() {
            None
        } else {
            Some(self.num / self.den)
        }
    }

    /// `ceil(num/den)`.
    pub fn ceil(&self) -> Option<U256> {
        if self.den.is_zero() {
            return None;
        }
        let quotient = self.num / self.den;
        let remainder = self.num % self.den;
        if remainder.is_zero() {
            Some(quotient)
        } else {
            quotient.checked_add(U256::one())
        }
    }

    /// Build `d` as a rational with denominator `den` WITHOUT going through
    /// `d * den` in `Decimal` space (which would overflow for a 24-digit size).
    /// The integer and fractional parts are converted separately.
    pub fn from_decimal_split(d: Decimal, den: u64) -> Option<Self> {
        if den == 0 || d.is_sign_negative() {
            return None;
        }
        let den_u = U256::from(den);
        // `den` goes through the same digit guard as every other conversion, so
        // an out-of-range denominator is ABSENT instead of silently truncated.
        let den_dec = u256_to_decimal(den_u)?;
        let integer = d.trunc();
        let fraction = d.checked_sub(integer)?;
        let frac_scaled = decimal_to_u256(fraction.checked_mul(den_dec)?.trunc())?;
        let int_scaled = decimal_to_u256(integer)?.checked_mul(den_u)?;
        Some(Self {
            num: int_scaled.checked_add(frac_scaled)?,
            den: den_u,
        })
    }
}

/// The grid an amount must land on to be representable on-chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingRule {
    /// The token's own minimum unit: any integer count of base units.
    MinUnit,
    /// A protocol-mandated coarser grid (vault shares, lot size, tick-sized
    /// amount): only multiples of `granularity` base units are representable.
    MultipleOf(U256),
}

impl RoundingRule {
    pub fn granularity(&self) -> U256 {
        match self {
            Self::MinUnit => U256::one(),
            Self::MultipleOf(g) => *g,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::MinUnit => "min_unit".to_string(),
            Self::MultipleOf(g) => format!("multiple_of({g})"),
        }
    }
}

/// Which way the amount is pushed. `Down` is conservative for an INPUT (never
/// commit more than authorized); `Up` is conservative for a DEBT (never
/// under-repay). Both are explicit — the module never picks silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingDirection {
    Down,
    Up,
}

impl RoundingDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
        }
    }
}

/// A rounded size plus everything needed to audit the decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundedSize {
    pub ideal: RationalAmount,
    pub rounded_wei: U256,
    pub rule: RoundingRule,
    /// The direction the caller asked for.
    pub requested_direction: RoundingDirection,
    /// The direction actually applied. Differs from the requested one only when
    /// an upward rounding would have crossed the domain's upper bound — in
    /// which case the conservative floor is used and
    /// `adjust_reason` says why.
    pub applied_direction: RoundingDirection,
    pub adjust_reason: Option<&'static str>,
    /// `rounded − floor(ideal)`: the drift the rounding introduced, signed.
    pub drift: SignedAmount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundingError {
    ZeroDenominator,
    ZeroGranularity,
    Overflow,
    /// The rounded amount fell below the domain's lower bound.
    BelowDomain {
        rounded: U256,
        lower: U256,
    },
    /// Even the conservative floor exceeded the domain's upper bound.
    AboveDomain {
        rounded: U256,
        upper: U256,
    },
}

impl RoundingError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZeroDenominator => "rounding_zero_denominator",
            Self::ZeroGranularity => "rounding_zero_granularity",
            Self::Overflow => "rounding_overflow",
            Self::BelowDomain { .. } => "rounding_below_domain",
            Self::AboveDomain { .. } => "rounding_above_domain",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::ZeroDenominator => "ideal rational has a zero denominator".to_string(),
            Self::ZeroGranularity => "rounding rule has a zero granularity".to_string(),
            Self::Overflow => "rounding arithmetic overflowed".to_string(),
            Self::BelowDomain { rounded, lower } => {
                format!("rounded {rounded} is below domain lower {lower}")
            }
            Self::AboveDomain { rounded, upper } => {
                format!("rounded {rounded} is above domain upper {upper}")
            }
        }
    }
}

/// Round `ideal` onto `rule`'s grid in the requested direction (overflow
/// checked, no `f64`).
pub fn round_rational(
    ideal: RationalAmount,
    rule: RoundingRule,
    direction: RoundingDirection,
) -> Result<U256, RoundingError> {
    if ideal.den.is_zero() {
        return Err(RoundingError::ZeroDenominator);
    }
    let granularity = rule.granularity();
    if granularity.is_zero() {
        return Err(RoundingError::ZeroGranularity);
    }
    let quantum = match direction {
        RoundingDirection::Down => ideal.floor().ok_or(RoundingError::ZeroDenominator)?,
        RoundingDirection::Up => ideal.ceil().ok_or(RoundingError::Overflow)?,
    };
    match direction {
        RoundingDirection::Down => (quantum / granularity)
            .checked_mul(granularity)
            .ok_or(RoundingError::Overflow),
        RoundingDirection::Up => {
            let slack = granularity
                .checked_sub(U256::one())
                .ok_or(RoundingError::ZeroGranularity)?;
            let blocks = quantum
                .checked_add(slack)
                .ok_or(RoundingError::Overflow)?
                .checked_div(granularity)
                .ok_or(RoundingError::Overflow)?;
            blocks
                .checked_mul(granularity)
                .ok_or(RoundingError::Overflow)
        }
    }
}

/// Round AND enforce the domain. An upward rounding that would exceed the
/// authorized upper bound is replaced by the conservative floor (recorded in
/// `adjust_reason`); a result outside the domain is an error, never a silently
/// clamped size.
pub fn round_within_domain(
    ideal: RationalAmount,
    rule: RoundingRule,
    direction: RoundingDirection,
    domain: &SizeDomain,
) -> Result<RoundedSize, RoundingError> {
    let mut applied = direction;
    let mut adjust_reason = None;
    let mut rounded = round_rational(ideal, rule, direction)?;

    if rounded > domain.upper() {
        if direction == RoundingDirection::Up {
            rounded = round_rational(ideal, rule, RoundingDirection::Down)?;
            applied = RoundingDirection::Down;
            adjust_reason = Some("up_rounding_exceeded_domain_upper_floor_used");
        }
        if rounded > domain.upper() {
            return Err(RoundingError::AboveDomain {
                rounded,
                upper: domain.upper(),
            });
        }
    }
    if rounded < domain.lower() {
        return Err(RoundingError::BelowDomain {
            rounded,
            lower: domain.lower(),
        });
    }

    let floor = ideal.floor().ok_or(RoundingError::ZeroDenominator)?;
    Ok(RoundedSize {
        ideal,
        rounded_wei: rounded,
        rule,
        requested_direction: direction,
        applied_direction: applied,
        adjust_reason,
        drift: SignedAmount::from_delta(floor, rounded),
    })
}

// ===========================================================================
// 4. Quotes, samples and the probe book
// ===========================================================================

/// One exact-size answer from a protocol quoter.
///
/// `amount_in_wei` repeats the size that was ASKED FOR: a quoter that answers
/// about a different size is rejected by [`Sample::from_quote`] instead of
/// having its number attributed to the wrong amount.
#[derive(Debug, Clone, PartialEq)]
pub struct CycleQuote {
    pub amount_in_wei: U256,
    pub amount_out_wei: U256,
    /// `value(amount_out) − value(amount_in)` — the post-swap gross of prompt
    /// §4. It is a QUOTE, not a realized gain.
    pub gross_usd: Decimal,
    /// Costs NOT already inside `gross_usd` (gas, ops, financing premium).
    /// A cost already embedded in `amount_out` must not be listed here again.
    pub external_costs_usd: Decimal,
    pub net_usd: Decimal,
    /// Which quoter produced it (adapter + block/pool identity).
    pub provenance: String,
}

impl CycleQuote {
    pub fn new(
        amount_in_wei: U256,
        amount_out_wei: U256,
        gross_usd: Decimal,
        external_costs_usd: Decimal,
        provenance: impl Into<String>,
    ) -> Self {
        // `gross − costs` is computed once, here, so every consumer reads the
        // same net. A costs figure that cannot be represented keeps the
        // subtraction saturating at the Decimal range rather than panicking.
        let net_usd = gross_usd
            .checked_sub(external_costs_usd)
            .unwrap_or(Decimal::MIN);
        Self {
            amount_in_wei,
            amount_out_wei,
            gross_usd,
            external_costs_usd,
            net_usd,
            provenance: provenance.into(),
        }
    }
}

/// A quoter's answer for one exact size.
#[derive(Debug, Clone, PartialEq)]
pub enum QuoteOutcome {
    Quoted(CycleQuote),
    /// No quote exists at this size — and the reason is carried, because
    /// "not computed" is not "computed zero" (prompt §4).
    Absent {
        reason: String,
    },
}

/// The seam every external pricing source plugs into.
pub trait CycleQuoter {
    fn quote_cycle(&mut self, amount_in_wei: U256) -> QuoteOutcome;

    /// Joint-vs-independent evidence for a route that SPLITS across pools.
    /// `None` for single-path quoters (R8: nothing is invented for them).
    fn split_evidence(&self) -> Option<(U256, U256)> {
        None
    }
}

/// A closure-backed quoter: the test/offline seam, and the shape a future
/// adapter over `StateProjector` + the kernels will produce.
pub struct FnQuoter<F>
where
    F: FnMut(U256) -> QuoteOutcome,
{
    f: F,
}

impl<F> FnQuoter<F>
where
    F: FnMut(U256) -> QuoteOutcome,
{
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F> CycleQuoter for FnQuoter<F>
where
    F: FnMut(U256) -> QuoteOutcome,
{
    fn quote_cycle(&mut self, amount_in_wei: U256) -> QuoteOutcome {
        (self.f)(amount_in_wei)
    }
}

/// A normalized observation at one size.
#[derive(Debug, Clone, PartialEq)]
pub enum Sample {
    Value {
        net_usd: Decimal,
        gross_usd: Decimal,
        costs_usd: Decimal,
        amount_out_wei: U256,
        provenance: String,
    },
    Absent {
        reason: String,
    },
}

impl Sample {
    /// Normalize a quoter answer, enforcing the size contract.
    pub fn from_quote(requested: U256, outcome: QuoteOutcome) -> Self {
        match outcome {
            QuoteOutcome::Absent { reason } => Self::Absent { reason },
            QuoteOutcome::Quoted(q) => {
                if q.amount_in_wei != requested {
                    return Self::Absent {
                        reason: format!(
                            "quoter_answered_foreign_size:{}!={}",
                            q.amount_in_wei, requested
                        ),
                    };
                }
                Self::Value {
                    net_usd: q.net_usd,
                    gross_usd: q.gross_usd,
                    costs_usd: q.external_costs_usd,
                    amount_out_wei: q.amount_out_wei,
                    provenance: q.provenance,
                }
            }
        }
    }

    pub fn net_usd(&self) -> Option<Decimal> {
        match self {
            Self::Value { net_usd, .. } => Some(*net_usd),
            Self::Absent { .. } => None,
        }
    }

    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value { .. })
    }

    pub fn absent_reason(&self) -> Option<&str> {
        match self {
            Self::Absent { reason } => Some(reason.as_str()),
            Self::Value { .. } => None,
        }
    }
}

/// Which part of the search produced a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStage {
    Coarse,
    Boundary,
    Refine,
    Requote,
}

impl ProbeStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Coarse => "coarse",
            Self::Boundary => "boundary",
            Self::Refine => "refine",
            Self::Requote => "requote",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SizeProbe {
    pub amount_in_wei: U256,
    pub sample: Sample,
    pub stage: ProbeStage,
}

/// Every size this pass evaluated, deduplicated by size.
///
/// Sharing a quote between consumers is explicitly allowed (prompt §7:
/// "Compartir cotizaciones compatibles es válido"): the same size is never
/// quoted twice unless `force` is set — which is what the RE-QUOTE does, because
/// a re-quote is a new observation and must not be answered from the cache.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProbeBook {
    probes: BTreeMap<U256, SizeProbe>,
    evaluations: usize,
}

impl ProbeBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an evaluation. Returns `true` when the evaluator actually ran.
    pub fn record(
        &mut self,
        amount_in_wei: U256,
        sample: Sample,
        stage: ProbeStage,
        force: bool,
    ) -> bool {
        if !force {
            if let Some(existing) = self.probes.get(&amount_in_wei) {
                if existing.stage != ProbeStage::Requote {
                    return false;
                }
            }
        }
        self.evaluations += 1;
        self.probes.insert(
            amount_in_wei,
            SizeProbe {
                amount_in_wei,
                sample,
                stage,
            },
        );
        true
    }

    pub fn evaluations(&self) -> usize {
        self.evaluations
    }

    pub fn len(&self) -> usize {
        self.probes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }

    /// Ascending by size — the order every downstream fold relies on.
    pub fn probes(&self) -> Vec<SizeProbe> {
        self.probes.values().cloned().collect()
    }

    pub fn get(&self, amount_in_wei: &U256) -> Option<&SizeProbe> {
        self.probes.get(amount_in_wei)
    }
}

// ===========================================================================
// 5. Multiscale exploration + interval refinement (prompt §6)
// ===========================================================================

/// Budget and shape of one search pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiscaleConfig {
    /// Points of the initial log-spaced grid.
    pub coarse_points: usize,
    /// How many refinement rounds run after the coarse pass.
    pub refine_rounds: usize,
    /// New interior points per refined interval per round.
    pub refine_points: usize,
    /// How many of the best-bounded intervals are refined per round.
    pub refine_intervals: usize,
    /// Hard ceiling on evaluator calls for this pass.
    pub max_probes: usize,
}

impl Default for MultiscaleConfig {
    fn default() -> Self {
        Self {
            coarse_points: 16,
            refine_rounds: 4,
            refine_points: 4,
            refine_intervals: 2,
            max_probes: 96,
        }
    }
}

impl MultiscaleConfig {
    /// Fail-fast on a degenerate knob: a silently clamped budget would misreport
    /// the resolution the search actually achieved.
    pub fn validate(&self) -> Result<(), String> {
        if self.coarse_points < 2 {
            return Err(format!(
                "joint_sizing: coarse_points={} < 2",
                self.coarse_points
            ));
        }
        if self.refine_points < 2 {
            return Err(format!(
                "joint_sizing: refine_points={} < 2",
                self.refine_points
            ));
        }
        if self.refine_intervals == 0 {
            return Err("joint_sizing: refine_intervals=0".to_string());
        }
        if self.refine_rounds > 32 {
            return Err(format!(
                "joint_sizing: refine_rounds={} above the 32-round ceiling",
                self.refine_rounds
            ));
        }
        if self.max_probes < self.coarse_points {
            return Err(format!(
                "joint_sizing: max_probes={} below coarse_points={}",
                self.max_probes, self.coarse_points
            ));
        }
        Ok(())
    }
}

/// The outcome of one search pass over one route variant.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchReport {
    /// Ascending by size.
    pub probes: Vec<SizeProbe>,
    /// Argmax over the COMPUTED samples — may be negative. Kept on purpose:
    /// prompt §6 forbids dropping a negative net just because no winner exists.
    pub best: Option<SizeProbe>,
    /// Argmax over the strictly positive samples.
    pub best_positive: Option<SizeProbe>,
    /// Sizes that are a local maximum within their contiguous computed run.
    /// Computed runs are broken by ABSENT samples, so a local maximum may sit
    /// exactly on a discontinuity edge — which is where tick/bin jumps live.
    pub local_maxima: Vec<U256>,
    /// `local_maxima.len() <= 1`. REPORTED, never assumed, never used to
    /// justify a unimodal-only method.
    pub unimodal_observed: bool,
    pub absent_samples: usize,
    pub boundaries_probed: usize,
    pub evaluations: usize,
    pub max_probes: usize,
    pub budget_exhausted: bool,
    pub scope: String,
    pub basis: &'static str,
}

impl SearchReport {
    pub fn computed_samples(&self) -> usize {
        self.probes.len() - self.absent_samples
    }
}

/// Build a report from a probe book. Kept separate so a caller can re-derive
/// the report after extra probes (e.g. the re-quote) land in the same book.
pub fn report_from_book(
    book: &ProbeBook,
    scope: &str,
    boundaries_probed: usize,
    budget_exhausted: bool,
    max_probes: usize,
) -> SearchReport {
    let probes = book.probes();

    let mut best_idx: Option<usize> = None;
    let mut best_positive_idx: Option<usize> = None;
    for (i, probe) in probes.iter().enumerate() {
        let Some(net) = probe.sample.net_usd() else {
            continue;
        };
        // Strict `>` while walking ASCENDING sizes keeps the SMALLEST size on a
        // tie — deterministic, and never a larger notional for the same net.
        if best_idx
            .and_then(|j| probes[j].sample.net_usd())
            .map(|current| net > current)
            .unwrap_or(true)
        {
            best_idx = Some(i);
        }
        if net > Decimal::ZERO
            && best_positive_idx
                .and_then(|j| probes[j].sample.net_usd())
                .map(|current| net > current)
                .unwrap_or(true)
        {
            best_positive_idx = Some(i);
        }
    }

    // Local maxima over CONTIGUOUS computed runs. An ABSENT probe ends a run:
    // nothing is interpolated across a discontinuity.
    let mut local_maxima: Vec<U256> = Vec::new();
    let mut cursor = 0usize;
    while cursor < probes.len() {
        if !probes[cursor].sample.is_value() {
            cursor += 1;
            continue;
        }
        let start = cursor;
        let mut end = cursor;
        while end < probes.len() && probes[end].sample.is_value() {
            end += 1;
        }
        let run = &probes[start..end];
        for k in 0..run.len() {
            let value = run[k].sample.net_usd().unwrap_or(Decimal::ZERO);
            let left_ok = k == 0
                || run[k - 1]
                    .sample
                    .net_usd()
                    .map(|l| value >= l)
                    .unwrap_or(true);
            let right_ok = k + 1 == run.len()
                || run[k + 1]
                    .sample
                    .net_usd()
                    .map(|r| value >= r)
                    .unwrap_or(true);
            if left_ok && right_ok {
                local_maxima.push(run[k].amount_in_wei);
            }
        }
        cursor = end;
    }

    let absent_samples = probes.iter().filter(|p| !p.sample.is_value()).count();
    SearchReport {
        unimodal_observed: local_maxima.len() <= 1,
        best: best_idx.map(|i| probes[i].clone()),
        best_positive: best_positive_idx.map(|i| probes[i].clone()),
        probes,
        local_maxima,
        absent_samples,
        boundaries_probed,
        evaluations: book.evaluations(),
        max_probes,
        budget_exhausted,
        scope: scope.to_string(),
        basis: JOINT_SEARCH_BASIS,
    }
}

/// Multiscale search over `domain`, recording every probe in `book`.
///
/// Shape: a log-spaced coarse grid, the caller's tick/bin BOUNDARIES, then
/// `refine_rounds` rounds that subdivide the best-bounded intervals. Refinement
/// is scoring-driven, so it does not presuppose a single basin; and an interval
/// with an ABSENT endpoint is never subdivided — a quote gap is a
/// discontinuity, not a smooth valley.
pub fn multiscale_search<F>(
    domain: &SizeDomain,
    cfg: &MultiscaleConfig,
    boundaries_wei: &[U256],
    scope: &str,
    book: &mut ProbeBook,
    eval: &mut F,
) -> Result<SearchReport, String>
where
    F: FnMut(U256) -> Sample,
{
    cfg.validate()?;

    /// Evaluate `x` unless the book already holds it (sharing compatible quotes
    /// is explicitly allowed by prompt §7). Returns `true` when the evaluator
    /// actually ran, so a caller can tell "added" from "already known".
    fn probe_at<F>(
        x: U256,
        stage: ProbeStage,
        force: bool,
        book: &mut ProbeBook,
        evaluations: &mut usize,
        eval: &mut F,
    ) -> bool
    where
        F: FnMut(U256) -> Sample,
    {
        if !force {
            if let Some(existing) = book.get(&x) {
                if existing.stage != ProbeStage::Requote {
                    return false;
                }
            }
        }
        let sample = eval(x);
        *evaluations += 1;
        book.record(x, sample, stage, true);
        true
    }

    let lower = domain.lower();
    let upper = domain.upper();
    let budget = cfg.max_probes;
    let mut spent = 0usize;

    let coarse = geometric_grid_exact(lower, upper, cfg.coarse_points);
    for x in &coarse {
        if spent >= budget {
            break;
        }
        probe_at(*x, ProbeStage::Coarse, false, book, &mut spent, eval);
    }

    let mut boundaries_probed = 0usize;
    for boundary in boundaries_wei {
        if *boundary > lower && *boundary < upper && spent < budget {
            probe_at(
                *boundary,
                ProbeStage::Boundary,
                false,
                book,
                &mut spent,
                eval,
            );
            boundaries_probed += 1;
        }
    }

    let mut budget_exhausted = spent >= budget;
    'rounds: for _ in 0..cfg.refine_rounds {
        if spent >= budget {
            budget_exhausted = true;
            break;
        }
        let sizes: Vec<U256> = book.probes().iter().map(|p| p.amount_in_wei).collect();
        if sizes.len() < 2 {
            break;
        }
        // Score each interval by the best COMPUTED endpoint; an interval with
        // an absent endpoint is unscored and therefore never refined.
        let mut scored: Vec<(Decimal, usize)> = Vec::new();
        for i in 0..sizes.len() - 1 {
            let left = book.get(&sizes[i]).and_then(|p| p.sample.net_usd());
            let right = book.get(&sizes[i + 1]).and_then(|p| p.sample.net_usd());
            if let (Some(l), Some(r)) = (left, right) {
                scored.push((if l >= r { l } else { r }, i));
            }
        }
        if scored.is_empty() {
            break;
        }
        // Descending by score, ascending by position: fully deterministic.
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut added = 0usize;
        for (_, i) in scored.iter().take(cfg.refine_intervals) {
            let a = sizes[*i];
            let b = sizes[*i + 1];
            let inner = geometric_grid_exact(a, b, cfg.refine_points + 2);
            for x in inner.into_iter().skip(1).take(cfg.refine_points) {
                if x <= a || x >= b {
                    continue;
                }
                if spent >= budget {
                    budget_exhausted = true;
                    break 'rounds;
                }
                if probe_at(x, ProbeStage::Refine, false, book, &mut spent, eval) {
                    added += 1;
                }
            }
        }
        if added == 0 {
            break;
        }
    }

    Ok(report_from_book(
        book,
        scope,
        boundaries_probed,
        budget_exhausted,
        budget,
    ))
}

/// `n` log-spaced points spanning `[lo, hi]`, generated by INTEGER arithmetic.
///
/// The only `f64` here is the dimensionless spacing ratio `(hi/lo)^(1/(n-1))`;
/// the amounts themselves are produced by multiplying/dividing `U256` by a
/// 2^64-scaled rational, so a 24-digit capital cap keeps every probe exact —
/// which a `f64 → amount` grid (the parent module's `geom_probes`) cannot.
///
/// Guarantees: `lo` first, `hi` last, strictly increasing, no interior point
/// above `hi`. When the geometric walk stalls (a span too narrow for 2^64 of
/// resolution) the grid falls back to an exact LINEAR interpolation, which is
/// the same curve to within the span's own rounding.
pub fn geometric_grid_exact(lo: U256, hi: U256, n: usize) -> Vec<U256> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 || hi <= lo {
        return vec![if hi.is_zero() { U256::one() } else { hi }];
    }

    let lo_f = u256_to_f64_lossy(lo).max(1.0);
    let hi_f = u256_to_f64_lossy(hi).max(lo_f);
    let mut points: Vec<U256> = Vec::with_capacity(n);
    if hi_f > lo_f {
        let ratio = (hi_f / lo_f).powf(1.0 / (n as f64 - 1.0));
        let den = U256::from(2u64).pow(U256::from(64u64));
        let num = f64_to_u256_clamped(ratio * 18_446_744_073_709_551_616.0);
        if num > den {
            points.push(lo);
            let mut current = lo;
            for _ in 1..(n - 1) {
                let next = current
                    .checked_mul(num)
                    .and_then(|v| v.checked_div(den))
                    .unwrap_or(hi);
                if next <= current || next >= hi {
                    break;
                }
                points.push(next);
                current = next;
            }
            points.push(hi);
        }
    }
    if points.len() < n {
        return linear_grid_exact(lo, hi, n);
    }
    points.dedup();
    if points.last() != Some(&hi) {
        points.push(hi);
    }
    points
}

/// `n` points from `lo` to `hi` inclusive, exact (full-precision `mul_div`).
fn linear_grid_exact(lo: U256, hi: U256, n: usize) -> Vec<U256> {
    if n <= 1 || hi <= lo {
        return vec![hi];
    }
    let span = hi - lo;
    let divisor = U256::from((n - 1) as u64);
    let mut points: Vec<U256> = Vec::with_capacity(n);
    for i in 0..n {
        let offset = mul_div(span, U256::from(i as u64), divisor);
        let candidate = lo.checked_add(offset).unwrap_or(hi);
        points.push(if candidate > hi { hi } else { candidate });
    }
    if let Some(last) = points.last_mut() {
        *last = hi;
    }
    points.dedup();
    if points.last() != Some(&hi) {
        points.push(hi);
    }
    points
}

/// The parabolic vertex through the argmax and its two neighbours, as an exact
/// rational. `None` when the bracket is not concave, when the vertex leaves the
/// bracket, or when a neighbour is not computed.
///
/// This is a CANDIDATE GENERATOR, not an optimality proof: the size it returns
/// is rounded and re-quoted like every other candidate, and only the re-quote
/// decides. That is what lets the module use a local model without ever
/// assuming the landscape is unimodal.
pub fn vertex_candidate(report: &SearchReport) -> Option<RationalAmount> {
    // Same tie rule as `report_from_book`: walking ascending sizes with a
    // STRICT improvement keeps the smallest size, so the vertex bracket cannot
    // depend on which side of a plateau the iteration happened to start.
    let mut argmax: Option<usize> = None;
    for (i, probe) in report.probes.iter().enumerate() {
        let Some(net) = probe.sample.net_usd() else {
            continue;
        };
        let improves = argmax
            .and_then(|j| report.probes[j].sample.net_usd())
            .map(|current| net > current)
            .unwrap_or(true);
        if improves {
            argmax = Some(i);
        }
    }
    let index = argmax?;
    if index == 0 || index + 1 >= report.probes.len() {
        return None;
    }
    let left = &report.probes[index - 1];
    let mid = &report.probes[index];
    let right = &report.probes[index + 1];
    let y0 = left.sample.net_usd()?;
    let y1 = mid.sample.net_usd()?;
    let y2 = right.sample.net_usd()?;

    let x0 = u256_to_decimal(left.amount_in_wei)?;
    let x1 = u256_to_decimal(mid.amount_in_wei)?;
    let x2 = u256_to_decimal(right.amount_in_wei)?;

    let d1 = x1.checked_sub(x0)?;
    let d2 = x2.checked_sub(x1)?;
    if d1 <= Decimal::ZERO || d2 <= Decimal::ZERO {
        return None;
    }
    // x* = x1 + d1·d2·(y0 − y2) / (2·(d2·(y0 − y1) − d1·(y1 − y2)))
    let numerator = d1.checked_mul(d2)?.checked_mul(y0.checked_sub(y2)?)?;
    let inner = d2
        .checked_mul(y0.checked_sub(y1)?)?
        .checked_sub(d1.checked_mul(y1.checked_sub(y2)?)?)?;
    let denominator = inner.checked_mul(Decimal::from(2u32))?;
    // A maximum needs negative curvature through the bracket.
    if denominator.is_zero() || !denominator.is_sign_negative() {
        return None;
    }
    let x_star = x1.checked_add(numerator.checked_div(denominator)?)?;
    if x_star <= x0 || x_star >= x2 {
        return None;
    }
    RationalAmount::from_decimal_split(x_star, IDEAL_PRECISION_DEN)
}

// ===========================================================================
// 6. Financing: principal, premium, repayment (prompt §6/§5)
// ===========================================================================

/// A provider's fee terms, in exactly the three states the doctrine separates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeeTerms {
    /// The fee was READ and is exactly zero. A CREDITED zero is preserved.
    CreditedZero { source: String },
    /// Exact premium in basis points of the principal, from the provider's own
    /// contract (e.g. Aave `FLASHLOAN_PREMIUM_TOTAL`).
    ExactBps { bps: Decimal, source: String },
    /// Exact flat premium, in the borrowed asset's base units.
    ExactFlatWei { amount_wei: U256, source: String },
    /// The fee could NOT be resolved. Nothing may be assumed about it — this is
    /// NOT a zero, and the caller must carry a resolution task.
    Absent { reason: String },
}

impl FeeTerms {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CreditedZero { .. } => "credited_zero",
            Self::ExactBps { .. } => "exact_bps",
            Self::ExactFlatWei { .. } => "exact_flat_wei",
            Self::Absent { .. } => "absent",
        }
    }

    pub fn is_absent(&self) -> bool {
        matches!(self, Self::Absent { .. })
    }

    pub fn source(&self) -> Option<&str> {
        match self {
            Self::CreditedZero { source }
            | Self::ExactBps { source, .. }
            | Self::ExactFlatWei { source, .. } => Some(source.as_str()),
            Self::Absent { .. } => None,
        }
    }

    /// Premium in base units, ROUNDED UP (never under-charge the provider and
    /// never publish a repayment smaller than the debt). `None` ⇔ ABSENT.
    pub fn premium_wei(&self, principal_wei: U256) -> Option<U256> {
        match self {
            Self::CreditedZero { .. } => Some(U256::zero()),
            Self::ExactFlatWei { amount_wei, .. } => Some(*amount_wei),
            Self::ExactBps { bps, .. } => {
                if bps.is_sign_negative() {
                    return None;
                }
                let (bps_num, bps_den) = decimal_to_rational(*bps, IDEAL_PRECISION_DEN)?;
                let denominator = bps_den.checked_mul(U256::from(10_000u64))?;
                let numerator = principal_wei.checked_mul(bps_num)?;
                let quotient = numerator / denominator;
                let remainder = numerator % denominator;
                if remainder.is_zero() {
                    Some(quotient)
                } else {
                    quotient.checked_add(U256::one())
                }
            }
            Self::Absent { .. } => None,
        }
    }
}

/// `d` as the exact rational `num/den` (see `RationalAmount::from_decimal_split`).
fn decimal_to_rational(d: Decimal, den: u64) -> Option<(U256, U256)> {
    let rational = RationalAmount::from_decimal_split(d, den)?;
    Some((rational.num, rational.den))
}

/// Where the route's input capital comes from. `FlashAccounting` is a distinct
/// kind on purpose: transient settlement (V4/Balancer-style) is NOT financing
/// and must never be priced as if a provider had lent the principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapitalSource {
    OwnCapital {
        available: Bound,
    },
    FlashFinancing {
        provider: String,
        asset: String,
        method: String,
        available: Bound,
        terms: FeeTerms,
    },
    FlashAccounting {
        venue: String,
    },
}

impl CapitalSource {
    pub fn own_capital(available: Bound) -> Self {
        Self::OwnCapital { available }
    }

    pub fn flash_financing(
        provider: impl Into<String>,
        asset: impl Into<String>,
        method: impl Into<String>,
        available: Bound,
        terms: FeeTerms,
    ) -> Self {
        Self::FlashFinancing {
            provider: provider.into(),
            asset: asset.into(),
            method: method.into(),
            available,
            terms,
        }
    }

    pub fn flash_accounting(venue: impl Into<String>) -> Self {
        Self::FlashAccounting {
            venue: venue.into(),
        }
    }

    pub fn id(&self) -> String {
        match self {
            Self::OwnCapital { .. } => "own_capital".to_string(),
            Self::FlashFinancing {
                provider, method, ..
            } => format!("{provider}:{method}"),
            Self::FlashAccounting { venue } => format!("flash_accounting:{venue}"),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::OwnCapital { .. } => "own_capital",
            Self::FlashFinancing { .. } => "flash_financing",
            Self::FlashAccounting { .. } => "flash_accounting",
        }
    }

    /// A financing PROVIDER is something that lends a principal and is repaid.
    /// Transient accounting is not one (prompt §6: "Distingue flash accounting
    /// de financiación flash").
    pub fn is_financing(&self) -> bool {
        !matches!(self, Self::FlashAccounting { .. })
    }

    pub fn terms(&self) -> Option<&FeeTerms> {
        match self {
            Self::FlashFinancing { terms, .. } => Some(terms),
            _ => None,
        }
    }
}

/// The three-state outcome of pricing one capital source.
#[derive(Debug, Clone, PartialEq)]
pub enum FinancingOutcome {
    /// Priced exactly.
    Computed {
        net_usd: Decimal,
        /// `proceeds − repayment` (can be negative).
        surplus_usd: Decimal,
        premium_wei: U256,
        repayment_wei: U256,
    },
    /// Could not be priced. `resolution_task` names the concrete read that
    /// would close it (prompt §5: "Un fee desconocido debe generar una tarea
    /// concreta de resolución").
    Absent {
        reason: String,
        resolution_task: String,
    },
    /// The provider cannot cover the principal — a THIRD state, distinct from
    /// "absent" and from "sufficient".
    Insufficient {
        available_wei: U256,
        needed_wei: U256,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FinancingEval {
    pub provider_id: String,
    pub kind: &'static str,
    pub is_financing: bool,
    pub principal_wei: U256,
    pub premium_wei: Bound,
    pub repayment_wei: Bound,
    pub outcome: FinancingOutcome,
    pub notes: Vec<&'static str>,
}

impl FinancingEval {
    pub fn net_usd(&self) -> Option<Decimal> {
        match &self.outcome {
            FinancingOutcome::Computed { net_usd, .. } => Some(*net_usd),
            _ => None,
        }
    }

    pub fn is_computed(&self) -> bool {
        matches!(self.outcome, FinancingOutcome::Computed { .. })
    }
}

/// Everything the financing comparison needs. `amount_out_wei` is the route's
/// proceeds in the debt asset; `amount_in_wei` is the capital the route needs.
#[derive(Debug, Clone, PartialEq)]
pub struct FinancingRequest<'a> {
    pub denomination: TokenDenomination,
    pub price_usd: Decimal,
    pub amount_in_wei: U256,
    pub amount_out_wei: U256,
    pub gas_usd: Decimal,
    pub ops_usd: Decimal,
    pub offers: &'a [CapitalSource],
}

/// Price every admissible source against the SAME route.
///
/// Accounting identity, applied to every offer without exception:
///
/// ```text
/// net_usd = value(proceeds) − value(repayment) − gas − ops
/// repayment = principal + premium
/// ```
///
/// The borrowed principal therefore never appears as revenue: it enters the
/// proceeds side and leaves through the repayment, so a zero-edge borrow is
/// always net-negative by at least the premium and the costs.
pub fn compare_financing(request: &FinancingRequest<'_>) -> Result<Vec<FinancingEval>, String> {
    if request.price_usd <= Decimal::ZERO {
        return Err(format!(
            "joint_sizing: non-positive price {} cannot value financing",
            request.price_usd
        ));
    }
    let denomination = request.denomination;
    let price = request.price_usd;
    let proceeds_usd = denomination
        .base_units_to_usd(request.amount_out_wei, price)
        .ok_or_else(|| {
            format!(
                "joint_sizing: proceeds {} out of exact decimal range",
                request.amount_out_wei
            )
        })?;

    let mut evals: Vec<FinancingEval> = Vec::with_capacity(request.offers.len());
    for offer in request.offers {
        let mut notes: Vec<&'static str> = Vec::new();
        match offer {
            CapitalSource::FlashAccounting { venue } => {
                // Transient settlement: the input is unlocked and must be
                // SETTLED before the same unlock closes, but no provider lends
                // it and no premium is charged. The principal is therefore
                // still owed (it leaves through the repayment) — what is absent
                // is the FINANCING, not the repayment. Pricing this as "no
                // principal" would count the principal as revenue, which is
                // exactly what prompt §4 forbids.
                let principal = request.amount_in_wei;
                notes.push("flash_accounting_is_not_financing");
                notes.push("no_provider_no_premium");
                notes.push("principal_settled_in_same_unlock");
                let principal_usd = denomination
                    .base_units_to_usd(principal, price)
                    .ok_or_else(|| {
                        "joint_sizing: flash-accounting principal out of range".to_string()
                    })?;
                let surplus = proceeds_usd - principal_usd;
                let net = surplus - request.gas_usd - request.ops_usd;
                evals.push(FinancingEval {
                    provider_id: format!("flash_accounting:{venue}"),
                    kind: "flash_accounting",
                    is_financing: false,
                    principal_wei: principal,
                    premium_wei: Bound::known(
                        U256::zero(),
                        "flash_accounting:no_premium_by_design",
                    ),
                    repayment_wei: Bound::known(
                        principal,
                        "flash_accounting:settled_in_same_unlock",
                    ),
                    outcome: FinancingOutcome::Computed {
                        net_usd: net,
                        surplus_usd: surplus,
                        premium_wei: U256::zero(),
                        repayment_wei: principal,
                    },
                    notes,
                });
            }
            CapitalSource::OwnCapital { available } => {
                let principal = request.amount_in_wei;
                match capacity_verdict(available, principal) {
                    CapacityVerdict::Absent { reason, task } => {
                        evals.push(FinancingEval {
                            provider_id: "own_capital".to_string(),
                            kind: "own_capital",
                            is_financing: false,
                            principal_wei: principal,
                            premium_wei: Bound::known(U256::zero(), "own_capital:no_provider_fee"),
                            repayment_wei: Bound::absent(reason.clone()),
                            outcome: FinancingOutcome::Absent {
                                reason,
                                resolution_task: task,
                            },
                            notes,
                        });
                        continue;
                    }
                    CapacityVerdict::Insufficient {
                        available_wei,
                        needed_wei,
                    } => {
                        evals.push(FinancingEval {
                            provider_id: "own_capital".to_string(),
                            kind: "own_capital",
                            is_financing: false,
                            principal_wei: principal,
                            premium_wei: Bound::known(U256::zero(), "own_capital:no_provider_fee"),
                            repayment_wei: Bound::absent("own_balance_insufficient"),
                            outcome: FinancingOutcome::Insufficient {
                                available_wei,
                                needed_wei,
                            },
                            notes,
                        });
                        continue;
                    }
                    CapacityVerdict::Sufficient => {}
                }
                // The operator's own capital is returned to the operator, so it
                // is not a cost: net is the route's own net.
                notes.push("own_capital_repaid_to_self");
                notes.push("principal_is_not_income");
                let repayment_usd = denomination
                    .base_units_to_usd(principal, price)
                    .ok_or_else(|| {
                        "joint_sizing: own principal out of decimal range".to_string()
                    })?;
                let surplus = proceeds_usd - repayment_usd;
                let net = surplus - request.gas_usd - request.ops_usd;
                evals.push(FinancingEval {
                    provider_id: "own_capital".to_string(),
                    kind: "own_capital",
                    is_financing: false,
                    principal_wei: principal,
                    premium_wei: Bound::known(U256::zero(), "own_capital:no_provider_fee"),
                    repayment_wei: Bound::known(principal, "own_capital:principal_returned"),
                    outcome: FinancingOutcome::Computed {
                        net_usd: net,
                        surplus_usd: surplus,
                        premium_wei: U256::zero(),
                        repayment_wei: principal,
                    },
                    notes,
                });
            }
            CapitalSource::FlashFinancing {
                provider,
                asset,
                method,
                available,
                terms,
            } => {
                let principal = request.amount_in_wei;
                match capacity_verdict(available, principal) {
                    CapacityVerdict::Absent { reason, task } => {
                        evals.push(FinancingEval {
                            provider_id: format!("{provider}:{method}"),
                            kind: "flash_financing",
                            is_financing: true,
                            principal_wei: principal,
                            premium_wei: match terms {
                                FeeTerms::Absent { reason } => Bound::absent(reason.clone()),
                                other => Bound::absent(format!(
                                    "capacity_absent_so_premium_not_priced:{}",
                                    other.as_str()
                                )),
                            },
                            repayment_wei: Bound::absent(reason.clone()),
                            outcome: FinancingOutcome::Absent {
                                reason,
                                resolution_task: task,
                            },
                            notes,
                        });
                        continue;
                    }
                    CapacityVerdict::Insufficient {
                        available_wei,
                        needed_wei,
                    } => {
                        evals.push(FinancingEval {
                            provider_id: format!("{provider}:{method}"),
                            kind: "flash_financing",
                            is_financing: true,
                            principal_wei: principal,
                            premium_wei: Bound::absent("capacity_insufficient"),
                            repayment_wei: Bound::absent("capacity_insufficient"),
                            outcome: FinancingOutcome::Insufficient {
                                available_wei,
                                needed_wei,
                            },
                            notes,
                        });
                        continue;
                    }
                    CapacityVerdict::Sufficient => {}
                }

                let Some(premium) = terms.premium_wei(principal) else {
                    // ABSENT premium: no number may be produced from it. The
                    // resolution task names the provider read that closes it AND
                    // carries the recorded reason verbatim.
                    let recorded = match terms {
                        FeeTerms::Absent { reason } => reason.clone(),
                        other => format!("premium_unresolvable_from_{}", other.as_str()),
                    };
                    evals.push(FinancingEval {
                        provider_id: format!("{provider}:{method}"),
                        kind: "flash_financing",
                        is_financing: true,
                        principal_wei: principal,
                        premium_wei: Bound::absent("financing_premium_absent"),
                        repayment_wei: Bound::absent("financing_premium_absent"),
                        outcome: FinancingOutcome::Absent {
                            reason: "financing_premium_absent".to_string(),
                            resolution_task: format!(
                                "read the premium of provider '{provider}' for asset '{asset}' via '{method}' \
                                 (recorded reason: {recorded}) and record it as \
                                 FeeTerms::ExactBps/ExactFlatWei"
                            ),
                        },
                        notes,
                    });
                    continue;
                };
                let Some(repayment) = principal.checked_add(premium) else {
                    evals.push(FinancingEval {
                        provider_id: format!("{provider}:{method}"),
                        kind: "flash_financing",
                        is_financing: true,
                        principal_wei: principal,
                        premium_wei: Bound::known(premium, terms.source().unwrap_or("provider")),
                        repayment_wei: Bound::absent("repayment_overflow"),
                        outcome: FinancingOutcome::Absent {
                            reason: "repayment_overflow".to_string(),
                            resolution_task:
                                "principal + premium overflowed U256 — the size is not representable"
                                    .to_string(),
                        },
                        notes,
                    });
                    continue;
                };
                notes.push("principal_is_not_income");
                notes.push("supplied_principal_not_counted_as_revenue");
                if matches!(terms, FeeTerms::CreditedZero { .. }) {
                    notes.push("zero_premium_credited");
                }
                let repayment_usd = denomination
                    .base_units_to_usd(repayment, price)
                    .ok_or_else(|| "joint_sizing: repayment out of decimal range".to_string())?;
                let surplus = proceeds_usd - repayment_usd;
                let net = surplus - request.gas_usd - request.ops_usd;
                evals.push(FinancingEval {
                    provider_id: format!("{provider}:{method}"),
                    kind: "flash_financing",
                    is_financing: true,
                    principal_wei: principal,
                    premium_wei: Bound::known(premium, terms.source().unwrap_or("provider_terms")),
                    repayment_wei: Bound::known(repayment, "principal_plus_premium"),
                    outcome: FinancingOutcome::Computed {
                        net_usd: net,
                        surplus_usd: surplus,
                        premium_wei: premium,
                        repayment_wei: repayment,
                    },
                    notes,
                });
            }
        }
    }
    Ok(evals)
}

enum CapacityVerdict {
    Sufficient,
    Insufficient {
        available_wei: U256,
        needed_wei: U256,
    },
    Absent {
        reason: String,
        task: String,
    },
}

fn capacity_verdict(available: &Bound, needed: U256) -> CapacityVerdict {
    match available {
        Bound::Absent { reason } => CapacityVerdict::Absent {
            reason: format!("capacity_absent:{reason}"),
            task: format!(
                "resolve the available principal of this source (was absent: {reason}) and report it as Bound::Known"
            ),
        },
        Bound::Known { value, .. } => {
            if *value < needed {
                CapacityVerdict::Insufficient {
                    available_wei: *value,
                    needed_wei: needed,
                }
            } else {
                CapacityVerdict::Sufficient
            }
        }
    }
}

/// The best PRICED offer by net. Only computed offers compete: an absent or
/// insufficient one can never win by omission.
pub fn best_financing(evals: &[FinancingEval]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, eval) in evals.iter().enumerate() {
        let Some(net) = eval.net_usd() else {
            continue;
        };
        let better = match best {
            Some(j) => net > evals[j].net_usd().unwrap_or(Decimal::MIN),
            None => true,
        };
        if better {
            best = Some(i);
        }
    }
    best
}

// ===========================================================================
// 7. Split routes over shared pools (prompt §6)
// ===========================================================================

/// Constant-product pool state the split evaluator composes over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpmmPoolState {
    pub pool_id: String,
    pub reserve_in: U256,
    pub reserve_out: U256,
    pub fee_bps: u32,
}

/// One path of a split: an ordered list of pool ids and its share of the input,
/// in basis points of the total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPath {
    pub path_id: String,
    pub pool_ids: Vec<String>,
    pub share_bps: u32,
}

/// A split allocation plus the pool state it consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPoolSplit {
    pub total_in_wei: U256,
    pub paths: Vec<SplitPath>,
    pub pools: BTreeMap<String, CpmmPoolState>,
}

/// Both figures, so the error that summing independent legs would make is
/// visible instead of being assumed away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitOutcome {
    /// Output of the SEQUENTIAL composition over one shared pool state — the
    /// figure that may be published.
    pub total_out_joint_wei: U256,
    /// Sum of each path priced against the UNTOUCHED state — an over-statement
    /// whenever two paths consume the same pool. Reported for comparison only.
    pub total_out_independent_wei: U256,
    pub overstatement_wei: U256,
    pub per_path_joint_out_wei: Vec<(String, U256)>,
    pub per_path_independent_out_wei: Vec<(String, U256)>,
    pub execution_order: Vec<String>,
    pub shared_pools: Vec<String>,
    pub notes: Vec<&'static str>,
}

impl SharedPoolSplit {
    /// Compose every path over ONE mutable pool state, in `paths` order.
    ///
    /// The order is part of the plan: the second consumer of a shared pool sees
    /// the reserves the first one left behind. Allocations are exact integers of
    /// base units (shares must sum to 10 000 bps; the indivisible remainder goes
    /// to the FIRST path, deterministically), so the split conserves the input.
    pub fn evaluate(&self) -> Result<SplitOutcome, String> {
        if self.paths.is_empty() {
            return Err("split_no_paths".to_string());
        }
        // Saturating: a malformed allocation table must be refused below, not
        // panic the sizing path with an arithmetic overflow.
        let share_sum: u32 = self
            .paths
            .iter()
            .fold(0u32, |acc, p| acc.saturating_add(p.share_bps));
        if share_sum != 10_000 {
            return Err(format!(
                "split_share_conservation_violated:sum_bps={share_sum}"
            ));
        }
        for path in &self.paths {
            if path.pool_ids.is_empty() {
                return Err(format!("split_path_empty:{}", path.path_id));
            }
            for pool_id in &path.pool_ids {
                if !self.pools.contains_key(pool_id) {
                    return Err(format!("split_pool_state_absent:{pool_id}"));
                }
            }
        }

        // Exact allocation with the remainder pushed onto the first path.
        let mut allocations: Vec<U256> = Vec::with_capacity(self.paths.len());
        let mut allocated = U256::zero();
        for path in &self.paths {
            let part = mul_div(
                self.total_in_wei,
                U256::from(path.share_bps as u64),
                U256::from(10_000u64),
            );
            allocated = allocated
                .checked_add(part)
                .ok_or("split_allocation_overflow")?;
            allocations.push(part);
        }
        if allocated > self.total_in_wei {
            return Err("split_allocation_exceeds_total".to_string());
        }
        if let Some(first) = allocations.first_mut() {
            *first = first
                .checked_add(self.total_in_wei - allocated)
                .ok_or("split_allocation_overflow")?;
        }

        // ── Joint: one mutable state, consumed in order. ────────────────────
        let mut joint_state = self.pools.clone();
        let mut joint_outs: Vec<(String, U256)> = Vec::new();
        let mut independent_outs: Vec<(String, U256)> = Vec::new();
        let mut execution_order: Vec<String> = Vec::new();

        for (index, path) in self.paths.iter().enumerate() {
            let amount = allocations[index];
            let joint_out = walk_path(&mut joint_state, &path.pool_ids, amount)?;
            joint_outs.push((path.path_id.clone(), joint_out));
            execution_order.push(path.path_id.clone());

            // ── Independent: a fresh copy per path (no cross-path effect). ──
            let mut fresh = self.pools.clone();
            let independent_out = walk_path(&mut fresh, &path.pool_ids, amount)?;
            independent_outs.push((path.path_id.clone(), independent_out));
        }

        let total_joint = joint_outs
            .iter()
            .try_fold(U256::zero(), |acc, (_, v)| acc.checked_add(*v))
            .ok_or("split_total_overflow")?;
        let total_independent = independent_outs
            .iter()
            .try_fold(U256::zero(), |acc, (_, v)| acc.checked_add(*v))
            .ok_or("split_total_overflow")?;

        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for path in &self.paths {
            for pool_id in &path.pool_ids {
                *counts.entry(pool_id.as_str()).or_insert(0) += 1;
            }
        }
        let shared_pools: Vec<String> = counts
            .iter()
            .filter(|(_, n)| **n > 1)
            .map(|(id, _)| (*id).to_string())
            .collect();

        let mut notes: Vec<&'static str> = Vec::new();
        if !shared_pools.is_empty() {
            notes.push("shared_pool_consumed_sequentially");
            notes.push("independent_sum_overstates_a_shared_pool");
        }
        notes.push("published_figure_is_the_joint_composition");

        Ok(SplitOutcome {
            total_out_joint_wei: total_joint,
            total_out_independent_wei: total_independent,
            overstatement_wei: total_independent.saturating_sub(total_joint),
            per_path_joint_out_wei: joint_outs,
            per_path_independent_out_wei: independent_outs,
            execution_order,
            shared_pools,
            notes,
        })
    }
}

/// Walk one path over `state`, MUTATING the reserves each hop consumes.
///
/// State update follows the V2 invariant exactly: the full input amount is
/// added to `reserve_in` (the fee stays in the pool, so `k` grows) and the
/// output leaves `reserve_out`.
fn walk_path(
    state: &mut BTreeMap<String, CpmmPoolState>,
    pool_ids: &[String],
    amount_in: U256,
) -> Result<U256, String> {
    let mut amount = amount_in;
    for pool_id in pool_ids {
        let pool = state
            .get_mut(pool_id)
            .ok_or_else(|| format!("split_pool_state_absent:{pool_id}"))?;
        if pool.reserve_in.is_zero() || pool.reserve_out.is_zero() {
            return Err(format!("split_pool_reserves_zero:{pool_id}"));
        }
        let out = v2_amount_out(amount, pool.reserve_in, pool.reserve_out, pool.fee_bps);
        if out.is_zero() {
            return Err(format!("split_hop_yields_zero:{pool_id}"));
        }
        if out >= pool.reserve_out {
            return Err(format!("split_pool_out_reserve_exhausted:{pool_id}"));
        }
        pool.reserve_in = pool
            .reserve_in
            .checked_add(amount)
            .ok_or_else(|| format!("split_pool_reserve_overflow:{pool_id}"))?;
        pool.reserve_out -= out;
        amount = out;
    }
    Ok(amount)
}

/// A [`CycleQuoter`] over a split allocation: prices the JOINT composition and
/// exposes the independent-sum comparison as evidence.
pub struct SplitQuoter {
    pub split: SharedPoolSplit,
    pub denomination: TokenDenomination,
    pub price_usd: Decimal,
    pub gas_usd: Decimal,
    pub ops_usd: Decimal,
    pub provenance: String,
    last: Option<SplitOutcome>,
}

impl SplitQuoter {
    pub fn new(
        split: SharedPoolSplit,
        denomination: TokenDenomination,
        price_usd: Decimal,
        gas_usd: Decimal,
        ops_usd: Decimal,
        provenance: impl Into<String>,
    ) -> Self {
        Self {
            split,
            denomination,
            price_usd,
            gas_usd,
            ops_usd,
            provenance: provenance.into(),
            last: None,
        }
    }

    pub fn last_outcome(&self) -> Option<&SplitOutcome> {
        self.last.as_ref()
    }
}

impl CycleQuoter for SplitQuoter {
    fn quote_cycle(&mut self, amount_in_wei: U256) -> QuoteOutcome {
        let mut split = self.split.clone();
        split.total_in_wei = amount_in_wei;
        let outcome = match split.evaluate() {
            Ok(o) => o,
            Err(reason) => return QuoteOutcome::Absent { reason },
        };
        let Some(out_usd) = self
            .denomination
            .base_units_to_usd(outcome.total_out_joint_wei, self.price_usd)
        else {
            return QuoteOutcome::Absent {
                reason: "split_joint_out_of_decimal_range".to_string(),
            };
        };
        let Some(in_usd) = self
            .denomination
            .base_units_to_usd(amount_in_wei, self.price_usd)
        else {
            return QuoteOutcome::Absent {
                reason: "split_total_in_out_of_decimal_range".to_string(),
            };
        };
        let quote = CycleQuote::new(
            amount_in_wei,
            outcome.total_out_joint_wei,
            out_usd - in_usd,
            self.gas_usd + self.ops_usd,
            format!("{}|joint_split", self.provenance),
        );
        self.last = Some(outcome);
        QuoteOutcome::Quoted(quote)
    }

    fn split_evidence(&self) -> Option<(U256, U256)> {
        self.last
            .as_ref()
            .map(|o| (o.total_out_joint_wei, o.total_out_independent_wei))
    }
}

// ===========================================================================
// 8. Joint driver: route × size × financing (prompt §6)
// ===========================================================================

/// Which way round the cycle is traded. Both are searched when the detector
/// supplies both; the winner is reported with its orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    TerminalAToB,
    TerminalBToA,
}

impl Orientation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TerminalAToB => "a_to_b",
            Self::TerminalBToA => "b_to_a",
        }
    }

    pub fn flipped(&self) -> Self {
        match self {
            Self::TerminalAToB => Self::TerminalBToA,
            Self::TerminalBToA => Self::TerminalAToB,
        }
    }
}

/// One candidate route: its identity, its orientation, and how it differs from
/// the baseline (pool change, mixed protocols, extra bounds).
#[derive(Debug, Clone, PartialEq)]
pub struct RouteVariant {
    pub variant_id: String,
    pub orientation: Orientation,
    pub route_label: String,
    /// Free-form change tags against the baseline route ("pool", "protocol",
    /// "split", "financing"): the detector's own vocabulary, carried verbatim.
    pub change_vs_baseline: Vec<String>,
    /// Bounds specific to THIS variant (a different pool has different depth).
    pub extra_limits: Vec<SizeLimit>,
    /// When set, the variant is sized as a SPLIT over shared pools.
    pub split: Option<SharedPoolSplit>,
}

impl RouteVariant {
    pub fn new(
        variant_id: impl Into<String>,
        orientation: Orientation,
        route_label: impl Into<String>,
    ) -> Self {
        Self {
            variant_id: variant_id.into(),
            orientation,
            route_label: route_label.into(),
            change_vs_baseline: Vec::new(),
            extra_limits: Vec::new(),
            split: None,
        }
    }

    /// The same route in both economic orientations (prompt §6).
    pub fn both_orientations(base_id: &str, route_label: &str) -> Vec<Self> {
        vec![
            Self::new(
                format!("{base_id}:a_to_b"),
                Orientation::TerminalAToB,
                route_label,
            ),
            Self::new(
                format!("{base_id}:b_to_a"),
                Orientation::TerminalBToA,
                route_label,
            ),
        ]
    }

    pub fn with_change(mut self, change: impl Into<String>) -> Self {
        self.change_vs_baseline.push(change.into());
        self
    }

    pub fn with_limit(mut self, limit: SizeLimit) -> Self {
        self.extra_limits.push(limit);
        self
    }

    pub fn with_split(mut self, split: SharedPoolSplit) -> Self {
        self.split = Some(split);
        self
    }
}

/// The search objective and the hard execution floor, kept SEPARATE.
///
/// Prompt §7: "Separa mínimo duro de ejecución y objetivo de búsqueda. … No
/// reduzcas `min_profit_usd` automáticamente para obtener una aprobación." This
/// type is immutable to the optimizer: nothing below this line ever writes to
/// it, so a threshold cannot be relaxed by the search it governs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JointObjective {
    /// The configured search objective (the "objetivo" of the card).
    pub search_target_usd: Decimal,
    /// The hard minimum net that authorizes execution.
    pub execution_min_net_usd: Decimal,
}

impl JointObjective {
    pub fn new(search_target_usd: Decimal, execution_min_net_usd: Decimal) -> Result<Self, String> {
        if search_target_usd < Decimal::ZERO {
            return Err(format!(
                "joint_sizing: negative search target {search_target_usd}"
            ));
        }
        if execution_min_net_usd < Decimal::ZERO {
            return Err(format!(
                "joint_sizing: negative execution minimum {execution_min_net_usd}"
            ));
        }
        Ok(Self {
            search_target_usd,
            execution_min_net_usd,
        })
    }
}

/// The five mutually exclusive verdicts of one sized variant. "Loss", "exactly
/// zero" and "not computed" are three different states (prompt §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointVerdict {
    TargetMet { net_usd: Decimal },
    BelowTarget { net_usd: Decimal, gap_usd: Decimal },
    NetZero,
    Loss { net_usd: Decimal },
    Absent,
}

impl JointVerdict {
    pub fn token(&self) -> &'static str {
        match self {
            Self::TargetMet { .. } => "target_met",
            Self::BelowTarget { .. } => "below_target",
            Self::NetZero => "net_zero",
            Self::Loss { .. } => "loss",
            Self::Absent => "absent",
        }
    }

    pub fn net_usd(&self) -> Option<Decimal> {
        match self {
            Self::TargetMet { net_usd } | Self::BelowTarget { net_usd, .. } => Some(*net_usd),
            Self::Loss { net_usd } => Some(*net_usd),
            Self::NetZero => Some(Decimal::ZERO),
            Self::Absent => None,
        }
    }

    /// `gap_usd = max(0, target − net)` (prompt §4).
    pub fn classify(net_usd: Option<Decimal>, target_usd: Decimal) -> Self {
        match net_usd {
            None => Self::Absent,
            Some(net) if net < Decimal::ZERO => Self::Loss { net_usd: net },
            Some(net) if net.is_zero() => Self::NetZero,
            Some(net) if net >= target_usd => Self::TargetMet { net_usd: net },
            Some(net) => Self::BelowTarget {
                net_usd: net,
                gap_usd: target_usd - net,
            },
        }
    }
}

/// The figure that may be published. It exists ONLY when the ROUNDED size was
/// re-quoted successfully (prompt §6: "Redondea a unidades mínimas y vuelve a
/// cotizar").
#[derive(Debug, Clone, PartialEq)]
pub struct PublishedSize {
    pub amount_in_wei: U256,
    pub amount_out_wei: U256,
    pub net_usd: Decimal,
    pub gross_usd: Decimal,
    pub costs_usd: Decimal,
    pub provenance: String,
    /// Always `"requote"` — the field exists so a consumer cannot mistake the
    /// ideal's figure for the published one.
    pub source: &'static str,
    pub ideal: RationalAmount,
    pub rounding: RoundedSize,
    /// `requote_net − ideal_net`: what forcing a representable size cost.
    pub rounding_effect_usd: Option<Decimal>,
}

/// The end of one variant's sizing: the ideal, the rounding, and the re-quote.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedSize {
    pub ideal: RationalAmount,
    pub rounded: RoundedSize,
    pub ideal_net_usd: Option<Decimal>,
    /// `Some` ⟺ the rounded size was quoted.
    pub published: Option<PublishedSize>,
    /// Why the rounded size could not be quoted. The ideal's figures are NOT
    /// published in this case — absence stays absence.
    pub requote_absent_reason: Option<String>,
    pub requote_sample: Sample,
}

/// Re-quote the ROUNDED size and publish only that.
pub fn finalize_rounded<F>(
    ideal: RationalAmount,
    rule: RoundingRule,
    direction: RoundingDirection,
    domain: &SizeDomain,
    ideal_net_usd: Option<Decimal>,
    book: &mut ProbeBook,
    eval: &mut F,
) -> Result<FinalizedSize, RoundingError>
where
    F: FnMut(U256) -> Sample,
{
    let rounded = round_within_domain(ideal, rule, direction, domain)?;
    // The RE-QUOTE is a new observation at a representable size: it must never
    // be answered from the search's cache, which is why every probe below is
    // recorded with `force = true` and stage `Requote`.
    let sample = eval(rounded.rounded_wei);
    book.record(
        rounded.rounded_wei,
        sample.clone(),
        ProbeStage::Requote,
        true,
    );

    let (published, absent_reason) = match &sample {
        Sample::Value {
            net_usd,
            gross_usd,
            costs_usd,
            amount_out_wei,
            provenance,
        } => (
            Some(PublishedSize {
                amount_in_wei: rounded.rounded_wei,
                amount_out_wei: *amount_out_wei,
                net_usd: *net_usd,
                gross_usd: *gross_usd,
                costs_usd: *costs_usd,
                provenance: provenance.clone(),
                source: "requote",
                ideal,
                rounding: rounded.clone(),
                rounding_effect_usd: ideal_net_usd.map(|ideal_net| *net_usd - ideal_net),
            }),
            None,
        ),
        Sample::Absent { reason } => (None, Some(reason.clone())),
    };

    Ok(FinalizedSize {
        ideal,
        rounded,
        ideal_net_usd,
        published,
        requote_absent_reason: absent_reason,
        requote_sample: sample,
    })
}

/// Everything one sizing pass needs, resolved once.
#[derive(Debug, Clone, PartialEq)]
pub struct JointRequest {
    pub denomination: TokenDenomination,
    pub price_usd: Decimal,
    pub limits: Vec<SizeLimit>,
    /// Smallest notional worth publishing, in USD. Converted to the domain's
    /// lower bound with the same exact decimal arithmetic as every other size.
    pub dust_floor_usd: Decimal,
    /// Tick/bin/settlement edges the search MUST probe (prompt §6).
    pub boundaries_wei: Vec<U256>,
    pub config: MultiscaleConfig,
    pub objective: JointObjective,
    pub financing_offers: Vec<CapitalSource>,
    pub gas_usd: Decimal,
    pub ops_usd: Decimal,
    pub rounding_rule: RoundingRule,
    pub rounding_direction: RoundingDirection,
}

/// Best-candidate pointer used by the three ranking slots of the report.
#[derive(Debug, Clone, PartialEq)]
pub struct BestCandidate {
    pub variant_id: String,
    pub orientation: Orientation,
    pub verdict: JointVerdict,
    pub net_usd: Decimal,
    pub amount_in_wei: U256,
    pub financing_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantOutcome {
    pub variant: RouteVariant,
    pub domain: Option<SizeDomain>,
    pub domain_reject: Option<DomainReject>,
    pub search: Option<SearchReport>,
    pub finalized: Option<FinalizedSize>,
    pub financing: Vec<FinancingEval>,
    pub best_financing: Option<usize>,
    pub verdict: JointVerdict,
    /// Why nothing could be published, when nothing could.
    pub absence_reason: Option<String>,
    /// `net ≥ execution_min_net_usd` on the PUBLISHED figure.
    pub execution_eligible: bool,
    pub search_error: Option<String>,
    /// `(joint, independent)` measured for a split variant.
    pub split_evidence: Option<(U256, U256)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JointReport {
    pub outcomes: Vec<VariantOutcome>,
    /// Best computed net across all variants — MAY BE NEGATIVE, and is kept.
    pub best_diagnostic: Option<BestCandidate>,
    /// Best net strictly above zero.
    pub best_feasible: Option<BestCandidate>,
    /// Best net at or above the configured objective.
    pub best_over_target: Option<BestCandidate>,
    /// Diverse alternatives (distinct variants), best first.
    pub alternatives: Vec<BestCandidate>,
    pub probes_total: usize,
    pub budget_exhausted: bool,
    pub scope: String,
    pub basis: &'static str,
}

fn candidate_from(outcome: &VariantOutcome) -> Option<BestCandidate> {
    let published = outcome.finalized.as_ref()?.published.as_ref()?;
    let provider = outcome
        .best_financing
        .and_then(|i| outcome.financing.get(i))
        .map(|e| e.provider_id.clone());
    Some(BestCandidate {
        variant_id: outcome.variant.variant_id.clone(),
        orientation: outcome.variant.orientation,
        verdict: outcome.verdict,
        net_usd: published.net_usd,
        amount_in_wei: published.amount_in_wei,
        financing_provider: provider,
    })
}

/// Jointly size and finance every variant, and classify the result.
///
/// The three ranking slots are computed from the SAME published figures and
/// never substitute for each other: a negative best is still the best
/// diagnostic (prompt §6: "No elimines un neto negativo porque aún no haya
/// ganador").
pub fn optimize_joint(
    request: &JointRequest,
    variants: &[RouteVariant],
    quoter_for: &mut dyn FnMut(&RouteVariant) -> Box<dyn CycleQuoter>,
) -> Result<JointReport, String> {
    request.config.validate()?;
    if variants.is_empty() {
        return Err("joint_sizing: no route variants supplied".to_string());
    }
    if request.price_usd <= Decimal::ZERO {
        return Err(format!(
            "joint_sizing: non-positive price {}",
            request.price_usd
        ));
    }
    let lower = if request.dust_floor_usd.is_zero() {
        U256::one()
    } else {
        request
            .denomination
            .usd_to_base_units(request.dust_floor_usd, request.price_usd)
            .ok_or_else(|| {
                format!(
                    "joint_sizing: dust floor {} USD is not representable for {} decimals",
                    request.dust_floor_usd,
                    request.denomination.decimals()
                )
            })?
            .max(U256::one())
    };

    let mut outcomes: Vec<VariantOutcome> = Vec::with_capacity(variants.len());
    let mut probes_total = 0usize;
    let mut budget_exhausted = false;

    for variant in variants {
        let mut limits = request.limits.clone();
        limits.extend(variant.extra_limits.iter().cloned());

        let domain = match SizeDomain::resolve(limits, lower) {
            Ok(d) => d,
            Err(reject) => {
                outcomes.push(VariantOutcome {
                    variant: variant.clone(),
                    domain: None,
                    domain_reject: Some(reject),
                    search: None,
                    finalized: None,
                    financing: Vec::new(),
                    best_financing: None,
                    verdict: JointVerdict::Absent,
                    absence_reason: None,
                    execution_eligible: false,
                    search_error: None,
                    split_evidence: None,
                });
                continue;
            }
        };

        // The quoter is either supplied by the caller (on-chain/local adapters)
        // or built here for a split variant — one seam, two producers.
        let mut quoter: Box<dyn CycleQuoter> = match &variant.split {
            Some(split) => Box::new(SplitQuoter::new(
                split.clone(),
                request.denomination,
                request.price_usd,
                request.gas_usd,
                request.ops_usd,
                format!("{}:{}", variant.variant_id, variant.route_label),
            )),
            None => quoter_for(variant),
        };

        let scope = format!(
            "{}|{}|{}",
            variant.variant_id,
            variant.orientation.as_str(),
            variant.route_label
        );

        let mut book = ProbeBook::new();
        let search_result = {
            let mut eval = |x: U256| Sample::from_quote(x, quoter.quote_cycle(x));
            multiscale_search(
                &domain,
                &request.config,
                &request.boundaries_wei,
                &scope,
                &mut book,
                &mut eval,
            )
        };

        let search = match search_result {
            Ok(report) => report,
            Err(error) => {
                outcomes.push(VariantOutcome {
                    variant: variant.clone(),
                    domain: Some(domain),
                    domain_reject: None,
                    search: None,
                    finalized: None,
                    financing: Vec::new(),
                    best_financing: None,
                    verdict: JointVerdict::Absent,
                    absence_reason: None,
                    execution_eligible: false,
                    search_error: Some(error),
                    split_evidence: quoter.split_evidence(),
                });
                continue;
            }
        };
        budget_exhausted |= search.budget_exhausted;

        // ── Candidates to re-quote: the local vertex (a continuous optimum the
        //    grid can never land on exactly) and the grid argmax itself. Both
        //    go through the same rounding + re-quote; only the re-quote decides.
        let mut finalized_candidates: Vec<FinalizedSize> = Vec::new();
        if let Some(best) = &search.best {
            let best_net = best.sample.net_usd();
            if let Some(ideal) = vertex_candidate(&search) {
                let mut eval = |x: U256| Sample::from_quote(x, quoter.quote_cycle(x));
                if let Ok(finalized) = finalize_rounded(
                    ideal,
                    request.rounding_rule,
                    request.rounding_direction,
                    &domain,
                    best_net,
                    &mut book,
                    &mut eval,
                ) {
                    finalized_candidates.push(finalized);
                }
            }
            let grid_ideal = RationalAmount::integer(best.amount_in_wei);
            let mut eval = |x: U256| Sample::from_quote(x, quoter.quote_cycle(x));
            if let Ok(finalized) = finalize_rounded(
                grid_ideal,
                request.rounding_rule,
                request.rounding_direction,
                &domain,
                best_net,
                &mut book,
                &mut eval,
            ) {
                finalized_candidates.push(finalized);
            }
        }

        // The winner is the best RE-QUOTED figure. A candidate whose re-quote
        // was absent never competes (no ideal figure is ever substituted).
        let mut winner: Option<FinalizedSize> = None;
        for candidate in finalized_candidates {
            if candidate.published.is_none() {
                // An unquotable candidate never competes; it is kept only so the
                // absence has a face when nothing else exists.
                if winner.is_none() {
                    winner = Some(candidate);
                }
                continue;
            }
            let candidate_net = candidate
                .published
                .as_ref()
                .map(|p| p.net_usd)
                .unwrap_or(Decimal::MIN);
            let better = match &winner {
                Some(current) => match current.published.as_ref().map(|p| p.net_usd) {
                    Some(current_net) => candidate_net > current_net,
                    None => true,
                },
                None => true,
            };
            if better {
                winner = Some(candidate);
            }
        }

        let (verdict, absence_reason, published_for_financing) = match &winner {
            None => (
                JointVerdict::Absent,
                Some("no_quoted_size_in_domain".to_string()),
                None,
            ),
            Some(finalized) => match &finalized.published {
                Some(published) => (
                    JointVerdict::classify(
                        Some(published.net_usd),
                        request.objective.search_target_usd,
                    ),
                    None,
                    Some(published.clone()),
                ),
                None => (
                    JointVerdict::Absent,
                    Some(format!(
                        "requote_absent_at_rounded_size:{}",
                        finalized
                            .requote_absent_reason
                            .clone()
                            .unwrap_or_else(|| "unspecified".to_string())
                    )),
                    None,
                ),
            },
        };

        let (financing, best_offer) = match &published_for_financing {
            None => (Vec::new(), None),
            Some(published) => {
                let freq = FinancingRequest {
                    denomination: request.denomination,
                    price_usd: request.price_usd,
                    amount_in_wei: published.amount_in_wei,
                    amount_out_wei: published.amount_out_wei,
                    gas_usd: request.gas_usd,
                    ops_usd: request.ops_usd,
                    offers: &request.financing_offers,
                };
                let evals = compare_financing(&freq)?;
                let best = best_financing(&evals);
                (evals, best)
            }
        };

        let execution_eligible = match &published_for_financing {
            Some(published) => {
                published.net_usd > Decimal::ZERO
                    && published.net_usd >= request.objective.execution_min_net_usd
            }
            None => false,
        };

        probes_total += book.len();
        outcomes.push(VariantOutcome {
            variant: variant.clone(),
            domain: Some(domain),
            domain_reject: None,
            search: Some(search),
            finalized: winner,
            financing,
            best_financing: best_offer,
            verdict,
            absence_reason,
            execution_eligible,
            search_error: None,
            split_evidence: quoter.split_evidence(),
        });
    }

    // ── Rankings: best diagnostic / feasible / over target / alternatives ──
    let mut ranked: Vec<BestCandidate> = outcomes.iter().filter_map(candidate_from).collect();
    ranked.sort_by(|a, b| {
        b.net_usd
            .cmp(&a.net_usd)
            .then(a.variant_id.cmp(&b.variant_id))
    });

    let best_diagnostic = ranked.first().cloned();
    let best_feasible = ranked.iter().find(|c| c.net_usd > Decimal::ZERO).cloned();
    let best_over_target = ranked
        .iter()
        .find(|c| c.net_usd >= request.objective.search_target_usd)
        .cloned();
    let mut alternatives: Vec<BestCandidate> = Vec::new();
    for candidate in &ranked {
        if alternatives
            .iter()
            .any(|c: &BestCandidate| c.variant_id == candidate.variant_id)
        {
            continue;
        }
        alternatives.push(candidate.clone());
        if alternatives.len() == 3 {
            break;
        }
    }

    Ok(JointReport {
        outcomes,
        best_diagnostic,
        best_feasible,
        best_over_target,
        alternatives,
        probes_total,
        budget_exhausted,
        scope: format!("variants={}", variants.len()),
        basis: JOINT_SEARCH_BASIS,
    })
}

/// The suite is pulled in with `include!` (rather than `mod tests;`) so the
/// module keeps its tests beside it in `size_optimizer/joint_sizing/tests.rs`
/// while remaining loadable from a harness that includes this file by `#[path]`.
#[cfg(test)]
mod tests {
    include!("joint_sizing/tests.rs");
}
