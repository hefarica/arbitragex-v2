//! Exact finite-size evaluation.
//!
//! PROMPT §6: *"Los pesos marginales `-log(rate_after_fee)` pueden generar
//! candidatos en los dominios apropiados, pero no prueban beneficio a tamaño
//! finito."* This module is the part that **does** decide: it walks a cycle at
//! an explicit input size with integer arithmetic, and returns
//! `gross − costs` in the start token's minimal units.
//!
//! ## What is counted and what is not
//!
//! * `gross_raw` = `amount_out − amount_in`, both in the start token's minimal
//!   units, because a closed cycle returns to its start token.
//! * `financing_raw` = the premium owed on the borrowed principal, computed as
//!   a floored fraction of `amount_in`. Self-funded cycles pass
//!   [`Bps::ZERO`](crate::units::Bps::ZERO) and the term is an exact zero — a
//!   *credited* zero, kept as a component rather than dropped.
//! * `unconditional_raw` = costs paid whatever the outcome (gas estimate). It is
//!   **not** slippage tolerance: PROMPT §5 forbids booking a configured
//!   tolerance as an expense, so no tolerance type exists here.
//!
//! The fee is already inside `amount_out` (the AMM keeps it), so it is never
//! subtracted a second time — `CycleEval` records that explicitly through
//! [`CycleEval::fees_embedded_in_quote`].
//!
//! ## Shared pools
//!
//! Two candidate cycles that traverse the same pool consume the *same*
//! liquidity. Summing their independently quoted outputs is therefore wrong.
//! [`PoolLedger`] carries mutable reserves and [`evaluate_split_joint`] applies
//! the legs in order against the joint state; it also returns the naive
//! independent sum so the overstatement is visible instead of assumed.

use crate::cycles::Cycle;
use crate::graph::{EdgeId, MultiGraph, PoolIndex, ProtocolVersion, TokenId};
use crate::units::{Bps, Raw};
use core::fmt;

/// Why an evaluation could not produce a number.
///
/// Every variant is a *reason*, never a substitute for a value: a failed
/// evaluation is `NotEvaluated(reason)` upstream, never `net = 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// A negative amount was supplied where a non-negative one is required.
    NegativeAmount { stage: &'static str },
    /// A checked integer operation overflowed `i128`.
    Overflow { stage: &'static str },
    /// A reserve participating in the swap was computed as exactly zero.
    ZeroReserve { pool_id: u64, side: &'static str },
    /// The edge id is not part of the graph.
    UnknownEdge(EdgeId),
    /// Consecutive hops do not connect, or the cycle does not close.
    OpenPath { expected: TokenId, got: TokenId },
    /// A node repeats inside the cycle.
    NotSimple,
    /// The cycle has no hops.
    EmptyCycle,
    /// No size domain was configured, so nothing could be quoted at finite size.
    NoSizeDomain,
    /// This version's swap curve is not implemented here and must be quoted by
    /// the protocol's own quoter (V3 ticks, StableSwap `D`, weighted vault).
    CurveNotImplemented { pool_id: u64, version: ProtocolVersion },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::NegativeAmount { stage } => write!(f, "negative_amount:{stage}"),
            EvalError::Overflow { stage } => write!(f, "overflow:{stage}"),
            EvalError::ZeroReserve { pool_id, side } => {
                write!(f, "zero_reserve:pool={pool_id}:{side}")
            }
            EvalError::UnknownEdge(id) => write!(f, "unknown_edge:{id}"),
            EvalError::OpenPath { expected, got } => {
                write!(f, "open_path:expected={expected}:got={got}")
            }
            EvalError::NotSimple => write!(f, "not_simple"),
            EvalError::EmptyCycle => write!(f, "empty_cycle"),
            EvalError::NoSizeDomain => write!(f, "no_size_domain"),
            EvalError::CurveNotImplemented { pool_id, version } => {
                write!(f, "curve_not_implemented:pool={pool_id}:{}", version.as_str())
            }
        }
    }
}

/// The non-negative, unconditional part of the cost model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExternalCosts {
    /// Costs paid regardless of the outcome, in start-token minimal units.
    pub unconditional_raw: Raw,
    /// Financing premium charged on the borrowed principal, in bps.
    pub financing_premium_bps: Bps,
}

impl ExternalCosts {
    /// No external cost at all — the honest baseline, not a mock.
    pub const NONE: ExternalCosts = ExternalCosts {
        unconditional_raw: Raw::ZERO,
        financing_premium_bps: Bps::ZERO,
    };

