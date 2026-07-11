/**
 * PaperExecutor — Holonomic Loop Resolution Shadow Executor (Task 6).
 *
 * 100% PASSIVE / PAPER-ONLY. Consumes from `arbx:hot:simulated` stream and
 * simulates the execution phase with artificial latency and variance.
 *
 * It only:
 *   1. Reads simulated opportunities from Redis stream `arbx:hot:simulated`
 *      (consumer group: paper-executor-g0, at-least-once, XACK after persist)
 *   2. Processes only status=passed opportunities
 *   3. Simulates execution delay (10-50ms artificial)
 *   4. Calculates actual_pnl with variance (±5%)
 *   5. Persists to paper_trade_runs table
 *   6. XADD to arbx:hot:paper_executed stream
 *
 * It NEVER:
 *   - touches real capital or broadcasts transactions
 *   - fabricates data when fields are missing
 *   - executes on live networks
 */

import { Redis } from "ioredis";
import pg from "pg";

const STREAM_IN = "arbx:hot:simulated";
const STREAM_OUT = "arbx:hot:paper_executed";
const GROUP = "paper-executor-g0";
const CONSUMER = process.env["HOSTNAME"] ?? "paper-executor-1";

/** Minimal structural logger */
export interface ExecutorLogger {
  info(obj: object, msg?: string): void;
  warn(obj: object, msg?: string): void;
  error(obj: object, msg?: string): void;
}

export interface PaperExecutorDeps {
  /** Connection string for a DEDICATED Redis connection (XREADGROUP blocks) */
  redisUrl: string;
  /** Shared pg pool from the api-server (DATABASE_URL must be configured) */
  pool: pg.Pool;
  logger: ExecutorLogger;
}

/** Simulated opportunity from the hot path */
interface SimulatedOpportunity {
  id: string;
  status: "passed" | "failed";
  net_profit_wei?: string;
  gas_used?: number;
  timestamp_ms: number;
  opportunity_id?: string;
  chain_id?: number;
  strategy_kind?: string;
}

/** Extract the value for `key` from a flat XREAD kv array [k1,v1,k2,v2,...] */
function fieldValue(kv: string[], key: string): string | undefined {
  for (let i = 0; i + 1 < kv.length; i += 2) {
    if (kv[i] === key) return kv[i + 1];
  }
  return undefined;
}

/** Parse simulated opportunity from stream fields */
function parseSimulatedOpportunity(kv: string[]): SimulatedOpportunity | null {
  const id = fieldValue(kv, "id");
  const status = fieldValue(kv, "status");
  const net_profit_wei = fieldValue(kv, "net_profit_wei");
  const gas_used = fieldValue(kv, "gas_used");
  const timestamp_ms = fieldValue(kv, "timestamp_ms");
  const opportunity_id = fieldValue(kv, "opportunity_id");
  const chain_id = fieldValue(kv, "chain_id");
  const strategy_kind = fieldValue(kv, "strategy_kind");

  if (!id || !status) return null;

  return {
    id,
    status: status as "passed" | "failed",
    net_profit_wei,
    gas_used: gas_used ? parseInt(gas_used, 10) : undefined,
    timestamp_ms: timestamp_ms ? parseInt(timestamp_ms, 10) : Date.now(),
    opportunity_id,
    chain_id: chain_id ? parseInt(chain_id, 10) : undefined,
    strategy_kind,
  };
}

/** Convert wei to USD (simplified — uses fixed rate for paper mode) */
function weiToUsd(wei: string): number {
  const eth = Number(BigInt(wei)) / 1e18;
  // Paper mode fixed rate: 1 ETH = $3500 USD
  return eth * 3500;
}

/** Simulate execution with artificial delay and variance */
async function simulateExecution(simProfitUsd: number): Promise<{
  executionTimeMs: number;
  actualPnlUsd: number;
}> {
  // Artificial execution delay: 10-50ms
  const executionTimeMs = 10 + Math.random() * 40;

  // Simulate variance: ±5% on the topological yield
  const variance = (Math.random() - 0.5) * 0.1; // -5% to +5%
  const actualPnlUsd = simProfitUsd * (1 + variance);

  // Apply the delay
  await new Promise((resolve) => setTimeout(resolve, executionTimeMs));

  return { executionTimeMs: Math.round(executionTimeMs), actualPnlUsd };
}

export class PaperExecutor {
  private redis: Redis | null = null;
  private running = false;
  private stopPromise: Promise<void> | null = null;

  constructor(private readonly deps: PaperExecutorDeps) {}

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    // Dedicated connection: a blocking XREADGROUP must not stall the shared client
    this.redis = new Redis(this.deps.redisUrl, { maxRetriesPerRequest: 3 });
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info(
      { event: "paper_executor.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER },
      "Paper Executor consuming simulated opportunities"
    );
  }

