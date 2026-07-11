"use client";

interface XRayCardProps {
  pair: string;
  yield: string;
  confidence: number;
  legs: number;
  ago: string;
  route: string;
  fees: string;
  tlsAmount: string;
  simVerdict: string;
  safetyA: number;
  safetyB: number;
}

export function XRayCard({
  pair,
  yield: yieldValue,
  confidence,
  legs,
  ago,
  route,
  fees,
  tlsAmount,
  simVerdict,
  safetyA,
  safetyB,
}: XRayCardProps) {
  return (
    <article className="xray-card" data-interactive>
      <div className="xray-main transition-opacity duration-300">
        <div className="flex items-baseline justify-between mb-3.5">
          <span className="font-semibold text-[17px] tracking-tight">{pair}</span>
          <span className="font-mono text-base text-[var(--success)] font-bold">
            {yieldValue}
          </span>
        </div>
        <div className="flex gap-4 font-mono text-[10.5px] text-[var(--muted)] tracking-wide">
          <span>
            <b className="text-[var(--foreground)]">{confidence}</b>% conf
          </span>
          <span>
            <b className="text-[var(--foreground)]">{legs}</b> legs
          </span>
          <span>{ago}</span>
          <span className="text-[var(--primary)]">[PAPER]</span>
        </div>
      </div>

      <div className="xray-panel">
        <div className="font-mono text-[9.5px] tracking-widest uppercase text-[var(--primary-2)] mb-2">
          X-Ray · route breakdown
        </div>
        <div className="space-y-1">
          <XRayRow label="ROUTE" value={route} bold />
          <XRayRow label="DECOHERENCIA" value={fees} bold />
          <XRayRow label="TLS AMOUNT" value={tlsAmount} bold />
          <XRayRow label="SIM VERDICT" value={`✓ ${simVerdict}`} ok />
          <XRayRow label="TOKEN SAFETY" value={`A ${safetyA} · B ${safetyB}`} bold />
        </div>
      </div>
    </article>
  );
}

interface XRayRowProps {
  label: string;
  value: string;
  bold?: boolean;
  ok?: boolean;
}

function XRayRow({ label, value, bold, ok }: XRayRowProps) {
  return (
    <div className="flex justify-between font-mono text-[10.5px] tracking-wide py-0.5 text-[var(--muted)]">
      <span>{label}</span>
      <span className={ok ? "text-[var(--success)]" : bold ? "text-[var(--foreground)] font-bold" : ""}>
        {value}
      </span>
    </div>
  );
}
