"use client";
/**
 * OpportunitiesByStrategyClient — live-polling view of convergence signals
 * grouped by strategy_kind.
 *
 * R1 Mounted-Snapshot Pattern: receives initialOpportunities from the Server
 * Component, then polls /api/opportunities/live every 4s.
 * Fail-honest: poll errors surface an inline banner; last good snapshot preserved.
 *
 * Safety: observe-only. No capital, no execution, no broadcast.
 * Zero-Mocks: all data from /api/opportunities/live; no fabricated defaults.
 */
import { useEffect, useState } from "react";
import { AlertCircleIcon, SatelliteDishIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { mapToOmniOpportunity, type OmniOpportunity, type StrategyKind } from "@/lib/store/types";
import { formatProfitUSD, formatPctOrDash } from "@/lib/format";
import { StrategyBadge } from "@/components/StrategyBadge";

const POLL_INTERVAL_MS = 4_000;

const STRATEGY_LABELS: Record<StrategyKind, string> = {
  dex_arb:       "DEX Arbitrage",
  triangular:    "Triangular",
  backrun:       "Backrun",
  liquidation:   "Liquidation",
  flashloan_arb: "Flashloan Arb",
};

// ─── Strategy group card ──────────────────────────────────────────────────────
function StrategyGroupCard({
  strategy,
  opps,
}: {
  strategy: StrategyKind;
  opps: OmniOpportunity[];
}) {
  const totalProfit = opps.reduce((sum, o) => sum + (o.net_expected_profit_usd ?? 0), 0);
  const avgRoi = opps.length > 0
    ? opps.reduce((sum, o) => sum + (o.roi_pct ?? 0), 0) / opps.length
    : 0;

  const fmtProfit = formatProfitUSD(totalProfit);
  const fmtRoiVal = formatPctOrDash(avgRoi);
  const fmtRoi    = typeof fmtRoiVal === 'string' ? { display: fmtRoiVal, tone: 'neutral' as const } : fmtRoiVal;

  return (
    <Card data-testid={`strategy-group-${strategy}`}>
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-sm">
            <StrategyBadge strategy_kind={strategy} />
            <span className="text-muted-foreground font-normal">{STRATEGY_LABELS[strategy]}</span>
          </CardTitle>
          <Badge variant="secondary" className="text-xs">{opps.length} signals</Badge>
        </div>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">Total profit: </span>
            <span className={`font-semibold font-mono tone-${fmtProfit.tone}`}>
              {fmtProfit.display}
            </span>
          </div>
          <div>
            <span className="text-muted-foreground">Avg ROI: </span>
            <span className="font-mono">{typeof fmtRoi === 'string' ? fmtRoi : fmtRoi.display}</span>
          </div>
        </div>

        {/* Top 3 signals */}
        {opps.slice(0, 3).map(opp => {
          const oppProfit = formatProfitUSD(opp.net_expected_profit_usd ?? 0);
          return (
            <div
              key={opp.id}
              className="mt-2 flex items-center justify-between rounded bg-muted/30 px-2 py-1 text-xs"
              data-testid={`opp-row-${opp.id}`}
            >
              <span className="font-mono text-muted-foreground truncate max-w-[120px]">
                {opp.pair_symbol ?? `${opp.token_in.slice(0, 6)}…`}
              </span>
              <span className={`font-semibold font-mono tone-${oppProfit.tone}`}>
                {oppProfit.display}
              </span>
            </div>
          );
        })}
        {opps.length > 3 && (
          <p className="mt-1 text-xs text-muted-foreground">+{opps.length - 3} more signals</p>
        )}
      </CardContent>
    </Card>
  );
}

// ─── Main client ───────────────────────────────────────────────────────────────
interface Props {
  initialOpportunities: OmniOpportunity[];
}

export function OpportunitiesByStrategyClient({ initialOpportunities }: Props) {
  const [opps, setOpps] = useState<OmniOpportunity[]>(initialOpportunities);
  const [pollError, setPollError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;

    const poll = async () => {
      try {
        const res = await fetch("/api/opportunities/live", {
          cache: "no-store",
          headers: { accept: "application/json" },
        });
        if (!alive) return;
        if (!res.ok) {
          setPollError(`HTTP ${res.status}`);
          return;
        }
        const data = await res.json();
        const raw: unknown[] = Array.isArray(data?.items) ? data.items : Array.isArray(data) ? data : [];
        setOpps(raw.map(r => mapToOmniOpportunity(r as Record<string, unknown>)));
        setPollError(null);
      } catch (e) {
        if (alive) setPollError((e as Error).message);
      }
    };

    const timer = setInterval(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  // Group by strategy_kind
  const byStrategy = opps.reduce<Record<string, OmniOpportunity[]>>((acc, opp) => {
    const key = opp.strategy_kind ?? "dex_arb";
    (acc[key] ??= []).push(opp);
    return acc;
  }, {});

  const strategyKeys = Object.keys(byStrategy) as StrategyKind[];

  return (
    <div className="space-y-6" data-testid="opportunities-by-strategy-panel">
      {pollError && (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>Poll error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{pollError}</code></AlertDescription>
        </Alert>
      )}

      {/* Summary strip */}
      <div className="flex flex-wrap items-center gap-4 rounded-lg border p-4" data-testid="by-strategy-summary">
        <div className="flex items-center gap-2">
          <SatelliteDishIcon className="h-4 w-4 text-primary" />
          <span className="text-sm font-medium">Total signals:</span>
          <span className="text-sm font-semibold">{opps.length}</span>
        </div>
        <span className="text-sm text-muted-foreground">
          Strategies active: <strong>{strategyKeys.length}</strong>
        </span>
      </div>

      {opps.length === 0 ? (
        <div className="rounded-lg border p-8 text-center text-sm text-muted-foreground" data-testid="by-strategy-empty">
          No convergence signals detected yet — the searcher-rs pipeline may still be initializing.
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" data-testid="by-strategy-grid">
          {strategyKeys.map(strategy => (
            <StrategyGroupCard
              key={strategy}
              strategy={strategy}
              opps={byStrategy[strategy] ?? []}
            />
          ))}
        </div>
      )}
    </div>
  );
}
