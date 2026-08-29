import { Redis } from "ioredis";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REDIS = process.env["REDIS_URL"] ?? "redis://redis:6379";
const DEFAULT_LAG_MAX = 500;

/** Minimal Redis surface G-PIPE-1 needs (ioredis satisfies it; tests fake it). */
export interface PipeRedisDeps {
  get: (key: string) => Promise<string | null>;
  xlen: (key: string) => Promise<number>;
  xinfo: (kind: "GROUPS", key: string, group?: string) => Promise<unknown>;
}

const KILLSWITCH_KEY = "arbx:killswitch";

interface GroupInfo {
  entriesRead: number | null; // null when Redis omits it (older server)
  lag: number | null; // null when Redis omits it (older server)
}

/**
 * G-PIPE-1 — Paper pipeline stream flow (detected → validated → simulated).
 *
 * Origin: 2026-08-29 A5-STALL — the Redis container was recreated during a
 * deploy with ALL persistence disabled, wiping `arbx:killswitch`. The
 * fail-closed default (configs/app.toml kill_switch_enabled_default=true)
 * then halted selector-api's consumer loop silently: zero logs, zero
 * consumption, lag 1781, for 4 days (paper runs froze 2026-08-25 12:07 →
 * A.5 blocker "runtime has not executed" was literally true). G-PAP-1 could
 * not see it: its "pipeline_active" reads the opportunities table, and
 * detection kept flowing — the stall was DOWNSTREAM of detection.
 *
 * Measurement (single Redis roundtrip set, no rate sampling needed):
 *   1. arbx:killswitch state — an enabled switch (explicit arm OR fail-closed
 *      default after key loss) halts the whole chain by design → red.
 *   2. Consumer-group lag = stream entries-added − group entries-read:
 *      selector-g0 on arbx:opps:detected and sim-ctl-g0 on arbx:opps:validated.
 *      A healthy consumer sits at 0..batch-size; A5-STALL showed 1781.
 *
 * Threshold: ARBX_PIPE_LAG_MAX (default 500) → red at/above. Malformed env
 * is yellow naming the var (mirrors G-DISK-1 handling).
 *
 * Honesty contract (R8): Redis unreachable → yellow with the exact error;
 * a missing group (consumer never created / exotic state) → yellow naming
 * it — never a fabricated lag, never a crash of verifyAll.
 */
