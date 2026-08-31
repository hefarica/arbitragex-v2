import { Redis } from "ioredis";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REDIS = process.env["REDIS_URL"] ?? "redis://redis:6379";
const DEFAULT_LAG_MAX = 500;

/** Minimal Redis surface G-PIPE-1 needs (ioredis satisfies it; tests fake it). */
export interface PipeRedisDeps {
  get: (key: string) => Promise<string | null>;
  xlen: (key: string) => Promise<number>;
  xinfo: (kind: "GROUPS", key: string, group?: string) => Promise<unknown>;
  xrange: (key: string, start: string, end: string, count: number) => Promise<unknown[]>;
}

const KILLSWITCH_KEY = "arbx:killswitch";

interface GroupInfo {
  entriesRead: number | null; // null when Redis omits it (older server)
  lag: number | null; // null when Redis omits it (older server)
  lastDeliveredId: string | null; // null when Redis omits it — raw-lag fallback applies
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
/**
 * G-PIPE-1b — deliverable backlog (ghost-lag fix, 2026-08-31).
 *
 * The server-reported group `lag` (entries-added − entries-read) counts entries
 * that were wiped/trimmed before delivery — after the 2026-08-29 killswitch-wipe
 * + stream recreation (16:28:45Z) those phantom entries can NEVER be read, so
 * the raw counter stayed permanently elevated and G-PIPE-1 read red forever
 * while the consumer was fully caught up. The REAL signal is the backlog of
 * entries physically PRESENT in the stream after the group's last-delivered-id:
 * deliverable work the consumer can actually consume. Counted with a
 * threshold-bounded XRANGE (exclusive start `(<last-delivered-id>`) — at most
 * lagMax ids are fetched, so the check is bounded regardless of stream size.
 * Returns null when the server omits last-delivered-id (older Redis) — caller
 * falls back to the raw-lag heuristic.
 */
async function deliverableBacklog(
  client: PipeRedisDeps,
  stream: string,
  group: GroupInfo,
  lagMax: number,
): Promise<{ count: number; exact: boolean } | null> {
  if (group.lastDeliveredId === null) return null;
  try {
    const entries = await client.xrange(stream, `(${group.lastDeliveredId}`, "+", lagMax);
    if (!Array.isArray(entries)) return null;
    // < lagMax returned → that IS the backlog (exact); == lagMax → at/above
    // threshold, which is all the gate needs (bounded read, no full scan).
    return { count: entries.length, exact: entries.length < lagMax };
  } catch {
    return null; // XRANGE unavailable → raw-lag fallback (defensive, R8)
  }
}

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
      // Typed overload: xrange(key, start, end, "COUNT", n) — the 4-arg
      // numeric form collides with the callback overload.
      xrange: (key: string, start: string, end: string, count: number) =>
        r.xrange(key, start, end, "COUNT", count),
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

    // G-PIPE-1b: gate on the DELIVERABLE backlog (entries present after the
    // group's last-delivered-id). The server lag is kept as context only —
    // it permanently includes wiped/trimmed phantom entries (2026-08-31 fix).
    const [detLen, valLen] = await Promise.all([
      client.xlen("arbx:opps:detected").catch(() => 0),
      client.xlen("arbx:opps:validated").catch(() => 0),
    ]);
    const [selectorBacklog, simBacklog] = await Promise.all([
      deliverableBacklog(client, "arbx:opps:detected", selector!, lagMax),
      deliverableBacklog(client, "arbx:opps:validated", simctl!, lagMax),
    ]);

    // Fallback (older Redis / XRANGE unavailable): raw server lag, else
    // entries-added − entries-read heuristic — the pre-1b behavior.
    const selectorLag = selector!.lag ?? (selector!.entriesRead !== null ? detLen - selector!.entriesRead : null);
    const simLag = simctl!.lag ?? (simctl!.entriesRead !== null ? valLen - simctl!.entriesRead : null);

    if (selectorBacklog === null && selectorLag === null) {
      return {
        ...base,
        status: "yellow",
        reason:
          "consumer group info lacks last-delivered-id/lag/entries-read (older Redis) — pipeline backlog not computable",
        evidence: { kind: "endpoint", ref: "XINFO GROUPS last-delivered-id/lag" },
      };
    }
    if (simBacklog === null && simLag === null) {
      return {
        ...base,
        status: "yellow",
        reason:
          "consumer group info lacks last-delivered-id/lag/entries-read (older Redis) — pipeline backlog not computable",
        evidence: { kind: "endpoint", ref: "XINFO GROUPS last-delivered-id/lag" },
      };
    }

    const selectorEffective = selectorBacklog?.count ?? selectorLag!;
    const simEffective = simBacklog?.count ?? simLag!;
    const selectorMode = selectorBacklog !== null ? "deliverable" : "raw-lag";
    const simMode = simBacklog !== null ? "deliverable" : "raw-lag";

    const evidence = {
      kind: "endpoint" as const,
      ref: `backlog(selector)=${selectorEffective}/${selectorMode} backlog(sim-ctl)=${simEffective}/${simMode} server-lag=${selectorLag ?? "?"}/${simLag ?? "?"} (killswitch ${killswitchDetail})`,
    };

    if (selectorEffective >= lagMax) {
      return {
        ...base,
        status: "red",
        reason: `selector consumer stalled: ${selectorEffective} entries behind on arbx:opps:detected (${selectorMode}, ≥${lagMax}) — 2026-08-29 A5-STALL signature`,
        evidence,
      };
    }
    if (simEffective >= lagMax) {
      return {
        ...base,
        status: "red",
        reason: `sim-ctl consumer stalled: ${simEffective} entries behind on arbx:opps:validated (${simMode}, ≥${lagMax})`,
        evidence,
      };
    }
    return {
      ...base,
      status: "green",
      reason: `streams flowing: backlog selector ${selectorEffective} (${selectorMode}), sim-ctl ${simEffective} (${simMode}) — below ${lagMax}; server-lag ${selectorLag ?? "?"}/${simLag ?? "?"} may include wiped phantom entries · kill-switch: ${killswitchDetail}`,
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
      const ldi = map.get("last-delivered-id");
      return {
        entriesRead: er === undefined || er === null ? null : Number(er),
        lag: lag === undefined || lag === null ? null : Number(lag),
        lastDeliveredId: ldi === undefined || ldi === null ? null : String(ldi),
      };
    }
  }
  return null;
}
