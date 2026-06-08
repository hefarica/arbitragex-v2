/**
 * ReadinessStepsResponseSchema contract tests.
 *
 * Guards the FE↔BE wire contract for the server-side 4-step "Live Readiness"
 * stepper (backend readiness-steps.ts). The frontend N/4 count is now driven by
 * this payload, so a drift here would silently break the stepper.
 *
 * Key invariants asserted:
 *   - a realistic 0/4 payload (all PENDING/BLOCKED) validates,
 *   - a 4/4 payload validates with live_enabled:false,
 *   - a payload that tries to report live_enabled:true is REJECTED (structural).
 */
import { describe, it, expect } from "vitest";
import { ReadinessStepsResponseSchema } from "./schemas";

const STEP_0_OF_4 = {
  generated_at: "2026-06-08T04:00:00.000Z",
  source: "runtime_evidence",
  mode: "paper_only",
  completed_steps: 0,
  total_steps: 4,
  readiness_score: "0/4",
  all_ready: false,
  live_enabled: false,
  capital_exposure_usd: 0,
  steps: [
    {
      step: 1,
      id: "topology",
      name: "Topology Vault",
      status: "PENDING",
      ready: false,
      reason: "topology_vault_empty: no non-empty snapshot persisted.",
      evidence: [],
      wss_active_count: 0,
      rpc_active_count: 0,
      snapshot_non_empty: false,
    },
    {
      step: 2,
      id: "credentials",
      name: "Credentials / Wallets",
      status: "BLOCKED",
      ready: false,
      reason: 'Blocked until "Topology Vault" passes.',
      evidence: [{ label: "signer", detail: "absent" }],
      signer: null,
      private_key: null,
      private_relay: null,
      redacted: true,
    },
    {
      step: 3,
      id: "markets",
      name: "Market Topology",
      status: "BLOCKED",
      ready: false,
      reason: 'Blocked until "Credentials / Wallets" passes.',
      evidence: [],
      chains_count: 0,
      dexes_count: 0,
      pools_count: 0,
      tokens_count: 0,
    },
    {
      step: 4,
      id: "engines",
      name: "Resolution Engines",
      status: "BLOCKED",
      ready: false,
      reason: 'Blocked until "Market Topology" passes.',
      evidence: [{ label: "engines", detail: "none enabled" }],
      engines_active: [],
      paper_ready: false,
      shadow_ready: false,
      live_enabled: false,
    },
  ],
};

describe("ReadinessStepsResponseSchema", () => {
  it("validates a realistic 0/4 payload", () => {
    const r = ReadinessStepsResponseSchema.safeParse(STEP_0_OF_4);
    expect(r.success).toBe(true);
  });

  it("validates a 4/4 payload with live_enabled:false", () => {
    const payload = {
      ...STEP_0_OF_4,
      completed_steps: 4,
      readiness_score: "4/4",
      all_ready: true,
      steps: STEP_0_OF_4.steps.map((s) => ({ ...s, status: "PASS", ready: true })),
    };
    const r = ReadinessStepsResponseSchema.safeParse(payload);
    expect(r.success).toBe(true);
  });

  it("REJECTS a payload that reports live_enabled:true (structural safety)", () => {
    const r = ReadinessStepsResponseSchema.safeParse({ ...STEP_0_OF_4, live_enabled: true });
    expect(r.success).toBe(false);
  });

  it("REJECTS a credentials signer value other than 'present'|null (no leakage)", () => {
    const bad = {
      ...STEP_0_OF_4,
      steps: STEP_0_OF_4.steps.map((s) => (s.id === "credentials" ? { ...s, signer: "0xRAWADDRESS" } : s)),
    };
    const r = ReadinessStepsResponseSchema.safeParse(bad);
    expect(r.success).toBe(false);
  });

  it("REJECTS an unknown source (must be runtime_evidence)", () => {
    const r = ReadinessStepsResponseSchema.safeParse({ ...STEP_0_OF_4, source: "mock" });
    expect(r.success).toBe(false);
  });
});
