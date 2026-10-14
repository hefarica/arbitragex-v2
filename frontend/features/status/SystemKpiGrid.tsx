"use client";

// PAPER-MODE-KPI-HONEST-01 (audit 2026-09-29): this file now resolves the
// paper-mode state through the canonical hook, so it must be a Client Component.

import type { ReactNode } from "react";
import { AlertCircleIcon, CheckCircle2Icon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react";

import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { MotionItem, MotionStagger } from "@/components/motion";
import { usePaperModeState } from "@/hooks/usePaperModeState";
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

export function SystemKpiGrid({ status, paperMode }: { status: StatusResponse; paperMode?: boolean }) {
  const s = status;
  const ksArmed = s.killswitch?.enabled ?? false;
  // PAPER-MODE-KPI-HONEST-01 (audit 2026-09-29): this cell used to hardcode
  // value="ON" with the hint "Real capital is not at risk until S9." and NO
  // producer anywhere in the stack — `StatusResponse` does not even carry a
  // paper-mode field. That is a false safety claim on the page the operator uses
  // to decide, and it would keep saying ON the day paper mode is switched off.
  // Now it reads the canonical resolver. Honesty detail: when the endpoint is
  // unreachable the resolver falls back to DEFAULT_SAFE_STATE, which is
  // `enabled: true, degraded: true, reasons: ["endpoint_unavailable"]` — so the
  // value remains fail-safe but the hint says the state could not be verified,
  // the tone degrades to warning, and a source conflict (a genuine safety event)
  // is surfaced as such instead of being averaged away.
  const paper = usePaperModeState();
  const paperEnabled = paper.isLoading ? (paperMode ?? true) : paper.data.enabled;
  const paperDegraded = !paper.isLoading && paper.data.degraded;
  const paperConflict = !paper.isLoading && paper.data.conflict;
  const paperHint = paper.isLoading
    ? "resolving paper-mode state…"
    : paperConflict
      ? `SOURCE CONFLICT — verify before any live decision (source: ${paper.data.source})`
      : paperDegraded
        ? `unverified — ${paper.data.reasons.join(", ") || "state endpoint unavailable"}`
        : `source: ${paper.data.source} · confidence: ${paper.data.confidence}`;
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
          value={paperEnabled ? "ON" : "OFF"}
          tone={paperConflict ? "danger" : paperDegraded ? "warning" : paperEnabled ? "success" : "danger"}
          hint={paperHint}
          icon={<ShieldCheckIcon className="size-4" />}
        />
      </MotionItem>
    </MotionStagger>
  );
}
