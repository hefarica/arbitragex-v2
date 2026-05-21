import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { FocusOnMount } from "@/components/focus-on-mount";
import { PageHeader } from "@/components/page-header";
import { KillswitchBanner } from "@/features/risk/KillswitchBanner";
import { RiskAlertsTable } from "@/features/risk/RiskAlertsTable";
import { RiskCircuitPanel } from "@/features/risk/RiskCircuitPanel";
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

      {/* A.6 (2026-05-13): comprehensive circuit breakers panel. Rendered
          BEFORE the recent events table so the operator sees current breaker
          state first. Additive — does not remove KillswitchBanner or
          RiskAlertsTable. */}
      <div className="mt-6">
        <RiskCircuitPanel />
      </div>

      <h2 className="mt-8">Recent events</h2>
      <RiskAlertsTable alerts={alerts} windowHours={window_hours} />
    </>
  );
}
