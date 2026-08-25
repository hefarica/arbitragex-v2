/**
 * FE-MASTER EMIT-02 Layer-2 / EMIT-03 — quote-anchor read + preview surface.
 *
 * EMIT-02 `GET /api/quote/anchor`: serves the snapshot published by
 * searcher-rs `quote_anchor_runtime::write_quote_anchor_snapshot` (Redis
 * `arbx:quote:anchor:<chain>`, TTL 35s) FLATTENED to the 8-key contract of
 * `QuoteAnchorResponseSchema` — the two preview-only sidecars
 * (`pairs_by_symbol` / `pools_by_symbol`) NEVER ride this response (they are
 * preview-impact INPUTS resolved server-side, not display data).
 *
 * EMIT-03 `POST /api/admin/quote/preview` (§10, corrected by operator ruling
 * 2026-08-23): deterministic re-ranking of the SAME live component rows under
 * the PROPOSED weights — never a mutation (QB-TOPOLOGY-01: quote/base is
 * metadata over a numeraire-agnostic graph; `graph_rebuild_required` is the
 * doctrine literal `false`). Response envelope:
 *   - `impact` — EXACT mirror of searcher `preview_impact_to_wire` (9 keys,
 *     `QuotePreviewImpactSchema`).
 *   - `proposed_quote_symbol` / `proposed_quote_score` / `proposed_tokens` —
 *     the §10 sketch fields: the frontend renders the proposed ranking, it
 *     never recomputes scores (§79).
 *
 * ## R8 Fail-Honest
 * - 503 `redis_unavailable` — no Redis connection / transient read error.
 * - 503 `quote_anchor_not_published` — key absent: the searcher has not
 *   published on this Redis (or the TTL lapsed with a dead searcher). Never a
 *   fabricated anchor (RULE 00).
 * - 503 `quote_anchor_snapshot_corrupted` — unparseable / malformed payload.
 * - `affected_cached_routes` is the honest 0: no route-cache counter exists
 *   yet (documented frontier decision in quote_anchor_signal.rs — the TS
 *   contract made these non-nullable ints).
 */
import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";

import { requireAdminToken } from "@arbx/shared";

/** Mirror of searcher `quote_anchor_runtime::QUOTE_ANCHOR_KEY_PREFIX`. */
export const QUOTE_ANCHOR_KEY_PREFIX = "arbx:quote:anchor:";

const AXES = ["prior", "liquidity", "venues", "stability", "cross_dex"] as const;

export interface QuoteAnchorDeps {
  redis: Redis | null;
  logger: { warn: (obj: object, msg?: string) => void };
  /** Admin token for the preview (mutation-simulation) surface. */
  adminToken: string;
}

function quoteAnchorKey(chainId: number): string {
  return `${QUOTE_ANCHOR_KEY_PREFIX}${chainId}`;
}

/** Row shape as published by `token_row_to_wire` (QuoteTokenRowSchema). */
interface WireTokenRow {
  symbol: string;
  address: string;
  components: Record<(typeof AXES)[number], number>;
  score: number;
}

/** Minimum snapshot fields this surface consumes; everything else passes
 * through verbatim on the GET (RULE 00 — this route computes nothing). */
interface WireSnapshot {
  quote_symbol: string;
  quote_version: number;
  tokens: WireTokenRow[];
  pairs_by_symbol?: Record<string, number>;
  pools_by_symbol?: Record<string, number>;
}

/** Parse + structurally validate the snapshot. `null` = corrupted (R8). */
function parseSnapshot(raw: string): WireSnapshot | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof parsed !== "object" || parsed === null) return null;
  const s = parsed as Record<string, unknown>;
  if (typeof s.quote_symbol !== "string" || s.quote_symbol.length === 0) return null;
  if (!Number.isSafeInteger(s.quote_version) || (s.quote_version as number) < 0) return null;
  if (!Array.isArray(s.tokens)) return null;
  for (const row of s.tokens) {
    if (typeof row !== "object" || row === null) return null;
    const r = row as Record<string, unknown>;
    if (typeof r.symbol !== "string" || typeof r.address !== "string") return null;
    const c = r.components;
    if (typeof c !== "object" || c === null) return null;
    for (const axis of AXES) {
      const v = (c as Record<string, unknown>)[axis];
      if (typeof v !== "number" || !Number.isFinite(v)) return null;
    }
  }
  return s as unknown as WireSnapshot;
}

/** Read + parse the snapshot for one chain. Errors map to their 503 reason. */
async function loadSnapshot(
  deps: QuoteAnchorDeps,
  chainId: number,
): Promise<{ ok: WireSnapshot } | { err: string; reason: string }> {
  if (!deps.redis) return { err: "redis_unavailable", reason: "no Redis connection" };
  let raw: string | null;
  try {
    raw = await deps.redis.get(quoteAnchorKey(chainId));
  } catch (e) {
    deps.logger.warn({ event: "quote_anchor.redis_get_failed", chain_id: chainId, err: (e as Error).message });
    return { err: "redis_unavailable", reason: (e as Error).message };
  }
  if (raw === null) {
    return {
      err: "quote_anchor_not_published",
      reason: "searcher-rs publishes arbx:quote:anchor:<chain> every tick (TTL 35s); key absent",
    };
  }
  const parsed = parseSnapshot(raw);
  if (parsed === null) {
    deps.logger.warn({ event: "quote_anchor.parse_failed", chain_id: chainId });
    return { err: "quote_anchor_snapshot_corrupted", reason: "unparseable or malformed snapshot" };
  }
  return { ok: parsed };
}

