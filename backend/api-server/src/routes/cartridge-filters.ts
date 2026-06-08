// Cartridge Filters routes — operator-tunable, per-chain route pre-filter preferences.
//
// Idea 1 (FOUNDATION, Phase 1): persist the operator's cartridge filter selection
// (strategy allowlist, min net profit, max slippage, optional custom base64 .rhai)
// so the UI has a durable place to read/write it.
//
//   GET  /api/v1/cartridge-filters?chain_id=1   → public read (configured:false if unset)
//   PUT  /admin/cartridge-filters/:chain_id     → admin: upsert + Redis SET + publish + audit
//
// HONESTY (RULE 00 / R8): this is a STORED PREFERENCE only. As of Phase 1 NOTHING in
// searcher-rs reads `arbx:cartridge:filters:<chain>` — the route pre-filter CONSUMER is a
// Phase 2 (hot-path Rust) follow-up, and durability (PG source-of-truth + boot
// re-hydration, mirroring the trading_config fix) is paired with that phase, since the key
// only becomes load-bearing once a consumer exists. The PUT response + boot log say so
// explicitly so no caller assumes the filter is active. Redis-only for now = ephemeral
// preference (lost on a Redis flush) by deliberate design, not oversight.

import { Router, type Request, type Response } from "express";
import type { Redis } from "ioredis";
import { z } from "zod";

export const CARTRIDGE_FILTERS_CHANNEL = "arbx:cartridge:filters:changes";

export const cartridgeFiltersKey = (chainId: number): string =>
  `arbx:cartridge:filters:${chainId}`;

// Permissive defaults = "no filter": empty allowlist means all strategies, 0 min profit,
// max slippage at the ceiling. A future searcher-rs consumer applies these as a pre-pass.
export const CartridgeFilterSchema = z.object({
  enabled_strategies: z.array(z.string().min(1).max(64)).max(32).default([]),
  min_profit_usd: z.number().nonnegative().default(0),
  max_slippage_bps: z.number().int().min(0).max(10_000).default(10_000),
  // ~500 KB cap on the base64 string (~375 KB decoded). null = no custom cartridge.
  custom_cartridge_b64: z.string().max(500_000).nullable().optional(),
  enabled: z.boolean().default(true),
});

export type CartridgeFilterInput = z.infer<typeof CartridgeFilterSchema>;

interface Deps {
  redis: Redis;
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  writeAudit: (
    action: string,
    actor: string,
    targetKind: string | null,
    targetId: string | null,
    before: unknown,
    after: unknown,
    ip: string | null,
    traceId: string | null,
    userAgent?: string | null,
  ) => Promise<void>;
  logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
}

const NOT_CONSUMED_NOTE =
  "Stored as operator preference. NOTE: not yet consumed by searcher-rs — the route " +
  "pre-filter consumer + PG-backed durability are a Phase 2 (hot-path) follow-up.";

export function buildCartridgeFiltersRouter(deps: Deps): Router {
  const r = Router();

  // ── Public read ────────────────────────────────────────────────────────
  r.get("/api/v1/cartridge-filters", async (req: Request, res: Response) => {
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isFinite(chainId) || chainId < 1) {
      res.status(400).json({ ok: false, error: "invalid_chain_id" });
      return;
    }
    try {
      const raw = await deps.redis.get(cartridgeFiltersKey(chainId));
      if (!raw) {
        // R8 fail-honest: absent key = not configured (NOT a fabricated default).
        res.status(200).json({ ok: true, configured: false, chain_id: chainId });
        return;
      }
      let parsed: unknown;
      try {
        parsed = JSON.parse(raw);
      } catch {
        res.status(200).json({ ok: false, configured: false, chain_id: chainId, error: "corrupt_state" });
        return;
      }
      res.status(200).json({ ok: true, configured: true, ...(parsed as Record<string, unknown>) });
    } catch (e) {
      res.status(503).json({ ok: false, error: "redis_unavailable", detail: (e as Error).message });
    }
  });

  // ── Admin upsert ───────────────────────────────────────────────────────
  r.put(
    "/admin/cartridge-filters/:chain_id",
    deps.requireAdminToken(deps.adminToken),
    async (req: Request, res: Response) => {
      const chainId = Number(req.params["chain_id"]);
      if (!Number.isFinite(chainId) || chainId < 1) {
        res.status(400).json({ ok: false, error: "invalid_chain_id" });
        return;
      }
      const parsed = CartridgeFilterSchema.safeParse(req.body);
      if (!parsed.success) {
        res.status(400).json({ ok: false, error: "invalid_request", details: parsed.error.flatten() });
        return;
      }
      const actor = req.header("x-arbx-actor") ?? "admin";
      const state = {
        chain_id: chainId,
        ...parsed.data,
        updated_at: new Date().toISOString(),
        updated_by: actor,
      };
      const json = JSON.stringify(state);
      try {
        const beforeRaw = await deps.redis.get(cartridgeFiltersKey(chainId));
        await deps.redis.set(cartridgeFiltersKey(chainId), json);
        const subscribers = await deps.redis.publish(CARTRIDGE_FILTERS_CHANNEL, json);
        try {
          await deps.writeAudit(
            "cartridge_filters.upsert",
            actor,
            "cartridge_filters",
            String(chainId),
            beforeRaw ? (JSON.parse(beforeRaw) as unknown) : null,
            state,
            req.ip ?? null,
            (req as Request & { traceId?: string }).traceId ?? null,
            req.header("user-agent") ?? null,
          );
        } catch (err) {
          deps.logger.warn(
            { event: "cartridge_filters.audit_failed", chain_id: chainId, err: (err as Error).message },
            "cartridge_filters audit write failed (non-fatal)",
          );
        }
        deps.logger.info(
          {
            event: "cartridge_filters.upsert",
            chain_id: chainId,
            actor,
            subscribers,
            consumed: false,
            channel: CARTRIDGE_FILTERS_CHANNEL,
          },
          "cartridge filters persisted (Redis-only Phase-1 foundation; not yet consumed)",
        );
        res.status(200).json({ ok: true, ...state, subscribers, note: NOT_CONSUMED_NOTE });
      } catch (e) {
        res.status(503).json({ ok: false, error: "redis_write_failed", detail: (e as Error).message });
      }
    },
  );

  return r;
}
