/**
 * Sprint 3 Task 3.4 — Convergence Metrics client component.
 *
 * Mounted Snapshot Pattern (R1): receives initialKpi/initialScurve from the
 * Server Component, hydrates state with them, polls every 30s. All
 * non-deterministic calls (Date, fetch) live inside useEffect.
 */
"use client";

// Classic-JSX runtime import for the vitest (esbuild) path — repo pattern
// (PairIntelligencePanel.tsx, ByModeKpiStrip.tsx); Next/swc ignores it.
import * as React from "react";
import { useEffect, useState } from "react";
import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card, CardContent } from "@/components/ui/card";
import { getOperationsKpi, getOperationsScurve, getScannerHeartbeat } from "@/lib/api-client";
import type {
  KpiPayload,
  SCurvePayload,
  ScannerHeartbeatResponse,
} from "@/lib/operations-schemas";
import type { CanonicalModeView } from "@/lib/apex/schemas/knobs";
import type { LatencyStageRow } from "@/lib/apex/schemas";

// ARBX-QB-07-008 (REQ-QB-015): the lat.* stage panel (p50/p95/headroom per
// discovery stage, workbook 10_LATENCY). Same both-ways isolation as the
// strip: a tick-fetch failure renders the honest absence, never blocks the
// KPIs — and the panel stays visible in the KPI outage branches below.
import { LatencyBudgetPanel } from "./components/LatencyBudgetPanel";
// ARBX-0011 (REQ-DASH-BY-MODE): by-mode KPI scope strip — the mode is a
// label over the ONE math pipeline (§34.1), fetched once at mount (boot
// snapshot; the 30s poll below refreshes measurements, not the mode view).
// R3 fix: isolation is BOTH ways — a knobs 503 never blocks the KPIs, and
// KPI absence (error/loading early-returns) must not hide the strip: it is
// doctrine labels, not KPI data, and an outage is exactly when the terminus
// scope matters most.
import { ByModeKpiStrip } from "./components/ByModeKpiStrip";
import { KPICard } from "./components/KPICard";
import { PipelineFunnelCard } from "./components/PipelineFunnelCard";
import { SCurveChart } from "./components/SCurveChart";
// FE-0038 (§46): the route-discovery funnel (Market Events → Reconciled) —
// the §46 chain over tick + outcomes sink + recon, windows disclosed.
import { RouteDiscoveryFunnelCard } from "./components/RouteDiscoveryFunnelCard";

interface Props {
  initialKpi: KpiPayload | null;
  initialScurve: SCurvePayload | null;
  initialHeartbeat: ScannerHeartbeatResponse | null;
  initialHeartbeatError: string | null;
  initialError: string | null;
  /** ARBX-0011: canonical mode view (boot snapshot — static, not polled). */
  initialModeView: CanonicalModeView | null;
  initialModeError: string | null;
  /** ARBX-QB-07-008: lat.* boot snapshot (live overlay via useRouteTick). */
  initialLatStages: LatencyStageRow[] | null;
  initialLatPass: boolean | null;
  initialLatCycles: number;
  initialLatError: string | null;
}

const POLL_MS = 30_000;

