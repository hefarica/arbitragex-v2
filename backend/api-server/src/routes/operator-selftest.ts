/**
 * operator-selftest — the Operator Self-Test Center aggregator (READ-ONLY).
 *
 *   GET /api/operator/credentials/status → presence-only credential matrix
 *   GET /api/operator/selftest           → 10-block operator checklist
 *
 * SECURITY DOCTRINE (HARD INVARIANTS — never violated):
 *   • NO env VALUE ever appears in any response — not masked, not truncated,
 *     not fingerprinted/hashed. Presence is reported as a boolean ONLY.
 *   • live_enabled is STRUCTURALLY false, capital_exposed STRUCTURALLY 0,
 *     broadcast STRUCTURALLY false (reused from the wallet SAFE_POSTURE
 *     compile-time literals). Nothing here can flip them.
 *   • There is NO signer, NO private key, NO broadcast path in this module.
 *   • Live-future secrets (relay auth, KMS, multisig) are NEVER consumed in
 *     shadow; their rows report DORMANT_SEALED / ABSENT_BY_POLICY only.
 *
 * Fail-honest (RULE 00 / R8): every block is computed from REAL probes
 * (pg SELECT 1, redis PING, sim-ctl /health, real table counts, the live
 * readiness verifiers). A probe failure → FAILING with the sanitized error;
 * empty-but-healthy → HONEST_EMPTY; operator-gated → BLOCKED_*. NEVER a
 * fabricated VERIFIED. Probes run under Promise.allSettled with hard
 * per-probe timeouts so the endpoint degrades, never hangs.
 *
 * This surface AGGREGATES the existing pages (it links to them); it does not
 * duplicate their data planes.
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";

import { __forTesting as walletInternals } from "./wallet.js";

// ---------------------------------------------------------------------------
// Status taxonomy — the ONLY allowed block statuses. NEVER "SUCCESS" without
// evidence; honesty over green.
// ---------------------------------------------------------------------------

export const SELFTEST_STATUSES = [
  "VERIFIED",
  "MISSING",
  "PARTIAL",
  "FAILING",
  "HONEST_EMPTY",
  "DISABLED_BY_POLICY",
  "BLOCKED_REQUIRES_OPERATOR_SECRET",
  "BLOCKED_REQUIRES_OPERATOR_CONFIG",
  "BLOCKED_REQUIRES_MIGRATION",
  "BLOCKED_REQUIRES_TIME_WINDOW",
] as const;
export type SelfTestStatus = (typeof SELFTEST_STATUSES)[number];

/** Credential rows extend the taxonomy with the two sealed/policy states. */
export type CredentialStatus = SelfTestStatus | "DORMANT_SEALED" | "ABSENT_BY_POLICY";

export interface CredentialEntry {
  name: string;
  present: boolean; // presence ONLY — the value NEVER leaves the server.
  scope: "server" | "public_build_env";
  exposed_to_frontend: boolean;
  status: CredentialStatus;
  note?: string;
}

export interface SelfTestBlock {
  block: string;
  status: SelfTestStatus;
  evidence: string;
  last_checked_at: string;
  link: string;
  latency_ms?: number;
  operator_action?: string;
  days_remaining?: number;
}

// Safe posture — reused from the wallet surface's compile-time literals so the
// two surfaces can never drift apart.
const SAFE_POSTURE = walletInternals.SAFE_POSTURE;

const PROBE_TIMEOUT_MS = 4_000;
const PAPER_WINDOW_DAYS = 7;

// ---------------------------------------------------------------------------
// Credential matrix — names + rules ONLY. Values are read exclusively inside
// `isPresent` and reduced to a boolean on the spot.
// ---------------------------------------------------------------------------

type CredCategory =
  | "required" // shadow/paper required, server scope
  | "rpc" // per-chain RPC endpoints
  | "siwe_domain" // either-of group
  | "public_wc" // NEXT_PUBLIC_* build-time public id
  | "optional_data" // optional data/risk providers
  | "live_future"; // DORMANT/SEALED — never used in shadow

interface CredentialSpec {
  name: string;
  category: CredCategory;
}

