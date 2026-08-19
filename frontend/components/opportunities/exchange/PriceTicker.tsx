"use client";
// frontend/components/opportunities/exchange/PriceTicker.tsx
//
// G-PRICE-1 — exchange-style live price tape for the Exchange page.
//
// Consumes usePricesStream (snapshot+push WS, edge-polling fallback) and
// renders one chip per requested symbol: price + direction (▲/▼) vs the
// previous full-map frame. Memory-disciplined like the card grid: symbols are
// capped by the caller and each chip is plain text (no logos, no motion).
//
// R8 fail-honest: a symbol with no live price renders "—" (never 0, never a
// fabricated value). Empty feed renders an explicit connecting/empty state.
// R1: the freshness timestamp is the only non-deterministic segment and is
// suppressed on its own <span>.

import React, { useMemo } from "react";
import { usePricesStream } from "@/lib/hooks/usePricesStream";

/** Hard cap on rendered chips — memory discipline (mirrors VISIBLE_CAP idea). */
const MAX_SYMBOLS = 14;

function formatPrice(v: number): string {
  // Compact adaptive precision: 4 significant-ish digits, no exponent leak.
  if (v >= 1000) return v.toLocaleString("en-US", { maximumFractionDigits: 2 });
  if (v >= 1) return v.toFixed(4).replace(/0+$/, "").replace(/\.$/, "");
  if (v >= 0.0001) return v.toFixed(6).replace(/0+$/, "").replace(/\.$/, "");
  return v.toPrecision(4);
}

export function PriceTicker({
  chainId,
  edgeUrl,
  symbols,
}: {
  chainId: number | null;
  edgeUrl: string;
  symbols: string[];
}) {
  const { state, status } = usePricesStream(chainId, edgeUrl);

  const upperSymbols = useMemo(
    () =>
      Array.from(new Set(symbols.map((s) => s.trim().toUpperCase()).filter(Boolean))).slice(
        0,
        MAX_SYMBOLS,
      ),
    [symbols],
  );

  if (chainId === null || upperSymbols.length === 0) return null;

  const statusClass =
    status === "LIVE" ? "led on" : status === "STALE" ? "led wait led-hot" : "led wait";

  return (
    <div
      className="glass-panel mb-4 flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-2"
      data-testid="price-ticker"
      data-status={status}
    >
      <span className="chip" title={`USD price stream · chain ${chainId} · snapshot+push`}>
        <span className={statusClass} />
        PRICES <b>{status}</b>
      </span>
      {upperSymbols.map((sym) => {
        const price = state.prices[sym];
        const prev = state.prevPrices[sym];
        const hasPrev = prev !== undefined && prev > 0;
        const up = price !== undefined && hasPrev && price > prev;
        const down = price !== undefined && hasPrev && price < prev;
        const dir = up ? "▲" : down ? "▼" : "•";
        return (
          <span key={sym} className="text-xs font-mono">
            <span className="text-muted-foreground">{sym}</span>{" "}
            {price !== undefined ? (
              <>
                <span className="text-foreground">${formatPrice(price)}</span>{" "}
                <span className={up ? "text-success" : "text-muted-foreground"}>{dir}</span>
              </>
            ) : (
              <span className="text-muted-foreground">—</span>
            )}
          </span>
        );
      })}
      <span className="ml-auto text-[10px] text-muted-foreground" suppressHydrationWarning>
        {state.ts ? `upd ${new Date(state.ts).toLocaleTimeString()}` : "waiting first frame…"}
      </span>
    </div>
  );
}
