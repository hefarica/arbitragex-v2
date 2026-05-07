/**
 * format.ts — Rich formatters returning { display, tone } for color-coded table cells.
 *
 * R8 fail-honest semantics:
 *   null | undefined | NaN → { display: "—", tone: "pending" }  (no data, not a value)
 *   0                      → { display: "$0.00", tone: "zero" }  (real zero, not pending)
 *   positive               → { display: "+$X.XX", tone: "positive" }
 *   negative               → { display: "-$X.XX", tone: "negative" }
 *
 * RULE: Do NOT import Date, Math.random, window, document, or any non-deterministic API here.
 * This module is used in Server Components (SSR) and must be pure.
 */

const DASH = "—";

/**
 * "zero"    — real zero value (e.g. $0.00 profit).
 * "neutral" — non-zero value that carries no directional sentiment (e.g. spread ≤ 0 bps).
 * "pending" — no data available (null/undefined/NaN per R8 fail-honest).
 */
export type Tone = "pending" | "zero" | "positive" | "negative" | "neutral";

export interface FormattedValue {
  display: string;
  tone: Tone;
}

const PENDING: FormattedValue = { display: DASH, tone: "pending" };

/**
 * Formats a USD profit/loss value for display in opportunity table cells.
 *
 * R8: null/undefined/NaN → DASH + pending tone.
 * 0 is a real value (zero profit) → "$0.00" + zero tone (plan §7.3).
 */
export function formatProfitUSD(value: number | null | undefined): FormattedValue {
  if (value == null || Number.isNaN(value)) return PENDING;
  if (value === 0) return { display: "$0.00", tone: "zero" };
  if (value > 0) return { display: `+$${value.toFixed(2)}`, tone: "positive" };
  return { display: `-$${Math.abs(value).toFixed(2)}`, tone: "negative" };
}

/**
 * Formats a spread value in basis points.
 * Positive bps = opportunity exists → positive tone.
 * Zero or negative → neutral tone (no directional edge).
 * null/undefined/NaN → pending tone per R8.
 *
 * NOTE: This helper is an extension beyond plan §7.3 (which specifies only
 * formatProfitUSD, formatPctOrDash, formatRiskOrDash, shortAddr).
 * Retained because it is used by existing tests and provides spread display utility.
 */
export function formatSpreadBps(bps: number | null | undefined): FormattedValue {
  if (bps == null || Number.isNaN(bps)) return PENDING;
  if (bps <= 0) return { display: `${bps.toFixed(1)}bps`, tone: "neutral" };
  return { display: `${bps.toFixed(1)}bps`, tone: "positive" };
}

/**
 * Formats a percentage value (0–100 scale, e.g. 2.5 = 2.5%).
 * R8: null/undefined/NaN → DASH (no data).
 * 0 → "0.00%" (real zero, not pending).
 *
 * @param value       - Percentage on 0–100 scale.
 * @param fractionDigits - Decimal places. Default 2.
 */
export function formatPctOrDash(
  value: number | null | undefined,
  fractionDigits = 2,
): string {
  if (value == null || Number.isNaN(value)) return DASH;
  return `${value.toFixed(fractionDigits)}%`;
}

/**
 * Formats a risk score (0–1 scale, e.g. 0.95 → "95.0%").
 * R8: null/undefined/NaN → DASH (no data).
 * 0 → "0.0%" (real zero, not pending).
 */
export function formatRiskOrDash(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return DASH;
  return `${(value * 100).toFixed(1)}%`;
}

/**
 * Shortens an address/hex string to "0x1234…5678" format.
 * Safe for seeds shorter than 10 chars: falls back to the full string.
 * Pure — no window/document access.
 */
export function shortAddr(addr: string): string {
  if (addr.length <= 10) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
}
