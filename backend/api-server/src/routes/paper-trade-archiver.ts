/**
 * Shadow Archiver — paper_trade_runs telemetry sink (FASE OMEGA SHADOW).
 *
 * 100% PASSIVE. It only:
 *   1. reads detected-opportunity messages from Redis stream `arbx:opps:detected`
 *      (consumer group, at-least-once, XACK after persist), and
 *   2. INSERTs the opportunity's *own* sim prediction into `paper_trade_runs`.
 *
 * It NEVER:
 *   - computes/derives profit (it copies sim_expected_profit from the payload),
 *   - fabricates a row when the payload lacks a usable value (it SKIPs + XACKs),
 *   - touches capital, signers, execution, or any write outside paper_trade_runs.
 *
 * Mirrors selector-api's StreamConsumer (XREADGROUP/XACK) but with no scoring,
 * no decisioning, and no downstream publish.
 *
 * NOTE: in OrchestratorMode::Shadow the searcher's dry-run emitter writes nothing
 * to this stream, so pure shadow telemetry requires a follow-up (publish dry-run
 * results to a stream on the Rust side). In V1/V2 modes this captures every
 * detected opportunity's sim prediction, which is exactly the drift-analysis input.
 */

import { createHash } from "node:crypto";
import { Redis } from "ioredis";
import pg from "pg";
import { OpportunitySchema, type Opportunity } from "@arbx/shared";
import { isOutlierProfit, DEFAULT_OUTLIER_MULT, DEFAULT_CAPITAL_FLOOR_USD } from "../lib/paper-outlier-guard.js";

const STREAM_IN = "arbx:opps:detected";
const GROUP = "paper-archiver-g0";
const CONSUMER = process.env["HOSTNAME"] ?? "paper-archiver-1";

/** Minimal structural logger — satisfied by the pino logger in index.ts. */
export interface ArchiverLogger {
  info(obj: object, msg?: string): void;
  warn(obj: object, msg?: string): void;
  error(obj: object, msg?: string): void;
}

export interface PaperTradeArchiverDeps {
  /** Connection string for a DEDICATED Redis connection (XREADGROUP blocks). */
  redisUrl: string;
  /** Shared pg pool from the api-server (DATABASE_URL must be configured). */
  pool: pg.Pool;
  logger: ArchiverLogger;
  /**
   * A-02 outlier guard: any |sim_expected_profit_usd| above this many × the
   * operator's capital is quarantined (token-as-USD from unsized engines /
   * Rhai `profit_usd_hint` smells exactly like this — $49–59M on a $1k capital).
   * Default 10×. The capital floor (default $1000) bounds the threshold when
   * trading_config.capital_usd is unavailable.
   */
  outlierMultiplier?: number;
  capitalFloorUsd?: number;
}

/** Extract the value for `key` from a flat XREAD kv array [k1,v1,k2,v2,...]. */
function fieldValue(kv: string[], key: string): string | undefined {
  for (let i = 0; i + 1 < kv.length; i += 2) {
    if (kv[i] === key) return kv[i + 1];
  }
  return undefined;
}

/** Deterministic route fingerprint from REAL payload fields (no invention). */
function routeHash(opp: Opportunity): string {
  const canonical = [
    opp.chain_id,
    opp.strategy_kind,
    opp.dex_a,
    opp.dex_b ?? "",
    opp.token_in,
    opp.token_out,
  ].join("|");
  return "rh:" + createHash("sha256").update(canonical).digest("hex").slice(0, 32);
}

export class PaperTradeArchiver {
  private redis: Redis | null = null;
  private running = false;
  private stopPromise: Promise<void> | null = null;
  /**
   * A-02: operator capital (USD) for the outlier guard. Refreshed from
   * trading_config via setCapitalUsd (called by index.ts on boot + config
   * reload). 0/unset → isOutlierProfit falls back to the floor.
   */
  private capitalUsd = 0;

  constructor(private readonly deps: PaperTradeArchiverDeps) {}

