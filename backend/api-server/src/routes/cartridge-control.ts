// CARTRIDGE-CONTROL (2026-09-24) — acople/desacople por cartucho.
/**
 * cartridge-control — operator couple/decouple control plane for ANY loaded
 * cartridge (the 264 numerados + 7 raíz + future v4 stems).
 *
 * Runtime contract (searcher-rs `cartridge_control.rs`):
 *   - Desired states live in the Redis hash `arbx:cartridges:control:<chain>`
 *     (field cartridge_id → "enabled" | "disabled"). Absent field = load-default
 *     (Active) — absence NEVER decouples.
 *   - Hot commands ride the PubSub channel `arbx:cartridges:control:commands`
 *     as JSON {chain_id, cartridge_id, desired, actor, reason}; the searcher
 *     applies pause/resume and re-publishes the registry snapshot so
 *     GET /api/cartridges reflects the new state without the 240s refresh.
 *
 * Endpoints (both gated by V-AT-1 admin token, same mount style as the
 * cartridges router):
 *
 *   GET /api/v1/cartridges/control?chain_id=1
 *     Merged view: registry snapshot (arbx:cartridges:registry:<chain>) joined
 *     with the desired-states hash → [{id, name, category, runtime_state,
 *     desired}] plus {desired_count}. Fail-honest: registry absent ⇒ rows []
 *     with `registry: "unavailable"` (never fabricated cartridges).
 *
 *   PUT /api/v1/cartridges/control   body { chain_id, cartridge_id, desired, reason }
 *     Sovereignty gates, in order (control-board house discipline):
 *       1. admin token (requireAdminToken — the edge adminProxy translates the
 *          browser httpOnly session cookie into x-arbx-admin-token);
 *       2. body validated server-side: chain_id positive int, cartridge_id
 *          non-empty AND restricted to [A-Za-z0-9_.-] (it becomes a Redis hash
 *          field and a PG text key — no injection surface), desired ∈
 *          {enabled, disabled}, reason non-empty AFTER TRIM (mandatory
 *          quién/cuándo/por qué audit record);
 *       3. AUDIT FIRST: INSERT into `cartridge_control` (migration 125) AND
 *          `audit_log` (migration 011) BEFORE any Redis write. Failure ⇒ 500
 *          and NO Redis mutation;
 *       4. Redis write: HSET the control hash + PUBLISH the command. Either
 *          failure ⇒ compensatory `cartridge_control.command_failed` audit row
 *          (append-only doctrine) and 503 — the ledger never claims a change
 *          that did not land;
 *       5. Response carries the command echo + `applied_by` note (the searcher
 *          applies asynchronously via the PubSub loop; idempotent re-sends are
 *          safe).
 *
 * This route NEVER touches signers, trading modes, or broadcast (§32/§33/§34):
 * decoupling a cartridge only stops its EVALUATION.
 */
import { Router, type Request, type Response, type NextFunction, type RequestHandler } from "express";

export interface CartridgeControlDeps {
  redis: unknown; // Redis client (ioredis-like: hgetall/hset/publish)
  pool: { query: (sql: string, params?: unknown[]) => Promise<unknown> };
  requireAdminToken?: (expected: string) => RequestHandler;
  adminToken?: string;
  logger: {
    warn: (obj: object, msg?: string) => void;
    info?: (obj: object, msg?: string) => void;
    error?: (obj: object, msg?: string) => void;
  };
}

const CONTROL_HASH = (chainId: number): string => `arbx:cartridges:control:${chainId}`;
const CONTROL_CHANNEL = "arbx:cartridges:control:commands";
const CARTRIDGE_ID_RE = /^[A-Za-z0-9_.-]{1,128}$/;

interface ControlRow {
  chain_id: number;
  cartridge_id: string;
  desired: "enabled" | "disabled";
  actor: string;
  reason: string;
}

