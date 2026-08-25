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
 *   detector  NOT EMITTED on the wire — "no emitido" (nivel-(b))
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
 *   latencia  NOT EMITTED per-opportunity yet — "no emitido" (nivel-(b);
 *             backend per-candidate latency emission is ARBX-FE-EMIT-09).
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import type { OmniOpportunity } from "@/lib/store/types";

const DASH = "—";
export const NOT_EMITTED = "no emitido";

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
      value: NOT_EMITTED,
      title: "detector_id no es columna del feed de oportunidades (nivel-(b))",
    },
    {
      label: "hops",
      value: opp.hop_count != null ? String(opp.hop_count) : DASH,
      title:
        opp.hop_count == null
          ? "sin route_metadata persistida — hop_count null (FE-0028), jamás el conteo sintético §29"
          : "hop_count = route_metadata.dex_adapters.length",
    },
    {
      label: "in",
      value:
        opp.simulated_amount_in_usd != null
          ? usd(opp.simulated_amount_in_usd)
          : DASH,
      title: `amount_in_wei=${opp.amount_in_wei ?? "no emitido"} · USD solo cuando la simulación lo computa (R8)`,
    },
    {
      label: "Gross",
      value: opp.expected_profit_usd != null ? usd(opp.expected_profit_usd) : DASH,
      title: "expected_profit_usd (gross, pre-costos)",
    },
    {
      label: "Net",
      value: opp.net_expected_profit_usd != null ? usd(opp.net_expected_profit_usd) : DASH,
      title: "net_expected_profit_usd (spine canónico)",
    },
    {
      label: "bps",
      value: bps ?? DASH,
      title:
        opp.roi_pct == null
          ? "roi_pct no computado (R8)"
          : `roi_pct ${opp.roi_pct.toFixed(4)}% × 100 — conversión de unidad, no un veredicto`,
    },
    {
      label: "Risk",
      value: opp.risk_score != null ? opp.risk_score.toFixed(2) : DASH,
      title: "risk_score del wire",
    },
    {
      label: "Sim",
      value:
        opp.simulated_net_profit_usd != null
          ? `~${usd(opp.simulated_net_profit_usd)}`
          : DASH,
      title:
        "forward-sim net como VALOR — el wire no persiste veredicto PASS/FAIL de simulación (§79); PASS/FAIL solo vive en el target verdict",
    },
    {
      label: "latencia",
      value: NOT_EMITTED,
      title:
        "latencia por-candidato no emitida aún (nivel-(b)) — emisión pendiente ARBX-FE-EMIT-09",
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
