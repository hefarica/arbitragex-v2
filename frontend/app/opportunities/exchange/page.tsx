import OpportunitiesExchangeClient, {
  type OpportunitiesSnapshot,
} from "./OpportunitiesExchangeClient";
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
