/**
 * canonical-knobs tests — XLS-CANON-01 surface.
 *
 * Pins the hard invariants of GET /api/v1/config/canonical-knobs:
 *   (a) 503 redis_unavailable when the Redis client is null (R8 fail-honest),
 *   (b) serves the EXACT searcher-published snapshot verbatim (RULE 00 — this
 *       route computes/defaults nothing),
 *   (c) 503 knobs_not_published when the key is absent — never a fabricated
 *       workbook default,
 *   (d) 503 knobs_snapshot_corrupted on unparseable payloads,
 *   (e) a transient Redis error degrades to 503, never a crash.
 */
import express, { type Express } from "express";
import request from "supertest";
import { describe, expect, it, vi } from "vitest";
import type Redis from "ioredis";

import { mountCanonicalKnobs } from "./canonical-knobs.js";

const logger = { warn: vi.fn() };

function buildApp(redis: unknown): Express {
  const app = express();
  mountCanonicalKnobs(app, { redis: redis as Redis | null, logger });
  return app;
}

const SNAPSHOT = JSON.stringify({
  max_hops: 7,
  min_hops: 2,
  selected_financing: "OWN_CAPITAL",
  execution_mode: "PAPER_SHADOW",
  killswitch: false,
  source: "canonical_knobs.rs (01_CONFIG ULTRA workbook)",
});

describe("GET /api/v1/config/canonical-knobs (XLS-CANON-01)", () => {
  it("returns 503 when Redis is null (fail-honest)", async () => {
    const res = await request(buildApp(null)).get("/api/v1/config/canonical-knobs");
    expect(res.status).toBe(503);
    expect(res.body).toEqual({ error: "redis_unavailable" });
  });

  it("serves the searcher-published snapshot verbatim — no computing, no defaults", async () => {
    const redis = { get: vi.fn(async () => SNAPSHOT) };
    const res = await request(buildApp(redis)).get("/api/v1/config/canonical-knobs");
    expect(res.status).toBe(200);
    expect(res.body.knobs).toEqual(JSON.parse(SNAPSHOT));
    expect(res.body.knobs.max_hops).toBe(7);
    expect(res.body.knobs.execution_mode).toBe("PAPER_SHADOW");
    expect(res.body.source).toContain("searcher-rs");
  });

  it("503 knobs_not_published when the key is absent — never fabricated", async () => {
    const redis = { get: vi.fn(async () => null) };
    const res = await request(buildApp(redis)).get("/api/v1/config/canonical-knobs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("knobs_not_published");
  });

  it("503 knobs_snapshot_corrupted on unparseable payload", async () => {
    const redis = { get: vi.fn(async () => "not-json{{") };
    const res = await request(buildApp(redis)).get("/api/v1/config/canonical-knobs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("knobs_snapshot_corrupted");
    expect(logger.warn).toHaveBeenCalled();
  });

  it("a transient Redis error degrades to 503 redis_unavailable, no crash", async () => {
    const redis = { get: vi.fn(async () => { throw new Error("ECONNRESET"); }) };
    const res = await request(buildApp(redis)).get("/api/v1/config/canonical-knobs");
    expect(res.status).toBe(503);
    expect(res.body).toEqual({ error: "redis_unavailable" });
  });
});
