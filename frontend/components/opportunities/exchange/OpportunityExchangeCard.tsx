/**
 * OpportunityExchangeCard — memory-disciplined trading card for the
 * `/opportunities/exchange` grid.
 *
 * Visual language is IDENTICAL to OpportunityTradeCard (same Tailwind tokens,
 * same primitives: TokenChip / ChainBadge / StrategyBadge / StatusPill, same
 * header / detection-time / token-pair / net-yield / capital-ledger / strategy-
 * config / execute blocks). Two deliberate differences:
 *
 *   1. NO framer-motion. The live grid mounts up to N cards; framer-motion's
 *      per-instance motion state (motion values, listeners) was the residual
 *      source of the 3.4 GB tab footprint. The card renders a plain <div>.
 *
 *   2. `content-visibility: auto` + `contain-intrinsic-size` — the browser
 *      skips layout/paint/decode for off-screen cards (native virtualization),
 *      so only the visible window actually costs render memory. Combined with
 *      `loading="lazy"` on every token logo (owned by TokenIcon) this bounds
 *      the grid's footprint to the viewport, not the list length.
 *
 *   3. ADDITIVE "Route A→B" section — renders the full multi-hop cycle
 *      (2..N legs) from `route_metadata` (migration 099), falling back to a
 *      synthetic 2-leg BUY→SELL cycle from dex_a/dex_b for legacy rows. This
 *      is the operator's "etapa a etapa" route view the card grid lacked.
 *
 * R8 fail-honest throughout: null → "—", never 0 dressed as a computed value.
 */
"use client";

import React, { useState } from "react";
import {
  AlertTriangle,
  ArrowDownRight,
  ArrowUpRight,
  CheckCircle2,
  Clock,
  Info,
  Loader2,
  Play,
  TrendingUp,
  XCircle,
} from "lucide-react";

import { TokenChip, type TokenInfo } from "@/components/TokenChip";
import { ChainBadge } from "@/components/ChainBadge";
import { StrategyBadge } from "@/components/StrategyBadge";
import { StatusPill } from "@/components/StatusPill";
import {
  formatPctOrDash,
  formatProfitUSD,
} from "@/lib/format";
import {
  deriveLegs,
  type OmniOpportunity,
  type RouteLeg,
} from "@/lib/store/types";
import type { StrategyRuntimeConfig } from "@/lib/schemas";
import { shortAddr } from "@/lib/format";

// ─── Tone → token-based class map (mirrors OpportunityTradeCard) ─────────────
const TONE_CLASS: Record<string, string> = {
  positive: "text-success",
  negative: "text-destructive",
  zero: "text-muted-foreground",
  neutral: "text-muted-foreground",
  pending: "text-muted-foreground/60 italic",
};

