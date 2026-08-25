import OpportunitiesExchangeClient, {
  type OpportunitiesSnapshot,
} from "./OpportunitiesExchangeClient";
import { getApiBaseUrl } from "@/lib/api-client";
// FE-0051 (§76): map at the Server Component — same wire, same mapper as the
// WS/polling paths (by-strategy precedent). The raw SSR rows used to bypass
// mapToOmniOpportunity, so semantic_violations was undefined and the card
// subtree crashed on first paint.
import { mapToOmniOpportunity } from "@/lib/store/types";
// SSOT "glass neon" design language (verbatim port of docs/atlas_264.html),
// scoped under .atlas-scope — only this page loads it.
import "./atlas-glass.css";

export const dynamic = "force-dynamic";

async function getInitialOpportunities(): Promise<OpportunitiesSnapshot> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    // viable_only=false so the SSR snapshot includes ALL recent detections
    // (rejected + viable), giving the operator a non-empty first paint instead
    // of a blank grid while the live feed warms up. R8: rows are real PG rows,
    // never fabricated.
    const res = await fetch(
      `${EDGE_URL}/api/opportunities/live?viable_only=false&limit=50`,
      {
        cache: "no-store",
      },
    );

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
  } catch {
    return {
      opportunities: [],
      serverTime: null,
      source: "server-fetch-failed",
    };
  }
}

export default async function OpportunitiesExchangePage() {
  const initialSnapshot = await getInitialOpportunities();

  return (
    <div className="min-h-screen">
      <OpportunitiesExchangeClient initialSnapshot={initialSnapshot} />
    </div>
  );
}
