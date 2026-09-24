// frontend/lib/store/__tests__/snapshot-payload.test.ts
//
// FE-SNAPSHOT-01 — the /api/opportunities/live snapshot parser as a pure seam.
// Both ingestion paths (degraded polling and the always-on LIVE reconcile)
// share it, so the parsing contract is pinned once here:
//   - { items: [...] } envelope (canonical api-server shape)
//   - bare array (defensive legacy shape)
//   - anything else → honest empty list (R8: never fabricate, never crash)
//
// AUDIT-CARDS-MINOR (§2) — parseWindowTotal (WO-H4 envelope counter) shares
// the same seam and the same fail-honest contract.
import { describe, it, expect } from "vitest";
import { parseSnapshotItems, parseWindowTotal } from "../snapshot-payload";

describe("parseSnapshotItems (FE-SNAPSHOT-01)", () => {
  it("maps the canonical { items: [...] } envelope", () => {
    const out = parseSnapshotItems({
      items: [{ id: "opp-1", detected_at: "2026-09-20T12:00:00Z" }],
    });
    expect(out).toHaveLength(1);
    expect(out[0]!.id).toBe("opp-1");
  });

  it("maps a bare array payload", () => {
    const out = parseSnapshotItems([{ id: "opp-2", detected_at: null }]);
    expect(out).toHaveLength(1);
    expect(out[0]!.id).toBe("opp-2");
  });

  it("returns an honest empty list for a non-array payload (R8)", () => {
    expect(parseSnapshotItems({ error: "boom" })).toEqual([]);
    expect(parseSnapshotItems(null)).toEqual([]);
    expect(parseSnapshotItems("nope")).toEqual([]);
    expect(parseSnapshotItems({ items: "not-an-array" })).toEqual([]);
  });

  it("parses identically for both caller shapes — empty items is empty", () => {
    expect(parseSnapshotItems({ items: [] })).toEqual([]);
    expect(parseSnapshotItems([])).toEqual([]);
  });
});

describe("parseWindowTotal (AUDIT-CARDS-MINOR §2 · WO-H4)", () => {
  it("reads window_total from the canonical envelope", () => {
    expect(
      parseWindowTotal({ count: 50, window: "latest", window_total: 12155, items: [] }),
    ).toBe(12155);
  });

  it("a REAL 0 passes through — computed empty window (R8: 0 ≠ absent)", () => {
    expect(parseWindowTotal({ count: 0, window: "latest", window_total: 0, items: [] })).toBe(0);
  });

  it("absent / null window_total → null, never an invented 0 (R8)", () => {
    // pre-WO-H4 edge / cached envelope without the field
    expect(parseWindowTotal({ count: 3, window: "latest", items: [] })).toBeNull();
    expect(parseWindowTotal({ window_total: null, items: [] })).toBeNull();
  });

  it("bare-array payloads have no envelope → null (legacy defensive shape)", () => {
    expect(parseWindowTotal([{ id: "opp-1" }])).toBeNull();
    expect(parseWindowTotal([])).toBeNull();
  });

  it("non-finite / non-number values → null (fail-honest, never NaN)", () => {
    expect(parseWindowTotal({ window_total: Number.NaN })).toBeNull();
    expect(parseWindowTotal({ window_total: "137" })).toBeNull();
    expect(parseWindowTotal({ window_total: true })).toBeNull();
  });

  it("null/undefined payload → null (same honesty as the items parser)", () => {
    expect(parseWindowTotal(null)).toBeNull();
    expect(parseWindowTotal(undefined)).toBeNull();
  });
});
