// frontend/lib/store/__tests__/search-state.test.ts
//
// SEARCH-STATE-01 (§11.2 / §11.3 / §11.5) — the visible search state.
//
// What is pinned here, and why each one is a §11 requirement rather than a
// preference:
//   · §11.2 "objetivo y brecha": the gap is max(0, target − net). The §11
//     presentation case (net −7.91 against a 50 target ⇒ gap 57.91) is used as
//     a FIXTURE OF THE RELATION, never as a runtime constant: no number from the
//     operator's screenshot is imported by the module under test.
//   · §11.5 "conserva el mejor resultado NEGATIVO": a negative plan survives as
//     first-class state, and the diagnostics stay visible with no positive at all.
//   · three distinct states: loss, real zero and absence (R8/§5).
//   · §11.3 "progreso real": a metric with no producer is ABSENT with a reason —
//     NEVER 0 (E2E-COMPUTE GUARD / R10).
//   · §11 (colour rule): the gap is a DISTANCE and must not be presented as a
//     reached state; `lifecycleOf` asserts a stage only from the field that
//     declares it (a positive number is not a target; a computed quote is not a
//     submission).
//   · SIM/LIVE isolation: a row carrying only the simulated ladder never borrows
//     a canonical figure, and vice versa.
import { describe, expect, it } from "vitest";

import { mapToOmniOpportunity, type OmniOpportunity } from "../types";
import {
  ABSENT_REASONS,
  capInventory,
  deriveStrategySearchState,
  lifecycleOf,
  planFactsOf,
  progressNumber,
  selectBestPlans,
  trackImprovement,
  EMPTY_IMPROVEMENT_STATE,
} from "../search-state";

function row(over: Record<string, unknown> = {}): OmniOpportunity {
  return mapToOmniOpportunity({
    id: "opp-1",
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-10-03T10:00:00Z",
    status: "rejected",
    trace_id: "trace-1",
    dex_a: "uniswap_v2",
    dex_b: "sushiswap",
    token_in: "0xaaa",
    token_out: "0xbbb",
    ...over,
  });
}

/**
 * One CLOSED canonical arithmetic: gross 10, costs 17.91 (gas 12 + TLS 5.91),
 * net −7.91 at a principal of 100, against a configured target of 50.
 * Synthetic fixture — the components sum to the total by construction, which is
 * exactly what `buildLedger` requires before the canonical basis may paint.
 */
function closedLoss(over: Record<string, unknown> = {}): OmniOpportunity {
  return row({
    economics: {
      computation_status: "computed",
      amount_in_usd: 100,
      gross_profit_usd: 10,
      total_cost_usd: 17.91,
      net_profit_usd: -7.91,
      gas_usd: 12,
      flash_fee_usd: 5.91,
      target_net_usd: 50,
      target_delta_usd: -57.91,
      meets_target: false,
      not_computed_reasons: {},
    },
    ...over,
  });
}

describe("search-state — §11.2 target and gap", () => {
  it("derives the gap as max(0, target − net) from the two COMPUTED figures (§11 case shape)", () => {
    const facts = planFactsOf(closedLoss());
    expect(progressNumber(facts.target)).toBe(50);
    expect(progressNumber(facts.net)).toBeCloseTo(-7.91, 6);
    // The §11 presentation case: −7.91 against 50 ⇒ 57.91 short. The relation is
    // what is asserted; the module hardcodes none of these numbers.
    expect(progressNumber(facts.gap)).toBeCloseTo(57.91, 6);
  });

  it("reports the gap as a DISTANCE: never negative once the target is met", () => {
    const winner = closedLoss({
      economics: {
        computation_status: "computed",
        amount_in_usd: 100,
        gross_profit_usd: 80,
        total_cost_usd: 10,
        net_profit_usd: 70,
        gas_usd: 10,
        target_net_usd: 50,
        target_delta_usd: 20,
        meets_target: true,
        not_computed_reasons: {},
      },
    });
    const facts = planFactsOf(winner);
    expect(progressNumber(facts.gap)).toBe(0);
    expect(facts.meets_target).toBe(true);
  });

  it("an absent target is ABSENT with the producer's own reason — never a 0 gap", () => {
    const noTarget = row({
      economics: {
        computation_status: "computed",
        amount_in_usd: 100,
        gross_profit_usd: 10,
        total_cost_usd: 17.91,
        net_profit_usd: -7.91,
        gas_usd: 12,
        flash_fee_usd: 5.91,
        not_computed_reasons: { target_net_usd: "no_target_configured" },
      },
    });
    const facts = planFactsOf(noTarget);
    expect(facts.target.state).toBe("ABSENT");
    if (facts.target.state === "ABSENT") {
      expect(facts.target.reason).toBe("no_target_configured");
    }
    expect(facts.gap.state).toBe("ABSENT");
    expect(progressNumber(facts.gap)).toBeNull(); // NEVER coerced to 0
    expect(facts.meets_target).toBeNull(); // not inferred
  });

  it("a gap needs BOTH figures: a net without a target is not a zero gap", () => {
    const facts = planFactsOf(row({ net_expected_profit_usd: -7.91, expected_profit_usd: 10 }));
    // The (gross, net) pair alone closes a canonical ladder but has no target.
    expect(facts.net.state).toBe("COMPUTED");
    expect(facts.gap.state).toBe("ABSENT");
    expect(progressNumber(facts.gap)).toBeNull();
  });
});

