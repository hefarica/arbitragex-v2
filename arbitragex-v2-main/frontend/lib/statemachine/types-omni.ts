/**
 * =============================================================================
 * OMEGA statemachine/types-omni — extends MutableEntityType to all 12 entities
 * =============================================================================
 *
 * Merge-instruction: replace `MutableEntityType` in
 * `frontend/lib/statemachine/types.ts` with the union below. All discriminated
 * unions that reference it inherit the extension transparently.
 *
 * State Machine UI contract (mandatory rendering per directive §7):
 *
 *   IDLE
 *     → VALIDATING
 *       → PREVIEW
 *         → CONFIRM_REQUIRED
 *           → WRITING_DB
 *             → EMITTING_RELOAD
 *               → WAITING_RUNTIME_ACK
 *                 → VERIFIED
 *                 | PARTIAL
 *                 | FAILED
 *                 → ROLLBACK_AVAILABLE
 *
 * "success" must NEVER render until runtime_ack.status = applied for every
 * targeted layer.
 *
 * @module statemachine/types-omni
 */

export type OmniMutableEntityType =
  | "chain"
  | "rpc"
  | "dex"
  | "pool"
  | "token"
  | "wallet"
  | "strategy"
  | "contract"
  | "risk_gate"
  | "capital_gate"
  | "relay"
  | "agent";

export type OmniRegistryMutationAction =
  | "create"
  | "update"
  | "delete"
  | "enable"
  | "disable"
  | "reload"
  | "rollback";

export type OmniActionResult = "success" | "partial" | "failed";

/** Discriminated union covering every UI state the operator can observe. */
export type OmniMachineState =
  | { kind: "IDLE" }
  | { kind: "VALIDATING"; payload: unknown; entityType: OmniMutableEntityType }
  | {
      kind: "PREVIEW";
      payload: unknown;
      entityType: OmniMutableEntityType;
      config_hash_before: string | null;
      config_hash_after: string;
      diff: Record<string, unknown>;
    }
  | { kind: "CONFIRM_REQUIRED"; reason: string; entityType: OmniMutableEntityType }
  | { kind: "WRITING_DB"; entityType: OmniMutableEntityType }
  | {
      kind: "EMITTING_RELOAD";
      entityType: OmniMutableEntityType;
      event_id: string;
      idempotency_key: string;
      channels: string[];
    }
  | {
      kind: "WAITING_RUNTIME_ACK";
      entityType: OmniMutableEntityType;
      event_id: string;
      expected_layers: string[];
      received_layers: string[];
      timeout_at: string;
    }
  | {
      kind: "VERIFIED";
      entityType: OmniMutableEntityType;
      event_id: string;
      config_hash_after: string;
      runtime_latency_ms: number;
    }
  | {
      kind: "PARTIAL";
      entityType: OmniMutableEntityType;
      event_id: string;
      missing_layers: string[];
      drift_observation_id: string | null;
    }
  | {
      kind: "FAILED";
      entityType: OmniMutableEntityType;
      event_id: string | null;
      error: string;
    }
  | {
      kind: "ROLLBACK_AVAILABLE";
      entityType: OmniMutableEntityType;
      original_state: unknown;
    };

/** Action layer expected for every entity action (used by the UI checker). */
export const EXPECTED_LAYERS_PER_ACTION: Record<OmniRegistryMutationAction, string[]> = {
  create:   ["api","persistence","redis_pubsub","searcher_rs","arc_swap","readiness","audit"],
  update:   ["api","persistence","redis_pubsub","searcher_rs","arc_swap","readiness","audit"],
  delete:   ["api","persistence","redis_pubsub","searcher_rs","arc_swap","readiness","audit"],
  enable:   ["api","persistence","redis_pubsub","searcher_rs","arc_swap","audit"],
  disable:  ["api","persistence","redis_pubsub","searcher_rs","arc_swap","audit"],
  reload:   ["redis_pubsub","searcher_rs","arc_swap","frontend_refresh","audit"],
  rollback: ["api","persistence","redis_pubsub","searcher_rs","arc_swap","readiness","audit"],
};

/** Per-entity reload channel resolver (mirrors backend). */
export function reloadChannelsFor(
  entity: OmniMutableEntityType,
  chainId: number | null,
): string[] {
  const map: Record<OmniMutableEntityType, string> = {
    chain:        "arbx:config:chains",
    rpc:          "arbx:config:rpcs",
    dex:          "arbx:config:dexes",
    pool:         "arbx:config:pools",
    token:        "arbx:config:tokens",
    wallet:       "arbx:config:wallets",
    strategy:     "arbx:config:strategies",
    contract:     "arbx:config:contracts",
    risk_gate:    "arbx:config:risk",
    capital_gate: "arbx:config:capital",
    relay:        "arbx:config:relays",
    agent:        "arbx:config:agents",
  };
  const base = map[entity];
  const channels = [`${base}:reload`];
  const isChainScoped = ["chain","rpc","dex","pool","token","wallet","contract","relay"].includes(entity);
  if (isChainScoped && chainId !== null) channels.push(`${base}:${chainId}:reload`);
  return channels;
}
