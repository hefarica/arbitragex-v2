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

// R1/hydration: time formatters MUST be deterministic — `toLocaleTimeString()`
// with no args renders differently on the server (Node, UTC/en) vs the browser
// (user locale/tz), producing React #425/#422 hydration failures (observed
// 2026-08-16 with es-ES/America/Bogota on /paper/history). Render explicit UTC
// via the getUTC* getters (no Intl dependency, byte-stable in every runtime).
// A trading ledger in UTC is also the honest display: it matches block
// timestamps and any by-hour analytics.
const p2 = (n: number) => String(n).padStart(2, "0");

export function fmtTime(iso: string | null | undefined): string {
  if (!iso) return DASH;
  const d = new Date(iso);
  return Number.isNaN(d.getTime())
    ? DASH
    : `${p2(d.getUTCHours())}:${p2(d.getUTCMinutes())}:${p2(d.getUTCSeconds())} UTC`;
}

export function fmtDateTime(iso: string | null | undefined): string {
  if (!iso) return DASH;
  const d = new Date(iso);
  return Number.isNaN(d.getTime())
    ? DASH
    : `${d.getUTCFullYear()}-${p2(d.getUTCMonth() + 1)}-${p2(d.getUTCDate())} ${p2(
        d.getUTCHours(),
      )}:${p2(d.getUTCMinutes())}:${p2(d.getUTCSeconds())} UTC`;
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
