import { describe, it, expect } from "vitest";
import { fmtAge, fmtDateTime, fmtMoney, fmtMs, fmtPct01, fmtPct100, fmtTime } from "./formatters";

const DASH = "—";

describe("fmtMoney", () => {
  it("returns dash for null/undefined", () => {
    expect(fmtMoney(null)).toBe(DASH);
    expect(fmtMoney(undefined)).toBe(DASH);
  });
  it("formats positive, negative, and zero with 2-decimal precision", () => {
    expect(fmtMoney(0)).toBe("$0.00");
    expect(fmtMoney(12.3)).toBe("$12.30");
    expect(fmtMoney(-5.5)).toBe("$-5.50");
    expect(fmtMoney(1234567.891)).toBe("$1234567.89");
  });
});

describe("fmtPct01", () => {
  it("treats input as a 0..1 rate and scales to percent", () => {
    expect(fmtPct01(0)).toBe("0.00%");
    expect(fmtPct01(0.1234)).toBe("12.34%");
    expect(fmtPct01(1)).toBe("100.00%");
  });
  it("returns dash for null/undefined", () => {
    expect(fmtPct01(null)).toBe(DASH);
    expect(fmtPct01(undefined)).toBe(DASH);
  });
});

describe("fmtPct100", () => {
  it("treats input as already-scaled percent value", () => {
    expect(fmtPct100(0)).toBe("0.00%");
    expect(fmtPct100(12.34)).toBe("12.34%");
    expect(fmtPct100(99.999)).toBe("100.00%");
  });
  it("returns dash for null/undefined", () => {
    expect(fmtPct100(null)).toBe(DASH);
    expect(fmtPct100(undefined)).toBe(DASH);
  });
});

describe("fmtMs", () => {
  it("rounds to integer milliseconds", () => {
    expect(fmtMs(0)).toBe("0 ms");
    expect(fmtMs(99.6)).toBe("100 ms");
    expect(fmtMs(1234.4)).toBe("1234 ms");
  });
  it("returns dash for null/undefined", () => {
    expect(fmtMs(null)).toBe(DASH);
    expect(fmtMs(undefined)).toBe(DASH);
  });
});

describe("fmtTime", () => {
  it("returns dash for empty/null/undefined/invalid", () => {
    expect(fmtTime(null)).toBe(DASH);
    expect(fmtTime(undefined)).toBe(DASH);
    expect(fmtTime("")).toBe(DASH);
    expect(fmtTime("not-a-date")).toBe(DASH);
  });
  it("returns a non-empty localized time for a valid ISO string", () => {
    const out = fmtTime("2026-04-22T12:00:00.000Z");
    expect(out).not.toBe(DASH);
    expect(out.length).toBeGreaterThan(0);
  });
});

describe("fmtDateTime", () => {
  it("returns dash for invalid input", () => {
    expect(fmtDateTime(null)).toBe(DASH);
    expect(fmtDateTime("garbage")).toBe(DASH);
  });
  it("returns a non-empty localized string for valid ISO", () => {
    const out = fmtDateTime("2026-04-22T12:00:00.000Z");
    expect(out).not.toBe(DASH);
    expect(out.length).toBeGreaterThan(0);
  });
});

describe("fmtAge", () => {
  const NOW = Date.parse("2026-04-22T12:00:00.000Z");

  it("returns dash for invalid/missing input", () => {
    expect(fmtAge(null, NOW)).toBe(DASH);
    expect(fmtAge("garbage", NOW)).toBe(DASH);
  });

  it("clamps to 0s for future timestamps", () => {
    const future = new Date(NOW + 60_000).toISOString();
    expect(fmtAge(future, NOW)).toBe("0s ago");
  });

  it("renders seconds when < 1 minute", () => {
    const t = new Date(NOW - 45_000).toISOString();
    expect(fmtAge(t, NOW)).toBe("45s ago");
  });

  it("renders minutes when < 1 hour", () => {
    const t = new Date(NOW - 5 * 60_000).toISOString();
    expect(fmtAge(t, NOW)).toBe("5m ago");
  });

  it("renders hours when < 1 day", () => {
    const t = new Date(NOW - 3 * 3600_000).toISOString();
    expect(fmtAge(t, NOW)).toBe("3h ago");
  });

  it("renders days for older timestamps", () => {
    const t = new Date(NOW - 2 * 86400_000).toISOString();
    expect(fmtAge(t, NOW)).toBe("2d ago");
  });
});
