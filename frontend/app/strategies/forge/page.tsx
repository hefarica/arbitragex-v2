import { RadarIcon, ShieldCheckIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { RouteDiscoveryPanel } from "@/features/route-discovery/RouteDiscoveryPanel";
import { CartridgeTelemetryPanel } from "@/features/route-discovery/CartridgeTelemetryPanel";
import { CartridgeFilterPanel } from "@/features/cartridge/CartridgeFilterPanel";

export const metadata = {
  title: "Strategy Forge — ArbitrageX",
};

/**
 * /strategies/forge — Strategy Forge (enterprise-premium shell).
 *
 * Surfaces the two LIVE shadow telemetry streams end-to-end, observe-only:
 *   - OMEGA Route Discovery radar (arbx:route_discovery:telemetry)
 *   - Cartridge telemetry (arbx:cartridge:telemetry)
 *
 * The radar discovers/classifies route topology and NEVER writes opportunities —
 * opportunities (arbx:opps:detected) are the native orchestrator's stream, on
 * /opportunities. Panels poll the public read-only REST snapshot every 5s.
 */
export default function StrategyForgePage() {
  return (
    <main className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 md:p-6">
      <header className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
        <div className="space-y-1">
          <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
            <RadarIcon className="h-7 w-7 text-primary" />
            <span className="text-gradient-primary">Strategy Forge</span>
          </h1>
          <p className="max-w-2xl text-sm text-muted-foreground">
            Live shadow telemetry — Route Discovery radar + cartridge evaluation. Observe-only:
            the radar discovers and classifies route topology but does not write opportunities.
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
        <RouteDiscoveryPanel />
        <CartridgeTelemetryPanel />
        <CartridgeFilterPanel />
      </section>
    </main>
  );
}
