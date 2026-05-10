"use client";

// FE-9: DateRangePicker for /recon page.
// R1 Mounted Snapshot Pattern: all state and effects are client-only.
// The server page passes initialSummary + initialTimeseries as props.
// When the operator changes the date range, this component re-fetches from
// the edge using NEXT_PUBLIC_EDGE_URL (baked at build time).

import { useState, useCallback } from "react";
import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ReconKpiGrid } from "@/features/recon/ReconKpiGrid";
import { TopStrategiesTable } from "@/features/recon/TopStrategiesTable";
import { CriticalAnomalies } from "@/features/recon/CriticalAnomalies";
import { AttemptsBreakdown } from "@/features/charts/AttemptsBreakdown";
import { RevertRateGauge } from "@/features/charts/RevertRateGauge";
import { StrategyScoreBarChart } from "@/features/charts/StrategyScoreBarChart";
import { ReconPnLChart } from "@/features/charts/ReconPnLChart";
import { MotionItem, MotionStagger } from "@/components/motion";
import { getReconSummary, getReconTimeseries } from "@/lib/api-client";
import type { ReconSummary, ReconTimeseriesResponse } from "@/lib/api-client";

// Quick-select presets
const PRESETS: { label: string; hours: number }[] = [
  { label: "1h", hours: 1 },
  { label: "6h", hours: 6 },
  { label: "24h", hours: 24 },
  { label: "7d", hours: 168 },
];

interface Props {
  initialSummary: ReconSummary;
  initialTimeseries: ReconTimeseriesResponse | null;
}

export function ReconClient({ initialSummary, initialTimeseries }: Props) {
  const [summary, setSummary] = useState<ReconSummary>(initialSummary);
  const [timeseries, setTimeseries] = useState<ReconTimeseriesResponse | null>(initialTimeseries);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Controlled window in hours — default mirrors SSR (1h).
  const [windowHours, setWindowHours] = useState<number>(initialSummary.window_hours);

  const applyWindow = useCallback(async (hours: number) => {
    setLoading(true);
    setError(null);
    // Bucket size: keep proportional (min 15m, max 60m).
    const bucketMinutes = hours <= 6 ? 15 : hours <= 48 ? 60 : 240;
    const [sumRes, tsRes] = await Promise.all([
      getReconSummary(hours),
      getReconTimeseries(hours, bucketMinutes),
    ]);
    setLoading(false);
    if (!sumRes.ok) {
      setError(`Summary fetch failed: ${sumRes.error}`);
      return;
    }
    setSummary(sumRes.data);
    setTimeseries(tsRes.ok ? tsRes.data : null);
    if (!tsRes.ok) {
      // Non-fatal — KPIs still show; just clear chart.
      setError(`Timeseries fetch failed: ${tsRes.error}`);
    }
  }, []);

  const handlePreset = (hours: number) => {
    setWindowHours(hours);
    void applyWindow(hours);
  };

  return (
    <div className="space-y-6">
      {/* ── DateRange controls ── */}
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm text-muted-foreground">Window:</span>
        {PRESETS.map((p) => (
          <Button
            key={p.hours}
            variant={windowHours === p.hours ? "default" : "outline"}
            size="sm"
            onClick={() => handlePreset(p.hours)}
            disabled={loading}
          >
            {p.label}
          </Button>
        ))}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void applyWindow(windowHours)}
          disabled={loading}
          title="Refresh current window"
          aria-label="Refresh"
        >
          <RefreshCwIcon className={`size-4 ${loading ? "animate-spin" : ""}`} />
        </Button>
        <span className="text-xs text-muted-foreground">
          {loading ? "Fetching…" : `Showing last ${windowHours}h`}
        </span>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>fetch error</AlertTitle>
          <AlertDescription><code className="font-mono text-xs">{error}</code></AlertDescription>
        </Alert>
      )}

      <ReconKpiGrid summary={summary} />

      {timeseries ? (
        <div className="mb-8">
          <ReconPnLChart response={timeseries} />
        </div>
      ) : !error ? (
        <Alert variant="destructive" className="mb-8">
          <AlertCircleIcon />
          <AlertTitle>timeseries unavailable</AlertTitle>
          <AlertDescription>No timeseries data for this window.</AlertDescription>
        </Alert>
      ) : null}

      <MotionStagger className="mb-8 grid gap-4 lg:grid-cols-3">
        <MotionItem><AttemptsBreakdown totals={summary.totals} /></MotionItem>
        <MotionItem><RevertRateGauge rate={summary.revert_rate} /></MotionItem>
        <MotionItem><StrategyScoreBarChart rows={summary.top_strategies} /></MotionItem>
      </MotionStagger>

      <h2>Top strategies</h2>
      <TopStrategiesTable rows={summary.top_strategies} />

      <h2>Critical anomalies (last 24h)</h2>
      <CriticalAnomalies rows={summary.critical_anomalies_24h} />
    </div>
  );
}
