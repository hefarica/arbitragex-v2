// frontend/lib/format-vigency.test.ts
//
// CARDS-DEDUP-HOPS: formatVigency produces the card's dual vigency ages —
// first detection (1ª) and last ratification (✓) — in discrete anti-
// saturation units, null-safe per R8, pure (now injected).
import { describe, it, expect } from "vitest";
import { formatVigency } from "@/lib/format";

const NOW = Date.parse("2026-09-20T12:00:00Z");
const T0 = "2026-09-20T09:00:00Z"; // 3h before NOW
const T_LAST = "2026-09-20T11:59:59Z"; // 1s before NOW

describe("formatVigency — CARDS-DEDUP-HOPS dual time", () => {
  it("grouped row: firstAge and lastAge from the aggregates, confirmed=true", () => {
    const v = formatVigency(T0, T_LAST, T0, NOW);
    expect(v.firstAge).toBe("3h");
    expect(v.lastAge).toBe("1s");
    expect(v.confirmed).toBe(true);
  });

  it("plain WS row (no aggregates): both ages fall back to detected_at, confirmed=false", () => {
    const v = formatVigency(null, null, T_LAST, NOW);
    expect(v.firstAge).toBe("1s");
    expect(v.lastAge).toBe("1s");
    expect(v.confirmed).toBe(false);
  });

  it("lastSeen present but firstSeen null: mixed fallback, confirmed=false", () => {
    const v = formatVigency(null, T_LAST, T0, NOW);
    expect(v.firstAge).toBe("3h"); // falls back to detected_at
    expect(v.lastAge).toBe("1s"); // from lastSeenAt
    expect(v.confirmed).toBe(false);
  });

  it("discrete units: s < 60, m < 60, h < 24, else d", () => {
    const at = (msAgo: number) => new Date(NOW - msAgo).toISOString();
    expect(formatVigency(at(59_000), at(0), null, NOW).firstAge).toBe("59s");
    expect(formatVigency(at(60_000), at(0), null, NOW).firstAge).toBe("1m");
    expect(formatVigency(at(59 * 60_000), at(0), null, NOW).firstAge).toBe("59m");
    expect(formatVigency(at(60 * 60_000), at(0), null, NOW).firstAge).toBe("1h");
    expect(formatVigency(at(23 * 3_600_000), at(0), null, NOW).firstAge).toBe("23h");
    expect(formatVigency(at(24 * 3_600_000), at(0), null, NOW).firstAge).toBe("1d");
    expect(formatVigency(at(5 * 86_400_000), at(0), null, NOW).firstAge).toBe("5d");
  });

  it("R8: undated row → null ages (sin fecha), never a fabricated 0s", () => {
    const v = formatVigency(null, null, null, NOW);
    expect(v.firstAge).toBeNull();
    expect(v.lastAge).toBeNull();
    expect(v.confirmed).toBe(false);
  });

  it("unparseable timestamps → null ages (never NaN strings)", () => {
    const v = formatVigency("not-a-date", "not-a-date", "not-a-date", NOW);
    expect(v.firstAge).toBeNull();
    expect(v.lastAge).toBeNull();
  });

  it("future timestamps clamp to 0s (clock skew tolerance)", () => {
    const future = new Date(NOW + 30_000).toISOString();
    const v = formatVigency(future, future, future, NOW);
    expect(v.firstAge).toBe("0s");
    expect(v.lastAge).toBe("0s");
  });
});
