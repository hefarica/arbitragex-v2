import { describe, it, expect } from "vitest";
import {
  resolveAllowedMode,
  isModeAllowed,
  signingPermitted,
  SAFE_MODE_POSTURE,
  type ModePosture,
} from "./modes";

const open: ModePosture = {
  live_enabled: true,
  readiness_green: true,
  kill_switch_off: true,
  simulation_ready: true,
  blind_signing_enabled: false,
  automation_canary_approved: false,
};

describe("wallet modes — deny-by-default", () => {
  it("safe/default posture resolves to READ_ONLY and permits no signing", () => {
    const d = resolveAllowedMode(SAFE_MODE_POSTURE);
    expect(d.mode).toBe("READ_ONLY");
    expect(d.automationLocked).toBe(true);
    expect(signingPermitted(SAFE_MODE_POSTURE)).toBe(false);
    expect(isModeAllowed("MANUAL_SIGN", SAFE_MODE_POSTURE)).toBe(false);
    expect(isModeAllowed("INTENT_SIGN", SAFE_MODE_POSTURE)).toBe(false);
    expect(isModeAllowed("AUTOMATION_LOCKED", SAFE_MODE_POSTURE)).toBe(false);
  });

  it("blind_signing_enabled forces READ_ONLY even if everything else is open", () => {
    const d = resolveAllowedMode({ ...open, blind_signing_enabled: true });
    expect(d.mode).toBe("READ_ONLY");
  });

  it("live + kill-switch-off permits MANUAL_SIGN but not INTENT_SIGN without sim/readiness", () => {
    const d = resolveAllowedMode({ ...open, readiness_green: false, simulation_ready: false });
    expect(d.mode).toBe("MANUAL_SIGN");
    expect(isModeAllowed("INTENT_SIGN", { ...open, readiness_green: false, simulation_ready: false })).toBe(false);
  });

  it("full green (no canary) permits INTENT_SIGN, and automation stays LOCKED", () => {
    const d = resolveAllowedMode(open);
    expect(d.mode).toBe("INTENT_SIGN");
    expect(d.automationLocked).toBe(true);
    expect(isModeAllowed("AUTOMATION_LOCKED", open)).toBe(false);
  });

  it("automation only unlocks with explicit canary approval AND full green", () => {
    const d = resolveAllowedMode({ ...open, automation_canary_approved: true });
    expect(d.mode).toBe("AUTOMATION_LOCKED");
    expect(d.automationLocked).toBe(false);
    // missing any green keeps automation locked
    expect(resolveAllowedMode({ ...open, automation_canary_approved: true, readiness_green: false }).automationLocked).toBe(true);
  });
});
