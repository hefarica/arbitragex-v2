export type PaperModeConfidence =
  | "explicit"
  | "explicit_legacy"
  | "observed"
  | "inferred"
  | "default_safe";

export type ChainPaperMode = {
  chain_id: number;
  enabled: boolean;
  source: "redis" | "env" | "default";
  confidence: PaperModeConfidence;
  conflict: boolean;
  updated_at: string | null;
};

export type PaperModeState = {
  enabled: boolean;
  chain_id: number | null;
  source: "redis" | "env" | "default";
  confidence: PaperModeConfidence;
  degraded: boolean;
  conflict: boolean;
  updated_at: string | null;
  reasons: string[];
  chains: ChainPaperMode[];
};

export interface ResolvePaperModeStateDeps {
  redis: Pick<RedisLike, "mget"> | null;
  env?: NodeJS.ProcessEnv;
  enabledChainIds: number[];
  chainId?: number | null;
  logger?: { warn?: (obj: object, msg?: string) => void };
}

export interface RedisLike {
  mget: (...keys: string[]) => Promise<(string | null)[]>;
}

const GLOBAL_KEY = "arbx:papermode";

function chainKey(chainId: number): string {
  return `arbx:papermode:chain:${chainId}`;
}

function parseEnabled(raw: string | null): boolean | null {
  if (raw === null) return null;
  try {
    const parsed = JSON.parse(raw) as { enabled?: unknown; updated_at?: string };
    if (typeof parsed.enabled === "boolean") return parsed.enabled;
    if (typeof parsed.enabled === "string")
      return parsed.enabled === "true" || parsed.enabled === "1";
    if (typeof parsed.enabled === "number") return parsed.enabled === 1;
  } catch {
    return raw === "1" || raw.toLowerCase() === "true";
  }
  return null;
}

function parseUpdatedAt(raw: string | null): string | null {
  if (raw === null) return null;
  try {
    const parsed = JSON.parse(raw) as { updated_at?: unknown };
    if (typeof parsed.updated_at === "string") return parsed.updated_at;
  } catch {
    return null;
  }
  return null;
}

const CONFIDENCE_RANK: Record<PaperModeConfidence, number> = {
  explicit: 4,
  explicit_legacy: 3,
  observed: 2,
  inferred: 1,
  default_safe: 0,
};

function minConfidence(
  a: PaperModeConfidence,
  b: PaperModeConfidence,
): PaperModeConfidence {
  return CONFIDENCE_RANK[a] < CONFIDENCE_RANK[b] ? a : b;
}

