/**
 * EDGE-AUDIT-BUCKET-01 (NR-0000, 2026-09-06) — pure per-IP rate-limit math.
 *
 * Extracted verbatim from index.ts's in-memory limiter so the NEW
 * classification rules live in a module the vitest suite can import without
 * booting the Express app (index.ts binds a port on import).
 *
 * Classification (all fail-closed on absent env — with neither token set the
 * behavior is byte-identical to the pre-2026-09-06 limiter):
 *   - "exempt"  — x-arbx-edge-token === ARBX_EDGE_TOKEN: internal SSR
 *     traffic (frontend Server Components via INTERNAL_EDGE_URL; the
 *     frontend already sends it — frontend/lib/api-client.ts). Parity with
 *     the Cloudflare Worker variant's SEC-1 exemption. dev-local (the
 *     variant that ACTUALLY runs in docker) was missing it, so every SSR
 *     fetch shared ONE public bucket keyed by the frontend container's IP —
 *     a page-sweep fan-out (≈16 polled endpoints per page) self-429'd the
 *     whole dashboard: the NR-0000 mechanism.
 *   - "audit"   — x-arbx-audit-token === EDGE_AUDIT_TOKEN (new optional
 *     env): the Holy Grail audit sweep gets its OWN bounded bucket
 *     (EDGE_AUDIT_RATE_LIMIT_PER_MIN, floor = public limit) on a separate
 *     keyspace — never an unbounded bypass.
 *   - "public"  — everything else: the pre-existing per-IP bucket.
 */

export type RateLimitClass = "public" | "audit" | "exempt";

export interface RateLimitDecision {
  klass: RateLimitClass;
  ok: boolean;
  /** null ⇒ exempt (no bucket consumed); the caller prints "exempt". */
  remaining: number | null;
}

interface BucketState {
  count: number;
  windowStart: number;
}

/** Accepts Express's `IncomingHttpHeaders` (index-signature type) directly. */
export type RateLimitHeaders = Record<string, unknown>;

export interface RateLimitSecrets {
  edgeToken: string;
  /** Empty string ⇒ audit classification impossible (fail-closed). */
  auditToken: string;
}

/**
 * Classify a request WITHOUT consuming any bucket. Token compare is
 * constant-shape (`typeof === "string"`), so a duplicated header
 * (string[]) never matches — duplicated secrets must not grant anything.
 */
export function classifyRateLimit(
  headers: RateLimitHeaders,
  secrets: RateLimitSecrets,
): RateLimitClass {
  const ssr = headers["x-arbx-edge-token"];
  if (secrets.edgeToken && typeof ssr === "string" && ssr === secrets.edgeToken) {
    return "exempt";
  }
  const audit = headers["x-arbx-audit-token"];
  if (secrets.auditToken && typeof audit === "string" && audit === secrets.auditToken) {
    return "audit";
  }
  return "public";
}

export interface RateLimiter {
  check(klass: RateLimitClass, key: string): RateLimitDecision;
}

/**
 * Two independent in-memory bucket stores (public / audit), same window
 * semantics as the original inline `hit()`: fixed 60s window keyed by IP,
 * counter resets when the window elapses. Non-atomic by design — same
 * trade-off the original made (worst case: one extra hit under burst).
 */
export function createRateLimiter(opts: {
  publicMax: number;
  auditMax: number;
  windowMs?: number;
  now?: () => number;
}): RateLimiter {
  const windowMs = opts.windowMs ?? 60_000;
  const now = opts.now ?? Date.now;
  const publicBuckets = new Map<string, BucketState>();
  const auditBuckets = new Map<string, BucketState>();

  function hitStore(
    store: Map<string, BucketState>,
    max: number,
    key: string,
  ): RateLimitDecision {
    const t = now();
    const cur = store.get(key);
    if (!cur || t - cur.windowStart > windowMs) {
      store.set(key, { count: 1, windowStart: t });
      return { klass: store === auditBuckets ? "audit" : "public", ok: true, remaining: max - 1 };
    }
    cur.count++;
    if (cur.count > max) {
      return { klass: store === auditBuckets ? "audit" : "public", ok: false, remaining: 0 };
    }
    return { klass: store === auditBuckets ? "audit" : "public", ok: true, remaining: max - cur.count };
  }

  return {
    check(klass: RateLimitClass, key: string): RateLimitDecision {
      if (klass === "exempt") return { klass, ok: true, remaining: null };
      if (klass === "audit") return hitStore(auditBuckets, opts.auditMax, key);
      return hitStore(publicBuckets, opts.publicMax, key);
    },
  };
}
