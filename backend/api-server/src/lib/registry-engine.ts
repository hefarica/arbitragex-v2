/**
 * =============================================================================
 * OMEGA Registry Engine — Generic 7-Layer CRUD with Idempotency + Hot-Reload
 * =============================================================================
 *
 * Sole purpose: enforce the 7-Layer Coherence Rule on every operational
 * entity registered in the canonical fabric.  Replaces ad-hoc CRUD code
 * with a single, audited, hot-reloadable, idempotent surface.
 *
 *   Frontend → Edge → API (this) → PostgreSQL → Redis pub/sub
 *           → searcher-rs Arc-swap → runtime_ack → Audit/Readiness
 *
 * Doctrine:
 *   • Zero-Mocks                — every code path executes real I/O.
 *   • Ghost Protocol            — no telemetry leaves the cluster.
 *   • Lexicón Absoluto          — Topological Yield / Holonomic Resolution.
 *   • Idempotency               — same idempotency_key + config_hash = no-op.
 *   • State Machine UI          — drives the front-end discriminated union.
 *
 * @module lib/registry-engine
 */

import { createHash, randomUUID } from "node:crypto";
import type { Request, Response, Router } from "express";
import type { Pool, PoolClient } from "pg";
import type { Redis } from "ioredis";
import { z, type ZodTypeAny } from "zod";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type EntityType =
  | "rpc"
  | "contract"
  | "risk_gate"
  | "capital_gate"
  | "relay"
  | "agent";

export type EntityRow = Record<string, unknown> & {
  id: string;
  config_hash: string;
};

export interface RegistryDescriptor<TSchema extends ZodTypeAny> {
  /** Logical entity type (matches audit_event.entity_type CHECK). */
  entity: EntityType;
  /** PostgreSQL table backing this entity. */
  table: string;
  /** Whether records carry a chain_id column (drives per-chain reload). */
  chainScoped: boolean;
  /** Zod schema for the writable payload (POST/PATCH body). */
  schema: TSchema;
  /** Columns that participate in config_hash computation (deterministic order). */
  hashColumns: readonly string[];
  /** Optional natural-key columns enabling idempotent upsert detection. */
  naturalKey?: readonly string[];
  /** Redis pub/sub channel base; per-chain channel appended when chainScoped. */
  reloadChannel: string; // e.g. "arbx:config:rpcs"
}

// ---------------------------------------------------------------------------
// Hash + idempotency primitives
// ---------------------------------------------------------------------------

/** Stable JSON.stringify with sorted keys — drives reproducible hashes. */
function canonicalize(input: unknown): string {
  if (input === null || typeof input !== "object") return JSON.stringify(input);
  if (Array.isArray(input)) return `[${input.map(canonicalize).join(",")}]`;
  const obj = input as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${canonicalize(obj[k])}`).join(",")}}`;
}

export function computeConfigHash(
  row: Record<string, unknown>,
  columns: readonly string[],
): string {
  const projection: Record<string, unknown> = {};
  for (const c of columns) projection[c] = row[c] ?? null;
  return createHash("sha256").update(canonicalize(projection)).digest("hex");
}

// ---------------------------------------------------------------------------
// Hot-reload publisher
// ---------------------------------------------------------------------------

export interface ReloadEvent {
  event_id: string;
  idempotency_key: string;
  resource: string;
  chain_id: number | null;
  action: "create" | "update" | "delete" | "enable" | "disable" | "reload";
  entity_id: string;
  config_hash_before: string | null;
  config_hash_after: string;
  actor: string;
  timestamp: string;
}

export async function publishReload(
  redis: Redis,
  descriptor: RegistryDescriptor<ZodTypeAny>,
  evt: ReloadEvent,
): Promise<void> {
  const payload = JSON.stringify(evt);
  const globalCh = `${descriptor.reloadChannel}:reload`;
  const channels: string[] = [globalCh];
  if (descriptor.chainScoped && evt.chain_id !== null) {
    channels.push(`${descriptor.reloadChannel}:${evt.chain_id}:reload`);
  }
  // Parallel publish — one event_id, multiple namespaces, idempotent on receiver.
  await Promise.all(channels.map((ch) => redis.publish(ch, payload)));
}

