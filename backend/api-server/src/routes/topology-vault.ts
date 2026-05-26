import { randomUUID, createHash } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import express, { type RequestHandler } from "express";
import { z } from "zod";

export type MempoolMode = "auto" | "filtered" | "firehose";

export interface TopologyProviderSnapshot {
  name: string;
  url_masked: string;
  scheme: "http" | "https" | "ws" | "wss";
  host: string;
  provider_kind: "alchemy" | "standard";
}

export interface TopologySnapshot {
  scope: string;
  chain_id: number;
  mempool_mode: MempoolMode;
  rpc_http_1: TopologyProviderSnapshot[];
  rpc_ws_1: TopologyProviderSnapshot[];
  checksum: string;
  version_id: number;
  updated_at: string;
}

export interface TopologyMutationRequest {
  scope?: string;
  chain_id?: number;
  mempool_mode: MempoolMode;
  rpc_http_1: string;
  rpc_ws_1: string;
}

export interface TopologyMutationCommitted {
  event: "topology.mutation.committed";
  mutation_id: string;
  version_id: number;
  scope: string;
  chain_id: number;
  mempool_mode: MempoolMode;
  rpc_http_1: string;
  rpc_ws_1: string;
  checksum: string;
  committed_at: string;
}

interface VaultRecord extends Required<TopologyMutationRequest> {
  version_id: number;
  updated_at: string;
  mutation_id: string;
  checksum: string;
}

