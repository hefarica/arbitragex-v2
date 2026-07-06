/**
 * Redis Streams consumer for selector-api.
 *
 * Reads `arbx:opps:detected` via consumer group "selector-g0".
 * Flow per message:
 *   parse → prefilter → safety → score → decide → persist → (if accept) publish validated → XACK
 *
 * At-least-once semantics: XACK only after successful persist. On crash mid-process,
 * next instance XREADGROUP picks pending messages (XPENDING/XAUTOCLAIM — added in S8
 * for HA; for now, single consumer + manual cleanup if needed).
 */

import { Redis } from "ioredis";
import type pg from "pg";
import type { Logger } from "pino";
import type { AppConfig, CircuitBreaker, KillSwitchClient } from "@arbx/shared";
import { OpportunitySchema, type Opportunity } from "@arbx/shared";
import { prefilter, decide, type Decision } from "./policy/engine.js";
import { scoreOpportunity, weightsFromConfig } from "./scoring/engine.js";
import { checkToken } from "./token_safety/client.js";
import { persistDecision } from "./persistence.js";

const STREAM_IN = "arbx:opps:detected";
const STREAM_OUT = "arbx:opps:validated";
const GROUP = "selector-g0";
const CONSUMER = process.env["HOSTNAME"] ?? "selector-1";
const MAXLEN_OUT = 10_000;

export interface ConsumerDeps {
  redis: Redis;
  pool: pg.Pool;
  logger: Logger;
  cfg: AppConfig;
  killSwitch: KillSwitchClient;
  tokenSafetyCb: CircuitBreaker;
  dbWritesCb: CircuitBreaker;
  streamConsumerCb: CircuitBreaker;
  metrics: {
    decisionsTotal: (labels: Record<string, string>) => void;
    processingSeconds: (v: number) => void;
    invalidMessagesTotal: () => void;
    lagGauge: (n: number) => void;
  };
}

export class StreamConsumer {
  private running = false;
  private stopPromise: Promise<void> | null = null;
  constructor(private readonly deps: ConsumerDeps) {}

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info({ event: "consumer.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER });
  }

  async stop(): Promise<void> {
    this.running = false;
    if (this.stopPromise) await this.stopPromise.catch(() => {});
  }

  private async ensureGroup(): Promise<void> {
    try {
      await this.deps.redis.xgroup("CREATE", STREAM_IN, GROUP, "$", "MKSTREAM");
      this.deps.logger.info({ event: "consumer.group_created", stream: STREAM_IN, group: GROUP });
    } catch (e) {
      const msg = (e as Error).message;
      if (!msg.includes("BUSYGROUP")) {
        this.deps.logger.warn({ event: "consumer.group_create_err", err: msg });
      }
    }
  }

  private async runLoop(): Promise<void> {
    while (this.running) {
      try {
        // Honor kill-switch — if ON, sleep and retry.
        if (await this.deps.killSwitch.isEnabled()) {
          await sleep(5000);
          continue;
        }

        await this.deps.streamConsumerCb.execute(() => this.readBatch());
      } catch (err) {
        this.deps.logger.error({ event: "consumer.loop_err", err: (err as Error).message });
        await sleep(1000);
      }
    }
  }

  /** One XREADGROUP call. Blocks up to 2s for new messages. */
  private async readBatch(): Promise<void> {
    const r = await this.deps.redis.xreadgroup(
      "GROUP", GROUP, CONSUMER,
      "COUNT", 16,
      "BLOCK", 2000,
      "STREAMS", STREAM_IN, ">",
    );
    if (!r || r.length === 0) {
      const lag = await this.lagFor(STREAM_IN, GROUP);
      this.deps.metrics.lagGauge(lag);
      return;
    }
    // r is [[stream_key, [[id, [field1, val1, field2, val2, ...]], ...]]]
    for (const [, entries] of r as Array<[string, Array<[string, string[]]>]>) {
      for (const [id, kv] of entries) {
        await this.processOne(id, kv);
      }
    }
  }

  private async lagFor(stream: string, group: string): Promise<number> {
    try {
      const info = await this.deps.redis.xpending(stream, group) as unknown as [number, string, string, Array<[string, number]>];
      return info?.[0] ?? 0;
    } catch {
      return 0;
    }
  }

  private async processOne(id: string, kv: string[]): Promise<void> {
    const t0 = Date.now();
    let opportunity: Opportunity | null = null;
    try {
      const json = fieldValue(kv, "json");
      if (!json) throw new Error("no_json_field");
      opportunity = OpportunitySchema.parse(JSON.parse(json));
    } catch (err) {
      this.deps.logger.warn({ event: "consumer.invalid_message", id, err: (err as Error).message });
      this.deps.metrics.invalidMessagesTotal();
      // XACK invalid messages to stop replay loops
      await this.deps.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    try {
      const decision = await this.decideForOpportunity(opportunity);
      await this.deps.dbWritesCb.execute(() =>
        persistDecision(this.deps.pool, opportunity!.id, opportunity!.chain_id, opportunity!.trace_id, decision),
      );

      if (decision.kind === "accept") {
        await this.publishValidated(opportunity, decision.score);
      }

      this.deps.metrics.decisionsTotal({
        decision: decision.kind,
        reason: decision.kind === "reject" ? decision.reason : "ok",
        chain_id: String(opportunity.chain_id),
      });

      await this.deps.redis.xack(STREAM_IN, GROUP, id);
    } catch (err) {
      // Do NOT ack — next iteration can retry (or XAUTOCLAIM by another consumer in S8).
      this.deps.logger.error({
        event: "consumer.process_err",
        opportunity_id: opportunity?.id, id, err: (err as Error).message,
      });
    } finally {
      this.deps.metrics.processingSeconds((Date.now() - t0) / 1000);
    }
  }

  private async decideForOpportunity(opp: Opportunity): Promise<Decision> {
    const pre = await prefilter(this.deps.redis, {
      opportunity: opp,
      killSwitchOn: await this.deps.killSwitch.isEnabled(),
      tokenSafetyCb: this.deps.tokenSafetyCb,
    });
    if (pre) return pre;

    const safety = await checkToken(this.deps.pool, this.deps.tokenSafetyCb, this.deps.cfg, opp.chain_id, opp.token_in);
    // Note: for paired safety we could check token_out too; kept single for S3 minimalism.
    const scored = scoreOpportunity(opp, null, safety.safety_score, weightsFromConfig(this.deps.cfg), this.deps.cfg.risk.max_gas_price_gwei);

    return decide({ scored, safety, sim: null, cfg: this.deps.cfg });
  }

  private async publishValidated(opp: Opportunity, score: number): Promise<void> {
    const payload = JSON.stringify({ ...opp, risk_score: score });
    await this.deps.redis.xadd(
      STREAM_OUT, "MAXLEN", "~", MAXLEN_OUT, "*",
      "json", payload,
    );
  }
}

function fieldValue(kv: string[], field: string): string | null {
  for (let i = 0; i + 1 < kv.length; i += 2) {
    if (kv[i] === field) return kv[i + 1] ?? null;
  }
  return null;
}

function sleep(ms: number): Promise<void> {
  return new Promise(res => setTimeout(res, ms));
}
