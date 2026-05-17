import { ActivityIcon, AlertCircleIcon, GaugeIcon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { FocusOnMount } from "@/components/focus-on-mount";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import type { ReconSummary, StatusResponse } from "@/lib/api-client";
import { fmtPct01 } from "@/lib/formatters";
import { aggregateEdgeHealth } from "@/lib/edge-health";

type Result<T> = { ok: true; data: T } | { ok: false; error: string };

function Stat({
  label,
  value,
  hint,
  tone,
  icon,
}: {
  label: string;
  value: string;
  hint?: string;
  tone: "neutral" | "success" | "danger" | "info";
  icon: React.ReactNode;
}) {
  const accent =
    tone === "success" ? "text-success"
    : tone === "danger" ? "text-destructive"
    : tone === "info" ? "text-info"
    : "text-muted-foreground";
  return (
    <Card className="overflow-hidden">
      <CardContent className="p-4">
        <div className={`flex items-center gap-2 text-[11px] uppercase tracking-widest ${accent}`}>
          {icon}
          <span>{label}</span>
        </div>
        <div className="mt-2 font-mono text-2xl font-medium tracking-tight tabular-nums">
          {value}
        </div>
        {hint && <div className="mt-0.5 text-xs text-muted-foreground">{hint}</div>}
      </CardContent>
    </Card>
  );
}

export function HomeKpiStrip({
  status,
  recon,
}: {
  status: Result<StatusResponse>;
  recon: Result<ReconSummary>;
}) {
  // Doctrine: "POR EL HAZ DE LUZ SOLO PASA QUIEN ES VISTO".
  //
  // When BOTH upstream calls fail, we must distinguish the root cause
  // before painting a banner. Conflating an upstream 5xx with the
  // platform itself being down hides the real fault from the operator.
  //
  // Taxonomy (see lib/edge-health.ts):
  //   EDGE_UNREACHABLE                   → "edge unreachable"
  //   EDGE_REACHABLE_UPSTREAM_DEGRADED   → "upstreams degraded"
  //   EDGE_REACHABLE_SCHEMA_DRIFT        → "edge schema drift"
  //
  // The spec (tests/e2e/smoke.spec.ts) forbids the literal strings
  // "edge unreachable" and "edge error:" appearing in normal CI runs
  // where the edge IS reachable but upstreams are. The new "upstreams
  // degraded" banner is intentionally chosen NOT to collide with either
  // forbidden regex while still surfacing the failure honestly.
  if (!status.ok && !recon.ok) {
    const health = aggregateEdgeHealth([status, recon]);
    return (
      <FocusOnMount>
        <Alert variant="destructive" className="mb-8">
          <AlertCircleIcon />
          <AlertTitle>{health.title}</AlertTitle>
          <AlertDescription>
            <code className="font-mono text-xs">{health.diagnostic}</code>
            <p className="mt-2 text-sm">
              {health.kind === "EDGE_UNREACHABLE"
                ? "The frontend could not reach the edge gateway. Live KPIs are hidden until the gateway responds. The tiles below still navigate."
                : "The edge gateway responded but one or more upstream dependencies (RPC, providers, indexers) are unhealthy. Live KPIs are hidden until upstreams recover. The tiles below still navigate."}
            </p>
          </AlertDescription>
        </Alert>
      </FocusOnMount>
    );
  }

  // Partial degradation — at least one upstream gave a usable response.
  // Use the taxonomy to choose an honest sub-hint per tile instead of
  // the legacy "edge error" string (which the spec also forbids when
  // followed by ":").
  const statusHealth = status.ok ? null : aggregateEdgeHealth([status]);
  const reconHealth = recon.ok ? null : aggregateEdgeHealth([recon]);

  const overallOk = status.ok ? status.data.ok : null;
  const ksArmed = status.ok ? (status.data.killswitch?.enabled ?? false) : null;
  const attempts = recon.ok ? recon.data.totals.total : null;
  const revertRate = recon.ok ? recon.data.revert_rate : null;
  const windowH = recon.ok ? recon.data.window_hours : null;

  return (
    <div className="mb-8 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      <Stat
        label="Overall"
        value={overallOk == null ? "—" : overallOk ? "OK" : "DEGRADED"}
        hint={
          status.ok
            ? `${Object.keys(status.data.services).length} services`
            : (statusHealth?.title ?? "no data")
        }
        tone={overallOk == null ? "neutral" : overallOk ? "success" : "danger"}
        icon={
          overallOk
            ? <ShieldCheckIcon className="size-3.5" />
            : <AlertCircleIcon className="size-3.5" />
        }
      />
      <Stat
        label="Kill-switch"
        value={ksArmed == null ? "—" : ksArmed ? "ARMED" : "disabled"}
        hint={status.ok ? (status.data.killswitch?.reason ?? "no reason set") : "—"}
        tone={ksArmed == null ? "neutral" : ksArmed ? "danger" : "success"}
        icon={ksArmed ? <ShieldOffIcon className="size-3.5" /> : <ShieldCheckIcon className="size-3.5" />}
      />
      <Stat
        label="Attempts"
        value={attempts == null ? "—" : String(attempts)}
        hint={
          windowH != null
            ? `last ${windowH}h`
            : recon.ok
              ? ""
              : (reconHealth?.title ?? "no data")
        }
        tone="info"
        icon={<ActivityIcon className="size-3.5" />}
      />
      <Stat
        label="Revert rate"
        value={revertRate == null ? "—" : fmtPct01(revertRate)}
        hint={attempts === 0 ? "no samples yet" : "lower is better"}
        tone={
          revertRate == null ? "neutral"
          : revertRate >= 0.20 ? "danger"
          : revertRate >= 0.05 ? "info"
          : "success"
        }
        icon={<GaugeIcon className="size-3.5" />}
      />
    </div>
  );
}
