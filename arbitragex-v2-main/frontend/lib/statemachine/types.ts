/**
 * =============================================================================
 * OMEGA Critical-Action State Machine Types
 * =============================================================================
 *
 * Every mutating operation on the operational registry flows through this
 * explicit state machine.  No database write happens outside these states.
 *
 * States are intentionally modelled as a discriminated union (kind tag) so
 * that TypeScript exhaustive-switch checking prevents unhandled transitions.
 *
 * Ghost Protocol: ACTIVO  |  R8 Fail-Honest: PRESERVADO
 *
 * @module statemachine/types
 */

// ---------------------------------------------------------------------------
// Action primitives
// ---------------------------------------------------------------------------

/**
 * The kinds of mutations that can be requested on the registry.
 *
 * These are the verbs the state machine understands.
 */
export type RegistryMutationAction =
  | "create"
  | "update"
  | "delete"
  | "enable"
  | "disable"
  | "reload"
  | "rollback";

/** The entity types that can be mutated through the state machine. */
export type MutableEntityType =
  | "chain"
  | "dex"
  | "pool"
  | "token"
  | "wallet"
  | "strategy"
  | "risk_gate"
  | "relay";

/** Outcome of a completed state-machine run. */
export type ActionResult = "success" | "partial" | "failed";

// ---------------------------------------------------------------------------
// State machine states (discriminated union)
// ---------------------------------------------------------------------------

/**
 * No operation in progress.  The machine rests here between actions.
 */
export interface IdleState {
  kind: "IDLE";
}

/**
 * Input validation and idempotency-key generation are running.
 *
 * The machine stays here until the payload passes schema validation
 * and a unique idempotency key has been minted.
 */
export interface ValidatingState {
  kind: "VALIDATING";
  /** UUID v4 used for exactly-once semantics across retries. */
  idempotency_key: string;
}

/**
 * A dry-run preview has been computed showing the before/after delta.
 *
 * The UI renders this state as a confirmation dialog with a diff view.
 */
export interface PreviewState {
  kind: "PREVIEW";
  /** Entity state before the mutation (deep clone). */
  before: unknown;
  /** Projected entity state after the mutation (deep clone). */
  after: unknown;
}

/**
 * Human confirmation is required before proceeding.
 *
 * Reached for actions flagged as high-risk (delete, rollback, kill_switch).
 */
export interface ConfirmRequiredState {
  kind: "CONFIRM_REQUIRED";
  /** One-line summary of what will change (e.g. "Disable 3 pools on Arbitrum"). */
  changes_summary: string;
}

/**
 * The validated mutation is being written to the canonical PostgreSQL store.
 *
 * The idempotency key guarantees that retries of this state are safe.
 */
export interface WritingDbState {
  kind: "WRITING_DB";
  idempotency_key: string;
}

/**
 * A cross-system reload event has been emitted so that every downstream
 * consumer (Redis cache, runtime hot-reload, frontend refetch) picks up
 * the new configuration.
 */
export interface EmittingReloadState {
  kind: "EMITTING_RELOAD";
  /** UUID of the reload event for tracing. */
  event_id: string;
}

/**
 * Waiting for the runtime engine to acknowledge that it has consumed
 * the reload event and applied the new configuration.
 */
export interface WaitingRuntimeAckState {
  kind: "WAITING_RUNTIME_ACK";
  event_id: string;
  /** Milliseconds elapsed since the reload event was emitted. */
  elapsed_ms: number;
}

/**
 * Full success: the mutation is persisted, the runtime has acknowledged,
 * and config hashes match end-to-end.
 */
export interface VerifiedState {
  kind: "VERIFIED";
  /** Hash of the entity after the successful mutation. */
  config_hash_after: string;
}

/**
 * Partial success: the database write succeeded but one or more
 * downstream consumers failed to apply the change.
 *
 * retryable = true means a manual retry may complete the propagation.
 */
export interface PartialState {
  kind: "PARTIAL";
  /** Human-readable explanation of what did NOT succeed. */
  reason: string;
  /** Whether a retry is expected to succeed. */
  retryable: boolean;
}

/**
 * Hard failure: the mutation could not be persisted or the database
 * transaction rolled back.
 *
 * rollback_available = true means the system captured a pre-mutation snapshot
 * that can be restored.
 */
export interface FailedState {
  kind: "FAILED";
  /** Error message or exception dump. */
  error: string;
  /** Whether a snapshot exists for rollback. */
  rollback_available: boolean;
}

/**
 * A pre-mutation snapshot is available for rollback.
 *
 * This is a terminal state reached after FAILED when rollback_available
 * is true.  The UI shows a "Rollback" button that transitions back to
 * WRITING_DB with the original state.
 */
export interface RollbackAvailableState {
  kind: "ROLLBACK_AVAILABLE";
  /** Deep clone of the entity state before the failed mutation. */
  original_state: unknown;
}

