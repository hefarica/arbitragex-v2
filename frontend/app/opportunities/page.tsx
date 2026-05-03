import OpportunitiesClient, { OpportunitiesSnapshot } from "./OpportunitiesClient";
import { getApiBaseUrl } from "@/lib/api-client";

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
    return {
      opportunities: Array.isArray(data?.items)
        ? data.items
        : Array.isArray(data)
        ? data
        : [],
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

  return <OpportunitiesClient initialSnapshot={initialSnapshot} />;
}
