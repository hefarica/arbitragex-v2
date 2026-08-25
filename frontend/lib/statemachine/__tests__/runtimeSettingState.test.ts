import { describe, expect, it } from "vitest";

import {
  RUNTIME_SETTING_STATE_COPY,
  deriveRuntimeSettingState,
  type RuntimeSettingAckOutcome,
} from "../runtimeSettingState";

const NONE: RuntimeSettingAckOutcome = { kind: "none" };
const WAITING: RuntimeSettingAckOutcome = { kind: "waiting" };
const APPLIED: RuntimeSettingAckOutcome = { kind: "applied" };
const REJECTED: RuntimeSettingAckOutcome = { kind: "rejected" };
const TIMEOUT: RuntimeSettingAckOutcome = { kind: "timeout" };

describe("deriveRuntimeSettingState — FE-0005 (§3/§14/§64)", () => {
  it("effective null with no mutation = NOT_EXPOSED (R8: absence, never zero)", () => {
    expect(
      deriveRuntimeSettingState({ configured: 22, effective: null, ack: NONE }),
    ).toBe("NOT_EXPOSED");
  });

  it("runtime reporting the configured value = EFFECTIVE (string/number equivalence)", () => {
    expect(
      deriveRuntimeSettingState({ configured: 0.5, effective: "0.5", ack: NONE }),
    ).toBe("EFFECTIVE");
  });

  it("saved but runtime silent on the new value = CONFIGURED (never claims convergence)", () => {
    expect(
      deriveRuntimeSettingState({ configured: 0.6, effective: 0.5, ack: NONE }),
    ).toBe("CONFIGURED");
  });

  it("pending event_id with no settle = WAITING_RUNTIME_ACK", () => {
    expect(
      deriveRuntimeSettingState({ configured: 0.6, effective: 0.5, ack: WAITING }),
    ).toBe("WAITING_RUNTIME_ACK");
  });

  it("ACK + version coherence = VERIFIED", () => {
    expect(
      deriveRuntimeSettingState({
        configured: 0.6,
        effective: 0.6,
        version: { configured: 3, effective: 3 },
        ack: APPLIED,
      }),
    ).toBe("VERIFIED");
  });

  it("ACK + versions disagree = DRIFT (§47 coherence beats the ack)", () => {
    expect(
      deriveRuntimeSettingState({
        configured: 0.6,
        effective: 0.6,
        version: { configured: 3, effective: 2 },
        ack: APPLIED,
      }),
    ).toBe("DRIFT");
  });

  it("ACK without a live effective version = APPLIED (cannot over-claim VERIFIED)", () => {
    expect(
      deriveRuntimeSettingState({
        configured: 0.6,
        effective: 0.6,
        version: { configured: 3, effective: null },
        ack: APPLIED,
      }),
    ).toBe("APPLIED");
  });

  it("ACK with no version prop at all = APPLIED", () => {
    expect(
      deriveRuntimeSettingState({ configured: 0.6, effective: 0.6, ack: APPLIED }),
    ).toBe("APPLIED");
  });

  it("nack terminal reason = REJECTED; nack timeout = TIMEOUT (refetch ground truth)", () => {
    expect(
      deriveRuntimeSettingState({ configured: 0.6, effective: 0.5, ack: REJECTED }),
    ).toBe("REJECTED");
    expect(
      deriveRuntimeSettingState({ configured: 0.6, effective: 0.5, ack: TIMEOUT }),
    ).toBe("TIMEOUT");
  });

  it("steady-state version mismatch = DRIFT even without a pending mutation", () => {
    expect(
      deriveRuntimeSettingState({
        configured: 0.6,
        effective: 0.6,
        version: { configured: 4, effective: 3 },
        ack: NONE,
      }),
    ).toBe("DRIFT");
  });

  it("steady-state with effective version still null falls through to value comparison", () => {
    expect(
      deriveRuntimeSettingState({
        configured: 0.6,
        effective: 0.6,
        version: { configured: 4, effective: null },
        ack: NONE,
      }),
    ).toBe("EFFECTIVE");
  });

  it("every render state ships explainability copy (§40)", () => {
    for (const copy of Object.values(RUNTIME_SETTING_STATE_COPY)) {
      expect(copy.label.length).toBeGreaterThan(0);
      expect(copy.hint.length).toBeGreaterThan(0);
    }
  });
});