export const CREDENTIAL_MATRIX: readonly CredentialSpec[] = [
  // shadow/paper required (server scope)
  { name: "DATABASE_URL", category: "required" },
  { name: "REDIS_URL", category: "required" },
  { name: "JWT_SECRET", category: "required" },
  { name: "ARBX_ADMIN_TOKEN", category: "required" },
  { name: "ARBX_SERVICE_TOKEN", category: "required" },
  // RPC per chain (1, 10, 56, 137, 8453, 42161)
  { name: "RPC_HTTP_1", category: "rpc" },
  { name: "RPC_WS_1", category: "rpc" },
  { name: "RPC_HTTP_10", category: "rpc" },
  { name: "RPC_WS_10", category: "rpc" },
  { name: "RPC_HTTP_56", category: "rpc" },
  { name: "RPC_WS_56", category: "rpc" },
  { name: "RPC_HTTP_137", category: "rpc" },
  { name: "RPC_WS_137", category: "rpc" },
  { name: "RPC_HTTP_8453", category: "rpc" },
  { name: "RPC_WS_8453", category: "rpc" },
  { name: "RPC_HTTP_42161", category: "rpc" },
  { name: "RPC_WS_42161", category: "rpc" },
  // SIWE domain — either of the two satisfies the group
  { name: "ARBX_WALLET_SIWE_DOMAIN", category: "siwe_domain" },
  { name: "PUBLIC_EDGE_HOST", category: "siwe_domain" },
  // WalletConnect — public build env, exposed to the frontend by design
  { name: "NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID", category: "public_wc" },
  // Optional data/risk providers
  { name: "TENDERLY_API_KEY", category: "optional_data" },
  { name: "GOPLUS_API_KEY", category: "optional_data" },
  { name: "HONEYPOT_IS_API_KEY", category: "optional_data" },
  // Live-future — DORMANT/SEALED: never used in shadow, never exposed
  { name: "FLASHBOTS_SIGNER_KEY", category: "live_future" },
  { name: "BLOXROUTE_AUTH", category: "live_future" },
  { name: "EDEN_AUTH", category: "live_future" },
  { name: "KMS_KEY_ID", category: "live_future" },
  { name: "SAFE_MULTISIG_ADDRESS", category: "live_future" },
  { name: "OPERATOR_APPROVAL_TOKEN", category: "live_future" },
] as const;

/** Presence check — the ONLY place an env value is touched. Returns a boolean
 *  immediately; the value itself never escapes this function. */
function isPresent(env: NodeJS.ProcessEnv, name: string): boolean {
  const raw = env[name];
  return typeof raw === "string" && raw.trim().length > 0;
}

function credentialStatus(
  category: CredCategory,
  present: boolean,
  siweGroupSatisfied: boolean,
): CredentialStatus {
  switch (category) {
    case "required":
      return present ? "VERIFIED" : "MISSING";
    case "rpc":
      return present ? "VERIFIED" : "PARTIAL";
    case "siwe_domain":
      return siweGroupSatisfied ? "VERIFIED" : "BLOCKED_REQUIRES_OPERATOR_CONFIG";
    case "public_wc":
      return present ? "VERIFIED" : "BLOCKED_REQUIRES_OPERATOR_SECRET";
    case "optional_data":
      return present ? "VERIFIED" : "HONEST_EMPTY";
    case "live_future":
      return present ? "DORMANT_SEALED" : "ABSENT_BY_POLICY";
  }
}

export function buildCredentialsMatrix(env: NodeJS.ProcessEnv): CredentialEntry[] {
  const siweGroupSatisfied = CREDENTIAL_MATRIX.filter((s) => s.category === "siwe_domain").some(
    (s) => isPresent(env, s.name),
  );
  return CREDENTIAL_MATRIX.map((spec) => {
    const present = isPresent(env, spec.name);
    const entry: CredentialEntry = {
      name: spec.name,
      present,
      scope: spec.category === "public_wc" ? "public_build_env" : "server",
      exposed_to_frontend: spec.category === "public_wc",
      status: credentialStatus(spec.category, present, siweGroupSatisfied),
    };
    if (spec.category === "siwe_domain" && !present && siweGroupSatisfied) {
      entry.note = "group satisfied by the other SIWE-domain variable";
    }
    if (spec.category === "live_future") {
      entry.note = "live-future credential — never consumed in shadow/paper; sealed by policy";
    }
    return entry;
  });
}

export interface CredentialsStatusResponse {
  mode: "shadow_paper";
  live_enabled: false;
  capital_exposed: 0;
  broadcast: false;
  policy: "presence_only_values_never_leave_the_server";
  secrets: CredentialEntry[];
  generated_at: string;
}

export function buildCredentialsResponse(
  env: NodeJS.ProcessEnv,
  now: Date,
): CredentialsStatusResponse {
  return {
    mode: "shadow_paper",
    live_enabled: SAFE_POSTURE.live_enabled,
    capital_exposed: SAFE_POSTURE.capital_exposed,
    broadcast: SAFE_POSTURE.broadcast,
    policy: "presence_only_values_never_leave_the_server",
    secrets: buildCredentialsMatrix(env),
    generated_at: now.toISOString(),
  };
}

// ---------------------------------------------------------------------------
// Helpers — timeout wrapper + error sanitizer (defense-in-depth: probe errors
// could embed connection URLs; we redact anything secret-shaped before it can
// reach an evidence string).
// ---------------------------------------------------------------------------

function withTimeout<T>(p: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`${label}_timeout_after_${ms}ms`)), ms);
    p.then(
      (v) => {
        clearTimeout(t);
        resolve(v);
      },
      (e) => {
        clearTimeout(t);
        reject(e);
      },
    );
  });
}

/** Redact URL-with-credentials, long hex, and JWT-shaped substrings from probe
 *  error messages. Evidence must NEVER carry secret-shaped material. */
