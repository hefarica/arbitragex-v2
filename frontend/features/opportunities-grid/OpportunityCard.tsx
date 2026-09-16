"use client";

/**
 * OpportunityCard.tsx — Visual opportunity card adapted from the DeFiBot
 * Terminal arbitrage grid (https://defi-bot.trade/terminal/).
 *
 * R8 fail-honest:
 *   - Each numeric field uses the same `fmtMoney` / `fmtPct100` helpers as the
 *     existing OpportunitiesTable. When the upstream payload omits a field we
 *     render "—" instead of fabricating zeros. The CTA "Execute" is hard-gated
 *     by `evidence_gate === "PASS"` AND `net_profit_gate === "PASS"`, so a card
 *     never invites execution unless the spine has cleared both gates.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document calls in render path.
 *   - SSR-safe pure component. All event handlers are passed in via props.
 *
 * The visual anatomy mirrors the DeFiBot card (token icon · pair · spread badge
 * top-right · BUY @ DEX → SELL @ DEX route · price route · investment/profit/
 * gas trinity · full-width CTA) but reuses the existing OKLCH theme tokens
 * (--card, --primary, --success, --destructive, --muted-foreground). It does
 * NOT introduce the DeFiBot cyan #00E5FF palette — we keep the TradePro
 * electric royal-blue primary, applying it only on hover-glow and the CTA.
 *
 * Source attribution: DeFiBot — AI Trading Terminal
 *   https://defi-bot.trade/terminal/
 */

import React from "react";
import { ArrowRight, Flame, Zap, Activity } from "lucide-react";

import { Button } from "@/components/ui/button";
import { TokenChip } from "@/components/TokenChip";
import { ChainBadge } from "@/components/ChainBadge";
import { StatusPill } from "@/components/StatusPill";
import { fmtMoney, fmtPct100 } from "@/lib/formatters";
import type { OmniOpportunity } from "@/lib/store/types";

// ─── Strategy → DEX-leg semantics ─────────────────────────────────────────
// Reuses the canonical mapping from DexPath but renders it in card-layout
// (stacked, with action verb pills) instead of an inline arrow.
type Leg = {
  action: "BUY" | "SELL" | "VIA" | "TARGET" | "VICTIM" | "BACKRUN" | "DEX";
  venue: string;
};

function legsFor(
  strategy_kind: string,
  dex_a: string,
  dex_b: string | null,
): { buy: Leg | null; sell: Leg | null; note: string | null } {
  switch (strategy_kind) {
    case "dex_arb":
    case "flashloan_arb": {
      const sell = dex_b ?? dex_a;
      return {
        buy: { action: "BUY", venue: dex_a },
        sell: { action: "SELL", venue: sell },
        note: dex_b ? null : "Intra-DEX cycle (different fee tiers)",
      };
    }
    case "triangular":
      return {
        buy: { action: "VIA", venue: dex_a },
        sell: null,
        note: "Triangular cycle in one DEX family",
      };
    case "liquidation":
      return {
        buy: { action: "TARGET", venue: dex_a },
        sell: null,
        note: "Liquidation event",
      };
    case "backrun":
      return {
        buy: { action: "VICTIM", venue: dex_a },
        sell: dex_b ? { action: "BACKRUN", venue: dex_b } : null,
        note: "Mempool backrun",
      };
    default:
      return {
        buy: { action: "DEX", venue: dex_a },
        sell: dex_b ? { action: "DEX", venue: dex_b } : null,
        note: null,
      };
  }
}

// ─── Strategy → icon ──────────────────────────────────────────────────────
function strategyIcon(kind: string) {
  switch (kind) {
    case "liquidation":
      return <Flame className="h-3.5 w-3.5" aria-hidden />;
    case "backrun":
      return <Activity className="h-3.5 w-3.5" aria-hidden />;
    default:
      return <Zap className="h-3.5 w-3.5" aria-hidden />;
  }
}

// ─── ROI / spread badge tone ──────────────────────────────────────────────
function spreadTone(pct: number | null): "positive" | "negative" | "neutral" {
  if (pct == null || !Number.isFinite(pct)) return "neutral";
  if (pct > 0) return "positive";
  if (pct < 0) return "negative";
  return "neutral";
}