// ---------------------------------------------------------------------------
// Audit writer
// ---------------------------------------------------------------------------

export async function writeAuditEvent(
  client: PoolClient,
  params: {
    actor: string;
    action: ReloadEvent["action"];
    entityType: EntityType;
    entityId: string;
    chainId: number | null;
    before: unknown;
    after: unknown;
    hashBefore: string | null;
    hashAfter: string;
    idempotencyKey: string;
    result: "success" | "partial" | "failed";
    error?: string | undefined;
    // Optional fields accept `undefined` explicitly so callsites can pass
    // `req.ip` / `req.header(...)` directly without a conditional spread
    // dance (exactOptionalPropertyTypes: true).
    ip?: string | undefined;
    userAgent?: string | undefined;
    traceId?: string | undefined;
  },
): Promise<void> {
  // OMEGA-8/M3 P1-3: PII hardening wired-in. ip_address envuelta con
  // arbx_anonymize_ip()::cidr (post-mig 070); user_agent con
  // arbx_hash_user_agent(). Raw IP/UA NUNCA llegan a la fila.
  await client.query(
    `INSERT INTO audit_event (
       actor, action, entity_type, entity_id, chain_id,
       before_state, after_state, config_hash_before, config_hash_after,
       idempotency_key, result, error, ip_address, user_agent, trace_id
     ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,
               arbx_anonymize_ip($13)::cidr,
               arbx_hash_user_agent($14),
               $15)
     ON CONFLICT (idempotency_key) DO NOTHING`,
    [
      params.actor,
      params.action,
      params.entityType,
      params.entityId,
      params.chainId,
      params.before ? JSON.stringify(params.before) : null,
      params.after ? JSON.stringify(params.after) : null,
      params.hashBefore,
      params.hashAfter,
      params.idempotencyKey,
      params.result,
      params.error ?? null,
      params.ip ?? null,
      params.userAgent ?? null,
      params.traceId ?? null,
    ],
  );
}

// ---------------------------------------------------------------------------
// Generic CRUD router builder
// ---------------------------------------------------------------------------

export interface RegistryDeps {
  db: Pool;
  redis: Redis;
  /** Resolves an authenticated actor from the request (must not return null). */
  actorOf: (req: Request) => string;
}

