/**
 * A-02 — Pure outlier guard for the paper-trade archiver.
 *
 * Extracted into its own module (no ioredis/pg deps) so it is unit-testable in
 * isolation. Some unsized emit paths (Rhai `profit_usd_hint`, backrun/cex_dex
 * placeholders) can carry token-as-USD magnitudes ($49–59M on a $1k capital);
 * this guard quarantines implausible sim profits before they contaminate the
 * paper-history average.
 */

/** Default outlier threshold multiplier + capital floor. */
export const DEFAULT_OUTLIER_MULT = 10;
export const DEFAULT_CAPITAL_FLOOR_USD = 1000;

/**
 * Returns true when `simProfitUsd` is implausible (> mult × capital).
 * capitalUsd <= 0 → use the floor. Non-finite → outlier.
 */
export function isOutlierProfit(
  simProfitUsd: number,
  capitalUsd: number,
  mult = DEFAULT_OUTLIER_MULT,
  floor = DEFAULT_CAPITAL_FLOOR_USD,
): boolean {
  if (!Number.isFinite(simProfitUsd)) return true;
  const cap = capitalUsd > 0 ? capitalUsd : floor;
  return Math.abs(simProfitUsd) > mult * cap;
}
