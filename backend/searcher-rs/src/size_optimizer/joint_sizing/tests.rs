// Tests for `joint_sizing` (prompt §14).
//
// Offline and deterministic by default. The ONE network test is `#[ignore]`d
// and states so in its name; it is run on demand against a real mainnet RPC,
// never in CI. Fixtures here are synthetic BY DESIGN and never reach the
// production stream (prompt §14: "Los fixtures sintéticos están permitidos en
// tests y deben estar aislados del stream y las métricas reales").

use super::*;
use crate::amm_math::v2_amount_out;
use ethers::types::U256;
use rust_decimal::Decimal;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::str::FromStr;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal literal")
}

fn u(v: u64) -> U256 {
    U256::from(v)
}

fn big(s: &str) -> U256 {
    U256::from_dec_str(s).expect("u256 literal")
}

fn weth18() -> TokenDenomination {
    TokenDenomination::new(18).expect("18 decimals")
}

fn value_sample(net: Decimal, amount_in: U256) -> Sample {
    Sample::Value {
        net_usd: net,
        gross_usd: net,
        costs_usd: Decimal::ZERO,
        amount_out_wei: amount_in,
        provenance: "test:fixture".to_string(),
    }
}

fn domain_with_cap(upper: u64, lower: u64) -> SizeDomain {
    SizeDomain::resolve(
        vec![SizeLimit::known(
            SizeLimitKind::AuthorizedCapital,
            "*",
            u(upper),
            "test.cap",
        )],
        u(lower),
    )
    .expect("a domain with one known cap")
}

fn count_quoter(net: &'static str) -> Box<dyn CycleQuoter> {
    Box::new(FnQuoter::new(move |x: U256| {
        QuoteOutcome::Quoted(CycleQuote::new(
            x,
            x,
            dec(net),
            Decimal::ZERO,
            "test:count",
        ))
    }))
}

fn request_with(target: &str, min_net: &str, offers: Vec<CapitalSource>) -> JointRequest {
    JointRequest {
        denomination: weth18(),
        price_usd: dec("2000"),
        limits: vec![SizeLimit::known(
            SizeLimitKind::AuthorizedCapital,
            "*",
            big("1000000000000000000000"),
            "test.cap",
        )],
        dust_floor_usd: dec("0.01"),
        boundaries_wei: Vec::new(),
        config: MultiscaleConfig {
            coarse_points: 8,
            refine_rounds: 1,
            refine_points: 2,
            refine_intervals: 1,
            max_probes: 24,
        },
        objective: JointObjective::new(dec(target), dec(min_net)).expect("objective"),
        financing_offers: offers,
        gas_usd: dec("1"),
        ops_usd: Decimal::ZERO,
        rounding_rule: RoundingRule::MinUnit,
        rounding_direction: RoundingDirection::Down,
    }
}

/// A capacity that covers every fixture principal below (100 WETH).
fn ample_capacity() -> Bound {
    Bound::known(big("100000000000000000000"), "test.provider.capacity")
}

/// The classic golden-section iteration, written here as an INDEPENDENT
/// reference so the hypothesis-violation test can show what it returns on a
/// landscape that violates its assumption. Test-local on purpose: production
/// code must not grow a second optimizer.
fn golden_section_reference<F: FnMut(f64) -> f64>(
    mut f: F,
    mut a: f64,
    mut b: f64,
    iterations: u32,
) -> f64 {
    let inv_phi = (5.0_f64.sqrt() - 1.0) / 2.0;
    let inv_phi2 = 1.0 - inv_phi;
    let mut c = a + inv_phi2 * (b - a);
    let mut d = a + inv_phi * (b - a);
    let mut fc = f(c);
    let mut fd = f(d);
    for _ in 0..iterations {
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = a + inv_phi2 * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + inv_phi * (b - a);
            fd = f(d);
        }
    }
    (a + b) / 2.0
}

// ===========================================================================
// §4 — money, denominations, exact decimals
// ===========================================================================

#[test]
fn denomination_converts_with_explicit_decimal_precision() {
    let usdc = TokenDenomination::new(6).expect("6 decimals");
    assert_eq!(
        usdc.usd_to_base_units(dec("1000"), dec("1")).expect("exact"),
        u(1_000_000_000)
    );
    // A fractional result is FLOORED: the engine never believes it can afford
    // more than the authorized capital buys.
    let wei = weth18()
        .usd_to_base_units(dec("1000"), dec("2698.7975"))
        .expect("representable");
    assert!(wei > big("370000000000000000"), "got {wei}");
    assert!(wei < big("371000000000000000"), "got {wei}");
    let back = weth18()
        .base_units_to_usd(wei, dec("2698.7975"))
        .expect("round trip");
    assert!(back <= dec("1000"), "floored USD must not exceed the cap");

    // 30 digits carry more precision than Decimal holds exactly → ABSENT.
    let huge = big("111111111111111111111111111111");
    assert_eq!(weth18().base_units_to_usd(huge, dec("1")), None);
    // …and the denomination itself is refused beyond the exact range.
    assert!(TokenDenomination::new(29).is_none());
    // A non-positive price can value nothing.
    assert_eq!(weth18().usd_to_base_units(dec("10"), Decimal::ZERO), None);
}

#[test]
fn signed_amounts_keep_the_sign_and_the_magnitude() {
    let up = SignedAmount::from_delta(u(10), u(25));
    assert!(!up.negative);
    assert_eq!(up.magnitude, u(15));
    let down = SignedAmount::from_delta(u(25), u(10));
    assert!(down.negative);
    assert_eq!(down.magnitude, u(15));
    assert_eq!(down.to_signed_usd(weth18(), dec("1")), Some(dec("-0.000000000000000015")));
    assert!(SignedAmount::from_delta(u(7), u(7)).is_zero());
}

// ===========================================================================
// §6 — the honest size domain
// ===========================================================================

#[test]
fn a_domain_without_any_known_upper_bound_is_absent_with_its_reasons() {
    let limits = vec![
        SizeLimit::absent(SizeLimitKind::OwnBalance, "WETH", "balance_not_read"),
        SizeLimit::absent(
            SizeLimitKind::BorrowCapacity,
            "aave-v3",
            "provider_capacity_not_read",
        ),
    ];
    let reject = SizeDomain::resolve(limits, u(1)).expect_err("no upper bound may be invented");
    assert_eq!(reject.as_str(), "domain_no_upper_bound");
    match reject {
        DomainReject::NoUpperBound { absent } => {
            assert_eq!(absent.len(), 2);
            assert_eq!(absent[0].0, SizeLimitKind::OwnBalance);
            assert_eq!(absent[0].1, "balance_not_read");
            assert_eq!(absent[1].1, "provider_capacity_not_read");
        }
        other => panic!("expected NoUpperBound, got {other:?}"),
    }
}

#[test]
fn the_binding_limit_is_the_smallest_known_one_and_absents_stay_reported() {
    let limits = vec![
        SizeLimit::known(
            SizeLimitKind::AuthorizedCapital,
            "*",
            u(1_000),
            "config.capital_usd",
        ),
        SizeLimit::known(
            SizeLimitKind::PoolLiquidity,
            "poolA",
            u(300),
            "reserves.poolA.reserve_in",
        ),
        SizeLimit::absent(
            SizeLimitKind::SlippageBudget,
            "route",
            "slippage_budget_unset",
        ),
    ];
    let domain = SizeDomain::resolve(limits, u(10)).expect("domain");
    assert_eq!(domain.upper(), u(300));
    assert_eq!(domain.binding(), SizeLimitKind::PoolLiquidity);
    assert_eq!(domain.lower(), u(10));
    let absent = domain.absent_limits();
    assert_eq!(absent.len(), 1);
    assert_eq!(absent[0].0, SizeLimitKind::SlippageBudget);
    assert_eq!(absent[0].2, "slippage_budget_unset");
    // A CREDITED zero is a known bound, not an absence.
    let zero_bound = SizeLimit::known(SizeLimitKind::LotGrid, "poolZ", U256::zero(), "grid");
    assert!(zero_bound.bound.value().is_some());
    assert!(zero_bound.bound.reason().is_none());
}

