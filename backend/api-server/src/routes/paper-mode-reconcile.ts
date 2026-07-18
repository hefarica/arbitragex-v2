import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";
import { randomUUID } from "node:crypto";

const RECONCILE_LUA = `
local key = KEYS[1]
local auditStream = KEYS[2]
local changeChannel = KEYS[3]
local newState = ARGV[1]
local correlationId = ARGV[2]
local actor = ARGV[3]
local reason = ARGV[4]
local chainId = ARGV[5]

if redis.call("EXISTS", key) == 1 then
  return {0, "already_exists"}
end

redis.call("SET", key, newState)
redis.call("XADD", auditStream, "*",
  "correlation_id", correlationId,
  "actor", actor,
  "reason", reason,
  "chain_id", chainId,
  "new_state", newState
)
redis.call("PUBLISH", changeChannel, newState)

return {1, "created"}
`;

type ReconcileAction = "created" | "already_exists" | "skipped";

interface ReconcileResult {
  chain_id: number;
  action: ReconcileAction;
  created: boolean;
}

export interface MountPaperModeReconcileDeps {
  redis: Redis | null;
  env: NodeJS.ProcessEnv;
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  logger: { warn: (obj: object, msg?: string) => void };
}

export function mountPaperModeReconcile(app: Application, deps: MountPaperModeReconcileDeps): void {
  const autoReconcile = (deps.env["ARBX_PAPER_AUTO_RECONCILE"] ?? "").toLowerCase() === "on";

  app.post("/admin/paper-mode/reconcile", deps.requireAdminToken(deps.adminToken), async (req: Request, res: Response) => {
    if (!autoReconcile) {
      res.status(503).json({ error: "reconcile_disabled", detail: "ARBX_PAPER_AUTO_RECONCILE is not 'on'" });
      return;
    }

    const { chain_id, new_state, dry_run, reason, correlation_id } = req.body ?? {};
    if (typeof chain_id !== "number" || !Number.isInteger(chain_id) || chain_id < 1) {
      res.status(400).json({ error: "invalid_chain_id", detail: "chain_id must be a positive integer" });
      return;
    }
    if (typeof new_state !== "string" || new_state.length === 0) {
      res.status(400).json({ error: "invalid_new_state", detail: "new_state must be a non-empty string" });
      return;
    }

    const dryRun = dry_run === true;
    const actor = req.header("x-arbx-actor") ?? "admin";
    const corrId = typeof correlation_id === "string" && correlation_id.length > 0 ? correlation_id : randomUUID();
    const why = typeof reason === "string" && reason.length > 0 ? reason : "reconcile";
    const key = `arbx:papermode:${chain_id}`;
    const auditStream = `arbx:papermode:audit:${chain_id}`;
    const changeChannel = `arbx:papermode:${chain_id}:changes`;

    if (dryRun) {
      const exists = deps.redis ? await deps.redis.exists(key) : 0;
      const result: ReconcileResult = {
        chain_id,
        action: exists ? "already_exists" : "created",
        created: !exists,
      };
      res.status(200).json({ dry_run: true, results: [result] });
      return;
    }

    if (!deps.redis) {
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }

    try {
      const raw = await deps.redis.eval(
        RECONCILE_LUA,
        3,
        key,
        auditStream,
        changeChannel,
        new_state,
        corrId,
        actor,
        why,
        String(chain_id),
      );
      const reply = raw as [number | string, string];
      const created = Number(reply[0]) === 1;
      const result: ReconcileResult = {
        chain_id,
        action: created ? "created" : "already_exists",
        created,
      };
      res.status(200).json({ dry_run: false, results: [result] });
    } catch (e) {
      deps.logger.warn({ event: "paper_mode_reconcile.failed", err: (e as Error).message });
      res.status(500).json({ error: "reconcile_failed", detail: (e as Error).message });
    }
  });
}