describe("search-state — §11.5 the best NEGATIVE survives, and three states stay distinct", () => {
  const loss = planFactsOf(closedLoss({ id: "loss" }));
  const worseLoss = planFactsOf(
    closedLoss({
      id: "worse",
      net_expected_profit_usd: -40,
      expected_profit_usd: 10,
      economics: null,
    }),
  );
  const zero = planFactsOf(row({ id: "zero", expected_profit_usd: 5, net_expected_profit_usd: 0 }));
  const winner = planFactsOf(
    closedLoss({
      id: "win",
      economics: {
        computation_status: "computed",
        amount_in_usd: 100,
        gross_profit_usd: 80,
        total_cost_usd: 10,
        net_profit_usd: 70,
        gas_usd: 10,
        target_net_usd: 50,
        target_delta_usd: 20,
        meets_target: true,
        not_computed_reasons: {},
      },
    }),
  );

  it("keeps the loss CLOSEST to zero as the best negative (not the worst)", () => {
    const best = selectBestPlans([worseLoss, loss]);
    expect(best.best_negative?.opportunity_id).toBe("loss");
    expect(progressNumber(best.best_negative!.net)).toBeCloseTo(-7.91, 6);
    expect(best.best_positive).toBeNull();
  });

  it("a computed ZERO is its own state — neither positive nor an absence", () => {
    const best = selectBestPlans([zero, loss]);
    expect(best.best_zero?.opportunity_id).toBe("zero");
    expect(best.best_positive).toBeNull();
    expect(best.best_negative?.opportunity_id).toBe("loss");
  });

  it("a positive plan never erases the negative one", () => {
    const best = selectBestPlans([winner, loss]);
    expect(best.best_positive?.opportunity_id).toBe("win");
    expect(best.best_negative?.opportunity_id).toBe("loss");
  });

  it("with no positive at all the phase is `searching` and the reason SAYS the negative is kept", () => {
    const state = deriveStrategySearchState("1|dex_arb", [closedLoss()], { nowMs: null });
    expect(state.phase).toBe("searching");
    expect(state.phase_reason).toContain("best negative retained");
    expect(state.best.best_negative).not.toBeNull();
  });

  it("diagnostics stay visible although no candidate is profitable", () => {
    const state = deriveStrategySearchState(
      "1|dex_arb",
      [closedLoss({ rejection_reason: "spread_zero_equilibrium" })],
      { nowMs: null },
    );
    expect(state.diagnostics).toEqual([
      { plan_key: state.plans[0]!.plan_key, reason: "spread_zero_equilibrium" },
    ]);
  });

  it("an empty inventory is `no_observations` — not a silent zero", () => {
    const state = deriveStrategySearchState("1|dex_arb", [], { nowMs: null });
    expect(state.phase).toBe("no_observations");
    expect(state.current).toBeNull();
    expect(state.best).toEqual({ best_positive: null, best_zero: null, best_negative: null });
  });

  it("stale observations are `search_stalled`, never `searching`", () => {
    const now = Date.parse("2026-10-03T11:00:00Z"); // 1 h after the detection
    const state = deriveStrategySearchState("1|dex_arb", [closedLoss()], {
      nowMs: now,
      vigencyMs: 300_000,
    });
    expect(state.phase).toBe("search_stalled");
  });
});