#[test]
fn a_domain_is_refused_when_the_upper_bound_sits_below_the_lower_one() {
    let limits = vec![SizeLimit::known(
        SizeLimitKind::LotGrid,
        "poolX",
        u(5),
        "tick_grid",
    )];
    let reject = SizeDomain::resolve(limits, u(10)).expect_err("upper < lower");
    assert_eq!(reject.as_str(), "domain_upper_below_lower");
    assert!(SizeDomain::resolve(Vec::new(), U256::zero()).is_err());
    assert_eq!(
        SizeDomain::resolve(Vec::new(), U256::zero())
            .expect_err("zero lower")
            .as_str(),
        "domain_zero_lower"
    );
}

// ===========================================================================
// §6/§4 — rounding to minimum units
// ===========================================================================

#[test]
fn rounding_floors_a_fractional_ideal_to_the_minimum_unit() {
    let ideal = RationalAmount::new(u(3), u(2)).expect("3/2");
    assert_eq!(
        round_rational(ideal, RoundingRule::MinUnit, RoundingDirection::Down).expect("down"),
        u(1)
    );
    assert_eq!(
        round_rational(ideal, RoundingRule::MinUnit, RoundingDirection::Up).expect("up"),
        u(2)
    );
    assert_eq!(ideal.floor(), Some(u(1)));
    assert_eq!(ideal.ceil(), Some(u(2)));

    // A coarser PROTOCOL grid (vault shares of 1e6 base units).
    let coarse = RationalAmount::integer(u(3_500_000));
    assert_eq!(
        round_rational(
            coarse,
            RoundingRule::MultipleOf(u(1_000_000)),
            RoundingDirection::Down
        )
        .expect("down"),
        u(3_000_000)
    );
    assert_eq!(
        round_rational(
            coarse,
            RoundingRule::MultipleOf(u(1_000_000)),
            RoundingDirection::Up
        )
        .expect("up"),
        u(4_000_000)
    );

    // Degenerate rules are refused, never guessed.
    assert_eq!(
        round_rational(
            ideal,
            RoundingRule::MultipleOf(U256::zero()),
            RoundingDirection::Down
        ),
        Err(RoundingError::ZeroGranularity)
    );
    assert_eq!(
        round_rational(
            RationalAmount {
                num: u(1),
                den: U256::zero()
            },
            RoundingRule::MinUnit,
            RoundingDirection::Down
        ),
        Err(RoundingError::ZeroDenominator)
    );
}

#[test]
fn rounding_tracks_the_token_decimals_through_the_usd_conversion() {
    // The same 1000 USD lands on a 6-decimals grid and an 18-decimals grid.
    let usdc = TokenDenomination::new(6).expect("6");
    let dai = TokenDenomination::new(18).expect("18");
    let usdc_units = usdc
        .usd_to_base_units(dec("1000"), dec("1"))
        .expect("usdc");
    let dai_units = dai
        .usd_to_base_units(dec("1000"), dec("1"))
        .expect("dai");
    assert_eq!(usdc_units, u(1_000_000_000));
    assert_eq!(dai_units, big("1000000000000000000000"));

    // Rounding DOWN to the minimum unit drops the fraction; the drift is
    // recorded as the exact difference, per token.
    let domain = domain_with_cap(2_000_000_000, 1);
    let ideal_usdc = RationalAmount::new(usdc_units * u(3), u(2)).expect("3/2 of the cap");
    let rounded = round_within_domain(
        ideal_usdc,
        RoundingRule::MinUnit,
        RoundingDirection::Down,
        &domain,
    )
    .expect("inside the domain");
    assert_eq!(rounded.rounded_wei, u(1_500_000_000));
    assert_eq!(rounded.drift.magnitude, U256::zero());
    assert_eq!(rounded.applied_direction, RoundingDirection::Down);
}

#[test]
fn rounding_up_is_replaced_by_the_conservative_floor_at_the_domain_edge() {
    let domain = domain_with_cap(100, 1);
    // ideal 100.4: rounding UP would ask for 101 — more than authorized.
    let ideal = RationalAmount::new(u(1004), u(10)).expect("100.4");
    let rounded = round_within_domain(
        ideal,
        RoundingRule::MinUnit,
        RoundingDirection::Up,
        &domain,
    )
    .expect("clamped inside the domain");
    assert_eq!(rounded.rounded_wei, u(100));
    assert_eq!(rounded.requested_direction, RoundingDirection::Up);
    assert_eq!(rounded.applied_direction, RoundingDirection::Down);
    assert_eq!(
        rounded.adjust_reason,
        Some("up_rounding_exceeded_domain_upper_floor_used")
    );
    assert!(rounded.drift.is_zero(), "floor(100.4) == 100 == rounded");

    // Below the domain's floor is an error, not a silent clamp.
    let tiny = RationalAmount::new(u(1), u(10)).expect("0.1");
    assert!(matches!(
        round_within_domain(tiny, RoundingRule::MinUnit, RoundingDirection::Down, &domain),
        Err(RoundingError::BelowDomain { .. })
    ));
}

// ===========================================================================
// §6 — multiscale exploration, refinement, boundaries, discontinuities
// ===========================================================================

#[test]
fn the_exact_grid_keeps_a_cap_far_beyond_f64_precision() {
    let hi = big("123456789012345678901234567");
    let grid = geometric_grid_exact(u(1), hi, 12);
    assert_eq!(grid.len(), 12);
    assert_eq!(grid[0], u(1));
    assert_eq!(*grid.last().expect("last"), hi, "the cap itself is the top probe");
    for pair in grid.windows(2) {
        assert!(pair[0] < pair[1], "grid must be strictly increasing: {pair:?}");
    }
    // Degenerate bounds keep their honest shape.
    assert_eq!(geometric_grid_exact(u(5), u(5), 4), vec![u(5)]);
    assert_eq!(geometric_grid_exact(u(1), u(10), 2), vec![u(1), u(10)]);
    let linear = linear_grid_exact(u(10), u(20), 3);
    assert_eq!(linear, vec![u(10), u(15), u(20)]);
}

#[test]
fn multiscale_finds_the_global_peak_of_a_bimodal_landscape() {
    // Two plateaus of different height: 60 on [100,200], 100 on [600,900].
    let mut eval = |x: U256| {
        let v = u256_to_f64_lossy(x);
        let net = if (600.0..=900.0).contains(&v) {
            dec("100")
        } else if (100.0..=200.0).contains(&v) {
            dec("60")
        } else {
            dec("1")
        };
        value_sample(net, x)
    };
    let domain = domain_with_cap(1_000, 1);
    let mut book = ProbeBook::new();
    let report = multiscale_search(
        &domain,
        &MultiscaleConfig::default(),
        &[],
        "bimodal",
        &mut book,
        &mut eval,
    )
    .expect("search");

    let best = report.best.as_ref().expect("a computed best");
    assert_eq!(
        best.sample.net_usd(),
        Some(dec("100")),
        "the global peak is the answer, not the wider lower one"
    );
    assert!(
        !report.unimodal_observed,
        "two basins were observed: {}", 
        report.local_maxima.len()
    );
    assert!(report.local_maxima.len() >= 2);
    assert_eq!(
        report.best_positive.as_ref().map(|p| p.amount_in_wei),
        Some(best.amount_in_wei)
    );
    assert!(report.computed_samples() >= report.probes.len() - report.absent_samples);
}

/// Prompt §6: "No presupongas unimodalidad ni uses búsqueda ternaria o Brent
/// como prueba de óptimo global sin sus hipótesis."
///
/// This is the case that VIOLATES the hypothesis, and the reason the motor does
/// not rely on it: an almost flat landscape (99) with a NARROW spike (100)
/// inside [95, 115]. Golden-section narrows away from the spike and returns a
/// strictly worse point; the multiscale search probes the spike and finds it.
#[test]
fn golden_section_misses_the_optimum_that_the_multiscale_search_finds() {
    let objective = |x: f64| -> f64 {
        if (95.0..=115.0).contains(&x) {
            100.0
        } else {
            99.0
        }
    };
    let golden_x = golden_section_reference(objective, 1.0, 1000.0, 60);
    assert!(
        objective(golden_x) < 100.0,
        "the hypothesis-violating case must actually break golden-section (got {} at {golden_x})",
        objective(golden_x)
    );

    let mut eval = |x: U256| {
        let v = u256_to_f64_lossy(x);
        let net = if (95.0..=115.0).contains(&v) {
            dec("100")
        } else {
            dec("99")
        };
        value_sample(net, x)
    };
    let domain = domain_with_cap(1_000, 1);
    let mut book = ProbeBook::new();
    let report = multiscale_search(
        &domain,
        &MultiscaleConfig::default(),
        &[],
        "hypothesis",
        &mut book,
        &mut eval,
    )
    .expect("search");

    let best = report.best.as_ref().expect("a computed best");
    assert_eq!(
        best.sample.net_usd(),
        Some(dec("100")),
        "the multiscale search must reach the narrow optimum golden-section missed"
    );
    let v = u256_to_f64_lossy(best.amount_in_wei);
    assert!(
        (95.0..=115.0).contains(&v),
        "the winner must sit inside the spike, got {v}"
    );
    assert!(!report.unimodal_observed);
}

