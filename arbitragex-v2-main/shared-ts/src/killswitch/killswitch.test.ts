import { describe, it, expect } from "vitest";
import { KillSwitchStateSchema } from "../contracts/index.js";

describe("KillSwitchState schema", () => {
  it("accepts a valid payload", () => {
    const ok = KillSwitchStateSchema.safeParse({
      enabled: true,
      reason: "manual_test",
      triggered_by: "test",
      updated_at: "2026-04-20T00:00:00.000Z",
    });
    expect(ok.success).toBe(true);
  });

  it("rejects extra fields", () => {
    const bad = KillSwitchStateSchema.safeParse({
      enabled: true,
      reason: null,
      triggered_by: null,
      updated_at: "2026-04-20T00:00:00.000Z",
      injected: "x",
    });
    expect(bad.success).toBe(false);
  });

  it("rejects missing updated_at", () => {
    const bad = KillSwitchStateSchema.safeParse({
      enabled: true, reason: null, triggered_by: null,
    });
    expect(bad.success).toBe(false);
  });
});