describe("search-state — §11.3 real progress, or ABSENT with its reason", () => {
  it("with NO producer the producer-only metrics are ABSENT (never a false zero)", () => {
    const target = planFactsOf(closedLoss());
    void target;
    const state = deriveStrategySearchState("1|dex_arb", [closedLoss()], { nowMs: null });
    for (const key of ["budget"] as const) {
      const cell = state.progress[key];
      expect(cell.state).toBe("ABSENT");
      if (cell.state === "ABSENT") expect(cell.reason).toBe(ABSENT_REASONS.noProducer);
    }
    // No snapshot has been accepted yet either.
    expect(state.progress.snapshot_age_ms.state).toBe("ABSENT");
    // No improvement has been observed (there is no prior best to improve on).
    expect(state.progress.last_improvement.state).toBe("ABSENT");
  });

  it("a client census is COMPUTED and NAMED as a client census, not as a producer counter", () => {
    const state = deriveStrategySearchState(
      "1|dex_arb",
      [
        closedLoss({ id: "a" }),
        closedLoss({ id: "b", detected_at: "2026-10-03T10:04:00Z" }),
      ],
      { nowMs: null },
    );
    const routes = state.progress.routes_explored;
    expect(routes.state).toBe("COMPUTED");
    if (routes.state === "COMPUTED") {
      expect(routes.scope).toBe("client_observed_inventory");
      expect(routes.value).toBe(1); // one route, two detections of it
    }
  });

  it("the quote census is only computed over rows that EMIT economics (R10)", () => {
    const noEconomics = deriveStrategySearchState(
      "1|dex_arb",
      [row({ id: "no-econ" })],
      { nowMs: null },
    );
    expect(noEconomics.progress.quotes_exact.state).toBe("ABSENT");
    if (noEconomics.progress.quotes_exact.state === "ABSENT") {
      expect(noEconomics.progress.quotes_exact.reason).toBe(ABSENT_REASONS.economicsNotEmitted);
    }
    expect(noEconomics.progress.quotes_error.state).toBe("ABSENT");
  });

  it("a real census of zero IS computed (0 is a measurement, not an absence)", () => {
    const state = deriveStrategySearchState("1|dex_arb", [closedLoss()], { nowMs: null });
    const incomplete = state.progress.quotes_incomplete;
    expect(incomplete.state).toBe("COMPUTED");
    if (incomplete.state === "COMPUTED") {
      expect(incomplete.value).toBe(0);
      expect(incomplete.scope).toBe("client_observed_inventory");
    }
  });

  it("sizes_quoted counts DISTINCT declared notions, and is ABSENT when none was declared", () => {
    const declared = (id: string, wei: string) =>
      closedLoss({
        id,
        route_metadata: {
          token_addresses: ["0xaaa", "0xbbb"],
          pool_addresses: ["0xp1"],
          dex_adapters: ["uniswap_v2"],
          economics_amount_in_wei: wei,
        },
      });
    const two = deriveStrategySearchState(
      "1|dex_arb",
      [declared("a", "1000000"), declared("b", "2000000")],
      { nowMs: null, publisher: null },
    );
    expect(progressNumber(two.progress.sizes_quoted)).toBe(2);

    const none = deriveStrategySearchState("1|dex_arb", [closedLoss()], { nowMs: null });
    expect(none.progress.sizes_quoted.state).toBe("ABSENT");
    if (none.progress.sizes_quoted.state === "ABSENT") {
      expect(none.progress.sizes_quoted.reason).toBe(ABSENT_REASONS.noDeclaredNotional);
    }
  });

  it("a producer declaration takes the server_session scope and is used verbatim", () => {
    const state = deriveStrategySearchState("1|dex_arb", [closedLoss()], {
      nowMs: null,
      publisher: {
        routes_explored: 17,
        sizes_quoted: 4,
        quotes_exact: 9,
        quotes_incomplete: 2,
        quotes_error: 1,
        budget_exhausted: true,
        budget_reason: "expansion_budget_exhausted",
      },
    });
    expect(progressNumber(state.progress.routes_explored)).toBe(17);
    expect(progressNumber(state.progress.budget)).toBe(1);
    const cell = state.progress.routes_explored;
    if (cell.state === "COMPUTED") expect(cell.scope).toBe("server_session");
  });

  it("an elapsed-time metric with NO clock is ABSENT — never a fabricated 0 age", () => {
    // The server render has no clock (R1): a 0 here would claim the snapshot
    // had just arrived, which is exactly the false zero R8/R10 forbid.
    const state = deriveStrategySearchState("1|dex_arb", [closedLoss()], {
      nowMs: null,
      snapshotReceivedAtMs: Date.parse("2026-10-03T10:00:00Z"),
      lastImprovementMs: Date.parse("2026-10-03T10:00:00Z"),
    });
    expect(state.progress.snapshot_age_ms).toEqual({
      state: "ABSENT",
      reason: ABSENT_REASONS.noClock,
    });
    expect(state.progress.last_improvement).toEqual({
      state: "ABSENT",
      reason: ABSENT_REASONS.noClock,
    });
    expect(progressNumber(state.progress.snapshot_age_ms)).toBeNull();
  });

  it("snapshot age is ABSENT until a snapshot is actually received, then measured", () => {
    const received = Date.parse("2026-10-03T10:00:00Z");
    const state = deriveStrategySearchState("1|dex_arb", [closedLoss()], {
      nowMs: received + 4_000,
      snapshotReceivedAtMs: received,
    });
    expect(progressNumber(state.progress.snapshot_age_ms)).toBe(4_000);
  });
});

