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
    const res = await fetch(`${EDGE_URL}/api/opportunities/live?order=profit_usd`, {
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

export default async function OpportunitiesPage({
  searchParams,
}: {
  /**
   * SHOW-REJECTED-01 (operator order 2026-09-27): `?show_rejected=1` deep-links
   * the "Mostrar rechazadas" toggle in its ON state. Read on the SERVER and
   * handed to the client as its initial state, so the first paint is identical
   * on both sides (R1) and the ON rendering is verifiable without a click.
   * Absent/other value ⇒ OFF, i.e. the real/live card set only.
   */
  searchParams?: { show_rejected?: string | string[] };
}) {
  const initialSnapshot = await getInitialOpportunities();
  const showRejected =
    (Array.isArray(searchParams?.show_rejected)
      ? searchParams?.show_rejected[0]
      : searchParams?.show_rejected) === "1";

  return (
    <div className="min-h-screen">
      <OpportunitiesClient
        initialSnapshot={initialSnapshot}
        initialShowRejected={showRejected}
      />
    </div>
  );
}
