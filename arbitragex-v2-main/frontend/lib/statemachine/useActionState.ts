"use client";

import { useState, useCallback } from "react";
import type { ActionState } from "./types";
import { useRuntimeAckSocket } from "./useRuntimeAckSocket";

/**
 * Hook that implements the critical-action state machine.
 *
 * Every config mutation (trading params, chain enable/disable, killswitch,
 * strategy weights, …) flows through a strict sequence of states:
 *
 *   IDLE → VALIDATING → PREVIEW → CONFIRM_REQUIRED → WRITING_DB
 *   → EMITTING_RELOAD → WAITING_RUNTIME_ACK → VERIFIED
 *
 * On failure the machine lands in FAILED where rollback may be offered.
 *
 * @example
 * ```tsx
 * const action = useActionState();
 *
 * async function handleUpdate() {
 *   action.start(crypto.randomUUID());
 *   action.preview(oldConfig, newConfig);
 *   // … operator reviews …
 *   action.confirm("Enable chain 137, bump gas +12%");
 *   // … operator clicks Confirm …
 *   action.writeDb(action.state.idempotency_key);
 *   await persistToPostgres(newConfig);
 *   action.emitReload(eventId);
 *   await waitForRuntimeAck(eventId);
 *   action.verify(configHash);
 * }
 * ```
 */
export function useActionState() {
  const [state, setState] = useState<ActionState>({ kind: "IDLE" });

  /**
   * Transition from IDLE → VALIDATING.
   * @param idempotencyKey - UUID that deduplicates this mutation server-side.
   */
  const start = useCallback((idempotencyKey: string) => {
    setState({ kind: "VALIDATING", idempotency_key: idempotencyKey });
  }, []);

  /**
   * Transition to PREVIEW showing before/after diff.
   * Call after validation succeeds and you have the computed delta.
   */
  const preview = useCallback((before: unknown, after: unknown) => {
    setState({ kind: "PREVIEW", before, after });
  }, []);

  /**
   * Transition to CONFIRM_REQUIRED — blocks until operator explicitly approves.
   * @param summary - Human-readable description of the change (logged to audit).
   */
  const confirm = useCallback((summary: string) => {
    setState({ kind: "CONFIRM_REQUIRED", changes_summary: summary });
  }, []);

  /**
   * Transition to WRITING_DB — PostgreSQL is the single source of truth.
   * @param key - The idempotency key originally passed to `start()`.
   */
  const writeDb = useCallback((key: string) => {
    setState({ kind: "WRITING_DB", idempotency_key: key });
  }, []);

  /**
   * Transition to EMITTING_RELOAD — notifies runtime via Redis pub/sub.
   * @param eventId - The config-changed event ID pushed to the reload channel.
   */
  const emitReload = useCallback((eventId: string) => {
    setState({ kind: "EMITTING_RELOAD", event_id: eventId });
  }, []);

  /**
   * Transition to WAITING_RUNTIME_ACK — polling until runtime applies the change.
   * @param eventId   - Same event passed to `emitReload()`.
   * @param elapsed   - Milliseconds already spent waiting (for UI progress).
   */
  const waitAck = useCallback((eventId: string, elapsed: number) => {
    setState({ kind: "WAITING_RUNTIME_ACK", event_id: eventId, elapsed_ms: elapsed });
  }, []);

  /**
   * Terminal success — config hash verified across all three layers.
   * @param hash - The SHA-256 hash of the applied configuration.
   */
  const verify = useCallback((hash: string) => {
    setState({ kind: "VERIFIED", config_hash_after: hash });
  }, []);

  /**
   * Terminal failure — records the error and whether rollback is available.
   * @param error   - Human-readable or structured error message.
   * @param rollback - Whether the system can auto-revert to the previous config.
   */
  const fail = useCallback((error: string, rollback: boolean) => {
    setState({ kind: "FAILED", error, rollback_available: rollback });
  }, []);

  /** Reset the machine to IDLE. Safe to call from any state. */
  const reset = useCallback(() => {
    setState({ kind: "IDLE" });
  }, []);

  // OMEGA-7 Runtime ACK WSS bridge: when the machine is parked in
  // WAITING_RUNTIME_ACK, subscribe to the backend WSS room `runtime_ack`
  // (PR #73, broadcast post-INSERT). Strict event_id correlation enforces
  // invariant I-1; safeParse enforces RG-1; heartbeat enforces RG-3.
  useRuntimeAckSocket({
    eventId: state.kind === "WAITING_RUNTIME_ACK" ? state.event_id : null,
    onAck: (payload) => {
      setState({ kind: "VERIFIED", config_hash_after: payload.config_hash_after });
    },
    onNack: (reason) => {
      setState({ kind: "FAILED", error: reason, rollback_available: true });
    },
  });

  return {
    state,
    start,
    preview,
    confirm,
    writeDb,
    emitReload,
    waitAck,
    verify,
    fail,
    reset,
  } as const;
}
