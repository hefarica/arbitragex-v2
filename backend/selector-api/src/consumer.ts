/**
 * Redis Streams consumer for selector-api.
 *
 * Reads `arbx:opps:detected` via consumer group "selector-g0".
 * Flow per message:
 *   parse → prefilter → safety → score → decide → persist → (if accept) publish validated → XACK
 *
 * At-least-once semantics: XACK only after successful persist. On crash mid-process
 * the entry stays in the group PEL; the maintenance sweep (WO-D8) XAUTOCLAIMs
 * stale pending entries from orphaned consumers and reprocesses them.
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
// WO-D8 (schema-drift-2026-09-20): consumers are named by HOSTNAME, so every
// container restart leaves an orphan holding claimed-but-unacked entries
// (6 096 pending measured). XREADGROUP ">" never sees them again.
const RECLAIM_MIN_IDLE_MS = 60_000;
const ORPHAN_PURGE_IDLE_MS = 300_000;
const SWEEP_INTERVAL_MS = 60_000;
const SWEEP_FIRST_DELAY_MS = 30_000;

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
  private sweepPromise: Promise<void> | null = null;
  // A5-STALL (2026-08-29): the kill-switch halt was 100% silent — 4 days of
  // zero consumption with zero logs. Transition + 10-min summary logs only
  // (R9: no per-loop flooding).
  private haltStartedAt: number | null = null;
  private haltLastLoggedAt = 0;
  constructor(private readonly deps: ConsumerDeps) {}

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.sweepPromise = this.maintenanceLoop();
    this.deps.logger.info({ event: "consumer.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER });
  }

  async stop(): Promise<void> {
    this.running = false;
    if (this.stopPromise) await this.stopPromise.catch(() => {});
    if (this.sweepPromise) await this.sweepPromise.catch(() => {});
    // Zero-loss deregistration: only remove our own consumer if its PEL is
    // empty — otherwise leave it for the next boot's sweep to XAUTOCLAIM its
    // pending before purging (DELCONSUMER would drop unacked entries).
    try {
      const consumers = (await this.deps.redis.xinfo("CONSUMERS", STREAM_IN, GROUP)) as unknown[];
      const self = consumers.map(normalizeConsumer).find((c) => String(c?.name ?? "") === CONSUMER);
      if (self && Number(self?.pending ?? 0) === 0) {
        await this.deps.redis.xgroup("DELCONSUMER", STREAM_IN, GROUP, CONSUMER);
      }
    } catch {
      // best-effort: SIGKILL never reaches this path; the sweep covers it.
    }
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
        // Honor kill-switch — if ON, sleep and retry. Log the halt loudly
        // (A5-STALL): a fail-closed default after Redis key loss previously
        // paused consumption here with no observable trace.
        if (await this.deps.killSwitch.isEnabled()) {
          const now = Date.now();
          if (this.haltStartedAt === null) {
            this.haltStartedAt = now;
            this.haltLastLoggedAt = now;
            this.deps.logger.warn({
              event: "consumer.halted_kill_switch",
              detail: "kill-switch enabled (explicit arm or fail-closed default after Redis key loss) — stream consumption paused",
            });
          } else if (now - this.haltLastLoggedAt >= 600_000) {
            this.haltLastLoggedAt = now;
            this.deps.logger.warn({
              event: "consumer.still_halted_kill_switch",
              halted_for_s: Math.round((now - this.haltStartedAt) / 1000),
            });
          }
          await sleep(5000);
          continue;
        }
        if (this.haltStartedAt !== null) {
          this.deps.logger.warn({
            event: "consumer.resumed_after_kill_switch",
            halted_for_s: Math.round((Date.now() - this.haltStartedAt) / 1000),
          });
          this.haltStartedAt = null;
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

  /**
   * WO-D8: periodic group-hygiene sweep. Short first delay after boot so a
   * fresh instance reclaims the predecessor's backlog without waiting a full
   * interval. Skipped while the kill-switch is armed — reclaiming would
   * process (and consume) backlog the main loop is deliberately holding.
   */
  private async maintenanceLoop(): Promise<void> {
    let waitMs = SWEEP_FIRST_DELAY_MS;
    while (this.running) {
      await sleep(waitMs);
      waitMs = SWEEP_INTERVAL_MS;
      if (!this.running) break;
      try {
        if (await this.deps.killSwitch.isEnabled()) continue;
        await this.purgeOrphanConsumers();
      } catch (e) {
        this.deps.logger.warn({ event: "consumer.sweep_err", err: (e as Error).message });
      }
    }
  }

  /**
   * Reclaims each orphan's pending FIRST (XAUTOCLAIM to this live consumer +
   * reprocess via processOne, which acks on success), only then DELCONSUMER —
   * DELCONSUMER alone would drop its PEL entries from the group, never
   * re-delivered to anyone (pattern proven in api-server/src/websocket.ts WO-15).
   */
  private async purgeOrphanConsumers(): Promise<void> {
    let consumers: unknown[] = [];
    try {
      consumers = (await this.deps.redis.xinfo("CONSUMERS", STREAM_IN, GROUP)) as unknown[];
    } catch (e) {
      this.deps.logger.warn({ event: "consumer.sweep_xinfo_err", err: (e as Error).message });
      return;
    }
    let purged = 0;
    let reclaimed = 0;
    let discarded = 0;
    for (const raw of consumers) {
      const c = normalizeConsumer(raw);
      const name = String(c?.name ?? "");
      const pending = Number(c?.pending ?? 0);
      const idle = Number(c?.idle ?? 0);
      if (!name || name === CONSUMER) continue; // self
      if (idle < ORPHAN_PURGE_IDLE_MS) continue; // live peer or recent predecessor
      if (pending > 0) {
        reclaimed += await this.reclaimPending();
      }
      try {
        const removed = Number(await this.deps.redis.xgroup("DELCONSUMER", STREAM_IN, GROUP, name));
        if (removed > 0) discarded += removed;
        purged++;
      } catch (e) {
        this.deps.logger.warn({ event: "consumer.delconsumer_err", name, err: (e as Error).message });
      }
    }
    // R9: one aggregated summary per sweep — never per-consumer logs.
    if (purged > 0 || reclaimed > 0) {
      this.deps.logger.info({
        event: "consumer.group_hygiene",
        stream: STREAM_IN,
        purged,
        reclaimed,
        discarded_pending: discarded,
      });
    }
  }

  /** XAUTOCLAIM loop: claims entries idle > RECLAIM_MIN_IDLE_MS to this
   * consumer and reprocesses them (processOne acks on success; failures stay
   * unacked in our PEL and are retried by the next sweep). */
  private async reclaimPending(): Promise<number> {
    let cursor = "0-0";
    let iterations = 0;
    let claimed = 0;
    do {
      const res = await this.deps.redis.xautoclaim(
        STREAM_IN, GROUP, CONSUMER,
        RECLAIM_MIN_IDLE_MS, cursor, "COUNT", 100,
      ) as [string, Array<[string, string[] | null]>] | null;
      cursor = res?.[0] ?? "0-0";
      for (const [id, kv] of res?.[1] ?? []) {
        if (!kv) {
          // Entry was trimmed from the stream but still sits in the PEL —
          // clear it so the group's pending count is honest.
          await this.deps.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
          continue;
        }
        await this.processOne(id, kv);
        claimed++;
      }
      iterations++;
      // Safety cap: 100 iterations × COUNT 100 = 10_000 entries per sweep;
      // if the cursor is still open the next sweep resumes from scratch.
    } while (cursor !== "0-0" && iterations < 100 && this.running);
    return claimed;
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

    // Worst-of-pair safety (2026-08-18, "real y confiable"): a route is only
    // as safe as its LEAST safe token. Previously only token_in was checked —
    // a WETH→SCAM pair sailed through on WETH's score alone. Both legs are
    // now checked (token_out on its destination chain when the opportunity
    // declares one) and the decision consumes the lower record.
    const safetyIn = await checkToken(this.deps.pool, this.deps.tokenSafetyCb, this.deps.cfg, opp.chain_id, opp.token_in);
    const safetyOut = await checkToken(this.deps.pool, this.deps.tokenSafetyCb, this.deps.cfg, opp.chain_id_out ?? opp.chain_id, opp.token_out);
    const safety = safetyIn.safety_score <= safetyOut.safety_score ? safetyIn : safetyOut;
    // SEL-03 note (2026-09-24): sim=null means the simulation_failed and
    // revert_risk_too_high gates in decide() are structurally non-operative
    // (consumer-side; tests DO cover decide() with sim — false confidence).
    // Wire sim-ctl to this consumer or mark the gates as pending in the docs.
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

// ioredis returns XINFO CONSUMERS as RAW ARRAYS of key/value pairs
// ([["name","c1","pending",0,...],...]), not objects — property access
// silently skipped every consumer (WO-15 hotfix in api-server). Normalize
// both shapes before use.
function normalizeConsumer(c: unknown): { name?: unknown; pending?: unknown; idle?: unknown } {
  if (Array.isArray(c)) {
    const rec: Record<string, unknown> = {};
    for (let i = 0; i + 1 < c.length; i += 2) rec[String(c[i])] = c[i + 1];
    return rec as { name?: unknown; pending?: unknown; idle?: unknown };
  }
  return (c ?? {}) as { name?: unknown; pending?: unknown; idle?: unknown };
}

function sleep(ms: number): Promise<void> {
  return new Promise(res => setTimeout(res, ms));
}
