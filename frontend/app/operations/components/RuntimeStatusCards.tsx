import { AlertCircleIcon, ActivityIcon, CpuIcon, DatabaseIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import type { StrategyRuntimeStatusResponse, StrategyRuntimeStatusItem } from "@/lib/operations-schemas";

interface Props {
  data: StrategyRuntimeStatusResponse | null;
  error: string | null;
  fetchedAt: string | null;
}

export function RuntimeStatusCards({ data, error, fetchedAt }: Props) {
  if (error) {
    return (
      <Alert variant="destructive">
        <AlertCircleIcon />
        <AlertTitle>Runtime Status Error</AlertTitle>
        <AlertDescription className="font-mono text-xs">{error}</AlertDescription>
      </Alert>
    );
  }

  if (!data) {
    return (
      <Card>
        <CardContent className="py-8 text-sm text-muted-foreground">Loading strategy runtime status…</CardContent>
      </Card>
    );
  }

  const getSemanticState = (s: StrategyRuntimeStatusItem) => {
    if (!s.engine_loaded) {
      return { color: "bg-gray-500", label: "Engine not loaded" };
    }
    
    if (s.data_dependencies_status !== "ok") {
      switch (s.data_dependencies_status) {
        case "armed_waiting_for_impact":
          return { color: "bg-yellow-500", label: "Armado, esperando impacto rentable" };
        case "waiting_for_profitable_base":
          return { color: "bg-yellow-500", label: "Esperando base profitable" };
        case "missing_lending_watchlist":
          return { color: "bg-red-500", label: "Requiere watchlist lending" };
        default:
          return { color: "bg-red-500", label: `Deps Error: ${s.data_dependencies_status}` };
      }
    }

    if (s.candidates_1h > 0) {
      return { color: "bg-green-500", label: "Produciendo" };
    } else if (s.rejections_1h > 0) {
      return { color: "bg-yellow-500", label: "Detectando, rechazado por gates" };
    } else {
      return { color: "bg-gray-500", label: "Idle / Sin señal" };
    }
  };

  const getTitle = (kind: string) => {
    if (kind === "dex_arb") return "Dex Arb";
    if (kind === "triangular_arb") return "Triangular Arb";
    if (kind === "flashloan_arb") return "Flashloan Arb";
    if (kind === "liquidation") return "Liquidation";
    return kind;
  };

  return (
    <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
      {data.strategies.map((s) => {
        const state = getSemanticState(s);
        return (
          <Card key={s.strategy_kind} className="overflow-hidden">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">{getTitle(s.strategy_kind)}</CardTitle>
              <div className={`h-3 w-3 rounded-full ${state.color}`} title={state.label} />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold mb-1">{s.candidates_1h} cands / 1h</div>
              <p className="text-xs text-muted-foreground font-mono truncate" title={state.label}>
                {state.label}
              </p>
              <div className="mt-4 grid grid-cols-2 gap-2 text-xs text-muted-foreground">
                <div className="flex items-center gap-1">
                  <DatabaseIcon className="w-3 h-3" />
                  <span>Invoked: {s.engine_invoked ? "Yes" : "No"}</span>
                </div>
                <div className="flex items-center gap-1">
                  <ActivityIcon className="w-3 h-3" />
                  <span>Rejects: {s.rejections_1h}</span>
                </div>
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