    /// Self-funded execution: no premium, but gas still applies.
    pub const fn self_funded(unconditional_raw: Raw) -> Self {
        ExternalCosts {
            unconditional_raw,
            financing_premium_bps: Bps::ZERO,
        }
    }

    pub const fn flash_financed(unconditional_raw: Raw, premium_bps: Bps) -> Self {
        ExternalCosts {
            unconditional_raw,
            financing_premium_bps: premium_bps,
        }
    }
}

/// The exact result of evaluating one cycle at one size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleEval {
    pub amount_in: Raw,
    pub amount_out: Raw,
    /// `amount_out − amount_in`, in start-token minimal units (may be negative).
    pub gross_raw: Raw,
    pub financing_raw: Raw,
    pub unconditional_raw: Raw,
    /// `financing_raw + unconditional_raw` — every applicable component resolved.
    pub costs_raw: Raw,
    /// `gross_raw − costs_raw`. Negative is a real computed loss.
    pub net_raw: Raw,
    pub hops: usize,
    /// Always `true`: the swap fee lives inside `amount_out` and is never
    /// subtracted again. Kept as an explicit, assertable contract.
    pub fees_embedded_in_quote: bool,
}

/// Constant-product quote with the protocol-favouring floor.
///
/// `out = floor(r_out · floor(x·(1−f)) / (r_in + floor(x·(1−f))))`
pub fn quote_constant_product(
    pool_id: u64,
    fee_bps: Bps,
    reserve_in: Raw,
    reserve_out: Raw,
    amount_in: Raw,
) -> Result<Raw, EvalError> {
    if amount_in.is_negative() {
        return Err(EvalError::NegativeAmount { stage: "swap_in" });
    }
    if reserve_in.is_zero() {
        return Err(EvalError::ZeroReserve {
            pool_id,
            side: "input",
        });
    }
    if reserve_out.is_zero() {
        return Err(EvalError::ZeroReserve {
            pool_id,
            side: "output",
        });
    }
    let effective_in = fee_bps
        .apply_floor(amount_in)
        .ok_or(EvalError::Overflow { stage: "fee_apply" })?;
    if effective_in.is_zero() {
        // A dust trade that cannot pay a whole unit out is a *computed* zero,
        // not a missing value.
        return Ok(Raw::ZERO);
    }
    let denominator = reserve_in
        .checked_add(effective_in)
        .ok_or(EvalError::Overflow {
            stage: "cpmm_denominator",
        })?;
    let numerator = reserve_out
        .checked_mul_int(effective_in.get())
        .ok_or(EvalError::Overflow {
            stage: "cpmm_numerator",
        })?;
    numerator
        .div_floor(denominator.get())
        .ok_or(EvalError::Overflow {
            stage: "cpmm_division",
        })
}

/// A snapshot of pool reserves that **mutates as trades are applied**, so
/// routes sharing a pool see each other's impact.
#[derive(Debug, Clone)]
pub struct PoolLedger<'a> {
    g: &'a MultiGraph,
    reserves: Vec<(Raw, Raw)>,
}

impl<'a> PoolLedger<'a> {
    /// Start from the graph's own (snapshot) reserves.
    pub fn new(g: &'a MultiGraph) -> Self {
        let reserves = g
            .pools()
            .iter()
            .map(|p| (p.reserve0, p.reserve1))
            .collect();
        PoolLedger { g, reserves }
    }

    pub fn reserves_of(&self, pool: PoolIndex) -> (Raw, Raw) {
        self.reserves[pool]
    }

    fn quote_at(
        &self,
        pool: PoolIndex,
        zero_for_one: bool,
        amount_in: Raw,
    ) -> Result<Raw, EvalError> {
        let pool_spec = self.g.pool(pool);
        match pool_spec.version {
            ProtocolVersion::V2 => {
                let (r0, r1) = self.reserves[pool];
                let (r_in, r_out) = if zero_for_one { (r0, r1) } else { (r1, r0) };
                quote_constant_product(pool_spec.pool_id, pool_spec.fee_bps, r_in, r_out, amount_in)
            }
            other => Err(EvalError::CurveNotImplemented {
                pool_id: pool_spec.pool_id,
                version: other,
            }),
        }
    }

    /// Quote one edge against the **current** ledger state without mutating it.
    pub fn quote_edge(&self, edge_id: EdgeId, amount_in: Raw) -> Result<Raw, EvalError> {
        let e = *self.g.edge(edge_id);
        self.quote_at(e.pool, e.zero_for_one, amount_in)
    }