#[test]
fn refinement_never_subdivides_a_quote_gap() {
    // [200, 400] is a band where the quoter answers nothing at all.
    let mut eval = |x: U256| {
        let v = u256_to_f64_lossy(x);
        if (200.0..=400.0).contains(&v) {
            Sample::Absent {
                reason: "pool_reverted".to_string(),
            }
        } else {
            value_sample(dec("1"), x)
        }
    };
    let domain = domain_with_cap(1_000, 1);
    let mut book = ProbeBook::new();
    let report = multiscale_search(
        &domain,
        &MultiscaleConfig::default(),
        &[],
        "gap",
        &mut book,
        &mut eval,
    )
    .expect("search");

    assert!(
        report.absent_samples > 0,
        "the gap must be OBSERVED as absent, never as a computed zero"
    );
    assert!(
        report
            .probes
            .iter()
            .any(|p| p.sample.absent_reason() == Some("pool_reverted")),
        "the reason travels with the probe"
    );
    for probe in &report.probes {
        if probe.stage == ProbeStage::Refine {
            assert!(
                probe.sample.is_value(),
                "refinement crossed a discontinuity at {}",
                probe.amount_in_wei
            );
        }
    }
    // The published best is still a computed value on the healthy side.
    assert!(report.best.is_some());
}

#[test]
fn a_tick_boundary_is_probed_and_the_step_optimum_is_found() {
    let boundary = u(500);
    let step = |x: U256| {
        if x == boundary {
            value_sample(dec("10"), x)
        } else {
            value_sample(dec("1"), x)
        }
    };

    // WITHOUT the detected boundary the search cannot see the step at all.
    let domain = domain_with_cap(1_000, 1);
    let mut blind_book = ProbeBook::new();
    let mut blind_eval = step;
    let blind = multiscale_search(
        &domain,
        &MultiscaleConfig::default(),
        &[],
        "blind",
        &mut blind_book,
        &mut blind_eval,
    )
    .expect("search");
    assert_eq!(
        blind.best.as_ref().and_then(|p| p.sample.net_usd()),
        Some(dec("1")),
        "without a boundary probe the step is invisible"
    );

    // WITH the boundary in the probe set the optimum is found exactly.
    let mut book = ProbeBook::new();
    let mut eval = step;
    let report = multiscale_search(
        &domain,
        &MultiscaleConfig::default(),
        &[boundary],
        "step",
        &mut book,
        &mut eval,
    )
    .expect("search");
    assert_eq!(report.boundaries_probed, 1);
    let probe = report
        .probes
        .iter()
        .find(|p| p.amount_in_wei == boundary)
        .expect("the boundary must appear as a probe");
    assert_eq!(probe.stage, ProbeStage::Boundary);
    assert_eq!(
        report.best.as_ref().map(|p| p.amount_in_wei),
        Some(boundary),
        "the boundary is where the optimum of a step landscape lives"
    );
}

#[test]
fn a_search_is_bounded_and_reports_a_spent_budget() {
    let mut eval = |x: U256| value_sample(dec("1"), x);
    let domain = domain_with_cap(1_000, 1);
    let cfg = MultiscaleConfig {
        coarse_points: 8,
        refine_rounds: 4,
        refine_points: 4,
        refine_intervals: 2,
        max_probes: 8,
    };
    let mut book = ProbeBook::new();
    let report = multiscale_search(&domain, &cfg, &[], "budget", &mut book, &mut eval).expect("search");
    assert_eq!(report.probes.len(), 8, "the budget is a hard ceiling");
    assert!(report.budget_exhausted, "an exhausted budget is reported");
    // Degenerate knobs fail fast instead of silently changing resolution.
    let bad = MultiscaleConfig {
        coarse_points: 1,
        ..MultiscaleConfig::default()
    };
    assert!(bad.validate().is_err());
    let bad2 = MultiscaleConfig {
        coarse_points: 16,
        max_probes: 4,
        ..MultiscaleConfig::default()
    };
    assert!(bad2.validate().is_err());
}

#[test]
fn the_vertex_candidate_needs_a_concave_bracket() {
    let probe = |x: u64, net: &str| SizeProbe {
        amount_in_wei: u(x),
        sample: value_sample(dec(net), u(x)),
        stage: ProbeStage::Coarse,
    };
    let report_with = |probes: Vec<SizeProbe>| SearchReport {
        probes,
        best: None,
        best_positive: None,
        local_maxima: Vec::new(),
        unimodal_observed: true,
        absent_samples: 0,
        boundaries_probed: 0,
        evaluations: 3,
        max_probes: 3,
        budget_exhausted: false,
        scope: "manual".to_string(),
        basis: JOINT_SEARCH_BASIS,
    };

    let concave = report_with(vec![probe(100, "0"), probe(200, "10"), probe(300, "0")]);
    let vertex = vertex_candidate(&concave).expect("a concave bracket has an interior vertex");
    assert_eq!(vertex.floor(), Some(u(200)), "the symmetric vertex is the centre");

    let convex = report_with(vec![probe(100, "10"), probe(200, "0"), probe(300, "10")]);
    assert_eq!(vertex_candidate(&convex), None, "a convex bracket has no maximum");

    let linear = report_with(vec![probe(100, "0"), probe(200, "5"), probe(300, "10")]);
    assert_eq!(vertex_candidate(&linear), None, "no curvature, no vertex");

    let asymmetric = report_with(vec![probe(100, "0"), probe(200, "10"), probe(300, "5")]);
    let skewed = vertex_candidate(&asymmetric).expect("still concave");
    let floor = skewed.floor().expect("floor");
    assert!(floor > u(200) && floor < u(300), "got {floor}");
}

// ===========================================================================
// §4/§6 — the re-quote IS the published figure
// ===========================================================================

#[test]
fn the_published_figure_is_the_requote_not_the_search_value() {
    let request = request_with("0", "0", Vec::new());
    let variants = vec![RouteVariant::new("v", Orientation::TerminalAToB, "v2")];
    // Every size answers 100 USD the FIRST time; the re-quote (a second call at
    // the same size) returns 99 — the shape of a quote that moved between the
    // search and the publication.
    let calls: Rc<RefCell<HashMap<U256, u32>>> = Rc::new(RefCell::new(HashMap::new()));
    let seen = Rc::clone(&calls);
    let mut quoter_for = move |_v: &RouteVariant| -> Box<dyn CycleQuoter> {
        let seen = Rc::clone(&seen);
        Box::new(FnQuoter::new(move |x: U256| {
            let mut map = seen.borrow_mut();
            let entry = map.entry(x).or_insert(0);
            *entry += 1;
            let net = if *entry == 1 { dec("100") } else { dec("99") };
            QuoteOutcome::Quoted(CycleQuote::new(x, x, net, Decimal::ZERO, "test:drift"))
        }))
    };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");
    let finalized = report.outcomes[0]
        .finalized
        .as_ref()
        .expect("a finalized size");
    let published = finalized
        .published
        .as_ref()
        .expect("the rounded size was re-quoted");
    assert_eq!(published.source, "requote");
    assert_eq!(
        published.net_usd,
        dec("99"),
        "the published figure must come from the RE-QUOTE, not from the search's 100"
    );
    assert_eq!(finalized.ideal_net_usd, Some(dec("100")));
    assert_eq!(published.rounding_effect_usd, Some(dec("-1")));
    assert_eq!(
        published.amount_in_wei, finalized.rounded.rounded_wei,
        "the published size is the rounded one"
    );
    // The re-quote really happened (a forced, second observation).
    assert!(calls.borrow().values().any(|n| *n >= 2));
}