/** Compact USD for ledger cells (`$12.5k`, `$1.8M`, `$0.0123`). */
function usd(value: number | null | undefined, digits = 2): string {
  if (value == null || !Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  if (abs >= 1_000_000) return `$${(value / 1_000_000).toFixed(2)}M`;
  if (abs >= 1_000) return `$${(value / 1_000).toFixed(1)}k`;
  if (abs >= 1) return `$${value.toFixed(digits)}`;
  return `$${value.toFixed(4)}`;
}

/** Freshness window (matches the existing card's 12s staleness heuristic). */
const STALE_SECS = 12;

interface SimEvidence {
  passed: boolean | null;
  gasEstimateWei: string | null;
  traceId: string | null;
  failReason: string | null;
}

export interface OpportunityExchangeCardProps {
  opp: OmniOpportunity;
  /** Live clock (ms epoch) from the parent ticker — drives age + vigency. */
  now: number;
  /** SSR/CSR gate so time-dependent text only renders client-side (R1). */
  isMounted: boolean;
  /** Whether a shadow-sim is currently running for this card. */
  simLoading: boolean;
  /**
   * Declared per-strategy config from trading_config.strategy_configs.
   * R8: null when the endpoint has no entry for this strategy_kind → "—".
   */
  strategyConfig?: StrategyRuntimeConfig | null;
  /** Effective execution mode label ("paper" | "live") for the execute button. */
  modeLabel: "paper" | "live";
  /** Trigger the shadow simulation (wired to POST .../simulate). */
  onExecute: (opportunityId: string) => Promise<void> | void;
  /** Open the full detail dialog (the old "Inspect details"). */
  onInspect: (opp: OmniOpportunity) => void;
}

// ─── SSOT two-state gate (RU-A) ─────────────────────────────────────────────
// A row that never reached economic evaluation is NOT an opportunity — it is a
// DETECTION diagnostic. Rendering the full trading-card skeleton for it (all
// "—") dressed a hollow shell as an opportunity: the operator saw "fallback
// cards" with no arbitrage and no profit. Fail-honest fix: render the shell as
// what it IS, with the machine reason decoded, and never offer Execute on an
// unevaluated row (garbage-in simulation).
export function isUnevaluatedShell(opp: OmniOpportunity): boolean {
  return (
    opp.status === "detected" &&
    opp.rejection_reason != null &&
    opp.expected_profit_usd == null &&
    opp.net_expected_profit_usd == null &&
    opp.simulated_net_profit_usd == null
  );
}

/** Human decoding of the machine rejection codes the engines emit. */
function decodeRejectionReason(reason: string): string {
  if (reason.startsWith("cartridge_unmapped_strategy_label:")) {
    const label = reason.split(":")[1] ?? "?";
    return `El cartucho detectó la forma pero su categoría "${label}" no está mapeada a ningún motor de evaluación — la economía (profit/ROI/riesgo) nunca se computó. Mapping en cartridge_boot.rs:1145 (RU-4 lo completa).`;
  }
  if (reason.includes("impact_zero")) return "El evento impactó 0 pools indexados.";
  if (reason.includes("no_price") || reason.includes("unknown_token_price"))
    return "Falta precio USD de un token del route — el valor no puede computarse honestamente.";
  if (reason.includes("anomalous_math")) return "La matemática del pool resultó anómala (reserves inválidas).";
  return "Razón sin decodificar — ver código crudo.";
}

function OpportunityExchangeCardImpl({
  opp,
  now,
  isMounted,
  simLoading,
  strategyConfig = null,
  modeLabel,
  onExecute,
  onInspect,
}: OpportunityExchangeCardProps) {
  const [evidence, setEvidence] = useState<SimEvidence | null>(null);

  // ── SSOT gate: hollow detections render as diagnostics, never as trades ──
  if (isUnevaluatedShell(opp)) {
    return <DetectionDiagnosticCard opp={opp} now={now} isMounted={isMounted} onInspect={onInspect} />;
  }

  // ── Detection time / age / vigency ─────────────────────────────────────────
  const detectedTime = new Date(opp.detected_at).getTime();
  const ageSecs = isMounted ? Math.max(0, Math.floor((now - detectedTime) / 1000)) : 0;
  const isStale = ageSecs > STALE_SECS;

  // ── Net priority: canonical spine → TS simulated → "—" ─────────────────────
  const canonicalNet = opp.net_expected_profit_usd ?? null;
  const simulatedNet = opp.simulated_net_profit_usd ?? null;
  const netSource: "canonical" | "simulated" | "none" =
    canonicalNet != null ? "canonical" : simulatedNet != null ? "simulated" : "none";
  const net = formatProfitUSD(canonicalNet ?? simulatedNet);

  const roi = opp.roi_pct;
  const roiTone: "pos" | "neg" | "muted" =
    roi == null ? "muted" : roi > 0 ? "pos" : roi < 0 ? "neg" : "muted";

  // ── Route legs (A→B cycle) ─────────────────────────────────────────────────
  const legs = deriveLegs(opp);
  const legCount = legs.length;
  const startToken = legs[0]?.token_in ?? opp.token_in;
  const endToken = legs[legs.length - 1]?.token_out ?? opp.token_in;

  // ── Step-ladder ledger (capital path USD) ──────────────────────────────────
  const simInUsd = opp.simulated_amount_in_usd ?? null;
  const capitalInUsd = simInUsd;
  const cb = opp.simulated_cost_breakdown;
  const grossUsd = opp.expected_profit_usd ?? null;
  const netUsd = canonicalNet ?? simulatedNet;

  const endValueUsd: number | null =
    capitalInUsd != null && netUsd != null ? capitalInUsd + netUsd : null;

  const flashFee = cb?.flashloan_fee_usd ?? null;
  const repayUsd: number | null =
    capitalInUsd != null && flashFee != null ? capitalInUsd + flashFee : null;

  const tgt = opp.simulated_target;
  const targetVerdict: { label: string; tone: "pass" | "fail" | "na" } = (() => {
    if (tgt == null) return { label: "no target", tone: "na" };
    const infeasible =
      tgt.binding_floor === "roi-unreachable" ||
      tgt.binding_floor === "net-per-usd-nonpositive";
    return tgt.meets_target_at_cap && !infeasible
      ? { label: "PASS", tone: "pass" }
      : { label: "FAIL", tone: "fail" };
  })();

  const costRows: Array<[string, number | null]> = [
    ["Gas", cb?.gas_usd ?? null],
    ["LP fees", cb?.lp_fees_usd ?? null],
    ["Decoherence (slippage)", cb?.slippage_usd ?? null],
    ["TLS fee (flash)", cb?.flashloan_fee_usd ?? null],
    ["Relay fee", cb?.relay_fee_usd ?? null],
    ["Capital cost", cb?.capital_cost_usd ?? null],
    ["Failure buffer", cb?.failure_buffer_usd ?? null],
    ["Ops overhead", cb?.ops_overhead_usd ?? null],
  ];
  const knownCostSum = costRows.reduce<number | null>(
    (acc, [, v]) => (v == null ? acc : (acc ?? 0) + v),
    null,
  );

  const handleExecute = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setEvidence(null);
    try {
      await onExecute(opp.id);
      setEvidence({ passed: null, gasEstimateWei: null, traceId: opp.trace_id, failReason: null });
    } catch (err) {
      setEvidence({
        passed: false,
        gasEstimateWei: null,
        traceId: opp.trace_id,
        failReason: (err as Error).message,
      });
    }
  };

  // ── content-visibility: the browser skips render for off-screen cards ──────
  // (native virtualization). `auto 640px` = remember last measured size,
  // initial estimate 640px so the scrollbar is correct before first paint.
  const cardStyle = {
    contentVisibility: "auto",
    containIntrinsicSize: "auto 640px",
  } as React.CSSProperties;

  return (
    <div
      style={cardStyle}
      className="relative bg-card text-card-foreground border border-border rounded-2xl p-4 shadow-lg hover:shadow-xl hover:border-primary/40 transition-colors overflow-hidden"
    >
      {/* ⓘ discreet Inspect affordance — top-right corner */}
      <button
        type="button"
        aria-label="Inspect details"
        title="Inspect details"
        onClick={(e) => {
          e.stopPropagation();
          onInspect(opp);
        }}
        className="absolute right-3 top-3 z-10 rounded-full p-1 text-muted-foreground/70 hover:text-foreground hover:bg-muted/60 border border-transparent hover:border-border transition-colors"
      >
        <Info size={15} />
      </button>

      {/* ── HEADER: chain · strategy · status · base token  |  ROI% ── */}
      <div className="flex items-start justify-between gap-2 mb-2.5 pr-7">
        <div className="flex items-center gap-1.5 flex-wrap min-w-0">
          <ChainBadge chain_id={opp.chain_id} />
          <StrategyBadge strategy_kind={opp.strategy_kind} />
          <StatusPill status={opp.status} rejection_reason={opp.rejection_reason} />
          {opp.chain_base_token_symbol && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-muted/50 text-muted-foreground/90 border border-border/60 font-mono uppercase tracking-wide">
              {opp.chain_base_token_symbol}
            </span>
          )}
        </div>
        <div
          className={`flex items-center gap-1 font-bold text-base whitespace-nowrap ${
            roiTone === "pos" ? "text-success" : roiTone === "neg" ? "text-destructive" : "text-muted-foreground"
          }`}
          title="Net Convergence Ratio (ROI %) — fail-honest '—' when not computed"
        >
          {roiTone === "pos" && <TrendingUp size={14} />}
          {formatPctOrDash(opp.roi_pct)}
        </div>
      </div>

      {/* ── DETECTION TIME · AGE · VIGENCY ── */}
      <div className="flex items-center justify-between gap-2 mb-3 text-[10px] font-mono">
        <div className="flex items-center gap-1.5 text-muted-foreground" suppressHydrationWarning>
          <Clock size={11} />
          <span suppressHydrationWarning>
            {isMounted
              ? new Date(opp.detected_at).toLocaleTimeString([], {
                  hour12: false,
                  hour: "2-digit",
                  minute: "2-digit",
                  second: "2-digit",
                })
              : "--:--:--"}
          </span>
          <span className="text-muted-foreground/60">·</span>
          <span suppressHydrationWarning>{isMounted ? `${ageSecs}s` : "--"}</span>
        </div>
        <span
          className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded border font-bold uppercase tracking-wide ${
            isStale
              ? "bg-destructive/10 text-destructive border-destructive/30"
              : "bg-success/10 text-success border-success/30"
          }`}
        >
          {isStale ? <AlertTriangle size={10} className="animate-pulse" /> : <CheckCircle2 size={10} />}
          {isStale ? "stale" : "vigente"}
        </span>
      </div>

      {/* ── ROUTE A→B (multi-leg cycle, etapa a etapa) ──
           The operator's "start token → each leg → close" view. Each leg shows
           the DEX adapter + the token pair it swaps; the start token closes
           back to itself for an atomic cycle. Honest "—" when no topology. */}
      <div className="rounded-lg border border-border bg-muted/20 mb-3 overflow-hidden">
        <div className="px-2.5 py-1.5 border-b border-border/60 flex items-center justify-between text-[9px] uppercase tracking-wide text-muted-foreground font-semibold">
          <span>Route A→B · {legCount > 0 ? `${legCount} leg${legCount > 1 ? "s" : ""}` : "no topology"}</span>
          <span className="font-mono normal-case tracking-normal opacity-70">
            {startToken === endToken ? "closed cycle" : "open path"}
          </span>
        </div>
        {legCount > 0 ? (
          <div className="p-2 space-y-1">
            {legs.map((leg) => (
              <RouteLegRow key={leg.index} leg={leg} opp={opp} />
            ))}
          </div>
        ) : (
          <div className="p-2 text-[11px] font-mono text-muted-foreground/60 italic">
            No route topology persisted for this detection.
          </div>
        )}
      </div>

      {/* ── EXECUTIVE RESULT: net yield + target verdict ── */}
      <div className="grid grid-cols-2 gap-2 mb-3">
        <div className="rounded-lg bg-muted/40 p-2">
          <div className="text-[9px] uppercase tracking-wide text-muted-foreground">Net yield</div>
          <div className="flex items-center gap-1">
            <span
              className={`font-mono text-lg font-bold ${TONE_CLASS[net.tone] ?? "text-muted-foreground"}`}
              title={
                netSource === "canonical"
                  ? "Canonical spine net = gross − all costs"
                  : netSource === "simulated"
                    ? "TS forward-sim net (canonical pending)"
                    : "Not yet computed (R8: '—')"
              }
            >
              {netSource === "simulated" ? `~${net.display}` : net.display}
            </span>
            {netSource === "simulated" && (
              <span className="text-[9px] font-bold px-1 rounded bg-info/15 text-info border border-info/40">SIM</span>
            )}
          </div>
        </div>
        <div className="rounded-lg bg-muted/40 p-2">
          <div className="text-[9px] uppercase tracking-wide text-muted-foreground">
            Target · {tgt?.target_source === "strategy_config" ? "/strategies" : tgt?.target_source === "simulation_tab" ? "Sim tab" : "none"}
          </div>
          <div
            className={`font-mono text-lg font-bold ${
              targetVerdict.tone === "pass"
                ? "text-success"
                : targetVerdict.tone === "fail"
                  ? "text-destructive"
                  : "text-muted-foreground"
            }`}
          >
            {targetVerdict.label}
          </div>
        </div>
      </div>

      {/* ── STEP LADDER: capital path with running USD totals ── */}
      <div className="rounded-lg border border-border bg-muted/20 mb-3 overflow-hidden">
        <div className="px-2.5 py-1.5 border-b border-border/60 text-[9px] uppercase tracking-wide text-muted-foreground font-semibold">
          Capital path (USD)
        </div>
        <div className="p-2 space-y-0.5 font-mono text-[11px]">
          <LedgerRow
            up
            label={`Flash loan in (TLS)${opp.chain_base_token_symbol ? ` · ${opp.chain_base_token_symbol}` : ""}`}
            value={capitalInUsd}
          />
          <LedgerRow
            label={`Buy A · ${opp.dex_a || "—"}`}
            value={capitalInUsd}
            muted
            hint="swap leg 1"
          />
          <LedgerRow
            label={`Buy B · ${opp.dex_b ?? "single-DEX"}`}
            value={null}
            muted
            hint={opp.dex_b ? "swap leg 2" : "1-leg route"}
          />
          <LedgerRow
            up
            label="Gross out (AMM spread)"
            value={grossUsd}
            tone="text-foreground"
          />
          <div className="my-1 border-t border-border/50" />
          <LedgerRow down label={`Repay (principal + TLS fee)`} value={repayUsd} />
          {costRows.map(([label, v]) => (
            <LedgerRow key={label} down label={label} value={v} small />
          ))}
          <div className="my-1 border-t border-border/50" />
          <LedgerRow
            label="Total cost"
            value={knownCostSum}
            tone="text-destructive"
            strong
          />
          <LedgerRow
            up={netUsd != null && netUsd > 0}
            down={netUsd != null && netUsd < 0}
            label={`Net yield${netSource === "simulated" ? " (SIM)" : ""}`}
            value={netUsd}
            tone={netUsd == null ? undefined : netUsd > 0 ? "text-success" : netUsd < 0 ? "text-destructive" : "text-muted-foreground"}
            strong
          />
        </div>
      </div>

      {/* ── APPLIED STRATEGY CONFIG (from /strategies) ── */}
      <div className="rounded-lg border border-border bg-muted/20 mb-3 p-2">
        <div className="text-[9px] uppercase tracking-wide text-muted-foreground font-semibold mb-1.5">
          Applied strategy config
        </div>
        {tgt ? (
          <div className="grid grid-cols-2 gap-x-3 gap-y-1 font-mono text-[11px]">
            <ConfigRow label="min net USD" value={tgt.target_net_usd != null ? usd(tgt.target_net_usd) : "—"} />
            <ConfigRow label="min ROI %" value={tgt.target_roi_pct != null ? `${tgt.target_roi_pct.toFixed(2)}%` : "—"} />
            <ConfigRow
              label="binding floor"
              value={tgt.binding_floor}
              tone={
                tgt.binding_floor === "roi-unreachable" || tgt.binding_floor === "net-per-usd-nonpositive"
                  ? "text-destructive"
                  : "text-foreground"
              }
            />
            <ConfigRow
              label="suggested borrow"
              value={Number.isFinite(tgt.suggested_amount_in_usd) ? usd(tgt.suggested_amount_in_usd) : "—"}
            />
          </div>
        ) : (
          <div className="text-[11px] font-mono text-muted-foreground/60 italic">
            No target applied (inverse-sizing not run for this route).
          </div>
        )}
      </div>

      {/* ── EXECUTE (shadow) + evidence ──
           The shadow simulate is mode-invariant and read-only (capital $0):
           safe under paper, testnet, and mainnet (§34). The label surfaces the
           effective terminus mode so the operator knows which settlement the
           real (non-shadow) path would target. */}
      <div className="space-y-2">
        <button
          type="button"
          onClick={handleExecute}
          disabled={simLoading}
          className="w-full py-2.5 rounded-lg bg-primary text-primary-foreground text-xs font-bold uppercase tracking-wide hover:bg-primary/90 active:scale-[0.99] disabled:opacity-60 disabled:cursor-not-allowed transition-all inline-flex items-center justify-center gap-2"
          title={`Shadow-simulate on the Anvil fork (read-only, capital $0). Effective terminus: ${modeLabel}.`}
        >
          {simLoading ? (
            <>
              <Loader2 size={14} className="animate-spin" /> Simulating…
            </>
          ) : (
            <>
              <Play size={14} /> Execute ({modeLabel})
            </>
          )}
        </button>

        {evidence && (
          <div className="rounded-md border border-border bg-muted/30 px-2 py-1.5 font-mono text-[10px] text-muted-foreground flex items-center gap-2">
            {evidence.passed === true && <CheckCircle2 size={11} className="text-success" />}
            {evidence.passed === false && <XCircle size={11} className="text-destructive" />}
            <span className="truncate">
              shadow-sim dispatched · trace{" "}
              <span className="text-foreground">{evidence.traceId ?? "—"}</span>
              {evidence.gasEstimateWei ? ` · gas ${evidence.gasEstimateWei}` : ""}
              {evidence.failReason ? ` · ${evidence.failReason}` : ""}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

// PERF: memoize so parent re-renders (age ticker, store batch replacement) do
// NOT re-render every card. Business-equality fields are checked because the
// store emits a fresh array after each batch replacement (reference equality on
// `opp` would always fail). Also keys on route_metadata identity so a newly
// enriched multi-leg topology re-renders the card.
export const OpportunityExchangeCard = React.memo(
  OpportunityExchangeCardImpl,
  (prev: OpportunityExchangeCardProps, next: OpportunityExchangeCardProps): boolean => {
    const p = prev.opp;
    const n = next.opp;
    const agePrev = Math.floor((prev.now - new Date(p.detected_at).getTime()) / 1000);
    const ageNext = Math.floor((next.now - new Date(n.detected_at).getTime()) / 1000);
    return (
      p.id === n.id &&
      p.status === n.status &&
      p.expected_profit_usd === n.expected_profit_usd &&
      p.net_expected_profit_usd === n.net_expected_profit_usd &&
      p.roi_pct === n.roi_pct &&
      p.detected_at === n.detected_at &&
      p.token_in_info?.logo_url === n.token_in_info?.logo_url &&
      p.token_out_info?.logo_url === n.token_out_info?.logo_url &&
      prev.isMounted === next.isMounted &&
      prev.simLoading === next.simLoading &&
      prev.modeLabel === next.modeLabel &&
      prev.strategyConfig === next.strategyConfig &&
      prev.onExecute === next.onExecute &&
      prev.onInspect === next.onInspect &&
      agePrev === ageNext
    );
  },
);

// ─── DetectionDiagnosticCard (RU-A SSOT) ────────────────────────────────────
// The honest face of a hollow detection: identity + WHY it was never evaluated.
// No ledger, no route placeholder, no Execute — an unevaluated row has no
// arbitrage to show, and pretending otherwise was the "fallback card" lie.
function DetectionDiagnosticCard({
  opp,
  now,
  isMounted,
  onInspect,
}: Pick<OpportunityExchangeCardProps, "opp" | "now" | "isMounted" | "onInspect">) {
  const ageSecs = isMounted ? Math.max(0, Math.floor((now - new Date(opp.detected_at).getTime()) / 1000)) : 0;
  const reason = opp.rejection_reason ?? "unknown";
  const degeneratePair = opp.token_in === opp.token_out;
  return (
    <div
      style={{ contentVisibility: "auto", containIntrinsicSize: "auto 210px" } as React.CSSProperties}
      className="relative bg-muted/30 text-muted-foreground border border-dashed border-border rounded-2xl p-4 overflow-hidden"
    >
      <button
        type="button"
        aria-label="Inspect details"
        onClick={(e) => {
          e.stopPropagation();
          onInspect(opp);
        }}
        className="absolute right-3 top-3 z-10 rounded-full p-1 text-muted-foreground/70 hover:text-foreground hover:bg-muted/60 border border-transparent hover:border-border transition-colors"
      >
        <Info size={15} />
      </button>

      {/* header: chain · strategy · DETECCIÓN badge */}
      <div className="flex items-center gap-1.5 flex-wrap mb-2 pr-7">
        <ChainBadge chain_id={opp.chain_id} />
        <StrategyBadge strategy_kind={opp.strategy_kind} />
        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[9px] font-bold uppercase tracking-wide bg-warning/10 text-warning border border-warning/40">
          <AlertTriangle size={10} /> Detección — sin evaluar
        </span>
      </div>

      {/* what IS real: cartridge identity, timestamp, age */}
      <div className="flex items-center justify-between text-[10px] font-mono mb-2" suppressHydrationWarning>
        <span className="flex items-center gap-1.5 text-muted-foreground/80">
          <Clock size={11} />
          {isMounted
            ? new Date(opp.detected_at).toLocaleTimeString([], { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })
            : "--:--:--"}
          <span className="text-muted-foreground/50">· {isMounted ? `${ageSecs}s` : "--"}</span>
        </span>
        <span className="font-mono text-[9px] text-muted-foreground/50">{opp.strategy_kind}</span>
      </div>

      {/* the WHY — decoded machine reason + raw code */}
      <div className="rounded-lg border border-warning/30 bg-warning/5 p-2.5 space-y-1.5">
        <div className="text-[10px] uppercase tracking-wide text-warning font-semibold">
          Por qué NO pasó a evaluación
        </div>
        <p className="text-[11px] leading-snug text-foreground/90">{decodeRejectionReason(reason)}</p>
        <code className="block text-[9px] font-mono text-muted-foreground/70 break-all">{reason}</code>
      </div>

      {/* degenerate-data flag when the detection itself carries no usable shape */}
      {degeneratePair && (
        <div className="mt-2 text-[10px] font-mono text-muted-foreground/60">
          dato crudo: token_in == token_out (forma degenerada, sin route)
        </div>
      )}

      {/* explicit truth: there is nothing to trade here — yet */}
      <div className="mt-2 text-[10px] text-muted-foreground/70 italic">
        Sin profit/ROI/riesgo: la evaluación económica no corrió. Cuando el motor correspondiente la evalúe, esta misma fila aparecerá como oportunidad con números reales.
      </div>
    </div>
  );
}

// ─── Route leg row (A→B etapa a etapa) ───────────────────────────────────────
function RouteLegRow({ leg, opp }: { leg: RouteLeg; opp: OmniOpportunity }) {
  // First/last legs reuse the enriched token metadata the row already carries;
  // intermediate legs resolve lazily via TokenChip's icon cascade.
  const isInStart = leg.index === 0;
  const infoFor = (addr: string): TokenInfo | null => {
    if (isInStart && addr === opp.token_in) return opp.token_in_info;
    if (addr === opp.token_out) return opp.token_out_info;
    return null;
  };
  const dexLabel = leg.dex || "—";
  return (
    <div className="flex items-center gap-2 min-w-0 text-[11px]">
      <span className="shrink-0 inline-flex items-center justify-center size-4 rounded-full bg-primary/10 text-primary border border-primary/30 font-bold text-[9px]">
        {leg.index + 1}
      </span>
      <span className="min-w-0 flex-1">
        <TokenChip token_address={leg.token_in} chain_id={opp.chain_id} info={infoFor(leg.token_in)} />
      </span>
      <span className="shrink-0 inline-flex items-center gap-1 px-1.5 py-0.5 rounded border bg-muted/40 border-border text-[9px] font-bold uppercase tracking-wide text-muted-foreground" title={`DEX adapter: ${dexLabel}`}>
        {dexLabel}
      </span>
      <span className="text-muted-foreground/60 shrink-0" aria-hidden="true">→</span>
      <span className="min-w-0 flex-1">
        <TokenChip token_address={leg.token_out} chain_id={opp.chain_id_out ?? opp.chain_id} info={infoFor(leg.token_out)} />
      </span>
      {leg.pool && (
        <span className="shrink-0 font-mono text-[9px] text-muted-foreground/50" title={`Pool: ${leg.pool}`}>
          {shortAddr(leg.pool)}
        </span>
      )}
    </div>
  );
}

// ─── Ledger row (capital path) ───────────────────────────────────────────────
function LedgerRow({
  label,
  value,
  up = false,
  down = false,
  muted = false,
  strong = false,
  small = false,
  tone,
  hint,
}: {
  label: string;
  value: number | null;
  up?: boolean;
  down?: boolean;
  muted?: boolean;
  strong?: boolean;
  small?: boolean;
  tone?: string;
  hint?: string;
}) {
  return (
    <div
      className={`flex items-center justify-between gap-2 ${
        muted ? "text-muted-foreground/70" : "text-foreground"
      } ${small ? "text-[10px]" : ""} ${strong ? "font-bold" : ""}`}
    >
      <span className="flex items-center gap-1 min-w-0">
        {up && <ArrowUpRight size={11} className="text-success shrink-0" />}
        {down && <ArrowDownRight size={11} className="text-destructive shrink-0" />}
        <span className="truncate">{label}</span>
        {hint && <span className="text-[9px] text-muted-foreground/50 italic">({hint})</span>}
      </span>
      <span className={tone ?? (muted ? "text-muted-foreground/60" : "text-foreground")}>
        {usd(value)}
      </span>
    </div>
  );
}

// ─── Config row (applied strategy config) ────────────────────────────────────
function ConfigRow({
  label,
  value,
  tone = "text-foreground",
}: {
  label: string;
  value: string;
  tone?: string;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-muted-foreground">{label}</span>
      <span className={tone}>{value}</span>
    </div>
  );
}
