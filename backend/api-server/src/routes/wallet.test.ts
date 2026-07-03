import { describe, it, expect } from "vitest";
import express from "express";
import request from "supertest";

import { __forTesting as wallet, mountWalletRoutes } from "./wallet.js";
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

// ---------------------------------------------------------------------------
// /api/wallet/simulate — deny-by-default verdict + fail-closed endpoint.
// ---------------------------------------------------------------------------

type Gates = Parameters<typeof wallet.simulateVerdict>[0];

// A maximally-green gates object (incl. live_gate_open forced true) to exercise
// the allow=true path. The ENDPOINT always forces live_gate_open=false.
function gates(over: Record<string, unknown> = {}): Gates {
  return {
    simulation_passed: true,
    calldata_hash_matches_sim: true,
    net_profit_usd: 100,
    risk_score: 90,
    gas_estimate: "1000",
    routeHash: `0x${"ab".repeat(32)}`,
    calldataHash: `0x${"cd".repeat(32)}`,
    policyId: "p1",
    readiness_green: true,
    kill_switch_off: true,
    live_gate_open: true,
    ...over,
  } as unknown as Gates;
}

describe("simulate verdict — deny-by-default", () => {
  it("allows only when EVERY gate is green", () => {
    const r = wallet.simulateVerdict(gates());
    expect(r.allow).toBe(true);
    expect(r.denied).toEqual([]);
  });

  it("live_gate_open=false (structural) → deny", () => {
    const r = wallet.simulateVerdict(gates({ live_gate_open: false }));
    expect(r.allow).toBe(false);
    expect(r.denied).toContain("live_gate_open");
  });

  it("readiness NO-GO → deny", () => {
    expect(wallet.simulateVerdict(gates({ readiness_green: false })).denied).toContain("readiness_green");
  });

  it("kill-switch on → deny", () => {
    expect(wallet.simulateVerdict(gates({ kill_switch_off: false })).denied).toContain("kill_switch_off");
  });

  it("calldata hash mismatch → deny", () => {
    expect(wallet.simulateVerdict(gates({ calldata_hash_matches_sim: false })).denied).toContain(
      "calldata_hash_matches_sim",
    );
  });

  it("net_profit_usd <= 0 → deny", () => {
    expect(wallet.simulateVerdict(gates({ net_profit_usd: 0 })).denied).toContain("net_profit_usd_positive");
    expect(wallet.simulateVerdict(gates({ net_profit_usd: null })).denied).toContain("net_profit_usd_positive");
  });

  it("risk_score below minimum (or null) → deny", () => {
    expect(wallet.simulateVerdict(gates({ risk_score: wallet.RISK_SCORE_MIN - 1 })).denied).toContain(
      "risk_score_minimum",
    );
    expect(wallet.simulateVerdict(gates({ risk_score: null })).denied).toContain("risk_score_minimum");
  });
});

describe("POST /api/wallet/simulate — fail-closed (no runtime wired)", () => {
  const app = express();
  app.use(express.json());
  mountWalletRoutes(app, { logger: { warn: () => {}, info: () => {} } });

  it("no fork-sim runtime → allow=false, reason=runtime_not_configured, broadcast disabled", async () => {
    const r = await request(app)
      .post("/api/wallet/simulate")
      .send({ chain_id: 11155111, calldataHash: `0x${"cd".repeat(32)}`, policyId: "p1" });
    expect(r.status).toBe(200);
    expect(r.body.allow).toBe(false);
    expect(r.body.reason).toBe("runtime_not_configured");
    expect(r.body.simulation_passed).toBe(false);
    expect(r.body.calldata_hash_matches_sim).toBe(false);
    expect(r.body.live_gate_open).toBe(false);
    // HARD posture always present + safe.
    expect(r.body.broadcast_allowed).toBe(false);
    expect(r.body.live_enabled).toBe(false);
    expect(r.body.capital_exposed).toBe(0);
    expect(r.body.broadcast).toBe(false);
  });

  it("even a PASSING fork-sim (readiness green, kill-switch off) still denies — live_gate_open is structural false", async () => {
    const app2 = express();
    app2.use(express.json());
    mountWalletRoutes(app2, {
      logger: { warn: () => {}, info: () => {} },
      readiness: async () => ({ green: true }),
      killSwitch: async () => ({ off: true }),
      forkSimulator: async (i) => ({
        passed: true,
        calldataHash: i.calldataHash ?? "",
        routeHash: `0x${"ab".repeat(32)}`,
        netProfitUsd: 50,
        riskScore: 90,
        gasEstimate: "21000",
      }),
    });
    const r = await request(app2)
      .post("/api/wallet/simulate")
      .send({ chain_id: 11155111, calldataHash: `0x${"cd".repeat(32)}` });
    expect(r.body.simulation_passed).toBe(true);
    expect(r.body.calldata_hash_matches_sim).toBe(true);
    expect(r.body.readiness_green).toBe(true);
    expect(r.body.kill_switch_off).toBe(true);
    expect(r.body.live_gate_open).toBe(false);
    expect(r.body.allow).toBe(false);
    expect(r.body.denied).toContain("live_gate_open");
    expect(r.body.broadcast_allowed).toBe(false);
  });
});