#[test]
fn an_absent_requote_publishes_absence_and_not_the_ideal_figure() {
    let domain = domain_with_cap(1_000, 1);
    let rounded_target = u(400);
    let mut eval = |x: U256| {
        if x == rounded_target {
            Sample::Absent {
                reason: "quoter_reverted_at_rounded_size".to_string(),
            }
        } else {
            value_sample(dec("42"), x)
        }
    };
    let ideal = RationalAmount::new(u(4001), u(10)).expect("400.1");
    let mut book = ProbeBook::new();
    let finalized = finalize_rounded(
        ideal,
        RoundingRule::MinUnit,
        RoundingDirection::Down,
        &domain,
        Some(dec("42")),
        &mut book,
        &mut eval,
    )
    .expect("rounding succeeded");

    assert_eq!(finalized.rounded.rounded_wei, rounded_target);
    assert!(
        finalized.published.is_none(),
        "an unquotable rounded size publishes NOTHING — the ideal's 42 is never substituted"
    );
    assert_eq!(
        finalized.requote_absent_reason.as_deref(),
        Some("quoter_reverted_at_rounded_size")
    );
    assert_eq!(
        finalized.requote_sample.absent_reason(),
        Some("quoter_reverted_at_rounded_size")
    );
    // …and the probe book records the re-quote as a forced observation.
    let requote = book
        .get(&rounded_target)
        .expect("requote probe recorded");
    assert_eq!(requote.stage, ProbeStage::Requote);
}

#[test]
fn a_quoter_that_answers_about_another_size_is_refused() {
    let outcome = QuoteOutcome::Quoted(CycleQuote::new(
        u(999),
        u(1),
        dec("5"),
        Decimal::ZERO,
        "test:foreign",
    ));
    match Sample::from_quote(u(1000), outcome) {
        Sample::Absent { reason } => {
            assert!(
                reason.starts_with("quoter_answered_foreign_size"),
                "got {reason}"
            );
        }
        other => panic!("a foreign-size answer must never be attributed: {other:?}"),
    }
}

// ===========================================================================
// §6/§5 — financing: principal, premium, repayment
// ===========================================================================

fn flash_provider(provider: &str, terms: FeeTerms, capacity: Bound) -> CapitalSource {
    CapitalSource::flash_financing(provider, "WETH", "flashLoanSimple", capacity, terms)
}

#[test]
fn credited_zero_an_absent_fee_and_an_insufficient_provider_are_three_states() {
    let principal = big("1000000000000000000");
    let proceeds = big("1001000000000000000");
    let credited = flash_provider(
        "balancer-v2",
        FeeTerms::CreditedZero {
            source: "vault:protocol_subsidised_zero_fee".to_string(),
        },
        ample_capacity(),
    );
    let absent = flash_provider(
        "aave-v3",
        FeeTerms::Absent {
            reason: "FLASHLOAN_PREMIUM_TOTAL_not_read".to_string(),
        },
        ample_capacity(),
    );
    let short = flash_provider(
        "compound-ish",
        FeeTerms::ExactBps {
            bps: dec("5"),
            source: "provider.contract".to_string(),
        },
        Bound::known(u(10), "test.tiny_capacity"),
    );
    let offers = vec![credited, absent, short];
    let evals = compare_financing(&FinancingRequest {
        denomination: weth18(),
        price_usd: dec("2000"),
        amount_in_wei: principal,
        amount_out_wei: proceeds,
        gas_usd: dec("1"),
        ops_usd: Decimal::ZERO,
        offers: &offers,
    })
    .expect("comparable");

    // (a) A credited zero stays a credited zero, and IS comparable.
    assert!(evals[0].is_computed());
    assert!(evals[0].notes.contains(&"zero_premium_credited"));
    match &evals[0].outcome {
        FinancingOutcome::Computed { premium_wei, .. } => {
            assert_eq!(*premium_wei, U256::zero());
        }
        other => panic!("credited zero must be computed, got {other:?}"),
    }

    // (b) An absent fee is NOT a zero: nothing is priced and a concrete
    //     resolution task is named.
    assert!(!evals[1].is_computed());
    match &evals[1].outcome {
        FinancingOutcome::Absent {
            reason,
            resolution_task,
        } => {
            assert_eq!(reason, "financing_premium_absent");
            assert!(
                resolution_task.contains("FLASHLOAN_PREMIUM_TOTAL"),
                "got {resolution_task}"
            );
        }
        other => panic!("an absent fee must stay absent, got {other:?}"),
    }
    assert_eq!(evals[1].premium_wei.value(), None);
    assert_eq!(evals[1].premium_wei.reason(), Some("financing_premium_absent"));

    // (c) A provider that cannot cover the principal is a THIRD state.
    match &evals[2].outcome {
        FinancingOutcome::Insufficient {
            available_wei,
            needed_wei,
        } => {
            assert_eq!(*available_wei, u(10));
            assert_eq!(*needed_wei, principal);
        }
        other => panic!("expected Insufficient, got {other:?}"),
    }
    assert_eq!(best_financing(&evals), Some(0), "only priced offers compete");
}

#[test]
fn an_absent_provider_capacity_is_absent_with_its_resolution_task() {
    let principal = big("1000000000000000000");
    let offers = vec![flash_provider(
        "aave-v3",
        FeeTerms::ExactBps {
            bps: dec("5"),
            source: "aave:FLASHLOAN_PREMIUM_TOTAL".to_string(),
        },
        Bound::absent("provider_capacity_not_read"),
    )];
    let evals = compare_financing(&FinancingRequest {
        denomination: weth18(),
        price_usd: dec("2000"),
        amount_in_wei: principal,
        amount_out_wei: principal,
        gas_usd: dec("1"),
        ops_usd: Decimal::ZERO,
        offers: &offers,
    })
    .expect("comparable");
    match &evals[0].outcome {
        FinancingOutcome::Absent {
            reason,
            resolution_task,
        } => {
            assert!(reason.starts_with("capacity_absent:"), "got {reason}");
            assert!(resolution_task.contains("available principal"));
        }
        other => panic!("an unknown capacity cannot be assumed sufficient: {other:?}"),
    }
    assert_eq!(best_financing(&evals), None);
}

#[test]
fn the_borrowed_principal_is_never_income() {
    let principal = big("1000000000000000000"); // 1 WETH
    // Zero edge: the proceeds are exactly the principal.
    let offers = vec![flash_provider(
        "aave-v3",
        FeeTerms::ExactBps {
            bps: dec("5"),
            source: "aave:FLASHLOAN_PREMIUM_TOTAL".to_string(),
        },
        ample_capacity(),
    )];
    let evals = compare_financing(&FinancingRequest {
        denomination: weth18(),
        price_usd: dec("2000"),
        amount_in_wei: principal,
        amount_out_wei: principal,
        gas_usd: dec("5"),
        ops_usd: dec("1"),
        offers: &offers,
    })
    .expect("comparable");
    assert!(evals[0].notes.contains(&"principal_is_not_income"));
    match &evals[0].outcome {
        FinancingOutcome::Computed {
            net_usd,
            surplus_usd,
            premium_wei,
            repayment_wei,
        } => {
            // 5 bps of 1 WETH = 0.0005 WETH = 1 USD at 2000.
            assert_eq!(*premium_wei, big("500000000000000"));
            assert_eq!(*repayment_wei, big("1000500000000000000"));
            assert_eq!(*surplus_usd, dec("-1"));
            assert_eq!(*net_usd, dec("-7"), "-1 surplus − 5 gas − 1 ops");
        }
        other => panic!("expected a computed zero-edge borrow, got {other:?}"),
    }

    // Scaling the principal does NOT create profit out of nothing.
    let doubled = compare_financing(&FinancingRequest {
        denomination: weth18(),
        price_usd: dec("2000"),
        amount_in_wei: principal * u(10),
        amount_out_wei: principal * u(10),
        gas_usd: dec("5"),
        ops_usd: dec("1"),
        offers: &offers,
    })
    .expect("comparable");
    let net = doubled[0].net_usd().expect("computed");
    assert!(net < Decimal::ZERO, "a zero-edge borrow is never profitable");
}

