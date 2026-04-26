import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { KillswitchBanner } from "@/features/risk/KillswitchBanner";
import { RiskAlertsTable } from "@/features/risk/RiskAlertsTable";
import { getRiskAlerts } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function RiskPage() {
  const res = await getRiskAlerts(24);

  if (!res.ok) {
    return (
      <>
        <PageHeader
          title="Risk & alerts"
          lede="Circuit-breakers, anomalies, blacklist hits, kill-switch log."
          showRefresh
        />
        <FocusOnMount>
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>edge error</AlertTitle>
            <AlertDescription><code className="font-mono text-xs">{res.error}</code></AlertDescription>
          </Alert>
        </FocusOnMount>
      </>
    );
  }

  const { alerts, window_hours, killswitch, ts } = res.data;

  return (
    <>
      <PageHeader
        title="Risk & alerts"
        lede={`Active kill-switch state and every risk event recorded by services in the last ${window_hours}h.`}
        meta={[`window: last ${window_hours}h`, `snapshot ${fmtTime(ts)}`]}
        showRefresh
      />

      {killswitch && <KillswitchBanner ks={killswitch} />}

      <h2>Recent events</h2>
      <RiskAlertsTable alerts={alerts} windowHours={window_hours} />
    </>
  );
}
