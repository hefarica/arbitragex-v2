/**
 * PR #1004 / contract §6 — the configured base price must not read like a quote.
 *
 * On 42161 and 8453 the served `base_token_price_usd` is a round 3000.0000
 * constant while chain 1 moves with the tower. The public read now carries
 * `configured_base_price_usd` and an explicit source.
 */
import { describe, expect, it, vi } from "vitest";
import express from "express";
import request from "supertest";
import { buildTradingConfigRouter } from "./trading-config.js";

const ROW = {
  chain_id: 42161,
  capital_usd: "1000",
  base_token_symbol: "WETH",
  base_token_price_usd: "3000.0000",
  allowed_token_symbols: ["WETH", "USDC"],
  token_prices_usd: { WETH: 3000 },
  // rowToRedisState stamps these; the read path requires real Dates.
  updated_at: new Date("2026-10-20T00:00:00Z"),
  created_at: new Date("2026-10-01T00:00:00Z"),
  updated_by: "operator",
};

function app() {
  const a = express();
  a.use(
    buildTradingConfigRouter({
      pool: {
        query: vi.fn(async (sql: string) =>
          sql.includes("FROM trading_config")
            ? { rows: [ROW], rowCount: 1 }
            : { rows: [], rowCount: 0 }),
      } as never,
      redis: { get: async () => null, set: async () => "OK", publish: async () => 0 } as never,
      logger: { warn: vi.fn(), info: vi.fn() },
      requireAdminToken: () => (_q: unknown, _s: unknown, next: () => void) => next(),
      publishRuntimeAck: async () => {},
    } as never),
  );
  return a;
}

describe("GET /api/v1/trading-config — configured vs measured price", () => {
  it("labels the base price as operator-configured, never as a quote", async () => {
    const res = await request(app()).get("/api/v1/trading-config?chain_id=42161");
    expect(res.status).toBe(200);
    expect(res.body.configured_base_price_usd).toBe(3000);
    expect(res.body.base_token_price_source).toBe("operator_configured");
  });
});
