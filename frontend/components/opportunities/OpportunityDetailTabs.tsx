"use client";

/**
 * FE-0034 (§37) — the tabbed detail body for one opportunity.
 *
 * Pure component over a single OmniOpportunity (§26/§27: the dialog no longer
 * maintains an inline mirror type — the mapper is the ONLY constructor). The
 * Sheet shell stays in OpportunityDetailDialog; this body is R1-testable via
 * renderToStaticMarkup.
 *
 * Eight §37 tabs, each honest to its wire source (§28/§29 discipline):
 *   Overview    headline rows + the §36 summary grid (FE-0033 spine — one
 *               summary, no parallel rendering of the same fields)
 *   Route       §38 edges from the PERSISTED topology (token path / dex /
 *               pool per leg; §29 fallback rendered marked); per-leg
 *               economics beyond amounts are declared gaps — see Ledger
 *   Ledger      HOPS-LEDGER-04: exact per-leg wei (in/out/direction) from the
 *               sizing kernel + the closed-cycle delta on the closing leg;
 *               absent honestly on non-Sized rows (R8)
 *   Economics   Gross = expected_profit_usd, Net = net_expected_profit_usd
 *               (fixes the legacy mislabel that showed gross as "Net Yield");
 *               §39 waterfall renders EVERY cost line of the simulated block
 *   Simulation  the simulated_* wire block as VALUES (§79 — no PASS verdict
 *               is invented here) + the on-demand SimulateButton
 *   Gates       §40 observed/required/delta/reason from simulated_target +
 *               rejection_reason and §30 semantic violations — all verdicts
 *               backend-owned
 *   Provenance  id / trace / block / detected_at (null-guarded — the legacy
 *               `new Date(null)` rendered 1970-01-01, a fabricated date)
 *   Latency     §45 per-candidate waterfall (FE-0037 ← ARBX-FE-EMIT-09 wire):
 *               the tick's top-K lat_candidates; no per-opp join key yet
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { SimulateButton } from "@/components/SimulateButton";
import { OpportunitySummaryGrid } from "@/components/opportunities/OpportunitySummaryGrid";
import { LatencyCandidatesPanel } from "@/components/opportunities/LatencyCandidatesPanel";
import {
  deriveLegs,
  deriveLegLedger,
  SYNTHETIC_LEGACY_VIEW_LABEL,
  type OmniOpportunity,
} from "@/lib/store/types";
import type {
  LatCandidateRow,
  LatCandidatesTelemetry,
} from "@/lib/apex/schemas";

const DETAIL_TABS = [
  "overview",
  "route",
  "ledger",
  "economics",
  "simulation",
  "gates",
  "provenance",
  "latency",
] as const;
type DetailTab = (typeof DETAIL_TABS)[number];

export type { DetailTab };

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5 py-2 border-b border-border last:border-0">
      <span className="text-xs text-muted-foreground uppercase tracking-wide">{label}</span>
      <span className="text-sm font-mono break-all">
        {value ?? <span className="text-muted-foreground/50 italic">—</span>}
      </span>
    </div>
  );
}

/** Sign-preserving currency (−$x, matching the §36 grid's usd()). */
function usd4(v: number): string {
  const s = v.toFixed(4);
  return `${v < 0 ? "-" : ""}$${v < 0 ? s.slice(1) : s}`;
}

/** 8 + … + 6 elision; full address lives in the cell title (§38). */
function shortAddr(a: string): string {
  return a.length <= 16 ? a : `${a.slice(0, 8)}…${a.slice(-6)}`;
}

/**
 * HOPS-LEDGER-04: exact wei → human token units. BigInt-only — no Number
 * (precision dies above 2^53) and no Intl (R1: locale nondeterminism in a
 * pure render). Integer grouping by hand; frac capped at 8 chars with "…"
 * (the exact wei always lives in the cell title). Null on non-numeric input
 * or unknown decimals — the caller then shows the raw wei (R8).
 */
