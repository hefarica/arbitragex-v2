/**
 * /opportunities/by-strategy — Convergence Signals grouped by strategy.
 *
 * Consumes the same /api/opportunities/live endpoint as the parent page,
 * but groups and summarises signals by strategy_kind. Observe-only.
 *
 * R1 Mounted-Snapshot Pattern: SSR snapshot → client live-polling.
 * Fail-honest: failed fetch → empty snapshot, never fabricated data.
 */
import { getApiBaseUrl } from "@/lib/api-client";
import { PageHeader } from "@/components/page-header";
import { OpportunitiesByStrategyClient } from "@/features/opportunities/OpportunitiesByStrategyClient";
import type { OmniOpportunity } from "@/lib/store/types";
import { mapToOmniOpportunity } from "@/lib/store/types";

export const metadata = {
  title: "By Strategy — ArbitrageX",
};

export const dynamic = "force-dynamic";
export const revalidate = 0;

async function getInitialOpportunities(): Promise<OmniOpportunity[]> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    const res = await fetch(`${EDGE_URL}/api/opportunities/live`, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!res.ok) return [];
    const data = await res.json();
    const raw: unknown[] = Array.isArray(data?.items) ? data.items : Array.isArray(data) ? data : [];
    return raw.map(r => mapToOmniOpportunity(r as Record<string, unknown>));
  } catch {
    return [];
  }
}

export default async function OpportunitiesByStrategyPage() {
  const initialOpportunities = await getInitialOpportunities();

  return (
    <>
      <PageHeader
        title="By Strategy"
        lede="Convergence signals grouped by strategy × canonical registry (§48) — a projection of the Exchange Feed (§49): same wire, same mapper, no second universe."
        showRefresh
      />
      <OpportunitiesByStrategyClient initialOpportunities={initialOpportunities} />
    </>
  );
}
