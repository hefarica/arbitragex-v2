/**
 * A3 PAPER_EXECUTED — unit del escritor del estado paper.
 *
 * Qué fija este test (y por qué importa):
 *   1. Con sim `passed` y neto > 0, el ejecutor persiste la evidencia
 *      (`paper_trade_runs`) y DESPUÉS mueve `opportunities.status` a
 *      `'paper_executed'` — nunca a `'executed'`.
 *   2. Con neto <= 0 NO escribe ni la evidencia ni el estado.
 *   3. Con sim `failed` no toca la DB en absoluto.
 *   4. El UPDATE del estado está GUARDADO (`status IN (...)`) para no pisar
 *      estados terminales (un `executed` on-chain jamás se degrada a paper).
 *   5. El ejecutor NO tiene camino de broadcast: su fuente no importa ni invoca
 *      firma/envío (`ethers`, `viem`, `web3`, `sendTransaction`,
 *      `eth_sendRawTransaction`) — el camino paper no difunde nada.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";

import { PaperExecutor, type PaperExecutorDeps } from "./executor.js";

interface QueryCall {
  sql: string;
  params: unknown[] | undefined;
}

function fakePool(rowsForInsert = "paper-run-0001", failUpdate = false) {
  const calls: QueryCall[] = [];
  const pool = {
    query: vi.fn(async (sql: string, params?: unknown[]) => {
      calls.push({ sql, params });
      if (/INSERT\s+INTO\s+paper_trade_runs/i.test(sql)) {
        return { rows: [{ id: rowsForInsert }], rowCount: 1 };
      }
      if (/UPDATE\s+opportunities/i.test(sql)) {
        if (failUpdate) throw new Error("pg down");
        return { rows: [], rowCount: 1 };
      }
      return { rows: [], rowCount: 0 };
    }),
  };
  return { pool, calls };
}

function fakeRedis() {
  return {
    xack: vi.fn(async () => 1),
    xadd: vi.fn(async () => "1-1"),
    hincrby: vi.fn(async () => 1),
  };
}

function fakeLogger() {
  return { info: vi.fn(), warn: vi.fn(), error: vi.fn() };
}

/** Construye el ejecutor sin `start()` (no abre Redis real). */
function makeExecutor(pool: unknown, logger = fakeLogger()) {
  const deps = {
    redisUrl: "redis://unused",
    pool,
    logger,
  } as unknown as PaperExecutorDeps;
  const executor = new PaperExecutor(deps) as unknown as {
    redis: unknown;
    processOne: (id: string, kv: string[]) => Promise<void>;
  };
  const redis = fakeRedis();
  executor.redis = redis;
  return { executor, redis, logger };
}

const OPPS_ID = "11111111-1111-4111-8111-111111111111";

function simMessage(over: {
  status?: string;
  net?: string;
  gross?: string;
  gas?: string;
  oppId?: string;
}) {
  const kv = [
    "id",
    "sim-0001",
    "status",
    over.status ?? "passed",
    "opportunity_id",
    over.oppId ?? OPPS_ID,
    "chain_id",
    "1",
    "strategy_kind",
    "dex_arb",
    "timestamp_ms",
    "1700000000000",
  ];
  if (over.net !== undefined) kv.push("net_topological_yield_wei", over.net);
  if (over.gross !== undefined) kv.push("gross_topological_yield_wei", over.gross);
  if (over.gas !== undefined) kv.push("gas_cost_wei", over.gas);
  return kv;
}

describe("A3 paper_executed — escritor del estado paper", () => {
  it("con sim passed y neto > 0: persiste evidencia y mueve status a paper_executed (nunca a executed)", async () => {
    const { pool, calls } = fakePool();
    const { executor, logger } = makeExecutor(pool);

    await executor.processOne("1-1", simMessage({ net: "1000000000000000" }));

    const insert = calls.find((c) => /INSERT\s+INTO\s+paper_trade_runs/i.test(c.sql));
    expect(insert, "la evidencia paper_trade_runs debe persistirse").toBeTruthy();

    const update = calls.find((c) => /UPDATE\s+opportunities/i.test(c.sql));
    expect(update, "el estado paper_executed debe escribirse").toBeTruthy();
    expect(update!.sql).toContain("status = 'paper_executed'");
    expect(update!.sql).toContain("status IN (");
    expect(update!.params?.[0]).toBe(OPPS_ID);

    // Ninguna escritura usa `executed` como valor de status.
    for (const c of calls) {
      expect(c.sql).not.toMatch(/status\s*=\s*'executed'/);
    }

    // La evidencia se publica en el log junto con el paper_run_id.
    const logged = logger.info.mock.calls.find(
      (c) => (c[0] as { event?: string })?.event === "paper_executor.paper_executed"
    );
    expect(logged, "el evento paper_executor.paper_executed debe registrarse").toBeTruthy();
    expect((logged![0] as { paper_run_id?: string }).paper_run_id).toBe("paper-run-0001");
  });

  it("con neto <= 0: no persiste evidencia ni escribe el estado", async () => {
    const { pool, calls } = fakePool();
    const { executor } = makeExecutor(pool);

    await executor.processOne("1-2", simMessage({ net: "0" }));

    expect(calls.some((c) => /INSERT\s+INTO\s+paper_trade_runs/i.test(c.sql))).toBe(false);
    expect(calls.some((c) => /UPDATE\s+opportunities/i.test(c.sql))).toBe(false);
  });

  it("con sim failed: no toca la DB", async () => {
    const { pool, calls } = fakePool();
    const { executor } = makeExecutor(pool);

    await executor.processOne("1-3", simMessage({ status: "failed", net: "1000000000000000" }));

    expect(calls).toHaveLength(0);
  });

  it("si el UPDATE del estado falla, la evidencia queda y el loop NO se rompe", async () => {
    const { pool, calls } = fakePool("paper-run-0002", /* failUpdate */ true);
    const { executor, logger } = makeExecutor(pool);

    await expect(
      executor.processOne("1-4", simMessage({ net: "1000000000000000" }))
    ).resolves.toBeUndefined();

    // La evidencia se persistió antes del fallo: no se pierde.
    expect(calls.some((c) => /INSERT\s+INTO\s+paper_trade_runs/i.test(c.sql))).toBe(true);
    const errLog = logger.error.mock.calls.find(
      (c) =>
        (c[0] as { event?: string })?.event === "paper_executor.paper_executed_status_err"
    );
    expect(errLog, "el fallo del UPDATE debe declararse").toBeTruthy();
  });
});

describe("A3 paper_executed — el camino paper no difunde", () => {
  const here = dirname(fileURLToPath(import.meta.url));
  const source = readFileSync(join(here, "executor.ts"), "utf8");

  it("no importa ni invoca firma/envío de transacciones", () => {
    for (const forbidden of [
      "ethers",
      "viem",
      "web3",
      "sendTransaction",
      "eth_sendRawTransaction",
      "eth_sendBundle",
      "privateKey",
      "private_key",
    ]) {
      expect(source.includes(forbidden), `executor.ts no debe contener '${forbidden}'`).toBe(false);
    }
  });

  it("el único estado que escribe es paper_executed", () => {
    expect(source).toContain("status = 'paper_executed'");
    expect(source).not.toMatch(/status\s*=\s*'executed'/);
    expect(source).not.toMatch(/status\s*=\s*'reconciled'/);
  });
});
