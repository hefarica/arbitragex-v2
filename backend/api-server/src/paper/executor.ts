/**
 * PaperExecutor — Holonomic Loop Resolution Shadow Executor (Task 6).
 *
 * 100% PASSIVE / PAPER-ONLY. Consumes from arbx:hot:simulated stream and
 * simulates the execution phase with fail-honest pattern (R8).
 *
 * OMEGA Pipeline Task 6 Requirements:
 *   - Consumer Group: paper-executor-g0
 *   - Consumer Name: api-server-{instance_id}
 *   - XREADGROUP with BLOCK 100
 *   - Calculates net_topological_yield_wei = gross_yield - gas_cost - decoherence_penalty
 *   - Persists to paper_trade_runs table
 *   - Emits to arbx:hot:paper_executed stream (MAXLEN ~5000)
 *   - Metrics: arbx:metrics:throughput:paper_executed + latency histogram
 */

import { Redis } from "ioredis";
import pg from "pg";

const STREAM_IN = "arbx:hot:simulated";
const STREAM_OUT = "arbx:hot:paper_executed";
const GROUP = "paper-executor-g0";
const INSTANCE_ID = process.env["HOSTNAME"] || `api-server-${process.pid}`;
const CONSUMER = `api-server-${INSTANCE_ID}`;

export interface ExecutorLogger {
  info(obj: object, msg?: string): void;
  warn(obj: object, msg?: string): void;
  error(obj: object, msg?: string): void;
}

export interface PaperExecutorDeps {
  redisUrl: string;
  pool: pg.Pool;
  logger: ExecutorLogger;
}

interface SimulatedOpportunity {
  id: string;
  status: "passed" | "failed";
  gross_topological_yield_wei: string | undefined;
  net_topological_yield_wei: string | undefined;
  gas_cost_wei: string | undefined;
  decoherence_penalty_wei: string | undefined;
  timestamp_ms: number;
  opportunity_id: string | undefined;
  chain_id: number | undefined;
  strategy_kind: string | undefined;
  token_pair: string | undefined;
}

interface PaperExecutionResult {
  id: string;
  status: "ACCEPTED" | "REJECTED";
  net_yield_wei: string;
  executed_at_ms: number;
  rejection_reason: string | undefined;
  execution_time_ms: number;
}

function fieldValue(kv: string[], key: string): string | undefined {
  for (let i = 0; i + 1 < kv.length; i += 2) {
    if (kv[i] === key) return kv[i + 1];
  }
  return undefined;
}


function parseSimulatedOpportunity(kv: string[]): SimulatedOpportunity | null {
  const id = fieldValue(kv, "id");
  const status = fieldValue(kv, "status");
  const gross_yield = fieldValue(kv, "gross_topological_yield_wei") || fieldValue(kv, "gross_yield_wei") || fieldValue(kv, "net_profit_wei");
  const net_yield = fieldValue(kv, "net_topological_yield_wei") || fieldValue(kv, "net_yield_wei");
  const gas_cost = fieldValue(kv, "gas_cost_wei") || fieldValue(kv, "gas_used");
  const decoherence = fieldValue(kv, "decoherence_penalty_wei") || fieldValue(kv, "slippage_wei");
  const timestamp_ms = fieldValue(kv, "timestamp_ms");
  const opportunity_id = fieldValue(kv, "opportunity_id");
  const chain_id = fieldValue(kv, "chain_id");
  const strategy_kind = fieldValue(kv, "strategy_kind");
  const token_pair = fieldValue(kv, "token_pair");

  if (!id || !status) return null;

  return {
    id,
    status: status as "passed" | "failed",
    gross_topological_yield_wei: gross_yield,
    net_topological_yield_wei: net_yield,
    gas_cost_wei: gas_cost,
    decoherence_penalty_wei: decoherence,
    timestamp_ms: timestamp_ms ? parseInt(timestamp_ms, 10) : Date.now(),
    opportunity_id,
    chain_id: chain_id ? parseInt(chain_id, 10) : undefined,
    strategy_kind,
    token_pair,
  };
}

function calculateNetTopologicalYield(
  sim: SimulatedOpportunity
): { net_yield_wei: bigint; calculation_basis: string } | null {
  if (sim.net_topological_yield_wei) {
    try {
      return {
        net_yield_wei: BigInt(sim.net_topological_yield_wei),
        calculation_basis: "pre_calculated_net",
      };
    } catch {
      // Fall through
    }
  }

  if (sim.gross_topological_yield_wei) {
    try {
      const gross = BigInt(sim.gross_topological_yield_wei);
      const gas = sim.gas_cost_wei ? BigInt(sim.gas_cost_wei) : BigInt(0);
      const decoherence = sim.decoherence_penalty_wei ? BigInt(sim.decoherence_penalty_wei) : BigInt(0);
      const net = gross - gas - decoherence;
      return {
        net_yield_wei: net,
        calculation_basis: "calculated_gross_minus_costs",
      };
    } catch {
      return null;
    }
  }

  return null;
}

function weiToUsd(wei: bigint): number {
  const eth = Number(wei) / 1e18;
  return eth * 3500;
}


export class PaperExecutor {
  private redis: Redis | null = null;
  private running = false;
  private stopPromise: Promise<void> | null = null;

