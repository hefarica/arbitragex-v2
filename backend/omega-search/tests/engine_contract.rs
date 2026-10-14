//! PROMPT §14 — mandatory search tests.
//!
//! `unwrap`/`expect` are allowed here exactly as the repo's own test modules do
//! (`route_discovery::guarantees` uses the same module-level allow): a failing
//! fixture must fail loudly, not degrade into a skipped assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//!
//! Every fixture here is synthetic and isolated from the live stream and from
//! production metrics, which §14 explicitly allows (*"Los fixtures sintéticos
//! están permitidos en tests y deben estar aislados del stream y las métricas
//! reales"*). No number below is presented as a productive opportunity: the
//! assertions are about the **engine's** behaviour — exactness, sign, stop
//! reporting, generation turnover and oracle agreement.
//!
//! The reference fixture is a genuine price dislocation between two constant
//! product pools over the same pair, with every expected value computed by hand
//! from the integer quote formula and asserted exactly:
//!
//! * pool A: reserves (1_000_000, 1_000_000), 30 bps
//! * pool B: reserves (1_000_000,   900_000), 30 bps
//!
//! At `x = 1_000` the profitable orientation `A:1→2 then B:2→1` yields
//! `996` then `1102`, i.e. `gross = +102`; the opposite orientation yields
//! `896` then `892`, i.e. `gross = −108`.

use omega_search::*;
use std::collections::BTreeMap;

const FEE: u32 = 30;

fn pool(id: u64, t0: TokenId, t1: TokenId, r0: i128, r1: i128, fee: u32) -> PoolSpec {
    PoolSpec {
        pool_id: id,
        version: ProtocolVersion::V2,
        token0: t0,
        token1: t1,
        fee_bps: Bps::new(fee).unwrap(),
        reserve0: Raw::new(r0),
        reserve1: Raw::new(r1),
        available: true,
    }
}

/// Two pools over the same pair with a real dislocation.
fn dislocation_graph() -> MultiGraph {
    MultiGraph::build(
        vec![
            pool(100, 1, 2, 1_000_000, 1_000_000, FEE),
            pool(200, 1, 2, 1_000_000, 900_000, FEE),
        ],
        &GraphPolicy::permissive_2_to_7(),
    )
}

fn sizes(values: &[i128]) -> Vec<Raw> {
    values.iter().copied().map(Raw::new).collect()
}

#[allow(clippy::too_many_arguments)]
fn cfg(
    size_grid: &[i128],
    costs: ExternalCosts,
    min: i128,
    target: i128,
    budget: u64,
    diverse_k: usize,
    policy: &GraphPolicy,
    marginal: BTreeMap<EdgeId, MarginalRate>,
) -> EngineConfig {
    EngineConfig {
        limits: EnumerateLimits::unbounded(),
        eval_budget: budget,
        sizes: sizes(size_grid),
        refine_rounds: 0,
        costs,
        family_weights: EngineConfig::uniform_family_weights(policy),
        diverse_k,
        min_profit: HardMinimum(Raw::new(min)),
        target: SearchTarget(Raw::new(target)),
        marginal_rates: marginal,
    }
}

fn engine_with(g: MultiGraph, policy: GraphPolicy, c: EngineConfig) -> SearchEngine {
    SearchEngine::new(g, policy, c)
}

// ---------------------------------------------------------------------------
// 1. The original route is kept as a diagnosis and beaten by a real alternative
// ---------------------------------------------------------------------------

