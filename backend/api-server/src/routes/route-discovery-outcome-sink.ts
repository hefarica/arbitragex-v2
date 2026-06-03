/**
 * Route-Discovery Outcome Sink — durable Postgres drain for the shadow outcomes
 * stream (FASE B Paso 2). 100% PASSIVE — twin of paper-trade-archiver.ts.
 *
 * It only:
 *   1. reads `rd_outcome_v1` messages from Redis stream
 *      `arbx:route_discovery:outcomes` (consumer group, at-least-once, XACK
 *      after persist), and
 *   2. INSERTs the message's REAL fields into `route_discovery_outcomes`.
 *
 * It NEVER:
 *   - computes/derives profit/hit (copies payload fields verbatim),
 *   - fabricates a row when a message is unparseable (SKIP + XACK),
 *   - touches `arbx:opps:detected`, `paper_trade_runs`, capital, signers, or exec.
 *
 * Idempotent: stream_id is UNIQUE → ON CONFLICT DO NOTHING (at-least-once safe).
 * Gated off by default: ARBX_ROUTE_DISCOVERY_OUTCOMES_SINK ∈ {on,1,true,shadow}.
 */

import { Redis } from "ioredis";
import pg from "pg";

const STREAM_IN = "arbx:route_discovery:outcomes";
const GROUP = "rd-outcome-sink-g0";
const CONSUMER = process.env["HOSTNAME"] ?? "rd-outcome-sink-1";

export interface SinkLogger {
  info(obj: object, msg?: string): void;
  warn(obj: object, msg?: string): void;
  error(obj: object, msg?: string): void;
}

export interface OutcomeSinkDeps {
  /** Connection string for a DEDICATED Redis connection (XREADGROUP blocks). */
  redisUrl: string;
  /** Shared pg pool from the api-server (DATABASE_URL must be configured). */
  pool: pg.Pool;
  logger: SinkLogger;
}

/** Independent gate. Default OFF (NO-ACTIVE). */
export function outcomeSinkEnabled(): boolean {
  const v = (process.env["ARBX_ROUTE_DISCOVERY_OUTCOMES_SINK"] ?? "")
    .toLowerCase();
  return v === "on" || v === "1" || v === "true" || v === "shadow";
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

export class RouteDiscoveryOutcomeSink {
  private redis: Redis | null = null;
  private running = false;
  private stopPromise: Promise<void> | null = null;

  constructor(private readonly deps: OutcomeSinkDeps) {}

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    this.redis = new Redis(this.deps.redisUrl, { maxRetriesPerRequest: 3 });
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info(
      { event: "rd_outcome_sink.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER },
      "Route-discovery outcome sink consuming shadow outcomes",
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
        this.deps.logger.warn({ event: "rd_outcome_sink.group_create_err", err: msg });
      }
    }
  }

  private async runLoop(): Promise<void> {
    while (this.running && this.redis) {
      try {
        const r = await this.redis.xreadgroup(
          "GROUP", GROUP, CONSUMER,
          "COUNT", 256,
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
        this.deps.logger.error({ event: "rd_outcome_sink.loop_err", err: (err as Error).message });
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
      this.deps.logger.warn({ event: "rd_outcome_sink.invalid_message", id, err: (err as Error).message });
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    try {
      await this.deps.pool.query(
        `INSERT INTO route_discovery_outcomes
           (stream_id, ts_ms, schema_ver, chain_id, cartridge_id, tx_hash,
            source_event, pool_hint, token_in, token_out, is_opportunity,
            estimated_profit, confidence, urgency, had_reserves, mode)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
         ON CONFLICT (stream_id) DO NOTHING`,
        [
          id,                       // stream_id (XADD id) — the idempotency key
          o.ts_ms,
          o.schema,
          o.chain_id,
          o.cartridge_id,
          o.tx_hash,
          o.source_event ?? null,
          o.pool_hint ?? null,
          o.token_in ?? null,
          o.token_out ?? null,
          o.is_opportunity,
          o.estimated_profit,
          o.confidence,
          o.urgency ?? null,
          o.had_reserves,
          o.mode,
        ],
      );
      await this.redis.xack(STREAM_IN, GROUP, id);
    } catch (err) {
      // Transient DB error → do NOT ack; a later read retries (at-least-once).
      this.deps.logger.error({
        event: "rd_outcome_sink.persist_err",
        id,
        err: (err as Error).message,
      });
    }
  }
}
