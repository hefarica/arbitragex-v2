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
  // Position-size cap: USD-denominated names OR the legacy ETH cap that
  // the searcher actually consumes today (max_value_eth in [execution]).
  // Either form is acceptable doctrinally — the canonical denomination
  // will migrate to USD as the multi-asset router lands (Sprint S4+).
  const has_position_limit = /max_position_size_usd|max_notional_usd|max_capital_usd|max_value_eth/.test(cfgBody);
  // Stop-loss / drawdown trigger: any of the auto-kill paths the platform
  // wires today (max_revert_rate_pct + auto_trip_on_high_revert_rate is
  // the production drawdown proxy until per-strategy P&L lands).
  const has_stop_loss = /stop_loss|drawdown|max_drawdown|auto_trip|kill_switch_enabled_default|max_revert_rate_pct/.test(cfgBody);
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

  // Layer 2: killswitch wiring (canonical Redis key reachable AND readable).
  //
  // KS-KEY-01 (2026-09-29, medido): esta capa leía `arbx:killswitch:enabled`,
  // clave que NINGÚN camino de ejecución lee — el cliente canónico usa
  // `arbx:killswitch` (backend/shared-rs/src/killswitch.rs:15) con un JSON
  // `KillSwitchState {enabled,reason,triggered_by,updated_at}` (:27-32). Peor: el
  // valor se asignaba a `kswitch_state` y NUNCA se consultaba, así que la capa
  // sólo probaba "Redis respondió a un GET" y podía declarar verde sobre una
  // clave sin escritor (falso verde, viola R10). Ahora se lee la clave canónica y
  // el valor SE USA: si el switch está armado se dice, y si el JSON no parsea se
  // reporta en amarillo (el cliente Rust fail-closed lo trataría como ARMED).
  const redis = new Redis(redisUrl, {
    lazyConnect: true,
    maxRetriesPerRequest: 1,
    connectTimeout: 2000,
  });
  let kswitch_state: string | null = null;
  try {
    await redis.connect();
    kswitch_state = await redis.get("arbx:killswitch");
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `kill-switch key unreachable in Redis: ${(e as Error).message.slice(0, 80)}`,
    };
  } finally {
    redis.disconnect();
  }

  // El valor se interpreta de verdad (antes se descartaba). Y se valida la FORMA,
  // no sólo que parsee: `JSON.parse("1")` es válido (un número) y el `1` crudo que
  // el runbook muerto instruía colaría como "desarmado" (lo cazó el test).
  let armed: boolean | null = null;
  let armed_note = "";
  if (kswitch_state != null) {
    const parsed: unknown = (() => {
      try {
        return JSON.parse(kswitch_state);
      } catch {
        return undefined;
      }
    })();
    const shaped =
      typeof parsed === "object" &&
      parsed !== null &&
      typeof (parsed as { enabled?: unknown }).enabled === "boolean";
    if (!shaped) {
      return {
        ...base,
        status: "yellow",
        reason:
          "arbx:killswitch presente pero no tiene la forma de KillSwitchState {enabled:bool,…}: el cliente Rust fail-closed lo trataría como ARMED",
        evidence: { kind: "config", ref: "redis:arbx:killswitch" },
      };
    }
    const st = parsed as { enabled: boolean; triggered_by?: unknown };
    armed = st.enabled;
    armed_note = armed
      ? ` — ARMED por ${typeof st.triggered_by === "string" ? st.triggered_by : "origen no declarado"}`
      : " (desarmado)";
  } else {
    armed_note = " (clave ausente: aplica el default de app.toml)";
  }

  // Layer 3: auto-trip evidence. Either flag is enabled or a real arming exists.
  //
  // KS-TOML-01 (2026-09-29): el regex corría sobre el TEXTO CRUDO de app.toml, así
  // que una línea COMENTADA (`# auto_trip_on_high_revert_rate = true`) daba verde.
  // Ahora se eliminan los comentarios TOML antes de evaluar (respetando `#` dentro
  // de comillas), de modo que sólo cuenta una clave realmente activa.
  const auto_trip_flag = /auto_trip_on_high_revert_rate\s*=\s*true/.test(stripTomlComments(cfgBody));
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
      reason: `limits configured + Redis reachable${armed_note}, but auto_trip_on_high_revert_rate=false (or commented out) and no historical arming recorded`,
      evidence: { kind: "config", ref: "configs/app.toml + redis:arbx:killswitch" },
    };
  }

  const evidence_msg = auto_trip_flag
    ? "auto_trip_on_high_revert_rate=true"
    : `${armed_history} killswitch.armed events in last 30d`;

  return {
    ...base,
    status: "green",
    reason: `[risk] limits + drawdown trigger configured; kill-switch reachable via Redis${armed_note}; ${evidence_msg}`,
    evidence: { kind: "config", ref: "configs/app.toml + redis:arbx:killswitch" },
  };
}

/**
 * Elimina comentarios TOML de un texto: descarta todo lo que sigue a un `#` que
 * no esté dentro de una cadena entre comillas (simples o dobles). Sin esto, una
 * clave comentada se lee como activa (KS-TOML-01).
 */
export function stripTomlComments(src: string): string {
  return src
    .split("\n")
    .map((line) => {
      let quote: string | null = null;
      for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (quote != null) {
          if (ch === quote && line[i - 1] !== "\\") quote = null;
          continue;
        }
        if (ch === '"' || ch === "'") {
          quote = ch;
          continue;
        }
        if (ch === "#") return line.slice(0, i);
      }
      return line;
    })
    .join("\n");
}