export function buildRegistryRouter<TSchema extends ZodTypeAny>(
  descriptor: RegistryDescriptor<TSchema>,
  deps: RegistryDeps,
  router: Router,
): Router {
  const base = `/${descriptor.entity.replace("_", "-")}s`;

  /** GET /:entity — list with optional chain filter */
  router.get(base, async (req, res) => {
    const chainFilter =
      descriptor.chainScoped && req.query.chain_id
        ? `WHERE chain_id = $1`
        : "";
    const params = chainFilter ? [Number(req.query.chain_id)] : [];
    const r = await deps.db.query(
      `SELECT * FROM ${descriptor.table} ${chainFilter}
       ORDER BY updated_at DESC LIMIT 500`,
      params,
    );
    res.json({ rows: r.rows, count: r.rowCount });
  });

  /** GET /:entity/:id */
  router.get(`${base}/:id`, async (req, res) => {
    const r = await deps.db.query(
      `SELECT * FROM ${descriptor.table} WHERE id = $1`,
      [req.params.id],
    );
    if (r.rowCount === 0) {
      res.status(404).json({ error: "not_found" });
      return;
    }
    res.json(r.rows[0]);
  });

  /** POST /:entity — create with idempotency_key + audit + reload */
  router.post(base, async (req, res) => {
    const parse = descriptor.schema.safeParse(req.body);
    if (!parse.success) {
      res.status(400).json({ error: "schema_violation", details: parse.error.format() });
      return;
    }
    const payload = parse.data as Record<string, unknown>;
    const idempotencyKey = String(
      req.header("Idempotency-Key") ?? req.body?.idempotency_key ?? randomUUID(),
    );
    const actor = deps.actorOf(req);
    const hashAfter = computeConfigHash(payload, descriptor.hashColumns);

    const client = await deps.db.connect();
    try {
      await client.query("BEGIN");

      // Idempotency replay: same key + same hash → return previous row.
      const dup = await client.query(
        `SELECT entity_id, after_state FROM audit_event
         WHERE idempotency_key = $1 AND entity_type = $2`,
        [idempotencyKey, descriptor.entity],
      );
      if ((dup.rowCount ?? 0) > 0) {
        await client.query("COMMIT");
        res.status(200).json({ idempotent: true, row: dup.rows[0].after_state });
        return;
      }

      const cols = Object.keys(payload);
      const placeholders = cols.map((_, i) => `$${i + 1}`).join(",");
      const ins = await client.query(
        `INSERT INTO ${descriptor.table} (${cols.join(",")}, config_hash, created_by, updated_by)
         VALUES (${placeholders}, $${cols.length + 1}, $${cols.length + 2}, $${cols.length + 2})
         RETURNING *`,
        [...cols.map((c) => payload[c]), hashAfter, actor],
      );
      const row = ins.rows[0];

      await writeAuditEvent(client, {
        actor,
        action: "create",
        entityType: descriptor.entity,
        entityId: String(row.id),
        chainId: descriptor.chainScoped ? Number(row.chain_id) : null,
        before: null,
        after: row,
        hashBefore: null,
        hashAfter,
        idempotencyKey,
        result: "success",
        ip: req.ip,
        userAgent: req.header("user-agent") ?? undefined,
        traceId: (req.header("x-request-id") as string) ?? undefined,
      });

      await client.query("COMMIT");

      const evt: ReloadEvent = {
        event_id: randomUUID(),
        idempotency_key: idempotencyKey,
        resource: descriptor.entity,
        chain_id: descriptor.chainScoped ? Number(row.chain_id) : null,
        action: "create",
        entity_id: String(row.id),
        config_hash_before: null,
        config_hash_after: hashAfter,
        actor,
        timestamp: new Date().toISOString(),
      };
      await publishReload(deps.redis, descriptor, evt);
      res.status(201).json({ row, event: evt });
    } catch (e) {
      await client.query("ROLLBACK");
      res.status(500).json({ error: "registry_write_failed", detail: (e as Error).message });
    } finally {
      client.release();
    }
  });

  /** PATCH /:entity/:id — update with hash diff + audit + reload */
  router.patch(`${base}/:id`, async (req, res) => {
    // .partial() only exists on ZodObject. All registry schemas are objects
    // (see RpcDescriptor / ContractDescriptor below), so the cast is safe.
    const parse = (descriptor.schema as unknown as z.ZodObject<z.ZodRawShape>)
      .partial()
      .safeParse(req.body);
    if (!parse.success) {
      res.status(400).json({ error: "schema_violation", details: parse.error.format() });
      return;
    }
    const patch = parse.data as Record<string, unknown>;
    const idempotencyKey = String(
      req.header("Idempotency-Key") ?? req.body?.idempotency_key ?? randomUUID(),
    );
    const actor = deps.actorOf(req);

    const client = await deps.db.connect();
    try {
      await client.query("BEGIN");
      const prev = await client.query(
        `SELECT * FROM ${descriptor.table} WHERE id = $1 FOR UPDATE`,
        [req.params.id],
      );
      if (prev.rowCount === 0) {
        await client.query("ROLLBACK");
        res.status(404).json({ error: "not_found" });
        return;
      }
      const before = prev.rows[0];
      const merged = { ...before, ...patch };
      const hashAfter = computeConfigHash(merged, descriptor.hashColumns);
      if (hashAfter === before.config_hash) {
        await client.query("COMMIT");
        res.status(200).json({ idempotent: true, row: before });
        return;
      }

      const setCols = Object.keys(patch);
      const setExpr = setCols.map((c, i) => `${c} = $${i + 1}`).join(", ");
      const upd = await client.query(
        `UPDATE ${descriptor.table}
            SET ${setExpr}, config_hash = $${setCols.length + 1},
                updated_at = now(), updated_by = $${setCols.length + 2}
          WHERE id = $${setCols.length + 3}
          RETURNING *`,
        [...setCols.map((c) => patch[c]), hashAfter, actor, req.params.id],
      );
      const row = upd.rows[0];

      await writeAuditEvent(client, {
        actor,
        action: "update",
        entityType: descriptor.entity,
        entityId: String(row.id),
        chainId: descriptor.chainScoped ? Number(row.chain_id) : null,
        before,
        after: row,
        hashBefore: before.config_hash,
        hashAfter,
        idempotencyKey,
        result: "success",
      });
      await client.query("COMMIT");

      const evt: ReloadEvent = {
        event_id: randomUUID(),
        idempotency_key: idempotencyKey,
        resource: descriptor.entity,
        chain_id: descriptor.chainScoped ? Number(row.chain_id) : null,
        action: "update",
        entity_id: String(row.id),
        config_hash_before: before.config_hash,
        config_hash_after: hashAfter,
        actor,
        timestamp: new Date().toISOString(),
      };
      await publishReload(deps.redis, descriptor, evt);
      res.status(200).json({ row, event: evt });
    } catch (e) {
      await client.query("ROLLBACK");
      res.status(500).json({ error: "registry_update_failed", detail: (e as Error).message });
    } finally {
      client.release();
    }
  });

  /** DELETE /:entity/:id — soft-delete via status = deprecated */
  router.delete(`${base}/:id`, async (req, res) => {
    const idempotencyKey = String(
      req.header("Idempotency-Key") ?? randomUUID(),
    );
    const actor = deps.actorOf(req);
    const client = await deps.db.connect();
    try {
      await client.query("BEGIN");
      const prev = await client.query(
        `SELECT * FROM ${descriptor.table} WHERE id = $1 FOR UPDATE`,
        [req.params.id],
      );
      if (prev.rowCount === 0) {
        await client.query("ROLLBACK");
        res.status(404).json({ error: "not_found" });
        return;
      }
      const before = prev.rows[0];
      if (before.status === "deprecated") {
        await client.query("COMMIT");
        res.status(200).json({ idempotent: true, row: before });
        return;
      }
      const merged = { ...before, status: "deprecated", enabled: false };
      const hashAfter = computeConfigHash(merged, descriptor.hashColumns);
      const upd = await client.query(
        `UPDATE ${descriptor.table}
            SET status = 'deprecated', enabled = FALSE,
                config_hash = $1, updated_at = now(), updated_by = $2
          WHERE id = $3 RETURNING *`,
        [hashAfter, actor, req.params.id],
      );
      const row = upd.rows[0];
      await writeAuditEvent(client, {
        actor,
        action: "delete",
        entityType: descriptor.entity,
        entityId: String(row.id),
        chainId: descriptor.chainScoped ? Number(row.chain_id) : null,
        before,
        after: row,
        hashBefore: before.config_hash,
        hashAfter,
        idempotencyKey,
        result: "success",
      });
      await client.query("COMMIT");

      const evt: ReloadEvent = {
        event_id: randomUUID(),
        idempotency_key: idempotencyKey,
        resource: descriptor.entity,
        chain_id: descriptor.chainScoped ? Number(row.chain_id) : null,
        action: "delete",
        entity_id: String(row.id),
        config_hash_before: before.config_hash,
        config_hash_after: hashAfter,
        actor,
        timestamp: new Date().toISOString(),
      };
      await publishReload(deps.redis, descriptor, evt);
      res.status(200).json({ row, event: evt });
    } catch (e) {
      await client.query("ROLLBACK");
      res.status(500).json({ error: "registry_delete_failed", detail: (e as Error).message });
    } finally {
      client.release();
    }
  });

  return router;
}