#[test]
fn losing_seed_is_kept_as_diagnosis_and_beaten_by_a_real_alternative() {
    let g = dislocation_graph();
    let policy = GraphPolicy::permissive_2_to_7();

    // Edges: 0 = A(1→2), 1 = A(2→1), 2 = B(1→2), 3 = B(2→1).
    // [2, 1] is the orientation that buys on B and sells on A: the loser.
    let seed = canonicalize(&g, &[2, 1]).expect("canonical 2-cycle");

    // Hand-computed proof that the seed really is the losing route.
    let seed_eval = evaluate_cycle(&g, &seed, Raw::new(1_000), &ExternalCosts::NONE).unwrap();
    assert_eq!(seed_eval.amount_out, Raw::new(892));
    assert_eq!(seed_eval.gross_raw, Raw::new(-108));

    let engine = engine_with(
        g.clone(),
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, Some(&seed));

    // The better alternative is found, exactly.
    let best = outcome.best_computed.as_ref().expect("a computed best");
    assert_eq!(best.net_raw(), Some(Raw::new(102)));
    assert_eq!(best.cycle.edges, vec![0, 3]);
    assert_ne!(best.cycle.edges, seed.edges, "the winner is a different route");

    // The loser is still visible as a diagnosis, with its provenance.
    let diag = outcome.seed_diagnosis.as_ref().expect("seed diagnosis");
    assert_eq!(diag.net_raw(), Some(Raw::new(-108)));
    assert_eq!(diag.origin, CandidateOrigin::Seed);

    // The seed is also enumerated; it must not be counted as a second candidate.
    assert!(
        outcome.duplicates_skipped >= 1,
        "the seed cycle is enumerated too and must be deduplicated"
    );
    assert_eq!(outcome.label, BEST_FOUND_LABEL);
    assert_eq!(outcome.stop.reason, StopReason::ScopeExhausted);
}

// ---------------------------------------------------------------------------
// 2. Size changes the result; the best size is not an endpoint
// ---------------------------------------------------------------------------

