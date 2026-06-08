"use client";

import Link from "next/link";
import { CheckCircle2, CircleDashed, LockKeyhole, RefreshCw, ShieldAlert } from "lucide-react";
import type { SystemReadinessState, SystemReadinessStep } from "@/hooks/useSystemReadiness";
import { useSystemReadiness } from "@/hooks/useSystemReadiness";

type Props = {
  readiness: SystemReadinessState;
  className?: string;
};

const STATUS_STYLES = {
  complete: "border-emerald-200 bg-emerald-50/80 text-emerald-800 shadow-[0_12px_40px_rgba(16,185,129,0.10)]",
  pending: "border-slate-200 bg-white/80 text-slate-800 shadow-[0_12px_40px_rgba(15,23,42,0.06)]",
  blocked: "border-slate-200 bg-slate-50/80 text-slate-500 opacity-70",
  error: "border-red-200 bg-red-50/80 text-red-800 shadow-[0_12px_40px_rgba(239,68,68,0.08)]",
} as const;

function StepIcon({ step }: { step: SystemReadinessStep }) {
  if (step.status === "complete") return <CheckCircle2 className="size-5 text-emerald-600" aria-hidden="true" />;
  if (step.status === "error") return <ShieldAlert className="size-5 text-red-600" aria-hidden="true" />;
  if (step.status === "blocked") return <LockKeyhole className="size-5 text-slate-400" aria-hidden="true" />;
  return <CircleDashed className="size-5 text-slate-500" aria-hidden="true" />;
}

function ReadinessStepCard({ step }: { step: SystemReadinessStep }) {
  const statusLabel = step.status === "complete" ? "Completado" : step.status === "blocked" ? "Bloqueado" : step.status === "error" ? "Error" : "Pendiente";
  return (
    <Link
      href={step.href}
      className={`group rounded-2xl border p-4 backdrop-blur-sm transition hover:-translate-y-0.5 hover:border-slate-300 ${STATUS_STYLES[step.status]}`}
      aria-label={`Paso ${step.index}: ${step.title}. Estado: ${statusLabel}`}
    >
      <div className="flex items-start gap-3">
        <div className="mt-0.5 rounded-full bg-white/70 p-1 ring-1 ring-black/5">
          <StepIcon step={step} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-500">Paso {step.index}</span>
            <span className="rounded-full border border-current/15 bg-white/60 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.16em]">
              {statusLabel}
            </span>
          </div>
          <h3 className="mt-2 text-sm font-bold text-slate-950">{step.title}</h3>
          <p className="mt-1 text-xs font-semibold text-slate-600">{step.label}</p>
          <p className="mt-2 text-xs leading-5 text-slate-600">{step.description}</p>
          <p className="mt-3 truncate font-mono text-[11px] text-slate-500" title={step.evidence}>{step.evidence}</p>
        </div>
      </div>
    </Link>
  );
}

export function ReadinessStepper({ readiness, className = "" }: Props) {
  const progress = `${readiness.completedCount}/${readiness.totalCount}`;
  return (
    <section className={`rounded-[1.5rem] border border-slate-200 bg-white/80 p-5 shadow-[0_20px_70px_rgba(15,23,42,0.08)] backdrop-blur-xl ${className}`}>
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.28em] text-slate-500">Live Readiness Tracker</p>
          <h2 className="mt-2 text-2xl font-bold tracking-tight text-slate-950">Secuencia de Ignición Operativa</h2>
          <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600">
            El estado se deriva de comprobaciones server-side. Ningún paso futuro se marca verde por simulación; permanece bloqueado hasta existir un endpoint de validación real.
          </p>
        </div>
        <div className="flex items-center gap-3 rounded-2xl border border-slate-200 bg-slate-50/80 px-4 py-3">
          {readiness.isLoading ? <RefreshCw className="size-4 animate-spin text-slate-500" aria-hidden="true" /> : null}
          <div>
            <div className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">Progreso</div>
            <div className="mt-1 font-mono text-2xl font-bold text-slate-950">{progress}</div>
          </div>
        </div>
      </div>

      {readiness.error && (
        <div className="mt-4 rounded-xl border border-red-200 bg-red-50/80 px-4 py-3 text-sm text-red-700">
          Verificador de readiness no disponible: <code className="font-mono text-xs">{readiness.error}</code>
        </div>
      )}

      <div className="mt-5 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        {readiness.steps.map((step) => <ReadinessStepCard key={step.id} step={step} />)}
      </div>

      <div className="mt-4 flex flex-col gap-3 rounded-2xl border border-amber-200 bg-amber-50/70 p-4 text-sm text-amber-900 md:flex-row md:items-center md:justify-between">
        <p>
          <strong>Guardrail LIVE:</strong> el modo LIVE permanece visualmente bloqueado hasta que los cuatro pasos del pipeline estén en verde.
        </p>
        <span className="rounded-full border border-amber-300 bg-white/70 px-3 py-1 font-mono text-xs uppercase tracking-[0.18em]">
          {readiness.allReady ? "LIVE habilitable" : "LIVE bloqueado"}
        </span>
      </div>
    </section>
  );
}

export function LiveReadinessStepper({ className = "" }: { className?: string }) {
  const readiness = useSystemReadiness();
  return <ReadinessStepper readiness={readiness} className={className} />;
}
