"use client";
/**
 * OpportunitiesByStrategyClient — By Strategy view (FE-0039 — §48/§49).
 *
 * §49 — this page is a PROJECTION of the Exchange Feed: same wire
 * (/api/opportunities/live), same mapper (mapToOmniOpportunity), same
 * null-honest identity (FE-0028/FE-0029). No second universe of strategies
 * and no second fetching semantic: the only added logic is the GROUPING,
 * which is a pure view fold (by-strategy-grouping.ts).
 *
 * §48 — the strategy axis derives from OmniOpportunity[] × the canonical
 * registry: cartridge_id (MEV-xx-xxx) is the registry key; a kind-only row
 * falls back to its 5-token family; a row with NEITHER lands in the honest
 * `unknown` bucket — never a "dex_arb" default (the exact semantic default
 * FE-0029 removed; this client's old `?? "dex_arb"` fold is gone with it).
 * Registry metadata renders verbatim on matched groups; unmatched cartridges
 * render as drift; the join coverage is disclosed in the summary strip.
 *
 * R1 Mounted-Snapshot Pattern: receives initialOpportunities from the Server
 * Component, then polls every 4s. Fail-honest: poll errors surface an inline
 * banner; last good snapshot preserved. Safety: observe-only.
 */
// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";
import { useEffect, useState } from "react";
import { AlertCircleIcon, SatelliteDishIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { mapToOmniOpportunity, type OmniOpportunity, type StrategyKind } from "@/lib/store/types";
import { formatProfitUSD, formatPctOrDash } from "@/lib/format";
import { StrategyBadge } from "@/components/StrategyBadge";
import { useOmniStore } from "@/lib/store/omni-store";
import {
  groupByStrategy,
  joinCoverage,
  type StrategyGroup,
} from "./by-strategy-grouping";

const POLL_INTERVAL_MS = 4_000;

const STRATEGY_LABELS: Record<StrategyKind, string> = {
  dex_arb:       "DEX Arbitrage",
  triangular:    "Triangular",
  backrun:       "Backrun",
  liquidation:   "Liquidation",
  flashloan_arb: "Flashloan Arb",
};

// ─── Strategy group card ──────────────────────────────────────────────────────
function StrategyGroupCard({ group }: { group: StrategyGroup }) {
  const { opps, registry, axis, key } = group;
  const kind = opps[0]?.strategy_kind ?? null;
  const totalProfit = opps.reduce((sum, o) => sum + (o.net_expected_profit_usd ?? 0), 0);
  const avgRoi = opps.length > 0
    ? opps.reduce((sum, o) => sum + (o.roi_pct ?? 0), 0) / opps.length
    : 0;

  const fmtProfit = formatProfitUSD(totalProfit);
  const fmtRoiVal = formatPctOrDash(avgRoi);
  const fmtRoi    = typeof fmtRoiVal === 'string' ? { display: fmtRoiVal, tone: 'neutral' as const } : fmtRoiVal;

  return (
    <Card data-testid={`strategy-group-${key}`}>
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="flex flex-wrap items-center gap-2 text-sm">
            <StrategyBadge strategy_kind={kind} />
            {axis === "registry" && (
              <span className="font-mono text-xs text-foreground">{key}</span>
            )}
            {axis === "kind" && key in STRATEGY_LABELS && (
              <span className="text-muted-foreground font-normal">{STRATEGY_LABELS[key as StrategyKind]}</span>
            )}
            {axis === "unknown" && (
              <span className="text-muted-foreground font-normal">unknown — payload sin strategy_id ni cartridge_id</span>
            )}
          </CardTitle>
          <Badge variant="secondary" className="text-xs">{opps.length} signals</Badge>
        </div>
        {/* §48 join disclosure — registry metadata verbatim, or honest drift. */}
        {axis === "registry" && registry && (
          <div className="flex flex-wrap gap-1">
            <Badge variant="outline" className="text-[10px]">{registry.name}</Badge>
            <Badge variant="outline" className="text-[10px]">{registry.family}</Badge>
            <Badge variant="outline" className="font-mono text-[10px]">{registry.detector_id}</Badge>
            <Badge variant="outline" className="text-[10px]">{registry.execution_class}</Badge>
          </div>
        )}
        {axis === "registry" && !registry && (
          <p className="text-[10px] text-destructive">
            cartridge sin match en el registry (drift honesto — jamás metadata prestada)
          </p>
        )}
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

  // §48: the registry join — static-per-canon catalog (ready-guarded fetch).
  const byMevId = useOmniStore((s) => s.strategyByMevId);
  const fetchStrategyCatalog = useOmniStore((s) => s.fetchStrategyCatalog);
  useEffect(() => {
    void fetchStrategyCatalog();
  }, [fetchStrategyCatalog]);

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

  const groups = groupByStrategy(opps, byMevId);
  const coverage = joinCoverage(groups);

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
          Strategies active: <strong>{groups.length}</strong>
        </span>
        <span className="text-sm text-muted-foreground">
          Registry join: <strong>{coverage.matched}</strong> matched · <strong>{coverage.unmatched}</strong> drift
          {coverage.unknown > 0 && (<> · <strong>{coverage.unknown}</strong> unknown</>)}
        </span>
      </div>

      {/* §49 provenance — same wire, same mapper, no second universe. */}
      <p className="text-[10px] text-muted-foreground" data-testid="by-strategy-provenance">
        Proyección del Exchange Feed: mismo wire /api/opportunities/live + mismo mapper
        mapToOmniOpportunity; sin universo propio de estrategias (§48) — el eje es el
        registry canónico (264).
      </p>

      {opps.length === 0 ? (
        <div className="rounded-lg border p-8 text-center text-sm text-muted-foreground" data-testid="by-strategy-empty">
          No convergence signals detected yet — the searcher-rs pipeline may still be initializing.
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" data-testid="by-strategy-grid">
          {groups.map(group => (
            <StrategyGroupCard key={group.key} group={group} />
          ))}
        </div>
      )}
    </div>
  );
}
