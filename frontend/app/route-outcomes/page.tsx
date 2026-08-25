import { RadarIcon, ShieldCheckIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { RouteDiscoveryOutcomesPanel } from "@/features/route-discovery/RouteDiscoveryOutcomesPanel";
import { ViableByHopsPanel } from "@/features/route-discovery/ViableByHopsPanel";
// FE-0038 (§47): the §47 group-bys over the same outcomes sink.
import { RouteOutcomesAnalyticsPanel } from "@/features/route-discovery/RouteOutcomesAnalyticsPanel";

export const metadata = {
  title: "Route Outcomes — ArbitrageX",
};

/**
 * /route-outcomes — FASE B Gate-C hit-rate analytics (enterprise-premium shell).
 *
 * Read-side of the durable `route_discovery_outcomes` sink. Surfaces, observe-only,
 * the resolved-outcome series the shadow emitter writes: total outcomes, the
 * opportunity hit-rate, and — most importantly — the honest reason distribution
 * (v3_sizing_pending, insufficient_pools, …) that explains WHY the convergence
 * yield is currently 0. The panel polls the public read-only REST snapshot every
 * 8s via the edge. It NEVER writes opportunities (arbx:opps:detected) or capital.
 */
export default function RouteOutcomesPage() {
  return (
    <main className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 md:p-6">
      <header className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
        <div className="space-y-1">
          <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
            <RadarIcon className="h-7 w-7 text-primary" />
            <span className="text-gradient-primary">Route Outcomes</span>
          </h1>
          <p className="max-w-2xl text-sm text-muted-foreground">
            Gate-C hit-rate series over the durable route-discovery outcomes table — the
            honest reason distribution behind the current convergence yield. Observe-only:
            this is analytics over resolved outcomes, not an execution surface.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="secondary" className="uppercase tracking-wide">
            shadow
          </Badge>
          <Badge variant="outline" className="gap-1 text-muted-foreground">
            <ShieldCheckIcon className="h-3 w-3" />
            read-only
          </Badge>
        </div>
      </header>

      <Separator />

      <section className="flex flex-col gap-6">
        <RouteDiscoveryOutcomesPanel />
        {/* XLS-DASH-01 — workbook 29_SUPER_DASHBOARD viable-KPI set (by hops /
            by kind / viability %) over REAL opportunities rows. */}
        <ViableByHopsPanel />
        {/* FE-0038 (§47): by-strategy / by-pair analytics + honest gaps for the
            dimensions the sink does not persist (hop / detector / DEX). */}
        <RouteOutcomesAnalyticsPanel />
      </section>
    </main>
  );
}
