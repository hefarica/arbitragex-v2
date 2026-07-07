import { describe, it, expect } from "vitest";
import {
  ALLOWED_BUCKET_MINUTES,
  DEFAULT_BUCKET_MINUTES,
  DEFAULT_HOURS,
  MAX_HOURS,
  MIN_HOURS,
  clampBucketMinutes,
  clampHours,
  rowToPoint,
} from "./recon-timeseries.js";

describe("clampHours", () => {
  it("returns default when input is nullish or non-numeric", () => {
    expect(clampHours(undefined)).toBe(DEFAULT_HOURS);
    expect(clampHours(null)).toBe(DEFAULT_HOURS);
    expect(clampHours("abc")).toBe(DEFAULT_HOURS);
    expect(clampHours(NaN)).toBe(DEFAULT_HOURS);
  });

  it("clamps to [1, 168] inclusive", () => {
    expect(clampHours(0)).toBe(MIN_HOURS);
    expect(clampHours(-5)).toBe(MIN_HOURS);
    expect(clampHours(1000)).toBe(MAX_HOURS);
    expect(clampHours(168)).toBe(168);
    expect(clampHours(1)).toBe(1);
  });

  it("accepts string-number inputs from query params", () => {
    expect(clampHours("24")).toBe(24);
    expect(clampHours("6")).toBe(6);
  });

  it("floors fractional inputs", () => {
    expect(clampHours(2.9)).toBe(2);
    expect(clampHours("12.5")).toBe(12);
  });
});

describe("clampBucketMinutes", () => {
  it("only accepts whitelisted values, falls back to default otherwise", () => {
    for (const m of ALLOWED_BUCKET_MINUTES) {
      expect(clampBucketMinutes(m)).toBe(m);
    }
    expect(clampBucketMinutes(1)).toBe(DEFAULT_BUCKET_MINUTES);
    expect(clampBucketMinutes(7)).toBe(DEFAULT_BUCKET_MINUTES);
    expect(clampBucketMinutes(9999)).toBe(DEFAULT_BUCKET_MINUTES);
    expect(clampBucketMinutes(undefined)).toBe(DEFAULT_BUCKET_MINUTES);
    expect(clampBucketMinutes("nope")).toBe(DEFAULT_BUCKET_MINUTES);
  });

  it("accepts string-number inputs for whitelisted values", () => {
    expect(clampBucketMinutes("15")).toBe(15);
    expect(clampBucketMinutes("60")).toBe(60);
  });
});

describe("rowToPoint", () => {
  it("converts Date bucket_start to ISO string", () => {
    const d = new Date("2026-04-22T12:00:00.000Z");
    const p = rowToPoint({
      bucket_start: d,
      attempts: 10, included: 7, reverted: 3,
      avg_pnl_included_usd: 12.34,
    });
    expect(p.bucket_start).toBe("2026-04-22T12:00:00.000Z");
  });

  it("derives revert_rate = reverted / attempts when attempts > 0", () => {
    const p = rowToPoint({
      bucket_start: new Date(), attempts: 10, included: 7, reverted: 3,
      avg_pnl_included_usd: 1,
    });
    expect(p.revert_rate).toBeCloseTo(0.3);
  });

  it("returns revert_rate=null when attempts=0 (empty bucket)", () => {
    const p = rowToPoint({
      bucket_start: new Date(), attempts: 0, included: 0, reverted: 0,
      avg_pnl_included_usd: null,
    });
    expect(p.revert_rate).toBeNull();
  });

  it("preserves null avg_pnl_included_usd (no synthesized zero)", () => {
    const p = rowToPoint({
      bucket_start: new Date(), attempts: 4, included: 0, reverted: 2,
      avg_pnl_included_usd: null,
    });
    expect(p.avg_pnl_included_usd).toBeNull();
  });

  it("passes through string bucket_start unchanged (when pg returns ISO string)", () => {
    const p = rowToPoint({
      bucket_start: "2026-04-22T12:00:00+00:00",
      attempts: 1, included: 1, reverted: 0,
      avg_pnl_included_usd: 5,
    });
    expect(p.bucket_start).toBe("2026-04-22T12:00:00+00:00");
  });
});
