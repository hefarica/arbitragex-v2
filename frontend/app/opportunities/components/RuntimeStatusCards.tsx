import { AlertCircleIcon, ActivityIcon, DatabaseIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import type { StrategyRuntimeStatusResponse, StrategyRuntimeStatusItem } from "@/lib/operations-schemas";

interface Props {
  data: StrategyRuntimeStatusResponse | null;
  error: string | null;
  fetchedAt: string | null;
}

export function RuntimeStatusCards({ data, error, fetchedAt }: Props) {
  if (error && !data) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <ActivityIcon className="size-4" /> Strategy Runtime Status
          </CardTitle>
          <CardDescription>
            <span className="flex items-center gap-1.5 text-warning">
              <AlertCircleIcon className="size-3.5" />
              <span className="font-mono text-xs">{error}</span>
            </span>
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  if (!data) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <ActivityIcon className="size-4 animate-pulse" /> Strategy Runtime Status
          </CardTitle>
          <CardDescription>Loading runtime status snapshot…</CardDescription>
        </CardHeader>
      </Card>
    );
  }

  const getSemanticState = (s: StrategyRuntimeStatusItem) => {
    if (!s.engine_loaded) {
      return { text: "text-muted-foreground", bar: "bg-muted-foreground/40", label: "Engine not loaded" };
    }
    
    if (s.data_dependencies_status !== "ok") {
      switch (s.data_dependencies_status) {
        case "armed_waiting_for_impact":
          return { text: "text-warning", bar: "bg-warning/40", label: "Armed, waiting for impact" };
        case "waiting_for_profitable_base":
          return { text: "text-warning", bar: "bg-warning/40", label: "Waiting for profitable base" };
        case "missing_lending_watchlist":
          return { text: "text-destructive", bar: "bg-destructive/40", label: "Missing lending watchlist" };
        default:
          return { text: "text-destructive", bar: "bg-destructive/40", label: `Deps Error: ${s.data_dependencies_status}` };
      }
    }

    if (s.candidates_1h > 0) {
      return { text: "text-success", bar: "bg-success/40", label: "Producing candidates" };
    } else if (s.rejections_1h > 0) {
      return { text: "text-warning", bar: "bg-warning/40", label: "Detecting, rejected by gates" };
    } else {
      return { text: "text-muted-foreground", bar: "bg-muted-foreground/40", label: "Idle / No signal" };
    }
  };

  const getTitle = (kind: string) => {
    if (kind === "dex_arb") return "Dex Arb";
    if (kind === "triangular_arb") return "Triangular Arb";
    if (kind === "flashloan_arb") return "Flashloan Arb";
    if (kind === "liquidation") return "Liquidation";
    return kind;
  };

  const fmtAge = (iso: string | null): string => {
    if (!iso) return "";
    const ms = Date.now() - new Date(iso).getTime();
    if (ms < 60_000) return `${Math.round(ms / 1000)}s ago`;
    return `${Math.round(ms / 60_000)}m ago`;
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center justify-between text-base">
          <span className="flex items-center gap-2">
            <ActivityIcon className="size-4 text-success" />
            Strategy Runtime Status
          </span>
          <span className="font-mono text-xs text-muted-foreground">{fmtAge(fetchedAt)}</span>
        </CardTitle>
        <CardDescription>
          Real-time observability of strategy engine states, dependencies, and candidate production.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
          {data.strategies.map((s) => {
            const state = getSemanticState(s);
            return (
              <div key={s.strategy_kind} className="flex flex-col space-y-2 border rounded-lg p-3 bg-card">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium">{getTitle(s.strategy_kind)}</span>
                  <div className={`h-2.5 w-2.5 rounded-full ${state.bar}`} title={state.label} />
                </div>
                <div>
                  <div className={`font-mono text-xl tabular-nums ${state.text}`}>{s.candidates_1h}</div>
                  <div className="text-muted-foreground text-[10px] uppercase tracking-wider">CANDS / 1H</div>
                </div>
                <div className="text-xs text-muted-foreground truncate" title={state.label}>
                  {state.label}
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground mt-2 pt-2 border-t">
                  <div className="flex items-center gap-1" title="Engine Invoked">
                    <DatabaseIcon className="w-3 h-3" />
                    <span>{s.engine_invoked ? "Yes" : "No"}</span>
                  </div>
                  <div className="flex items-center gap-1" title="Rejections / 1H">
                    <ActivityIcon className="w-3 h-3" />
                    <span>{s.rejections_1h}</span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}
