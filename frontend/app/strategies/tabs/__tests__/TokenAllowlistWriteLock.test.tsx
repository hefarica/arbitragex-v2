/**
 * Write lock (contract decision 3, ratified 2026-10-21).
 *
 * The ranking may serve NOT COMPUTED rows; the allowlist is a write and cannot
 * be saved without a verifiable rule population. These cases pin the reason the
 * operator reads, because a disabled button with no reason invites a bypass.
 */
import { describe, expect, it } from "vitest";
import { writeLockReason } from "../TokenAllowlistTab";

describe("writeLockReason — cerrojo de guardado", () => {
  it("allows the save when the gate is open", () => {
    expect(writeLockReason({ allowed: true, reasons: [] })).toBeNull();
    expect(writeLockReason(null)).toBeNull();
  });

  it("names the volume-freshness cause, not a generic 'blocked'", () => {
    const r = writeLockReason({ allowed: false, reasons: ["selection_not_computed"] });
    expect(r).toContain("no es computable");
    expect(r).toContain("frescura de volumen");
  });

  it("names the rule-population cause", () => {
    const r = writeLockReason({ allowed: false, reasons: ["no_verified_rule_population"] });
    expect(r).toContain("población verificable");
  });

  it("prints both when both hold, and never invents a reason", () => {
    const r = writeLockReason({
      allowed: false,
      reasons: ["selection_not_computed", "no_verified_rule_population"],
    });
    expect(r).toContain("frescura de volumen");
    expect(r).toContain("población verificable");
    expect(writeLockReason({ allowed: false, reasons: ["something_else"] })).toBe("Guardado bloqueado.");
  });
});
