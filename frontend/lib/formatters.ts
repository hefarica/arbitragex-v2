const DASH = "—";

export function fmtMoney(n: number | null | undefined): string {
  return n == null ? DASH : `$${n.toFixed(2)}`;
}

export function fmtPct01(n: number | null | undefined): string {
  return n == null ? DASH : `${(n * 100).toFixed(2)}%`;
}

export function fmtPct100(n: number | null | undefined): string {
  return n == null ? DASH : `${n.toFixed(2)}%`;
}

export function fmtMs(n: number | null | undefined): string {
  return n == null ? DASH : `${n.toFixed(0)} ms`;
}

export function fmtTime(iso: string | null | undefined): string {
  if (!iso) return DASH;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? DASH : d.toLocaleTimeString();
}

export function fmtDateTime(iso: string | null | undefined): string {
  if (!iso) return DASH;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? DASH : d.toLocaleString();
}

export function fmtAge(iso: string | null | undefined, nowMs: number = Date.now()): string {
  if (!iso) return DASH;
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return DASH;
  const deltaSec = Math.max(0, Math.floor((nowMs - t) / 1000));
  if (deltaSec < 60) return `${deltaSec}s ago`;
  if (deltaSec < 3600) return `${Math.floor(deltaSec / 60)}m ago`;
  if (deltaSec < 86400) return `${Math.floor(deltaSec / 3600)}h ago`;
  return `${Math.floor(deltaSec / 86400)}d ago`;
}
