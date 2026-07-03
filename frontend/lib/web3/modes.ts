// Wallet operating modes for the Command Center.
//
// READ_ONLY is the ABSOLUTE default. Higher modes are DENY-BY-DEFAULT: a mode is only permitted
// when the backend posture explicitly opens the corresponding gates. No function here signs or
// broadcasts anything — modes only decide WHICH affordances the UI may offer; the actual
// signing/broadcast paths remain independently gated by the Policy Engine + backend live posture.
//
// Invariants preserved:
//   Connect wallet != signer / != broadcast / != capital / != approval / != automation.

export const WALLET_MODES = ["READ_ONLY", "MANUAL_SIGN", "INTENT_SIGN", "AUTOMATION_LOCKED"] as const;
export type WalletMode = (typeof WALLET_MODES)[number];

export const MODE_RANK: Record<WalletMode, number> = {
  READ_ONLY: 0,
  MANUAL_SIGN: 1,
  INTENT_SIGN: 2,
  AUTOMATION_LOCKED: 3,
};

// Backend/runtime posture that gates which mode is permitted. All-false is the safe default.
export interface ModePosture {
  live_enabled: boolean;
  readiness_green: boolean;
  kill_switch_off: boolean;
  simulation_ready: boolean;
  blind_signing_enabled: boolean; // MUST be false — any true forces READ_ONLY
  automation_canary_approved: boolean; // M5/A.9/live-canary human sign-off; false until granted
}

export const SAFE_MODE_POSTURE: ModePosture = {
  live_enabled: false,
  readiness_green: false,
  kill_switch_off: false,
  simulation_ready: false,
  blind_signing_enabled: false,
  automation_canary_approved: false,
};

export interface ModeDecision {
  mode: WalletMode; // highest currently-permitted mode
  automationLocked: boolean; // true unless the live-canary gate is fully open
  reason: string; // why this ceiling (human-readable)
}

// Returns the HIGHEST mode the given posture permits. Deny-by-default: anything not explicitly
// enabled collapses to READ_ONLY. blind_signing being enabled ALWAYS forces READ_ONLY.
export function resolveAllowedMode(p: ModePosture): ModeDecision {
  if (p.blind_signing_enabled) {
    return { mode: "READ_ONLY", automationLocked: true, reason: "blind_signing_enabled — forced read-only" };
  }
  const automationLocked = !(
    p.automation_canary_approved &&
    p.live_enabled &&
    p.readiness_green &&
    p.kill_switch_off &&
    p.simulation_ready
  );
  if (!automationLocked) {
    return { mode: "AUTOMATION_LOCKED", automationLocked: false, reason: "live-canary approved: automation ceiling" };
  }
  if (p.live_enabled && p.readiness_green && p.kill_switch_off && p.simulation_ready) {
    return { mode: "INTENT_SIGN", automationLocked, reason: "live+readiness+killswitch-off+sim-ready: legible EIP-712 intent signing permitted" };
  }
  if (p.live_enabled && p.kill_switch_off) {
    return { mode: "MANUAL_SIGN", automationLocked, reason: "live+killswitch-off: explicit manual signing permitted" };
  }
  return { mode: "READ_ONLY", automationLocked, reason: "posture not open — read-only" };
}

// Is a specific requested mode currently permitted?
export function isModeAllowed(requested: WalletMode, p: ModePosture): boolean {
  const ceiling = resolveAllowedMode(p);
  if (requested === "AUTOMATION_LOCKED") return !ceiling.automationLocked;
  return MODE_RANK[requested] <= MODE_RANK[ceiling.mode];
}

// May the UI offer any signing affordance at all (MANUAL_SIGN or above)?
export function signingPermitted(p: ModePosture): boolean {
  const { mode } = resolveAllowedMode(p);
  return MODE_RANK[mode] >= MODE_RANK.MANUAL_SIGN;
}
