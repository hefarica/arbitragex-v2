"use client";

/**
 * FE-0033 (§36) — the canonical opportunity summary grid.
 *
 * One compact, wire-grade summary of the row: ruta / strategy / detector /
 * hops / in / Gross / Net / bps / Risk / Sim / latencia. Pure props over a
 * single OmniOpportunity — the TradeCard mounts it now and the FE-0034
 * detail dialog reuses it as the Overview spine (§26: one model, one
 * summary; no parallel rendering of the same fields).
 *
 * Column → wire source (all null-honest; §28/§29 discipline):
 *   ruta      dex_a → dex_b
 *   strategy  strategy_kind (null ⇒ "—")
 *   detector  detector_id (WO-CARDS-COMPLETE-01); null ⇒ "no emitido"
 *             (nivel-(b) resuelto — el campo YA se emite)
 *   hops      hop_count — route_metadata-grade ONLY (FE-0028); null ⇒ "—",
 *             never the §29 synthetic count
 *   in        simulated_amount_in_usd when computed (amount_in_wei in the
 *             title); "—" when neither exists
 *   Gross     expected_profit_usd
 *   Net       net_expected_profit_usd (canonical spine net)
 *   bps       roi_pct × 100 — a UNIT conversion of a wire value, not a new
 *             verdict; null ⇒ "—"
 *   Risk      risk_score
 *   Sim       simulated_net_profit_usd as a VALUE ("~$x"). No PASS verdict:
 *             the wire persists no simulation verdict (§79 — the FE never
 *             recomputes one); the only PASS/FAIL on this card is the
 *             strategy-target verdict, owned by meets_target_at_cap.
 *   latencia  pipeline_latency_ms (WO-CARDS-COMPLETE-01); null ⇒ "no emitido"
 *             (nivel-(b) resuelto).
 * Null economics with a rejection_reason on the row render "no computado"
 * with the R8 reason in the title (WO-CARDS-COMPLETE-01) — a bare dash only
 * when there is no reason to state (R8: absence is a state, not silence).
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import type { OmniOpportunity } from "@/lib/store/types";

const DASH = "—";
export const NOT_EMITTED = "no emitido";
/**
 * @deprecated CARDS-QUIET-01 (2026-09-26, operator order): the loud
 * `"no computado"` string is no longer rendered. Operator: "I am not interested
 * in 'no computado'; what I need is a value — the CORRECT value." A cell with no
 * computed value renders the quiet `DASH` (the honest empty state, R8) and the
 * machine reason travels in the cell `title` so nothing is hidden, only not
 * shouted. Kept exported for the audit trail / any external reader.
 */
export const NOT_COMPUTED = "no computado";

/**
 * Exact USD for the wire-grade summary cells (`$10.50`, `-$1.50`).
 *
 * CARDS-MAGNITUDE-01 (2026-09-26): below $1M the digits stay EXACT — this grid
 * is the wire-grade view and its pinned tests depend on `$4500.00`. Above $1M
 * the value collapses onto the T/B/M ladder instead of printing an unbounded
 * mantissa: PROVEN on the live feed, a route sized against a 6-decimal opening
 * token carried `simulated_amount_in_usd = 999935091316.8`, and the `in` cell
 * painted the 12-digit blob `$999935091316.80` — the operator's "suspicious
 * magnitude". A figure nobody can read is not a value displayed.
 * R8: non-finite never renders a number.
 */
function usd(v: number, digits = 2): string {
  if (!Number.isFinite(v)) return DASH;
  const abs = Math.abs(v);
  const sign = v < 0 ? "-" : "";
  if (abs >= 1e15) return `${sign}$${abs.toExponential(2).replace("e+", "e")}`;
  if (abs >= 1e12) return `${sign}$${(abs / 1e12).toFixed(2)}T`;
  if (abs >= 1e9) return `${sign}$${(abs / 1e9).toFixed(2)}B`;
  if (abs >= 1e6) return `${sign}$${(abs / 1e6).toFixed(2)}M`;
  return `${sign}$${abs.toFixed(digits)}`;
}

