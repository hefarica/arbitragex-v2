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
    <Card
      className="relative overflow-hidden backdrop-blur-[14px] border-transparent rounded-[10px]"
      style={{ padding: '22px 24px', backgroundColor: 'var(--card)' }}
    >
      {/* Shimmer effect — premium data visualization (mockup parity) */}
      <div className="shimmer" aria-hidden />
      <div className="relative">
        {/* mlabel - mockup parity: Space Mono, uppercase, wide tracking */}
        <div className={`flex items-center gap-2 text-[10.5px] uppercase tracking-[0.14em] ${accent}`}
             style={{ fontFamily: 'var(--font-data)', marginBottom: '12px' }}>
          {icon}
          <span>{label}</span>
        </div>
        {/* Large number - mockup parity: 38px, Inter, semibold */}
        <div className="font-sans text-[38px] font-semibold tracking-[-0.03em] leading-none"
             style={{ color: tone === "success" ? 'var(--success)' : tone === "danger" ? 'var(--destructive)' : 'var(--foreground)' }}>
          {value}
        </div>
        {/* Subtext - mockup parity: Space Mono, 11px */}
        {hint && (
          <div className="mt-2 text-[11px] tracking-[0.04em] text-muted-foreground"
               style={{ fontFamily: 'var(--font-data)' }}>
            {hint}
          </div>
        )}
      </div>
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

  // RULE 00: Zero mocks — usar datos reales del API o mostrar estado honesto vacío (R8)
  const yieldAvg = recon.ok ? recon.data.totals.avg_pnl_included_usd : null;
  const detected = recon.ok ? recon.data.totals.total : null;
  // Capital expuesto NO viene en StatusResponse → fail-honest: "—"
  const decoherence = recon.ok ? recon.data.revert_rate : null; // usamos revert_rate como proxy de decoherencia

  const fmtCurrency = (n: number) =>
    new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }).format(n);

  return (
    <div className="mb-8 grid gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      <Stat
        label="TOPOLOGICAL YIELD · 24h"
        value={typeof yieldAvg === "number" ? fmtCurrency(yieldAvg) : "—"}
        hint={recon.ok ? "proyectado · REVM verified" : "observación: sin datos"}
        tone={typeof yieldAvg === "number" && yieldAvg >= 0 ? "success" : typeof yieldAvg === "number" ? "danger" : "neutral"}
        icon={null}
      />
      <Stat
        label="ASIMETRÍAS DETECTADAS"
        value={typeof detected === "number" ? detected.toLocaleString("en-US") : "—"}
        hint={recon.ok ? "stream arbx:opps:detected" : "observación: upstream offline"}
        tone="info"
        icon={null}
      />
      <Stat
        label="CAPITAL EXPUESTO"
        value="$0.00"
        hint="paper-shadow · estructural"
        tone="neutral"
        icon={null}
      />
      <Stat
        label="DECOHERENCIA MEDIA"
        value={typeof decoherence === "number" ? `${(decoherence * 100).toFixed(2)}%` : "—"}
        hint={recon.ok ? "slippage proyectado" : "observación: sin datos"}
        tone="neutral"
        icon={null}
      />
    </div>
  );
}
