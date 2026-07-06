/**
 * Ω-S5++ C9.∞ APEX · State machine para acción crítica
 * -------------------------------------------------------------------------
 * Modelo finito y verificable:
 *
 *   IDLE → EDITING → VALIDATING → PREVIEW → CONFIRM_REQUIRED →
 *   SUBMITTING → PERSISTING → RELOAD_EMITTED → WAITING_RUNTIME_ACK →
 *   VERIFIED  (terminal éxito)
 *     ↘ PARTIAL  (terminal parcial)
 *     ↘ FAILED   (terminal fallo, con ROLLBACK_AVAILABLE flag)
 *
 * Toda transición es pura y declarativa.
 * Γ.StateMachine: equivalente a un autómata DFA con 11 estados y 14 eventos.
 */
export type CriticalActionState =
  | 'IDLE'
  | 'EDITING'
  | 'VALIDATING'
  | 'PREVIEW'
  | 'CONFIRM_REQUIRED'
  | 'SUBMITTING'
  | 'PERSISTING'
  | 'RELOAD_EMITTED'
  | 'WAITING_RUNTIME_ACK'
  | 'VERIFIED'
  | 'PARTIAL'
  | 'FAILED';

export type CriticalActionEvent =
  | { kind: 'EDIT' }
  | { kind: 'VALIDATE_OK' }
  | { kind: 'VALIDATE_FAIL'; reason: string }
  | { kind: 'PREVIEW_READY' }
  | { kind: 'REQUIRE_CONFIRMATION' }
  | { kind: 'CONFIRM' }
  | { kind: 'SUBMIT_OK' }
  | { kind: 'SUBMIT_FAIL'; reason: string }
  | { kind: 'PERSIST_OK' }
  | { kind: 'PERSIST_FAIL'; reason: string }
  | { kind: 'RELOAD_EMITTED' }
  | { kind: 'ACK_OK' }
  | { kind: 'ACK_TIMEOUT' }
  | { kind: 'ACK_FAIL'; reason: string }
  | { kind: 'RESET' };

export interface CriticalActionContext {
  readonly state: CriticalActionState;
  readonly errorReason: string | null;
  readonly rollbackAvailable: boolean;
}

export const INITIAL_CONTEXT: CriticalActionContext = {
  state: 'IDLE',
  errorReason: null,
  rollbackAvailable: false,
};

const TERMINAL: ReadonlySet<CriticalActionState> = new Set(['VERIFIED', 'PARTIAL', 'FAILED']);

export function isTerminal(state: CriticalActionState): boolean {
  return TERMINAL.has(state);
}

/**
 * Transición pura. Mantiene INV-3 (causality): cada cambio depende sólo
 * del estado actual y el evento.
 */
export function transition(
  ctx: CriticalActionContext,
  event: CriticalActionEvent,
): CriticalActionContext {
  if (event.kind === 'RESET') return INITIAL_CONTEXT;
  switch (ctx.state) {
    case 'IDLE':
      if (event.kind === 'EDIT') return { ...ctx, state: 'EDITING' };
      return ctx;
    case 'EDITING':
      if (event.kind === 'VALIDATE_OK') return { ...ctx, state: 'VALIDATING' };
      return ctx;
    case 'VALIDATING':
      if (event.kind === 'VALIDATE_FAIL') {
        return { state: 'FAILED', errorReason: event.reason, rollbackAvailable: false };
      }
      if (event.kind === 'PREVIEW_READY') return { ...ctx, state: 'PREVIEW' };
      return ctx;
    case 'PREVIEW':
      if (event.kind === 'REQUIRE_CONFIRMATION') return { ...ctx, state: 'CONFIRM_REQUIRED' };
      return ctx;
    case 'CONFIRM_REQUIRED':
      if (event.kind === 'CONFIRM') return { ...ctx, state: 'SUBMITTING' };
      return ctx;
    case 'SUBMITTING':
      if (event.kind === 'SUBMIT_OK') return { ...ctx, state: 'PERSISTING' };
      if (event.kind === 'SUBMIT_FAIL') {
        return { state: 'FAILED', errorReason: event.reason, rollbackAvailable: false };
      }
      return ctx;
    case 'PERSISTING':
      if (event.kind === 'PERSIST_OK') return { ...ctx, state: 'RELOAD_EMITTED', rollbackAvailable: true };
      if (event.kind === 'PERSIST_FAIL') {
        return { state: 'FAILED', errorReason: event.reason, rollbackAvailable: true };
      }
      return ctx;
    case 'RELOAD_EMITTED':
      if (event.kind === 'RELOAD_EMITTED') return { ...ctx, state: 'WAITING_RUNTIME_ACK' };
      return ctx;
    case 'WAITING_RUNTIME_ACK':
      if (event.kind === 'ACK_OK') return { state: 'VERIFIED', errorReason: null, rollbackAvailable: true };
      if (event.kind === 'ACK_TIMEOUT') {
        return { state: 'PARTIAL', errorReason: 'runtime ack timeout', rollbackAvailable: true };
      }
      if (event.kind === 'ACK_FAIL') {
        return { state: 'PARTIAL', errorReason: event.reason, rollbackAvailable: true };
      }
      return ctx;
    case 'VERIFIED':
    case 'PARTIAL':
    case 'FAILED':
      return ctx;
  }
}