function summaryCells(opp: OmniOpportunity): Array<{
  label: string;
  value: string;
  title: string;
}> {
  // CARDS-PRECEDENCE-02 (2026-09-26): the canonical ratio wins; when it is
  // absent but the TS forward-sim computed one, the SIMULATED value is the
  // computed value and must be displayed (marked `~`) instead of an empty cell.
  // Live evidence (GET /api/opportunities/live): `roi_pct` is null on 38/38 rows
  // while `simulated_roi_pct` is non-null on 3 — those 3 cards carried a
  // computed ROI that this cell never showed.
  const roiPct = opp.roi_pct ?? opp.simulated_roi_pct ?? null;
  const roiIsSimulated = opp.roi_pct == null && opp.simulated_roi_pct != null;
  const bps = roiPct != null ? `${roiIsSimulated ? "~" : ""}${(roiPct * 100).toFixed(0)}` : null;
  // CARDS-QUIET-01 (2026-09-26, operator order): a null economic value renders
  // the QUIET empty state (`DASH`), never a loud "no computado" wall.
  //
  // Precedence rule: a COMPUTED value always wins the cell; when there is no
  // computed value the cell goes quiet — a placeholder must never compete with
  // (or stand in for) a number. This is what the operator reported: cards
  // showing `no computado` across RUTA/STRATEGY/DETECTOR/HOPS/SIM/LATENCIA next
  // to real figures, i.e. a placeholder wall drowning the values. It was loud
  // by construction: the old branch fired on EVERY null economic cell of ANY
  // row carrying a rejection_reason — and on the live feed that is 41/41 rows
  // (36 of them with 6 such cells), because a rejected row has null economics
  // by definition.
  //
  // R8/RULE 00 are preserved, not weakened: absence stays absence (never 0,
  // never a fabricated figure), and the machine reason is still surfaced — it
  // moves from the cell body to the `title` tooltip, so the information is one
  // hover away instead of occupying the value slot. `rejection_reason` itself
  // is still rendered verbatim by the card's rejection banner and by the
  // detail view's "Rejection Reason" row.
  const cell = (
    label: string,
    v: string | null,
    title: string,
  ): { label: string; value: string; title: string } =>
    v != null
      ? { label, value: v, title }
      : {
          label,
          value: DASH,
          title:
            opp.rejection_reason != null
              ? `${title} · no computado: ${opp.rejection_reason} (R8)`
              : title,
        };
  return [
    {
      label: "ruta",
      value: opp.dex_a ? (opp.dex_b ? `${opp.dex_a} → ${opp.dex_b}` : opp.dex_a) : DASH,
      title: "dex_a → dex_b del wire",
    },
    {
      label: "strategy",
      value: opp.strategy_kind ?? DASH,
      title:
        opp.strategy_kind == null
          ? "strategy_kind ausente en el payload (§28)"
          : "strategy_kind del wire",
    },
    {
      // CARDS-QUIET-01: same precedence rule as `cell` — a computed detector_id
      // wins; absent goes QUIET (the "no emitido" marker moves to the title).
      label: "detector",
      value: opp.detector_id ?? DASH,
      title:
        opp.detector_id == null
          ? "detector_id ausente en el payload — no emitido (nivel-(b) resuelto, R8)"
          : "detector_id del wire",
    },
    {
      label: "hops",
      value: opp.hop_count != null ? String(opp.hop_count) : DASH,
      title:
        opp.hop_count == null
          ? "sin route_metadata persistida — hop_count null (FE-0028), jamás el conteo sintético §29"
          : "hop_count = route_metadata.dex_adapters.length",
    },
    cell(
      "in",
      opp.simulated_amount_in_usd != null ? usd(opp.simulated_amount_in_usd) : null,
      `amount_in_wei=${opp.amount_in_wei ?? "no emitido"} · USD solo cuando la simulación lo computa (R8)`,
    ),
    cell(
      "Gross",
      opp.expected_profit_usd != null ? usd(opp.expected_profit_usd) : null,
      "expected_profit_usd (gross, pre-costos)",
    ),
    cell(
      "Net",
      // CARDS-PRECEDENCE-02: the CARD HEADLINE already renders
      // `net_expected_profit_usd ?? simulated_net_profit_usd` (canonical spine
      // net, else the TS forward-sim net). This cell used only the canonical
      // field, so a row whose ONLY computed net is the simulated one rendered a
      // quiet dash here while the same card showed `~$x SIM` two rows above —
      // one field, two precedence rules, i.e. a computed value losing to a
      // placeholder. Same rule here now, with the same `~` source mark.
      opp.net_expected_profit_usd != null
        ? usd(opp.net_expected_profit_usd)
        : opp.simulated_net_profit_usd != null
          ? `~${usd(opp.simulated_net_profit_usd)}`
          : null,
      opp.net_expected_profit_usd != null
        ? "net_expected_profit_usd (spine canónico)"
        : "simulated_net_profit_usd (TS forward-sim; canónico pendiente) — '~' marca el origen",
    ),
    cell(
      "bps",
      bps,
      // CARDS-PRECEDENCE-02: same rule — `roi_pct` is the canonical ratio and it
      // is null on 38/38 live rows today, while `simulated_roi_pct` IS computed
      // on 3 of them. The cell fell back to nothing instead of to that computed
      // value; `~` marks the simulated source.
      opp.roi_pct != null
        ? `roi_pct ${opp.roi_pct.toFixed(4)}% × 100 — conversión de unidad, no un veredicto`
        : opp.simulated_roi_pct != null
          ? `simulated_roi_pct ${opp.simulated_roi_pct.toFixed(4)}% × 100 — '~' marca el origen (canónico pendiente)`
          : "roi_pct no computado (R8)",
    ),
    cell(
      "Risk",
      // FRONT-05 fix (2026-09-24): risk_score is a 0-1 FRACTION on the wire;
      // the raw display (0.11) contradicted formatRiskOrDash's "11.0%" for
      // the same field. Unify to percent via the same formula.
      opp.risk_score != null ? `${(opp.risk_score * 100).toFixed(1)}%` : null,
      "risk_score del wire (fracción 0-1, renderizada como %)",
    ),
    cell(
      "Sim",
      opp.simulated_net_profit_usd != null
        ? `~${usd(opp.simulated_net_profit_usd)}`
        : null,
      "forward-sim net como VALOR — el wire no persiste veredicto PASS/FAIL de simulación (§79); PASS/FAIL solo vive en el target verdict",
    ),
    {
      // CARDS-QUIET-01: computed latency wins; absent goes QUIET.
      label: "latencia",
      value: opp.pipeline_latency_ms != null ? `${opp.pipeline_latency_ms}ms` : DASH,
      title:
        opp.pipeline_latency_ms == null
          ? "pipeline_latency_ms ausente en el payload — no emitido (nivel-(b) resuelto, R8)"
          : "pipeline_latency_ms del wire (detección → emisión)",
    },
  ];
}

export function OpportunitySummaryGrid({ opp }: { opp: OmniOpportunity }) {
  return (
    <div
      data-testid="opportunity-summary-grid"
      className="rounded-lg border border-border bg-muted/20 p-2"
    >
      <div className="grid grid-cols-3 gap-x-2 gap-y-1 font-mono text-[11px]">
        {summaryCells(opp).map((c) => (
          <div key={c.label} className="min-w-0" title={c.title}>
            <div className="text-[9px] uppercase tracking-wide text-muted-foreground">
              {c.label}
            </div>
            <div className="truncate">{c.value}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