export interface TopologyVaultDeps {
  redis: { publish(channel: string, message: string): Promise<number> };
  requireAdminToken: (token: string) => RequestHandler;
  adminToken: string;
  writeAudit?: (
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
  logger: {
    info: (obj: unknown, msg?: string) => void;
    warn: (obj: unknown, msg?: string) => void;
    error?: (obj: unknown, msg?: string) => void;
  };
  vaultPath?: string;
  channel?: string;
  now?: () => Date;
}

interface ParsedProvider {
  name: string;
  url: string;
  parsed: URL;
  scheme: "http" | "https" | "ws" | "wss";
}

const DEFAULT_SCOPE = "staging";
const DEFAULT_CHAIN_ID = 1;
export const TOPOLOGY_MUTATION_CHANNEL = "arbx:topology:mutation";

const MutationSchema = z.object({
  scope: z.string().trim().min(1).max(32).regex(/^[a-z0-9_-]+$/).default(DEFAULT_SCOPE),
  chain_id: z.number().int().positive().default(DEFAULT_CHAIN_ID),
  mempool_mode: z.enum(["auto", "filtered", "firehose"]),
  rpc_http_1: z.string().trim().min(8).max(8192),
  rpc_ws_1: z.string().trim().min(8).max(8192),
}).strict();

function defaultVaultPath(): string {
  return process.env["TOPOLOGY_VAULT_PATH"] ?? "/var/lib/arbx/topology-vault.staging.json";
}

function canonicalize(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalize).join(",")}]`;
  if (value && typeof value === "object") {
    const obj = value as Record<string, unknown>;
    return `{${Object.keys(obj).sort().map((k) => `${JSON.stringify(k)}:${canonicalize(obj[k])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

export function computeTopologyChecksum(value: unknown): string {
  return createHash("sha256").update(canonicalize(value)).digest("hex");
}

function isAllowedProviderScheme(url: URL, family: "http" | "ws"): boolean {
  if (family === "http") return url.protocol === "http:" || url.protocol === "https:";
  return url.protocol === "ws:" || url.protocol === "wss:";
}

const PROXY_HOST_PATTERNS = [
  /(^|\.)ngrok(-free)?\.app$/i,
  /(^|\.)trycloudflare\.com$/i,
  /(^|\.)workers\.dev$/i,
  /(^|\.)vercel\.app$/i,
  /(^|\.)netlify\.app$/i,
  /(^|\.)onrender\.com$/i,
  /(^|\.)fly\.dev$/i,
  /(^|\.)localtunnel\.me$/i,
  /(^|\.)serveo\.net$/i,
  /proxy/i,
];

export function isExplicitProxyUrl(url: URL): boolean {
  const host = url.hostname.toLowerCase();
  return PROXY_HOST_PATTERNS.some((rx) => rx.test(host));
}

export function isAlchemyProvider(p: { name: string; parsed: URL }): boolean {
  return p.name.toLowerCase() === "alchemy" || p.parsed.hostname.toLowerCase().includes("alchemy.com");
}

function parseProviderToken(token: string, family: "http" | "ws", index: number): ParsedProvider {
  const trimmed = token.trim();
  if (!trimmed) throw new Error("empty_provider");
  const eq = trimmed.indexOf("=");
  const name = eq > 0 ? trimmed.slice(0, eq).trim() : `provider${index + 1}`;
  const rawUrl = eq > 0 ? trimmed.slice(eq + 1).trim() : trimmed;
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(name)) throw new Error("invalid_provider_name");
  let parsed: URL;
  try { parsed = new URL(rawUrl); } catch { throw new Error("invalid_url"); }
  if (!isAllowedProviderScheme(parsed, family)) throw new Error(`invalid_${family}_scheme`);
  if (parsed.username || parsed.password) throw new Error("userinfo_forbidden");
  if (isExplicitProxyUrl(parsed)) throw new Error("proxy_forbidden");
  const scheme = parsed.protocol.slice(0, -1) as ParsedProvider["scheme"];
  return { name, url: rawUrl, parsed, scheme };
}

export function parseProviderList(input: string, family: "http" | "ws"): ParsedProvider[] {
  const providers = input.split(",").map((token, index) => parseProviderToken(token, family, index));
  if (providers.length === 0) throw new Error("no_providers");
  return providers;
}

export function maskUrl(rawUrl: string): string {
  const u = new URL(rawUrl);
  const suffixSource = `${u.pathname}${u.search}${u.hash}`;
  const compactSuffix = suffixSource.replace(/[^a-zA-Z0-9]/g, "");
  const suffix = compactSuffix.length >= 4 ? compactSuffix.slice(-4) : "set";
  const port = u.port ? `:${u.port}` : "";
  return `${u.protocol}//...${u.hostname}${port}/****${suffix}`;
}

function toProviderSnapshot(p: ParsedProvider): TopologyProviderSnapshot {
  return {
    name: p.name,
    url_masked: maskUrl(p.url),
    scheme: p.scheme,
    host: p.parsed.hostname,
    provider_kind: isAlchemyProvider(p) ? "alchemy" : "standard",
  };
}

function toSnapshot(record: VaultRecord): TopologySnapshot {
  return {
    scope: record.scope,
    chain_id: record.chain_id,
    mempool_mode: record.mempool_mode,
    rpc_http_1: parseProviderList(record.rpc_http_1, "http").map(toProviderSnapshot),
    rpc_ws_1: parseProviderList(record.rpc_ws_1, "ws").map(toProviderSnapshot),
    checksum: record.checksum,
    version_id: record.version_id,
    updated_at: record.updated_at,
  };
}

export function assertMutationCompatible(req: Required<TopologyMutationRequest>): void {
  parseProviderList(req.rpc_http_1, "http");
  const wsProviders = parseProviderList(req.rpc_ws_1, "ws");
  if (req.mempool_mode === "filtered" && !isAlchemyProvider(wsProviders[0]!)) {
    throw new Error("filtered_requires_first_ws_alchemy");
  }
}

function buildRecord(input: Required<TopologyMutationRequest>, now: Date, previousVersion = 0): VaultRecord {
  assertMutationCompatible(input);
  const version_id = Math.max(Math.floor(now.getTime() / 1000), previousVersion + 1);
  const checksumPayload = {
    scope: input.scope,
    chain_id: input.chain_id,
    mempool_mode: input.mempool_mode,
    rpc_http_1: input.rpc_http_1,
    rpc_ws_1: input.rpc_ws_1,
  };
  const checksum = computeTopologyChecksum(checksumPayload);
  return {
    ...input,
    version_id,
    updated_at: now.toISOString(),
    mutation_id: randomUUID(),
    checksum,
  };
}

async function readVault(vaultPath: string): Promise<VaultRecord | null> {
  try {
    const raw = await readFile(vaultPath, "utf8");
    return JSON.parse(raw) as VaultRecord;
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "ENOENT") return null;
    throw e;
  }
}

async function writeVault(vaultPath: string, record: VaultRecord): Promise<void> {
  await mkdir(path.dirname(vaultPath), { recursive: true, mode: 0o700 });
  const tmp = `${vaultPath}.${process.pid}.${Date.now()}.tmp`;
  await writeFile(tmp, `${JSON.stringify(record, null, 2)}\n`, { mode: 0o600 });
  await rename(tmp, vaultPath);
}

function buildCommittedEvent(record: VaultRecord): TopologyMutationCommitted {
  return {
    event: "topology.mutation.committed",
    mutation_id: record.mutation_id,
    version_id: record.version_id,
    scope: record.scope,
    chain_id: record.chain_id,
    mempool_mode: record.mempool_mode,
    rpc_http_1: record.rpc_http_1,
    rpc_ws_1: record.rpc_ws_1,
    checksum: record.checksum,
    committed_at: record.updated_at,
  };
}

function reqUA(req: express.Request): string | null {
  const ua = req.header("user-agent");
  return ua && ua.length > 0 ? ua : null;
}

function safeErrorCode(e: unknown): string {
  const msg = e instanceof Error ? e.message : "invalid_topology";
  if (/^[a-z0-9_]+$/.test(msg)) return msg;
  return "invalid_topology";
}

export function buildTopologyVaultRouter(deps: TopologyVaultDeps): express.Router {
  const router = express.Router();
  const adminGate = deps.requireAdminToken(deps.adminToken);
  const vaultPath = deps.vaultPath ?? defaultVaultPath();
  const channel = deps.channel ?? TOPOLOGY_MUTATION_CHANNEL;
  const now = deps.now ?? (() => new Date());

  router.get("/api/admin/topology/snapshot", adminGate, async (_req, res) => {
    try {
      const record = await readVault(vaultPath);
      if (!record) {
        res.status(200).json({
          ok: true,
          topology: null,
          ui: { background: "#FFFFFF", mode_options: ["auto", "filtered", "firehose"] },
          source: "empty_vault",
        });
        return;
      }
      res.status(200).json({
        ok: true,
        topology: toSnapshot(record),
        ui: { background: "#FFFFFF", mode_options: ["auto", "filtered", "firehose"] },
        source: "vault",
      });
    } catch (e) {
      deps.logger.warn({ event: "topology.snapshot.failed", err: (e as Error).message });
      res.status(500).json({ error: "topology_snapshot_failed" });
    }
  });

  router.post("/api/admin/topology/mutations", adminGate, async (req, res) => {
    const parsed = MutationSchema.safeParse(req.body ?? {});
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
      return;
    }

    try {
      const previous = await readVault(vaultPath);
      const record = buildRecord(parsed.data, now(), previous?.version_id ?? 0);
      const beforeSnapshot = previous ? toSnapshot(previous) : null;
      await writeVault(vaultPath, record);
      const event = buildCommittedEvent(record);
      const subscribers = await deps.redis.publish(channel, JSON.stringify(event));
      const afterSnapshot = toSnapshot(record);
      const actor = req.header("x-arbx-actor") ?? "admin";

      await deps.writeAudit?.(
        "topology.mutation.commit",
        actor,
        "topology",
        `${record.scope}:${record.chain_id}`,
        beforeSnapshot,
        { ...afterSnapshot, mutation_id: record.mutation_id, subscribers_notified: subscribers },
        req.ip ?? null,
        (req as express.Request & { traceId?: string }).traceId ?? null,
        reqUA(req),
      );

      deps.logger.info({
        event: "topology.mutation.committed",
        mutation_id: record.mutation_id,
        scope: record.scope,
        chain_id: record.chain_id,
        checksum: record.checksum,
        version_id: record.version_id,
        subscribers_notified: subscribers,
      }, "Topology Vault mutation committed");

      res.status(202).json({
        ok: true,
        mutation_id: record.mutation_id,
        version_id: record.version_id,
        published_to_channel: channel,
        subscribers_notified: subscribers,
        topology: afterSnapshot,
      });
    } catch (e) {
      const code = safeErrorCode(e);
      const status = code === "filtered_requires_first_ws_alchemy" || code.startsWith("invalid_") || code === "proxy_forbidden" || code === "userinfo_forbidden" ? 400 : 500;
      deps.logger.warn({ event: "topology.mutation.rejected", reason: code, error_message: e instanceof Error ? e.message : String(e), payload: req.body });
      res.status(status).json({ error: code });
    }
  });

  return router;
}

export const __forTesting = {
  MutationSchema,
  assertMutationCompatible,
  computeTopologyChecksum,
  isAlchemyProvider,
  isExplicitProxyUrl,
  maskUrl,
  parseProviderList,
  TOPOLOGY_MUTATION_CHANNEL,
};