    /// Quote and apply one edge, moving the pool's reserves.
    ///
    /// The fee stays in the pool: the input side grows by the full `amount_in`
    /// and the output side shrinks by `amount_out`.
    pub fn apply_edge(&mut self, edge_id: EdgeId, amount_in: Raw) -> Result<Raw, EvalError> {
        if edge_id >= self.g.edges().len() {
            return Err(EvalError::UnknownEdge(edge_id));
        }
        let e = *self.g.edge(edge_id);
        let out = self.quote_at(e.pool, e.zero_for_one, amount_in)?;
        let (r0, r1) = self.reserves[e.pool];
        let updated = if e.zero_for_one {
            (
                r0.checked_add(amount_in)
                    .ok_or(EvalError::Overflow { stage: "reserve_in" })?,
                r1.checked_sub(out)
                    .ok_or(EvalError::Overflow { stage: "reserve_out" })?,
            )
        } else {
            (
                r0.checked_sub(out)
                    .ok_or(EvalError::Overflow { stage: "reserve_out" })?,
                r1.checked_add(amount_in)
                    .ok_or(EvalError::Overflow { stage: "reserve_in" })?,
            )
        };
        self.reserves[e.pool] = updated;
        Ok(out)
    }

    /// Walk the cycle at `amount_in`, applying every hop to the ledger.
    pub fn apply_cycle(
        &mut self,
        cycle: &Cycle,
        amount_in: Raw,
        costs: &ExternalCosts,
    ) -> Result<CycleEval, EvalError> {
        if cycle.edges.is_empty() {
            return Err(EvalError::EmptyCycle);
        }
        if amount_in.is_negative() {
            return Err(EvalError::NegativeAmount { stage: "cycle_in" });
        }
        let start = self
            .g
            .edge(cycle.edges[0])
            .token_in;
        let mut expected = start;
        let mut amount = amount_in;
        for edge_id in &cycle.edges {
            let e = *self.g.edge(*edge_id);
            if e.token_in != expected {
                return Err(EvalError::OpenPath {
                    expected,
                    got: e.token_in,
                });
            }
            amount = self.apply_edge(*edge_id, amount)?;
            expected = e.token_out;
        }
        if expected != start {
            return Err(EvalError::OpenPath {
                expected: start,
                got: expected,
            });
        }

        let gross_raw = amount
            .checked_sub(amount_in)
            .ok_or(EvalError::Overflow { stage: "gross" })?;
        let financing_raw = costs
            .financing_premium_bps
            .of_floor(amount_in)
            .ok_or(EvalError::Overflow {
                stage: "financing_premium",
            })?;
        let costs_raw = financing_raw
            .checked_add(costs.unconditional_raw)
            .ok_or(EvalError::Overflow { stage: "costs_total" })?;
        let net_raw = gross_raw
            .checked_sub(costs_raw)
            .ok_or(EvalError::Overflow { stage: "net" })?;

        Ok(CycleEval {
            amount_in,
            amount_out: amount,
            gross_raw,
            financing_raw,
            unconditional_raw: costs.unconditional_raw,
            costs_raw,
            net_raw,
            hops: cycle.edges.len(),
            fees_embedded_in_quote: true,
        })
    }
}

/// Evaluate one cycle against a **fresh** ledger (the graph's snapshot state).
pub fn evaluate_cycle(
    g: &MultiGraph,
    cycle: &Cycle,
    amount_in: Raw,
    costs: &ExternalCosts,
) -> Result<CycleEval, EvalError> {
    let mut ledger = PoolLedger::new(g);
    ledger.apply_cycle(cycle, amount_in, costs)
}

/// Result of evaluating several legs that may share pools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitEval {
    pub legs: Vec<CycleEval>,
    /// Net after applying every leg **in order against the joint state**.
    pub joint_net_raw: Raw,
    /// What summing each leg's own quote against the unmutated snapshot gives.
    ///
    /// This is the number PROMPT §6 forbids using (`"no sumes outputs
    /// independientes que consumen la misma liquidez"`). It is returned only so
    /// the caller can *see* the overstatement, and it is `>= joint_net_raw`
    /// whenever the legs share a pool.
    pub naive_independent_sum_net_raw: Raw,
    /// `true` when at least two legs touch a common pool.
    pub shares_liquidity: bool,
}

