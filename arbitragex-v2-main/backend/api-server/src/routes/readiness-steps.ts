/**
 * readiness-steps — server-side, READ-ONLY, REDACTED evaluator for the 4-step
 * "Live Readiness" stepper the operator sees at /live-readiness.
 *
 *   GET /api/v1/readiness/steps   (edge alias: /api/readiness/steps)
 *
 * WHY THIS EXISTS (audit 2026-06-08): the frontend stepper used to derive its
 * "N/4" headline from `localStorage` (Steps 2-4) plus an ADMIN-gated topology
 * snapshot (Step 1, which returns 401 to a normal operator browser). That made
 * the readiness count a CLIENT-SIDE FICTION — the same backend could show 4/4 in
 * one browser and 0/4 in another. RULE 00 / zero-mocks forbids that. This route
 * moves all four signals SERVER-SIDE, evaluated from real evidence, so the count
 * reflects the actual system state regardless of which browser asks.
 *
 * The four steps (cascade: step N is BLOCKED until step N-1 PASSES):
 *   1. Topology Vault   — ≥1 active WSS + ≥1 active RPC provider in the vault.
 *   2. Credentials      — server-side signer / authorized key / private relay
 *                         PRESENT (redacted: only "present"|null is ever emitted).
 *   3. Market Topology  — chains + dexes + pools registered in Postgres.
 *   4. Resolution Engines — ≥1 enabled strategy (paper/shadow ready; never live).
 *
 * Honesty contract (RULE 00 / R8 fail-honest):
 *   - PASS requires REAL evidence; absence → PENDING (if unblocked) or BLOCKED.
 *   - A hard read error (vault throw / DB down) → FAIL on that step, never a PASS.
 *   - Secrets are NEVER emitted: env vars report presence only ("present"|null);
 *     topology URLs pass through the canonical maskUrl; markets/engines are counts
 *     and public strategy slugs only.
 *   - live_enabled is STRUCTURALLY false. This route cannot toggle anything.
 *
 * This route is READ-ONLY; it never writes and never flips a flag.
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";

import { loadTopologySnapshot, type TopologySnapshot } from "./topology-vault.js";

// ---------------------------------------------------------------------------
// Wire contract — frontend Zod schema mirrors these exactly.
// ---------------------------------------------------------------------------

export type StepStatus = "PASS" | "PENDING" | "BLOCKED" | "FAIL";

export interface StepEvidence {
  label: string;
  // Redacted, human-readable detail. NEVER a raw secret/url/key.
  detail: string;
}

interface StepCommon {
  step: number;
  id: "topology" | "credentials" | "markets" | "engines";
  name: string;
  status: StepStatus;
  ready: boolean;
  reason: string;
  evidence: StepEvidence[];
}

export interface TopologyStep extends StepCommon {
  id: "topology";
  wss_active_count: number;
  rpc_active_count: number;
  snapshot_non_empty: boolean;
}

export interface CredentialsStep extends StepCommon {
  id: "credentials";
  signer: "present" | null;
  private_key: "present" | null;
  private_relay: "present" | null;
  redacted: true;
}

export interface MarketsStep extends StepCommon {
  id: "markets";
  chains_count: number | null;
  dexes_count: number | null;
  pools_count: number | null;
  tokens_count: number | null;
}

export interface EnginesStep extends StepCommon {
  id: "engines";
  engines_active: string[];
  paper_ready: boolean;
  shadow_ready: boolean;
  live_enabled: false;
}

export type ReadinessStep = TopologyStep | CredentialsStep | MarketsStep | EnginesStep;

export interface ReadinessStepsResponse {
  generated_at: string;
  source: "runtime_evidence";
  mode: "paper_only";
  completed_steps: number;
  total_steps: 4;
  readiness_score: string; // "N/4"
  all_ready: boolean;
  live_enabled: false;
  capital_exposure_usd: 0;
  steps: ReadinessStep[];
}

// ---------------------------------------------------------------------------
// Env probes — presence only, NEVER the value. A step "passes" on presence; the
// raw secret is never read into the response.
// ---------------------------------------------------------------------------

// First match wins; reported as a single "present"|null. Documented so the
// operator knows which vars satisfy each credential slot.
export const SIGNER_ENV_VARS = ["SIM_SIGNER_ADDRESS", "SIGNER_ADDRESS"] as const;
export const PRIVATE_KEY_ENV_VARS = [
  "EXECUTOR_SIGNER_KEY",
  "SIM_SIGNER_KEY",
  "ARBX_SIGNER_PRIVATE_KEY",
  "PRIVATE_KEY",
] as const;
export const PRIVATE_RELAY_ENV_VARS = [
  "PRIVATE_RELAY_URL",
  "FLASHBOTS_RELAY_URL",
  "ARBX_PRIVATE_RELAY_URL",
  "MEV_RELAY_URL",
] as const;

function anyEnvPresent(names: readonly string[]): boolean {
  return names.some((n) => {
    const raw = process.env[n];
    return typeof raw === "string" && raw.length > 0;
  });
}

function presentOrNull(present: boolean): "present" | null {
  return present ? "present" : null;
}

// ---------------------------------------------------------------------------
// Pure step evaluators — intrinsic readiness, BEFORE the cascade is applied.
// Each returns { ready, fail, ...fields, reason, evidence }. Exported for tests.
// ---------------------------------------------------------------------------

export interface TopologyInput {
  ok: boolean; // false → hard read error
  snapshot: TopologySnapshot | null;
  error: string | null;
}

export function evalTopology(t: TopologyInput): Omit<TopologyStep, "step" | "id" | "name" | "status"> & { fail: boolean } {
  if (!t.ok) {
    return {
      ready: false,
      fail: true,
      wss_active_count: 0,
      rpc_active_count: 0,
      snapshot_non_empty: false,
      reason: `topology_vault_read_error: ${t.error ?? "unknown"}`,
      evidence: [{ label: "vault", detail: "vault read failed — cannot evaluate ingest manifolds" }],
    };
  }
  const snap = t.snapshot;
  const http = Array.isArray(snap?.rpc_http_1) ? snap!.rpc_http_1 : [];
  const ws = Array.isArray(snap?.rpc_ws_1) ? snap!.rpc_ws_1 : [];
  const wssActive = ws.filter((p) => p.scheme === "wss" && Boolean(p.host)).length;
  const rpcActive = http.filter((p) => (p.scheme === "https" || p.scheme === "http") && Boolean(p.host)).length;
  const nonEmpty = http.length > 0 || ws.length > 0;
  const ready = wssActive >= 1 && rpcActive >= 1;
  const evidence: StepEvidence[] = [];
  for (const p of ws) evidence.push({ label: `wss:${p.name}`, detail: `${p.host} (${p.url_masked})` });
  for (const p of http) evidence.push({ label: `rpc:${p.name}`, detail: `${p.host} (${p.url_masked})` });
  const reason = ready
    ? `${wssActive} active WSS + ${rpcActive} active RPC provider(s) in topology vault (version ${snap?.version_id ?? "n/a"}).`
    : !nonEmpty
      ? "topology_vault_empty: no non-empty snapshot persisted. Publish ≥1 RPC + ≥1 WSS provider in the Topology Vault."
      : "topology_no_active_wss: snapshot present but missing an active WSS (or RPC) provider.";
  return { ready, fail: false, wss_active_count: wssActive, rpc_active_count: rpcActive, snapshot_non_empty: nonEmpty, reason, evidence };
}

export interface CredentialsInput {
  signerPresent: boolean;
  privateKeyPresent: boolean;
  privateRelayPresent: boolean;
}

export function evalCredentials(c: CredentialsInput): Omit<CredentialsStep, "step" | "id" | "name" | "status"> & { fail: boolean } {
  // PASS requires a server-side signer OR an authorized private key. A private
  // relay alone is not enough (it is the venue, not the authority).
  const ready = c.signerPresent || c.privateKeyPresent;
  const evidence: StepEvidence[] = [
    { label: "signer", detail: c.signerPresent ? "present (redacted)" : "absent" },
    { label: "private_key", detail: c.privateKeyPresent ? "present (redacted)" : "absent" },
    { label: "private_relay", detail: c.privateRelayPresent ? "present (redacted)" : "absent" },
  ];
  const reason = ready
    ? "Authorized signer/key present server-side (redacted). Credentials slot satisfied."
    : "credentials_absent: no server-side signer or authorized key detected. Configure SIM_SIGNER_ADDRESS or an authorized key (paper/shadow needs no live key).";
  return {
    ready,
    fail: false,
    signer: presentOrNull(c.signerPresent),
    private_key: presentOrNull(c.privateKeyPresent),
    private_relay: presentOrNull(c.privateRelayPresent),
    redacted: true,
    reason,
    evidence,
  };
}

export interface MarketsInput {
  chains: number | null;
  dexes: number | null;
  pools: number | null;
  tokens: number | null;
  dbError: boolean;
}

export function evalMarkets(m: MarketsInput): Omit<MarketsStep, "step" | "id" | "name" | "status"> & { fail: boolean } {
  if (m.dbError) {
    return {
      ready: false,
      fail: true,
      chains_count: m.chains,
      dexes_count: m.dexes,
      pools_count: m.pools,
      tokens_count: m.tokens,
      reason: "market_topology_db_unavailable: Postgres registry could not be read.",
      evidence: [{ label: "db", detail: "registry query failed — cannot count chains/dexes/pools/tokens" }],
    };
  }
  const ready = (m.chains ?? 0) >= 1 && (m.dexes ?? 0) >= 1 && (m.pools ?? 0) >= 1;
  const reason = ready
    ? `Market topology registered: ${m.chains} chain(s), ${m.dexes} dex(es), ${m.pools} pool(s), ${m.tokens ?? 0} token(s).`
    : "market_topology_incomplete: need ≥1 chain, ≥1 dex and ≥1 pool registered. Seed chains/dexes/pools via the admin registries.";
  return {
    ready,
    fail: false,
    chains_count: m.chains,
    dexes_count: m.dexes,
    pools_count: m.pools,
    tokens_count: m.tokens,
    reason,
    evidence: [
      { label: "chains", detail: String(m.chains ?? "n/a") },
      { label: "dexes", detail: String(m.dexes ?? "n/a") },
      { label: "pools", detail: String(m.pools ?? "n/a") },
      { label: "tokens", detail: String(m.tokens ?? "n/a") },
    ],
  };
}

export interface EnginesInput {
  enginesActive: string[];
  paperMode: boolean;
  shadowMode: boolean;
  dbError: boolean;
}

export function evalEngines(e: EnginesInput): Omit<EnginesStep, "step" | "id" | "name" | "status"> & { fail: boolean } {
  if (e.dbError) {
    return {
      ready: false,
      fail: true,
      engines_active: [],
      paper_ready: false,
      shadow_ready: false,
      live_enabled: false,
      reason: "resolution_engines_db_unavailable: could not read enabled strategies.",
      evidence: [{ label: "db", detail: "trading_config query failed — cannot enumerate active engines" }],
    };
  }
  const ready = e.enginesActive.length >= 1;
  return {
    ready,
    fail: false,
    engines_active: e.enginesActive,
    paper_ready: ready && e.paperMode,
    shadow_ready: ready && e.shadowMode,
    live_enabled: false,
    reason: ready
      ? `${e.enginesActive.length} resolution engine(s) enabled: ${e.enginesActive.join(", ")}. paper=${e.paperMode} shadow=${e.shadowMode}, live disabled.`
      : "resolution_engines_off: no enabled strategy. Enable ≥1 engine (SVS/DLP/Backrun/...) in trading_config.enabled_strategies.",
    evidence: e.enginesActive.length
      ? e.enginesActive.map((s) => ({ label: "engine", detail: s }))
      : [{ label: "engines", detail: "none enabled" }],
  };
}

// ---------------------------------------------------------------------------
// Cascade: step N is BLOCKED until step N-1 PASSES. A FAIL/PENDING is only
// surfaced when the step is actually reachable (prior step passed). Step 1 is
// always reachable.
// ---------------------------------------------------------------------------

export function statusFor(ready: boolean, fail: boolean, prevPass: boolean): StepStatus {
  if (!prevPass) return "BLOCKED";
  if (fail) return "FAIL";
  return ready ? "PASS" : "PENDING";
}

export interface StepInputs {
  topology: TopologyInput;
  credentials: CredentialsInput;
  markets: MarketsInput;
  engines: EnginesInput;
}

const STEP_NAMES = {
  topology: "Topology Vault",
  credentials: "Credentials / Wallets",
  markets: "Market Topology",
  engines: "Resolution Engines",
} as const;

export function assembleSteps(inputs: StepInputs): ReadinessStep[] {
  const t = evalTopology(inputs.topology);
  const c = evalCredentials(inputs.credentials);
  const m = evalMarkets(inputs.markets);
  const e = evalEngines(inputs.engines);

  const s1 = statusFor(t.ready, t.fail, true);
  const s2 = statusFor(c.ready, c.fail, s1 === "PASS");
  const s3 = statusFor(m.ready, m.fail, s2 === "PASS");
  const s4 = statusFor(e.ready, e.fail, s3 === "PASS");

  const blockedReason = (prevName: string): string => `Blocked until "${prevName}" passes.`;

  const topology: TopologyStep = {
    step: 1,
    id: "topology",
    name: STEP_NAMES.topology,
    status: s1,
    ready: s1 === "PASS",
    wss_active_count: t.wss_active_count,
    rpc_active_count: t.rpc_active_count,
    snapshot_non_empty: t.snapshot_non_empty,
    reason: t.reason,
    evidence: t.evidence,
  };
  const credentials: CredentialsStep = {
    step: 2,
    id: "credentials",
    name: STEP_NAMES.credentials,
    status: s2,
    ready: s2 === "PASS",
    signer: c.signer,
    private_key: c.private_key,
    private_relay: c.private_relay,
    redacted: true,
    reason: s2 === "BLOCKED" ? blockedReason(STEP_NAMES.topology) : c.reason,
    evidence: c.evidence,
  };
  const markets: MarketsStep = {
    step: 3,
    id: "markets",
    name: STEP_NAMES.markets,
    status: s3,
    ready: s3 === "PASS",
    chains_count: m.chains_count,
    dexes_count: m.dexes_count,
    pools_count: m.pools_count,
    tokens_count: m.tokens_count,
    reason: s3 === "BLOCKED" ? blockedReason(STEP_NAMES.credentials) : m.reason,
    evidence: m.evidence,
  };
  const engines: EnginesStep = {
    step: 4,
    id: "engines",
    name: STEP_NAMES.engines,
    status: s4,
    ready: s4 === "PASS",
    engines_active: e.engines_active,
    paper_ready: e.paper_ready,
    shadow_ready: e.shadow_ready,
    live_enabled: false,
    reason: s4 === "BLOCKED" ? blockedReason(STEP_NAMES.markets) : e.reason,
    evidence: e.evidence,
  };

  return [topology, credentials, markets, engines];
}

export function buildResponse(steps: ReadinessStep[], now: Date): ReadinessStepsResponse {
  const completed = steps.filter((s) => s.status === "PASS").length;
  return {
    generated_at: now.toISOString(),
    source: "runtime_evidence",
    mode: "paper_only",
    completed_steps: completed,
    total_steps: 4,
    readiness_score: `${completed}/4`,
    all_ready: completed === 4,
    live_enabled: false,
    capital_exposure_usd: 0,
    steps,
  };
}

// ---------------------------------------------------------------------------
// Async evidence gatherers (vault file + env + Postgres). Each is fail-honest:
// a thrown query degrades to an error flag / null counts, NEVER a fake PASS.
// ---------------------------------------------------------------------------

async function gatherTopology(): Promise<TopologyInput> {
  try {
    const snapshot = await loadTopologySnapshot();
    return { ok: true, snapshot, error: null };
  } catch (err) {
    return { ok: false, snapshot: null, error: (err as Error).message };
  }
}

function gatherCredentials(): CredentialsInput {
  return {
    signerPresent: anyEnvPresent(SIGNER_ENV_VARS),
    privateKeyPresent: anyEnvPresent(PRIVATE_KEY_ENV_VARS),
    privateRelayPresent: anyEnvPresent(PRIVATE_RELAY_ENV_VARS),
  };
}

async function countTable(pool: pg.Pool, table: string): Promise<number | null> {
  // to_regclass guard so a missing table degrades to null (not a 500).
  const exists = await pool.query("SELECT to_regclass($1) IS NOT NULL AS ok", [`public.${table}`]);
  if (exists.rows[0]?.ok !== true) return null;
  const r = await pool.query(`SELECT COUNT(*)::int AS n FROM ${table}`);
  return r.rows[0]?.n ?? 0;
}

async function gatherMarkets(
  pool: pg.Pool | null,
  logger: { warn: (obj: object, msg?: string) => void },
): Promise<MarketsInput> {
  if (!pool) return { chains: null, dexes: null, pools: null, tokens: null, dbError: true };
  try {
    const [chains, dexes, pools, tokens] = await Promise.all([
      countTable(pool, "chains"),
      countTable(pool, "dexes"),
      countTable(pool, "pools"),
      countTable(pool, "tokens"),
    ]);
    return { chains, dexes, pools, tokens, dbError: false };
  } catch (err) {
    logger.warn({ event: "readiness_steps.markets_err", err: (err as Error).message });
    return { chains: null, dexes: null, pools: null, tokens: null, dbError: true };
  }
}

function isPaperMode(): boolean {
  const v = process.env["ARBX_TRADE_MODE"];
  return v === undefined ? true : v === "paper";
}

function isShadowMode(): boolean {
  const v = (process.env["ARBX_ROUTE_DISCOVERY_MODE"] ?? "").toLowerCase();
  return v === "shadow" || v === "active" || v === "on";
}

async function gatherEngines(
  pool: pg.Pool | null,
  logger: { warn: (obj: object, msg?: string) => void },
): Promise<EnginesInput> {
  const paperMode = isPaperMode();
  const shadowMode = isShadowMode();
  if (!pool) return { enginesActive: [], paperMode, shadowMode, dbError: true };
  try {
    const exists = await pool.query("SELECT to_regclass('public.trading_config') IS NOT NULL AS ok");
    if (exists.rows[0]?.ok !== true) return { enginesActive: [], paperMode, shadowMode, dbError: false };
    const r = await pool.query(
      "SELECT DISTINCT s AS engine FROM trading_config t, unnest(t.enabled_strategies) AS s WHERE t.enabled = TRUE ORDER BY s",
    );
    const enginesActive = r.rows.map((row) => String(row.engine)).filter((s) => s.length > 0);
    return { enginesActive, paperMode, shadowMode, dbError: false };
  } catch (err) {
    logger.warn({ event: "readiness_steps.engines_err", err: (err as Error).message });
    return { enginesActive: [], paperMode, shadowMode, dbError: true };
  }
}

// ---------------------------------------------------------------------------
// Test-only exports — pure functions, asserted without Express/PG/file IO.
// ---------------------------------------------------------------------------

export const __forTesting = {
  evalTopology,
  evalCredentials,
  evalMarkets,
  evalEngines,
  statusFor,
  assembleSteps,
  buildResponse,
  anyEnvPresent,
  presentOrNull,
  SIGNER_ENV_VARS,
  PRIVATE_KEY_ENV_VARS,
  PRIVATE_RELAY_ENV_VARS,
};

// ---------------------------------------------------------------------------
// Route mounting.
// ---------------------------------------------------------------------------

export function mountReadinessSteps(
  app: Application,
  deps: { pool: pg.Pool | null; logger: { warn: (obj: object, msg?: string) => void } },
): void {
  app.get("/api/v1/readiness/steps", async (_req: Request, res: Response) => {
    try {
      const [topology, markets, engines] = await Promise.all([
        gatherTopology(),
        gatherMarkets(deps.pool, deps.logger),
        gatherEngines(deps.pool, deps.logger),
      ]);
      const credentials = gatherCredentials();
      const steps = assembleSteps({ topology, credentials, markets, engines });
      res.status(200).json(buildResponse(steps, new Date()));
    } catch (e) {
      deps.logger.warn({ event: "readiness_steps.failed", err: (e as Error).message });
      res.status(503).json({ error: "readiness_steps_collection_failed", detail: (e as Error).message });
    }
  });
}