// ---------------------------------------------------------------------------
// Canonical descriptors for the 6 new entities
// (chains, dexes, pools, tokens, wallets, strategies already exist)
// ---------------------------------------------------------------------------

export const RpcDescriptor: RegistryDescriptor<ZodTypeAny> = {
  entity: "rpc",
  table: "rpc_endpoints",
  chainScoped: true,
  reloadChannel: "arbx:config:rpcs",
  hashColumns: [
    "chain_id","url","transport","tier","auth_kind","auth_credential_id",
    "weight","rate_limit_rps","max_concurrency","enabled","status","notes",
  ],
  naturalKey: ["chain_id","url"],
  schema: z.object({
    chain_id: z.number().int().positive(),
    url: z.string().url(),
    transport: z.enum(["http","https","ws","wss","ipc"]),
    tier: z.enum(["primary","secondary","fallback","archive"]).default("primary"),
    auth_kind: z.enum(["none","header","bearer","basic","query"]).default("none"),
    auth_credential_id: z.string().uuid().nullable().optional(),
    weight: z.number().int().min(0).max(10000).default(100),
    rate_limit_rps: z.number().int().positive().nullable().optional(),
    max_concurrency: z.number().int().positive().default(16),
    enabled: z.boolean().default(true),
    status: z.enum(["active","paused","blocked","pending_validation","deprecated"]).default("pending_validation"),
    notes: z.string().max(2000).nullable().optional(),
  }),
};