#[test]
fn a_bps_premium_rounds_up_and_never_under_charges() {
    let terms = FeeTerms::ExactBps {
        bps: dec("5"),
        source: "aave:FLASHLOAN_PREMIUM_TOTAL".to_string(),
    };
    assert_eq!(terms.premium_wei(u(1)), Some(U256::one()), "ceil(5e-4) = 1");
    assert_eq!(terms.premium_wei(u(10_000)), Some(u(5)));
    assert_eq!(terms.premium_wei(U256::zero()), Some(U256::zero()));
    assert_eq!(
        terms.premium_wei(big("20000000000000000000")),
        Some(big("10000000000000000")),
        "20 WETH at 5 bps = 0.01 WETH"
    );
    // The other two states are not interchangeable with the above.
    assert_eq!(
        FeeTerms::CreditedZero {
            source: "balancer-v2".to_string()
        }
        .premium_wei(u(1)),
        Some(U256::zero())
    );
    assert_eq!(
        FeeTerms::Absent {
            reason: "not read".to_string()
        }
        .premium_wei(u(1)),
        None,
        "an absent fee is NEVER zero"
    );
    assert_eq!(
        FeeTerms::ExactFlatWei {
            amount_wei: u(7),
            source: "flat".to_string()
        }
        .premium_wei(u(1)),
        Some(u(7))
    );
}

#[test]
fn flash_accounting_is_not_flash_financing() {
    let principal = big("1000000000000000000");
    let proceeds = big("1002000000000000000");
    let offers = vec![
        CapitalSource::flash_accounting("uniswap-v4:PoolManager"),
        flash_provider(
            "balancer-v2",
            FeeTerms::CreditedZero {
                source: "vault:zero_fee_by_design".to_string(),
            },
            ample_capacity(),
        ),
    ];
    let evals = compare_financing(&FinancingRequest {
        denomination: weth18(),
        price_usd: dec("2000"),
        amount_in_wei: principal,
        amount_out_wei: proceeds,
        gas_usd: dec("1"),
        ops_usd: Decimal::ZERO,
        offers: &offers,
    })
    .expect("comparable");

    assert!(!evals[0].is_financing, "transient accounting is not a lender");
    assert!(evals[0].notes.contains(&"flash_accounting_is_not_financing"));
    assert!(evals[0].notes.contains(&"no_provider_no_premium"));
    assert!(evals[1].is_financing);

    // Both settle the principal, and neither charges a premium — but the KINDS
    // differ, and the accounting of both is explicit: the principal is repaid,
    // never counted as revenue.
    assert_eq!(evals[0].principal_wei, principal);
    assert_eq!(evals[0].repayment_wei.value(), Some(principal));
    assert_eq!(evals[0].premium_wei.value(), Some(U256::zero()));
    assert_eq!(evals[1].principal_wei, principal);
    assert_eq!(evals[1].repayment_wei.value(), Some(principal));
    assert_eq!(evals[0].kind, "flash_accounting");
    assert_eq!(evals[1].kind, "flash_financing");
    assert_eq!(
        evals[0].net_usd(),
        Some(dec("3")),
        "a 0.002 WETH edge = 4 USD, minus 1 gas"
    );
    assert_eq!(evals[1].net_usd(), Some(dec("3")));
}

#[test]
fn own_capital_is_compared_against_the_providers() {
    let principal = big("1000000000000000000");
    let proceeds = big("1001000000000000000");
    let offers = vec![
        CapitalSource::own_capital(ample_capacity()),
        flash_provider(
            "aave-v3",
            FeeTerms::ExactBps {
                bps: dec("5"),
                source: "aave:FLASHLOAN_PREMIUM_TOTAL".to_string(),
            },
            ample_capacity(),
        ),
    ];
    let evals = compare_financing(&FinancingRequest {
        denomination: weth18(),
        price_usd: dec("2000"),
        amount_in_wei: principal,
        amount_out_wei: proceeds,
        gas_usd: dec("1"),
        ops_usd: Decimal::ZERO,
        offers: &offers,
    })
    .expect("comparable");
    assert!(evals[0].notes.contains(&"own_capital_repaid_to_self"));
    // Own capital pays no premium; the flash provider pays 5 bps = 1 USD.
    assert_eq!(evals[0].net_usd(), Some(dec("1")));
    assert_eq!(evals[1].net_usd(), Some(dec("0")));
    assert_eq!(best_financing(&evals), Some(0));
}

// ===========================================================================
// §6 — split routes over shared pools
// ===========================================================================

fn pool(id: &str, reserve_in: u64, reserve_out: u64, fee_bps: u32) -> (String, CpmmPoolState) {
    (
        id.to_string(),
        CpmmPoolState {
            pool_id: id.to_string(),
            reserve_in: u(reserve_in),
            reserve_out: u(reserve_out),
            fee_bps,
        },
    )
}

#[test]
fn a_split_that_shares_a_pool_is_not_the_sum_of_its_independent_legs() {
    let mut pools = BTreeMap::new();
    pools.insert("P".to_string(), pool("P", 1_000_000, 1_000_000, 30).1);
    pools.insert("X".to_string(), pool("X", 1_000_000, 1_000_000, 30).1);
    pools.insert("Y".to_string(), pool("Y", 1_000_000, 1_000_000, 30).1);
    let split = SharedPoolSplit {
        total_in_wei: u(300_000),
        paths: vec![
            SplitPath {
                path_id: "p1".to_string(),
                pool_ids: vec!["P".to_string(), "X".to_string()],
                share_bps: 5_000,
            },
            SplitPath {
                path_id: "p2".to_string(),
                pool_ids: vec!["P".to_string(), "Y".to_string()],
                share_bps: 5_000,
            },
        ],
        pools,
    };
    let outcome = split.evaluate().expect("joint evaluation");
    assert_eq!(outcome.shared_pools, vec!["P".to_string()]);
    assert_eq!(outcome.execution_order, vec!["p1", "p2"]);
    assert!(outcome.notes.contains(&"shared_pool_consumed_sequentially"));

    // The joint figure is the SMALLER one: the second consumer of P meets the
    // reserves the first one left behind.
    assert!(
        outcome.total_out_joint_wei < outcome.total_out_independent_wei,
        "joint {} independent {}",
        outcome.total_out_joint_wei,
        outcome.total_out_independent_wei
    );
    assert_eq!(
        outcome.overstatement_wei,
        outcome.total_out_independent_wei - outcome.total_out_joint_wei
    );
    // The FIRST path is unaffected (it ran on the untouched state)…
    assert_eq!(
        outcome.per_path_joint_out_wei[0].1,
        outcome.per_path_independent_out_wei[0].1
    );
    // …and the SECOND is strictly worse, which is precisely the error that
    // summing independent legs would publish.
    assert!(
        outcome.per_path_joint_out_wei[1].1 < outcome.per_path_independent_out_wei[1].1
    );

    // Independent control: recompose the joint figure by hand from the state
    // update rules, in the published execution order.
    let a = u(150_000);
    let p_out_1 = v2_amount_out(a, u(1_000_000), u(1_000_000), 30);
    let x_out = v2_amount_out(p_out_1, u(1_000_000), u(1_000_000), 30);
    let p_in_after = u(1_000_000) + a;
    let p_out_after = u(1_000_000) - p_out_1;
    let p_out_2 = v2_amount_out(a, p_in_after, p_out_after, 30);
    let y_out = v2_amount_out(p_out_2, u(1_000_000), u(1_000_000), 30);
    assert_eq!(outcome.total_out_joint_wei, x_out + y_out);
}

#[test]
fn a_split_without_shared_pools_has_no_overstatement() {
    let mut pools = BTreeMap::new();
    for id in ["P", "Q", "X", "Y"] {
        pools.insert(id.to_string(), pool(id, 1_000_000, 1_000_000, 30).1);
    }
    let split = SharedPoolSplit {
        total_in_wei: u(300_000),
        paths: vec![
            SplitPath {
                path_id: "p1".to_string(),
                pool_ids: vec!["P".to_string(), "X".to_string()],
                share_bps: 5_000,
            },
            SplitPath {
                path_id: "p2".to_string(),
                pool_ids: vec!["Q".to_string(), "Y".to_string()],
                share_bps: 5_000,
            },
        ],
        pools,
    };
    let outcome = split.evaluate().expect("joint evaluation");
    assert!(outcome.shared_pools.is_empty());
    assert_eq!(outcome.overstatement_wei, U256::zero());
    assert_eq!(
        outcome.total_out_joint_wei,
        outcome.total_out_independent_wei
    );
}

