import { RadarIcon } from "lucide-react";
import { RouteDiscoveryPanel } from "@/features/route-discovery/RouteDiscoveryPanel";
import { CartridgeTelemetryPanel } from "@/features/route-discovery/CartridgeTelemetryPanel";

export const metadata = {
  title: "Strategy Forge — ArbitrageX",
};

/**
 * /strategies/forge — Strategy Forge UI.
 *
 * Surfaces the two LIVE shadow telemetry streams end-to-end:
 *   - OMEGA Route Discovery radar (arbx:route_discovery:telemetry)
 *   - Cartridge telemetry (arbx:cartridge:telemetry)
 *
 * Both are observe-only. The radar discovers/classifies route topology and
 * NEVER writes opportunities — opportunities (arbx:opps:detected) are the native
 * orchestrator's stream, shown on /opportunities.
 */
export default function StrategyForgePage() {
  return (
    <div className="p-8">
      <div className="mb-6">
        <h1 className="text-3xl font-bold tracking-tight flex items-center gap-3">
          <RadarIcon className="w-8 h-8 text-primary" />
          Strategy Forge
        </h1>
        <p className="text-muted-foreground text-sm mt-1">
          Live shadow telemetry — Route Discovery radar + cartridge evaluation. Observe-only;
          the radar does not write opportunities.
        </p>
      </div>
      <div className="space-y-6 max-w-6xl">
        <RouteDiscoveryPanel />
        <CartridgeTelemetryPanel />
      </div>
    </div>
  );
}
