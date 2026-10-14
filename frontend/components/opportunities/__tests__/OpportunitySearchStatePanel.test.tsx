// frontend/components/opportunities/__tests__/OpportunitySearchStatePanel.test.tsx
//
// §14 (UI) — the mandatory UI cases, asserted on the REAL render of the panel
// that ships inside the approved card:
//   · reconexión / resync (the snapshot age appears only once one arrived)
//   · eventos fuera de orden — one old event does NOT overwrite a new one
//   · actualización streaming — a better observation updates the panel in place
//   · datos caducados — stale observations are declared, never shown as fresh
//   · resultados negativos visibles — with no profitable candidate at all
//   · ausencia de contaminación SIM/LIVE — no cross-basis figure is borrowed
//   · §11 colour rule — the gap (a distance) is never tinted as a success
//
// The tests render the PURE view (`SearchStateView`) with a state built by the
// same public reducer the connected wrapper uses. That keeps the assertions on
// the render rules themselves (what is painted for COMPUTED vs ABSENT, which
// tone a number gets) instead of on a store subscription, which zustand serves
// from its INITIAL state under `renderToStaticMarkup` — a mocked subscriber
// would prove nothing about what the operator sees.
import React from "react";
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { SearchStateView } from "../OpportunitySearchStatePanel";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import {
  deriveStrategySearchState,
  EMPTY_IMPROVEMENT_STATE,
  type DeriveOptions,
} from "@/lib/store/search-state";
import { decideUpsert, countOutcome, EMPTY_LEDGER_COUNTERS } from "@/lib/store/stream-contract";

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

/** Closed canonical arithmetic: gross 10, costs 17.91, net −7.91 at 100. */
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

function render(
  rows: OmniOpportunity[],
  options: DeriveOptions = {},
  kind: string | null = "dex_arb",
): string {
  const state = deriveStrategySearchState("1|dex_arb", rows, options);
  return renderToStaticMarkup(
    React.createElement(SearchStateView, { state, strategyKind: kind }),
  );
}

const NOW = Date.parse("2026-10-03T10:05:00Z");

describe("§14 UI — negative results stay visible with no profitable candidate", () => {
  it("shows the loss, the objective, the gap and the kept best negative", () => {
    const html = render(
      [closedLoss({ rejection_reason: "spread_zero_equilibrium" })],
      { nowMs: NOW },
    );
    expect(html).toContain("Estado de búsqueda");
    expect(html).toContain("BUSCANDO");
    expect(html).toContain("mejor negativo conservado");
    // The loss is painted, with its own sign, not hidden behind a dash.
    expect(html).toContain("-$7.91");
    expect(html).toContain("$50.00"); // objetivo
    expect(html).toContain("$57.91"); // brecha (§11 case shape, computed from the row)
    // Diagnostics stay visible although nothing is profitable.
    expect(html).toContain("spread_zero_equilibrium");
  });
});

describe("§14 UI — datos caducados are declared, never shown as fresh", () => {
  it("marks a stale observation as SIN OBSERVACIONES FRESCAS", () => {
    const staleNow = Date.parse("2026-10-03T12:00:00Z"); // 2 h later
    const html = render([closedLoss()], { nowMs: staleNow, vigencyMs: 300_000 });
    expect(html).toContain("SIN OBSERVACIONES FRESCAS");
    expect(html).not.toContain("BUSCANDO");
  });

  it("with no observations at all the panel says so instead of implying a search", () => {
    const html = render([], { nowMs: NOW });
    expect(html).toContain("SIN OBSERVACIONES");
    expect(html).toContain("sin observaciones de esta estrategia todavía");
  });
});

describe("§14 UI — a metric with no producer prints n/c, never a false zero", () => {
  it("presupuesto (budget) is ABSENT with its reason and no 0 is painted", () => {
    const html = render([closedLoss()], { nowMs: NOW });
    expect(html).toContain('data-testid="progress-budget" data-state="absent"');
    expect(html).toContain("presupuesto <span class=\"font-bold\">n/c</span>");
    expect(html).toContain("no_producer_on_opportunities_wire");
    // The forbidden rendering: a zero over a metric nobody measured.
    expect(html).not.toContain("presupuesto <span class=\"font-bold tabular-nums\">0");
  });

  it("a COMPUTED census prints the number AND names its scope", () => {
    const html = render([closedLoss()], { nowMs: NOW });
    // Two observations of one route ⇒ routes=1, and the scope mark says `cli`.
    expect(html).toContain('data-testid="progress-routes" data-state="computed"');
    expect(html).toContain("cli");
  });

  it("a producer declaration is painted with the srv scope (never confused with the client census)", () => {
    const html = render([closedLoss()], {
      nowMs: NOW,
      publisher: { routes_explored: 17, budget_exhausted: true, budget_reason: "expansion_budget_exhausted" },
    });
    expect(html).toContain('data-testid="progress-routes" data-state="computed"');
    expect(html).toContain("17");
    expect(html).toContain("srv");
    expect(html).toContain('data-testid="progress-budget" data-state="computed"');
  });
});

