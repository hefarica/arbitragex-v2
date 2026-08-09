/**
 * Strategy kind helpers — derive family/label from any strategy_kind string.
 *
 * The backend sends 269 possible values: 5 base families + 264 cartridge IDs
 * (e.g., "mev_01_001_dex_dex_arbitrage"). This module provides dynamic
 * labelling so StrategyBadge can render ALL 269, not just the base 5.
 */

/** The 5 base strategy families (mirrors shared-ts). */
export const BASE_STRATEGIES = [
  "dex_arb",
  "triangular",
  "backrun",
  "liquidation",
  "flashloan_arb",
] as const;

/**
 * Extract the family group from a strategy_kind string.
 * Base kinds → return as-is.
 * Cartridge kinds (mev_01_001_...) → return "MEV-01" family prefix.
 */
export function familyOf(kind: string): string {
  const match = kind.match(/^mev_(\d{2})_/);
  if (match) return `MEV-${match[1]}`;
  return kind;
}

/** Tailwind class for a given family (colour-coded by MEV group). */
const FAMILY_COLOURS: Record<string, string> = {
  "MEV-01": "bg-primary/10 text-primary border border-primary/30",
  "MEV-02": "bg-accent/10 text-accent-foreground border border-accent/30",
  "MEV-03": "bg-warning/10 text-warning border border-warning/30",
  "MEV-04": "bg-info/10 text-info border border-info/30",
  "MEV-05": "bg-success/10 text-success border border-success/30",
  "MEV-06": "bg-destructive/10 text-destructive border border-destructive/30",
  "MEV-07": "bg-purple-500/10 text-purple-400 border border-purple-500/30",
  "MEV-08": "bg-blue-500/10 text-blue-400 border border-blue-500/30",
  "MEV-09": "bg-pink-500/10 text-pink-400 border border-pink-500/30",
  "MEV-10": "bg-orange-500/10 text-orange-400 border border-orange-500/30",
  "MEV-11": "bg-teal-500/10 text-teal-400 border border-teal-500/30",
};

export function familyColour(kind: string): string {
  const fam = familyOf(kind);
  return FAMILY_COLOURS[fam] ?? "bg-muted/60 text-muted-foreground border border-border";
}

/** Check if a strategy_kind is one of the 5 base families. */
export function isBaseStrategy(kind: string): boolean {
  return (BASE_STRATEGIES as readonly string[]).includes(kind);
}
