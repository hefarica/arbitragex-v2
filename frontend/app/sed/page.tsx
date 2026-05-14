import { SedConvergencePanel } from "@/features/sed/SedConvergencePanel";
import { ActivityIcon } from "lucide-react";

export const metadata = {
  title: "SED Pipeline — ArbitrageX",
};

export default function SedPage() {
  return (
    <div className="p-8">
      <div className="mb-6">
        <h1 className="text-3xl font-bold tracking-tight flex items-center gap-3">
          <ActivityIcon className="w-8 h-8 text-primary" />
          SED Pipeline
        </h1>
        <p className="text-muted-foreground text-sm mt-1">
          Spectral Economic Drive — real-time telemetry and pipeline convergence monitoring
        </p>
      </div>
      <div className="max-w-4xl">
        <SedConvergencePanel />
      </div>
    </div>
  );
}