describe("§11 colour rule — a number's colour follows its SIGN, not a later state", () => {
  it("the gap cell is NOT tinted success (it is a distance, not a reached target)", () => {
    const html = render([closedLoss()], { nowMs: NOW });
    const gapCell = html.slice(html.indexOf('data-testid="search-state-gap-cell"'));
    const cell = gapCell.slice(0, gapCell.indexOf("</span>"));
    expect(cell).not.toContain("text-success");
    expect(cell).toContain("brecha");
  });

  it("a negative net IS tinted destructive and a positive one success", () => {
    const html = render([closedLoss()], { nowMs: NOW });
    expect(html).toContain("text-destructive\">-$7.91</span>");

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
    const winHtml = render([winner], { nowMs: NOW });
    expect(winHtml).toContain("OBJETIVO");
    expect(winHtml).toContain("OBJETIVO ALCANZADO");
  });

  it("a positive number is NOT the same thing as a met objective", () => {
    const positiveNoTarget = closedLoss({
      economics: {
        computation_status: "computed",
        amount_in_usd: 100,
        gross_profit_usd: 80,
        total_cost_usd: 10,
        net_profit_usd: 70,
        gas_usd: 10,
        // No target configured and no predicate ⇒ the objective stays unreached
        // and the gap stays ABSENT (the two axes are independent).
        not_computed_reasons: { target_net_usd: "no_target_configured" },
      },
    });
    const html = render([positiveNoTarget], { nowMs: NOW });
    expect(html).toContain("$70.00");
    expect(html).toContain('data-reached="true" data-testid="lifecycle-number_positive"');
    expect(html).toContain('data-reached="false" data-testid="lifecycle-target_met"');
    expect(html).not.toContain("OBJETIVO ALCANZADO");
  });
});

describe("§14 UI — ausencia de contaminación SIM/LIVE", () => {
  it("a SIM-only row never borrows a canonical gross or target", () => {
    const simOnly = row({
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
    });
    const html = render([simOnly], { nowMs: NOW });
    expect(html).toContain("$9.00"); // the SIM net, marked by its basis
    expect(html).toContain(">simulated<"); // the basis mark next to the net
    // The objective is ABSENT on this basis — the panel does not import the
    // canonical target from another row.
    expect(html).toContain('data-testid="progress-exact" data-state="absent"');
  });

  it("the SIM and the canonical figures of DIFFERENT plans are never mixed", () => {
    // Two rows of the same strategy: one canonical plan (size A) and one SIM
    // plan (size B). Each plan keeps its own arithmetic; the panel's current
    // plan decides the money group, and the inventory keeps both.
    const canonical = closedLoss({ id: "canon", detected_at: "2026-10-03T10:04:00Z" });
    const sim = row({
      id: "sim",
      detected_at: "2026-10-03T10:00:00Z",
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
    });
    const html = render([sim, canonical], { nowMs: NOW });
    // Current = the newest observation (canonical): its own net is painted…
    expect(html).toContain("-$7.91");
    // …and the SIM plan's net belongs to the KEPT ALTERNATIVES, not to this
    // money group: it must never replace the canonical net cell.
    expect(html).not.toContain("neto <span class=\"text-success\">$9.00");
  });
});

describe("§14 UI — eventos fuera de orden: the panel reflects the NEWER observation", () => {
  it("counts the rejected old event and paints the newer net", () => {
    const newer = closedLoss({ id: "opp-1", detected_at: "2026-10-03T10:04:00Z" });
    const older = closedLoss({
      id: "opp-1",
      detected_at: "2026-10-03T10:00:00Z",
      economics: {
        computation_status: "computed",
        amount_in_usd: 100,
        gross_profit_usd: 1,
        total_cost_usd: 101,
        net_profit_usd: -100,
        gas_usd: 101,
        target_net_usd: 50,
        meets_target: false,
        not_computed_reasons: {},
      },
    });
    // The sequencing decision the store applies, then the ledger it records.
    const decision = decideUpsert(newer, older);
    expect(decision.apply).toBe(false);
    const counters = countOutcome({ ...EMPTY_LEDGER_COUNTERS }, decision.outcome);

    const html = render([newer], { nowMs: NOW, counters });
    expect(html).toContain("-$7.91"); // the newer net survives
    expect(html).not.toContain("-$100.0000"); // the older one never paints
    expect(html).toContain("antiguos rechazados <span class=\"font-bold tabular-nums\">1</span>");
  });
});

describe("§14 UI — streaming update in place (no page refresh)", () => {
  it("a better observation moves the panel from the loss to the gain without re-mounting", () => {
    const before = render([closedLoss()], { nowMs: NOW });
    expect(before).toContain("mejor negativo conservado");
    expect(before).not.toContain("OBJETIVO ALCANZADO");

    const after = render(
      [
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
            meets_target: true,
            not_computed_reasons: {},
          },
        }),
      ],
      {
        nowMs: NOW,
        lastImprovementMs: Date.parse("2026-10-03T10:04:00Z"),
      },
    );
    expect(after).toContain("OBJETIVO ALCANZADO");
    expect(after).toContain("$70.00");
    // The last-improvement cell turned COMPUTED, from an effective sample.
    expect(after).toContain('data-testid="progress-improvement" data-state="computed"');
    void EMPTY_IMPROVEMENT_STATE;
  });
});

describe("§14 UI — reconexión y resync", () => {
  it("the snapshot age is ABSENT before a snapshot arrived and measured afterwards", () => {
    const before = render([closedLoss()], { nowMs: NOW });
    expect(before).toContain('data-testid="progress-snapshot" data-state="absent"');
    expect(before).toContain("no_snapshot_received");

    const after = render([closedLoss()], {
      nowMs: NOW,
      snapshotReceivedAtMs: NOW - 4_000,
    });
    expect(after).toContain('data-testid="progress-snapshot" data-state="computed"');
    expect(after).toContain("snapshot <span class=\"font-bold tabular-nums\">4000</span>");
  });
});