/** Deterministic re-ranking under proposed weights — the SAME rows, new
 * scores (dot product with the 5 axes), sorted score desc → symbol asc →
 * address asc (mirrors `select_quote_anchor`'s tie-break chain). Comparators
 * are PLAIN byte-order (`<`/`>`), never `localeCompare` — the Rust side sorts
 * by `String` Ord, so the tie-break must be locale-independent. */
function reRank(
  rows: WireTokenRow[],
  weights: Record<(typeof AXES)[number], number>,
): { symbol: string; address: string; components: WireTokenRow["components"]; score: number }[] {
  const byteOrder = (x: string, y: string): number => (x < y ? -1 : x > y ? 1 : 0);
  return rows
    .map((r) => ({
      symbol: r.symbol,
      address: r.address,
      components: r.components,
      score: AXES.reduce((acc, axis) => acc + weights[axis] * r.components[axis], 0),
    }))
    .sort((a, b) => b.score - a.score || byteOrder(a.symbol, b.symbol) || byteOrder(a.address, b.address));
}

export function mountQuoteAnchor(app: Application, deps: QuoteAnchorDeps): void {
  // ── EMIT-02 Layer-2: the flattened 8-key anchor view ─────────────────────
  app.get("/api/quote/anchor", async (req: Request, res: Response) => {
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isInteger(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    const loaded = await loadSnapshot(deps, chainId);
    if ("err" in loaded) {
      res.status(503).json({ error: loaded.err, detail: loaded.reason, chain_id: chainId });
      return;
    }
    // Strip the preview-only sidecars: the response carries EXACTLY the 8
    // keys of QuoteAnchorResponseSchema (.strict() on the consumer side).
    const { pairs_by_symbol: _p, pools_by_symbol: _q, ...view } = loaded.ok;
    res.status(200).json(view);
  });

  // ── EMIT-03: preview-before-apply for quote weights (admin, no mutation) ──
  app.post("/api/admin/quote/preview", requireAdminToken(deps.adminToken), async (req, res) => {
    const body = (req.body ?? {}) as { chain_id?: unknown; weights?: unknown };
    const chainId = Number(body.chain_id);
    if (!Number.isInteger(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    // Weight validation mirrors quote_score.rs `QuoteWeights::validate` at the
    // wire-field level: five non-negative axes summing to 1 within 1e-9 (the
    // backend knob validation stays the authority; this is fail-fast input
    // hygiene with the SAME epsilon, never a second source of truth for the
    // weights' semantics).
    const w = (body.weights ?? {}) as Record<string, unknown>;
    if (
      AXES.some((a) => {
        const v = w[a];
        return typeof v !== "number" || !Number.isFinite(v) || v < 0;
      }) ||
      Object.keys(w).length !== AXES.length
    ) {
      res.status(400).json({ error: "invalid_weights", axes: AXES, min: 0 });
      return;
    }
    const sum = AXES.reduce((acc, a) => acc + (w[a] as number), 0);
    if (Math.abs(sum - 1.0) > 1e-9) {
      res.status(400).json({ error: "invalid_weights_sum", expected: 1, epsilon: 1e-9, got: sum });
      return;
    }
    const weights = Object.fromEntries(AXES.map((a) => [a, w[a] as number])) as Record<
      (typeof AXES)[number],
      number
    >;

    const loaded = await loadSnapshot(deps, chainId);
    if ("err" in loaded) {
      res.status(503).json({ error: loaded.err, detail: loaded.reason, chain_id: chainId });
      return;
    }
    const snap = loaded.ok;
    const proposed = reRank(snap.tokens, weights);
    // Writer invariant: the anchor row always heads `tokens` — an empty
    // table (hence an empty re-ranking) is a corrupted snapshot, not an
    // honest empty preview.
    const head = proposed[0];
    if (head === undefined) {
      res.status(503).json({ error: "quote_anchor_snapshot_corrupted", chain_id: chainId });
      return;
    }
    const anchorChanges = head.symbol !== snap.quote_symbol;
    // Pairs/pools touched by the transition = the two anchors' footprints
    // (union semantics degenerate when nothing changes → honest 0, coherent
    // with quote_revaluation_required gating on the SAME boolean).
    const sidecar = (m: Record<string, number> | undefined, sym: string): number => {
      const v = m?.[sym];
      return typeof v === "number" && Number.isFinite(v) ? v : 0;
    };
    const affectedPairs = anchorChanges
      ? sidecar(snap.pairs_by_symbol, snap.quote_symbol) + sidecar(snap.pairs_by_symbol, head.symbol)
      : 0;
    const affectedEdges = anchorChanges
      ? sidecar(snap.pools_by_symbol, snap.quote_symbol) + sidecar(snap.pools_by_symbol, head.symbol)
      : 0;

    res.status(200).json({
      impact: {
        graph_rebuild_required: false, // QB-TOPOLOGY-01 doctrine literal
        quote_revaluation_required: anchorChanges,
        quote_cache_invalidation_required: anchorChanges,
        affected_pairs: affectedPairs,
        affected_edges: affectedEdges,
        affected_cached_routes: 0, // no route-cache counter exists yet — honest zero
        current_quote_version: snap.quote_version,
        proposed_quote_version: snap.quote_version + (anchorChanges ? 1 : 0),
        topology_version_unchanged: true,
      },
      proposed_quote_symbol: head.symbol,
      proposed_quote_score: head.score,
      proposed_tokens: proposed,
    });
  });
}
