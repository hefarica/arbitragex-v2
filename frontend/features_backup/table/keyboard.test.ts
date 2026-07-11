import { describe, expect, it } from "vitest";

import { nextRowIndex, PAGE_JUMP } from "./keyboard";

describe("DataTable nextRowIndex", () => {
  it("ArrowDown advances by one and clamps to last row", () => {
    expect(nextRowIndex("ArrowDown", 0, 5)).toBe(1);
    expect(nextRowIndex("ArrowDown", 4, 5)).toBe(4);
  });

  it("ArrowUp retreats by one and clamps to zero", () => {
    expect(nextRowIndex("ArrowUp", 3, 5)).toBe(2);
    expect(nextRowIndex("ArrowUp", 0, 5)).toBe(0);
  });

  it("Home jumps to the first row", () => {
    expect(nextRowIndex("Home", 7, 10)).toBe(0);
    expect(nextRowIndex("Home", 0, 10)).toBe(0);
  });

  it("End jumps to the last row", () => {
    expect(nextRowIndex("End", 0, 10)).toBe(9);
    expect(nextRowIndex("End", 9, 10)).toBe(9);
  });

  it("PageDown advances by PAGE_JUMP and clamps", () => {
    expect(nextRowIndex("PageDown", 0, 100)).toBe(PAGE_JUMP);
    expect(nextRowIndex("PageDown", 95, 100)).toBe(99);
  });

  it("PageUp retreats by PAGE_JUMP and clamps", () => {
    expect(nextRowIndex("PageUp", 50, 100)).toBe(50 - PAGE_JUMP);
    expect(nextRowIndex("PageUp", 3, 100)).toBe(0);
  });

  it("returns null for unhandled keys (lets browser propagate)", () => {
    expect(nextRowIndex("Tab", 0, 5)).toBeNull();
    expect(nextRowIndex("Enter", 0, 5)).toBeNull();
    expect(nextRowIndex(" ", 0, 5)).toBeNull();
    expect(nextRowIndex("a", 0, 5)).toBeNull();
  });

  it("returns null when there are no rows (no-op)", () => {
    expect(nextRowIndex("ArrowDown", 0, 0)).toBeNull();
    expect(nextRowIndex("Home", 0, 0)).toBeNull();
  });
});
