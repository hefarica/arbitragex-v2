/**
 * G-SIM-1 PR-B2b Fase 3 (A2) — HTTP client for searcher-rs /route/:opp_id.
 *
 * When `route_source = "searcher_api"`, api-server calls this client to fetch
 * route metadata from searcher-rs (which has fresher in-memory data than the
 * persisted PG column for hot opportunities).
 *
 * R8 fail-honest: returns null on any error (not_found, network, timeout).
 * The caller falls back to the PG path (A1) or returns 503.
 */

const SEARCHER_BASE =
  process.env['SEARCHER_URL'] ??
  process.env['SEARCHER_RS_INTERNAL_URL'] ??
  'http://searcher-rs:9001';

const TIMEOUT_MS = 5_000;

/** Response shape from GET /route/:opp_id (mirrors searcher-rs RouteMetadataResponse). */
export interface SearcherRouteResponse {
  opportunity_id: string;
  populated: boolean;
  route_metadata: {
    pool_addresses?: string[];
    token_addresses?: string[];
    dex_adapters?: string[];
    decimals?: Record<string, number>;
  };
}

/**
 * Fetch route metadata from searcher-rs for a given opportunity.
 *
 * Returns:
 * - `{ populated: true, route_metadata: {...} }` when searcher-rs has topology
 * - `{ populated: false, route_metadata: {} }` when the row exists but has no route
 * - `null` on network error / timeout / not_found (caller falls back)
 */
export async function fetchRouteFromSearcher(
  opportunityId: string,
): Promise<SearcherRouteResponse | null> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);

  try {
    const resp = await fetch(`${SEARCHER_BASE}/route/${opportunityId}`, {
      method: 'GET',
      headers: { accept: 'application/json' },
      signal: ctrl.signal,
    });

    if (resp.status === 404) {
      // Opportunity not found in searcher-rs cache/DB — not an error, just
      // signals the caller to try A1 (PG) or A3 (sim-ctl lookup).
      return null;
    }

    if (!resp.ok) {
      // 500 or other — searcher-rs had an internal error. Fail-honest: null.
      return null;
    }

    const body = (await resp.json()) as SearcherRouteResponse;
    return body;
  } catch {
    // Network error / timeout / abort — searcher-rs unreachable.
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Check whether searcher-rs /route endpoint is reachable (health probe).
 * Used by the route_source selector to show A2 as available/disabled.
 */
export async function isSearcherRouteAvailable(): Promise<boolean> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 2_000);

  try {
    // The /route/:opp_id endpoint returns 404 for unknown IDs but that proves
    // it's mounted and reachable. Use a sentinel probe UUID.
    const resp = await fetch(`${SEARCHER_BASE}/route/00000000-0000-0000-0000-000000000000`, {
      method: 'GET',
      headers: { accept: 'application/json' },
      signal: ctrl.signal,
    });
    // 404 = endpoint exists, just no row. 200 = endpoint exists + row found.
    // Anything else (connection refused, timeout) = unavailable.
    return resp.status === 404 || resp.status === 200;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}
