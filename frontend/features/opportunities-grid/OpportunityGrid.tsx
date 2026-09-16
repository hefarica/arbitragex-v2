"use client";

/**
 * OpportunityGrid.tsx — Responsive grid wrapper for OpportunityCard.
 *
 * Mirrors the DeFiBot Arbitrage Monitor 5-column responsive layout
 * (https://defi-bot.trade/terminal/ — "Arbitrage Monitor" tab) while reusing
 * the existing OKLCH theme tokens and zero-mocks data contract.
 *
 * R8 fail-honest:
 *   - Empty list renders a documented empty state, never a placeholder card.
 *   - The header count reflects the actual `items.length` — no fake "live"
 *     numbers.
 */

import React from "react";

import { OpportunityCard } from "./OpportunityCard";
import type { OmniOpportunity } from "@/lib/store/types";

export interface OpportunityGridProps {
  items: OmniOpportunity[];
  onExecute?: (opp: OmniOpportunity) => void;
  onInspect?: (opp: OmniOpportunity) => void;
  /** Optional title; defaults to "Arbitrage Monitor". */
  title?: string;
}

export function OpportunityGrid({
  items,
  onExecute,
  onInspect,
  title = "Arbitrage Monitor",
}: OpportunityGridProps) {
  const profitableCount = items.filter(
    (o) =>
      (o.simulated_net_profit_usd ?? o.expected_profit_usd ?? 0) > 0,
  ).length;

  return (
    <section
      data-testid="opportunity-grid"
      className="flex flex-col gap-4"
      aria-label={title}
    >
      <header className="flex items-center justify-between">
        <h2 className="text-base font-semibold tracking-tight">{title}</h2>
        <p className="text-xs text-muted-foreground">
          {items.length === 0
            ? "Waiting for opportunities"
            : `${profitableCount} profitable · ${items.length} total`}
        </p>
      </header>

      {items.length === 0 ? (
        <div
          role="status"
          className="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border bg-card/60 py-12 text-center"
        >
          <p className="text-sm font-medium text-muted-foreground">
            No opportunities yet
          </p>
          <p className="max-w-sm text-xs text-muted-foreground/80">
            The spine will surface opportunities once detection, validation and
            simulation gates clear. No simulated demo data is rendered.
          </p>
        </div>
      ) : (
        <div
          // 1 col on mobile · 2 on sm · 3 on lg · 4 on xl · 5 on 2xl —
          // matches DeFiBot 5-col grid on wide screens.
          className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5"
        >
          {items.map((opp) => (
            <OpportunityCard
              key={opp.id}
              opportunity={opp}
              onExecute={onExecute}
              onInspect={onInspect}
            />
          ))}
        </div>
      )}
    </section>
  );
}
