/**
 * =============================================================================
 * FE-MASTER · RuntimeSettingState derivation — FE-0005 (§3/§14/§64)
 * =============================================================================
 *
 * Pure state derivation for the configured ≠ effective contract. The wire
 * truth (Runtime ACK payloads, version counters) lives in the hooks/slices;
 * this module only DECIDES what the operator should see, so it is fully
 * testable without React or sockets.
 *
 * Doctrine (FE-MASTER §14 + ruling EMIT-04 consumption 2026-08-23):
 *   - HTTP 200 ≠ APPLIED. Only a runtime ACK broadcast (event_id bijection,
 *     useRuntimeAckSocket I-1/RG-1/RG-2) moves the state past WAITING.
 *   - null effective = NOT computed/served — rendered "—", NEVER zero (R8).
 *   - TIMEOUT is honest: the PUT already returned an event_id whose row may
 *     never have landed (EMIT-04 outer-catch). The consumer refetches and the
 *     version counters are the ground truth afterwards.
 *   - DRIFT = configured/effective VERSIONS disagree (§47), whether or not an
 *     ack is pending. Value-level comparison alone never claims convergence
 *     when versions exist to prove it.
 */

export type RuntimeSettingRenderState =
  | "NOT_EXPOSED"
  | "CONFIGURED"
  | "EFFECTIVE"
  | "WAITING_RUNTIME_ACK"
  | "APPLIED"
  | "VERIFIED"
  | "REJECTED"
  | "TIMEOUT"
  | "DRIFT";

/** What the ACK channel reported for the current ackEventId. */
export type RuntimeSettingAckOutcome =
  | { kind: "none" } // no mutation in flight (steady state)
  | { kind: "waiting" } // event_id pending — no settle yet
  | { kind: "applied" } // onAck fired (hook maps status applied|received)
  | { kind: "rejected" } // onNack with a terminal non-timeout reason
  | { kind: "timeout" }; // onNack("timeout") — refetch for ground truth

export interface RuntimeSettingVersion {
  /** Version stamped by the mutation (e.g. universe_version from the PUT). */
  configured: number;
  /** Version served live by the runtime; null = not served yet (R8). */
  effective: number | null;
}

export interface RuntimeSettingStateInput {
  configured: string | number;
  effective: string | number | null;
  version?: RuntimeSettingVersion;
  ack: RuntimeSettingAckOutcome;
}

export function deriveRuntimeSettingState(
  input: RuntimeSettingStateInput,
): RuntimeSettingRenderState {
  const { configured, effective, version, ack } = input;

  // Mutation lifecycle first — the operator's question is "did my change land?"
  if (ack.kind === "rejected") return "REJECTED";
  if (ack.kind === "timeout") return "TIMEOUT";
  if (ack.kind === "applied") {
    // ACK says the persistence layer applied it. Coherence (§47) is provable
    // only when a live effective version was served to compare.
    if (version && version.effective !== null) {
      return version.effective === version.configured ? "VERIFIED" : "DRIFT";
    }
    return "APPLIED";
  }
  if (ack.kind === "waiting") return "WAITING_RUNTIME_ACK";

  // Steady state — describe the observed world without a pending mutation.
  if (effective === null) return "NOT_EXPOSED";
  if (version && version.effective !== null && version.effective !== version.configured) {
    return "DRIFT";
  }
  return String(effective) === String(configured) ? "EFFECTIVE" : "CONFIGURED";
}

/** Gate-explainability copy (§40): one plain sentence per state. */
export const RUNTIME_SETTING_STATE_COPY: Record<
  RuntimeSettingRenderState,
  { label: string; hint: string }
> = {
  NOT_EXPOSED: {
    label: "NOT EXPOSED",
    hint: "Runtime has not served this value — not computed, never zero.",
  },
  CONFIGURED: {
    label: "CONFIGURED",
    hint: "Saved. Runtime has not reported convergence with this value.",
  },
  EFFECTIVE: {
    label: "EFFECTIVE",
    hint: "Runtime reports exactly the configured value.",
  },
  WAITING_RUNTIME_ACK: {
    label: "WAITING ACK",
    hint: "Persisted — waiting for the runtime ACK broadcast (HTTP 200 is not APPLIED).",
  },
  APPLIED: {
    label: "APPLIED",
    hint: "Runtime ACK received at the persistence layer; no live version served yet to cross-check.",
  },
  VERIFIED: {
    label: "VERIFIED",
    hint: "ACK received and configured/effective versions agree.",
  },
  REJECTED: {
    label: "REJECTED",
    hint: "Runtime rejected this change.",
  },
  TIMEOUT: {
    label: "TIMEOUT",
    hint: "No ACK landed — refetch: the durable version counters are the ground truth.",
  },
  DRIFT: {
    label: "DRIFT",
    hint: "Configured and effective versions disagree.",
  },
};
