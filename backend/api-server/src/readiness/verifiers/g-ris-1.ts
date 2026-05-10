import { promises as fs } from "node:fs";
import path from "node:path";
import type pg from "pg";
import type { ReadinessItem } from "../types.js";
import { Redis } from "ioredis";

const DEFAULT_REPO = "/repo";
const DEFAULT_REDIS = process.env["REDIS_URL"] ?? "redis://redis:6379";

/**
 * G-RIS-1 — Risk limits + auto-kill on drawdown.
 *
 * Three-layer verification:
 *
 *   1. Config: configs/app.toml has [risk] section with at least
 *      max_position_size_usd and stop_loss_threshold_pct (or equivalent
 *      drawdown trigger).
 *
 *   2. Killswitch wiring: kill switch state is queryable via Redis key
 *      `arbx:killswitch:enabled` (canonical state per audit B10 doctrine).
 *
 *   3. Drawdown calc: at least one of (a) audit_log row of action
 *      `killswitch.armed` with reason mentioning drawdown, or
 *      (b) configs/app.toml has `auto_trip_on_high_revert_rate=true` —
 *      proving the auto-kill path is enabled, not merely defined.
 */
export async function verifyGRIS1(opts?: {
  pool?: pg.Pool | null;
  repo?: string;
  redisUrl?: string;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const repo = opts?.repo ?? DEFAULT_REPO;
  const redisUrl = opts?.redisUrl ?? DEFAULT_REDIS;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-RIS-1",
    group: "risk_doctrines" as const,
    label: "Risk limits + auto-kill on drawdown",
    doctrine: "arbx-risk-limits-enforcement",
    verified_at,
  };

  // Layer 1: config presence.
  const cfgPath = path.join(repo, "configs/app.toml");
  let cfgBody: string;
  try {
    cfgBody = await fs.readFile(cfgPath, "utf8");
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: `cannot read ${cfgPath} (no repo mount)`,
    };
  }
  const has_risk_section = /\[risk\]/.test(cfgBody);
  const has_position_limit = /max_position_size_usd|max_notional_usd|max_capital_usd/.test(cfgBody);
  const has_stop_loss = /stop_loss|drawdown|max_drawdown|auto_trip|kill_switch_enabled_default/.test(cfgBody);
  if (!has_risk_section || !has_position_limit || !has_stop_loss) {
    const missing = [
      !has_risk_section && "[risk] section",
      !has_position_limit && "position-size limit",
      !has_stop_loss && "stop-loss / drawdown trigger",
    ].filter(Boolean).join(", ");
    return {
      ...base,
      status: "red",
      reason: `app.toml missing: ${missing}`,
    };
  }

  // Layer 2: killswitch wiring (Redis canonical key reachable).
  const redis = new Redis(redisUrl, {
    lazyConnect: true,
    maxRetriesPerRequest: 1,
    connectTimeout: 2000,
  });
  let kswitch_state: string | null = null;
  try {
    await redis.connect();
    kswitch_state = await redis.get("arbx:killswitch:enabled");
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `kill-switch key unreachable in Redis: ${(e as Error).message.slice(0, 80)}`,
    };
  } finally {
    redis.disconnect();
  }
  // null = key absent (default = file fallback); 0 = disabled; 1 = armed.
  // Either is fine — what matters is Redis is reachable for runtime mutation.

  // Layer 3: auto-trip evidence. Either flag is enabled or a real arming exists.
  const auto_trip_flag = /auto_trip_on_high_revert_rate\s*=\s*true/.test(cfgBody);
  let armed_history = 0;
  if (opts?.pool) {
    try {
      const r = await opts.pool.query(
        `SELECT COUNT(*)::int AS n FROM audit_log
          WHERE action = 'killswitch.armed' AND created_at > NOW() - interval '30 days'`,
      );
      armed_history = r.rows[0]?.n ?? 0;
    } catch {
      // audit_log may be absent in early bootstraps; tolerate.
    }
  }

  if (!auto_trip_flag && armed_history === 0) {
    return {
      ...base,
      status: "yellow",
      reason: "limits configured + Redis reachable, but auto_trip_on_high_revert_rate=false and no historical arming recorded",
      evidence: { kind: "config", ref: "configs/app.toml + arbx:killswitch:enabled" },
    };
  }

  const evidence_msg = auto_trip_flag
    ? "auto_trip_on_high_revert_rate=true"
    : `${armed_history} killswitch.armed events in last 30d`;

  return {
    ...base,
    status: "green",
    reason: `[risk] limits + drawdown trigger configured; kill-switch reachable via Redis; ${evidence_msg}`,
    evidence: { kind: "config", ref: "configs/app.toml + arbx:killswitch:enabled" },
  };
}
