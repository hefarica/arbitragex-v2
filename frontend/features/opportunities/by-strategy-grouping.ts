/**
 * FE-MASTER · By-strategy grouping model (FE-0039 — §48).
 *
 * Pure, framework-free: derives the by-strategy view FROM the exchange feed
 * rows joined with the canonical registry — "sin universo propio" means the
 * strategy axis IS the registry (264 workbook strategies), never a second
 * hardcoded list of families. The feed carries two identity fields
 * (FE-0028/FE-0029): cartridge_id (MEV-xx-xxx, the registry key) and
 * strategy_kind (5-token family). Grouping prefers the registry key; a row
 * with NEITHER lands in the honest `unknown` bucket — never a "dex_arb"
 * default (the exact semantic default FE-0029 removed from the mapper).
 *
 * Join results are disclosed, not inferred: a cartridge_id with no registry
 * match renders as drift, not as silence; registry rows with zero feed
 * signals are NOT fabricated (the view groups what the feed carries).
 */

import type { OmniOpportunity } from "@/lib/store/types";
import type { StrategyCatalogRow } from "@/lib/apex/schemas";

/** MEV-xx-xxx workbook shape — the registry's own key format (INV-2). */
export const MEV_ID_PATTERN = /^MEV-\d{2,3}-\d{3}$/;

export interface StrategyGroup {
  /** cartridge_id | strategy_kind | "unknown" — the display/grouping key. */
  key: string;
  /** true = grouped by registry key (cartridge); false = kind fallback; unknown otherwise. */
  axis: "registry" | "kind" | "unknown";
  opps: OmniOpportunity[];
  /** Registry join — null = no match (drift for registry axis, expected for the others). */
  registry: StrategyCatalogRow | null;
}

export interface ByMevIdLike {
  get(id: string): StrategyCatalogRow | undefined;
}

export function groupByStrategy(
  opps: readonly OmniOpportunity[],
  byMevId: ByMevIdLike | null,
): StrategyGroup[] {
  const groups = new Map<string, StrategyGroup>();
  for (const opp of opps) {
    const cartridge = opp.cartridge_id;
    const key =
      cartridge !== null && cartridge !== ""
        ? cartridge
        : opp.strategy_kind !== null
          ? opp.strategy_kind
          : "unknown";
    let g = groups.get(key);
    if (!g) {
      g = {
        key,
        axis:
          cartridge !== null && cartridge !== ""
            ? "registry"
            : opp.strategy_kind !== null
              ? "kind"
              : "unknown",
        opps: [],
        registry: null,
      };
      groups.set(key, g);
    }
    g.opps.push(opp);
  }
  for (const g of groups.values()) {
    g.registry =
      g.axis === "registry" && byMevId ? byMevId.get(g.key) ?? null : null;
  }
  // Doctrine order: registry groups first (by signal count desc), then kind
  // fallbacks, and the unknown bucket ALWAYS last — never hidden, never first.
  const rank = (g: StrategyGroup) =>
    g.axis === "registry" ? 0 : g.axis === "kind" ? 1 : 2;
  return [...groups.values()].sort(
    (a, b) => rank(a) - rank(b) || b.opps.length - a.opps.length || a.key.localeCompare(b.key),
  );
}

/** Join coverage for the summary strip — honest matched/unmatched counts. */
export function joinCoverage(groups: readonly StrategyGroup[]): {
  matched: number;
  unmatched: number;
  unknown: number;
} {
  let matched = 0;
  let unmatched = 0;
  let unknown = 0;
  for (const g of groups) {
    if (g.axis === "registry") {
      if (g.registry) matched++;
      else unmatched++;
    } else if (g.axis === "unknown") {
      unknown++;
    }
  }
  return { matched, unmatched, unknown };
}
