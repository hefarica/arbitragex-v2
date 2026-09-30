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
} from "@/lib/opportunity-ledger";
// ECON-DECLARE-01 — the notional-basis declaration the producer writes, and the
// label every economic cell must carry so a value is never read as one ladder
// with a figure of a different provenance.
import {
  BASIS_LABEL,
  figureBasis,
  declarationOf,
  formatWeiNotional,
  undeclaredReason,
  type FigureBasis,
} from "@/lib/opportunity-declaration";

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

/**
 * ECON-DECLARE-01 — exact wei → a readable notional. Shared with the card's
 * ladder band so one notional never renders two ways.
 */
const formatWei = formatWeiNotional;

function summaryCells(opp: OmniOpportunity): Array<{
  label: string;
  value: string;
  title: string;
  basis: FigureBasis;
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
    basis: FigureBasis = "wire",
  ): { label: string; value: string; title: string; basis: FigureBasis } =>
    v != null
      ? { label, value: v, title, basis }
      : {
          label,
          value: DASH,
          basis,
          title:
            opp.rejection_reason != null
              ? `${title} · no computado: ${opp.rejection_reason} (R8)`
              : title,
        };

  // ECON-DECLARE-01: the bases are read ONCE per row so every cell that belongs
  // to the same producer carries the same label.
  const basisGross = figureBasis(opp, "gross");
  const basisNet = opp.net_expected_profit_usd != null ? figureBasis(opp, "net") : "sim";
  const basisAmount = figureBasis(opp, "amount");
  const declaration = declarationOf(opp);

  // ECON-DECLARE-01 — the `in` cell.
  //
  // CARDS-NOTIONAL-01 made this cell go QUIET whenever the shown gross could not
  // be attributed to `simulated_amount_in_usd`, so that `IN $0.00` could never sit
  // beside `GROSS $1.47M`. That is correct about the ARITHMETIC and wrong about
  // the DISPLAY: on the live feed it hid a value the wire carried (7 of the 18
  // rows with a gross, measured 2026-09-27T02:06Z). The declaration is what lets
  // both survive: the value is shown and its basis is stated, so a reader sees
  // two figures of two different producers instead of one broken ladder.
  //
  // Precedence: the SIM notional (the only priced one) → the producer's DECLARED
  // notional → the row's own amount (exact wei, never unit-guessed).
  const inValue: string | null =
    opp.simulated_amount_in_usd != null
      ? usd(opp.simulated_amount_in_usd)
      : declaration.notionalWei != null
        ? `${formatWei(declaration.notionalWei)} wei`
        : opp.amount_in_wei != null && opp.amount_in_wei !== "0"
          ? `${formatWei(opp.amount_in_wei)} wei`
          : null;
  const inBasis: FigureBasis =
    opp.simulated_amount_in_usd != null
      ? "sim"
      : declaration.notionalWei != null
        ? basisAmount
        : "undeclared";
  const inMisattributed =
    opp.simulated_amount_in_usd != null &&
    !grossIsAttributableToPrincipal(opp.expected_profit_usd, opp.simulated_amount_in_usd);
  const inTitle = [
    opp.simulated_amount_in_usd != null
      ? `amount_in_wei=${opp.amount_in_wei ?? "no emitido"} valorado al precio vivo — notional del ladder SIM`
      : declaration.notionalWei != null
        ? `notional DECLARADO por el productor (route_metadata.economics_amount_in_wei=${declaration.notionalWei}) — es el tamaño al que pertenecen las cifras económicas de esta fila`
        : opp.amount_in_wei != null
          ? `amount_in_wei=${opp.amount_in_wei} — el wire trae el monto en wei; sin precio no hay notional en USD (R8)`
          : `amount_in_wei no emitido y ningún productor declaró notional (R8)`,
    inMisattributed
      ? `CARDS-NOTIONAL-01: el bruto mostrado (${opp.expected_profit_usd}) NO es atribuible a este notional (${opp.simulated_amount_in_usd}) — se midieron en tamaños distintos; ambos se muestran, cada uno con su basis`
      : null,
    declaration.declared
      ? `basis declarada por el productor: gross=${BASIS_LABEL[basisGross]} net=${BASIS_LABEL[basisNet]} amount=${BASIS_LABEL[basisAmount]}`
      : undeclaredReason("route_metadata.economics_basis"),
  ]
    .filter((s): s is string => s != null)
    .join(" · ");

  return [
    {
      label: "ruta",
      value: opp.dex_a ? (opp.dex_b ? `${opp.dex_a} → ${opp.dex_b}` : opp.dex_a) : DASH,
      title: "dex_a → dex_b del wire",
      basis: "wire",
    },
    {
      label: "strategy",
      value: opp.strategy_kind ?? DASH,
      title:
        opp.strategy_kind == null
          ? "strategy_kind ausente en el payload (§28)"
          : "strategy_kind del wire",
      basis: "wire",
    },
    {
      label: "detector",
      value: opp.detector_id ?? DASH,
      title:
        opp.detector_id == null
          ? "detector_id ausente en el payload — no emitido (nivel-(b) resuelto, R8)"
          : "detector_id del wire",
      basis: "wire",
    },
    {
      label: "hops",
      value: opp.hop_count != null ? String(opp.hop_count) : DASH,
      title:
        opp.hop_count == null
          ? "sin route_metadata persistida — hop_count null (FE-0028), jamás el conteo sintético §29"
          : "hop_count = route_metadata.dex_adapters.length",
      basis: "wire",
    },
    cell(
      "in",
      // CARDS-NOTIONAL-01: a notional is published only when the gross shown
      // beside it can be attributed to it — same arithmetic, one size, checked
      // with the searcher's OWN `SANITY_PROFIT_MULT_OF_CAP` (5×) bound rather
      // than a threshold re-invented here. On the live feed that bound is what
      // separates a legitimate «in $2688.25 / Gross $52.14» pair from the
      // `in $0.00` beside `Gross $1.47M` the operator photographed.
      //
      // ECON-DECLARE-01 (resolución del merge): el principal del ladder CERRADO
      // (`ledger.principal_usd`) manda cuando es verificable — es el tamaño exacto
      // al que se midió el `Gross` de al lado. Cuando no lo es, NO se oculta la
      // cifra que el forward-sim computó: se publica marcada `~`; y si tampoco hay
      // notional en USD, cae al notional DECLARADO por el productor / al monto en
      // wei de la fila (`inValue`), siempre con su unidad a la vista. La
      // declaración viaja además en el `title` y en el `basis` de la celda, así
      // que dos cifras de dos productores se leen como dos, nunca como un ladder
      // roto. R8: sin ningún notional, la celda queda en guion.
      ledger.principal_usd != null
        ? usd(ledger.principal_usd)
        : opp.simulated_amount_in_usd != null
          ? `${inMisattributed ? "~" : ""}${usd(opp.simulated_amount_in_usd)}`
          : inValue,
      inTitle,
      inBasis,
    ),
    cell(
      "Gross",
      ledger.gross_usd != null ? usd(ledger.gross_usd) : null,
      ledger.basis === "canonical"
        ? "economics.gross_profit_usd del searcher al mismo sized notional que `in`"
        : "simulated_gross_usd del mismo forward-sim",
      basisGross,
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
      basisNet,
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
      opp.risk_score != null ? `${(opp.risk_score * 100).toFixed(1)}%` : null,
      "risk_score del wire (fracción 0-1, renderizada como %)",
      figureBasis(opp, "risk"),
    ),
    cell(
      "block",
      opp.economics?.quote_block != null ? String(opp.economics.quote_block) : null,
      "economics.quote_block — bloque real de la quote/reservas usada por el searcher",
    ),
    {
      label: "latencia",
      value: opp.pipeline_latency_ms != null ? `${opp.pipeline_latency_ms}ms` : DASH,
      title:
        opp.pipeline_latency_ms == null
          ? "pipeline_latency_ms ausente en el payload — no emitido (nivel-(b) resuelto, R8)"
          : "pipeline_latency_ms del wire (detección → emisión)",
      basis: "wire",
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
            {/* ECON-DECLARE-01: the value cell carries the value AND its basis.
                `data-testid` is the hook gate (ii) reads: a cell must never
                render — while the wire carries a value for its field. The basis
                label is rendered only beside a real value, so a dash is never
                dressed up with a provenance it does not have. */}
            <div className="truncate">
              <span data-testid={`opp-cell-${c.label}`}>{c.value}</span>
              {c.value !== DASH && (
                <span
                  data-basis={c.basis}
                  className="ml-1 text-[9px] text-muted-foreground/70"
                >
                  {BASIS_LABEL[c.basis]}
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