// ---------------------------------------------------------------------------
// State union
// ---------------------------------------------------------------------------

/**
 * Every possible state of the critical-action state machine.
 *
 * Use an exhaustive switch on `state.kind` to guarantee every state is
 * handled in UI and reducer logic:
 *
 * ```typescript
 * switch (state.kind) {
 *   case "IDLE":               // ...
 *   case "VALIDATING":         // ...
 *   // ... all cases required by TypeScript exhaustiveness checking
 * }
 * ```
 */
export type ActionState =
  | IdleState
  | ValidatingState
  | PreviewState
  | ConfirmRequiredState
  | WritingDbState
  | EmittingReloadState
  | WaitingRuntimeAckState
  | VerifiedState
  | PartialState
  | FailedState
  | RollbackAvailableState;

// ---------------------------------------------------------------------------
// Transition event (input that drives state changes)
// ---------------------------------------------------------------------------

/**
 * Events that the state-machine reducer consumes to move from one
 * ActionState to the next.
 */
export type ActionEvent =
  | { type: "SUBMIT"; payload: unknown; action: RegistryMutationAction; entityType: MutableEntityType }
  | { type: "VALIDATION_OK"; idempotency_key: string }
  | { type: "VALIDATION_FAIL"; errors: string[] }
  | { type: "PREVIEW_OK"; before: unknown; after: unknown }
  | { type: "CONFIRM" }
  | { type: "REJECT" }
  | { type: "DB_WRITE_OK"; event_id: string }
  | { type: "DB_WRITE_FAIL"; error: string }
  | { type: "RELOAD_EMITTED"; event_id: string }
  | { type: "RUNTIME_ACK"; config_hash_after: string }
  | { type: "RUNTIME_NACK"; reason: string }
  | { type: "TIMEOUT"; elapsed_ms: number }
  | { type: "RETRY" }
  | { type: "ROLLBACK" }
  | { type: "RESET" };

// ---------------------------------------------------------------------------
// State-machine context (used by React/useReducer or XState)
// ---------------------------------------------------------------------------

/**
 * Persistent context that accompanies the state machine across transitions.
 *
 * Kept separate from ActionState so that the discriminated union stays
 * clean and serialisable.
 */
export interface ActionContext {
  /** The mutation being processed (undefined in IDLE). */
  pending_action?: RegistryMutationAction;
  /** The entity type being mutated (undefined in IDLE). */
  pending_entity_type?: MutableEntityType;
  /** The original payload submitted by the user. */
  pending_payload?: unknown;
  /** Idempotency key minted during VALIDATING. */
  idempotency_key?: string;
  /** ISO-8601 timestamp when the action was first submitted. */
  started_at?: string;
  /** ISO-8601 timestamp of the most recent transition. */
  last_transition_at?: string;
  /** Number of retry attempts already consumed. */
  retry_count: number;
  /** Maximum retries permitted for this action. */
  max_retries: number;
  /** Aggregated log of every transition for forensic debugging. */
  transition_log: TransitionLogEntry[];
}

/** A single entry in the forensic transition log. */
export interface TransitionLogEntry {
  from: ActionState["kind"];
  to: ActionState["kind"];
  event: ActionEvent["type"];
  at: string;
  meta?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Machine definition (config + reducer signature)
// ---------------------------------------------------------------------------

/**
 * Configuration that parameterises the behaviour of the state machine.
 */
export interface StateMachineConfig {
  /** Actions that require human confirmation before WRITING_DB. */
  confirm_required_actions: RegistryMutationAction[];
  /** Actions that are allowed to show a PREVIEW step. */
  preview_enabled_actions: RegistryMutationAction[];
  /** Timeout (ms) for the runtime to acknowledge a reload event. */
  runtime_ack_timeout_ms: number;
  /** Maximum retry attempts before a transition is considered permanently FAILED. */
  max_retries: number;
}

/**
 * Tuple returned by the reducer: the new state and an optional side-effect
 * description (e.g. "emit websocket event", "call fetch").
 */
export interface StateMachineResult {
  state: ActionState;
  context: ActionContext;
  effect?: StateMachineEffect;
}

/** Declarative side-effect description — the caller executes it. */
export type StateMachineEffect =
  | { kind: "VALIDATE"; payload: unknown; entityType: MutableEntityType }
  | { kind: "DB_WRITE"; idempotency_key: string; payload: unknown }
  | { kind: "EMIT_RELOAD"; event_id: string }
  | { kind: "WAIT_RUNTIME_ACK"; event_id: string; timeout_ms: number }
  | { kind: "SHOW_CONFIRM"; changes_summary: string }
  | { kind: "SHOW_PREVIEW"; before: unknown; after: unknown }
  | { kind: "LOG_AUDIT"; event: unknown }
  | { kind: "NOTIFY"; message: string; severity: "info" | "warning" | "error" };