export async function verifyGPIPE1(opts?: {
  now?: () => Date;
  redis?: PipeRedisDeps;
  url?: string;
  lagMax?: number;
}): Promise<ReadinessItem> {
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-PIPE-1",
    group: "operations" as const,
    label: "Paper pipeline stream flow (detected→validated→simulated)",
    doctrine: "safe-production-observability",
    verified_at,
  };

  const lagRaw = process.env["ARBX_PIPE_LAG_MAX"] ?? String(DEFAULT_LAG_MAX);
  const lagEnv = Number(lagRaw);
  if (!Number.isFinite(lagEnv) || lagEnv < 1) {
    return {
      ...base,
      status: "yellow",
      reason: `ARBX_PIPE_LAG_MAX="${lagRaw}" is malformed (expected integer ≥ 1) — pipeline lag not evaluated`,
    };
  }
  const lagMax = opts?.lagMax ?? lagEnv;

  // Connect only when no injected client (tests / callers pass a fake).
  let owned: Redis | null = null;
  let client: PipeRedisDeps;
  if (opts?.redis) {
    client = opts.redis;
  } else {
    owned = new Redis(opts?.url ?? DEFAULT_REDIS, {
      lazyConnect: true,
      maxRetriesPerRequest: 1,
      connectTimeout: 2000,
    });
    try {
      await owned.connect();
    } catch (e) {
      owned.disconnect();
      return {
        ...base,
        status: "yellow",
        reason: `Redis unreachable for pipeline streams: ${(e as Error).message.slice(0, 80)}`,
      };
    }
    // Narrow adapter: ioredis's overloaded xinfo signature doesn't match the
    // minimal PipeRedisDeps shape structurally.
    const r = owned;
    client = {
      get: (key: string) => r.get(key),
      xlen: (key: string) => r.xlen(key),
      xinfo: (kind: "GROUPS", key: string) => r.xinfo(kind, key),
    };
  }

  try {
    // (1) Kill-switch state — the A5-STALL root cause, surfaced directly.
    let killswitchOn = false;
    let killswitchDetail = "key absent (fail-closed default applies at consumers)";
    const raw = await client.get(KILLSWITCH_KEY);
    if (raw !== null) {
      try {
        const parsed = JSON.parse(raw) as { enabled?: unknown };
        killswitchOn = parsed.enabled === true || parsed.enabled === "true";
        killswitchDetail = `key present, enabled=${String(parsed.enabled)}`;
      } catch {
        killswitchDetail = "key present but unparseable";
      }
    }
    if (killswitchOn) {
      return {
        ...base,
        status: "red",
        reason: `kill-switch ENABLED — detection→sim→execute chain halted by design (explicit arm or fail-closed default after Redis key loss) · ${killswitchDetail}`,
        evidence: { kind: "endpoint", ref: `GET ${KILLSWITCH_KEY}` },
      };
    }

    // (2) Consumer-group lag on both hops of the chain.
    const [detGroups, valGroups] = await Promise.all([
      client.xinfo("GROUPS", "arbx:opps:detected").catch(() => null),
      client.xinfo("GROUPS", "arbx:opps:validated").catch(() => null),
    ]);
    const selector = findGroup(detGroups, "selector-g0");
    const simctl = findGroup(valGroups, "sim-ctl-g0");

    const missing: string[] = [];
    if (!selector) missing.push("selector-g0 on arbx:opps:detected");
    if (!simctl) missing.push("sim-ctl-g0 on arbx:opps:validated");
    if (missing.length > 0) {
      return {
        ...base,
        status: "yellow",
        reason: `consumer group(s) not found: ${missing.join("; ")} — consumer never created or streams absent`,
        evidence: { kind: "endpoint", ref: "XINFO GROUPS arbx:opps:{detected,validated}" },
      };
    }

    // Prefer the server-reported lag; fall back to entries-added − entries-read.
    const [detLen, valLen] = await Promise.all([
      client.xlen("arbx:opps:detected").catch(() => 0),
      client.xlen("arbx:opps:validated").catch(() => 0),
    ]);
    const selectorLag = selector!.lag ?? (selector!.entriesRead !== null ? detLen - selector!.entriesRead : null);
    const simLag = simctl!.lag ?? (simctl!.entriesRead !== null ? valLen - simctl!.entriesRead : null);

    if (selectorLag === null || simLag === null) {
      return {
        ...base,
        status: "yellow",
        reason:
          "consumer group info lacks lag/entries-read (older Redis) — pipeline lag not computable",
        evidence: { kind: "endpoint", ref: "XINFO GROUPS lag/entries-read" },
      };
    }

    const evidence = {
      kind: "endpoint" as const,
      ref: `XINFO GROUPS selector-g0=${selectorLag} sim-ctl-g0=${simLag} (killswitch ${killswitchDetail})`,
    };

    if (selectorLag >= lagMax) {
      return {
        ...base,
        status: "red",
        reason: `selector consumer stalled: ${selectorLag} entries behind on arbx:opps:detected (≥${lagMax}) — 2026-08-29 A5-STALL signature`,
        evidence,
      };
    }
    if (simLag >= lagMax) {
      return {
        ...base,
        status: "red",
        reason: `sim-ctl consumer stalled: ${simLag} entries behind on arbx:opps:validated (≥${lagMax})`,
        evidence,
      };
    }
    return {
      ...base,
      status: "green",
      reason: `streams flowing: selector lag ${selectorLag}, sim-ctl lag ${simLag} (below ${lagMax}) · kill-switch: ${killswitchDetail}`,
      evidence,
    };
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `pipeline stream check failed: ${(e as Error).message.slice(0, 80)}`,
    };
  } finally {
    owned?.disconnect();
  }
}

/** Extract one group's info from XINFO GROUPS reply (array-of-arrays flat K/V). */
function findGroup(reply: unknown, name: string): GroupInfo | null {
  if (!Array.isArray(reply)) return null;
  for (const entry of reply) {
    if (!Array.isArray(entry)) continue;
    const map = new Map<string, unknown>();
    for (let i = 0; i + 1 < entry.length; i += 2) {
      map.set(String(entry[i]), entry[i + 1]);
    }
    if (map.get("name") === name) {
      const er = map.get("entries-read");
      const lag = map.get("lag");
      return {
        entriesRead: er === undefined || er === null ? null : Number(er),
        lag: lag === undefined || lag === null ? null : Number(lag),
      };
    }
  }
  return null;
}