function weiToHuman(wei: string, decimals: number | undefined): string | null {
  // Cap at uint256's max meaningful decimals (78) — a corrupt/hostile map
  // value in the millions would RangeError String.repeat and crash the Sheet.
  if (decimals == null || !Number.isInteger(decimals) || decimals < 0 || decimals > 78) {
    return null;
  }
  const neg = wei.startsWith("-");
  const digits = neg ? wei.slice(1) : wei;
  if (!/^\d+$/.test(digits)) return null;
  const d = decimals; // decimals map values are uint8 on the Rust side
  let body: string;
  if (digits.length <= d) {
    body = `0.${"0".repeat(d - digits.length)}${digits}`;
  } else {
    const int = digits.slice(0, digits.length - d).replace(/\B(?=(\d{3})+(?=$))/g, ",");
    const frac = digits.slice(digits.length - d);
    body = frac.length > 0 ? `${int}.${frac}` : int;
  }
  // Display cap: 8 frac chars — the exact wei rides the title attr.
  const [ipRaw, fp] = body.split(".");
  const ip = ipRaw ?? "";
  const frac = fp != null && fp.length > 8 ? `${fp.slice(0, 8)}…` : fp;
  const shown = frac != null ? `${ip}.${frac}` : ip;
  return neg ? `-${shown}` : shown;
}

/**
 * FE-0035 (§39): the full simulated cost waterfall — every line of the Rust
 * spine's SimulatedCostBreakdown, nothing hidden. Lines are VALUES from the
 * simulated block; the FE never asserts gross − Σcosts = net (§79 — the wire
 * owns that arithmetic, the tab only renders both ends).
 */
function costLines(c: NonNullable<OmniOpportunity["simulated_cost_breakdown"]>): Array<{
  label: string;
  usd: number;
}> {
  return [
    { label: "Gas", usd: c.gas_usd },
    { label: "LP fees", usd: c.lp_fees_usd },
    { label: "Decoherencia de Estado", usd: c.slippage_usd },
    { label: "Failure buffer", usd: c.failure_buffer_usd },
    { label: "Copied buffer", usd: c.copied_buffer_usd },
    { label: "Capital cost", usd: c.capital_cost_usd },
    { label: "Ops overhead", usd: c.ops_overhead_usd },
    { label: "TLS fee", usd: c.flashloan_fee_usd },
    { label: "Relay fee", usd: c.relay_fee_usd },
  ];
}

function tokenCell(opp: OmniOpportunity, side: "in" | "out"): string {
  const addr = side === "in" ? opp.token_in : opp.token_out;
  const info = side === "in" ? opp.token_in_info : opp.token_out_info;
  const symbol = info?.symbol ?? "?";
  return `${symbol} (${addr.slice(0, 10)}…)`;
}

