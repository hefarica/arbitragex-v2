/**
 * Edge health taxonomy — strict distinction between:
 *
 *   EDGE_UNREACHABLE
 *     The frontend cannot reach its own edge/backend gateway.
 *     Causes: network error, DNS failure, connection refused, total
 *     fetch timeout, abort, non-interpretable HTTP response.
 *     UI must call this out as a fault of the platform itself.
 *
 *   EDGE_REACHABLE_UPSTREAM_DEGRADED
 *     The edge/backend responded (we got an HTTP status code back),
 *     but one or more dependencies (RPC, providers, indexers,
 *     databases, external APIs) are unhealthy, not configured, or
 *     returned 5xx. The platform itself is alive — its upstreams
 *     are not. UI must call this "degraded", not "unreachable".
 *
 *   EDGE_REACHABLE_SCHEMA_DRIFT
 *     The edge responded with a body that does not match our Zod
 *     contract. Different from upstream degradation: this is a
 *     protocol-level mismatch and must be surfaced as such.
 *
 *   EDGE_REACHABLE_LIVE
 *     Edge responded with valid data.
 *
 * Doctrine: "POR EL HAZ DE LUZ SOLO PASA QUIEN ES VISTO".
 * The frontend MUST NOT mislabel an upstream 503 as "edge unreachable".
 * That conflation hides the real root cause from the operator.
 *
 * Mapping from api-client.ts Result<T>.error strings:
 *   "edge HTTP 5xx..."                  → UPSTREAM_DEGRADED
 *   "edge HTTP 4xx..."                  → UPSTREAM_DEGRADED (auth / shape)
 *   "edge timeout after Nms"            → UNREACHABLE
 *   "edge returned invalid JSON: ..."   → SCHEMA_DRIFT
 *   "edge response shape invalid: ..."  → SCHEMA_DRIFT
 *   anything else (fetch threw)         → UNREACHABLE
 */

export type EdgeHealthKind =
  | "EDGE_REACHABLE_LIVE"
  | "EDGE_REACHABLE_UPSTREAM_DEGRADED"
  | "EDGE_REACHABLE_SCHEMA_DRIFT"
  | "EDGE_UNREACHABLE";

export type EdgeResult<T> = { ok: true; data: T } | { ok: false; error: string };

export type EdgeHealth = {
  kind: EdgeHealthKind;
  /** Short human label for AlertTitle. NEVER contains "edge unreachable" unless kind === EDGE_UNREACHABLE. */
  title: string;
  /** Verbatim diagnostic preserved from the api-client result. Empty on LIVE. */
  diagnostic: string;
};

const EDGE_HTTP_RE = /^edge HTTP (\d{3})/i;
const EDGE_TIMEOUT_RE = /^edge timeout/i;
const EDGE_INVALID_JSON_RE = /^edge returned invalid JSON/i;
const EDGE_SHAPE_INVALID_RE = /^edge response shape invalid/i;

/**
 * Classify a single Result<T> against the edge health taxonomy.
 * Pure function; safe to use in Server Components and tests.
 */
export function classifyEdgeResult<T>(res: EdgeResult<T>): EdgeHealth {
  if (res.ok) {
    return { kind: "EDGE_REACHABLE_LIVE", title: "live", diagnostic: "" };
  }

  const err = res.error || "";

  // HTTP status received → edge IS reachable.
  const httpMatch = err.match(EDGE_HTTP_RE);
  if (httpMatch) {
    return {
      kind: "EDGE_REACHABLE_UPSTREAM_DEGRADED",
      title: "upstreams degraded",
      diagnostic: err,
    };
  }

  if (EDGE_INVALID_JSON_RE.test(err) || EDGE_SHAPE_INVALID_RE.test(err)) {
    return {
      kind: "EDGE_REACHABLE_SCHEMA_DRIFT",
      title: "edge schema drift",
      diagnostic: err,
    };
  }

  if (EDGE_TIMEOUT_RE.test(err)) {
    return {
      kind: "EDGE_UNREACHABLE",
      title: "edge unreachable",
      diagnostic: err,
    };
  }

  // Anything else means the underlying fetch threw before we ever saw
  // a status line — DNS, ECONNREFUSED, abort, TLS, etc. That IS the
  // edge being unreachable from the frontend's perspective.
  return {
    kind: "EDGE_UNREACHABLE",
    title: "edge unreachable",
    diagnostic: err,
  };
}

/**
 * Combine multiple results into the WORST observed health.
 *
 * Severity ordering (worst first):
 *   EDGE_UNREACHABLE > EDGE_REACHABLE_SCHEMA_DRIFT >
 *   EDGE_REACHABLE_UPSTREAM_DEGRADED > EDGE_REACHABLE_LIVE
 *
 * If ANY result is UNREACHABLE the aggregate is UNREACHABLE.
 * If ALL results are LIVE the aggregate is LIVE.
 * Otherwise the aggregate reflects the worst non-unreachable signal.
 */
export function aggregateEdgeHealth(
  results: ReadonlyArray<EdgeResult<unknown>>,
): EdgeHealth {
  if (results.length === 0) {
    return { kind: "EDGE_REACHABLE_LIVE", title: "live", diagnostic: "" };
  }

  const classified = results.map((r) => classifyEdgeResult(r));
  const order: EdgeHealthKind[] = [
    "EDGE_UNREACHABLE",
    "EDGE_REACHABLE_SCHEMA_DRIFT",
    "EDGE_REACHABLE_UPSTREAM_DEGRADED",
    "EDGE_REACHABLE_LIVE",
  ];

  for (const kind of order) {
    const hit = classified.find((c) => c.kind === kind);
    if (hit) return hit;
  }
  // Unreachable in practice; preserve type-safety.
  return classified[0]!;
}
