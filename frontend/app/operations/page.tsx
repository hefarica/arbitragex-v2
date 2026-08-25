/**
 * Sprint 3 Task 3.4 — /operations Server Component.
 *
 * Fetches initial KPI + S-curve snapshot via INTERNAL_EDGE_URL inside the
 * Docker network and hands them to OperationsClient as `initialSnapshot`.
 * Client polls every 30s thereafter.
 */
import { PageHeader } from "@/components/page-header";
import {
  getCanonicalKnobs,
  getOperationsKpi,
  getOperationsScurve,
  getRouteDiscoveryTick,
  getScannerHeartbeat,
} from "@/lib/api-client";
import { extractCanonicalMode } from "@/lib/apex/schemas/knobs";

import { OperationsClient } from "./OperationsClient";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function OperationsPage() {
  // Pipeline funnel snapshot is independent of KPI/S-curve — failure to
  // fetch it (404 = searcher down) must NOT propagate to initialError
  // and hide the rest of the page (R8 fail-honest, partial degradation).
  // ARBX-0011: the canonical-knobs mode view gets the same isolation — a
  // 503 (knobs_not_published pre-boot) renders the honest absence chip,
  // never blocks the KPIs. Boot snapshot: fetched once, not polled.
  // ARBX-QB-07-008: the lat.* boot snapshot (tick GET) gets the SAME
  // isolation — its failure/absence renders the honest dash table, never
  // blocks the KPIs (bidirectional: OperationsClient keeps the panel
  // visible in ITS outage branches too).
  const [kpiRes, scurveRes, heartbeatRes, knobsRes, tickRes] = await Promise.all([
    getOperationsKpi(1),
    getOperationsScurve(1, 15),
    getScannerHeartbeat(1),
    getCanonicalKnobs(),
    getRouteDiscoveryTick(1),
  ]);

  const initialKpi = kpiRes.ok ? kpiRes.data : null;
  const initialScurve = scurveRes.ok ? scurveRes.data : null;
  const initialHeartbeat = heartbeatRes.ok ? heartbeatRes.data : null;
  const initialHeartbeatError = !heartbeatRes.ok ? heartbeatRes.error : null;
  const initialError = !kpiRes.ok ? kpiRes.error : !scurveRes.ok ? scurveRes.error : null;
  const initialModeView = knobsRes.ok ? extractCanonicalMode(knobsRes.data.knobs) : null;
  const initialModeError = !knobsRes.ok
    ? knobsRes.error
    : initialModeView === null
      ? "mode fields absent from knobs snapshot (searcher boot snapshot shape)"
      : null;
  // QB-07-008: partial() schema — absent keys are real backend states, so
  // they map to the honest absence (null), never defaults.
  const initialLatStages = tickRes.ok ? tickRes.data.lat_stages ?? null : null;
  const initialLatPass = tickRes.ok ? tickRes.data.lat_pass_p95 ?? null : null;
  const initialLatCycles = tickRes.ok ? tickRes.data.lat_cycles ?? 0 : 0;
  const initialLatError = !tickRes.ok ? tickRes.error : null;

  return (
    <>
      <PageHeader
        title="Convergence Metrics"
        lede="Earned Value convergence metrics + scanner pipeline funnel. Updated every 30s."
        showRefresh
      />
      <OperationsClient
        initialKpi={initialKpi}
        initialScurve={initialScurve}
        initialHeartbeat={initialHeartbeat}
        initialHeartbeatError={initialHeartbeatError}
        initialError={initialError}
        initialModeView={initialModeView}
        initialModeError={initialModeError}
        initialLatStages={initialLatStages}
        initialLatPass={initialLatPass}
        initialLatCycles={initialLatCycles}
        initialLatError={initialLatError}
      />
    </>
  );
}
