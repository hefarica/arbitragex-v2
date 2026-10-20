// frontend/features/route-discovery/unavailable-copy.ts
//
// R8 (fail-honest) for the two outcomes panels' STALE alert.
//
// Measured defect (2026-10-19, operator screenshot): both panels rendered
// "api-server returns 503 when Postgres is unreachable" — for EVERY 503. That
// is false for the two reasons the route actually emits in production:
//   - `rollup_backfilling`: the 5-minute rollup is catching up and the route
//     refuses to serve an undercounted window (self-healing, not an outage);
//   - `query_failed`: an aggregation blew its 15s per-statement budget.
// A panel that names the wrong cause sends the operator to the wrong system.
// This maps the VERBATIM upstream reason (already surfaced by the hook) to the
// observed meaning, and uses the route's own recovery detail when it is there.
//
// The interval text is a constant, not prose: RDO-503-REASON-UI (2026-09-17)
// fixed it to 60s in the Gate-C panel while the analytics panel kept saying
// "every 8s" — stale since RDO-SUMMARY-HANG raised POLL_MS.

/** Recovery detail the route carries on a `rollup_backfilling` 503. */
export type { OutcomesUnavailableDetail } from "@/lib/hooks/useRouteDiscoveryOutcomes";
import type { OutcomesUnavailableDetail } from "@/lib/hooks/useRouteDiscoveryOutcomes";

/** Poll cadence of useRouteDiscoveryOutcomes (POLL_MS). One source, two panels. */
export const OUTCOMES_POLL_SECONDS = 60;

/**
 * Reason -> what was actually observed. Never invents a cause; an unknown
 * reason is quoted verbatim rather than explained away.
 */
export function outcomesUnavailableCopy(
  reason: string | null,
  detail?: OutcomesUnavailableDetail | null,
): string {
  switch (reason) {
    case "db_unavailable":
      return "api-server has no Postgres pool — the series cannot be read until the database is reachable.";
    case "rollup_backfilling": {
      const n = detail?.missing_buckets;
      const retry = detail?.retry_after_s;
      const gap =
        typeof n === "number" ? ` (${n} bucket${n === 1 ? "" : "s"} still to fill)` : "";
      const when = typeof retry === "number" ? ` It retries in ${retry}s.` : "";
      return `The 5-minute rollup is catching up${gap}; the route refuses to serve an undercounted window.${when}`;
    }
    case "query_failed":
      return "The aggregation blew its 15s per-statement budget (query_failed). A trimmed series would be a lie, so none is served.";
    case "edge_unreachable":
      return "The edge did not answer — api-server was never asked.";
    default:
      return reason
        ? `The outcomes summary could not be read (${reason}).`
        : "The outcomes summary could not be read.";
  }
}