#[test]
fn a_split_must_conserve_its_allocations_and_know_its_pools() {
    let mut pools = BTreeMap::new();
    pools.insert("P".to_string(), pool("P", 1_000_000, 1_000_000, 30).1);
    let bad_shares = SharedPoolSplit {
        total_in_wei: u(1_000),
        paths: vec![
            SplitPath {
                path_id: "p1".to_string(),
                pool_ids: vec!["P".to_string()],
                share_bps: 5_000,
            },
            SplitPath {
                path_id: "p2".to_string(),
                pool_ids: vec!["P".to_string()],
                share_bps: 4_000,
            },
        ],
        pools: pools.clone(),
    };
    assert_eq!(
        bad_shares.evaluate().expect_err("shares must sum to 10 000"),
        "split_share_conservation_violated:sum_bps=9000"
    );

    let missing_pool = SharedPoolSplit {
        total_in_wei: u(1_000),
        paths: vec![SplitPath {
            path_id: "p1".to_string(),
            pool_ids: vec!["P".to_string(), "GHOST".to_string()],
            share_bps: 10_000,
        }],
        pools,
    };
    assert_eq!(
        missing_pool
            .evaluate()
            .expect_err("an unknown pool cannot be priced"),
        "split_pool_state_absent:GHOST"
    );
}

#[test]
fn a_split_quoter_publishes_the_joint_output_and_exposes_the_comparison() {
    let mut pools = BTreeMap::new();
    pools.insert("P".to_string(), pool("P", 1_000_000, 1_000_000, 30).1);
    pools.insert("X".to_string(), pool("X", 1_000_000, 1_000_000, 30).1);
    pools.insert("Y".to_string(), pool("Y", 1_000_000, 1_000_000, 30).1);
    let split = SharedPoolSplit {
        total_in_wei: u(0),
        paths: vec![
            SplitPath {
                path_id: "p1".to_string(),
                pool_ids: vec!["P".to_string(), "X".to_string()],
                share_bps: 5_000,
            },
            SplitPath {
                path_id: "p2".to_string(),
                pool_ids: vec!["P".to_string(), "Y".to_string()],
                share_bps: 5_000,
            },
        ],
        pools,
    };
    let mut quoter = SplitQuoter::new(
        split,
        weth18(),
        dec("2000"),
        dec("1"),
        Decimal::ZERO,
        "test:split",
    );
    match quoter.quote_cycle(u(300_000)) {
        QuoteOutcome::Quoted(quote) => {
            assert_eq!(quote.amount_in_wei, u(300_000));
            assert!(quote.provenance.ends_with("|joint_split"));
        }
        other => panic!("the joint composition must be quotable: {other:?}"),
    }
    let (joint, independent) = quoter.split_evidence().expect("split evidence");
    assert!(joint < independent, "joint {joint} independent {independent}");
}

// ===========================================================================
// §6 — orientations, variants, and the three ranking slots
// ===========================================================================

#[test]
fn both_orientations_are_searched_and_the_profitable_one_wins() {
    let request = request_with("0", "0", Vec::new());
    let variants = RouteVariant::both_orientations("base", "uniswap-v2>uniswap-v3");
    let mut quoter_for = |v: &RouteVariant| -> Box<dyn CycleQuoter> {
        let profitable = v.orientation == Orientation::TerminalBToA;
        Box::new(FnQuoter::new(move |x: U256| {
            let net = if profitable { dec("10") } else { dec("-10") };
            QuoteOutcome::Quoted(CycleQuote::new(x, x, net, Decimal::ZERO, "test:orientation"))
        }))
    };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");
    assert_eq!(report.outcomes.len(), 2, "the losing orientation is kept");
    let best = report.best_diagnostic.as_ref().expect("a diagnostic");
    assert_eq!(best.orientation, Orientation::TerminalBToA);
    assert_eq!(best.net_usd, dec("10"));
    assert_eq!(best.variant_id, "base:b_to_a");
    assert!(report.best_feasible.is_some());
    // The a→b orientation is a MEASURED loss, not an absence.
    let losing = report
        .outcomes
        .iter()
        .find(|o| o.variant.orientation == Orientation::TerminalAToB)
        .expect("the losing variant");
    assert_eq!(losing.verdict.token(), "loss");
    assert_eq!(losing.verdict.net_usd(), Some(dec("-10")));
}

#[test]
fn a_variant_that_changes_pool_is_compared_against_the_baseline() {
    let request = request_with("0", "0", Vec::new());
    let variants = vec![
        RouteVariant::new("baseline", Orientation::TerminalAToB, "v2>v2")
            .with_change("baseline"),
        RouteVariant::new("pool-swap", Orientation::TerminalAToB, "v2>v2")
            .with_change("pool")
            .with_limit(SizeLimit::known(
                SizeLimitKind::PoolLiquidity,
                "poolB",
                big("500000000000000"),
                "reserves.poolB",
            )),
    ];
    let mut quoter_for = |v: &RouteVariant| -> Box<dyn CycleQuoter> {
        let net = if v.variant_id == "pool-swap" {
            dec("25")
        } else {
            dec("5")
        };
        Box::new(FnQuoter::new(move |x: U256| {
            QuoteOutcome::Quoted(CycleQuote::new(x, x, net, Decimal::ZERO, "test:variant"))
        }))
    };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");
    let best = report.best_diagnostic.as_ref().expect("diagnostic");
    assert_eq!(best.variant_id, "pool-swap");
    assert_eq!(best.net_usd, dec("25"));
    assert_eq!(report.alternatives.len(), 2, "both variants are alternatives");

    // The variant's OWN limit binds it, independently of the shared request.
    let swapped = report
        .outcomes
        .iter()
        .find(|o| o.variant.variant_id == "pool-swap")
        .expect("variant");
    let domain = swapped.domain.as_ref().expect("domain");
    assert_eq!(domain.upper(), big("500000000000000"));
    assert_eq!(domain.binding(), SizeLimitKind::PoolLiquidity);
}

#[test]
fn an_all_negative_scope_keeps_the_best_diagnostic_and_claims_no_winner() {
    let request = request_with("50", "50", Vec::new());
    let variants = vec![
        RouteVariant::new("a", Orientation::TerminalAToB, "v2"),
        RouteVariant::new("b", Orientation::TerminalAToB, "v3"),
    ];
    let mut quoter_for = |v: &RouteVariant| -> Box<dyn CycleQuoter> {
        let net = if v.variant_id == "a" { dec("-7.91") } else { dec("-12") };
        Box::new(FnQuoter::new(move |x: U256| {
            QuoteOutcome::Quoted(CycleQuote::new(x, x, net, Decimal::ZERO, "test:negative"))
        }))
    };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");
    let diagnostic = report.best_diagnostic.as_ref().expect("the loss is kept");
    assert_eq!(diagnostic.net_usd, dec("-7.91"));
    assert_eq!(diagnostic.verdict.token(), "loss");
    assert!(report.best_feasible.is_none());
    assert!(report.best_over_target.is_none());
    // The gap is measured against the configured objective, not a relaxed one.
    match diagnostic.verdict {
        JointVerdict::Loss { .. } => {}
        other => panic!("expected a loss, got {other:?}"),
    }
}

#[test]
fn net_zero_below_target_and_the_threshold_edge_are_distinct_states() {
    let target = dec("50");
    assert_eq!(
        JointVerdict::classify(Some(dec("50")), target).token(),
        "target_met"
    );
    match JointVerdict::classify(Some(dec("49.99")), target) {
        JointVerdict::BelowTarget { net_usd, gap_usd } => {
            assert_eq!(net_usd, dec("49.99"));
            assert_eq!(gap_usd, dec("0.01"));
        }
        other => panic!("expected BelowTarget, got {other:?}"),
    }
    assert_eq!(
        JointVerdict::classify(Some(Decimal::ZERO), target).token(),
        "net_zero"
    );
    assert_eq!(
        JointVerdict::classify(Some(dec("-0.01")), target).token(),
        "loss"
    );
    assert_eq!(JointVerdict::classify(None, target).token(), "absent");
    // Exactly zero is NOT "target met", even with a zero objective.
    assert_eq!(
        JointVerdict::classify(Some(Decimal::ZERO), Decimal::ZERO).token(),
        "net_zero"
    );
    assert_eq!(
        JointVerdict::classify(Some(dec("0.2")), Decimal::ZERO).token(),
        "target_met"
    );
}

