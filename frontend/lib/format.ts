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
 * "zero"    — real zero value (e.g. $0.00 yield).
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
 * Half a cent — the smallest non-zero magnitude that 2 decimals can express, and
 * therefore the boundary below which a NONZERO value would render as a false
 * `0.00` (CARDS-FALSEZERO-01). Exported so the summary grid's and the card's
 * local `usd()` apply the same boundary as `formatProfitUSD`.
 */
export const SUB_CENT_USD = 0.005;

/**
 * CARDS-FALSEZERO-01 (2026-09-27) — render a NONZERO magnitude below the
 * half-cent resolution of the USD cells without collapsing it onto `0.00`.
 *
 * `(1.2e-5).toFixed(2) === "0.00"` is a fabricated ZERO for a figure the kernel
 * computed: the mirror of R8's `None ≠ Some(0)` (`Some(-1.2e-5) ≠ Some(0)`).
 * Trailing zeros are trimmed so the digits match the wire (`0.000005`, not
 * `0.0000050`); a magnitude that leaves fixed notation keeps engineering
 * notation (`2.4e-7`) rather than inventing a new unit suffix.
 *
 * Shared by `formatProfitUSD` and by BOTH card surfaces' local `usd()` (summary
 * grid + ladder), so one wire value never renders three ways.
 */
export function formatSubCentUsd(magnitude: number): string {
  const precise = magnitude.toPrecision(2);
  if (precise.includes("e")) return precise;
  return precise.replace(/0+$/, "").replace(/\.$/, "");
}

/**
 * Formats a USD yield/loss value for display in opportunity table cells.
 *
 * R8: null/undefined/NaN → DASH + pending tone.
 * 0 is a real value (zero yield) → "$0.00" + zero tone (plan §7.3).
 *
 * CARDS-FALSEZERO-01 (2026-09-27): a NONZERO figure below this cell's
 * 2-decimal resolution must NOT be painted as `$0.00` / `-$0.00`. That asserts
 * `Some(0)` for a value the kernel DID compute — the mirror image of the R8
 * rule (`None ≠ Some(0)`): here `Some(-1.2e-5) ≠ Some(0)`. Measured on the live
 * feed (`GET /api/opportunities/live`, 323 unique rows over 45 polls,
 * 2026-09-27T02:2xZ): 26 rows carried `net_expected_profit_usd` between
 * -1.2e-5 and -1.4e-5 and EVERY cell that displayed it painted `-$0.00`, i.e.
 * the operator read "exactly zero" where the sizing kernel had computed a
 * loss. Sub-cent magnitudes keep significant digits (`-$0.000012`) instead of
 * collapsing to a false zero; the sign stays BEFORE the `$` so one value never
 * renders two ways (the card's own `usd()` does the same).
 */
export function formatProfitUSD(value: number | null | undefined): FormattedValue {
  if (value == null || Number.isNaN(value)) return PENDING;
  if (value === 0) return { display: "$0.00", tone: "zero" };
  const magnitude = Math.abs(value);
  if (magnitude < SUB_CENT_USD) {
    const small = `$${formatSubCentUsd(magnitude)}`;
    return value > 0
      ? { display: `+${small}`, tone: "positive" }
      : { display: `-${small}`, tone: "negative" };
  }
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

/**
 * CARDS-DEDUP-HOPS: ages for the card's dual vigency line — "time since the
 * FIRST detection" (1ª) and "time since the LAST vigency ratification" (✓),
 * in DISCRETE units (s < 60, m < 60, h < 24, else d) so the text does not
 * churn continuously (anti-saturation convention, operator order 2026-09-20).
 *
 * R8: null inputs → null ages (sin fecha), never a fabricated 0s. When the
 * row carries no aggregates (plain WS detection) both ages fall back to
 * detected_at and `confirmed` is false — the card then renders the legacy
 * single-age view.
 * Pure: `now` is injected (no Date.now() here — SSR-safe).
 */
export interface VigencyAges {
  firstAge: string | null;
  lastAge: string | null;
  confirmed: boolean;
}

function discreteAge(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

export function formatVigency(
  firstSeenAt: string | null,
  lastSeenAt: string | null,
  detectedAt: string | null,
  now: number,
): VigencyAges {
  const parse = (v: string | null) => (v == null ? NaN : Date.parse(v));
  const firstTs = firstSeenAt ?? detectedAt;
  const lastTs = lastSeenAt ?? detectedAt;
  const f = parse(firstTs);
  const l = parse(lastTs);
  return {
    firstAge: Number.isNaN(f) ? null : discreteAge(now - f),
    lastAge: Number.isNaN(l) ? null : discreteAge(now - l),
    confirmed: firstSeenAt != null,
  };
}
