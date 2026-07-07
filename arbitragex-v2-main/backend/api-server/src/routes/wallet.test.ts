import { describe, it, expect } from "vitest";

import { __forTesting as wallet } from "./wallet.js";
import { __forTesting as siwe } from "./auth-siwe.js";

/**
 * Unit tests for the Web3 safe-gated wallet + SIWE pure logic.
 *
 * These assert the HARD INVARIANTS at the source: the safe posture is literally
 * { live_enabled:false, capital_exposed:0, broadcast:false }, the intent
 * terminal state is BROADCAST_DISABLED, nonces are single-use + expiring, and
 * session tokens are HMAC-verified (tamper → reject).
 */

describe("wallet safe posture", () => {
  it("SAFE_POSTURE is the hardcoded-safe set", () => {
    expect(wallet.SAFE_POSTURE).toEqual({ live_enabled: false, capital_exposed: 0, broadcast: false });
  });

  it("terminal intent state is BROADCAST_DISABLED with policy reason", () => {
    expect(wallet.TERMINAL_STATE).toBe("BROADCAST_DISABLED");
    expect(wallet.BROADCAST_DISABLED_REASON).toBe("broadcast_disabled_by_policy");
  });

  it("safety gate matrix proves capital_exposure 0 at PASS", () => {
    const exposure = wallet.safetyGates().find((g) => g.name === "capital_exposure");
    expect(exposure).toBeTruthy();
    expect(exposure?.status).toBe("PASS");
    expect(exposure?.value).toBe(0);
  });

  it("BROADCAST_DISABLED is one of the documented intent states", () => {
    expect(wallet.INTENT_STATES).toContain("BROADCAST_DISABLED");
  });

  it("readIntent ignores any attempt to inject capital/broadcast flags", () => {
    const parsed = wallet.readIntent({ kind: "swap", broadcast: true, live: true } as unknown);
    expect(parsed.kind).toBe("swap");
    // No broadcast/live keys survive parsing.
    expect((parsed as Record<string, unknown>).broadcast).toBeUndefined();
    expect((parsed as Record<string, unknown>).live).toBeUndefined();
  });
});

describe("SIWE nonce store", () => {
  it("issues a non-trivial nonce and consumes it exactly once", () => {
    const n = siwe.issueNonce();
    expect(typeof n).toBe("string");
    expect(n.length).toBeGreaterThanOrEqual(8);
    expect(siwe.consumeNonce(n)).toBe(true);
    // Second consume fails (single-use).
    expect(siwe.consumeNonce(n)).toBe(false);
  });

  it("rejects an unknown nonce", () => {
    expect(siwe.consumeNonce("never-issued-nonce")).toBe(false);
  });

  it("rejects an expired nonce", () => {
    const now = 1_000_000;
    const n = siwe.issueNonce(now);
    // Consume well past the TTL.
    expect(siwe.consumeNonce(n, now + siwe.NONCE_TTL_MS + 1)).toBe(false);
  });
});

describe("SIWE session token (HMAC)", () => {
  const secret = "x".repeat(64); // >= 32 bytes.
  const baseClaims = {
    address: "0x1111111111111111111111111111111111111111",
    chainId: 1,
    iat: Date.now(),
    exp: Date.now() + 60_000,
    mode: "wallet_identity_only" as const,
  };

  it("round-trips a valid session", () => {
    const token = siwe.signSession(baseClaims, secret);
    const claims = siwe.verifySession(token, secret);
    expect(claims).not.toBeNull();
    expect(claims?.address).toBe(baseClaims.address);
    expect(claims?.mode).toBe("wallet_identity_only");
  });

  it("rejects a tampered token", () => {
    const token = siwe.signSession(baseClaims, secret);
    const tampered = token.slice(0, -2) + (token.endsWith("AA") ? "BB" : "AA");
    expect(siwe.verifySession(tampered, secret)).toBeNull();
  });

  it("rejects a token signed with a different secret", () => {
    const token = siwe.signSession(baseClaims, secret);
    expect(siwe.verifySession(token, "y".repeat(64))).toBeNull();
  });

  it("rejects an expired session", () => {
    const expired = { ...baseClaims, exp: Date.now() - 1 };
    const token = siwe.signSession(expired, secret);
    expect(siwe.verifySession(token, secret)).toBeNull();
  });
});
