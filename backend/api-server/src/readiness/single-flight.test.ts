import { describe, it, expect, vi } from "vitest";
import { createSingleFlight } from "./single-flight.js";

describe("in-flight readiness reads (no settled cache)", () => {
  it("coalesces overlapping reads for one dependency", async () => {
    const run = createSingleFlight<object, number>();
    const pool = {};
    const read = vi.fn(async () => 42);
    const a = run(pool, read), b = run(pool, read);
    expect(a).toBe(b);
    expect(await a).toBe(42);
    expect(read).toHaveBeenCalledTimes(1);
  });
  it("never returns a settled result to the next request", async () => {
    const run = createSingleFlight<object, number>();
    const pool = {};
    expect(await run(pool, async () => 1)).toBe(1);
    expect(await run(pool, async () => 2)).toBe(2);
  });
  it("isolates different pools", async () => {
    const run = createSingleFlight<object, string>();
    const [a, b] = await Promise.all([run({}, async () => "A"), run({}, async () => "B")]);
    expect([a, b]).toEqual(["A", "B"]);
  });
  it("propagates failure to all waiters and releases the entry", async () => {
    const run = createSingleFlight<string, number>();
    const failure = new Error("read failed");
    const a = run("pool", async () => { throw failure; });
    const b = run("pool", async () => 123);
    expect(a).toBe(b);
    const outcomes = await Promise.allSettled([a, b]);
    expect(outcomes).toEqual([
      { status: "rejected", reason: failure }, { status: "rejected", reason: failure },
    ]);
    expect(await run("pool", async () => 7)).toBe(7);
  });
  it("cleans up a synchronous throw as well", async () => {
    const run = createSingleFlight<string, number>();
    await expect(run("x", () => { throw new Error("sync failure"); })).rejects.toThrow("sync failure");
    expect(await run("x", async () => 8)).toBe(8);
  });
  it("keeps a pending read shared until it settles", async () => {
    const run = createSingleFlight<string, number>();
    let resolve!: (n: number) => void;
    const a = run("x", () => new Promise<number>((r) => { resolve = r; }));
    await Promise.resolve();
    const b = run("x", async () => 99);
    expect(a).toBe(b);
    resolve(3);
    expect(await b).toBe(3);
  });
});
