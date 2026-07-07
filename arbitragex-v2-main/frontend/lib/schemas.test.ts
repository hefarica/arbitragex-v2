import { describe, it, expect } from "vitest";
import {
  ExecutionRowSchema,
  KillSwitchStateSchema,
  OpportunitiesLiveSchema,
  ReconTimeseriesResponseSchema,
  RiskAlertRowSchema,
  StatusResponseSchema,
} from "./schemas";

describe("KillSwitchStateSchema", () => {
  it("accepts a well-formed payload", () => {
    const r = KillSwitchStateSchema.safeParse({
      enabled: true,
      reason: "manual armed by operator",
      triggered_by: "alice",
      updated_at: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(true);
  });

  it("accepts explicit nulls for reason and triggered_by (never optional)", () => {
    const r = KillSwitchStateSchema.safeParse({
      enabled: false,
      reason: null,
      triggered_by: null,
      updated_at: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(true);
  });

  it("rejects missing required fields", () => {
    const r = KillSwitchStateSchema.safeParse({ enabled: true });
    expect(r.success).toBe(false);
  });

  it("rejects wrong type for enabled", () => {
    const r = KillSwitchStateSchema.safeParse({
      enabled: "yes",
      reason: null,
      triggered_by: null,
      updated_at: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(false);
  });
});

describe("StatusResponseSchema", () => {
  it("parses a services map with ok/status/detail", () => {
    const r = StatusResponseSchema.safeParse({
      ok: true,
      services: {
        "api-server": { ok: true, status: 200 },
        "selector-api": { ok: false, detail: "connection refused" },
      },
      killswitch: null,
      env: "dev",
      version: "0.1.0",
      ts: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(true);
  });
});

describe("OpportunitiesLiveSchema", () => {
  it("accepts the canonical operator payload", () => {
    const r = OpportunitiesLiveSchema.safeParse({
      count: 1,
      window: "60s",
      items: [
        {
          id: "op-1", chain_id: 1, strategy_kind: "dex_arb",
          dex_a: "uniswap", dex_b: "curve", pair_symbol: "WETH/USDC",
          token_in: "0xa", token_out: "0xb", amount_in_wei: "1000000",
          expected_profit_usd: 12.34, roi_pct: 0.5, risk_score: null,
          block_number: 1000, status: "detected",
          detected_at: "2026-04-22T12:00:00.000Z",
          trace_id: "t-1",
        },
      ],
      ts: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(true);
  });
});

describe("ExecutionRowSchema", () => {
  it("accepts every valid status enum value", () => {
    const statuses = ["submitted", "included", "reverted", "dropped", "replaced", "not_implemented"] as const;
    for (const s of statuses) {
      const r = ExecutionRowSchema.safeParse({
        id: "e1", tx_hash: null, bundle_hash: null, relay_name: "flashbots",
        status: s, block_included: null, gas_used_wei: null, gas_price_effective_wei: null,
        expected_profit_usd: null, actual_profit_usd: null, error_message: null,
        trace_id: "t", submitted_at: "2026-04-22T12:00:00.000Z", confirmed_at: null,
        chain_id: 1, strategy_kind: "dex_arb", pair_symbol: null,
      });
      expect(r.success).toBe(true);
    }
  });

  it("rejects invalid status values", () => {
    const r = ExecutionRowSchema.safeParse({
      id: "e1", tx_hash: null, bundle_hash: null, relay_name: "flashbots",
      status: "bogus_status", block_included: null, gas_used_wei: null,
      gas_price_effective_wei: null, expected_profit_usd: null,
      actual_profit_usd: null, error_message: null, trace_id: "t",
      submitted_at: "2026-04-22T12:00:00.000Z", confirmed_at: null,
      chain_id: 1, strategy_kind: "dex_arb", pair_symbol: null,
    });
    expect(r.success).toBe(false);
  });
});

describe("RiskAlertRowSchema", () => {
  it("accepts any JSON value in payload (unknown)", () => {
    for (const payload of [{}, null, "string", 42, true, [1, 2, 3]]) {
      const r = RiskAlertRowSchema.safeParse({
        id: "a1",
        event_type: "manual",
        severity: "info",
        source_service: "api-server",
        payload,
        trace_id: null,
        opportunity_id: null,
        created_at: "2026-04-22T12:00:00.000Z",
      });
      expect(r.success).toBe(true);
    }
  });

  it("enforces severity enum", () => {
    const r = RiskAlertRowSchema.safeParse({
      id: "a1", event_type: "manual", severity: "hell",
      source_service: "api-server", payload: {}, trace_id: null,
      opportunity_id: null, created_at: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(false);
  });
});

describe("ReconTimeseriesResponseSchema", () => {
  it("parses a typical response with points", () => {
    const r = ReconTimeseriesResponseSchema.safeParse({
      window_hours: 24,
      bucket_minutes: 60,
      points: [
        {
          bucket_start: "2026-04-22T00:00:00.000Z",
          attempts: 10, included: 7, reverted: 3,
          avg_pnl_included_usd: 12.5, revert_rate: 0.3,
        },
        {
          bucket_start: "2026-04-22T01:00:00.000Z",
          attempts: 0, included: 0, reverted: 0,
          avg_pnl_included_usd: null, revert_rate: null,
        },
      ],
      ts: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(true);
  });

  it("accepts zero points (quiet window)", () => {
    const r = ReconTimeseriesResponseSchema.safeParse({
      window_hours: 1, bucket_minutes: 5, points: [],
      ts: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(true);
  });

  it("rejects a point missing required fields", () => {
    const r = ReconTimeseriesResponseSchema.safeParse({
      window_hours: 24, bucket_minutes: 60,
      points: [{ bucket_start: "x", attempts: 1 }],
      ts: "2026-04-22T12:00:00.000Z",
    });
    expect(r.success).toBe(false);
  });
});
