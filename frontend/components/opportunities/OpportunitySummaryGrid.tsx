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
export const NOT_COMPUTED = "no computado";

function usd(v: number, digits = 2): string {
  const s = v.toFixed(digits);
  return `${v < 0 ? "-" : ""}$${v < 0 ? s.slice(1) : s}`;
}

function summaryCells(opp: OmniOpportunity): Array<{
  label: string;
  value: string;
  title: string;
}> {
  const bps = opp.roi_pct != null ? (opp.roi_pct * 100).toFixed(0) : null;
  // WO-CARDS-COMPLETE-01: a null economic value on a row that carries a
  // rejection_reason is "no computado" with the R8 reason in the title —
  // the bare dash stays only when there is no reason to state.
  const cell = (
    label: string,
    v: string | null,
    title: string,
  ): { label: string; value: string; title: string } =>
    v != null
      ? { label, value: v, title }
      : opp.rejection_reason != null
        ? { label, value: NOT_COMPUTED, title: `no computado: ${opp.rejection_reason} (R8)` }
        : { label, value: DASH, title };
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
      label: "detector",
      value: opp.detector_id ?? NOT_EMITTED,
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
      opp.net_expected_profit_usd != null ? usd(opp.net_expected_profit_usd) : null,
      "net_expected_profit_usd (spine canónico)",
    ),
    cell(
      "bps",
      bps,
      opp.roi_pct == null
        ? "roi_pct no computado (R8)"
        : `roi_pct ${opp.roi_pct.toFixed(4)}% × 100 — conversión de unidad, no un veredicto`,
    ),
    cell(
      "Risk",
      opp.risk_score != null ? opp.risk_score.toFixed(2) : null,
      "risk_score del wire",
    ),
    cell(
      "Sim",
      opp.simulated_net_profit_usd != null
        ? `~${usd(opp.simulated_net_profit_usd)}`
        : null,
      "forward-sim net como VALOR — el wire no persiste veredicto PASS/FAIL de simulación (§79); PASS/FAIL solo vive en el target verdict",
    ),
    {
      label: "latencia",
      value: opp.pipeline_latency_ms != null ? `${opp.pipeline_latency_ms}ms` : NOT_EMITTED,
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
