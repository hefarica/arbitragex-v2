import OpportunitiesClient, { type OpportunitiesSnapshot } from "./OpportunitiesClient";
import { getApiBaseUrl } from "@/lib/api-client";
// FE-0051 (§76): map at the Server Component — same wire, same mapper as the
// WS/polling paths (by-strategy precedent). The raw SSR rows used to bypass
// mapToOmniOpportunity, so semantic_violations was undefined and
// QuarantineStrip crashed the card on first paint.
import { mapToOmniOpportunity } from "@/lib/store/types";

export const dynamic = "force-dynamic";

async function getInitialOpportunities(): Promise<OpportunitiesSnapshot> {
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
    const raw: unknown[] = Array.isArray(data?.items)
      ? data.items
      : Array.isArray(data)
      ? data
      : [];
    return {
      opportunities: raw.map(r => mapToOmniOpportunity(r as Record<string, unknown>)),
      serverTime: new Date().toISOString(),
      source: "server-snapshot",
    };
  } catch (e) {
    return {
      opportunities: [],
      serverTime: null,
      source: "server-fetch-failed",
    };
  }
}

export default async function OpportunitiesPage() {
  const initialSnapshot = await getInitialOpportunities();

  return (
    <div className="min-h-screen">
      <OpportunitiesClient initialSnapshot={initialSnapshot} />
    </div>
  );
}