export const ContractDescriptor: RegistryDescriptor<ZodTypeAny> = {
  entity: "contract",
  table: "contract_registry",
  chainScoped: true,
  reloadChannel: "arbx:config:contracts",
  hashColumns: [
    "chain_id","label","address","deployer","salt","init_code_hash",
    "abi_version","contract_kind","proxy_kind","implementation",
    "deploy_tx","deploy_block","verified","enabled","status","metadata",
  ],
  naturalKey: ["chain_id","label"],
  schema: z.object({
    chain_id: z.number().int().positive(),
    label: z.string().min(1).max(128),
    address: z.string().regex(/^0x[0-9a-fA-F]{40}$/),
    deployer: z.string().regex(/^0x[0-9a-fA-F]{40}$/),
    salt: z.string().regex(/^0x[0-9a-fA-F]{64}$/),
    init_code_hash: z.string().regex(/^0x[0-9a-fA-F]{64}$/),
    abi_version: z.string().min(1).max(32),
    contract_kind: z.enum([
      "factory","resolution_core","adapter_uniswap_v2","adapter_uniswap_v3",
      "adapter_balancer","adapter_curve","adapter_pancake_v3","adapter_gmx",
      "adapter_synthetix","flashloan_aave","flashloan_balancer","flashloan_dydx",
      "flashloan_uniswap_v3","wallet_topology","gas_sponsor","cold_treasury",
      "execution_signer_guard","allowance_manager","admin_timelock","custom",
    ]),
    proxy_kind: z.enum(["uups","transparent","beacon","minimal","none"]).nullable().optional(),
    implementation: z.string().regex(/^0x[0-9a-fA-F]{40}$/).nullable().optional(),
    deploy_tx: z.string().regex(/^0x[0-9a-fA-F]{64}$/).nullable().optional(),
    deploy_block: z.number().int().nonnegative().nullable().optional(),
    verified: z.boolean().default(false),
    enabled: z.boolean().default(true),
    status: z.enum(["active","paused","blocked","pending_validation","deprecated"]).default("pending_validation"),
    metadata: z.record(z.unknown()).default({}),
  }),
};

