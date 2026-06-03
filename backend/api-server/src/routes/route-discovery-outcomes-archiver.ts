/**
 * Route-Discovery Outcomes Archiver — durable sink for Fase B shadow outcomes.
 *
 * 100% PASSIVE. Reads `arbx:route_discovery:outcomes` (the Rust shadow emitter's
 * stream, Fase B Paso 1) via a consumer group (XREADGROUP/XACK, at-least-once) and
 * INSERTs each resolved outcome into `route_discovery_outcomes`. Idempotent via the
 * Redis stream entry id (UNIQUE stream_id + ON CONFLICT DO NOTHING). Preserves the
 * >=2-week series the capped stream (MAXLEN ~1M) would otherwise trim (~6 days).
 *
 * NEVER touches arbx:opps:detected, capital, signers, execution. Zero-Mocks: only
 * persists fields present in the REAL payload; never fabricates. Fail-closed:
 * transient DB errors are NOT XACKed (retried); unparseable/incomplete messages are
 * skipped + XACKed (no replay loop). Gated off by ARBX_RD_OUTCOMES_ARCHIVER_MODE.
 *
 * Mirrors `paper-trade-archiver.ts` (same XREADGROUP/XACK pattern).
 */

import { Redis } from "ioredis";
import pg from "pg";

const STREAM_IN = "arbx:route_discovery:outcomes";
const GROUP = "rd-outcomes-archiver-g0";
const CONSUMER = process.env["HOSTNAME"] ?? "rd-outcomes-archiver-1";

/** Minimal structural logger — satisfied by the pino logger in index.ts. */
export interface ArchiverLogger {
  info(obj: object, msg?: string): void;
  warn(obj: object, msg?: string): void;
  error(obj: object, msg?: string): void;
}

export interface RouteDiscoveryOutcomesArchiverDeps {
  /** Connection string for a DEDICATED Redis connection (XREADGROUP blocks). */
  redisUrl: string;
  /** Shared pg pool from the api-server (DATABASE_URL must be configured). */
  pool: pg.Pool;
  logger: ArchiverLogger;
}

/** Shape of the `rd_outcome_v1` payload the Rust emitter XADDs (field "json"). */
interface RdOutcome {
  schema?: string;
  ts_ms?: number;
  chain_id?: number;
  cartridge_id?: string;
  tx_hash?: string;
  source_event?: string;
  pool_hint?: string;
  token_in?: string;
  token_out?: string;
  is_opportunity?: boolean;
  estimated_profit?: number;
  confidence?: number;
  urgency?: string;
  had_reserves?: boolean;
  mode?: string;
}

/** Extract the value for `key` from a flat XREAD kv array [k1,v1,k2,v2,...]. */
function fieldValue(kv: string[], key: string): string | undefined {
  for (let i = 0; i + 1 < kv.length; i += 2) {
    if (kv[i] === key) return kv[i + 1];
  }
  return undefined;
}

export class RouteDiscoveryOutcomesArchiver {
  private redis: Redis | null = null;
  private running = false;
  private stopPromise: Promise<void> | null = null;

  constructor(private readonly deps: RouteDiscoveryOutcomesArchiverDeps) {}

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    // Dedicated connection: a blocking XREADGROUP must not stall the shared client.
    this.redis = new Redis(this.deps.redisUrl, { maxRetriesPerRequest: 3 });
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info(
      { event: "rd_outcomes_archiver.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER },
      "route_discovery outcomes archiver consuming shadow outcomes",
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
      // Start id "0" (not "$"): consume the existing backlog too, so outcomes that
      // accrued before the archiver started are persisted (not just new entries).
      await this.redis.xgroup("CREATE", STREAM_IN, GROUP, "0", "MKSTREAM");
    } catch (e) {
      const msg = (e as Error).message;
      if (!msg.includes("BUSYGROUP")) {
        this.deps.logger.warn({ event: "rd_outcomes_archiver.group_create_err", err: msg });
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
        this.deps.logger.error({ event: "rd_outcomes_archiver.loop_err", err: (err as Error).message });
        await new Promise((res) => setTimeout(res, 1000));
      }
    }
  }

  private async processOne(id: string, kv: string[]): Promise<void> {
    if (!this.redis) return;

    // Parse — invalid/unparseable messages are SKIPPED + XACKed (never invented).
    let o: RdOutcome;
    try {
      const json = fieldValue(kv, "json");
      if (!json) throw new Error("no_json_field");
      o = JSON.parse(json) as RdOutcome;
    } catch (err) {
      this.deps.logger.warn({ event: "rd_outcomes_archiver.invalid_message", id, err: (err as Error).message });
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // Required NOT-NULL fields. If the real payload lacks them, SKIP (never fabricate).
    if (
      typeof o.chain_id !== "number" ||
      typeof o.cartridge_id !== "string" ||
      typeof o.is_opportunity !== "boolean" ||
      typeof o.had_reserves !== "boolean"
    ) {
      this.deps.logger.warn({ event: "rd_outcomes_archiver.incomplete_payload", id });
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    try {
      await this.deps.pool.query(
        `INSERT INTO route_discovery_outcomes
           (stream_id, schema_ver, ts_ms, chain_id, cartridge_id, tx_hash, source_event,
            pool_hint, token_in, token_out, is_opportunity, estimated_profit, confidence,
            urgency, had_reserves, mode)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
         ON CONFLICT (stream_id) DO NOTHING`,
        [
          id,
          o.schema ?? "rd_outcome_v1",
          o.ts_ms ?? 0,
          o.chain_id,
          o.cartridge_id,
          o.tx_hash ?? null,
          o.source_event ?? null,
          o.pool_hint ?? null,
          o.token_in ?? null,
          o.token_out ?? null,
          o.is_opportunity,
          o.estimated_profit ?? null,
          o.confidence ?? null,
          o.urgency ?? null,
          o.had_reserves,
          o.mode ?? "shadow",
        ],
      );
      await this.redis.xack(STREAM_IN, GROUP, id);
    } catch (err) {
      // Transient DB error — do NOT ack; a later read retries (at-least-once).
      this.deps.logger.error({
        event: "rd_outcomes_archiver.persist_err",
        id,
        err: (err as Error).message,
      });
    }
  }
}
