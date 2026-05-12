/**
 * Sprint 3 Task 3.4 — /operations Server Component.
 *
 * Fetches initial KPI + S-curve snapshot via INTERNAL_EDGE_URL inside the
 * Docker network and hands them to OperationsClient as `initialSnapshot`.
 * Client polls every 30s thereafter.
 */
import { PageHeader } from "@/components/page-header";
import { getOperationsKpi, getOperationsScurve, getScannerHeartbeat, getStrategyRuntimeStatus } from "@/lib/api-client";

import { OperationsClient } from "./OperationsClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function OperationsPage() {
  // Pipeline funnel snapshot is independent of KPI/S-curve — failure to
  // fetch it (404 = searcher down) must NOT propagate to initialError
  // and hide the rest of the page (R8 fail-honest, partial degradation).
  const [kpiRes, scurveRes, heartbeatRes, runtimeStatusRes] = await Promise.all([
    getOperationsKpi(1),
    getOperationsScurve(1, 15),
    getScannerHeartbeat(1),
    getStrategyRuntimeStatus(1),
  ]);

  const initialKpi = kpiRes.ok ? kpiRes.data : null;
  const initialScurve = scurveRes.ok ? scurveRes.data : null;
  const initialHeartbeat = heartbeatRes.ok ? heartbeatRes.data : null;
  const initialHeartbeatError = !heartbeatRes.ok ? heartbeatRes.error : null;
  const initialRuntimeStatus = runtimeStatusRes.ok ? runtimeStatusRes.data : null;
  const initialRuntimeStatusError = !runtimeStatusRes.ok ? runtimeStatusRes.error : null;
  const initialError = !kpiRes.ok ? kpiRes.error : !scurveRes.ok ? scurveRes.error : null;

  return (
    <>
      <PageHeader
        title="Operations PnL"
        lede="PMI Earned Value Management metrics + scanner pipeline funnel. Updated every 30s."
        showRefresh
      />
      <OperationsClient
        initialKpi={initialKpi}
        initialScurve={initialScurve}
        initialHeartbeat={initialHeartbeat}
        initialHeartbeatError={initialHeartbeatError}
        initialError={initialError}
        initialRuntimeStatus={initialRuntimeStatus}
        initialRuntimeStatusError={initialRuntimeStatusError}
      />
    </>
  );
}
