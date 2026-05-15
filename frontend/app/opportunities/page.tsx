import OpportunitiesClient, { type OpportunitiesSnapshot } from "./OpportunitiesClient";
import { getValidated } from "@/lib/frontier";
import { OpportunitiesLiveSchema } from "@/lib/schemas";

export const dynamic = "force-dynamic";

// OMEGA-8 / M5 Fase 8 (P1-OP-1): SSR snapshot is now Zod-parsed via the
// shared frontier client. Invalid shape no longer hands the client unknown
// items; instead the snapshot.source carries the precise reason
// (server-fetch-failed | server-snapshot-invalid | server-snapshot-unavailable).
// Fase 10 (P1-OP-2): `serverTime` is sourced from the backend payload (`ts`),
// not regenerated on the server — preventing the client from comparing an
// SSR-locked time against a client-generated one and triggering R1 mismatch.
async function getInitialOpportunities(): Promise<OpportunitiesSnapshot> {
  const r = await getValidated("/api/opportunities/live", OpportunitiesLiveSchema);
  if (r.kind === "ok") {
    // The Zod schema validates the envelope + minimum row fields; the page's
    // `OpportunityListItem` carries extra optional fields (token_info, sim
    // breakdowns) that the schema treats as unknown. The runtime payload is
    // the same object — Zod doesn't strip; we narrow the static type here.
    return {
      opportunities: r.data.items as unknown as OpportunitiesSnapshot["opportunities"],
      serverTime: r.data.ts,
      source: "server-snapshot",
    };
  }
  if (r.kind === "invalid_response") {
    return {
      opportunities: [],
      serverTime: null,
      source: "server-snapshot-invalid",
    };
  }
  return {
    opportunities: [],
    serverTime: null,
    source: "server-fetch-failed",
  };
}

export default async function OpportunitiesPage() {
  const initialSnapshot = await getInitialOpportunities();
  return <OpportunitiesClient initialSnapshot={initialSnapshot} />;
}
