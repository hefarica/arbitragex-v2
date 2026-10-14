import OpportunitiesClient, { type OpportunitiesSnapshot } from "./OpportunitiesClient";
import { getApiBaseUrl } from "@/lib/api-client";
// FE-0051 (§76): map at the Server Component — same wire, same mapper as the
// WS/polling paths (by-strategy precedent). The raw SSR rows used to bypass
// mapToOmniOpportunity, so semantic_violations was undefined and
// QuarantineStrip crashed the card on first paint.
import { mapToOmniOpportunity } from "@/lib/store/types";

export const dynamic = "force-dynamic";

async function getInitialOpportunities(
  windowSeconds: number,
  routeRepresentative: "latest" | "best_net",
): Promise<OpportunitiesSnapshot> {
  const EDGE_URL = process.env.INTERNAL_EDGE_URL || getApiBaseUrl();
  try {
    // WINDOW-01: the lookback rides the snapshot (api-server clamps it to
    // [10 s, 86400 s]). Default 300 s = previous behaviour.
    // ROUTE-REP-01: the representative selector rides it too, so the first paint
    // and the 5-second reconcile agree on which row represents a route.
    const res = await fetch(
      `${EDGE_URL}/api/opportunities/live?order=profit_usd&max_age_seconds=${windowSeconds}` +
        `&route_representative=${routeRepresentative}`,
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
   * Absent/other value ⇒ OFF, i.e. the gate-D scope (viable + rejected with
   * computed net > 0).
   *
   * WINDOW-01: `?window_seconds=3600` deep-links the live lookback.
   */
  searchParams?: {
    show_rejected?: string | string[];
    window_seconds?: string | string[];
    route_rep?: string | string[];
  };
}) {
  const first = (v: string | string[] | undefined) => (Array.isArray(v) ? v[0] : v);
  const showRejected = first(searchParams?.show_rejected) === "1";
  // ROUTE-REP-01: by default the grid shows a route's COMPUTED GAIN — the row
  // the server picks is the best computed net in the window, not whichever
  // re-detection happened last (measured: +$0.1198 at 11:51 buried by −$12.93
  // at 11:58 on the same route ⇒ 0 cards on screen). `?route_rep=latest`
  // restores the previous semantics exactly.
  const routeRepresentative: "latest" | "best_net" =
    first(searchParams?.route_rep) === "latest" ? "latest" : "best_net";
  const requested = Number(first(searchParams?.window_seconds) ?? 300);
  const windowSeconds = Number.isFinite(requested)
    ? Math.max(10, Math.min(86_400, Math.trunc(requested)))
    : 300;

  const initialSnapshot = await getInitialOpportunities(windowSeconds, routeRepresentative);

  return (
    <div className="min-h-screen">
      <OpportunitiesClient
        initialSnapshot={initialSnapshot}
        initialShowRejected={showRejected}
        initialWindowSeconds={windowSeconds}
        initialRouteRep={routeRepresentative}
      />
    </div>
  );
}