#[test]
fn the_search_objective_and_the_execution_minimum_are_independent() {
    // Prompt §7: a 0.20 net may be positive without being the objective met nor
    // authorizing execution.
    let request = request_with("5", "50", Vec::new());
    let variants = vec![RouteVariant::new("small", Orientation::TerminalAToB, "v2")];
    let mut quoter_for = |_v: &RouteVariant| -> Box<dyn CycleQuoter> {
        Box::new(FnQuoter::new(|x: U256| {
            QuoteOutcome::Quoted(CycleQuote::new(
                x,
                x,
                dec("0.2"),
                Decimal::ZERO,
                "test:small",
            ))
        }))
    };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");
    let outcome = &report.outcomes[0];
    assert_eq!(outcome.verdict.token(), "below_target");
    assert!(
        !outcome.execution_eligible,
        "0.20 USD is below the 50 USD hard minimum"
    );
    assert!(report.best_feasible.is_some(), "positive, but not over target");
    assert!(report.best_over_target.is_none());

    // A big net below a bigger objective is executable but still below target.
    let request2 = request_with("100", "50", Vec::new());
    let variants2 = vec![RouteVariant::new("big", Orientation::TerminalAToB, "v2")];
    let mut quoter_for2 = |_v: &RouteVariant| -> Box<dyn CycleQuoter> {
        Box::new(FnQuoter::new(|x: U256| {
            QuoteOutcome::Quoted(CycleQuote::new(x, x, dec("60"), Decimal::ZERO, "test:big"))
        }))
    };
    let report2 = optimize_joint(&request2, &variants2, &mut quoter_for2).expect("joint report");
    let outcome2 = &report2.outcomes[0];
    assert_eq!(outcome2.verdict.token(), "below_target");
    assert!(outcome2.execution_eligible);
    assert!(report2.best_over_target.is_none());
    match outcome2.verdict {
        JointVerdict::BelowTarget { gap_usd, .. } => assert_eq!(gap_usd, dec("40")),
        other => panic!("expected BelowTarget, got {other:?}"),
    }

    // Negative thresholds are refused outright — never clamped to zero.
    assert!(JointObjective::new(dec("-1"), dec("0")).is_err());
    assert!(JointObjective::new(dec("0"), dec("-1")).is_err());
}

#[test]
fn an_incomplete_leg_yields_absence_and_never_a_zero_premium() {
    let request = request_with(
        "5",
        "0",
        vec![flash_provider(
            "aave-v3",
            FeeTerms::Absent {
                reason: "FLASHLOAN_PREMIUM_TOTAL_not_read".to_string(),
            },
            ample_capacity(),
        )],
    );
    let variants = vec![RouteVariant::new("v", Orientation::TerminalAToB, "v2>v3")];
    let mut quoter_for = |_v: &RouteVariant| -> Box<dyn CycleQuoter> {
        Box::new(FnQuoter::new(|_x: U256| QuoteOutcome::Absent {
            reason: "leg2_no_quote".to_string(),
        }))
    };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");
    let outcome = &report.outcomes[0];
    assert_eq!(outcome.verdict, JointVerdict::Absent);
    assert_eq!(
        outcome.absence_reason.as_deref(),
        Some("no_quoted_size_in_domain")
    );
    assert!(
        outcome.financing.is_empty(),
        "no proceeds means NO financing evaluation — and never a fabricated zero premium"
    );
    assert!(outcome.finalized.is_none());
    let search = outcome.search.as_ref().expect("search ran");
    assert_eq!(search.computed_samples(), 0);
    assert_eq!(search.absent_samples, search.probes.len());
    assert!(search
        .probes
        .iter()
        .all(|p| p.sample.absent_reason() == Some("leg2_no_quote")));
}

#[test]
fn a_variant_without_a_resolvable_domain_does_not_stop_the_others() {
    let mut request = request_with("0", "0", Vec::new());
    request.limits = vec![SizeLimit::absent(
        SizeLimitKind::OwnBalance,
        "WETH",
        "balance_not_read",
    )];
    let variants = vec![
        RouteVariant::new("blind", Orientation::TerminalAToB, "v2"),
        RouteVariant::new("bounded", Orientation::TerminalAToB, "v2").with_limit(
            SizeLimit::known(
                SizeLimitKind::AuthorizedCapital,
                "*",
                big("1000000000000000000"),
                "test.cap",
            ),
        ),
    ];
    let mut quoter_for = |_v: &RouteVariant| -> Box<dyn CycleQuoter> { count_quoter("3") };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");

    let blind = &report.outcomes[0];
    assert_eq!(blind.verdict, JointVerdict::Absent);
    let reject = blind.domain_reject.as_ref().expect("domain rejected");
    assert_eq!(reject.as_str(), "domain_no_upper_bound");
    assert!(blind.search.is_none(), "no search over an invented range");

    let bounded = &report.outcomes[1];
    assert!(bounded.domain.is_some());
    assert!(bounded.finalized.is_some());
    assert_eq!(report.best_diagnostic.as_ref().map(|b| b.variant_id.clone()), Some("bounded".to_string()));
}

#[test]
fn the_joint_driver_reports_split_evidence_and_a_bounded_budget() {
    let mut pools = BTreeMap::new();
    // Unit-scale pools whose depth is a MATERIAL fraction of the sizes the
    // domain admits, so the shared pool is genuinely consumed.
    for id in ["P", "X", "Y"] {
        pools.insert(id.to_string(), pool(id, 100_000, 100_000, 30).1);
    }
    let split = SharedPoolSplit {
        total_in_wei: u(0),
        paths: vec![
            SplitPath {
                path_id: "p1".to_string(),
                pool_ids: vec!["P".to_string(), "X".to_string()],
                share_bps: 5_000,
            },
            SplitPath {
                path_id: "p2".to_string(),
                pool_ids: vec!["P".to_string(), "Y".to_string()],
                share_bps: 5_000,
            },
        ],
        pools,
    };
    let mut request = request_with("0", "0", Vec::new());
    // The fixture pools are unit-scale (1e5 base units, 18 decimals, $2000), so
    // the domain is stated in those units: the dust floor is set so the SMALLEST
    // publishable notional already consumes a material share of the shared
    // pool. (At a size where the consumption rounds below one base unit the
    // joint and independent sums must coincide — that is arithmetic, not a
    // defect — so the fixture is built where the effect is representable.)
    request.dust_floor_usd = dec("0.00000000002"); // → 10 000 wei at $2000, 18 dp
    request.limits = vec![SizeLimit::known(
        SizeLimitKind::PoolLiquidity,
        "P",
        u(20_000),
        "test.shared_pool_depth",
    )];
    let variants = vec![
        RouteVariant::new("split", Orientation::TerminalAToB, "v2-split").with_split(split)
    ];
    let mut quoter_for = |_v: &RouteVariant| -> Box<dyn CycleQuoter> {
        panic!("a split variant builds its own quoter")
    };
    let report = optimize_joint(&request, &variants, &mut quoter_for).expect("joint report");
    let outcome = &report.outcomes[0];
    let (joint, independent) = outcome.split_evidence.expect("split evidence travels");
    assert!(
        joint < independent,
        "the joint state must be strictly smaller when the shared pool is materially consumed"
    );
    assert!(report.probes_total > 0);
    // Every probe stayed inside the authorized domain.
    let search = outcome.search.as_ref().expect("search ran");
    for probe in &search.probes {
        assert!(probe.amount_in_wei <= u(20_000));
        assert!(probe.amount_in_wei >= u(10_000));
    }
}

// ===========================================================================
// §14 — LIVE data: the protocol's own quoter, across real tick boundaries
// ===========================================================================

/// A [`CycleQuoter`] over the REAL mainnet Uniswap-V3 QuoterV2 (via the repo's
/// aggregate3/Multicall3 path). The cycle closes in WETH — WETH → USDC → WETH
/// through one pool — so the surplus is directly comparable in wei and the
/// objective needs no external price. NO signer, no broadcast, no capital:
/// `eth_call` only (read-only).
struct RealV3Quoter {
    rt: tokio::runtime::Runtime,
    provider: std::sync::Arc<shared_rs::rpc_failover::AlloyHttpProvider>,
    quoter: ethers::types::Address,
    multicall: ethers::types::Address,
    pool: ethers::types::Address,
    weth: ethers::types::Address,
    usdc: ethers::types::Address,
    fee: u32,
    denomination: TokenDenomination,
    price_weth_usd: Decimal,
    provenance: String,
    legs_quoted: usize,
}