  async stop(): Promise<void> {
    this.running = false;
    if (this.stopPromise) await this.stopPromise.catch(() => {});
    if (this.redis) await this.redis.quit().catch(() => {});
    this.deps.logger.info({ event: "paper_executor.stopped" }, "Paper Executor stopped");
  }

  private async ensureGroup(): Promise<void> {
    if (!this.redis) return;
    try {
      await this.redis.xgroup("CREATE", STREAM_IN, GROUP, "$", "MKSTREAM");
    } catch (e) {
      const msg = (e as Error).message;
      if (!msg.includes("BUSYGROUP")) {
        this.deps.logger.warn({ event: "paper_executor.group_create_err", err: msg });
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
          "STREAMS", STREAM_IN, ">"
        );
        if (!r || (r as unknown[]).length === 0) continue;
        for (const [, entries] of r as Array<[string, Array<[string, string[]]>]>) {
          for (const [id, kv] of entries) {
            await this.processOne(id, kv);
          }
        }
      } catch (err) {
        this.deps.logger.error({ event: "paper_executor.loop_err", err: (err as Error).message });
        await new Promise((res) => setTimeout(res, 1000));
      }
    }
  }

  private async processOne(id: string, kv: string[]): Promise<void> {
    if (!this.redis) return;

    // Parse — invalid/unparseable messages are SKIPPED + XACKed
    const sim = parseSimulatedOpportunity(kv);
    if (!sim) {
      this.deps.logger.warn({ event: "paper_executor.invalid_message", id }, "failed to parse message");
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // Only process passed opportunities (R8: fail-honest)
    if (sim.status !== "passed") {
      this.deps.logger.info(
        { event: "paper_executor.skip_failed", opportunity_id: sim.id, status: sim.status },
        "skipping non-passed opportunity"
      );
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // Fail-honest: skip if missing required fields
    if (!sim.net_profit_wei || !sim.opportunity_id || !sim.chain_id || !sim.strategy_kind) {
      this.deps.logger.info(
        { event: "paper_executor.skip_incomplete", opportunity_id: sim.id },
        "missing required fields — skipped"
      );
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    // Calculate simulated profit in USD
    const simProfitUsd = weiToUsd(sim.net_profit_wei);

    try {
      // Simulate execution with delay and variance
      const { executionTimeMs, actualPnlUsd } = await simulateExecution(simProfitUsd);

      // Persist to paper_trade_runs
      const paperRunId = await this.persistRun(sim, simProfitUsd, actualPnlUsd, executionTimeMs);

      // XADD to arbx:hot:paper_executed stream
      await this.redis.xadd(
        STREAM_OUT,
        "MAXLEN", "~", 1000,
        "*",
        "id", sim.id,
        "opportunity_id", sim.opportunity_id,
        "paper_run_id", paperRunId,
        "execution_time_ms", executionTimeMs.toString(),
        "paper_pnl_usd", actualPnlUsd.toFixed(6),
        "status", "completed",
        "timestamp_ms", Date.now().toString()
      );

      // Acknowledge the message
      await this.redis.xack(STREAM_IN, GROUP, id);

      this.deps.logger.info(
        {
          event: "paper_executor.completed",
          opportunity_id: sim.opportunity_id,
          paper_run_id: paperRunId,
          execution_time_ms: executionTimeMs,
          paper_pnl_usd: actualPnlUsd,
        },
        "paper execution completed"
      );
    } catch (err) {
      const code = (err as { code?: string }).code;
      if (code === "23503") {
        // FK: the opportunity row is not (yet) persisted
        this.deps.logger.warn(
          { event: "paper_executor.skip_opportunity_absent", opportunity_id: sim.opportunity_id },
          "opportunity not present in opportunities table — skipped"
        );
        await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
        return;
      }
      // Transient DB error — do NOT ack; a later read can retry
      this.deps.logger.error({
        event: "paper_executor.persist_err",
        opportunity_id: sim.opportunity_id,
        id,
        err: (err as Error).message,
      });
    }
  }

  private async persistRun(
    sim: SimulatedOpportunity,
    simProfitUsd: number,
    actualPnlUsd: number,
    executionTimeMs: number
  ): Promise<string> {
    const result = await this.deps.pool.query<{ id: string }>(
      `INSERT INTO paper_trade_runs
         (opportunity_id, chain_id, strategy_kind, sim_expected_profit_usd,
          sim_gas_cost_usd, sim_block_number, sim_timestamp,
          actual_profit_usd, actual_timestamp, execution_time_ms, reason)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
       RETURNING id`,
      [
        sim.opportunity_id,
        sim.chain_id,
        sim.strategy_kind,
        simProfitUsd,
        null, // sim_gas_cost_usd — not available from hot stream
        null, // sim_block_number — not available from hot stream
        new Date(sim.timestamp_ms),
        actualPnlUsd,
        new Date(),
        executionTimeMs,
        "paper_executor_shadow",
      ]
    );
    return result.rows[0].id;
  }
}