/// Apply `steps` sequentially against one joint ledger.
///
/// Order matters and is the caller's: this function never reorders, because the
/// execution order is part of the plan.
pub fn evaluate_split_joint(
    g: &MultiGraph,
    steps: &[(Cycle, Raw)],
    costs: &ExternalCosts,
) -> Result<SplitEval, EvalError> {
    let mut naive: Raw = Raw::ZERO;
    for (cycle, size) in steps {
        let one = evaluate_cycle(g, cycle, *size, costs)?;
        naive = naive
            .checked_add(one.net_raw)
            .ok_or(EvalError::Overflow {
                stage: "naive_sum",
            })?;
    }

    let mut ledger = PoolLedger::new(g);
    let mut legs: Vec<CycleEval> = Vec::with_capacity(steps.len());
    let mut joint: Raw = Raw::ZERO;
    for (cycle, size) in steps {
        let evaluated = ledger.apply_cycle(cycle, *size, costs)?;
        joint = joint
            .checked_add(evaluated.net_raw)
            .ok_or(EvalError::Overflow { stage: "joint_sum" })?;
        legs.push(evaluated);
    }

    let mut seen: Vec<PoolIndex> = Vec::new();
    let mut shares = false;
    for (cycle, _) in steps {
        for pool in cycle.pool_footprint(g) {
            if seen.contains(&pool) {
                shares = true;
            } else {
                seen.push(pool);
            }
        }
    }

    Ok(SplitEval {
        legs,
        joint_net_raw: joint,
        naive_independent_sum_net_raw: naive,
        shares_liquidity: shares,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cycles::{enumerate_bounded, EnumerateLimits};
    use crate::graph::{GraphPolicy, PoolSpec};
    use std::collections::BTreeSet;

    fn pool(id: u64, t0: TokenId, t1: TokenId, r0: i128, r1: i128) -> PoolSpec {
        PoolSpec {
            pool_id: id,
            version: ProtocolVersion::V2,
            token0: t0,
            token1: t1,
            fee_bps: Bps::new(30).unwrap(),
            reserve0: Raw::new(r0),
            reserve1: Raw::new(r1),
            available: true,
        }
    }

    #[test]
    fn constant_product_quote_is_exact_and_floors() {
        // r_in = r_out = 1_000_000, fee 30 bps, x = 1_000.
        // eff = 997 ; out = floor(1_000_000·997 / 1_000_997) = 996
        let out = quote_constant_product(
            7,
            Bps::new(30).unwrap(),
            Raw::new(1_000_000),
            Raw::new(1_000_000),
            Raw::new(1_000),
        )
        .unwrap();
        assert_eq!(out, Raw::new(996));
    }

    #[test]
    fn zero_fee_is_a_credited_zero_and_missing_is_not_dressed_up() {
        let out = quote_constant_product(
            7,
            Bps::ZERO,
            Raw::new(1_000_000),
            Raw::new(1_000_000),
            Raw::new(1_000),
        )
        .unwrap();
        assert_eq!(out, Raw::new(999));
        // A zero reserve is a named failure, not a zero output.
        let err = quote_constant_product(
            7,
            Bps::ZERO,
            Raw::ZERO,
            Raw::new(1_000),
            Raw::new(1_000),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EvalError::ZeroReserve {
                pool_id: 7,
                side: "input"
            }
        );
    }

    #[test]
    fn dust_below_one_unit_yields_a_computed_zero() {
        let out = quote_constant_product(
            7,
            Bps::new(30).unwrap(),
            Raw::new(1_000_000),
            Raw::new(1_000_000),
            Raw::new(1),
        )
        .unwrap();
        assert_eq!(out, Raw::new(0));
    }

    #[test]
    fn non_v2_curves_fail_with_a_named_reason_never_a_fake_number() {
        let mut v3 = pool(8, 1, 2, 1_000, 1_000);
        v3.version = ProtocolVersion::V3;
        let g = MultiGraph::build(vec![pool(7, 1, 2, 1_000, 1_000), v3], &GraphPolicy::permissive_2_to_7());
        let cycle = Cycle { edges: vec![0, 1] };
        // Edges 0,1 belong to pool 7 (V2) → evaluable.
        assert!(evaluate_cycle(&g, &cycle, Raw::new(100), &ExternalCosts::NONE).is_ok());
        // Edges 2,3 belong to the V3 pool → named failure.
        let v3_cycle = Cycle { edges: vec![2, 3] };
        let err = evaluate_cycle(&g, &v3_cycle, Raw::new(100), &ExternalCosts::NONE).unwrap_err();
        assert_eq!(
            err,
            EvalError::CurveNotImplemented {
                pool_id: 8,
                version: ProtocolVersion::V3
            }
        );
    }

    #[test]
    fn financing_and_unconditional_costs_both_enter_exactly_once() {
        let g = MultiGraph::build(
            vec![pool(7, 1, 2, 1_000_000, 1_000_000)],
            &GraphPolicy::permissive_2_to_7(),
        );
        let cycle = Cycle { edges: vec![0, 1] };
        let costs = ExternalCosts::flash_financed(Raw::new(5), Bps::new(9).unwrap());
        let e = evaluate_cycle(&g, &cycle, Raw::new(10_000), &costs).unwrap();
        assert_eq!(e.financing_raw, Raw::new(9)); // floor(10_000·9/10_000)
        assert_eq!(e.unconditional_raw, Raw::new(5));
        assert_eq!(e.costs_raw, Raw::new(14));
        assert_eq!(e.gross_raw, e.amount_out.checked_sub(Raw::new(10_000)).unwrap());
        assert_eq!(e.net_raw, e.gross_raw.checked_sub(Raw::new(14)).unwrap());
        assert!(e.fees_embedded_in_quote);
    }

    #[test]
    fn open_path_is_rejected() {
        let g = MultiGraph::build(
            vec![pool(7, 1, 2, 1_000_000, 1_000_000)],
            &GraphPolicy::permissive_2_to_7(),
        );
        // 1→2 then 1→2 again: hop 2 expects token 2.
        let broken = Cycle { edges: vec![0, 0] };
        assert!(matches!(
            evaluate_cycle(&g, &broken, Raw::new(100), &ExternalCosts::NONE),
            Err(EvalError::OpenPath { .. })
        ));
    }

    #[test]
    fn shared_pool_split_overstates_when_summed_independently() {
        // Two pools over (1,2): a cycle that buys on 20 and sells on 21.
        let g = MultiGraph::build(
            vec![pool(20, 1, 2, 1_000_000, 1_000_000), pool(21, 1, 2, 1_000_000, 1_000_000)],
            &GraphPolicy::permissive_2_to_7(),
        );
        let (cycles, _) = enumerate_bounded(&g, &GraphPolicy::permissive_2_to_7(), EnumerateLimits::unbounded());
        assert_eq!(cycles.len(), 2, "one cycle per direction");

        let c = cycles[0].clone();
        let another = cycles[0].clone();
        let split = evaluate_split_joint(
            &g,
            &[(c, Raw::new(50_000)), (another, Raw::new(50_000))],
            &ExternalCosts::NONE,
        )
        .unwrap();
        assert!(split.shares_liquidity, "the two legs reuse the same pools");
        assert_eq!(split.legs.len(), 2);
        assert!(
            split.naive_independent_sum_net_raw > split.joint_net_raw,
            "independent sum {} must overstate joint {}",
            split.naive_independent_sum_net_raw,
            split.joint_net_raw
        );
    }

    #[test]
    fn disjoint_split_legs_agree_with_the_independent_sum() {
        let g = MultiGraph::build(
            vec![
                pool(20, 1, 2, 1_000_000, 1_000_000),
                pool(21, 1, 2, 1_000_000, 900_000),
                pool(30, 3, 4, 1_000_000, 1_000_000),
                pool(31, 3, 4, 1_000_000, 900_000),
            ],
            &GraphPolicy::permissive_2_to_7(),
        );
        let policy = GraphPolicy::permissive_2_to_7();
        let (cycles, _) = enumerate_bounded(&g, &policy, EnumerateLimits::unbounded());
        let pair_a: BTreeSet<u64> = cycles
            .iter()
            .find(|c| c.touches_pool(&g, 20))
            .unwrap()
            .pool_ids(&g)
            .into_iter()
            .collect();
        assert_eq!(pair_a, [20u64, 21].into_iter().collect::<BTreeSet<_>>());
        let leg_a = cycles.iter().find(|c| c.touches_pool(&g, 20)).unwrap().clone();
        let leg_b = cycles.iter().find(|c| c.touches_pool(&g, 30)).unwrap().clone();
        let split = evaluate_split_joint(
            &g,
            &[(leg_a, Raw::new(10_000)), (leg_b, Raw::new(10_000))],
            &ExternalCosts::NONE,
        )
        .unwrap();
        assert!(!split.shares_liquidity);
        assert_eq!(split.naive_independent_sum_net_raw, split.joint_net_raw);
    }
}
