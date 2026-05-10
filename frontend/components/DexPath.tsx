import React from "react";

/**
 * DexPath.tsx — Pure component that renders the DEX leg path of an
 * opportunity with explicit BUY / SELL / VIA semantics per strategy_kind
 * so the operator sees at a glance which venue is the buy side and which
 * is the sell side, rather than just two DEX names with an arrow between.
 *
 * Strategy semantics (canonical mapping):
 *
 *   dex_arb         dex_a = BUY venue,  dex_b = SELL venue
 *   flashloan_arb   dex_a = BUY venue,  dex_b = SELL venue (flash-loan backed)
 *   triangular      dex_a = VIA (3 swaps inside one DEX family); dex_b unused
 *   liquidation     dex_a = TARGET protocol (e.g. Aave V3); dex_b unused
 *   backrun         dex_a = VICTIM target; dex_b = our BACKRUN venue
 *   cex_dex         dex_a = DEX leg; dex_b unused (CEX side carried elsewhere)
 *
 * R1 Mounted Snapshot compliance: pure component, no hooks, no Date/Math
 * randomness, deterministic across SSR + CSR.
 *
 * R8 fail-honest: unknown strategy_kind renders the legacy "dex_a → dex_b"
 * fallback so we never lie about the leg semantics.
 */

export interface DexPathProps {
  strategy_kind: string;
  dex_a: string;
  dex_b: string | null;
}

interface Leg {
  /** Action verb shown in uppercase pill before the DEX name. */
  action: "BUY" | "SELL" | "VIA" | "TARGET" | "VICTIM" | "BACKRUN" | "DEX";
  /** DEX or protocol name as recorded in the opportunity row. */
  venue: string;
}

function legsFor(strategy_kind: string, dex_a: string, dex_b: string | null): Leg[] {
  switch (strategy_kind) {
    case "dex_arb":
    case "flashloan_arb":
      // Two-leg atomic arb: buy low on dex_a, sell high on dex_b.
      // When dex_b is null the searcher detected an intra-DEX cycle (two
      // pools of the same DEX family on different fee tiers, e.g.
      // Uniswap V3 0.05% ↔ 0.30%). Render as BUY → SELL inside the same
      // venue so the operator still sees the action semantics.
      return dex_b
        ? [{ action: "BUY", venue: dex_a }, { action: "SELL", venue: dex_b }]
        : [{ action: "BUY", venue: dex_a }, { action: "SELL", venue: dex_a }];
    case "triangular":
      // Three-swap cycle inside one DEX family — the venue is the cycle's host.
      return [{ action: "VIA", venue: dex_a }];
    case "liquidation":
      // Liquidation against a lending protocol (Aave / Compound / etc.).
      return [{ action: "TARGET", venue: dex_a }];
    case "backrun":
      return dex_b
        ? [{ action: "VICTIM", venue: dex_a }, { action: "BACKRUN", venue: dex_b }]
        : [{ action: "VICTIM", venue: dex_a }];
    case "cex_dex":
      // DEX leg only; CEX side is carried in metadata, not dex_a/dex_b.
      return [{ action: "DEX", venue: dex_a }];
    default:
      // Unknown strategy — surface the raw fields so we never invent semantics.
      return dex_b
        ? [{ action: "DEX", venue: dex_a }, { action: "DEX", venue: dex_b }]
        : [{ action: "DEX", venue: dex_a }];
  }
}

function actionStyle(action: Leg["action"]): { bg: string; text: string; border: string } {
  switch (action) {
    case "BUY":
      return { bg: "bg-success/15", text: "text-success", border: "border-success/40" };
    case "SELL":
      return { bg: "bg-destructive/15", text: "text-destructive", border: "border-destructive/40" };
    case "TARGET":
      return { bg: "bg-warning/15", text: "text-warning", border: "border-warning/40" };
    case "VICTIM":
      return { bg: "bg-destructive/10", text: "text-destructive/80", border: "border-destructive/30" };
    case "BACKRUN":
      return { bg: "bg-info/15", text: "text-info", border: "border-info/40" };
    case "VIA":
      return { bg: "bg-primary/10", text: "text-primary", border: "border-primary/30" };
    case "DEX":
    default:
      return { bg: "bg-muted/40", text: "text-muted-foreground", border: "border-border" };
  }
}

export function DexPath({ strategy_kind, dex_a, dex_b }: DexPathProps) {
  const legs = legsFor(strategy_kind, dex_a, dex_b);
  return (
    <span className="inline-flex items-center gap-1.5 flex-wrap text-xs">
      {legs.map((leg, idx) => {
        const style = actionStyle(leg.action);
        return (
          <React.Fragment key={`${leg.action}-${leg.venue}-${idx}`}>
            <span
              className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded border font-bold tracking-wide ${style.bg} ${style.text} ${style.border}`}
              title={`${leg.action} on ${leg.venue}`}
            >
              <span>{leg.action}</span>
              <span className="font-mono opacity-90">{leg.venue}</span>
            </span>
            {idx < legs.length - 1 && (
              <span className="text-muted-foreground/60" aria-hidden="true">
                →
              </span>
            )}
          </React.Fragment>
        );
      })}
    </span>
  );
}