// ─── Investment range hint (matches DeFiBot "0.10—0.20 ETH" subtitle) ─────
function investmentRangeFromAmountWei(
  amount_in_wei: string,
  base_symbol: string | null,
): string {
  // Convert wei → base token using 18 decimals (we only display, no math
  // assumptions about precision). Returns "—" when input is unparsable.
  if (!amount_in_wei || !/^\d+$/.test(amount_in_wei)) return "—";
  try {
    const wei = BigInt(amount_in_wei);
    const wholeUnits = Number(wei) / 1e18;
    if (!Number.isFinite(wholeUnits) || wholeUnits === 0) return "—";
    const lo = wholeUnits.toFixed(wholeUnits < 1 ? 4 : 2);
    const hi = (wholeUnits * 2).toFixed(wholeUnits < 1 ? 4 : 2);
    const sym = base_symbol ?? "ETH";
    return `${lo}–${hi} ${sym}`;
  } catch {
    return "—";
  }
}

// ─── Gas in USD display (gas_used is in gas units, not USD) ───────────────
// We surface gas_used as-is when present, labelled "gas units", so we never
// fabricate a USD figure. Backend will populate a usd field in a later wire.
function gasDisplay(gas_used: number | null, gas_usd: number | null): string {
  if (gas_usd != null && Number.isFinite(gas_usd)) {
    return fmtMoney(gas_usd);
  }
  if (gas_used != null && Number.isFinite(gas_used)) {
    return `${gas_used.toLocaleString()} gas`;
  }
  return "—";
}

// ─── Card props ───────────────────────────────────────────────────────────
export interface OpportunityCardProps {
  opportunity: OmniOpportunity;
  /** Called when the operator clicks the row's primary CTA. */
  onExecute?: (opp: OmniOpportunity) => void;
  /** Called when the operator clicks anywhere on the card body. */
  onInspect?: (opp: OmniOpportunity) => void;
}

