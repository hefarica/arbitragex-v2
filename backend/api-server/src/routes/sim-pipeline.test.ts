/**
 * sim-pipeline tests — A.8 surface, per-strategy (STRAT-IDENT-01).
 *
 * Pins the hard invariants of GET /api/v1/sim/pipeline:
 *   (a) 503 when the pool is null (R8 fail-honest, no fabricated zeros),
 *   (b) per-strategy rows pass through with strategy_key intact — the route
 *       NEVER collapses rows into a class/pair bucket,
 *   (c) legacy rows (strategy_key NULL pre-STRAT-IDENT-01) stay NULL — no
 *       back-fill invention,
 *   (d) a failing scored_opportunities query yields per_strategy null + 200
 *       (the circuit summary still serves), never a crash,
 *   (e) posture is advisory-only (observe_only_advisory, capital = 0 implied).
 */
import express, { type Express } from "express";
import request from "supertest";
import { describe, expect, it, vi } from "vitest";
import type pg from "pg";

import { mountSimPipeline } from "./sim-pipeline.js";

const logger = { warn: vi.fn() };

function buildApp(pool: pg.Pool | null): Express {
  const app = express();
  mountSimPipeline(app, { pool, logger });
  return app;
}

function fakePoolOk(): pg.Pool {
  return {
    query: vi.fn(async (text: string) => {
      if (text.includes("FROM scored_opportunities")) {
        return {
          rows: [
            {
              strategy_key: "mev_01_015_flashloan_atomic",
              scored: 120,
              accepted: 118,
              avg_posterior_prob: 0.51,
              avg_kelly_fraction: 0.25,
              avg_recommended_usd: 1250.0,
              evidence_rows: 96,
              last_scored_at: new Date("2026-08-23T10:00:00Z"),
              source_context: "flat_prior",
            },
            {
              strategy_key: "flashloan_arb",
              scored: 40,
              accepted: 40,
              avg_posterior_prob: 0.51,
              avg_kelly_fraction: 0.25,
              avg_recommended_usd: 1250.0,
              evidence_rows: 40,
              last_scored_at: new Date("2026-08-23T09:00:00Z"),
              source_context: "flat_prior",
            },
            {
              // Legacy row scored before STRAT-IDENT-01 — identity unknown.
              strategy_key: null,
              scored: 5,
              accepted: 5,
              avg_posterior_prob: 0.5,
              avg_kelly_fraction: 0.25,
              avg_recommended_usd: 1250.0,
              evidence_rows: 0,
              last_scored_at: new Date("2026-08-20T09:00:00Z"),
              source_context: "flat_prior",
            },
          ],
        };
      }
      if (text.includes("FROM bayesian_priors")) {
        return { rows: [{ n: 0 }] };
      }
      return { rows: [] };
    }),
  } as unknown as pg.Pool;
}

describe("GET /api/v1/sim/pipeline (STRAT-IDENT-01)", () => {
  it("returns 503 when the PG pool is null (fail-honest, no fabricated zeros)", async () => {
    const res = await request(buildApp(null)).get("/api/v1/sim/pipeline");
    expect(res.status).toBe(503);
    expect(res.body).toEqual({ error: "db_unavailable" });
  });

  it("serves per-strategy rows with identity intact — no class/pair collapse", async () => {
    const res = await request(buildApp(fakePoolOk())).get("/api/v1/sim/pipeline");
    expect(res.status).toBe(200);
    const per = res.body.per_strategy as Array<{ strategy_key: string | null; scored: number }>;
    expect(per).toHaveLength(3);
    const keys = per.map((r) => r.strategy_key);
    expect(keys).toContain("mev_01_015_flashloan_atomic");
    expect(keys).toContain("flashloan_arb");
    // Identified-strategy count EXCLUDES the legacy NULL bucket.
    expect(res.body.strategy_count).toBe(2);
    expect(res.body.calibrated_strategies).toBe(0);
  });

  it("keeps legacy NULL strategy_key NULL — never back-filled", async () => {
    const res = await request(buildApp(fakePoolOk())).get("/api/v1/sim/pipeline");
    const legacy = (res.body.per_strategy as Array<{ strategy_key: string | null }>).find(
      (r) => r.strategy_key === null,
    );
    expect(legacy).toBeDefined();
    expect(legacy!.strategy_key).toBeNull();
  });

  it("advisory posture + strategy calibration identity are declared", async () => {
    const res = await request(buildApp(fakePoolOk())).get("/api/v1/sim/pipeline");
    expect(res.body.scoring_circuit.posture).toBe("observe_only_advisory");
    expect(res.body.scoring_circuit.calibration_identity).toContain("strategy");
  });

  it("a failing scored_opportunities query degrades to per_strategy null, still 200", async () => {
    const pool = {
      query: vi.fn(async (text: string) => {
        if (text.includes("FROM scored_opportunities")) throw new Error("relation missing");
        if (text.includes("FROM bayesian_priors")) return { rows: [{ n: 0 }] };
        return { rows: [] };
      }),
    } as unknown as pg.Pool;
    const res = await request(buildApp(pool)).get("/api/v1/sim/pipeline");
    expect(res.status).toBe(200);
    expect(res.body.per_strategy).toBeNull();
    expect(res.body.strategy_count).toBeNull();
    expect(logger.warn).toHaveBeenCalled();
  });
});
