import { ActivityIcon, AlertCircleIcon, GaugeIcon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { FocusOnMount } from "@/components/focus-on-mount";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import type { ReconSummary, StatusResponse } from "@/lib/api-client";
import { fmtPct01 } from "@/lib/formatters";

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
  // If both upstreams failed, show the worse error verbatim and stop.
  if (!status.ok && !recon.ok) {
    return (
      <FocusOnMount>
        <Alert variant="destructive" className="mb-8">
          <AlertCircleIcon />
          <AlertTitle>edge unreachable</AlertTitle>
          <AlertDescription>
            <code className="font-mono text-xs">{status.error}</code>
            <p className="mt-2 text-sm">
              Live KPIs are hidden because no upstream responded. The tiles below
              still navigate.
            </p>
          </AlertDescription>
        </Alert>
      </FocusOnMount>
    );
  }

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
        hint={status.ok ? `${Object.keys(status.data.services).length} services` : "edge error"}
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
        hint={windowH != null ? `last ${windowH}h` : recon.ok ? "" : "edge error"}
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