  constructor(private readonly deps: PaperExecutorDeps) {}

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    this.redis = new Redis(this.deps.redisUrl, { maxRetriesPerRequest: 3 });
    await this.ensureGroup();
    this.stopPromise = this.runLoop();
    this.deps.logger.info(
      { event: "paper_executor.started", stream: STREAM_IN, group: GROUP, consumer: CONSUMER },
      "Paper Executor consuming simulated opportunities (Task 6)"
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
      await this.redis.xgroup("CREATE", STREAM_IN, GROUP, "0", "MKSTREAM");
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
          "BLOCK", 100,
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
    const processStartMs = Date.now();

    const sim = parseSimulatedOpportunity(kv);
    if (!sim) {
      this.deps.logger.warn({ event: "paper_executor.invalid_message", id }, "failed to parse message");
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    if (sim.status !== "passed") {
      this.deps.logger.info(
        { event: "paper_executor.skip_failed", opportunity_id: sim.id, status: sim.status },
        "skipping non-passed opportunity"
      );
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    if (!sim.opportunity_id || !sim.chain_id || !sim.strategy_kind) {
      this.deps.logger.info(
        { event: "paper_executor.skip_incomplete", opportunity_id: sim.id },
        "missing required fields - skipped"
      );
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }


    const yieldCalc = calculateNetTopologicalYield(sim);
    if (!yieldCalc) {
      this.deps.logger.info(
        { event: "paper_executor.calculation_failed", opportunity_id: sim.id },
        "could not calculate net topological yield - skipped"
      );
      await this.emitResult({
        id: sim.id,
        status: "REJECTED",
        net_yield_wei: "0",
        executed_at_ms: Date.now(),
        rejection_reason: "calculation_failed",
        execution_time_ms: Date.now() - processStartMs,
      });
      await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
      return;
    }

    const { net_yield_wei, calculation_basis } = yieldCalc;
    const executionTimeMs = Date.now() - processStartMs;

    const isAccepted = net_yield_wei > BigInt(0);
    const status: "ACCEPTED" | "REJECTED" = isAccepted ? "ACCEPTED" : "REJECTED";
    const rejectionReason = isAccepted ? undefined : "net_yield_non_positive";

    try {
      let paperRunId: string | undefined;
      if (isAccepted) {
        paperRunId = await this.persistRun(sim, net_yield_wei, executionTimeMs, calculation_basis);
      }

      await this.emitResult({
        id: sim.id,
        status,
        net_yield_wei: net_yield_wei.toString(),
        executed_at_ms: Date.now(),
        rejection_reason: rejectionReason,
        execution_time_ms: executionTimeMs,
      });

      await this.updateMetrics(status, executionTimeMs);
      await this.redis.xack(STREAM_IN, GROUP, id);

      this.deps.logger.info(
        {
          event: "paper_executor.completed",
          opportunity_id: sim.opportunity_id,
          paper_run_id: paperRunId,
          status,
          net_yield_wei: net_yield_wei.toString(),
          execution_time_ms: executionTimeMs,
          calculation_basis,
        },
        `paper execution ${status.toLowerCase()}`
      );
    } catch (err) {
      const code = (err as { code?: string }).code;
      if (code === "23503") {
        this.deps.logger.warn(
          { event: "paper_executor.skip_opportunity_absent", opportunity_id: sim.opportunity_id },
          "opportunity not present in opportunities table - skipped"
        );
        await this.redis.xack(STREAM_IN, GROUP, id).catch(() => {});
        return;
      }
      this.deps.logger.error({
        event: "paper_executor.persist_err",
        opportunity_id: sim.opportunity_id,
        id,
        err: (err as Error).message,
      });
    }
  }


  private async emitResult(result: PaperExecutionResult): Promise<void> {
    if (!this.redis) return;

    const fields: (string | number)[] = [
      "id", result.id,
      "status", result.status,
      "net_yield_wei", result.net_yield_wei,
      "executed_at_ms", result.executed_at_ms,
      "execution_time_ms", result.execution_time_ms,
    ];

    if (result.rejection_reason) {
      fields.push("rejection_reason", result.rejection_reason);
    }

    await this.redis.xadd(STREAM_OUT, "MAXLEN", "~", 5000, "*", ...fields);
  }

  private async updateMetrics(status: "ACCEPTED" | "REJECTED", executionTimeMs: number): Promise<void> {
    if (!this.redis) return;

    await this.redis.hincrby("arbx:metrics:throughput:paper_executed", status.toLowerCase(), 1);

    const bucket = this.getLatencyBucket(executionTimeMs);
    await this.redis.hincrby("arbx:metrics:latency:paper_execution", bucket, 1);
  }

  private getLatencyBucket(ms: number): string {
    if (ms < 5) return "lt_5ms";
    if (ms < 10) return "lt_10ms";
    if (ms < 25) return "lt_25ms";
    if (ms < 50) return "lt_50ms";
    if (ms < 100) return "lt_100ms";
    return "gte_100ms";
  }


  private async persistRun(
    sim: SimulatedOpportunity,
    netYieldWei: bigint,
    executionTimeMs: number,
    calculationBasis: string
  ): Promise<string> {
    const netYieldUsd = weiToUsd(netYieldWei);

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
        netYieldUsd,
        null,
        null,
        new Date(sim.timestamp_ms),
        netYieldUsd,
        new Date(),
        executionTimeMs,
        `paper_executor:${calculationBasis}`,
      ]
    );
    if (!result.rows[0]) throw new Error("persist_failed");
    return result.rows[0].id;
  }
}

