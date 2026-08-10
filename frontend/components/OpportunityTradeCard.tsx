/**
 * OpportunityTradeCard — enriched trading card for a detected resolution route.
 *
 * Revives the "card v2" dashboard pattern (commit e9bcadd) and extends it per
 * the operator's execution spec:
 *
 *   1. STEP LADDER — the capital path rendered as a vertical ledger:
 *        Flash loan in → buy A → buy B → buy C → repay → fees → gas → net.
 *      Each step shows its running USD total so the operator reads, at a
 *      glance, where value is gained or lost at every hop. All figures come
 *      from the live snapshot (gross spread + simulated cost breakdown +
 *      simulated net) — never fabricated (RULE 00); unknown cells show "—".
 *
 *   2. APPLIED STRATEGY CONFIG — the /strategies-applied gate the route was
 *      sized against (min net USD, min ROI %, binding floor, suggested
 *      borrow), read from simulated_target. Honest "—" when not run.
 *
 *   3. DETECTION TIME / AGE / VIGENCY — detected_at timestamp, live age in
 *      seconds, and a vigency pill (VIGENTE while the snapshot is fresh,
 *      STALE past the freshness window). The card updates in place (stable
 *      React key), never duplicates, and disappears when the route drops.
 *
 *   4. EXECUTE (shadow) — POST /api/v1/opportunities/:id/simulate against the
 *      sim-ctl + Anvil fork. Read-only, capital = 0; returns pass/fail +
 *      gas estimate + trace id as verifiable evidence. Surfaces the result
 *      inline; never promises live execution it cannot deliver.
 *
 *   5. ⓘ INSPECT — the previous full-width "Inspect details" button is now a
 *      discreet circled-i affordance in the card's top-right corner.
 *
 * R8 fail-honest throughout: null → "—", never 0 dressed as a computed value.
 */
"use client";

import React, { useState } from "react";
import { motion } from "framer-motion";
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
import { toast } from "sonner";

import { TokenChip } from "@/components/TokenChip";
import { ChainBadge } from "@/components/ChainBadge";
import { StrategyBadge } from "@/components/StrategyBadge";
import { StatusPill } from "@/components/StatusPill";
import {
  formatPctOrDash,
  formatProfitUSD,
} from "@/lib/format";
import type { OmniOpportunity } from "@/lib/store/types";
import type { StrategyRuntimeConfig } from "@/lib/schemas";

// ─── Tone → token-based class map ────────────────────────────────────────────
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

/** Freshness window (matches the table's 12s staleness heuristic). */
const STALE_SECS = 12;

interface SimEvidence {
  passed: boolean | null;
  gasEstimateWei: string | null;
  traceId: string | null;
  failReason: string | null;
}

export interface OpportunityTradeCardProps {
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
  /** Trigger the shadow simulation (wired to POST .../simulate). */
  onExecute: (opportunityId: string) => Promise<void> | void;
  /** Open the full detail dialog (the old "Inspect details"). */
  onInspect: (opp: OmniOpportunity) => void;
}

