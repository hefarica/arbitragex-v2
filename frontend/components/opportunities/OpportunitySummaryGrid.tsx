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
// CARDS-NOTIONAL-01 — the same SSOT the card ladder and the notifier use.
import {
  buildLedger,
  grossIsAttributableToPrincipal,
  GROSS_OVER_PRINCIPAL_SANITY_MULT,
} from "@/lib/opportunity-ledger";
// CARDS-FALSEZERO-01 — the shared sub-cent renderer (see `lib/format.ts`).
import { formatSubCentUsd, SUB_CENT_USD } from "@/lib/format";

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
  // CARDS-FALSEZERO-01 (2026-09-27): a real zero stays `$0.00` (R8: Some(0)),
  // and a NONZERO figure below half a cent never collapses onto it. Measured on
  // the live feed: `net_expected_profit_usd = -0.000012` painted `-$0.00` on 26
  // of 323 unique rows — the operator read "exactly zero" for a computed loss.
  if (abs === 0) return "$0.00";
  if (abs >= 1e15) return `${sign}$${abs.toExponential(2).replace("e+", "e")}`;
  if (abs >= 1e12) return `${sign}$${(abs / 1e12).toFixed(2)}T`;
  if (abs >= 1e9) return `${sign}$${(abs / 1e9).toFixed(2)}B`;
  if (abs >= 1e6) return `${sign}$${(abs / 1e6).toFixed(2)}M`;
  if (abs < SUB_CENT_USD) return `${sign}$${formatSubCentUsd(abs)}`;
  return `${sign}$${abs.toFixed(digits)}`;
}

