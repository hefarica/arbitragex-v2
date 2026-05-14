/**
 * Ω-S5++ C9.∞ APEX · Hook tipado useChains
 * -------------------------------------------------------------------------
 * Hook canónico para listar y mutar cadenas. Demuestra el patrón APEX:
 *   1. Solo retorna estados tipados explícitos.
 *   2. Mutaciones requieren idempotency_key.
 *   3. Optimistic updates SOLO si hay rollback declarado.
 *   4. PENDING_RUNTIME_ACK es estado de primera clase.
 *
 * NOTA Mirror Law: este hook se ubica en `lib/apex/hooks/` — extiende, no
 * reemplaza, los hooks existentes en `lib/hooks/`. La página puede importar
 * el que necesite sin romper la compatibilidad con el v5 base.
 */
import { useCallback, useEffect, useState } from 'react';
import type { TypedApiClient, ApiResult } from '../api/client';
import {
  ChainListResponseSchema,
  ChainCreateRequestSchema,
  ChainEditRequestSchema,
  ChainToggleRequestSchema,
  MutationResponseSchema,
  type ChainEntity,
  type ChainCreateRequest,
  type ChainEditRequest,
  type ChainToggleRequest,
  type IdempotencyKey,
} from '../schemas';

// ─── Estado público del hook ────────────────────────────────────────────
export type ChainListState =
  | { phase: 'idle' }
  | { phase: 'loading' }
  | { phase: 'success'; chains: ReadonlyArray<ChainEntity>; sequence_id: number }
  | { phase: 'blocked'; reason: string; next_action: string }
  | { phase: 'unavailable'; reason: string }
  | { phase: 'error'; code: string; message: string }
  | { phase: 'parse_error'; message: string };

export type MutationPhase =
  | 'IDLE'
  | 'VALIDATING'
  | 'SUBMITTING'
  | 'PERSISTING'
  | 'WAITING_RUNTIME_ACK'
  | 'VERIFIED'
  | 'PARTIAL'
  | 'BLOCKED'
  | 'FAILED'
  | 'ROLLBACK_AVAILABLE';

export interface MutationState {
  readonly phase: MutationPhase;
  readonly message: string | null;
  readonly auditEventId: string | null;
}

// ─── Helper: traduce ApiResult a phase del list ─────────────────────────
function resultToListState<T extends { chains: ReadonlyArray<ChainEntity>; sequence_id: number }>(
  result: ApiResult<T>,
): ChainListState {
  switch (result.kind) {
    case 'ok':
      return { phase: 'success', chains: result.data.chains, sequence_id: result.data.sequence_id };
    case 'blocked':
      return { phase: 'blocked', reason: result.data.reason, next_action: result.data.next_action };
    case 'unavailable':
      return { phase: 'unavailable', reason: result.data.reason };
    case 'error':
      return { phase: 'error', code: result.data.code, message: result.data.message };
    case 'network_error':
      return { phase: 'error', code: 'NETWORK', message: result.message };
    case 'parse_error':
      return { phase: 'parse_error', message: result.message };
  }
}

function resultToMutationState(result: ApiResult<z.infer<typeof MutationResponseSchema>>): MutationState {
  switch (result.kind) {
    case 'ok': {
      if (result.data.status === 'OK') {
        return {
          phase: result.data.ack_pending ? 'WAITING_RUNTIME_ACK' : 'VERIFIED',
          message: 'Persisted to DB',
          auditEventId: result.data.audit_event_id,
        };
      }
      // status not OK inside OK kind shouldn't happen due to discriminated union,
      // but TS narrowing requires the branch.
      return { phase: 'FAILED', message: 'Unexpected response shape', auditEventId: null };
    }
    case 'blocked':
      return { phase: 'BLOCKED', message: result.data.reason, auditEventId: null };
    case 'unavailable':
      return { phase: 'PARTIAL', message: result.data.reason, auditEventId: null };
    case 'error':
      return { phase: 'FAILED', message: result.data.message, auditEventId: null };
    case 'network_error':
      return { phase: 'FAILED', message: result.message, auditEventId: null };
    case 'parse_error':
      return { phase: 'FAILED', message: result.message, auditEventId: null };
  }
}

// We use a tiny type-only import for zod here (no runtime cost)
import type { z } from 'zod';

// ─── Hook principal ─────────────────────────────────────────────────────
export interface UseChainsApi {
  readonly listState: ChainListState;
  readonly mutationState: MutationState;
  readonly reload: () => Promise<void>;
  readonly createChain: (req: ChainCreateRequest) => Promise<MutationState>;
  readonly editChain: (req: ChainEditRequest) => Promise<MutationState>;
  readonly toggleChain: (req: ChainToggleRequest) => Promise<MutationState>;
}

export function useChains(client: TypedApiClient): UseChainsApi {
  const [listState, setListState] = useState<ChainListState>({ phase: 'idle' });
  const [mutationState, setMutationState] = useState<MutationState>({
    phase: 'IDLE',
    message: null,
    auditEventId: null,
  });

  const reload = useCallback(async () => {
    setListState({ phase: 'loading' });
    const result = await client.get('/api/v1/chains', ChainListResponseSchema);
    setListState(resultToListState(result));
  }, [client]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const runMutation = useCallback(
    async <TReq>(
      path: string,
      method: 'POST' | 'PUT',
      reqSchema: z.ZodSchema<TReq>,
      body: TReq,
      idempotencyKey: IdempotencyKey,
    ): Promise<MutationState> => {
      setMutationState({ phase: 'VALIDATING', message: null, auditEventId: null });
      const op = method === 'POST' ? client.post : client.put;
      setMutationState({ phase: 'SUBMITTING', message: null, auditEventId: null });
      const result = await op(path, reqSchema, MutationResponseSchema, body, idempotencyKey);
      const next = resultToMutationState(result);
      setMutationState(next);
      if (next.phase === 'WAITING_RUNTIME_ACK' || next.phase === 'VERIFIED') {
        void reload();
      }
      return next;
    },
    [client, reload],
  );

  const createChain = useCallback(
    (req: ChainCreateRequest) =>
      runMutation('/api/v1/admin/chains', 'POST', ChainCreateRequestSchema, req, req.idempotency_key),
    [runMutation],
  );

  const editChain = useCallback(
    (req: ChainEditRequest) =>
      runMutation(
        `/api/v1/admin/chains/${req.chain_id}`,
        'PUT',
        ChainEditRequestSchema,
        req,
        req.idempotency_key,
      ),
    [runMutation],
  );

  const toggleChain = useCallback(
    (req: ChainToggleRequest) =>
      runMutation(
        `/api/v1/admin/chains/${req.chain_id}/toggle`,
        'PUT',
        ChainToggleRequestSchema,
        req,
        req.idempotency_key,
      ),
    [runMutation],
  );

  return { listState, mutationState, reload, createChain, editChain, toggleChain };
}
