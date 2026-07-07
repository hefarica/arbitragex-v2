import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { ReconClient } from "@/app/recon/ReconClient";
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
      {/* FE-9: ReconClient owns the date-range selector and re-fetches on change. */}
      <ReconClient
        initialSummary={s}
        initialTimeseries={tsRes.ok ? tsRes.data : null}
      />
    </>
  );
}