#[test]
fn size_choice_changes_the_sign_and_the_best_size_is_interior() {
    let policy = GraphPolicy::permissive_2_to_7();

    let small = engine_with(
        dislocation_graph(),
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut l1 = CandidateLedger::new(SnapshotId(1));
    let o_small = small.run(&mut l1, None);
    assert_eq!(o_small.best_computed_net(), Some(Raw::new(102)));

    let huge = engine_with(
        dislocation_graph(),
        policy.clone(),
        cfg(&[500_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut l2 = CandidateLedger::new(SnapshotId(1));
    let o_huge = huge.run(&mut l2, None);
    // At 500_000 the same cycle inverts: the round trip loses.
    assert!(o_huge.best_computed_net().unwrap().is_negative());
    assert_eq!(o_huge.every_candidate_negative, Some(true));
    assert_eq!(o_huge.stop.reason, StopReason::ScopeExhausted);

    let grid = engine_with(
        dislocation_graph(),
        policy.clone(),
        cfg(
            &[1_000, 10_000, 500_000],
            ExternalCosts::NONE,
            0,
            0,
            1_000,
            4,
            &policy,
            BTreeMap::new(),
        ),
    );
    let mut l3 = CandidateLedger::new(SnapshotId(1));
    let o_grid = grid.run(&mut l3, None);
    // Interior maximum of the grid: strictly better than either endpoint.
    assert_eq!(o_grid.best_computed_net(), Some(Raw::new(816)));
    assert!(o_grid.best_computed_net().unwrap() > o_small.best_computed_net().unwrap());
    assert!(o_grid.best_computed_net().unwrap() > o_huge.best_computed_net().unwrap());
    assert_eq!(o_grid.scope.sizes_tried, 3);
}

// ---------------------------------------------------------------------------
// 3. All candidates negative: no hang, no sign flip
// ---------------------------------------------------------------------------

#[test]
fn all_candidates_negative_terminates_and_keeps_the_sign() {
    let policy = GraphPolicy::permissive_2_to_7();
    let engine = engine_with(
        dislocation_graph(),
        policy.clone(),
        // A fixed unconditional cost (gas) larger than the best gross at any
        // size in the grid: 49 at x=500, 102 at x=1_000.
        cfg(
            &[500, 1_000],
            ExternalCosts::self_funded(Raw::new(1_000)),
            0,
            0,
            1_000,
            4,
            &policy,
            BTreeMap::new(),
        ),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    assert_eq!(outcome.every_candidate_negative, Some(true));
    assert_eq!(outcome.best_computed_net(), Some(Raw::new(-898)));
    assert!(outcome.best_computed_net().unwrap().is_negative());
    assert!(outcome.is_no_opportunity());
    assert!(!outcome.has_executable_candidate());
    // gap = target(0) − best(−898)
    assert_eq!(outcome.gap_to_target, Some(Raw::new(898)));
    assert_eq!(outcome.label, BEST_FOUND_LABEL);
    assert_eq!(outcome.scope.cycles_evaluated, 2);
    assert!(outcome.best_feasible.is_none());
}

// ---------------------------------------------------------------------------
// 4. Unreachable objective
// ---------------------------------------------------------------------------

#[test]
fn unreachable_target_publishes_the_gap_and_keeps_the_best_number() {
    let policy = GraphPolicy::permissive_2_to_7();
    let engine = engine_with(
        dislocation_graph(),
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 1_000_000, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    assert_eq!(outcome.best_computed_net(), Some(Raw::new(102)));
    assert!(outcome.best_over_target.is_none());
    assert_eq!(outcome.gap_to_target, Some(Raw::new(999_898)));
    assert!(outcome.is_no_opportunity());
    // A negative-or-insufficient result is not deleted when there is no winner.
    assert!(outcome.best_computed.is_some());
    assert_eq!(outcome.target, SearchTarget(Raw::new(1_000_000)));
}

// ---------------------------------------------------------------------------
// 5. Positive net below the execution minimum is NOT authorized
// ---------------------------------------------------------------------------

#[test]
fn net_positive_below_execution_minimum_is_reported_but_not_authorized() {
    let policy = GraphPolicy::permissive_2_to_7();
    let engine = engine_with(
        dislocation_graph(),
        policy.clone(),
        cfg(
            &[1_000],
            ExternalCosts::NONE,
            /* min */ 150,
            /* target */ 1_000_000,
            1_000,
            4,
            &policy,
            BTreeMap::new(),
        ),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    // +102 is a real computed positive...
    assert_eq!(outcome.best_computed_net(), Some(Raw::new(102)));
    assert_eq!(outcome.every_candidate_negative, Some(false));
    // ...but it is below the hard execution minimum and below the objective.
    assert!(outcome.best_feasible.is_none(), "min = 150 > 102");
    assert!(outcome.best_over_target.is_none());
    assert!(!outcome.has_executable_candidate());
    assert!(outcome.is_no_opportunity());
    assert_eq!(outcome.min_profit, HardMinimum(Raw::new(150)));
    assert_eq!(outcome.gap_to_target, Some(Raw::new(999_898)));
}

// ---------------------------------------------------------------------------
// 6. Budget exhausted: publish scope, best, gap and stop reason
// ---------------------------------------------------------------------------

#[test]
fn budget_exhaustion_publishes_scope_best_gap_and_reason() {
    let g = dislocation_graph();
    let policy = GraphPolicy::permissive_2_to_7();
    let seed = canonicalize(&g, &[2, 1]).unwrap();
    let engine = engine_with(
        g,
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 50, /* budget */ 0, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, Some(&seed));

    match outcome.stop.reason {
        StopReason::EvalBudgetExhausted { pending } => {
            assert!(pending >= 2, "both enumerated cycles stay vigente, got {pending}");
        }
        other => panic!("expected eval_budget_exhausted, got {other:?}"),
    }
    assert_eq!(outcome.stop.budget_total, 0);
    assert_eq!(outcome.stop.budget_used, 0);
    assert!(outcome.stop.pending_work >= 2);

    // Scope is published anyway.
    assert_eq!(outcome.scope.cycles_enumerated, 2);
    assert_eq!(outcome.scope.authorized_hops, vec![2, 3, 4, 5, 6, 7]);
    assert_eq!(outcome.scope.edges, 4);
    // Best so far = the seed only, and the gap is still computed.
    assert_eq!(outcome.best_computed_net(), Some(Raw::new(-108)));
    assert_eq!(outcome.gap_to_target, Some(Raw::new(158)));
    assert_eq!(outcome.label, BEST_FOUND_LABEL);
}

// ---------------------------------------------------------------------------
// 7. The marginal model proposes a candidate that finite size rejects
// ---------------------------------------------------------------------------

#[test]
fn marginal_generator_proposes_a_candidate_that_finite_size_rejects() {
    let g = dislocation_graph();
    let policy = GraphPolicy::permissive_2_to_7();

    // Exact marginal rates: edge 0 (A: 1→2) = r1/r0 = 1/1,
    // edge 3 (B: 2→1) = r0/r1 = 1_000_000/900_000 = 10/9.
    let mut rates: BTreeMap<EdgeId, MarginalRate> = BTreeMap::new();
    rates.insert(0, MarginalRate { num: 1, den: 1 });
    rates.insert(3, MarginalRate { num: 10, den: 9 });

    // Verdict of the marginal model on the profitable orientation: above one.
    let profitable = canonicalize(&g, &[0, 3]).unwrap();
    assert_eq!(
        marginal_product_verdict(&g, &profitable, &rates),
        MarginalVerdict::AboveOne
    );

    // A fixed unconditional cost (gas) of 200 makes the same route lose at
    // every size in the grid (gross 49 at 500, 102 at 1_000).
    let engine = engine_with(
        g,
        policy.clone(),
        cfg(
            &[500, 1_000],
            ExternalCosts::self_funded(Raw::new(200)),
            0,
            0,
            1_000,
            4,
            &policy,
            rates,
        ),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    // The generator did propose it...
    assert!(outcome.marginal.above_one >= 1);
    let proposed = ledger
        .current()
        .iter()
        .find(|c| c.origin == CandidateOrigin::MarginalProduct)
        .expect("the above-one cycle is tagged MarginalProduct");
    assert_eq!(proposed.cycle.edges, vec![0, 3]);

    // ...and the exact finite-size evaluation rejected it: negative, no flip.
    assert_eq!(outcome.best_computed_net(), Some(Raw::new(-98)));
    assert_eq!(outcome.every_candidate_negative, Some(true));
    // The other orientation had no rate supplied: a third state, not "below one".
    assert!(outcome.marginal.missing_rate >= 1);
}

// ---------------------------------------------------------------------------
// 8. Oracle: exhaustive enumeration + exact evaluation, versus the engine
// ---------------------------------------------------------------------------

fn oracle_graph() -> MultiGraph {
    MultiGraph::build(
        vec![
            pool(10, 1, 2, 1_000_000, 1_000_000, FEE),
            pool(11, 1, 2, 1_000_000, 980_000, FEE),
            pool(12, 2, 3, 1_000_000, 1_000_000, FEE),
            pool(13, 3, 1, 1_000_000, 1_000_000, FEE),
            pool(14, 3, 4, 1_000_000, 1_000_000, FEE),
            pool(15, 4, 1, 1_000_000, 1_000_000, FEE),
            pool(16, 4, 5, 1_000_000, 1_000_000, FEE),
            pool(17, 5, 1, 1_000_000, 1_000_000, FEE),
        ],
        &GraphPolicy::permissive_2_to_7(),
    )
}

#[test]
fn bounded_engine_matches_exhaustive_oracle_on_a_small_graph() {
    let g = oracle_graph();
    let policy = GraphPolicy::permissive_2_to_7();

    // Oracle: every cycle, every size, exact evaluation.
    let oracle_cycles = enumerate_exhaustive(&g, &policy);
    assert!(!oracle_cycles.is_empty());
    let grid = [Raw::new(1_000), Raw::new(7_500)];
    let mut oracle_best: Option<Raw> = None;
    for cycle in &oracle_cycles {
        for size in grid {
            if let Ok(evaluated) = evaluate_cycle(&g, cycle, size, &ExternalCosts::NONE) {
                oracle_best = Some(match oracle_best {
                    None => evaluated.net_raw,
                    Some(current) => current.max(evaluated.net_raw),
                });
            }
        }
    }
    assert!(oracle_best.is_some(), "the oracle evaluated at least one cycle");

    let engine = engine_with(
        g.clone(),
        policy.clone(),
        cfg(&[1_000, 7_500], ExternalCosts::NONE, 0, 0, 100_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    // Same cycle space, same best number: the pruning did not lose the optimum.
    assert_eq!(
        outcome.scope.cycles_enumerated,
        oracle_cycles.len(),
        "bounded enumeration must cover the same authorized space"
    );
    assert_eq!(outcome.scope.cycles_evaluated, oracle_cycles.len());
    assert_eq!(outcome.scope.cycles_not_evaluated, 0);
    assert_eq!(outcome.best_computed_net(), oracle_best);
    assert_eq!(outcome.stop.reason, StopReason::ScopeExhausted);
}

// ---------------------------------------------------------------------------
// 9. The heuristic prune is identifiable and measured against the oracle
// ---------------------------------------------------------------------------

#[test]
fn dominance_prune_is_identified_and_does_not_change_the_optimum_here() {
    let policy_open = GraphPolicy::permissive_2_to_7();
    let policy_pruned = GraphPolicy {
        dominance_prune: true,
        ..GraphPolicy::permissive_2_to_7()
    };
    let pools = vec![
        pool(10, 1, 2, 1_000_000, 1_000_000, FEE),
        pool(11, 1, 2, 100_000, 100_000, FEE), // same fee, shallower → dominated
        pool(12, 1, 2, 50_000, 50_000, FEE),   // same fee, shallowest → dominated
    ];
    let g_open = MultiGraph::build(pools.clone(), &policy_open);
    let g_pruned = MultiGraph::build(pools, &policy_pruned);

    assert_eq!(g_open.dominance_dropped(), 0);
    // Two survivors per (pair, direction, version, equal fee) are kept;
    // the third (shallowest) pool is dropped in each direction.
    assert_eq!(g_pruned.dominance_dropped(), 2, "1 dropped pool × 2 directions");
    assert!(g_pruned.edges().len() < g_open.edges().len());

    let oracle_cycles = enumerate_exhaustive(&g_open, &policy_open);
    let mut oracle_best: Option<Raw> = None;
    for cycle in &oracle_cycles {
        if let Ok(evaluated) = evaluate_cycle(&g_open, cycle, Raw::new(1_000), &ExternalCosts::NONE)
        {
            oracle_best = Some(match oracle_best {
                None => evaluated.net_raw,
                Some(current) => current.max(evaluated.net_raw),
            });
        }
    }

    let mk = |g: MultiGraph, p: &GraphPolicy| {
        engine_with(
            g,
            p.clone(),
            cfg(&[1_000], ExternalCosts::NONE, 0, 0, 100_000, 4, p, BTreeMap::new()),
        )
    };
    let mut l_open = CandidateLedger::new(SnapshotId(1));
    let o_open = mk(g_open, &policy_open).run(&mut l_open, None);
    let mut l_pruned = CandidateLedger::new(SnapshotId(1));
    let o_pruned = mk(g_pruned, &policy_pruned).run(&mut l_pruned, None);

    assert_eq!(o_open.best_computed_net(), oracle_best);
    // Measured, not assumed: on this graph the prune keeps the same optimum.
    assert_eq!(o_pruned.best_computed_net(), oracle_best);
    // And the prune is visible in the published scope.
    assert!(o_pruned.scope.edges < o_open.scope.edges);
    assert!(o_pruned.scope.cycles_enumerated <= o_open.scope.cycles_enumerated);
}

// ---------------------------------------------------------------------------
// 10. Diverse alternatives do not share pools
// ---------------------------------------------------------------------------

#[test]
fn diverse_alternatives_avoid_shared_pools() {
    // Two independent dislocations on disjoint token pairs.
    let g = MultiGraph::build(
        vec![
            pool(100, 1, 2, 1_000_000, 1_000_000, FEE),
            pool(200, 1, 2, 1_000_000, 900_000, FEE),
            pool(300, 3, 4, 1_000_000, 1_000_000, FEE),
            pool(400, 3, 4, 1_000_000, 900_000, FEE),
        ],
        &GraphPolicy::permissive_2_to_7(),
    );
    let policy = GraphPolicy::permissive_2_to_7();
    let engine = engine_with(
        g.clone(),
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    let best = outcome.best_computed.as_ref().unwrap();
    assert_eq!(best.net_raw(), Some(Raw::new(102)));
    let best_pools = best.pools.clone();
    assert_eq!(best_pools.len(), 2);

    assert!(
        !outcome.diverse_alternatives.is_empty(),
        "the second pair offers a pool-disjoint alternative"
    );
    for alt in &outcome.diverse_alternatives {
        assert!(
            alt.pools.is_disjoint(&best_pools),
            "an alternative must not reuse the best candidate's pools"
        );
    }
    for i in 0..outcome.diverse_alternatives.len() {
        for j in (i + 1)..outcome.diverse_alternatives.len() {
            assert!(outcome.diverse_alternatives[i]
                .pools
                .is_disjoint(&outcome.diverse_alternatives[j].pools));
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Generation turnover invalidates a stale positive result
// ---------------------------------------------------------------------------

#[test]
fn generation_turnover_invalidates_a_stale_positive_result() {
    let policy = GraphPolicy::permissive_2_to_7();

    let before = engine_with(
        dislocation_graph(),
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let o1 = before.run(&mut ledger, None);
    assert_eq!(o1.best_computed_net(), Some(Raw::new(102)));
    assert_eq!(o1.scope.generation, GenerationId(1));

    // A relevant change arrives → new generation bound to a new snapshot.
    let g2 = ledger.open_generation(SnapshotId(2));
    assert_eq!(g2, GenerationId(2));
    assert!(ledger.current().is_empty(), "current candidates are invalidated");
    // Both cycles of the old snapshot were current and are now invalidated.
    assert_eq!(ledger.invalidated_count(), 2);

    // The new snapshot is devalued: 600 bps of fee on both pools.
    let devalued = MultiGraph::build(
        vec![
            pool(100, 1, 2, 1_000_000, 1_000_000, 600),
            pool(200, 1, 2, 1_000_000, 900_000, 600),
        ],
        &GraphPolicy::permissive_2_to_7(),
    );
    let after = engine_with(
        devalued,
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let o2 = after.run(&mut ledger, None);

    // The stale positive is NOT the current answer any more.
    assert_eq!(o2.scope.generation, GenerationId(2));
    // Hand-computed: the best orientation now yields 939 then 979 → −21.
    assert_eq!(o2.best_computed_net(), Some(Raw::new(-21)));
    assert!(o2.best_computed_net().unwrap().is_negative());
    assert!(o2.is_no_opportunity());
    assert_eq!(o2.every_candidate_negative, Some(true));
    assert!(o2.invalidated >= 1);

    // The historical maximum is kept in its own slot, as a record only.
    assert_eq!(
        ledger.historical_max().unwrap().net_raw(),
        Some(Raw::new(102))
    );
    assert_eq!(ledger.historical_max().unwrap().generation, GenerationId(1));
    let current_best = ledger.current_best().unwrap();
    assert_eq!(current_best.net_raw(), Some(Raw::new(-21)));
    assert_ne!(
        current_best.net_raw(),
        ledger.historical_max().unwrap().net_raw(),
        "historical max and current best are separate values"
    );
}

// ---------------------------------------------------------------------------
// 13. Loss, a credited zero and absence are three distinct states
// ---------------------------------------------------------------------------

#[test]
fn break_even_is_a_credited_zero_not_a_loss_and_not_absence() {
    // 500 bps on both pools makes the round trip break even EXACTLY at x=1_000:
    // 950 → 949 out of A, then 901 effective → 1000 out of B, so gross = 0.
    // A float pipeline would land near zero; exact integers land on it.
    let g = MultiGraph::build(
        vec![
            pool(100, 1, 2, 1_000_000, 1_000_000, 500),
            pool(200, 1, 2, 1_000_000, 900_000, 500),
        ],
        &GraphPolicy::permissive_2_to_7(),
    );
    let policy = GraphPolicy::permissive_2_to_7();

    let profitable = canonicalize(&g, &[0, 3]).unwrap();
    let zero = evaluate_cycle(&g, &profitable, Raw::new(1_000), &ExternalCosts::NONE).unwrap();
    assert_eq!(zero.amount_out, Raw::new(1_000));
    assert_eq!(zero.gross_raw, Raw::ZERO);
    assert_eq!(zero.net_raw, Raw::ZERO);
    assert!(zero.net_raw.is_zero() && !zero.net_raw.is_negative());

    let loser = canonicalize(&g, &[2, 1]).unwrap();
    let loss = evaluate_cycle(&g, &loser, Raw::new(1_000), &ExternalCosts::NONE).unwrap();
    assert_eq!(loss.net_raw, Raw::new(-190));

    // (a) credited zero: a number WAS computed and it is exactly zero.
    let engine = engine_with(
        g.clone(),
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 1, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);
    assert_eq!(outcome.best_computed_net(), Some(Raw::ZERO));
    assert_eq!(outcome.every_candidate_negative, Some(false));
    assert!(outcome.best_feasible.is_some(), "0 ≥ min 0");
    assert!(outcome.is_no_opportunity(), "target 1 is not reached by 0");
    assert_eq!(outcome.gap_to_target, Some(Raw::new(1)));
    assert_eq!(outcome.scope.cycles_evaluated, 2);

    // (b) absence: no number at all is a different state from zero.
    let empty_policy = GraphPolicy {
        hop_lengths: HopLengthMask::none(),
        ..GraphPolicy::default()
    };
    let silent = engine_with(
        g,
        empty_policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 1, 1_000, 4, &empty_policy, BTreeMap::new()),
    );
    let mut ledger2 = CandidateLedger::new(SnapshotId(1));
    let o_absent = silent.run(&mut ledger2, None);
    assert_eq!(o_absent.best_computed_net(), None);
    assert_ne!(
        o_absent.best_computed_net(),
        Some(Raw::ZERO),
        "absence must never be rendered as a zero"
    );
    assert_eq!(o_absent.every_candidate_negative, None);
    assert_eq!(o_absent.gap_to_target, None);
}

// ---------------------------------------------------------------------------
// 14. Empty / unevaluable scopes are reported, never rendered as zero
// ---------------------------------------------------------------------------

#[test]
fn empty_authorized_scope_is_reported_not_crashed() {
    let policy = GraphPolicy {
        hop_lengths: HopLengthMask::none(),
        ..GraphPolicy::default()
    };
    let engine = engine_with(
        dislocation_graph(),
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    assert_eq!(outcome.stop.reason, StopReason::EmptyAuthorizedScope);
    assert_eq!(outcome.scope.authorized_hops, Vec::<u8>::new());
    assert_eq!(outcome.best_computed_net(), None);
    assert_eq!(outcome.gap_to_target, None, "no number computed → no gap");
    assert_eq!(outcome.every_candidate_negative, None, "absence is not 'all negative'");
    assert!(outcome.is_no_opportunity());
    assert_eq!(outcome.label, BEST_FOUND_LABEL);
}

#[test]
fn a_policy_with_no_cycle_reports_no_candidate_in_scope() {
    let g = MultiGraph::build(
        vec![pool(100, 1, 2, 1_000_000, 1_000_000, FEE)],
        &GraphPolicy::permissive_2_to_7(),
    );
    let policy = GraphPolicy::permissive_2_to_7();
    let engine = engine_with(
        g,
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    assert_eq!(outcome.stop.reason, StopReason::NoCandidateInScope);
    assert_eq!(outcome.best_computed_net(), None);
    assert_eq!(outcome.every_candidate_negative, None);
}

#[test]
fn unevaluable_cycle_keeps_its_named_reason_and_no_fake_number() {
    let mut v3 = pool(300, 1, 2, 1_000_000, 800_000, FEE);
    v3.version = ProtocolVersion::V3;
    let g = MultiGraph::build(
        vec![pool(100, 1, 2, 1_000_000, 1_000_000, FEE), v3],
        &GraphPolicy::permissive_2_to_7(),
    );
    let policy = GraphPolicy::permissive_2_to_7();
    let engine = engine_with(
        g,
        policy.clone(),
        cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new()),
    );
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);

    // The V3-containing cycles are enumerated but cannot be quoted here.
    assert!(outcome.scope.cycles_not_evaluated >= 1);
    assert_eq!(outcome.scope.cycles_evaluated, 0);
    assert_eq!(outcome.best_computed_net(), None, "no number was computed");
    assert_eq!(outcome.every_candidate_negative, None);
    let reason = ledger
        .current()
        .iter()
        .find_map(|c| c.not_evaluated_reason())
        .expect("a named reason");
    assert!(matches!(reason, EvalError::CurveNotImplemented { .. }));
}

#[test]
fn fair_share_weights_are_required_to_cover_every_authorized_family() {
    let policy = GraphPolicy::permissive_2_to_7();
    // Deliberately configure only the 2-hop family.
    let mut c = cfg(&[1_000], ExternalCosts::NONE, 0, 0, 1_000, 4, &policy, BTreeMap::new());
    c.family_weights = vec![(FamilyId::new(2), 4)];
    let engine = engine_with(dislocation_graph(), policy.clone(), c);

    // The engine widens the weights instead of silently losing the 3..7 work,
    // and publishes which families it had to add.
    assert!(!engine.auto_added_families().is_empty());
    let mut ledger = CandidateLedger::new(SnapshotId(1));
    let outcome = engine.run(&mut ledger, None);
    assert_eq!(
        outcome.scope.auto_added_families,
        vec![
            FamilyId::new(3),
            FamilyId::new(4),
            FamilyId::new(5),
            FamilyId::new(6),
            FamilyId::new(7)
        ]
    );
    assert_eq!(outcome.stop.reason, StopReason::ScopeExhausted);
}
