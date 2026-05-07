import { describe, it, expect } from "vitest";
import { formatProfitUSD, formatSpreadBps, shortAddr } from "../format";

describe("formatProfitUSD", () => {
  it("returns pending tone for null", () => {
    const r = formatProfitUSD(null);
    expect(r.display).toBe("—");
    expect(r.tone).toBe("pending");
  });

  it("returns pending tone for undefined", () => {
    const r = formatProfitUSD(undefined);
    expect(r.display).toBe("—");
    expect(r.tone).toBe("pending");
  });

  it("returns pending tone for NaN (R8: no data, not a value)", () => {
    const r = formatProfitUSD(NaN);
    expect(r.display).toBe("—");
    expect(r.tone).toBe("pending");
  });

  it("returns neutral tone for zero (real value, not pending)", () => {
    const r = formatProfitUSD(0);
    expect(r.display).toBe("$0.00");
    expect(r.tone).toBe("neutral");
  });

  it("returns positive tone for positive profit", () => {
    const r = formatProfitUSD(12.5);
    expect(r.display).toBe("+$12.50");
    expect(r.tone).toBe("positive");
  });

  it("returns negative tone for negative profit", () => {
    const r = formatProfitUSD(-3.75);
    expect(r.display).toBe("-$3.75");
    expect(r.tone).toBe("negative");
  });
});

describe("formatSpreadBps", () => {
  it("returns pending tone for null/undefined", () => {
    expect(formatSpreadBps(null).tone).toBe("pending");
    expect(formatSpreadBps(undefined).tone).toBe("pending");
    expect(formatSpreadBps(null).display).toBe("—");
  });

  it("returns positive tone for positive bps", () => {
    const r = formatSpreadBps(25.5);
    expect(r.display).toBe("25.5bps");
    expect(r.tone).toBe("positive");
  });

  it("returns neutral tone for zero or negative bps", () => {
    const z = formatSpreadBps(0);
    expect(z.display).toBe("0.0bps");
    expect(z.tone).toBe("neutral");

    const neg = formatSpreadBps(-5);
    expect(neg.tone).toBe("neutral");
  });
});

describe("shortAddr", () => {
  it("truncates long addresses to 0xAAAA…BBBB format", () => {
    const addr = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
    expect(shortAddr(addr)).toBe("0xc02a…6cc2");
  });

  it("returns the full string for short inputs (<= 10 chars)", () => {
    expect(shortAddr("0x1234")).toBe("0x1234");
    expect(shortAddr("0x")).toBe("0x");
  });
});