function badRequest(res: Response, error: string, detail?: string): void {
  res.status(400).json({ error, ...(detail ? { detail } : {}) });
}

function validateBody(body: unknown): { ok: true; cmd: ControlRow } | { ok: false; error: string; detail?: string } {
  if (typeof body !== "object" || body === null) return { ok: false, error: "invalid_body" };
  const b = body as Record<string, unknown>;
  const chainId = Number(b.chain_id);
  if (!Number.isInteger(chainId) || chainId <= 0) return { ok: false, error: "invalid_chain_id" };
  const cartridgeId = typeof b.cartridge_id === "string" ? b.cartridge_id.trim() : "";
  if (!CARTRIDGE_ID_RE.test(cartridgeId)) {
    return {
      ok: false,
      error: "invalid_cartridge_id",
      detail: "non-empty, max 128 chars, [A-Za-z0-9_.-]",
    };
  }
  if (b.desired !== "enabled" && b.desired !== "disabled") {
    return { ok: false, error: "invalid_desired", detail: "must be 'enabled' | 'disabled'" };
  }
  const reason = typeof b.reason === "string" ? b.reason.trim() : "";
  if (reason.length === 0) return { ok: false, error: "reason_required" };
  return {
    ok: true,
    cmd: {
      chain_id: chainId,
      cartridge_id: cartridgeId,
      desired: b.desired,
      actor: "admin",
      reason,
    },
  };
}

async function getControlView(
  req: Request,
  res: Response,
  redis: NonNullable<CartridgeControlDeps["redis"]> & {
    get(key: string): Promise<string | null>;
    hgetall(key: string): Promise<Record<string, string>>;
  },
): Promise<void> {
  const chainId = Number(req.query.chain_id ?? 1);
  if (!Number.isInteger(chainId) || chainId <= 0) {
    badRequest(res, "invalid_chain_id");
    return;
  }
  // Fail-honest reads: registry absent ⇒ empty rows (RULE 00); Redis errors ⇒ 503.
  let registryRaw: string | null = null;
  let desired: Record<string, string> = {};
  try {
    registryRaw = await redis.get(`arbx:cartridges:registry:${chainId}`);
    desired = await redis.hgetall(CONTROL_HASH(chainId));
  } catch {
    res.status(503).json({ error: "redis_unavailable" });
    return;
  }
  interface RegistryEntry {
    id: string;
    name: string;
    category: string;
    state: string;
  }
  let entries: RegistryEntry[] = [];
  let registryStatus = "unavailable";
  if (registryRaw) {
    try {
      const parsed = JSON.parse(registryRaw) as { cartridges?: RegistryEntry[] };
      if (Array.isArray(parsed.cartridges)) {
        entries = parsed.cartridges;
        registryStatus = "ok";
      }
    } catch {
      registryStatus = "corrupted";
    }
  }
  const rows = entries.map((e) => ({
    id: e.id,
    name: e.name,
    category: e.category,
    runtime_state: e.state,
    desired: desired[e.id] ?? "enabled(default)",
  }));
  res.json({
    chain_id: chainId,
    registry: registryStatus,
    desired_count: Object.keys(desired).length,
    cartridges: rows,
  });
}

