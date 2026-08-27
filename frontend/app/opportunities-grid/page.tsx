/**
 * =============================================================================
 * OMEGA Opportunity Grid — Server Entry (Next.js App Router)
 * =============================================================================
 *
 * Alternate visual surface to /opportunities, inspired by the operational
 * anatomy of DeFiBot.trade's terminal (card-grid layout with quick presets,
 * AI agent status banner and a 4-card stats strip) — adapted **without
 * changing our TradePro theme**:
 *   - We keep OKLCH electric-royal-blue primary (no cyan).
 *   - We keep shadcn/ui primitives, Tailwind v4 tokens and Geist fonts.
 *   - We replicate ONLY layout/anatomy, never the source's exact colours,
 *     wallets, opcodes or any code paths.
 *
 * Doctrine markers:
 *   - R1 Mounted Snapshot: no Date.now/Math.random/window in render.
 *     Server-side fetch is honest (HTTP cache: 'no-store'); client hydrates
 *     from the same Omni-Store as /opportunities.
 *   - R8 Fail-Honest: never fabricate zeros. Empty list → empty state;
 *     stats with no telemetry → "—".
 *
 * Mapping to OMEGA_BENCHMARK_30_E2E.xlsx features:
 *   - F-51 Strategy Router (visible chips per card)
 *   - F-54 Operational Presets (Conservative/Balanced/Aggressive)
 *   - F-71 Public verifiable stats (TerminalStatsBar — null until executions)
 */

import OpportunitiesGridClient, {
  type OpportunitiesGridSnapshot,
} from "./OpportunitiesGridClient";
import { getApiBaseUrl } from "@/lib/api-client";

export const dynamic = "force-dynamic";

async function getInitialOpportunities(): Promise<OpportunitiesGridSnapshot> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    const res = await fetch(`${EDGE_URL}/api/opportunities/live`, {
      cache: "no-store",
    });

    if (!res.ok) {
      return {
        opportunities: [],
        serverTime: null,
        source: "server-fetch-failed",
      };
    }

    const data = await res.json();
    return {
      opportunities: Array.isArray(data?.items)
        ? data.items
        : Array.isArray(data)
        ? data
        : [],
      serverTime: new Date().toISOString(),
      source: "server-snapshot",
    };
  } catch {
    // R8 fail-honest: surface no data instead of fabricating empty array
    // disguised as "success". `source` marks the failure.
    return {
      opportunities: [],
      serverTime: null,
      source: "server-fetch-failed",
    };
  }
}

export default async function OpportunitiesGridPage() {
  const initialSnapshot = await getInitialOpportunities();
  return <OpportunitiesGridClient initialSnapshot={initialSnapshot} />;
}