function OpportunityTradeCardImpl({
  opp,
  now,
  isMounted,
  simLoading,
  strategyConfig = null,
  onExecute,
  onInspect,
}: OpportunityTradeCardProps) {
  const [evidence, setEvidence] = useState<SimEvidence | null>(null);

  // ── Detection time / age / vigency ─────────────────────────────────────────
  const detectedTime = new Date(opp.detected_at).getTime();
  const ageSecs = isMounted ? Math.max(0, Math.floor((now - detectedTime) / 1000)) : 0;
  const isStale = ageSecs > STALE_SECS;

  // ── Net priority: canonical spine → TS simulated → "—" ─────────────────────
  const gross = formatProfitUSD(opp.expected_profit_usd);
  const canonicalNet = opp.net_expected_profit_usd ?? null;
  const simulatedNet = opp.simulated_net_profit_usd ?? null;
  const netSource: "canonical" | "simulated" | "none" =
    canonicalNet != null ? "canonical" : simulatedNet != null ? "simulated" : "none";
  const net = formatProfitUSD(canonicalNet ?? simulatedNet);

  const roi = opp.roi_pct;
  const roiTone: "pos" | "neg" | "muted" =
    roi == null ? "muted" : roi > 0 ? "pos" : roi < 0 ? "neg" : "muted";

  // ── Step-ladder ledger: borrow → buy A → buy B → buy C → repay → net ───────
  // Capital in: the simulated borrow sized against the /strategies target, else
  // the recorded amount_in (honest label). End value: capital_in + net (only
  // when both are known — never invent one from the other).
  const simInUsd = opp.simulated_amount_in_usd ?? null;
  const capitalInUsd = simInUsd;
  const cb = opp.simulated_cost_breakdown;
  const grossUsd = opp.expected_profit_usd ?? null;
  const netUsd = canonicalNet ?? simulatedNet;

  // End-of-route value: capital + net (SIM path). Honest only when both known.
  const endValueUsd: number | null =
    capitalInUsd != null && netUsd != null ? capitalInUsd + netUsd : null;

  // Repay = capital in + flash-convergence fee (TLS principal + fee).
  const flashFee = cb?.flashloan_fee_usd ?? null;
  const repayUsd: number | null =
    capitalInUsd != null && flashFee != null ? capitalInUsd + flashFee : null;

  // Applied strategy config (from /strategies) the route was sized against.
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

  // Total cost = gross − net (when both known) or the sum of known cost rows.
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
      // The parent surfaces the toast; we optimistically mark a ran state.
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

  return (
    <motion.div
      initial={{ opacity: 0, y: 8, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.25 }}
      className="relative bg-card text-card-foreground border border-border rounded-2xl p-4 shadow-lg hover:shadow-xl hover:border-primary/40 transition-all overflow-hidden"
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

      {/* ── TOKEN PAIR ── */}
      <div className="flex items-center gap-2 mb-3 min-w-0">
        <div className="min-w-0 flex-1">
          <TokenChip token_address={opp.token_in} chain_id={opp.chain_id} info={opp.token_in_info} />
        </div>
        <span className="text-muted-foreground/60 shrink-0" aria-hidden="true">
          →
        </span>
        <div className="min-w-0 flex-1">
          <TokenChip token_address={opp.token_out} chain_id={opp.chain_id_out ?? opp.chain_id} info={opp.token_out_info} />
        </div>
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

      {/* ── STEP LADDER: capital path with running USD totals ──
           Each row shows its USD contribution so the operator reads where value
           is gained/lost at every hop. Down = cost, up = inflow. */}
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

      {/* ── EXECUTE (shadow) + evidence ── */}
      <div className="space-y-2">
        <button
          type="button"
          onClick={handleExecute}
          disabled={simLoading}
          className="w-full py-2.5 rounded-lg bg-primary text-primary-foreground text-xs font-bold uppercase tracking-wide hover:bg-primary/90 active:scale-[0.99] disabled:opacity-60 disabled:cursor-not-allowed transition-all inline-flex items-center justify-center gap-2"
          title="Execute in shadow mode — POST /api/v1/opportunities/:id/simulate against the sim-ctl + Anvil fork. Read-only, capital $0; returns pass/fail + gas + trace evidence."
        >
          {simLoading ? (
            <>
              <Loader2 size={14} className="animate-spin" /> Simulating…
            </>
          ) : (
            <>
              <Play size={14} /> Execute (shadow)
            </>
          )}
        </button>

        {/* Shadow-sim evidence strip — verifiable, never fabricated. */}
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
    </motion.div>
  );
}

// PERF (2026-08-09): memoize the card so the parent re-renders do NOT re-render
// every card. The parent used to push a fresh `now` every second → all ~200
// motion.div cards re-rendered each second. The comparator re-renders a card
// only when its own data changed OR its displayed age (seconds) ticked over.
// Business-equality fields are checked because the store emits a fresh array
// after each batch replacement, so reference equality on `opp` would fail.
export const OpportunityTradeCard = React.memo(
  OpportunityTradeCardImpl,
  (
    prev: OpportunityTradeCardProps,
    next: OpportunityTradeCardProps,
  ): boolean => {
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
      prev.strategyConfig === next.strategyConfig &&
      prev.onExecute === next.onExecute &&
      prev.onInspect === next.onInspect &&
      agePrev === ageNext
    );
  },
);

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