function summaryCells(opp: OmniOpportunity): Array<{
  label: string;
  value: string;
  title: string;
}> {
  // CARDS-NOTIONAL-01 (2026-09-26): this grid is where the two figures that
  // contradict each other on the live card are painted side by side —
  // `in  $0.00` (the SIM's notional, from `amount_in_wei`) and
  // `Gross $1.47M` (the searcher's fast-filter gross at ITS own size). Neither
  // number is individually wrong, but the PORTRAIT is, because a reader takes
  // "in" and "Gross" to be two cells of one arithmetic.
  //
  // `buildLedger` decides which closed arithmetic the row actually has. The
  // grid then (a) only publishes a notional when the shown gross belongs to
  // that notional, and (b) labels every economic cell with the producer it came
  // from, so no two cells can be read as one ladder unless they are one.
  const ledger = buildLedger(opp);
  // CARDS-PRECEDENCE-02 (2026-09-26): the canonical ratio wins; when it is
  // absent but the TS forward-sim computed one, the SIMULATED value is the
  // computed value and must be displayed (marked `~`) instead of an empty cell.
  // Live evidence (GET /api/opportunities/live): `roi_pct` is null on 38/38 rows
  // while `simulated_roi_pct` is non-null on 3 — those 3 cards carried a
  // computed ROI that this cell never showed.
  //
  // CARDS-NOTIONAL-01: the ratio must belong to the SAME notional as the cell
  // above it, so the precedence now follows the rendered ladder's basis rather
  // than a fixed field order. A ratio measured on the ladder's own notional is
  // bounded by that notional; a ratio borrowed from another producer is not.
  const roiPct =
    ledger.basis === "canonical"
      ? opp.economics?.roi_pct ?? opp.roi_pct ?? null
      : ledger.basis === "simulated"
        ? opp.simulated_roi_pct ?? null
        : null;
  const roiIsSimulated = ledger.basis === "simulated" && roiPct != null;
  const bps = roiPct != null ? `${roiIsSimulated ? "~" : ""}${(roiPct * 100).toFixed(0)}` : null;
  // CARDS-NOTIONAL-02 (2026-09-27) — the `in` cell stops hiding a computed
  // notional.
  //
  // Measured on the live feed (2026-09-27T02:0xZ): 5 rows carried
  // `simulated_amount_in_usd` ($0.0000044 … $0.99987154) AND a searcher gross
  // measured at another size (e.g. gross 710273.79750992 vs principal
  // 0.99987154). `grossIsAttributableToPrincipal` rightly refuses to present the
  // pair as ONE arithmetic, but the cell's only two options were "paint the
  // number" or "paint the dash" — so it painted `—`: a figure the forward-sim
  // HAD computed vanished from the card. The operator's rule is the opposite
  // ("si el kernel computó una cifra, la card debe mostrarla").
  //
  // The fix keeps CARDS-NOTIONAL-01 intact by separating the two surfaces it
  // always distinguished (see `opportunity-ledger.ts`: the canonical figures
  // "are still displayed — in the summary grid, marked with their own basis —
  // they simply stop masquerading as one capital path"):
  //   · the LADDER (OpportunityTradeCard's "Capital path") stays quiet — no
  //     principal is ever painted beside a gross of another size;
  //   · the GRID publishes the notional with the SAME `~` mark the Net/bps/Sim
  //     cells already use for "this figure comes from the forward-sim producer",
  //     and the cell `title` states the machine reason verbatim (the 5× bound
  //     and the two sizes), so nothing competes with a number and nothing hides.
  // R8: still never a fabricated 0 — an absent notional keeps its dash.
  const notionalUsd =
    opp.simulated_amount_in_usd != null && Number.isFinite(opp.simulated_amount_in_usd)
      ? opp.simulated_amount_in_usd
      : null;
  const notionalAttributable = grossIsAttributableToPrincipal(
    opp.expected_profit_usd,
    notionalUsd,
  );
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
      // CARDS-NOTIONAL-01/02: the notional published here is the CLOSED ladder's
      // own principal (`ledger.principal_usd`) whenever it is verifiable — the
      // exact size the `Gross` cell beside it was measured at. `buildLedger`
      // returns `principal_usd: null` for the case this cell used to hide: a gross
      // measured at ANOTHER size (the `in $0.00` beside `Gross $1.47M` the
      // operator photographed). main rendered the dash there; this fix shows the
      // forward-sim's own notional instead, marked `~` unless the searcher's gross
      // is attributable to it under the searcher's OWN
      // `SANITY_PROFIT_MULT_OF_CAP` (5×) bound rather than a threshold
      // re-invented here. The two surfaces stay separated: the LADDER never
      // publishes a principal beside a gross of another size, and the GRID never
      // hides a figure the forward-sim did compute — the machine reason (the two
      // sizes, the 5× bound) travels in the `title`. R8: an absent notional keeps
      // its dash, never a fabricated 0.
      ledger.principal_usd != null
        ? usd(ledger.principal_usd)
        : notionalUsd != null
          ? `${notionalAttributable ? "" : "~"}${usd(notionalUsd)}`
          : null,
      ledger.principal_usd != null
        ? ledger.basis === "canonical"
          ? `economics.amount_in_usd del sizing kernel; amount_in_wei=${opp.economics?.amount_in_wei ?? opp.amount_in_wei ?? "no emitido"}`
          : `simulated_amount_in_usd del mismo forward-sim (basis=${ledger.basis})`
        : notionalUsd == null
          ? `sin principal verificable para el basis=${ledger.basis} (R8)`
          : notionalAttributable
            ? `simulated_amount_in_usd del forward-sim valorado al precio vivo (amount_in_wei=${opp.amount_in_wei ?? "no emitido"}) — el ladder no publica principal (basis=${ledger.basis})`
            : `CARDS-NOTIONAL-01/02: notional del forward-sim (amount_in_wei=${opp.amount_in_wei ?? "no emitido"}); el bruto mostrado (${opp.expected_profit_usd}) se midió en OTRO tamaño (>${GROSS_OVER_PRINCIPAL_SANITY_MULT}× este principal) — '~' marca el origen y el ladder no publica principal`,
    ),
    cell(
      "Gross",
      ledger.gross_usd != null ? usd(ledger.gross_usd) : null,
      ledger.basis === "canonical"
        ? "economics.gross_profit_usd del searcher al mismo sized notional que `in`"
        : "simulated_gross_usd del mismo forward-sim",
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
      ledger.net_usd != null
        ? `${ledger.basis === "simulated" ? "~" : ""}${usd(ledger.net_usd)}`
        : null,
      ledger.basis === "canonical"
        ? "economics.net_profit_usd del searcher; mismo sized notional"
        : "simulated_net_profit_usd del mismo forward-sim — '~' marca el origen",
    ),
    cell(
      "bps",
      bps,
      // CARDS-PRECEDENCE-02: same rule — `roi_pct` is the canonical ratio and it
      // is null on 38/38 live rows today, while `simulated_roi_pct` IS computed
      // on 3 of them. The cell fell back to nothing instead of to that computed
      // value; `~` marks the simulated source.
      //
      // CARDS-NOTIONAL-01: the ratio is now the one measured on the RENDERED
      // ladder's notional (see the precedence above), so `bps` is bounded by the
      // same notional as the cells it sits beside — `~1513332276940974` was
      // `net_sim / $0.0000045 × 100`, i.e. a ratio whose denominator the reader
      // could not see.
      roiPct != null
        ? `${roiIsSimulated ? "simulated_roi_pct" : "economics.roi_pct"} ${roiPct.toFixed(4)}% × 100 — mismo basis=${ledger.basis}`
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
      "block",
      opp.economics?.quote_block != null ? String(opp.economics.quote_block) : null,
      "economics.quote_block — bloque real de la quote/reservas usada por el searcher",
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
