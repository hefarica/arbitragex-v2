import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  CircuitBreaker,
  CircuitBreakerOpenError,
  CircuitBreakerRegistry,
} from "./index.js";

describe("CircuitBreaker", () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it("starts closed", () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 3, window_ms: 1000, cooldown_ms: 1000 });
    expect(cb.state()).toBe("closed");
  });

  it("closed -> open after N failures in window", async () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 3, window_ms: 1000, cooldown_ms: 1000 });
    const boom = () => Promise.reject(new Error("boom"));
    await expect(cb.execute(boom)).rejects.toThrow("boom");
    await expect(cb.execute(boom)).rejects.toThrow("boom");
    expect(cb.state()).toBe("closed");
    await expect(cb.execute(boom)).rejects.toThrow("boom");
    expect(cb.state()).toBe("open");
  });

  it("open -> half_open after cooldown", async () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 1, window_ms: 1000, cooldown_ms: 5000 });
    const boom = () => Promise.reject(new Error("x"));
    await expect(cb.execute(boom)).rejects.toThrow();
    expect(cb.state()).toBe("open");
    vi.advanceTimersByTime(6000);
    expect(cb.state()).toBe("half_open");
  });

  it("half_open -> closed after N successes", async () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 1, window_ms: 1000, cooldown_ms: 500, half_open_probe_count: 2 });
    await expect(cb.execute(() => Promise.reject(new Error("x")))).rejects.toThrow();
    vi.advanceTimersByTime(600);
    expect(cb.state()).toBe("half_open");
    await cb.execute(() => Promise.resolve("ok"));
    expect(cb.state()).toBe("half_open");
    await cb.execute(() => Promise.resolve("ok"));
    expect(cb.state()).toBe("closed");
  });

  it("half_open -> open if probe fails", async () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 1, window_ms: 1000, cooldown_ms: 500, half_open_probe_count: 3 });
    await expect(cb.execute(() => Promise.reject(new Error("x")))).rejects.toThrow();
    vi.advanceTimersByTime(600);
    expect(cb.state()).toBe("half_open");
    await expect(cb.execute(() => Promise.reject(new Error("bad_probe")))).rejects.toThrow();
    expect(cb.state()).toBe("open");
  });

  it("throws CircuitBreakerOpenError when open without running fn", async () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 1, window_ms: 1000, cooldown_ms: 10000 });
    await expect(cb.execute(() => Promise.reject(new Error("x")))).rejects.toThrow();
    const fn = vi.fn(() => Promise.resolve("unreachable"));
    await expect(cb.execute(fn)).rejects.toBeInstanceOf(CircuitBreakerOpenError);
    expect(fn).not.toHaveBeenCalled();
  });

  it("manual trip + reset", async () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 10, window_ms: 1000, cooldown_ms: 1000 });
    cb.trip("manual");
    expect(cb.state()).toBe("open");
    cb.reset();
    expect(cb.state()).toBe("closed");
  });

  it("emits events on trip and reset", async () => {
    const cb = new CircuitBreaker({ name: "t", threshold: 1, window_ms: 1000, cooldown_ms: 1000 });
    const tripEvents: unknown[] = [];
    const resetEvents: unknown[] = [];
    cb.on("trip", e => tripEvents.push(e));
    cb.on("reset", e => resetEvents.push(e));
    await expect(cb.execute(() => Promise.reject(new Error("x")))).rejects.toThrow();
    expect(tripEvents.length).toBe(1);
    cb.reset();
    expect(resetEvents.length).toBe(1);
  });

  it("snapshot exposes useful state", () => {
    const cb = new CircuitBreaker({ name: "snap", threshold: 2, window_ms: 1000, cooldown_ms: 500 });
    const s = cb.snapshot();
    expect(s.name).toBe("snap");
    expect(s.state).toBe("closed");
    expect(s.failures_in_window).toBe(0);
  });
});

describe("CircuitBreakerRegistry", () => {
  it("registers and retrieves", () => {
    const reg = new CircuitBreakerRegistry();
    const cb = new CircuitBreaker({ name: "r1", threshold: 1, window_ms: 1, cooldown_ms: 1 });
    reg.register(cb);
    expect(reg.get("r1")).toBe(cb);
    expect(reg.snapshots()).toHaveLength(1);
  });
  it("deduplicates by name", () => {
    const reg = new CircuitBreakerRegistry();
    const cb1 = new CircuitBreaker({ name: "r", threshold: 1, window_ms: 1, cooldown_ms: 1 });
    const cb2 = new CircuitBreaker({ name: "r", threshold: 99, window_ms: 1, cooldown_ms: 1 });
    reg.register(cb1);
    expect(reg.register(cb2)).toBe(cb1);
  });
});
