/**
 * Sprint 3 Task 3.4 — /operations Server Component.
 *
 * Fetches initial KPI + S-curve snapshot via INTERNAL_EDGE_URL inside the
 * Docker network and hands them to OperationsClient as `initialSnapshot`.
 * Client polls every 30s thereafter.
 */
import { PageHeader } from "@/components/page-header";
import { getOperationsKpi, getOperationsScurve } from "@/lib/api-client";

import { OperationsClient } from "./OperationsClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function OperationsPage() {
  const [kpiRes, scurveRes] = await Promise.all([
    getOperationsKpi(1),
    getOperationsScurve(1, 15),
  ]);

  const initialKpi = kpiRes.ok ? kpiRes.data : null;
  const initialScurve = scurveRes.ok ? scurveRes.data : null;
  const initialError = !kpiRes.ok ? kpiRes.error : !scurveRes.ok ? scurveRes.error : null;

  return (
    <>
      <PageHeader
        title="Operations PnL"
        lede="PMI Earned Value Management metrics translated to crypto MEV operations. Updated every 30s."
        showRefresh
      />
      <OperationsClient
        initialKpi={initialKpi}
        initialScurve={initialScurve}
        initialError={initialError}
      />
    </>
  );
}
