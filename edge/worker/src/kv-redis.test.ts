import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { Redis } from "ioredis";
import { RedisKV } from "./kv-redis.js";

const REDIS_URL = process.env["REDIS_URL"] ?? "redis://localhost:6379";

describe("RedisKV", () => {
  let redis: Redis;
  let kv: RedisKV;

  beforeAll(() => {
    redis = new Redis(REDIS_URL, { maxRetriesPerRequest: 3 });
    kv = new RedisKV(redis, "test:kv:");
  });

  afterAll(async () => {
    const keys = await redis.keys("test:kv:*");
    if (keys.length) await redis.del(...keys);
    await redis.quit();
  });

  it("puts and gets a string value", async () => {
    await kv.put("greeting", "hello");
    expect(await kv.get("greeting")).toBe("hello");
  });

  it("returns null for a missing key", async () => {
    expect(await kv.get("does-not-exist")).toBeNull();
  });

  it("gets a JSON value with type:'json'", async () => {
    await kv.put("obj", JSON.stringify({ fails: 3, blockedUntil: 0 }));
    const v = await kv.get<{ fails: number; blockedUntil: number }>("obj", "json");
    expect(v).toEqual({ fails: 3, blockedUntil: 0 });
  });

  it("returns null on type:'json' when value is not parseable", async () => {
    await kv.put("bad", "{not json");
    expect(await kv.get("bad", "json")).toBeNull();
  });

  it("applies expirationTtl via SET EX", async () => {
    await kv.put("ephemeral", "gone", { expirationTtl: 1 });
    expect(await kv.get("ephemeral")).toBe("gone");
    await new Promise((r) => setTimeout(r, 1100));
    expect(await kv.get("ephemeral")).toBeNull();
  });

  it("deletes a key", async () => {
    await kv.put("todelete", "x");
    await kv.delete("todelete");
    expect(await kv.get("todelete")).toBeNull();
  });

  it("namespaces keys with the prefix", async () => {
    await kv.put("namespaced", "v");
    const raw = await redis.get("test:kv:namespaced");
    expect(raw).toBe("v");
  });
});
