"use client";

import { useEffect, useState } from "react";
import { getOpportunitiesLive } from "@/lib/api-client";
import { terminalOpportunityState } from "@/lib/opportunity-presentation";
import type { OpportunityRow } from "@/lib/schemas";

interface TickerItem {
  pair: string;
  from: string;
  to: string;
  yield: number | null;
  status: string;
  rejectionReason: string | null;
  ago: string;
}

/**
 * DAPP-SURFACE-FAIL (a11y): the ticker renders inside the root layout and its
 * error state used to dump the raw upstream error — including full Cloudflare
 * 502 JSON bodies — into an aria-labeled region. Bound it: keep the status
 * code / first meaningful token, collapse JSON, cap the length. The full error
 * remains available via the page's own error surfaces; the ticker is a
 * decorative marquee, its job is to say "feed unavailable", not to echo payloads.
 */
export function summarizeTickerError(err: string): string {
  const status = err.match(/HTTP (\d{3})/)?.[1];
  const head = (err.split("\n")[0] ?? err).replace(/\s+/g, " ").trim();
  const concise = status ? `edge HTTP ${status}` : head;
  return concise.length > 120 ? `${concise.slice(0, 117)}…` : concise;
}

function formatAgo(detectedAt: string): string {
  const detected = new Date(detectedAt).getTime();
  const now = Date.now();
  const diffSeconds = Math.floor((now - detected) / 1000);

  if (diffSeconds < 60) return `${diffSeconds}s`;
  if (diffSeconds < 3600) return `${Math.floor(diffSeconds / 60)}m`;
  return `${Math.floor(diffSeconds / 3600)}h`;
}

export function opportunityToTickerItem(opp: OpportunityRow): TickerItem | null {
  // Use net_expected_profit_usd (NET yield) when available, fallback to expected_profit_usd (GROSS)
  const profit = opp.net_expected_profit_usd ?? opp.expected_profit_usd ?? null;
  if (profit === null || !Number.isFinite(profit)) return null;

  const pair = opp.pair_symbol ?? `${opp.token_in.slice(0, 6)}…/${opp.token_out.slice(0, 6)}…`;
  const from = opp.dex_a ?? "Unknown";
  const to = opp.dex_b ?? opp.dex_a ?? "Unknown";

  // USD is not a percentage. Without recorded ROI/capital there is no
  // denominator, so never infer ROI from a hypothetical $1000 position.
  const yieldPct = opp.roi_pct != null && Number.isFinite(opp.roi_pct) ? opp.roi_pct : null;

  return {
    pair,
    from,
    to,
    yield: yieldPct,
    status: terminalOpportunityState(opp) ?? opp.status,
    rejectionReason: opp.rejection_reason ?? null,
    ago: formatAgo(opp.detected_at),
  };
}

export function formatTickerYield(value: number | null): string {
  return value == null || !Number.isFinite(value)
    ? "ROI —"
    : `ROI ${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

export function OpportunityTicker() {
  const [mounted, setMounted] = useState(false);
  const [items, setItems] = useState<TickerItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setMounted(true);

    async function fetchOpportunities() {
      const result = await getOpportunitiesLive(20);
      if (result.ok) {
        const tickerItems = result.data.items
          .map(opportunityToTickerItem)
          .filter((item): item is TickerItem => item !== null);
        setItems(tickerItems);
        setError(null);
      } else {
        setError(result.error);
        setItems([]);
      }
      setLoading(false);
    }

    fetchOpportunities();

    // Refresh every 30 seconds
    const interval = setInterval(fetchOpportunities, 30000);
    return () => clearInterval(interval);
  }, []);

  if (!mounted) {
    return (
      <div className="ticker" role="status" aria-label="Live opportunity feed">
        <div className="ticker-track">
          <span className="ticker-item">Loading opportunities...</span>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="ticker" role="status" aria-label="Live opportunity feed">
        <div className="ticker-track">
          <span className="ticker-item">Loading opportunities...</span>
        </div>
      </div>
    );
  }

  if (error || items.length === 0) {
    return (
      <div className="ticker" role="status" aria-label="Live opportunity feed">
        <div className="ticker-track">
          <span className="ticker-item">
            {error
              ? `Opportunity feed unavailable — ${summarizeTickerError(error)} (retrying every 30s)`
              : "No topological convergence detected — waiting for market topology..."}
          </span>
        </div>
      </div>
    );
  }

  // Duplicate items for seamless loop
  const displayItems = [...items, ...items];
  const latest = items[0]; // non-empty in this branch, but tsc cannot narrow it

  return (
    <div className="ticker">
      <span className="sr-only">
        {latest
          ? `Live opportunity feed: ${items.length} recent opportunities — latest ${latest.pair} ${latest.from} to ${latest.to} ${latest.status.toUpperCase()} · ${formatTickerYield(latest.yield)}.`
          : "Live opportunity feed."}
      </span>
      <div className="ticker-track" aria-hidden="true">
        {displayItems.map((item, idx) => {
          const isPositive = item.yield != null && item.yield >= 0;
          const tone = item.yield == null ? undefined : isPositive ? "pos" : "neg";
          return (
            <span key={idx} className="ticker-item">
              <b>{item.pair}</b>
              <span>·</span>
              <span>{item.from} → {item.to}</span>
              <span>·</span>
              <span title={item.rejectionReason ?? `Recorded status: ${item.status}`}>
                {item.status.toUpperCase()}
              </span>
              <span className={tone}>{formatTickerYield(item.yield)}</span>
              {item.yield != null && (
                <span className={`arr ${tone}`}>{isPositive ? "▲" : "▼"}</span>
              )}
              <span>·</span>
              <span className="ago">{item.ago}</span>
            </span>
          );
        })}
      </div>
    </div>
  );
}