impl RealV3Quoter {
    fn new(rpc: &str) -> Option<Self> {
        use alloy::providers::ProviderBuilder;
        let url = rpc.trim().parse().ok()?;
        let provider: std::sync::Arc<shared_rs::rpc_failover::AlloyHttpProvider> =
            std::sync::Arc::new(
                ProviderBuilder::new()
                    .disable_recommended_fillers()
                    .connect_http(url),
            );
        let (quoter, multicall) = crate::v3_quote_provider::resolve_quoter_multicall(1)?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        Some(Self {
            rt,
            provider,
            quoter,
            multicall,
            // Canonical mainnet USDC/WETH 0.05% pool (token0=USDC, token1=WETH).
            pool: ethers::types::Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").ok()?,
            weth: ethers::types::Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").ok()?,
            usdc: ethers::types::Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").ok()?,
            fee: 500,
            denomination: weth18(),
            price_weth_usd: dec("2000"),
            provenance: "uniswap-v3:quoter_v2_onchain:usdc-weth-0.05%".to_string(),
            legs_quoted: 0,
        })
    }

    fn one_leg(&mut self, token_in: ethers::types::Address, token_out: ethers::types::Address, amount: U256) -> Option<U256> {
        let request = crate::amm_math::V3QuoteRequest {
            pool_addr: self.pool,
            token_in,
            token_out,
            amount_in: amount,
            fee_bps: self.fee,
        };
        let results = self
            .rt
            .block_on(crate::amm_math::v3_quote_exact_in_multicall(
                self.provider.clone(),
                self.quoter,
                self.multicall,
                vec![request],
            ))
            .ok()?;
        self.legs_quoted += 1;
        let first = results.into_iter().next()?;
        if !first.success || first.amount_out.is_zero() {
            None
        } else {
            Some(first.amount_out)
        }
    }
}

impl CycleQuoter for RealV3Quoter {
    fn quote_cycle(&mut self, amount_in_wei: U256) -> QuoteOutcome {
        let (weth, usdc) = (self.weth, self.usdc);
        let Some(usdc_out) = self.one_leg(weth, usdc, amount_in_wei) else {
            return QuoteOutcome::Absent {
                reason: "leg1_quote_failed".to_string(),
            };
        };
        let Some(weth_back) = self.one_leg(usdc, weth, usdc_out) else {
            return QuoteOutcome::Absent {
                reason: "leg2_quote_failed".to_string(),
            };
        };
        let Some(out_usd) = self.denomination.base_units_to_usd(weth_back, self.price_weth_usd) else {
            return QuoteOutcome::Absent {
                reason: "out_of_decimal_range".to_string(),
            };
        };
        let Some(in_usd) = self.denomination.base_units_to_usd(amount_in_wei, self.price_weth_usd) else {
            return QuoteOutcome::Absent {
                reason: "in_of_decimal_range".to_string(),
            };
        };
        QuoteOutcome::Quoted(CycleQuote::new(
            amount_in_wei,
            weth_back,
            out_usd - in_usd,
            Decimal::ZERO,
            self.provenance.clone(),
        ))
    }
}

/// LIVE_DATA_VERIFIED (when run): real QuoterV2 quotes across real tick
/// boundaries, and a published figure that is the RE-QUOTE at the rounded size.
///
/// Run on demand (never in CI, which has no RPC):
///
/// ```text
/// $env:ARBX_V3_QUOTE_TEST_RPC="https://ethereum-rpc.publicnode.com"
/// cargo test -p searcher-rs --lib joint_sizing::tests::real_quoter -- --ignored --nocapture
/// ```
#[test]
#[ignore = "network: requires ARBX_V3_QUOTE_TEST_RPC (real mainnet RPC)"]
fn real_quoter_v2_tick_crossing_and_requote() {
    let Ok(rpc) = std::env::var("ARBX_V3_QUOTE_TEST_RPC") else {
        eprintln!("skipped: ARBX_V3_QUOTE_TEST_RPC unset");
        return;
    };
    let mut quoter = RealV3Quoter::new(&rpc).expect("a real quoter");

    // A ladder spanning 0.001 → 1000 WETH: the larger sizes cross many ticks of
    // the concentrated pool, so the average rate must degrade with size. That
    // monotonic degradation IS the tick/liquidity crossing, measured on the
    // protocol's own quoter rather than on a CPMM stand-in.
    let ladder: Vec<U256> = vec![
        big("1000000000000000"),
        big("10000000000000000"),
        big("100000000000000000"),
        big("1000000000000000000"),
        big("10000000000000000000"),
        big("100000000000000000000"),
        big("1000000000000000000000"),
    ];
    let mut previous: Option<(U256, U256)> = None;
    for size in &ladder {
        match quoter.quote_cycle(*size) {
            QuoteOutcome::Quoted(quote) => {
                assert_eq!(
                    quote.amount_in_wei, *size,
                    "the quoter must answer about the size it was asked"
                );
                assert!(
                    quote.amount_out_wei < *size,
                    "a round trip through one pool must lose to fees + impact: in {size} out {}",
                    quote.amount_out_wei
                );
                if let Some((previous_out, previous_in)) = previous {
                    assert!(
                        quote.amount_out_wei * previous_in < previous_out * *size,
                        "the average rate must degrade with size (cross-multiplied): \
                         prev {previous_out}/{previous_in} vs now {}/{}",
                        quote.amount_out_wei,
                        size
                    );
                }
                println!(
                    "LIVE quoter_v2 size_in_wei={size} out_wei={} net_usd={}",
                    quote.amount_out_wei, quote.net_usd
                );
                previous = Some((quote.amount_out_wei, *size));
            }
            QuoteOutcome::Absent { reason } => {
                panic!("the real quoter must answer at {size}: {reason}")
            }
        }
    }

    // Now size the same real cycle through the motor.
    let domain = SizeDomain::resolve(
        vec![SizeLimit::known(
            SizeLimitKind::PoolLiquidity,
            "0x88e6…5640",
            big("1000000000000000000000"),
            "live.ladder.max",
        )],
        big("1000000000000000"),
    )
    .expect("a bounded live domain");
    let cfg = MultiscaleConfig {
        coarse_points: 6,
        refine_rounds: 1,
        refine_points: 2,
        refine_intervals: 1,
        max_probes: 10,
    };
    let mut book = ProbeBook::new();
    let report = {
        let mut eval = |x: U256| Sample::from_quote(x, quoter.quote_cycle(x));
        multiscale_search(&domain, &cfg, &[], "live", &mut book, &mut eval).expect("search")
    };
    assert!(report.computed_samples() > 0, "real quotes must be computed");
    let best = report.best.clone().expect("a computed best on live data");
    println!(
        "LIVE search best size_in_wei={} net_usd={:?} probes={} local_maxima={} unimodal={}",
        best.amount_in_wei,
        best.sample.net_usd(),
        report.probes.len(),
        report.local_maxima.len(),
        report.unimodal_observed
    );

    // The published figure must be the RE-QUOTE at the rounded size…
    let ideal = RationalAmount::integer(best.amount_in_wei);
    let mut eval = |x: U256| Sample::from_quote(x, quoter.quote_cycle(x));
    let finalized = finalize_rounded(
        ideal,
        RoundingRule::MinUnit,
        RoundingDirection::Down,
        &domain,
        best.sample.net_usd(),
        &mut book,
        &mut eval,
    )
    .expect("rounding inside the live domain");
    let published = finalized.published.clone().expect("re-quoted on live data");
    assert_eq!(published.source, "requote");
    assert_eq!(published.amount_in_wei, finalized.rounded.rounded_wei);
    assert_eq!(published.amount_in_wei, best.amount_in_wei);

    // …and an INDEPENDENT single quote at that same size must reproduce it
    // exactly. This is what makes the published number the re-quote's.
    match quoter.quote_cycle(published.amount_in_wei) {
        QuoteOutcome::Quoted(control) => {
            assert_eq!(
                control.net_usd, published.net_usd,
                "the published figure must be reproducible by a direct quote at the published size"
            );
            assert_eq!(control.amount_out_wei, published.amount_out_wei);
        }
        QuoteOutcome::Absent { reason } => panic!("control quote failed: {reason}"),
    }
    println!(
        "LIVE requote size_in_wei={} net_usd={} ideal_net_usd={:?} legs_quoted={}",
        published.amount_in_wei, published.net_usd, finalized.ideal_net_usd, quoter.legs_quoted
    );
}