export const RiskGateDescriptor: RegistryDescriptor<ZodTypeAny> = {
  entity: "risk_gate",
  table: "risk_gates",
  chainScoped: false,
  reloadChannel: "arbx:config:risk",
  hashColumns: [
    "scope","scope_ref","gate_kind","threshold_value","comparator",
    "action_on_breach","enabled","status",
  ],
  schema: z.object({
    scope: z.enum(["global","chain","wallet","strategy","token","route","dex","time_window"]),
    scope_ref: z.string().max(256).nullable().optional(),
    gate_kind: z.enum([
      "min_confidence","max_slippage_bps","min_liquidity_usd","max_price_impact_bps",
      "min_spectral_gap","max_decoherence","blacklist_token","blacklist_route",
      "time_lock","circuit_breaker",
    ]),
    threshold_value: z.number(),
    comparator: z.enum(["lt","lte","gt","gte","eq","neq","in","nin"]),
    action_on_breach: z.enum(["reject","warn","throttle","pause_chain","pause_global"]),
    enabled: z.boolean().default(true),
    status: z.enum(["active","paused","blocked","pending_validation","deprecated"]).default("active"),
  }),
};

export const CapitalGateDescriptor: RegistryDescriptor<ZodTypeAny> = {
  entity: "capital_gate",
  table: "capital_gates",
  chainScoped: false,
  reloadChannel: "arbx:config:capital",
  hashColumns: [
    "scope","scope_ref","name","capital_cap_usd","max_gas_burn_usd",
    "max_drawdown_pct","max_consecutive_err","max_revert_rate_pct",
    "max_latency_ms","window_seconds","enabled","status",
  ],
  schema: z.object({
    scope: z.enum(["global","chain","wallet","strategy","token","route"]),
    scope_ref: z.string().max(256).nullable().optional(),
    name: z.string().min(1).max(128),
    capital_cap_usd: z.number().nonnegative().default(0),
    max_gas_burn_usd: z.number().nonnegative().default(0),
    max_drawdown_pct: z.number().min(0).max(100).default(0),
    max_consecutive_err: z.number().int().nonnegative().default(0),
    max_revert_rate_pct: z.number().min(0).max(100).default(0),
    max_latency_ms: z.number().int().nonnegative().default(0),
    window_seconds: z.number().int().nonnegative().default(0),
    enabled: z.boolean().default(true),
    status: z.enum(["active","paused","blocked","pending_validation","deprecated"]).default("active"),
  }),
};

export const RelayDescriptor: RegistryDescriptor<ZodTypeAny> = {
  entity: "relay",
  table: "relay_endpoints",
  chainScoped: true,
  reloadChannel: "arbx:config:relays",
  hashColumns: [
    "chain_id","name","url","relay_kind","auth_credential_id",
    "priority","timeout_ms","enabled","status",
  ],
  naturalKey: ["chain_id","name"],
  schema: z.object({
    chain_id: z.number().int().positive(),
    name: z.string().min(1).max(128),
    url: z.string().url(),
    relay_kind: z.enum(["flashbots","bloxroute","eden","manifold","public","custom"]),
    auth_credential_id: z.string().uuid().nullable().optional(),
    priority: z.number().int().min(0).max(10000).default(100),
    timeout_ms: z.number().int().positive().default(5000),
    enabled: z.boolean().default(true),
    status: z.enum(["active","paused","blocked","pending_validation","deprecated"]).default("pending_validation"),
  }),
};

export const AgentDescriptor: RegistryDescriptor<ZodTypeAny> = {
  entity: "agent",
  table: "agent_registry",
  chainScoped: false,
  reloadChannel: "arbx:config:agents",
  hashColumns: [
    "role","discipline","rank","active","capabilities","evidence_bindings","status",
  ],
  schema: z.object({
    role: z.string().min(1).max(128),
    discipline: z.string().min(1).max(128),
    rank: z.enum(["phd","nobel","principal","architect","auditor"]),
    active: z.boolean().default(true),
    capabilities: z.array(z.string()).default([]),
    evidence_bindings: z.array(z.string()).default([]),
    status: z.enum(["active","paused","blocked","pending_validation","deprecated"]).default("active"),
  }),
};

export const ALL_DESCRIPTORS = [
  RpcDescriptor,
  ContractDescriptor,
  RiskGateDescriptor,
  CapitalGateDescriptor,
  RelayDescriptor,
  AgentDescriptor,
] as const;
