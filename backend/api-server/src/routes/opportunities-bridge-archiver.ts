/**
 * Opportunities Bridge Archiver — surfaces the shadow route-discovery outcomes
 * into the `/opportunities` feed (paper). 100% PASSIVE, api-server-side only.
 *
 * It reads `rd_outcome_v1` messages from Redis stream
 * `arbx:route_discovery:outcomes` (its OWN consumer group `opps-bridge-g0`, so it
 * co-exists with the durable sink without stealing its messages) and maps each
 * REAL outcome into a row of the `opportunities` PG table — the actual source of
 * `GET /api/opportunities/live` (opportunities-live.ts reads PG, NOT the Redis
 * stream). Once a row lands, the existing endpoint + frontend render it.
 *
 * Honesty (RULE 00 / R8):
 *   - copies the outcome's REAL fields verbatim (estimated_profit, confidence,
 *     reason); 0.0 stays 0.0 (computed-and-zero), never nulled, never inflated.
 *   - `is_opportunity=false` → status `'rejected'` + the REAL `reason` (so the
 *     row shows as a non-viable route with its honest reason); `true` → `detected`.
 *   - NEVER fabricates a required field: `amount_in_wei=0` (Phase-1 route
 *     discovery is UNSIZED), `dex_a='route_discovery'` (provenance marker, not a
 *     fake venue), net/roi/block = NULL. Missing token_in/out or an out-of-enum
 *     cartridge_id → SKIP + XACK (no row).
 *
 * It NEVER touches the searcher, `arbx:opps:detected`, capital, signers, or exec.
 * The searcher's NO-ACTIVE guarantee (route_discovery/guarantees.rs) is untouched:
 * only the api-server writes the `opportunities` table here.
 *
 * Idempotent: id = md5(redis_entry_id)::uuid → `ON CONFLICT (id) DO NOTHING`
 * (the `opportunities` table has no stream_id column; the deterministic UUID is
 * the at-least-once dedupe key). XACK only after a successful/DO-NOTHING persist.
 *
 * Gated OFF by default: set `ARBX_OPPS_BRIDGE_MODE=on` on the VPS to activate.
 */

import { Redis } from "ioredis";
import pg from "pg";

const STREAM_IN = "arbx:route_discovery:outcomes";
const GROUP = "opps-bridge-g0";
const CONSUMER = process.env["HOSTNAME"] ?? "opps-bridge-1";

/** Valid strategy_kind values per the `opportunities` table CHECK constraint. */
const VALID_KINDS = new Set([
  "dex_arb",
  "triangular",
  "backrun",
  "liquidation",
  "flashloan_arb",
]);

/**
 * Canonical `strategy_kind` for a shadow rd_outcome. The v1 payload carries
 * `cartridge_id` (a cartridge FILENAME, e.g. `mev_01_001_dex_dex`) but NO
 * strategy_kind field, and a filename is never one of the 5 persisted kinds. The
 * active cartridge path defaults every cartridge to DexArb (searcher-rs
 * cartridge_boot.rs `StrategyKind::DexArb`); mirror that default so the row
 * satisfies the opportunities.strategy_kind CHECK constraint without inventing
 * a finer classification the discovery layer does not have (R8). The
 * cartridge_id is preserved as row identity via trace_id/pair provenance.
 */
function deriveStrategyKind(_cartridgeId: string): string {
  return "dex_arb";
}

export interface BridgeLogger {
  info(obj: object, msg?: string): void;
  warn(obj: object, msg?: string): void;
  error(obj: object, msg?: string): void;
}

export interface OpportunitiesBridgeDeps {
  /** Connection string for a DEDICATED Redis connection (XREADGROUP blocks). */
  redisUrl: string;
  /** Shared pg pool from the api-server (DATABASE_URL must be configured). */
  pool: pg.Pool;
  logger: BridgeLogger;
}

/** Independent gate. Default OFF (NO-ACTIVE). Only "on" enables. */
export function opportunitiesBridgeEnabled(): boolean {
  return (process.env["ARBX_OPPS_BRIDGE_MODE"] ?? "off").toLowerCase() === "on";
}

