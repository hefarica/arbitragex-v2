/**
 * OMEGA-8 / M3 FASE 6 — P1-3: PII hardening wired-in (audit_event).
 *
 * This test verifies that the SQL emitted by `audit_event` writes wraps
 * raw `ip_address` and `user_agent` parameters with the migration-053
 * helpers (`arbx_anonymize_ip()::cidr` and `arbx_hash_user_agent()`), so
 * a code-level bypass (skipping mig 070 retroactive cleanup) cannot
 * persist plaintext PII for new rows.
 *
 * Strategy: read the source file as text and assert the SQL fragment is
 * present. This is a defense against accidental regression (the wrapping
 * is the contract; removing it silently is the bug we are guarding
 * against).
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";

describe("OMEGA-8/M3 P1-3: PII hardening wired-in", () => {
  it("registry-engine.ts wraps ip_address with arbx_anonymize_ip()::cidr in audit_event INSERT", () => {
    const src = readFileSync(
      path.join(__dirname, "registry-engine.ts"),
      "utf8",
    );
    expect(src).toContain("arbx_anonymize_ip");
    expect(src).toContain("::cidr");
  });

  it("registry-engine.ts wraps user_agent with arbx_hash_user_agent in audit_event INSERT", () => {
    const src = readFileSync(
      path.join(__dirname, "registry-engine.ts"),
      "utf8",
    );
    expect(src).toContain("arbx_hash_user_agent");
  });

  it("audit_event INSERT in registry-engine.ts does NOT bind raw $13/$14 to ip/UA columns", () => {
    const src = readFileSync(
      path.join(__dirname, "registry-engine.ts"),
      "utf8",
    );
    // Find the INSERT block and confirm $13 / $14 are wrapped.
    const insertBlock = src.match(/INSERT INTO audit_event[\s\S]*?ON CONFLICT/);
    expect(insertBlock).not.toBeNull();
    expect(insertBlock![0]).toContain("arbx_anonymize_ip($13)");
    expect(insertBlock![0]).toContain("arbx_hash_user_agent($14)");
  });
});
