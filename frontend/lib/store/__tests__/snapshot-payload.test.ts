// frontend/lib/store/__tests__/snapshot-payload.test.ts
//
// FE-SNAPSHOT-01 — the /api/opportunities/live snapshot parser as a pure seam.
// Both ingestion paths (degraded polling and the always-on LIVE reconcile)
// share it, so the parsing contract is pinned once here:
//   - { items: [...] } envelope (canonical api-server shape)
//   - bare array (defensive legacy shape)
//   - anything else → honest empty list (R8: never fabricate, never crash)
import { describe, it, expect } from "vitest";
import { parseSnapshotItems } from "../snapshot-payload";

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