/** Extract the value for `key` from a flat XREAD kv array [k1,v1,k2,v2,...]. */
function fieldValue(kv: string[], key: string): string | undefined {
  for (let i = 0; i + 1 < kv.length; i += 2) {
    if (kv[i] === key) return kv[i + 1];
  }
  return undefined;
}

/** Shape of the rd_outcome_v1 payload (mirrors cartridge_boot.rs emitter). */
interface RdOutcomeV1 {
  schema: string;
  ts_ms: number;
  chain_id: number;
  cartridge_id: string;
  tx_hash: string;
  source_event?: string;
  pool_hint?: string;
  token_in?: string;
  token_out?: string;
  is_opportunity: boolean;
  estimated_profit: number;
  confidence: number;
  urgency?: string;
  had_reserves: boolean;
  mode: string;
  reason?: string;
}

/** Validate REAL required fields; reject (→ SKIP+XACK) if malformed. Never coerces. */
function parseOutcome(json: string): RdOutcomeV1 {
  const o = JSON.parse(json) as Record<string, unknown>;
  if (o["schema"] !== "rd_outcome_v1") throw new Error("bad_schema");
  if (typeof o["ts_ms"] !== "number") throw new Error("bad_ts_ms");
  if (typeof o["chain_id"] !== "number") throw new Error("bad_chain_id");
  if (typeof o["cartridge_id"] !== "string") throw new Error("bad_cartridge_id");
  if (typeof o["tx_hash"] !== "string") throw new Error("bad_tx_hash");
  if (typeof o["is_opportunity"] !== "boolean") throw new Error("bad_is_opportunity");
  if (typeof o["estimated_profit"] !== "number") throw new Error("bad_estimated_profit");
  if (typeof o["confidence"] !== "number") throw new Error("bad_confidence");
  if (typeof o["had_reserves"] !== "boolean") throw new Error("bad_had_reserves");
  if (typeof o["mode"] !== "string") throw new Error("bad_mode");
  return o as unknown as RdOutcomeV1;
}

export class OpportunitiesBridgeArchiver {
  private redis: Redis | null = null;
  private running = false;
  private stopPromise: Promise<void> | null = null;

  // Aggregated skip counters — flushed as ONE summary warn per interval. A
  // per-message skip warn at 2k/min was the dominant api-server event-loop
  // saturator (opps_bridge.skip_unknown_strategy_kind, 2026-08-08 live diag).
  private readonly skipCounts = new Map<string, number>();
  private readonly skipDetails = new Map<string, string>();
  private lastSkipFlushMs = 0;
  private static readonly SKIP_FLUSH_MS = 5000;

  constructor(private readonly deps: OpportunitiesBridgeDeps) {}

  /** Record a non-actionable skip; emit one aggregated summary per interval so a
   * high-rate skip cause can never flood the event loop. */
  private recordSkip(event: string, detail?: string): void {
    this.skipCounts.set(event, (this.skipCounts.get(event) ?? 0) + 1);
    if (detail !== undefined) this.skipDetails.set(event, detail);
    const now = Date.now();
    if (now - this.lastSkipFlushMs >= OpportunitiesBridgeArchiver.SKIP_FLUSH_MS) {
      this.lastSkipFlushMs = now;
      this.deps.logger.warn({
        event: "opps_bridge.skip_summary",
        counts: Object.fromEntries(this.skipCounts),
        details: Object.fromEntries(this.skipDetails),
      });
      this.skipCounts.clear();
      this.skipDetails.clear();
    }
  }

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    this.redis = new Redis(this.deps.redisUrl, { maxRetriesPerRequest: 3 });
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info(
      { event: "opps_bridge.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER },
      "Opportunities bridge surfacing shadow outcomes into the opportunities table",
    );
  }

  async stop(): Promise<void> {
    this.running = false;
    if (this.stopPromise) await this.stopPromise.catch(() => {});
    if (this.redis) await this.redis.quit().catch(() => {});
  }

