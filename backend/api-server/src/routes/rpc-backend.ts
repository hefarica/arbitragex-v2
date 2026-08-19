/**
 * RPC backend toggle — alloy dual-track FASE 4
 * (docs/plans/alloy-parallel-path-macro-plan.md §2.3, §FASE 4).
 *
 * Endpoints (both admin-gated via requireAdminToken):
 *
 *   GET  /api/admin/rpc-backend
 *     → { services: [{ name, current, options }], updated_at }
 *
 *   PUT  /api/admin/rpc-backend
 *     Body: { service: "searcher-rs" | "relays-client" | "sim-ctl" | "all",
 *             backend: "ethers" | "alloy" | "shadow" }
 *     → { ok: true, service, backend, updated_at, results: [...] }
 *
 * Redis (NO TTL — the toggle persists across restarts, by design):
 *   arbx:rpc_backend:<service> = "ethers" | "alloy" | "shadow"
 *   Absent key → "ethers" (production default; the ethers track is canonical
 *   until the alloy track is shadow-verified per the macro plan).
 *
 * Change notification (same pattern as paper-mode):
 *   PUBLISH arbx:rpc_backend:changes            {service, backend, updated_at}
 *   PUBLISH arbx:rpc_backend:<service>:changes  (same payload)
 *
 * Audit: every EFFECTIVE change writes one audit_log row per service with
 * before/after (quién / cuándo / de-qué-a-qué). A no-op write (same backend)
 * is answered honestly with changed:false and is NOT audited as a change.
 *
 * R8 fail-honest: Redis unavailable → 503 redis_unavailable. A stored value
 * outside the enum is surfaced verbatim (never silently coerced) so operators
 * see the anomaly.
 *
 * Mode-invariant (CLAUDE.md §34.1): this toggle only selects the RPC
 * implementation track. It never touches trading mode, capital, or broadcast
 * gates — those live in the terminus (relays-client live_exec_policy).
 */
import type { Request, Response } from "express";
import type { Redis } from "ioredis";
import { z } from "zod";

export const RPC_BACKEND_SERVICES = ["searcher-rs", "relays-client", "sim-ctl"] as const;
export const RPC_BACKEND_KINDS = ["ethers", "alloy", "shadow"] as const;

export type RpcBackendServiceName = (typeof RPC_BACKEND_SERVICES)[number];
export type RpcBackendKind = (typeof RPC_BACKEND_KINDS)[number];

const PutBody = z.object({
  service: z.enum([...RPC_BACKEND_SERVICES, "all"]),
  backend: z.enum(RPC_BACKEND_KINDS),
});

const keyOf = (service: RpcBackendServiceName): string => `arbx:rpc_backend:${service}`;

function isValidKind(v: string | null): v is RpcBackendKind {
  return v !== null && (RPC_BACKEND_KINDS as readonly string[]).includes(v);
}

export function mountRpcBackend(
  app: import("express").Application,
  deps: {
    redis: Redis | null;
    requireAdminToken: (expected: string) => import("express").RequestHandler;
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
      userAgent: string | null,
    ) => Promise<void>;
    reqUA: (req: Request) => string | null;
    logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
  },
): void {
  const { redis, requireAdminToken, adminToken, writeAudit, reqUA, logger } = deps;

  // ── GET — current selection per service ───────────────────────────────────
  app.get("/api/admin/rpc-backend", requireAdminToken(adminToken), async (_req, res) => {
    if (!redis) {
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }
    try {
      const stored = await redis.mget(...RPC_BACKEND_SERVICES.map(keyOf));
      const services = RPC_BACKEND_SERVICES.map((name, i) => {
        const raw = stored[i] ?? null;
        return {
          name,
          // Absent key → canonical default "ethers". Invalid stored value is
          // surfaced verbatim (R8 — never fabricate, never silently coerce).
          current: raw ?? "ethers",
          options: [...RPC_BACKEND_KINDS],
        };
      });
      res.status(200).json({
        services,
        updated_at: new Date().toISOString(),
      });
    } catch (e) {
      logger.warn({ event: "rpc_backend.get_failed", err: (e as Error).message });
      res.status(503).json({ error: "redis_error", detail: (e as Error).message });
    }
  });

  // ── PUT — switch one service (or "all") to a backend track ────────────────
  app.put("/api/admin/rpc-backend", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    if (!redis) {
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }
    const parsed = PutBody.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({
        error: "invalid_body",
        detail: `service must be one of ${[...RPC_BACKEND_SERVICES, "all"].join("|")}; backend one of ${RPC_BACKEND_KINDS.join("|")}`,
        issues: parsed.error.issues,
      });
      return;
    }
    const { service, backend } = parsed.data;
    const targets: RpcBackendServiceName[] =
      service === "all" ? [...RPC_BACKEND_SERVICES] : [service];
    const actor = req.header("x-arbx-actor") ?? "admin";
    const updatedAt = new Date().toISOString();

    try {
      const prev = await redis.mget(...targets.map(keyOf));
      const results: Array<{ service: RpcBackendServiceName; previous: string | null; changed: boolean }> = [];

      for (const [i, target] of targets.entries()) {
        const before = prev[i] ?? null;
        if (before === backend) {
          results.push({ service: target, previous: before, changed: false });
          continue;
        }
        const payload = JSON.stringify({ service: target, backend, updated_at: updatedAt, actor });
        await redis.set(keyOf(target), backend);
        await Promise.all([
          redis.publish("arbx:rpc_backend:changes", payload),
          redis.publish(`arbx:rpc_backend:${target}:changes`, payload),
        ]);
        await writeAudit(
          "admin.rpc_backend.update",
          actor,
          "rpc_backend",
          target,
          { backend: before ?? "ethers(unset)" },
          { backend },
          req.ip ?? null,
          (req as Request & { traceId?: string }).traceId ?? null,
          reqUA(req),
        );
        results.push({ service: target, previous: before, changed: true });
      }

      logger.info({ event: "rpc_backend.updated", service, backend, actor, results });
      res.status(200).json({ ok: true, service, backend, updated_at: updatedAt, results });
    } catch (e) {
      logger.warn({ event: "rpc_backend.put_failed", service, backend, err: (e as Error).message });
      res.status(503).json({ error: "redis_error", detail: (e as Error).message });
    }
  });
}