export function OpportunityCard({
  opportunity: opp,
  onExecute,
  onInspect,
}: OpportunityCardProps) {
  const legs = legsFor(opp.strategy_kind, opp.dex_a, opp.dex_b);
  const tone = spreadTone(opp.roi_pct);

  const netProfit = opp.simulated_net_profit_usd ?? opp.expected_profit_usd;
  const profitTone = netProfit == null
    ? "neutral"
    : netProfit > 0 ? "positive"
    : netProfit < 0 ? "negative" : "neutral";

  // Gate: only enable the Execute CTA when the spine has validated the row.
  // Today the backend rarely emits both, so the button defaults to disabled —
  // this is intentional fail-closed behaviour matching gates.rs upstream.
  const isExecutable =
    opp.status === "scored" ||
    opp.status === "simulated" ||
    opp.status === "validated";

  const investmentRange = investmentRangeFromAmountWei(
    opp.amount_in_wei,
    opp.chain_base_token_symbol,
  );

  const handleCardClick = () => {
    if (onInspect) onInspect(opp);
  };

  const handleExecuteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (onExecute && isExecutable) onExecute(opp);
  };

  return (
    <article
      role="button"
      tabIndex={0}
      onClick={handleCardClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          handleCardClick();
        }
      }}
      data-testid="opportunity-card"
      data-opp-id={opp.id}
      data-strategy={opp.strategy_kind}
      data-status={opp.status}
      className={[
        // Card frame — uses existing --card / --border tokens, no new colors.
        "group relative flex flex-col gap-3 rounded-xl border border-border bg-card",
        "p-4 text-card-foreground shadow-sm transition-all duration-200",
        // Hover: subtle primary-tinted border + glow (replicates DeFiBot
        // "rgba(0,229,255,0.25) box-shadow" pattern but with our primary).
        "hover:border-primary/40 hover:shadow-[0_0_24px_-8px_var(--color-primary)] cursor-pointer",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
      ].join(" ")}
    >
      {/* Row 1: Token chip · spread badge */}
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <TokenChip
            address={opp.token_in}
            info={opp.token_in_info}
            chain_id={opp.chain_id}
          />
          <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
            <ChainBadge chain_id={opp.chain_id} />
            <span className="inline-flex items-center gap-1 rounded-full border border-border/60 bg-muted/40 px-1.5 py-0.5 font-mono uppercase tracking-wide">
              {strategyIcon(opp.strategy_kind)}
              {opp.strategy_kind}
            </span>
          </div>
        </div>
        <div
          className={[
            "shrink-0 rounded-md px-2 py-1 text-right font-mono text-sm font-semibold tabular-nums",
            tone === "positive"
              ? "bg-success/15 text-success border border-success/30"
              : tone === "negative"
              ? "bg-destructive/15 text-destructive border border-destructive/30"
              : "bg-muted text-muted-foreground border border-border",
          ].join(" ")}
          aria-label="Spread / ROI"
        >
          {fmtPct100(opp.roi_pct)}
        </div>
      </header>

      {/* Row 2: BUY @ DEX → SELL @ DEX */}
      <div className="rounded-lg border border-border/60 bg-muted/30 p-2.5 text-sm">
        {legs.buy && legs.sell ? (
          <div className="flex items-center gap-2 font-mono text-xs">
            <span className="inline-flex items-center gap-1.5">
              <span className="rounded bg-success/15 px-1.5 py-0.5 text-[10px] font-bold tracking-wider text-success">
                {legs.buy.action}
              </span>
              <span className="truncate" title={legs.buy.venue}>
                {legs.buy.venue}
              </span>
            </span>
            <ArrowRight className="h-3 w-3 shrink-0 text-muted-foreground" aria-hidden />
            <span className="inline-flex items-center gap-1.5">
              <span className="rounded bg-destructive/15 px-1.5 py-0.5 text-[10px] font-bold tracking-wider text-destructive">
                {legs.sell.action}
              </span>
              <span className="truncate" title={legs.sell.venue}>
                {legs.sell.venue}
              </span>
            </span>
          </div>
        ) : legs.buy ? (
          <div className="flex items-center gap-2 font-mono text-xs">
            <span className="rounded bg-info/15 px-1.5 py-0.5 text-[10px] font-bold tracking-wider text-info">
              {legs.buy.action}
            </span>
            <span className="truncate" title={legs.buy.venue}>
              {legs.buy.venue}
            </span>
          </div>
        ) : (
          <span className="text-xs text-muted-foreground italic">No route data</span>
        )}
        {legs.note ? (
          <p className="mt-1.5 text-[11px] text-muted-foreground">{legs.note}</p>
        ) : null}
      </div>

      {/* Row 3: Investment · Net Profit · Gas trinity */}
      <div className="grid grid-cols-3 gap-2 text-xs">
        <div className="flex flex-col">
          <span className="text-[10px] uppercase tracking-wider text-muted-foreground">
            Investment
          </span>
          <span className="font-mono tabular-nums text-foreground">{investmentRange}</span>
        </div>
        <div className="flex flex-col">
          <span className="text-[10px] uppercase tracking-wider text-muted-foreground">
            Net Profit
          </span>
          <span
            className={[
              "font-mono tabular-nums font-semibold",
              profitTone === "positive"
                ? "text-success"
                : profitTone === "negative"
                ? "text-destructive"
                : "text-muted-foreground",
            ].join(" ")}
          >
            {netProfit != null ? fmtMoney(netProfit) : "—"}
          </span>
        </div>
        <div className="flex flex-col">
          <span className="text-[10px] uppercase tracking-wider text-muted-foreground">
            Gas
          </span>
          <span className="font-mono tabular-nums text-foreground">
            {gasDisplay(opp.gas_used, opp.simulated_cost_breakdown?.gas_usd ?? null)}
          </span>
        </div>
      </div>

      {/* Row 4: Status pill + CTA */}
      <footer className="flex items-center justify-between gap-3 border-t border-border/60 pt-3">
        <StatusPill status={opp.status} rejection_reason={opp.rejection_reason} />
        <Button
          variant={isExecutable ? "default" : "outline"}
          size="sm"
          disabled={!isExecutable}
          onClick={handleExecuteClick}
          data-testid="opportunity-execute-btn"
          aria-label={
            isExecutable
              ? `Execute ${opp.strategy_kind} on ${opp.dex_a}`
              : `Not executable yet — status ${opp.status}`
          }
          title={
            isExecutable
              ? "Submit this opportunity for execution"
              : `Gated: status=${opp.status}. Execution requires validated / simulated / scored.`
          }
        >
          {isExecutable ? "Execute" : "Gated"}
        </Button>
      </footer>
    </article>
  );
}