export function OpportunityDetailTabs({
  opp,
  defaultTab = "overview",
  latencyRows,
  latencyMeta,
}: {
  opp: OmniOpportunity;
  defaultTab?: DetailTab;
  /** FE-0037 (§45): the tick's per-candidate rows + meta (EMIT-09). Absent
   *  (undefined) = backend pre-EMIT-09 — the panel renders the group gap. */
  latencyRows?: LatCandidateRow[];
  latencyMeta?: LatCandidatesTelemetry["lat_candidates_meta"];
}) {
  const chain =
    opp.chain_id_out != null && opp.chain_id_out !== opp.chain_id
      ? `${opp.chain_id} → ${opp.chain_id_out}`
      : String(opp.chain_id);
  const simCost = opp.simulated_cost_breakdown;
  // §38 edges: deriveLegs yields wire legs from route_metadata, or the §29
  // synthetic fallback (marked) when only dex_a/dex_b exist.
  const legs = deriveLegs(opp);
  const hasSynthetic = legs.some((l) => l.synthetic === true);
  const target = opp.simulated_target;
  // HOPS-LEDGER-04: null = no honest ledger (not-Sized / triangular / absent).
  const rm = opp.route_metadata;
  const ledger = deriveLegLedger(opp);

  return (
    <Tabs defaultValue={defaultTab} className="w-full">
      <TabsList className="flex-wrap h-auto max-w-full">
        {DETAIL_TABS.map((t) => (
          <TabsTrigger key={t} value={t} className="capitalize">
            {t}
          </TabsTrigger>
        ))}
      </TabsList>

      {/* ── Overview: headline + the §36 grid spine (FE-0033) ── */}
      <TabsContent value="overview" className="mt-2">
        <Row label="Status" value={opp.status} />
        <Row label="Strategy" value={opp.strategy_kind} />
        <Row label="Chain" value={chain} />
        <Row label="Pair" value={opp.pair_symbol} />
        <Row label="Token In" value={tokenCell(opp, "in")} />
        <Row label="Token Out" value={tokenCell(opp, "out")} />
        <div className="mt-3">
          <OpportunitySummaryGrid opp={opp} />
        </div>
      </TabsContent>

      {/* ── Route (§38): edges from the PERSISTED topology; per-leg economics
          (amounts/rate/fee/liquidity/impact/gas) are not persisted — scanner
          keeps them in memory — so they are declared gaps, never fabricated. */}
      <TabsContent value="route" className="mt-2">
        <Row
          label="Hops"
          value={opp.hop_count != null ? String(opp.hop_count) : null}
        />
        <Row label="Block" value={opp.block_number} />
        {legs.length > 0 ? (
          <table className="mt-2 w-full text-xs font-mono">
            <thead>
              <tr className="border-b border-border text-left text-muted-foreground">
                <th className="py-1.5 pr-2 font-medium uppercase tracking-wide">Leg</th>
                <th className="py-1.5 pr-2 font-medium uppercase tracking-wide">Token path</th>
                <th className="py-1.5 pr-2 font-medium uppercase tracking-wide">DEX</th>
                <th className="py-1.5 font-medium uppercase tracking-wide">Pool</th>
              </tr>
            </thead>
            <tbody>
              {legs.map((l) => (
                <tr key={l.index} className="border-b border-border/50 align-top">
                  <td className="py-1.5 pr-2 whitespace-nowrap">
                    {l.index}
                    {l.synthetic && (
                      <span
                        className="ml-1 text-[9px] uppercase tracking-wide text-muted-foreground/70"
                        title="Leg del fallback legacy — display only (§29)"
                      >
                        syn
                      </span>
                    )}
                  </td>
                  <td className="py-1.5 pr-2" title={`${l.token_in} → ${l.token_out}`}>
                    {shortAddr(l.token_in)} → {shortAddr(l.token_out)}
                  </td>
                  <td className="py-1.5 pr-2">{l.dex || "—"}</td>
                  <td className="py-1.5" title={l.pool || undefined}>
                    {l.pool ? shortAddr(l.pool) : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="mt-2 text-xs italic text-muted-foreground/70">
            Sin route_metadata persistida — hops null (FE-0028), jamás el conteo
            sintético §29.
          </p>
        )}
        {hasSynthetic && (
          <p className="mt-2 text-[11px] uppercase tracking-wide text-muted-foreground/70">
            {SYNTHETIC_LEGACY_VIEW_LABEL} — topología de fallback, no ROUTE
            VERIFIED (§29).
          </p>
        )}
        <p className="mt-2 text-xs italic text-muted-foreground/70">
          Per-leg economics: los MONTOS exactos por hop ya se emiten para filas
          Sized — tab Ledger (HOPS-LEDGER-04). Rate / fee / liquidity / impact /
          gas / state age: no emitidos en el wire persistido (nivel-(b)) — viven
          en memoria del scanner.
        </p>
      </TabsContent>

      {/* ── Ledger (HOPS-LEDGER-04): exact per-leg wei from the sizing kernel.
          All-or-nothing wire: present ONLY on Sized rows whose kernel computed
          leg outputs (2-leg V2/V3); the triangular kernel exposes only the
          final cycle amount → honest absence here (R8). Δ ciclo is the
          closed-cycle delta (final out − initial in, opening-token wei) on the
          closing leg alone — intermediate hops carry no delta because wei of
          different tokens don't subtract. ── */}
      <TabsContent value="ledger" className="mt-2">
        {ledger != null && rm != null ? (
          <div className="overflow-x-auto">
            <table className="mt-2 w-full text-xs font-mono">
              <thead>
                <tr className="border-b border-border text-left text-muted-foreground">
                  <th className="py-1.5 pr-2 font-medium uppercase tracking-wide">Hop</th>
                  <th
                    className="py-1.5 pr-2 font-medium uppercase tracking-wide"
                    title="zero_for_one — convención token0<token1 (hecho de deployment)"
                  >
                    Dir
                  </th>
                  <th className="py-1.5 pr-2 font-medium uppercase tracking-wide">In (wei)</th>
                  <th className="py-1.5 pr-2 font-medium uppercase tracking-wide">Out (wei)</th>
                  <th className="py-1.5 font-medium uppercase tracking-wide">Δ ciclo</th>
                </tr>
              </thead>
              <tbody>
                {ledger.map((e) => {
                  const tokIn = rm.token_addresses[e.index] ?? "";
                  const tokOut = rm.token_addresses[e.index + 1] ?? "";
                  const openDecimals = rm.decimals?.[rm.token_addresses[0] ?? ""];
                  const humanIn = weiToHuman(e.amount_in_wei, rm.decimals?.[tokIn]);
                  const humanOut = weiToHuman(e.amount_out_wei, rm.decimals?.[tokOut]);
                  const delta = e.cycle_delta_wei;
                  return (
                    <tr key={e.index} className="border-b border-border/50 align-top">
                      <td className="py-1.5 pr-2 whitespace-nowrap">
                        {e.index + 1}/{ledger.length}
                      </td>
                      <td
                        className="py-1.5 pr-2 whitespace-nowrap"
                        title={`zero_for_one=${e.zero_for_one}`}
                      >
                        {e.zero_for_one ? "0→1" : "1→0"}
                      </td>
                      <td
                        className="py-1.5 pr-2 break-all"
                        title={`${tokIn} · in_wei=${e.amount_in_wei}`}
                      >
                        {humanIn ?? (
                          <span className="text-muted-foreground">{e.amount_in_wei}</span>
                        )}
                      </td>
                      <td
                        className="py-1.5 pr-2 break-all"
                        title={`${tokOut} · out_wei=${e.amount_out_wei}`}
                      >
                        {humanOut ?? (
                          <span className="text-muted-foreground">{e.amount_out_wei}</span>
                        )}
                      </td>
                      <td
                        className="py-1.5 break-all"
                        title={delta != null ? `cycle_delta_wei=${delta}` : undefined}
                      >
                        {delta != null ? (
                          <span
                            className={
                              delta.startsWith("-") ? "text-destructive" : "text-success"
                            }
                          >
                            {delta.startsWith("-") ? "" : "+"}
                            {weiToHuman(delta, openDecimals) ?? delta}
                          </span>
                        ) : (
                          <span className="text-muted-foreground/50 italic">—</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            <p className="mt-2 text-[11px] italic text-muted-foreground/70">
              Montos EXACTOS en wei del kernel de sizing (filas Sized); el título
              de cada celda lleva el wei completo. Δ ciclo = out final − in
              inicial en wei del token base — sólo el hop de cierre del ciclo.
            </p>
          </div>
        ) : (
          <p className="mt-2 text-xs italic text-muted-foreground/70">
            Sin ledger por-leg persistido — los montos exactos por hop se emiten
            sólo cuando el kernel de sizing los computó (filas Sized, kernels
            2-leg V2/V3); el kernel triangular expone sólo el monto final del
            ciclo (R8: ausencia = no computado, jamás cero).
          </p>
        )}
      </TabsContent>

      {/* ── Economics: Gross vs Net from their canonical wire fields ── */}
      <TabsContent value="economics" className="mt-2">
        <Row
          label="Gross (USD)"
          value={opp.expected_profit_usd != null ? usd4(opp.expected_profit_usd) : null}
        />
        <Row
          label="Net (USD)"
          value={
            opp.net_expected_profit_usd != null ? usd4(opp.net_expected_profit_usd) : null
          }
        />
        <Row
          label="Convergence Ratio"
          value={opp.roi_pct != null ? `${opp.roi_pct.toFixed(4)}%` : null}
        />
        <Row
          label="Risk Score"
          value={opp.risk_score != null ? opp.risk_score.toFixed(4) : null}
        />
        <Row label="Amount In (wei)" value={opp.amount_in_wei} />
        <Row
          label="Amount In (USD, simulado)"
          value={opp.simulated_amount_in_usd != null ? usd4(opp.simulated_amount_in_usd) : null}
        />
        {/* §39 waterfall: every cost line of the simulated block, sin ocultar.
            §79: the FE renders both ends (Gross wire, Net wire) and the lines —
            it never asserts gross − Σcosts = net; that arithmetic is the wire's. */}
        {simCost != null ? (
          <div className="mt-3 border-t border-border pt-2">
            <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Cascada de costos (desglose simulado — §39)
            </p>
            {costLines(simCost).map((line) => (
              <div
                key={line.label}
                className="flex items-baseline justify-between border-b border-border/50 py-1 font-mono text-xs"
              >
                <span className="text-muted-foreground">− {line.label}</span>
                <span>{usd4(line.usd)}</span>
              </div>
            ))}
            <p className="mt-2 text-[11px] italic text-muted-foreground/70">
              Líneas del bloque simulado; el FE no recomputa Gross − Σ = Net (§79)
              — ambos extremos son campos propios del wire.
            </p>
          </div>
        ) : (
          <p className="mt-2 text-xs italic text-muted-foreground/70">
            Sin desglose de costos persistido — la cascada §39 existe sólo cuando
            la simulación escribió el bloque (R8).
          </p>
        )}
      </TabsContent>

      {/* ── Simulation: wire block as VALUES (§79) + on-demand button ── */}
      <TabsContent value="simulation" className="mt-2">
        <Row
          label="Simulated Net (USD)"
          value={
            opp.simulated_net_profit_usd != null
              ? `~${usd4(opp.simulated_net_profit_usd)}`
              : null
          }
        />
        <Row
          label="Simulated ROI"
          value={
            opp.simulated_roi_pct != null ? `${opp.simulated_roi_pct.toFixed(4)}%` : null
          }
        />
        <Row label="Simulated At" value={opp.simulated_at} />
        {simCost != null ? (
          <>
            <Row label="Gas (USD)" value={usd4(simCost.gas_usd)} />
            <Row label="LP Fees (USD)" value={usd4(simCost.lp_fees_usd)} />
            <Row label="Decoherencia (USD)" value={usd4(simCost.slippage_usd)} />
            <Row label="Failure Buffer (USD)" value={usd4(simCost.failure_buffer_usd)} />
            <Row label="TLS Fee (USD)" value={usd4(simCost.flashloan_fee_usd)} />
          </>
        ) : (
          <p className="mt-2 text-xs italic text-muted-foreground/70">
            Sin desglose simulado persistido — «—» = no computado (R8).
          </p>
        )}
        {/* G-SIM-1 PR-B2b Fase 5 — on-demand simulation with route_source
            selector (A1/A2/A3). Result displays inline; honest errors. */}
        <div className="mt-4 border-t border-border pt-4">
          <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            On-demand simulation
          </div>
          <SimulateButton opportunityId={opp.id} />
        </div>
      </TabsContent>

      {/* ── Gates: backend-owned verdicts only ── */}
      <TabsContent value="gates" className="mt-2">
        <Row label="Rejection Reason" value={opp.rejection_reason} />
        <Row
          label="Semantic Violations (§30)"
          value={
            opp.semantic_violations.length > 0
              ? `QUARANTINED — ${opp.semantic_violations.join(" · ")}`
              : "0 (validación limpia)"
          }
        />
        {target != null ? (
          <>
            <Row
              label="Target Verdict"
              value={target.meets_target_at_cap ? "PASS" : "FAIL"}
            />
            {/* §40: observed / required / delta / reason — every value wire-owned
                except delta, a display subtraction declared in the title (unit
                conversion of the same magnitude, never a second verdict). */}
            <Row label="Observed (net @ tamaño sugerido)" value={usd4(target.suggested_net_usd)} />
            <Row
              label="Required (target_net)"
              value={target.target_net_usd != null ? usd4(target.target_net_usd) : null}
            />
            {target.target_net_usd != null && (
              <Row
                label="Delta"
                value={
                  <span className={target.suggested_net_usd - target.target_net_usd >= 0 ? "text-success" : "text-destructive"}>
                    {target.suggested_net_usd >= target.target_net_usd ? "+" : ""}
                    {usd4(target.suggested_net_usd - target.target_net_usd)}
                  </span>
                }
              />
            )}
            <Row label="Reason (binding_floor)" value={target.binding_floor} />
            {target.notes.length > 0 && (
              <Row label="Notes" value={target.notes.join(" · ")} />
            )}
            <Row
              label="Sizing: required / cap / sugerido"
              value={`${usd4(target.required_amount_in_usd)} / ${usd4(target.cap_amount_in_usd)} / ${usd4(target.suggested_amount_in_usd)}`}
            />
          </>
        ) : (
          <p className="mt-2 text-xs italic text-muted-foreground/70">
            Sin veredicto de target persistido (R8) — el FE no recomputa uno (§79).
          </p>
        )}
        <Row
          label="Risk Score (observado)"
          value={opp.risk_score != null ? opp.risk_score.toFixed(4) : null}
        />
        <p className="mt-2 text-xs italic text-muted-foreground/70">
          Umbral requerido de riesgo y gates por-leg: no emitidos en el wire
          (nivel-(b)) — el risk gate vive en el backend pre-execución.
        </p>
      </TabsContent>

      {/* ── Provenance ── */}
      <TabsContent value="provenance" className="mt-2">
        <Row label="ID" value={opp.id} />
        <Row label="Trace ID" value={opp.trace_id} />
        <Row label="Block" value={opp.block_number} />
        {/* FE-0029 guard: `new Date(null)` renders 1970-01-01 — a fabricated
            date. Null stays null → the Row renders the honest dash. */}
        <Row
          label="Detected At"
          value={opp.detected_at != null ? new Date(opp.detected_at).toLocaleString() : null}
        />
        <Row label="Chain" value={chain} />
        <Row label="Bridge" value={opp.bridge} />
        <Row
          label="Bridge Fee (USD)"
          value={opp.bridge_fee_usd != null ? usd4(opp.bridge_fee_usd) : null}
        />
      </TabsContent>

      {/* ── Latency (§45, FE-0037): per-candidate waterfall from the tick's
          lat_candidates (EMIT-09). The panel owns the three honest states
          (group-absent / honest-empty-tick / per-key reprice absence). ── */}
      <TabsContent value="latency" className="mt-2">
        <Row label="Latencia por-candidato" value="top-K del tick (§45)" />
        <LatencyCandidatesPanel rows={latencyRows} meta={latencyMeta} />
      </TabsContent>
    </Tabs>
  );
}
