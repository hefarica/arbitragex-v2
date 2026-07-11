import type { ReactNode } from "react";

import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { MotionItem, MotionStagger } from "@/components/motion";
import type { ReconSummary } from "@/lib/api-client";
import { fmtMoney, fmtMs, fmtPct01 } from "@/lib/formatters";

type KpiTone = "success" | "danger" | "warning" | "info";

const TONE_CLASSES: Record<KpiTone, string> = {
  success: "bg-success/10 text-success",
  danger: "bg-destructive/10 text-destructive",
  warning: "bg-warning/15 text-warning",
  info: "bg-info/10 text-info",
};

function Kpi({ title, value, hint, tone }: { title: string; value: string; hint?: string; tone?: KpiTone }) {
  const pill = tone ? TONE_CLASSES[tone] : "bg-muted text-muted-foreground";
  return (
    <Card>
      <CardHeader>
        <div className={`inline-flex w-fit items-center gap-2 rounded-md px-2 py-0.5 text-[11px] uppercase tracking-widest ${pill}`}>
          {title}
        </div>
        <CardTitle className="mt-2 text-2xl font-medium tracking-tight font-mono tabular-nums">{value}</CardTitle>
        {hint && <CardDescription>{hint}</CardDescription>}
      </CardHeader>
    </Card>
  );
}

export function ReconKpiGrid({ summary }: { summary: ReconSummary }): ReactNode {
  const s = summary;
  return (
    <MotionStagger className="mb-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
      <MotionItem><Kpi title="Attempts" value={String(s.totals.total)} hint={`in last ${s.window_hours}h`} /></MotionItem>
      <MotionItem><Kpi title="Included" value={String(s.totals.included)} hint="blocks confirmed" tone="success" /></MotionItem>
      <MotionItem><Kpi title="Reverted" value={String(s.totals.reverted)} hint="on-chain reverts" tone="danger" /></MotionItem>
      <MotionItem><Kpi title="Revert rate" value={fmtPct01(s.revert_rate)} hint="lower is better" /></MotionItem>
      <MotionItem><Kpi title="Avg PnL (incl.)" value={fmtMoney(s.totals.avg_pnl_included_usd)} hint="per included tx" /></MotionItem>
      <MotionItem><Kpi title="Avg confirm latency" value={fmtMs(s.totals.avg_confirm_latency_ms)} hint="submit → block" /></MotionItem>
    </MotionStagger>
  );
}