  /** A-02: update the capital floor used by the outlier guard. */
  setCapitalUsd(usd: number): void {
    this.capitalUsd = Number.isFinite(usd) && usd > 0 ? usd : 0;
  }

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    // Dedicated connection: a blocking XREADGROUP must not stall the shared client.
    this.redis = new Redis(this.deps.redisUrl, { maxRetriesPerRequest: 3 });
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info(
      { event: "paper_archiver.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER },
      "Shadow Archiver consuming detected opportunities",
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
        this.deps.logger.warn({ event: "paper_archiver.group_create_err", err: msg });
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
        this.deps.logger.error({ event: "paper_archiver.loop_err", err: (err as Error).message });
        await new Promise((res) => setTimeout(res, 1000));
      }
    }
  }

  private async processOne(id: string, kv: string[]): Promise<void> {
    if (!this.redis) return;

    // Parse — invalid/unparseable messages are SKIPPED + XACKed (never invented).
    let opp: Opportunity;
    try {
      const json = fieldValue(kv, "json");
      if (!json) throw new Error("no_json_field");
      opp = OpportunitySchema.parse(JSON.parse(json));
    } catch (err) {
      this.deps.logger.warn({ event: "paper_archiver.invalid_message", id, err: (err as Error).message });
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // sim_expected_profit_usd is NOT NULL in the schema. We COPY it from the
    // payload — never compute it. If the payload carries neither net nor gross
    // expected profit, we honestly SKIP (do NOT fabricate a value).
    const simProfit = opp.net_expected_profit_usd ?? opp.expected_profit_usd ?? null;
    if (simProfit === null || simProfit === undefined) {
      this.deps.logger.info(
        { event: "paper_archiver.skip_no_sim_profit", opportunity_id: opp.id },
        "no sim profit in payload — skipped (not fabricated)",
      );
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // A-02 outlier guard: some unsized emit paths (Rhai profit_usd_hint,
    // backrun/cex_dex placeholders) can carry token-as-USD magnitudes ($49–59M
    // on a $1k capital). Quarantine implausible values rather than letting them
    // contaminate the paper-history average ($10M fictitious). SKIP + XACK.
    const mult = this.deps.outlierMultiplier ?? DEFAULT_OUTLIER_MULT;
    const floor = this.deps.capitalFloorUsd ?? DEFAULT_CAPITAL_FLOOR_USD;
    if (isOutlierProfit(simProfit, this.capitalUsd, mult, floor)) {
      this.deps.logger.warn(
        {
          event: "paper_archiver.outlier_quarantined",
          opportunity_id: opp.id,
          sim_profit_usd: simProfit,
          capital_usd: this.capitalUsd,
          threshold: mult,
        },
        "sim profit exceeds outlier threshold — quarantined (token-as-USD suspected)",
      );
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    try {
      await this.deps.pool.query(
        `INSERT INTO paper_trade_runs
           (opportunity_id, chain_id, strategy_kind, sim_expected_profit_usd,
            sim_block_number, reason, route_hash)
         VALUES ($1, $2, $3, $4, $5, $6, $7)`,
        [
          opp.id,
          opp.chain_id,
          opp.strategy_kind,
          simProfit,
          opp.block_number ?? null,
          opp.rejection_reason ?? null,
          routeHash(opp),
        ],
      );
      await this.redis.xack(STREAM_IN, GROUP, id);
    } catch (err) {
      const code = (err as { code?: string }).code;
      if (code === "23503") {
        // FK: the opportunity row is not (yet) persisted. Passive sink — do not
        // fail the pipeline, do not retry forever. Log + XACK + move on.
        this.deps.logger.warn(
          { event: "paper_archiver.skip_opportunity_absent", opportunity_id: opp.id },
          "opportunity not present in opportunities table — skipped",
        );
        await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
        return;
      }
      // Transient DB error — do NOT ack; a later read can retry.
      this.deps.logger.error({
        event: "paper_archiver.persist_err",
        opportunity_id: opp.id,
        id,
        err: (err as Error).message,
      });
    }
  }
}