async function putControl(
  req: Request,
  res: Response,
  deps: CartridgeControlDeps,
): Promise<void> {
  const { logger } = deps;
  const redis = deps.redis as NonNullable<CartridgeControlDeps["redis"]> & {
    hset(key: string, field: string, value: string): Promise<unknown>;
    publish(channel: string, message: string): Promise<unknown>;
  };
  const validated = validateBody(req.body);
  if (!validated.ok) {
    badRequest(res, validated.error, validated.detail);
    return;
  }
  const cmd = validated.cmd;
  const actor = req.header("x-arbx-actor") ?? "admin";
  const at = new Date().toISOString();

  // Previous desired state for the before_state audit column (absent = null).
  let before: string | null = null;
  try {
    const prev = await (
      deps.redis as NonNullable<CartridgeControlDeps["redis"]> & {
        hget(key: string, field: string): Promise<string | null>;
      }
    ).hget(CONTROL_HASH(cmd.chain_id), cmd.cartridge_id);
    before = prev ?? null;
  } catch {
    res.status(503).json({ error: "redis_unavailable" });
    return;
  }

  // Gate: audit FIRST (cartridge_control + audit_log) — failure ⇒ NO Redis write.
  const afterPayload = {
    wo: "CARTRIDGE-CONTROL",
    chain_id: cmd.chain_id,
    cartridge_id: cmd.cartridge_id,
    desired: cmd.desired,
    reason: cmd.reason,
    actor,
    at,
    applied: true,
  };
  try {
    await deps.pool.query(
      `INSERT INTO cartridge_control (chain_id, cartridge_id, desired, actor, reason)
       VALUES ($1, $2, $3, $4, $5)`,
      [cmd.chain_id, cmd.cartridge_id, cmd.desired, actor, cmd.reason],
    );
    await deps.pool.query(
      `INSERT INTO audit_log (actor, action, target_kind, target_id, before_state, after_state)
       VALUES ($1, 'cartridge_control.command', 'cartridge', $2, $3::jsonb, $4::jsonb)`,
      [
        actor,
        `${cmd.chain_id}:${cmd.cartridge_id}`,
        JSON.stringify({ desired: before }),
        JSON.stringify(afterPayload),
      ],
    );
  } catch (e) {
    logger.warn({ event: "cartridge_control.audit_insert_failed", err: (e as Error).message });
    res.status(500).json({
      error: "audit_write_failed",
      detail: "cartridge_control/audit_log INSERT failed — Redis NOT written (audit-first sovereignty gate)",
    });
    return;
  }

  // Redis write: desired hash + hot command publish. Failure ⇒ compensatory
  // audit row (append-only) + 503.
  try {
    await redis.hset(CONTROL_HASH(cmd.chain_id), cmd.cartridge_id, cmd.desired);
    await redis.publish(
      CONTROL_CHANNEL,
      JSON.stringify({ ...cmd, actor, at }),
    );
  } catch (e) {
    const redisError = (e as Error).message;
    logger.warn({ event: "cartridge_control.redis_write_failed", err: redisError });
    try {
      await deps.pool.query(
        `INSERT INTO cartridge_control (chain_id, cartridge_id, desired, actor, reason)
         VALUES ($1, $2, $3, $4, $5)`,
        [
          cmd.chain_id,
          cmd.cartridge_id,
          cmd.desired,
          actor,
          `[COMMAND_FAILED redis_error=${redisError}] ${cmd.reason}`,
        ],
      );
    } catch {
      logger.error?.({ event: "cartridge_control.compensating_row_failed" });
    }
    res.status(503).json({ error: "redis_write_failed", detail: redisError });
    return;
  }

  res.json({
    ok: true,
    command: afterPayload,
    applied_by: "searcher cartridge_control loop (PubSub, idempotent)",
    verify: `GET /api/v1/cartridges/control?chain_id=${cmd.chain_id} or GET /api/cartridges?chain_id=${cmd.chain_id}`,
  });
}

export function buildCartridgeControlRouter(deps: CartridgeControlDeps): Router {
  const router = Router();
  const redis = deps.redis as NonNullable<CartridgeControlDeps["redis"]> & {
    get(key: string): Promise<string | null>;
    hgetall(key: string): Promise<Record<string, string>>;
  };
  if (deps.requireAdminToken && deps.adminToken) {
    router.use(deps.requireAdminToken(deps.adminToken));
  }
  router.get("/control", (req: Request, res: Response, next: NextFunction) => {
    void getControlView(req, res, redis).catch(next);
  });
  router.put("/control", (req: Request, res: Response, next: NextFunction) => {
    void putControl(req, res, deps).catch(next);
  });
  return router;
}
