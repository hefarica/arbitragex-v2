/**
 * Scored-opportunities Archiver — Gate C scoring telemetry sink.
 *
 * 100% PASSIVE. It only:
 *   1. reads ConfidenceScore messages from Redis stream `arbx:scoring:scored`
 *      (consumer group, at-least-once, XACK after persist), produced by the
 *      Rust `OpportunityEmitter` (Gate C scoring wired at the real emit path), and
 *   2. INSERTs each scored opportunity into `scored_opportunities`.
 *
 * It NEVER:
 *   - computes/derives a score (it copies the Rust-computed ConfidenceScore),
 *   - fabricates a row when the payload is unparseable (it SKIPs + XACKs),
 *   - touches capital, signers, execution, or any write outside scored_opportunities.
 *
 * Twin of paper-trade-archiver.ts (XREADGROUP/XACK). `recommended_usd` is a
 * HYPOTHETICAL paper sizing figure, never real capital. Idempotent at-least-once
 * consume via `scored_opportunities.stream_id UNIQUE` + `ON CONFLICT DO NOTHING`.
 */

import { Redis } from "ioredis";
import pg from "pg";
import { z } from "zod";

const STREAM_IN = "arbx:scoring:scored";
const GROUP = "scoring-archiver-g0";
const CONSUMER = process.env["HOSTNAME"] ?? "scoring-archiver-1";

/** Minimal structural logger — satisfied by the pino logger in index.ts. */
export interface ArchiverLogger {
  info(obj: object, msg?: string): void;
  warn(obj: object, msg?: string): void;
  error(obj: object, msg?: string): void;
}

export interface ScoredOpportunitiesArchiverDeps {
  /** Connection string for a DEDICATED Redis connection (XREADGROUP blocks). */
  redisUrl: string;
  /** Shared pg pool from the api-server (DATABASE_URL must be configured). */
  pool: pg.Pool;
  logger: ArchiverLogger;
}

/**
 * Wire contract for one scored opportunity. Mirrors the Rust XADD payload
 * (`opportunity_emitter::score_and_publish`). Only REAL computed fields — no
 * invention. `.passthrough()` is NOT used; unknown fields are dropped.
 */
const ScoredRecordSchema = z.object({
  opportunity_id: z.string().min(1),
  token_pair: z.string().min(1),
  posterior_prob: z.number().finite(),
  kelly_fraction: z.number().finite(),
  recommended_usd: z.number().finite(),
  net_profit_usd: z.number().finite().nullable().optional(),
  bayesian_accepted: z.boolean(),
  prior_log_odds: z.number().finite().nullable().optional(),
  chain_id: z.number().int().nullable().optional(),
  source_context: z.string().nullable().optional(),
  scoring_mode: z.string().default("paper"),
  evidence_vector: z.unknown().nullable().optional(),
});
type ScoredRecord = z.infer<typeof ScoredRecordSchema>;

/** Extract the value for `key` from a flat XREAD kv array [k1,v1,k2,v2,...]. */
function fieldValue(kv: string[], key: string): string | undefined {
  for (let i = 0; i + 1 < kv.length; i += 2) {
    if (kv[i] === key) return kv[i + 1];
  }
  return undefined;
}

export class ScoredOpportunitiesArchiver {
  private redis: Redis | null = null;
  private running = false;
  private stopPromise: Promise<void> | null = null;

  constructor(private readonly deps: ScoredOpportunitiesArchiverDeps) {}

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    // Dedicated connection: a blocking XREADGROUP must not stall the shared client.
    this.redis = new Redis(this.deps.redisUrl, { maxRetriesPerRequest: 3 });
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info(
      { event: "scoring_archiver.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER },
      "Gate C scoring archiver consuming scored opportunities",
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
        this.deps.logger.warn({ event: "scoring_archiver.group_create_err", err: msg });
      }
    }
  }

  private async runLoop(): Promise<void> {
    while (this.running && this.redis) {
      try {
        const r = await this.redis.xreadgroup(
          "GROUP", GROUP, CONSUMER,
          "COUNT", 32,
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
        this.deps.logger.error({ event: "scoring_archiver.loop_err", err: (err as Error).message });
        await new Promise((res) => setTimeout(res, 1000));
      }
    }
  }

  private async processOne(id: string, kv: string[]): Promise<void> {
    if (!this.redis) return;

    // Parse — invalid/unparseable messages are SKIPPED + XACKed (never invented).
    let rec: ScoredRecord;
    try {
      const json = fieldValue(kv, "json");
      if (!json) throw new Error("no_json_field");
      rec = ScoredRecordSchema.parse(JSON.parse(json));
    } catch (err) {
      this.deps.logger.warn({ event: "scoring_archiver.invalid_message", id, err: (err as Error).message });
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    try {
      // stream_id UNIQUE → idempotent at-least-once consume.
      await this.deps.pool.query(
        `INSERT INTO scored_opportunities
           (stream_id, opportunity_id, token_pair, posterior_prob, kelly_fraction,
            recommended_usd, net_profit_usd, bayesian_accepted, prior_log_odds,
            chain_id, source_context, scoring_mode, evidence_vector)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         ON CONFLICT (stream_id) DO NOTHING`,
        [
          id,
          rec.opportunity_id,
          rec.token_pair,
          rec.posterior_prob,
          rec.kelly_fraction,
          rec.recommended_usd,
          rec.net_profit_usd ?? null,
          rec.bayesian_accepted,
          rec.prior_log_odds ?? null,
          rec.chain_id ?? null,
          rec.source_context ?? null,
          rec.scoring_mode,
          rec.evidence_vector ? JSON.stringify(rec.evidence_vector) : null,
        ],
      );
      await this.redis.xack(STREAM_IN, GROUP, id);
    } catch (err) {
      // Transient DB error — do NOT ack; a later read can retry.
      this.deps.logger.error({
        event: "scoring_archiver.persist_err",
        opportunity_id: rec.opportunity_id,
        id,
        err: (err as Error).message,
      });
    }
  }
}
