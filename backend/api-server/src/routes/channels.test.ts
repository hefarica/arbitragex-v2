/**
 * CHANNELS-01 (2026-09-26) — /api/v1/channels contract tests.
 *
 * Mirrors the house fake-redis harness (websocket-hot-streamer.test.ts:
 * xinfo replies mirror the REAL ioredis raw-array shape). Asserts R8/R10:
 * missing streams degrade per-stream (never crash the envelope), nulls are
 * never coerced to fabricated numbers, and zombies are flagged by the pure
 * rule (pending == 0 && idle > threshold).
 */

import { describe, expect, it } from "vitest";
import express from "express";
import request from "supertest";
import {
  channelsStreamList,
  mountChannels,
  parseFlatPairs,
  tsFromId,
  type ChannelReport,
} from "./channels.js";

type FakeRedis = Record<string, unknown>;

function makeApp(fake: FakeRedis | null) {
  const app = express();
  mountChannels(app, {
    redis: (fake as never) ?? null,
    logger: { warn: () => {} },
  });
  return app;
}

// Realistic ioredis raw shapes (flat arrays / rows of flat arrays).
const DETECTED_XINFO_STREAM = [
  "length",
  10004,
  "radix-tree-keys",
  1,
  "radix-tree-nodes",
  2,
  "last-generated-id",
  "1758871320000-5",
  "max-deleted-entry-id",
  "0-0",
  "entries-added",
  500123,
  "recorded-first-entry-id",
  "1758871200000-1",
];

const DETECTED_XINFO_GROUPS = [
  ["name", "enricher", "consumers", 1, "pending", 0, "last-delivered-id", "1758871320000-5"],
  ["name", "selector-g0", "consumers", 3, "pending", 2, "last-delivered-id", "1758871319000-0"],
];

const DETECTED_XINFO_CONSUMERS = [
  ["name", "selector-live", "pending", 2, "idle", 1200, "inactive", 900],
  // zombie: nothing pending, idle 11 minutes
  ["name", "selector-orphan-abc", "pending", 0, "idle", 660000, "inactive", 660000],
];

const MISSING_XLEN_ERR = "ERR no such key";

describe("CHANNELS-01 — pure parsers", () => {
  it("parseFlatPairs reads ioredis raw arrays (WO-15)", () => {
    const p = parseFlatPairs(DETECTED_XINFO_STREAM);
    expect(p["length"]).toBe(10004);
    expect(p["last-generated-id"]).toBe("1758871320000-5");
    expect(p["no-such-field"]).toBeUndefined();
  });

  it("tsFromId extracts ms from entry ids and refuses junk", () => {
    expect(tsFromId("1758871320000-5")).toBe(1758871320000);
    expect(tsFromId("junk")).toBeNull();
    expect(tsFromId(42)).toBeNull();
    expect(tsFromId(undefined)).toBeNull();
  });

  it("channelsStreamList falls back to the declared topology, env wins when present", () => {
    const fallback = channelsStreamList(undefined);
    expect(fallback).toContain("arbx:opps:detected");
    expect(fallback.length).toBeGreaterThanOrEqual(8);
    expect(channelsStreamList(" a , b ,, ")[Symbol.iterator]().toArray()).toEqual(["a", "b"]);
    expect(channelsStreamList("   ")).toEqual(fallback);
  });
});

describe("CHANNELS-01 — endpoint behavior (fake redis)", () => {
  it("reports a live stream: heartbeat, groups, pending, zombie flag", async () => {
    const fake: FakeRedis = {
      xlen: async (key: string) => {
        if (key === "arbx:opps:detected") return 10004;
        throw new Error(MISSING_XLEN_ERR);
      },
      xinfo: async (kind: string, key: string, group?: string) => {
        if (kind === "STREAM" && key === "arbx:opps:detected") return DETECTED_XINFO_STREAM;
        if (kind === "GROUPS" && key === "arbx:opps:detected") return DETECTED_XINFO_GROUPS;
        if (kind === "CONSUMERS" && key === "arbx:opps:detected" && group === "enricher")
          return [];
        if (kind === "CONSUMERS" && key === "arbx:opps:detected" && group === "selector-g0")
          return DETECTED_XINFO_CONSUMERS;
        throw new Error(MISSING_XLEN_ERR);
      },
      xpending: async () => [2, "1758871200000-0", "1758871310000-0", []],
    };
    const res = await request(makeApp(fake)).get("/api/v1/channels");
    expect(res.status).toBe(200);
    expect(res.body.source).toEqual({ redis: "ok" });
    const detected: ChannelReport = res.body.channels.find(
      (c: ChannelReport) => c.stream === "arbx:opps:detected",
    );
    expect(detected.exists).toBe(true);
    expect(detected.xlen).toBe(10004);
    expect(detected.producer_last_entry_ms).toBe(1758871320000);
    const selector = detected.groups?.find((g) => g.name === "selector-g0");
    expect(selector?.pending).toBe(2);
    expect(selector?.oldest_pending_ms).toBe(1758871200000);
    const orphan = selector?.consumer_reports.find((c) => c.name === "selector-orphan-abc");
    expect(orphan?.zombie).toBe(true);
    const live = selector?.consumer_reports.find((c) => c.name === "selector-live");
    expect(live?.zombie).toBe(false);
  });

  it("a missing stream degrades per-stream (exists:false, nulls) — envelope stays 200", async () => {
    const fake: FakeRedis = {
      xlen: async () => {
        throw new Error(MISSING_XLEN_ERR);
      },
      xinfo: async () => {
        throw new Error(MISSING_XLEN_ERR);
      },
      xpending: async () => {
        throw new Error(MISSING_XLEN_ERR);
      },
    };
    const res = await request(makeApp(fake))
      .get("/api/v1/channels")
      .set("x-test-streams", "one");
    expect(res.status).toBe(200);
    const only: ChannelReport = res.body.channels[0];
    expect(only.exists).toBe(false);
    expect(only.xlen).toBeNull();
    expect(only.producer_last_entry_ms).toBeNull();
    expect(only.groups).toBeNull();
    expect(typeof only.error).toBe("string");
  });

  it("redis null → 503 redis_unavailable (no fabrication)", async () => {
    const res = await request(makeApp(null)).get("/api/v1/channels");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });

  it("ARBX_CHANNELS_STREAMS narrows the reported list (env override)", async () => {
    const prev = process.env["ARBX_CHANNELS_STREAMS"];
    process.env["ARBX_CHANNELS_STREAMS"] = "arbx:opps:detected";
    try {
      const fake: FakeRedis = {
        xlen: async (key: string) => {
          if (key !== "arbx:opps:detected") throw new Error(MISSING_XLEN_ERR);
          return 7;
        },
        xinfo: async () => [],
        xpending: async () => [0, "0-0", "0-0", []],
      };
      const res = await request(makeApp(fake)).get("/api/v1/channels");
      expect(res.status).toBe(200);
      expect(res.body.channels.length).toBe(1);
      expect(res.body.channels[0].stream).toBe("arbx:opps:detected");
    } finally {
      if (prev === undefined) delete process.env["ARBX_CHANNELS_STREAMS"];
      else process.env["ARBX_CHANNELS_STREAMS"] = prev;
    }
  });
});
