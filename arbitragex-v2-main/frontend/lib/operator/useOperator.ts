/**
 * Hook useOperator() — fuente única de identidad + gates + manifest.
 *
 * Cero datos hardcodeados, cero mocks, cero placeholders.
 * Si /api/operator/me devuelve BLOCKED → estado BLOCKED visible.
 */

'use client';

import { useEffect, useState, useCallback } from 'react';
import type { OperatorMeResponse, RegistryKey, OperatorRole } from './types';

export type OperatorState =
  | { status: 'loading' }
  | { status: 'ready'; data: OperatorMeResponse }
  | { status: 'blocked'; reason: string }
  | { status: 'unavailable'; error: string };

export function useOperator(): OperatorState & {
  refresh: () => void;
  canAccessRegistry: (registry: RegistryKey) => boolean;
  hasMinRole: (role: OperatorRole) => boolean;
} {
  const [state, setState] = useState<OperatorState>({ status: 'loading' });

  const load = useCallback(async () => {
    setState({ status: 'loading' });
    try {
      const res = await fetch('/api/operator/me', { credentials: 'include' });
      if (res.status === 401 || res.status === 403) {
        const body = await res.json().catch(() => ({ reason: 'OPERATOR_UNAUTHENTICATED' }));
        setState({ status: 'blocked', reason: body.reason ?? 'UNKNOWN' });
        return;
      }
      if (!res.ok) {
        setState({ status: 'unavailable', error: `HTTP_${res.status}` });
        return;
      }
      const data = (await res.json()) as OperatorMeResponse;
      setState({ status: 'ready', data });
    } catch (err) {
      setState({
        status: 'unavailable',
        error: err instanceof Error ? err.message : 'NETWORK_ERROR',
      });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const canAccessRegistry = useCallback(
    (registry: RegistryKey): boolean => {
      if (state.status !== 'ready') return false;
      const op = state.data.operator;
      if (op.role === 'sovereign') return true;
      return op.allowed_registries.includes(registry);
    },
    [state]
  );

  const hasMinRole = useCallback(
    (role: OperatorRole): boolean => {
      if (state.status !== 'ready') return false;
      const order: Record<OperatorRole, number> = {
        observer: 1,
        steward: 2,
        sovereign: 3,
      };
      return order[state.data.operator.role] >= order[role];
    },
    [state]
  );

  return Object.assign(state, {
    refresh: load,
    canAccessRegistry,
    hasMinRole,
  });
}