export function sanitizeDetail(msg: string): string {
  return msg
    .replace(/[a-zA-Z][a-zA-Z0-9+.-]*:\/\/[^\s"']+/g, "[redacted-url]")
    .replace(/0x[0-9a-fA-F]{32,}/g, "[redacted-hex]")
    .replace(/eyJ[A-Za-z0-9_-]{10,}/g, "[redacted-token]")
    .slice(0, 240);
}

// ---------------------------------------------------------------------------
// Minimal structural deps — index.ts passes the real pg.Pool / ioredis client;
// tests pass fakes.
// ---------------------------------------------------------------------------

export interface RedisLike {
  ping(): Promise<string>;
  get(key: string): Promise<string | null>;
  /** Optional: full ioredis client supports KEYS. Used to scan per-chain papermode keys. */
  keys?(pattern: string): Promise<string[]>;
}

export interface ReadinessSummaryLike {
  summary: { green: number; yellow: number; red: number; pending: number; total: number };
  flip_blocked: boolean;
}

export interface OperatorSelfTestDeps {
  pool: pg.Pool | null;
  redis: RedisLike | null;
  logger: { warn: (obj: object, msg?: string) => void };
  /** Live-readiness rollup (block 10) — index.ts injects () => verifyAll({pool}). */
  readiness?: () => Promise<ReadinessSummaryLike>;
  env?: NodeJS.ProcessEnv;
  fetchImpl?: typeof fetch;
  now?: () => Date;
}

interface ProbeCtx {
  pool: pg.Pool | null;
  redis: RedisLike | null;
  env: NodeJS.ProcessEnv;
  fetchImpl: typeof fetch;
  readiness: (() => Promise<ReadinessSummaryLike>) | null;
  now: () => Date;
}

function block(
  slug: string,
  link: string,
  ctx: ProbeCtx,
  fields: {
    status: SelfTestStatus;
    evidence: string;
    latency_ms?: number;
    operator_action?: string;
    days_remaining?: number;
  },
): SelfTestBlock {
  const b: SelfTestBlock = {
    block: slug,
    status: fields.status,
    evidence: fields.evidence,
    last_checked_at: ctx.now().toISOString(),
    link,
  };
  if (fields.latency_ms !== undefined) b.latency_ms = fields.latency_ms;
  if (fields.operator_action !== undefined) b.operator_action = fields.operator_action;
  if (fields.days_remaining !== undefined) b.days_remaining = fields.days_remaining;
  return b;
}

async function tableExists(pool: pg.Pool, table: string): Promise<boolean> {
  const r = await pool.query("SELECT to_regclass($1) IS NOT NULL AS ok", [`public.${table}`]);
  return r.rows[0]?.ok === true;
}

// ---------------------------------------------------------------------------
// Block 1 — credentials roll-up (worst-of required; presence only).
// ---------------------------------------------------------------------------

export function probeCredentialsBlock(ctx: ProbeCtx): SelfTestBlock {
  const matrix = buildCredentialsMatrix(ctx.env);
  const byName = new Map(matrix.map((e) => [e.name, e]));
  const requiredSpecs = CREDENTIAL_MATRIX.filter((s) => s.category === "required");
  const requiredPresent = requiredSpecs.filter((s) => byName.get(s.name)?.present === true).length;
  const rpcSpecs = CREDENTIAL_MATRIX.filter((s) => s.category === "rpc");
  const rpcPresent = rpcSpecs.filter((s) => byName.get(s.name)?.present === true).length;
  const siweOk = CREDENTIAL_MATRIX.filter((s) => s.category === "siwe_domain").some(
    (s) => byName.get(s.name)?.status === "VERIFIED",
  );
  const wcPresent = byName.get("NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID")?.present === true;

  let status: SelfTestStatus;
  let operator_action: string | undefined;
  if (requiredPresent < requiredSpecs.length) {
    status = "MISSING";
    operator_action = "Set the absent required server env vars (see the credentials table below), then restart api-server.";
  } else if (!siweOk) {
    status = "BLOCKED_REQUIRES_OPERATOR_CONFIG";
    operator_action = "Set ARBX_WALLET_SIWE_DOMAIN or PUBLIC_EDGE_HOST so SIWE binds to the public domain.";
  } else if (rpcPresent < rpcSpecs.length) {
    status = "PARTIAL";
    operator_action = "Optional: add the absent per-chain RPC_HTTP_*/RPC_WS_* endpoints to widen chain coverage.";
  } else {
    status = "VERIFIED";
  }

  const evidence =
    `required ${requiredPresent}/${requiredSpecs.length} · rpc ${rpcPresent}/${rpcSpecs.length} · ` +
    `siwe_domain ${siweOk ? "ok" : "absent"} · walletconnect_project_id ${wcPresent ? "present" : "absent (BLOCKED_REQUIRES_OPERATOR_SECRET — expected today)"}`;

  const fields: Parameters<typeof block>[3] = { status, evidence };
  if (operator_action !== undefined) fields.operator_action = operator_action;
  return block("credentials", "/settings/credentials", ctx, fields);
}

// ---------------------------------------------------------------------------
// Block 2 — connections (pg SELECT 1 + redis PING, real latencies).
// ---------------------------------------------------------------------------

export async function probeConnections(ctx: ProbeCtx): Promise<SelfTestBlock> {
  let pgOk = false;
  let pgMs: number | null = null;
  let pgErr: string | null = null;
  if (ctx.pool) {
    const t0 = Date.now();
    try {
      await withTimeout(ctx.pool.query("SELECT 1"), PROBE_TIMEOUT_MS, "pg_select1");
      pgOk = true;
      pgMs = Date.now() - t0;
    } catch (e) {
      pgErr = sanitizeDetail((e as Error).message);
    }
  } else {
    pgErr = "pg pool not configured (DATABASE_URL absent)";
  }

  let redisOk = false;
  let redisMs: number | null = null;
  let redisErr: string | null = null;
  if (ctx.redis) {
    const t0 = Date.now();
    try {
      await withTimeout(ctx.redis.ping(), PROBE_TIMEOUT_MS, "redis_ping");
      redisOk = true;
      redisMs = Date.now() - t0;
    } catch (e) {
      redisErr = sanitizeDetail((e as Error).message);
    }
  } else {
    redisErr = "redis client not configured (REDIS_URL absent)";
  }

  const status: SelfTestStatus = pgOk && redisOk ? "VERIFIED" : pgOk || redisOk ? "PARTIAL" : "FAILING";
  const evidence =
    `pg SELECT 1 ${pgOk ? `ok ${pgMs}ms` : `failed: ${pgErr}`} · ` +
    `redis PING ${redisOk ? `ok ${redisMs}ms` : `failed: ${redisErr}`}`;

  const fields: Parameters<typeof block>[3] = { status, evidence };
  const best = pgMs ?? redisMs;
  if (best !== null) fields.latency_ms = Math.max(pgMs ?? 0, redisMs ?? 0);
  return block("connections", "/rpcs", ctx, fields);
}

// ---------------------------------------------------------------------------
// Block 3 — wallet safe posture (structural literals; reused, not re-derived).
// ---------------------------------------------------------------------------

export function probeWallet(ctx: ProbeCtx): SelfTestBlock {
  const safe =
    SAFE_POSTURE.live_enabled === false &&
    SAFE_POSTURE.capital_exposed === 0 &&
    SAFE_POSTURE.broadcast === false;
  // SAFE_POSTURE is `as const` literals, so `safe` is structurally true; the
  // FAILING branch exists as an honest tripwire should the constants ever drift.
  return block("wallet", "/wallet", ctx, {
    status: safe ? "VERIFIED" : "FAILING",
    evidence: safe
      ? "live_enabled=false · capital_exposed=0 · broadcast=false (compile-time literals; intents terminate at BROADCAST_DISABLED)"
      : "wallet safe posture drifted from the structural literals — investigate immediately",
  });
}

// ---------------------------------------------------------------------------
// Block 4 — settings (trading_config row exists for any chain).
// ---------------------------------------------------------------------------

export async function probeSettings(ctx: ProbeCtx): Promise<SelfTestBlock> {
  const link = "/config/trading";
  if (!ctx.pool) {
    return block("settings", link, ctx, {
      status: "FAILING",
      evidence: "cannot read trading_config — pg pool not configured (DATABASE_URL absent)",
    });
  }
  try {
    if (!(await withTimeout(tableExists(ctx.pool, "trading_config"), PROBE_TIMEOUT_MS, "settings_regclass"))) {
      return block("settings", link, ctx, {
        status: "BLOCKED_REQUIRES_MIGRATION",
        evidence: "trading_config table absent in Postgres",
        operator_action: "Apply the pending database migrations on this environment.",
      });
    }
    const r = await withTimeout(
      ctx.pool.query("SELECT COUNT(*)::int AS n FROM trading_config"),
      PROBE_TIMEOUT_MS,
      "settings_count",
    );
    const n = Number(r.rows[0]?.n ?? 0);
    if (n >= 1) {
      return block("settings", link, ctx, {
        status: "VERIFIED",
        evidence: `trading_config rows: ${n} (≥1 chain configured)`,
      });
    }
    return block("settings", link, ctx, {
      status: "BLOCKED_REQUIRES_OPERATOR_CONFIG",
      evidence: "trading_config table exists but holds 0 rows — no chain configured",
      operator_action: "Publish a trading config for at least one chain via /config/trading (admin PUT /admin/trading-config/<chain_id>).",
    });
  } catch (e) {
    return block("settings", link, ctx, {
      status: "FAILING",
      evidence: `trading_config probe failed: ${sanitizeDetail((e as Error).message)}`,
    });
  }
}

// ---------------------------------------------------------------------------
// Block 5 — data feeds (opportunities rows + 24h recency).
// ---------------------------------------------------------------------------

export async function probeDataFeeds(ctx: ProbeCtx): Promise<SelfTestBlock> {
  const link = "/opportunities";
  if (!ctx.pool) {
    return block("data_feeds", link, ctx, {
      status: "FAILING",
      evidence: "cannot read opportunities — pg pool not configured (DATABASE_URL absent)",
    });
  }
  try {
    if (!(await withTimeout(tableExists(ctx.pool, "opportunities"), PROBE_TIMEOUT_MS, "feeds_regclass"))) {
      return block("data_feeds", link, ctx, {
        status: "BLOCKED_REQUIRES_MIGRATION",
        evidence: "opportunities table absent in Postgres",
        operator_action: "Apply the pending database migrations on this environment.",
      });
    }
    const r = await withTimeout(
      ctx.pool.query(
        "SELECT COUNT(*)::int AS total, COUNT(*) FILTER (WHERE detected_at > NOW() - INTERVAL '24 hours')::int AS recent FROM opportunities",
      ),
      PROBE_TIMEOUT_MS,
      "feeds_count",
    );
    const total = Number(r.rows[0]?.total ?? 0);
    const recent = Number(r.rows[0]?.recent ?? 0);
    if (recent > 0) {
      return block("data_feeds", link, ctx, {
        status: "VERIFIED",
        evidence: `opportunities total ${total} · ${recent} detected in the last 24h`,
      });
    }
    if (total > 0) {
      return block("data_feeds", link, ctx, {
        status: "HONEST_EMPTY",
        evidence: `opportunities total ${total} but 0 in the last 24h — feed quiet or stalled (see R7 pipeline trace)`,
      });
    }
    return block("data_feeds", link, ctx, {
      status: "HONEST_EMPTY",
      evidence: "opportunities table exists but holds 0 rows — no detections persisted yet",
    });
  } catch (e) {
    return block("data_feeds", link, ctx, {
      status: "FAILING",
      evidence: `opportunities probe failed: ${sanitizeDetail((e as Error).message)}`,
    });
  }
}

// ---------------------------------------------------------------------------
// Block 6 — strategies (cartridge_registry count + state rollup).
// ---------------------------------------------------------------------------

export async function probeStrategies(ctx: ProbeCtx): Promise<SelfTestBlock> {
  const link = "/strategies/forge";
  if (!ctx.pool) {
    return block("strategies", link, ctx, {
      status: "FAILING",
      evidence: "cannot read cartridge_registry — pg pool not configured (DATABASE_URL absent)",
    });
  }
  try {
    const r = await withTimeout(
      ctx.pool.query(
        "SELECT state, COUNT(*)::int AS n FROM cartridge_registry GROUP BY state ORDER BY state",
      ),
      PROBE_TIMEOUT_MS,
      "strategies_count",
    );
    const rows = r.rows as Array<{ state?: unknown; n?: unknown }>;
    const total = rows.reduce((acc, row) => acc + Number(row.n ?? 0), 0);
    if (total === 0) {
      return block("strategies", link, ctx, {
        status: "HONEST_EMPTY",
        evidence: "cartridge_registry holds 0 cartridges — forge a strategy to populate it",
      });
    }
    const rollup = rows.map((row) => `${String(row.state)}:${Number(row.n ?? 0)}`).join(" ");
    return block("strategies", link, ctx, {
      status: "VERIFIED",
      evidence: `cartridge_registry total ${total} (${rollup})`,
    });
  } catch (e) {
    // 42P01 undefined_table → the registry was never migrated here. Honest empty
    // (the prompt-level contract), not a crash.
    if ((e as { code?: string }).code === "42P01") {
      return block("strategies", link, ctx, {
        status: "HONEST_EMPTY",
        evidence: "cartridge_registry table absent (42P01) — strategy forge not migrated on this environment",
      });
    }
    return block("strategies", link, ctx, {
      status: "FAILING",
      evidence: `cartridge_registry probe failed: ${sanitizeDetail((e as Error).message)}`,
    });
  }
}

// ---------------------------------------------------------------------------
// Block 7 — simulation (sim-ctl /health, same target the readiness verifier
// G-SIM-1 layer-1 checks; cheap fetch only).
// ---------------------------------------------------------------------------

export async function probeSimulation(ctx: ProbeCtx): Promise<SelfTestBlock> {
  const link = "/worker-health";
  const simUrl =
    ctx.env["SIM_URL"] ?? ctx.env["SIM_CTL_INTERNAL_URL"] ?? "http://sim-ctl:3003";
  const controller = new AbortController();
  const t = setTimeout(() => controller.abort(), PROBE_TIMEOUT_MS);
  const t0 = Date.now();
  try {
    const r = await ctx.fetchImpl(`${simUrl}/health`, { signal: controller.signal });
    const ms = Date.now() - t0;
    if (r.ok) {
      return block("simulation", link, ctx, {
        status: "VERIFIED",
        evidence: `sim-ctl /health HTTP ${r.status} in ${ms}ms`,
        latency_ms: ms,
      });
    }
    return block("simulation", link, ctx, {
      status: "FAILING",
      evidence: `sim-ctl /health responded HTTP ${r.status} — simulation runtime unhealthy`,
      latency_ms: ms,
    });
  } catch (e) {
    return block("simulation", link, ctx, {
      status: "FAILING",
      evidence: `sim-ctl /health unreachable: ${sanitizeDetail((e as Error).message)}`,
    });
  } finally {
    clearTimeout(t);
  }
}

// ---------------------------------------------------------------------------
// Block 8 — signals (route_discovery_outcomes recency).
// ---------------------------------------------------------------------------

export async function probeSignals(ctx: ProbeCtx): Promise<SelfTestBlock> {
  const link = "/route-outcomes";
  if (!ctx.pool) {
    return block("signals", link, ctx, {
      status: "FAILING",
      evidence: "cannot read route_discovery_outcomes — pg pool not configured (DATABASE_URL absent)",
    });
  }
  try {
    if (!(await withTimeout(tableExists(ctx.pool, "route_discovery_outcomes"), PROBE_TIMEOUT_MS, "signals_regclass"))) {
      return block("signals", link, ctx, {
        status: "BLOCKED_REQUIRES_MIGRATION",
        evidence: "route_discovery_outcomes table absent in Postgres",
        operator_action: "Apply the pending database migrations on this environment.",
      });
    }
    const r = await withTimeout(
      ctx.pool.query(
        "SELECT COUNT(*)::int AS total, MAX(ts_ms)::bigint AS last_ts FROM route_discovery_outcomes",
      ),
      PROBE_TIMEOUT_MS,
      "signals_count",
    );
    const total = Number(r.rows[0]?.total ?? 0);
    const lastTs = r.rows[0]?.last_ts != null ? Number(r.rows[0].last_ts) : null;
    const dayAgo = ctx.now().getTime() - 24 * 60 * 60 * 1000;
    if (total > 0 && lastTs !== null && lastTs > dayAgo) {
      return block("signals", link, ctx, {
        status: "VERIFIED",
        evidence: `route_discovery_outcomes total ${total} · latest within the last 24h`,
      });
    }
    if (total > 0) {
      return block("signals", link, ctx, {
        status: "HONEST_EMPTY",
        evidence: `route_discovery_outcomes total ${total} but none in the last 24h — shadow emitter quiet or stalled`,
      });
    }
    return block("signals", link, ctx, {
      status: "HONEST_EMPTY",
      evidence: "route_discovery_outcomes holds 0 rows — shadow outcome emitter has not persisted yet",
    });
  } catch (e) {
    return block("signals", link, ctx, {
      status: "FAILING",
      evidence: `route_discovery_outcomes probe failed: ${sanitizeDetail((e as Error).message)}`,
    });
  }
}

// ---------------------------------------------------------------------------
// Block 9 — paper window (redis arbx:papermode + MIN(detected_at) age ≥ 7d).
// ---------------------------------------------------------------------------

/**
 * Parse a papermode redis value. Accepts JSON `{enabled:true}` (B0.2 writer)
 * or bare legacy "1"/"true". Returns null when the value is absent/unparseable
 * so the caller can fall through to the next source.
 */
function parsePaperEnabled(raw: string | null): boolean | null {
  if (raw === null) return null;
  try {
    return (JSON.parse(raw) as { enabled?: unknown }).enabled === true;
  } catch {
    // bare key (legacy) — "1"/"true" = enabled, anything else = disabled
    return raw === "1" || raw === "true";
  }
}

/**
 * Read the armed paper-mode state from Redis. Mirrors g-pap-1.ts:
 *   1. legacy global `arbx:papermode` (read-only fallback)
 *   2. per-chain `arbx:papermode:<chain_id>` keys (the writer since B0.2,
 *      2026-05-13 — POST /admin/config/paper-mode arms per chain and no
 *      longer writes the global key)
 * TRUE if ANY source reports enabled=true (safe-side: never reports live
 * unless we have positive evidence paper is OFF). Returns the source label
 * for evidence. Returns {enabled:null} when redis is silent so the env
 * default applies.
 */
async function paperModeFromRedis(redis: RedisLike): Promise<{ enabled: boolean | null; source: string }> {
  // 1. legacy global key
  const globalRaw = await withTimeout(redis.get("arbx:papermode"), PROBE_TIMEOUT_MS, "paper_redis_get_global");
  const globalEnabled = parsePaperEnabled(globalRaw);
  if (globalEnabled === true) {
    return { enabled: true, source: "redis:arbx:papermode (legacy global)" };
  }
  // 2. per-chain keys (the real armed state for the configured chains)
  if (typeof redis.keys === "function") {
    const chainKeys = (await withTimeout(redis.keys("arbx:papermode:*"), PROBE_TIMEOUT_MS, "paper_redis_keys"))
      .filter((k) => !k.endsWith(":changes") && k !== "arbx:papermode");
    for (const k of chainKeys) {
      const v = parsePaperEnabled(await withTimeout(redis.get(k), PROBE_TIMEOUT_MS, "paper_redis_get_chain"));
      if (v === true) {
        return { enabled: true, source: `redis:${k}` };
      }
    }
    if (chainKeys.length > 0) {
      // per-chain keys exist but none enabled → honest OFF (positive evidence)
      return { enabled: false, source: `redis:arbx:papermode:* (${chainKeys.length} chain keys, all disabled)` };
    }
  }
  // redis silent → defer to env default
  return { enabled: globalEnabled === false ? false : null, source: globalRaw !== null ? "redis:arbx:papermode (legacy global, disabled)" : "env_default" };
}

export async function probePaper(ctx: ProbeCtx): Promise<SelfTestBlock> {
  const link = "/executions";
  let redisEnabled: boolean | null = null;
  let redisSource = "env_default";
  let redisNote = "";
  if (ctx.redis) {
    try {
      const r = await paperModeFromRedis(ctx.redis);
      redisEnabled = r.enabled;
      redisSource = r.source;
    } catch (e) {
      redisNote = ` (redis read failed: ${sanitizeDetail((e as Error).message)}; using env default)`;
    }
  } else {
    redisNote = " (redis client not configured; using env default)";
  }
  // Resolve final enabled flag: positive redis evidence wins; otherwise env default.
  let enabled: boolean;
  let source: string;
  if (redisEnabled !== null) {
    enabled = redisEnabled;
    source = redisSource;
  } else {
    const mode = ctx.env["ARBX_TRADE_MODE"];
    enabled = mode === undefined || mode === "paper";
    source = "env_default";
  }
  const paper = { enabled, source };
  if (!paper.enabled) {
    return block("paper", link, ctx, {
      status: "BLOCKED_REQUIRES_OPERATOR_CONFIG",
      evidence: `paper feed reported OFF by ${paper.source}${redisNote}`,
      operator_action: "Re-enable the paper feed (arbx:papermode {enabled:true} / ARBX_TRADE_MODE=paper).",
    });
  }
  if (!ctx.pool) {
    return block("paper", link, ctx, {
      status: "FAILING",
      evidence: `paper feed on (${paper.source})${redisNote} but cannot measure the observation window — pg pool not configured`,
    });
  }
  try {
    if (!(await withTimeout(tableExists(ctx.pool, "opportunities"), PROBE_TIMEOUT_MS, "paper_regclass"))) {
      return block("paper", link, ctx, {
        status: "BLOCKED_REQUIRES_MIGRATION",
        evidence: "opportunities table absent — paper window cannot be measured",
        operator_action: "Apply the pending database migrations on this environment.",
      });
    }
    const r = await withTimeout(
      ctx.pool.query("SELECT MIN(detected_at) AS first FROM opportunities"),
      PROBE_TIMEOUT_MS,
      "paper_min",
    );
    const first = r.rows[0]?.first ? new Date(String(r.rows[0].first)) : null;
    if (first === null || Number.isNaN(first.getTime())) {
      return block("paper", link, ctx, {
        status: "BLOCKED_REQUIRES_TIME_WINDOW",
        evidence: `paper feed on (${paper.source})${redisNote} but no observations persisted yet — the ${PAPER_WINDOW_DAYS}d window has not started`,
        days_remaining: PAPER_WINDOW_DAYS,
      });
    }
    const ageDays = (ctx.now().getTime() - first.getTime()) / 86_400_000;
    if (ageDays >= PAPER_WINDOW_DAYS) {
      return block("paper", link, ctx, {
        status: "VERIFIED",
        evidence: `paper feed on (${paper.source}) · oldest observation ${ageDays.toFixed(1)}d ago (≥${PAPER_WINDOW_DAYS}d window satisfied)`,
      });
    }
    const remaining = Math.max(1, Math.ceil(PAPER_WINDOW_DAYS - ageDays));
    return block("paper", link, ctx, {
      status: "BLOCKED_REQUIRES_TIME_WINDOW",
      evidence: `paper feed on (${paper.source}) · oldest observation ${ageDays.toFixed(1)}d ago — ${remaining}d remaining of the ${PAPER_WINDOW_DAYS}d window`,
      days_remaining: remaining,
    });
  } catch (e) {
    return block("paper", link, ctx, {
      status: "FAILING",
      evidence: `paper window probe failed: ${sanitizeDetail((e as Error).message)}`,
    });
  }
}

// ---------------------------------------------------------------------------
// Block 10 — live readiness rollup (the existing 17-verifier gate).
// ---------------------------------------------------------------------------

export async function probeLiveReadiness(ctx: ProbeCtx): Promise<SelfTestBlock> {
  const link = "/live-readiness";
  if (!ctx.readiness) {
    return block("live_readiness", link, ctx, {
      status: "FAILING",
      evidence: "readiness verifier not wired into this surface",
    });
  }
  try {
    const r = await withTimeout(ctx.readiness(), PROBE_TIMEOUT_MS * 2, "readiness_rollup");
    const { green, total } = r.summary;
    if (green === total && !r.flip_blocked) {
      return block("live_readiness", link, ctx, {
        status: "VERIFIED",
        evidence: `${green}/${total} readiness items green · flip_blocked=false`,
      });
    }
    return block("live_readiness", link, ctx, {
      status: "PARTIAL",
      evidence: `${green}/${total} readiness items green (yellow ${r.summary.yellow}, red ${r.summary.red}, pending ${r.summary.pending}) · flip_blocked=${r.flip_blocked}`,
    });
  } catch (e) {
    return block("live_readiness", link, ctx, {
      status: "FAILING",
      evidence: `readiness rollup failed: ${sanitizeDetail((e as Error).message)}`,
    });
  }
}

// ---------------------------------------------------------------------------
// Aggregator — Promise.allSettled so one bad probe NEVER takes the endpoint down.
// ---------------------------------------------------------------------------

export const SELFTEST_BLOCK_SLUGS = [
  "credentials",
  "connections",
  "wallet",
  "settings",
  "data_feeds",
  "strategies",
  "simulation",
  "signals",
  "paper",
  "live_readiness",
] as const;

const BLOCK_LINKS: Record<(typeof SELFTEST_BLOCK_SLUGS)[number], string> = {
  credentials: "/settings/credentials",
  connections: "/rpcs",
  wallet: "/wallet",
  settings: "/config/trading",
  data_feeds: "/opportunities",
  strategies: "/strategies/forge",
  simulation: "/worker-health",
  signals: "/route-outcomes",
  paper: "/executions",
  live_readiness: "/live-readiness",
};

export interface SelfTestResponse {
  mode: "shadow_paper";
  live_enabled: false;
  capital_exposed: 0;
  broadcast: false;
  generated_at: string;
  total_blocks: number;
  verified_count: number;
  blocks: SelfTestBlock[];
}

export async function runSelfTest(ctx: ProbeCtx): Promise<SelfTestResponse> {
  const probes: Array<Promise<SelfTestBlock>> = [
    Promise.resolve(probeCredentialsBlock(ctx)),
    probeConnections(ctx),
    Promise.resolve(probeWallet(ctx)),
    probeSettings(ctx),
    probeDataFeeds(ctx),
    probeStrategies(ctx),
    probeSimulation(ctx),
    probeSignals(ctx),
    probePaper(ctx),
    probeLiveReadiness(ctx),
  ];
  const settled = await Promise.allSettled(probes);
  const blocks = settled.map((s, i) => {
    const slug = SELFTEST_BLOCK_SLUGS[i]!;
    if (s.status === "fulfilled") return s.value;
    // A probe rejected past its own guards — surface it honestly as FAILING.
    return block(slug, BLOCK_LINKS[slug], ctx, {
      status: "FAILING",
      evidence: `probe rejected: ${sanitizeDetail((s.reason as Error)?.message ?? "unknown")}`,
    });
  });
  return {
    mode: "shadow_paper",
    live_enabled: SAFE_POSTURE.live_enabled,
    capital_exposed: SAFE_POSTURE.capital_exposed,
    broadcast: SAFE_POSTURE.broadcast,
    generated_at: ctx.now().toISOString(),
    total_blocks: blocks.length,
    verified_count: blocks.filter((b) => b.status === "VERIFIED").length,
    blocks,
  };
}

// ---------------------------------------------------------------------------
// Route mounting — public read surface behind the existing globalLimiter
// (applied in index.ts before all data routes).
// ---------------------------------------------------------------------------

export function mountOperatorSelfTest(app: Application, deps: OperatorSelfTestDeps): void {
  const ctx: ProbeCtx = {
    pool: deps.pool,
    redis: deps.redis,
    env: deps.env ?? process.env,
    fetchImpl: deps.fetchImpl ?? fetch,
    readiness: deps.readiness ?? null,
    now: deps.now ?? (() => new Date()),
  };

  app.get("/api/operator/credentials/status", (_req: Request, res: Response) => {
    try {
      res.status(200).json(buildCredentialsResponse(ctx.env, ctx.now()));
    } catch (e) {
      deps.logger.warn({ event: "operator_selftest.credentials_failed", err: (e as Error).message });
      res.status(503).json({ error: "credentials_status_failed" });
    }
  });

  app.get("/api/operator/selftest", async (_req: Request, res: Response) => {
    try {
      res.status(200).json(await runSelfTest(ctx));
    } catch (e) {
      deps.logger.warn({ event: "operator_selftest.failed", err: (e as Error).message });
      res.status(503).json({ error: "selftest_failed", detail: sanitizeDetail((e as Error).message) });
    }
  });
}

// Test-only exports.
export const __forTesting = {
  buildCredentialsMatrix,
  buildCredentialsResponse,
  credentialStatus,
  sanitizeDetail,
  runSelfTest,
  probeCredentialsBlock,
  probeConnections,
  probeWallet,
  probeSettings,
  probeDataFeeds,
  probeStrategies,
  probeSimulation,
  probeSignals,
  probePaper,
  probeLiveReadiness,
  SELFTEST_BLOCK_SLUGS,
  CREDENTIAL_MATRIX,
};