export function OperationsClient({
  initialKpi,
  initialScurve,
  initialHeartbeat,
  initialHeartbeatError,
  initialError,
  initialModeView,
  initialModeError,
  initialLatStages,
  initialLatPass,
  initialLatCycles,
  initialLatError,
}: Props) {
  const [kpi, setKpi] = useState<KpiPayload | null>(initialKpi);
  const [scurve, setScurve] = useState<SCurvePayload | null>(initialScurve);
  const [heartbeat, setHeartbeat] = useState<ScannerHeartbeatResponse | null>(initialHeartbeat);
  const [heartbeatError, setHeartbeatError] = useState<string | null>(initialHeartbeatError);
  const [error, setError] = useState<string | null>(initialError);

  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      const [k, s, h] = await Promise.all([
        getOperationsKpi(1),
        getOperationsScurve(1, 15),
        getScannerHeartbeat(1),
      ]);
      if (cancelled) return;
      if (k.ok) {
        setKpi(k.data);
        setError(null);
      } else if (!kpi) {
        setError(k.error);
      }
      if (s.ok) setScurve(s.data);
      if (h.ok) {
        setHeartbeat(h.data);
        setHeartbeatError(null);
      } else {
        setHeartbeatError(h.error);
      }
    };
    const id = setInterval(tick, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
    // We deliberately do not include `kpi` in deps; we want one persistent timer.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ARBX-0011 R3 fix: the strip renders in EVERY branch (see header note).
  // QB-07-008: the latency panel gets the same both-ways treatment.
  const modeStrip = (
    <ByModeKpiStrip view={initialModeView} error={initialModeError} />
  );
  const latPanel = (
    <LatencyBudgetPanel
      initialStages={initialLatStages}
      initialPassP95={initialLatPass}
      initialCycles={initialLatCycles}
      initialError={initialLatError}
    />
  );

  if (error && !kpi) {
    return (
      <>
        {modeStrip}
        {latPanel}
        <Alert variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>convergence endpoint error</AlertTitle>
          <AlertDescription className="font-mono text-xs">{error}</AlertDescription>
        </Alert>
      </>
    );
  }

  if (!kpi) {
    return (
      <>
        {modeStrip}
        {latPanel}
        <Card>
          <CardContent className="py-8 text-sm text-muted-foreground">Loading convergence metrics…</CardContent>
        </Card>
      </>
    );
  }

  const k = kpi.kpis;
  const fmtUsd = (v: number) => `$${v.toFixed(2)}`;

  return (
    <>
      {modeStrip}
      <div className="mb-6 grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <KPICard
          title="CPI · Capital Efficiency"
          value={k.cpi.toFixed(4)}
          hint={`yield / capital · ${k.cpi >= 0.05 ? "above target" : "below target"}`}
          positive={k.cpi >= 0.05}
        />
        <KPICard
          title="SPI · Velocity Index"
          value={k.spi.toFixed(2)}
          hint={`ops vs schedule · ${k.spi >= 1 ? "on track" : "behind"}`}
          positive={k.spi >= 1}
        />
        <KPICard
          title="EAC · Forecast Daily Yield"
          value={fmtUsd(k.eac_usd)}
          hint="end-of-day projection"
          positive={k.eac_usd >= 0}
        />
        <KPICard
          title="ETC · Remaining Runway"
          value={fmtUsd(k.etc_usd)}
          hint="capital × CPI × remaining_day"
          positive={k.etc_usd >= 0}
        />
        <KPICard
          title="TCPI · Required Efficiency"
          value={k.tcpi.toFixed(4)}
          hint="needed CPI to hit target"
          positive={k.tcpi <= 1.5}
        />
        <KPICard
          title="VAC · Projected Variance"
          value={fmtUsd(k.vac_usd)}
          hint="target − forecast"
          positive={k.vac_usd <= 0}
        />
        <KPICard
          title="CV · Realized Variance"
          value={fmtUsd(k.cv_usd)}
          hint="actual − planned (so far today)"
          positive={k.cv_usd >= 0}
        />
        <KPICard
          title="Ops completed today"
          value={kpi.ops_completed.toString()}
          hint={`target ${kpi.ops_target_per_day}/day`}
          positive={kpi.ops_completed > 0}
        />
      </div>
      <div className="mb-6">
        <PipelineFunnelCard
          snapshot={heartbeat?.snapshot ?? null}
          error={heartbeatError}
          fetchedAt={heartbeat?.fetched_at ?? null}
        />
      </div>
      {/* FE-0038 (§46): the route-discovery half of the funnel — distinct from
          the scanner (mempool) funnel above; wires and stages do not overlap. */}
      <div className="mb-6">
        <RouteDiscoveryFunnelCard />
      </div>
      {/* ARBX-QB-07-008: the lat.* budget panel rides beside the discovery
          funnel — same wire (tick), same read-only posture. */}
      <div className="mb-6">{latPanel}</div>
      {scurve && <SCurveChart data={scurve} />}
    </>
  );
}
