export const ALLOWED_BUCKET_MINUTES = new Set([5, 15, 30, 60, 240, 1440]);
export const DEFAULT_BUCKET_MINUTES = 60;
export const DEFAULT_HOURS = 24;
export const MIN_HOURS = 1;
export const MAX_HOURS = 168;

export function clampHours(input: unknown): number {
  const n = Number(input ?? DEFAULT_HOURS);
  if (!Number.isFinite(n)) return DEFAULT_HOURS;
  return Math.max(MIN_HOURS, Math.min(MAX_HOURS, Math.floor(n)));
}

export function clampBucketMinutes(input: unknown): number {
  const n = Number(input ?? DEFAULT_BUCKET_MINUTES);
  if (!Number.isFinite(n)) return DEFAULT_BUCKET_MINUTES;
  return ALLOWED_BUCKET_MINUTES.has(n) ? n : DEFAULT_BUCKET_MINUTES;
}

export interface TimeseriesRawRow {
  bucket_start: Date | string;
  attempts: number;
  included: number;
  reverted: number;
  avg_pnl_included_usd: number | null;
}

export interface TimeseriesPoint {
  bucket_start: string;
  attempts: number;
  included: number;
  reverted: number;
  avg_pnl_included_usd: number | null;
  revert_rate: number | null;
}

export function rowToPoint(r: TimeseriesRawRow): TimeseriesPoint {
  return {
    bucket_start: r.bucket_start instanceof Date ? r.bucket_start.toISOString() : String(r.bucket_start),
    attempts: r.attempts,
    included: r.included,
    reverted: r.reverted,
    avg_pnl_included_usd: r.avg_pnl_included_usd,
    revert_rate: r.attempts > 0 ? r.reverted / r.attempts : null,
  };
}
