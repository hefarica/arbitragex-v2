// frontend/components/cex/__tests__/flash.test.ts
//
// WO-PRICE-EXCHANGE-V1 (FE) — CEX flash primitive.
//
// Contract under test (operator order 2026-09-18):
//   (a) flash ONLY on change — an identical value NEVER animates.
// The state machine is exported pure (nextFlashState) precisely so this is
// testable without a DOM: seq only advances when the value actually moved,
// and an unchanged step returns the SAME object (no key change → no remount
// → no CSS animation replay).
import { describe, expect, it } from "vitest";

import {
  flashClass,
  flashDirection,
  INITIAL_FLASH,
  nextFlashState,
} from "../flash";

describe("flashDirection — pure transition direction", () => {
  it("identical values → null (no animation)", () => {
    expect(flashDirection(5, 5)).toBeNull();
    expect(flashDirection(-1.25, -1.25)).toBeNull();
    expect(flashDirection(0, 0)).toBeNull();
  });

  it("increase → up, decrease → down", () => {
    expect(flashDirection(5, 6)).toBe("up");
    expect(flashDirection(-2, -1)).toBe("up");
    expect(flashDirection(6, 5)).toBe("down");
    expect(flashDirection(0.1, 0.05)).toBe("down");
  });

  it("first observation / dash transitions → null (nothing comparable)", () => {
    expect(flashDirection(null, 5)).toBeNull();
    expect(flashDirection(5, null)).toBeNull();
    expect(flashDirection(undefined, 5)).toBeNull();
  });

  it("non-finite sides are treated as unavailable, never as a change", () => {
    expect(flashDirection(NaN, 5)).toBeNull();
    expect(flashDirection(5, Infinity)).toBeNull();
    expect(flashDirection(Infinity, 5)).toBeNull();
  });
});

describe("nextFlashState — flash fires only on change", () => {
  it("first observation records the value without animating (dir null)", () => {
    const m = nextFlashState(INITIAL_FLASH, 10);
    expect(m.prev).toBe(10);
    expect(m.dir).toBeNull();
  });

  it("a change advances seq and carries the direction", () => {
    let m = nextFlashState(INITIAL_FLASH, 10);
    m = nextFlashState(m, 12);
    expect(m.seq).toBe(2);
    expect(m.dir).toBe("up");
    expect(flashClass(m)).toBe("arbx-flash-up");
    m = nextFlashState(m, 11);
    expect(m.dir).toBe("down");
    expect(flashClass(m)).toBe("arbx-flash-down");
  });

  it("an IDENTICAL value returns the SAME object — no seq bump, no replay", () => {
    let m = nextFlashState(INITIAL_FLASH, 10);
    m = nextFlashState(m, 12); // change → seq 2, dir up
    const unchanged = nextFlashState(m, 12);
    expect(unchanged).toBe(m); // object identity: key/class stay put
    expect(unchanged.seq).toBe(2);
    // a later identical render keeps returning the same object
    expect(nextFlashState(unchanged, 12)).toBe(unchanged);
  });

  it("consecutive same-direction changes keep advancing seq (replay key)", () => {
    let m = nextFlashState(INITIAL_FLASH, 10);
    m = nextFlashState(m, 11);
    const seq1 = m.seq;
    m = nextFlashState(m, 12);
    expect(m.seq).toBe(seq1 + 1);
    expect(m.dir).toBe("up");
  });

  it("value → dash → value does not flash (nothing comparable in between)", () => {
    let m = nextFlashState(INITIAL_FLASH, 10);
    m = nextFlashState(m, null);
    expect(m.dir).toBeNull();
    m = nextFlashState(m, 10);
    expect(m.dir).toBeNull();
  });
});