describe("search-state — §11.3 last improvement comes from EFFECTIVE SAMPLES", () => {
  it("records only a STRICT improvement, stamped with the observation's own clock", () => {
    const first = planFactsOf(closedLoss({ id: "a" }));
    const after = trackImprovement({ ...EMPTY_IMPROVEMENT_STATE }, first);
    expect(after.bestNetSeen).toBeCloseTo(-7.91, 6);
    expect(after.lastImprovementMs).toBe(Date.parse("2026-10-03T10:00:00Z"));

    // A WORSE observation is not an improvement…
    const worse = planFactsOf(
      closedLoss({ id: "b", net_expected_profit_usd: -40, expected_profit_usd: 10, economics: null }),
    );
    expect(trackImprovement(after, worse)).toBe(after);

    // …and a better one updates the stamp with ITS OWN clock, never Date.now().
    const better = planFactsOf(
      closedLoss({
        id: "c",
        detected_at: "2026-10-03T10:09:00Z",
        economics: {
          computation_status: "computed",
          amount_in_usd: 100,
          gross_profit_usd: 20,
          total_cost_usd: 17.91,
          net_profit_usd: 2.09,
          gas_usd: 12,
          flash_fee_usd: 5.91,
          not_computed_reasons: {},
        },
      }),
    );
    const improved = trackImprovement(after, better);
    expect(improved.bestNetSeen).toBeCloseTo(2.09, 6);
    expect(improved.lastImprovementMs).toBe(Date.parse("2026-10-03T10:09:00Z"));
  });
});

describe("search-state — lifecycle stages are asserted only by the field that declares them", () => {
  it("a POSITIVE number is not a target, and a computed quote is not a submission", () => {
    const winner = planFactsOf(
      closedLoss({
        economics: {
          computation_status: "computed",
          amount_in_usd: 100,
          gross_profit_usd: 80,
          total_cost_usd: 10,
          net_profit_usd: 70,
          gas_usd: 10,
          target_net_usd: 50,
          target_delta_usd: 20,
          meets_target: null, // the producer did NOT declare the predicate
          not_computed_reasons: {},
        },
      }),
    );
    const stages = lifecycleOf(winner);
    const by = (k: string) => stages.find((s) => s.key === k)!;
    expect(by("number_positive").reached).toBe(true);
    expect(by("target_met").reached).toBe(false); // not inferred from the sign
    expect(by("submitted").reached).toBe(false);
    expect(by("included").reached).toBe(false);
    expect(by("settled").reached).toBe(false);
    expect(by("reconciled").reached).toBe(false);
    // Each chip names the wire field it read.
    expect(by("target_met").source).toBe("economics.meets_target");
  });

  it("the canonical lifecycle statuses drive submission/inclusion/reconciliation", () => {
    const stages = lifecycleOf(planFactsOf(closedLoss({ status: "reconciled" })));
    const by = (k: string) => stages.find((s) => s.key === k)!;
    expect(by("submitted").reached).toBe(true);
    expect(by("included").reached).toBe(true);
    expect(by("reconciled").reached).toBe(true);
    // …and the chip SAYS the stage was implied by a later status, not observed
    // directly. An implied stage is never dressed as an observed one.
    expect(by("submitted").source).toBe("status=reconciled ≥ executing");
  });

  it("a REJECTED row claims no lifecycle stage", () => {
    const stages = lifecycleOf(planFactsOf(closedLoss({ status: "rejected" })));
    const by = (k: string) => stages.find((s) => s.key === k)!;
    expect(by("submitted").reached).toBe(false);
    expect(by("included").reached).toBe(false);
    expect(by("reconciled").reached).toBe(false);
    expect(by("submitted").source).toContain("fuera del orden canónico");
  });
});

