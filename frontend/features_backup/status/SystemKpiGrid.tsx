import type { ReactNode } from "react";
import { AlertCircleIcon, CheckCircle2Icon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { MotionItem, MotionStagger } from "@/components/motion";
import type { StatusResponse } from "@/lib/api-client";

type KpiTone = "success" | "warning" | "danger" | "info";

const TONE_CLASSES: Record<KpiTone, string> = {
  success: "text-success",
  warning: "text-warning",
  danger: "text-destructive",
  info: "text-info",
};

function Kpi({
  title, value, tone, hint, icon,
}: { title: string; value: string; tone: KpiTone; hint?: string; icon?: ReactNode }) {
  return (
    <Card>
      <CardHeader>
        <div className={`flex items-center gap-2 text-xs uppercase tracking-widest ${TONE_CLASSES[tone]}`}>
          {icon}
          <span>{title}</span>
        </div>
        <CardTitle className="mt-2 text-3xl font-medium tracking-tight">{value}</CardTitle>
        {hint && <CardDescription>{hint}</CardDescription>}
      </CardHeader>
    </Card>
  );
}

export function SystemKpiGrid({ status }: { status: StatusResponse }) {
  const s = status;
  const ksArmed = s.killswitch?.enabled ?? false;
  return (
    <MotionStagger className="mb-8 grid gap-4 sm:grid-cols-3">
      <MotionItem>
        <Kpi
          title="Overall"
          value={s.ok ? "OK" : "DEGRADED"}
          tone={s.ok ? "success" : "danger"}
          hint={`${Object.keys(s.services).length} services monitored`}
          icon={s.ok ? <CheckCircle2Icon className="size-4" /> : <AlertCircleIcon className="size-4" />}
        />
      </MotionItem>
      <MotionItem>
        <Kpi
          title="Kill-switch"
          value={ksArmed ? "ARMED" : "disabled"}
          tone={ksArmed ? "danger" : "success"}
          hint={s.killswitch?.reason ?? "no reason set"}
          icon={ksArmed ? <ShieldOffIcon className="size-4" /> : <ShieldCheckIcon className="size-4" />}
        />
      </MotionItem>
      <MotionItem>
        <Kpi
          title="Paper-mode"
          value="ON"
          tone="success"
          hint="Real capital is not at risk until S9."
          icon={<ShieldCheckIcon className="size-4" />}
        />
      </MotionItem>
    </MotionStagger>
  );
}
