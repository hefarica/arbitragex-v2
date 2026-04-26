import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { ReconKpiGrid } from "@/features/recon/ReconKpiGrid";
import { TopStrategiesTable } from "@/features/recon/TopStrategiesTable";
import { CriticalAnomalies } from "@/features/recon/CriticalAnomalies";
import { AttemptsBreakdown } from "@/features/charts/AttemptsBreakdown";
import { RevertRateGauge } from "@/features/charts/RevertRateGauge";
import { StrategyScoreBarChart } from "@/features/charts/StrategyScoreBarChart";
import { ReconPnLChart } from "@/features/charts/ReconPnLChart";
import { MotionItem, MotionStagger } from "@/components/motion";
import { getReconSummary, getReconTimeseries } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function ReconPage() {
  const [summaryRes, tsRes] = await Promise.all([
    getReconSummary(1),
    getReconTimeseries(24, 60),
  ]);

  if (!summaryRes.ok) {
    return (
      <>
        <PageHeader
          title="Recon & PnL"
          lede="Realised PnL, adaptive strategy scores, critical anomalies."
          showRefresh
        />
        <FocusOnMount>
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>edge error</AlertTitle>
            <AlertDescription><code className="font-mono text-xs">{summaryRes.error}</code></AlertDescription>
          </Alert>
        </FocusOnMount>
      </>
    );
  }

  const s = summaryRes.data;

  return (
    <>
      <PageHeader
        title="Recon & PnL"
        lede="Realised PnL and adaptive strategy scoring — both populated by the recon learning loop (S6) as executions are reconciled against their pre-trade simulation."
        meta={[`window: last ${s.window_hours}h`, `snapshot ${fmtTime(s.ts)}`]}
        showRefresh
      />

      <ReconKpiGrid summary={s} />

      {tsRes.ok ? (
        <div className="mb-8">
          <ReconPnLChart response={tsRes.data} />
        </div>
      ) : (
        <Alert variant="destructive" className="mb-8">
          <AlertCircleIcon />
          <AlertTitle>timeseries unavailable</AlertTitle>
          <AlertDescription>
            <code className="font-mono text-xs">{tsRes.error}</code>
          </AlertDescription>
        </Alert>
      )}

      <MotionStagger className="mb-8 grid gap-4 lg:grid-cols-3">
        <MotionItem><AttemptsBreakdown totals={s.totals} /></MotionItem>
        <MotionItem><RevertRateGauge rate={s.revert_rate} /></MotionItem>
        <MotionItem><StrategyScoreBarChart rows={s.top_strategies} /></MotionItem>
      </MotionStagger>

      <h2>Top strategies</h2>
      <TopStrategiesTable rows={s.top_strategies} />

      <h2>Critical anomalies (last 24h)</h2>
      <CriticalAnomalies rows={s.critical_anomalies_24h} />
    </>
  );
}