describe("search-state — SIM/LIVE isolation (no cross-basis borrowing)", () => {
  it("a SIM-only row paints the simulated basis and does NOT borrow a canonical gross", () => {
    const sim = planFactsOf(
      row({
        simulated_gross_usd: 12,
        simulated_costs_total_usd: 3,
        simulated_net_profit_usd: 9,
        simulated_amount_in_usd: 500,
        simulated_cost_breakdown: {
          gas_usd: 1,
          lp_fees_usd: 0,
          slippage_usd: 0,
          failure_buffer_usd: 1,
          copied_buffer_usd: 0,
          capital_cost_usd: 0,
          ops_overhead_usd: 1,
          flashloan_fee_usd: 0,
          relay_fee_usd: 0,
        },
      }),
    );
    expect(sim.basis).toBe("simulated");
    expect(progressNumber(sim.net)).toBe(9);
    // No target on the wire ⇒ ABSENT, not a 0 borrowed from anywhere.
    expect(sim.target.state).toBe("ABSENT");
    expect(sim.gap.state).toBe("ABSENT");
  });

  it("a canonical-only row never borrows the SIM ladder", () => {
    const canonical = planFactsOf(closedLoss());
    expect(canonical.basis).toBe("canonical");
    expect(progressNumber(canonical.gross)).toBe(10);
    expect(progressNumber(canonical.net)).toBeCloseTo(-7.91, 6);
  });

  it("a row with NO figure at all is basis `none`: every money cell ABSENT (R8)", () => {
    const quiet = planFactsOf(row({ id: "no-figures" }));
    expect(quiet.basis).toBe("none");
    expect(quiet.net.state).toBe("ABSENT");
    expect(progressNumber(quiet.net)).toBeNull();
    expect(quiet.gross.state).toBe("ABSENT");
    expect(quiet.target.state).toBe("ABSENT");
    expect(quiet.gap.state).toBe("ABSENT");
  });

  it("a NON-closing ladder still PUBLISHES its wire figures with the reason attached", () => {
    // OPERATOR ORDER 2026-09-27 ("quita el render que esconde los números"):
    // the ledger publishes a present figure even when the ladder does not close.
    // This panel inherits that rule — it must not re-hide what the SSOT shows —
    // and the reason travels on `basis_reason` for the cell's tooltip.
    const published = planFactsOf(
      row({ expected_profit_usd: 10, net_expected_profit_usd: 20 }),
    );
    expect(published.basis).toBe("canonical");
    expect(published.basis_reason).toContain("ladder not closed");
    expect(progressNumber(published.net)).toBe(20);
    expect(progressNumber(published.gross)).toBe(10);
  });
});

describe("search-state — §11.5 a display cap never loses the inventory", () => {
  it("reports what it hid", () => {
    const capped = capInventory([1, 2, 3, 4, 5], 2);
    expect(capped.shown).toEqual([1, 2]);
    expect(capped.hidden).toBe(3);
    expect(capped.total).toBe(5);
  });

  it("the strategy state counts the FULL inventory even when a cap is applied elsewhere", () => {
    const rows = Array.from({ length: 7 }, (_, i) =>
      closedLoss({ id: `opp-${i}`, detected_at: `2026-10-03T10:0${i}:00Z` }),
    );
    const state = deriveStrategySearchState("1|dex_arb", rows, { nowMs: null });
    expect(state.inventory_size).toBe(7);
    expect(state.plans).toHaveLength(7);
    // Newest first, so the UI's first row is the live observation.
    expect(state.plans[0]!.opportunity_id).toBe("opp-6");
  });
});