export async function resolvePaperModeState(
  deps: ResolvePaperModeStateDeps,
): Promise<PaperModeState> {
  const env = deps.env ?? process.env;
  const enabledChainIds = deps.enabledChainIds;
  const targetChainId = deps.chainId ?? null;
  const now = new Date().toISOString();

  const archiverOn =
    (env["ARBX_PAPER_ARCHIVER_MODE"] ?? "").toLowerCase() === "on";
  const tradeMode = env["ARBX_TRADE_MODE"];

  const chains: ChainPaperMode[] = [];
  const reasons: string[] = [];

  // Layer 1: Redis via MGET only.
  let globalRaw: string | null = null;
  const perChainRaws: (string | null)[] = [];

  if (deps.redis) {
    try {
      const keys = [GLOBAL_KEY, ...enabledChainIds.map(chainKey)];
      const values = await deps.redis.mget(...keys);
      globalRaw = values[0] ?? null;
      perChainRaws.push(...values.slice(1));
    } catch (e) {
      deps.logger?.warn?.(
        { event: "paper_mode_state.redis_err", err: (e as Error).message },
        "paper mode redis mget failed",
      );
      reasons.push("redis read failed; falling back to env inference");
    }
  }

  const anyPerChain = perChainRaws.some((r) => r !== null);

  for (let i = 0; i < enabledChainIds.length; i++) {
    const chainId = enabledChainIds[i];
    const raw = perChainRaws[i] ?? null;
    const enabled = parseEnabled(raw);
    const updatedAt = parseUpdatedAt(raw);

    let chain: ChainPaperMode;

    if (enabled !== null) {
      chain = {
        chain_id: chainId,
        enabled,
        source: "redis",
        confidence: "explicit",
        conflict: false,
        updated_at: updatedAt ?? now,
      };
    } else if (globalRaw !== null) {
      const globalEnabled = parseEnabled(globalRaw);
      chain = {
        chain_id: chainId,
        enabled: globalEnabled ?? true,
        source: "redis",
        confidence: "explicit_legacy",
        conflict: false,
        updated_at: parseUpdatedAt(globalRaw) ?? now,
      };
      reasons.push(`chain ${chainId} falling back to legacy global key`);
    } else if (archiverOn) {
      chain = {
        chain_id: chainId,
        enabled: true,
        source: "env",
        confidence: "inferred",
        conflict: false,
        updated_at: null,
      };
    } else if (tradeMode !== undefined) {
      chain = {
        chain_id: chainId,
        enabled: tradeMode === "paper",
        source: "env",
        confidence: "inferred",
        conflict: false,
        updated_at: null,
      };
    } else {
      chain = {
        chain_id: chainId,
        enabled: true,
        source: "default",
        confidence: "default_safe",
        conflict: false,
        updated_at: null,
      };
    }

    chains.push(chain);
  }

  // If no chains were requested, still resolve a single state from global/env.
  if (chains.length === 0) {
    const globalEnabled = parseEnabled(globalRaw);
    if (globalEnabled !== null) {
      chains.push({
        chain_id: targetChainId ?? 0,
        enabled: globalEnabled,
        source: "redis",
        confidence: "explicit_legacy",
        conflict: false,
        updated_at: parseUpdatedAt(globalRaw) ?? now,
      });
      reasons.push("resolved from legacy global key");
    } else if (archiverOn) {
      chains.push({
        chain_id: targetChainId ?? 0,
        enabled: true,
        source: "env",
        confidence: "inferred",
        conflict: false,
        updated_at: null,
      });
    } else if (tradeMode !== undefined) {
      chains.push({
        chain_id: targetChainId ?? 0,
        enabled: tradeMode === "paper",
        source: "env",
        confidence: "inferred",
        conflict: false,
        updated_at: null,
      });
    } else {
      chains.push({
        chain_id: targetChainId ?? 0,
        enabled: true,
        source: "default",
        confidence: "default_safe",
        conflict: false,
        updated_at: null,
      });
    }
  }

  // Aggregate: enabled only if ALL chains are ON; confidence = MIN across chains.
  const anyOn = chains.some((c) => c.enabled);
  const anyOff = chains.some((c) => !c.enabled);
  const allEnabled = chains.every((c) => c.enabled);
  let aggregateConfidence: PaperModeConfidence = chains[0]?.confidence ?? "default_safe";
  for (const c of chains) {
    aggregateConfidence = minConfidence(aggregateConfidence, c.confidence);
  }

  const degraded = aggregateConfidence === "explicit_legacy";

  // Conflict detection.
  for (const c of chains) {
    c.conflict = (anyOn && anyOff) || (!c.enabled && archiverOn);
  }

  const aggregateChainId = targetChainId ?? (chains.length === 1 ? chains[0].chain_id : null);

  if (allEnabled && !anyOff && reasons.length === 0 && !degraded) {
    reasons.push("all chains report paper mode enabled");
  }
  if (anyOff) {
    reasons.push("one or more chains report paper mode disabled");
  }
  if (degraded) {
    reasons.push("legacy global key is degraded; migrate to per-chain keys");
  }

  const conflict = chains.some((c) => c.conflict);

  const aggregate: PaperModeState = {
    enabled: allEnabled,
    chain_id: aggregateChainId,
    source:
      aggregateConfidence === "explicit"
        ? "redis"
        : aggregateConfidence === "explicit_legacy"
          ? "redis"
          : aggregateConfidence === "observed"
            ? "redis"
            : aggregateConfidence === "inferred"
              ? "env"
              : "default",
    confidence: aggregateConfidence,
    degraded,
    conflict,
    updated_at: allEnabled ? now : null,
    reasons,
    chains,
  };

  return aggregate;
}