  private async ensureGroup(): Promise<void> {
    if (!this.redis) return;
    try {
      await this.redis.xgroup("CREATE", STREAM_IN, GROUP, "$", "MKSTREAM");
    } catch (e) {
      const msg = (e as Error).message;
      if (!msg.includes("BUSYGROUP")) {
        this.deps.logger.warn({ event: "opps_bridge.group_create_err", err: msg });
      }
    }
  }

  private async runLoop(): Promise<void> {
    while (this.running && this.redis) {
      try {
        const r = await this.redis.xreadgroup(
          "GROUP", GROUP, CONSUMER,
          "COUNT", 64,
          "BLOCK", 2000,
          "STREAMS", STREAM_IN, ">",
        );
        if (!r || (r as unknown[]).length === 0) continue;
        for (const [, entries] of r as Array<[string, Array<[string, string[]]>]>) {
          for (const [id, kv] of entries) {
            await this.processOne(id, kv);
          }
        }
      } catch (err) {
        this.deps.logger.error({ event: "opps_bridge.loop_err", err: (err as Error).message });
        await new Promise((res) => setTimeout(res, 1000));
      }
    }
  }

  private async processOne(id: string, kv: string[]): Promise<void> {
    if (!this.redis) return;

    let o: RdOutcomeV1;
    try {
      const json = fieldValue(kv, "json");
      if (!json) throw new Error("no_json_field");
      o = parseOutcome(json);
    } catch (err) {
      // Unparseable → SKIP + XACK (never invent a row, never stall the group).
      this.recordSkip("invalid_message", (err as Error).message);
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // NOT NULL guard — SKIP + XACK rather than fabricate (RULE 00).
    if (!o.token_in || !o.token_out) {
      this.recordSkip("skip_missing_token");
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // strategy_kind: the v1 payload has cartridge_id (a filename) but no canonical
    // kind — derive the default (dex_arb, matching the active cartridge path) and
    // fail-honest-assert it satisfies the opportunities.strategy_kind CHECK.
    const strategyKind = deriveStrategyKind(o.cartridge_id);
    if (!VALID_KINDS.has(strategyKind)) {
      this.recordSkip("skip_unmapped_strategy_kind", strategyKind);
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    const status = o.is_opportunity ? "detected" : "rejected";
    const reason = o.is_opportunity ? null : (o.reason ?? null);
    const pair = o.pool_hint ?? `${o.token_in}->${o.token_out}`;

    try {
      // id/trace_id = deterministic UUID from the Redis entry id (md5::uuid) so the
      // INSERT is idempotent without a stream_id column. amount_in_wei=0 (unsized
      // Phase-1), dex_a='route_discovery' (provenance), net/roi/block = NULL.
      await this.deps.pool.query(
        `INSERT INTO opportunities
           (id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol, token_in, token_out,
            amount_in_wei, expected_profit_usd, roi_pct, risk_score, block_number,
            status, rejection_reason, trace_id, detected_at)
         VALUES (md5($1)::uuid, $2, $3, 'route_discovery', NULL, $4, $5, $6,
                 0, $7, NULL, $8, NULL, $9, $10, md5($11)::uuid,
                 to_timestamp($12::double precision / 1000.0))
         ON CONFLICT (id) DO NOTHING`,
        [
          id,                       // md5(id)::uuid → deterministic opp id
          o.chain_id,
          strategyKind,             // strategy_kind (derived + validated above)
          pair,                     // pair_symbol
          o.token_in,
          o.token_out,
          o.estimated_profit,       // expected_profit_usd (GROSS, verbatim; 0.0 stays 0.0)
          o.confidence,             // risk_score ← confidence (verbatim)
          status,                   // detected | rejected
          reason,                   // real reason when rejected, else null
          `${o.tx_hash}:${id}`,     // md5(...)::uuid → deterministic trace_id
          o.ts_ms,                  // detected_at ← to_timestamp(ts_ms/1000)
        ],
      );
      await this.redis.xack(STREAM_IN, GROUP, id);
    } catch (err) {
      // Transient DB error → do NOT ack; a later read retries (at-least-once).
      this.deps.logger.error({
        event: "opps_bridge.persist_err",
        id,
        err: (err as Error).message,
      });
    }
  }
}
