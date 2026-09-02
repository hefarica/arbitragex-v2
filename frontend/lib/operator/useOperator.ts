/**
 * Hook useOperator() — fuente única de identidad + gates + manifest.
 *
 * Cero datos hardcodeados, cero mocks, cero placeholders.
 * Si /api/operator/me devuelve BLOCKED → estado BLOCKED visible.
 *
 * OPERATOR-IDENTITY (2026-09-02, workbook 20260902_152349Z): the registry
 * pages mount several <OperatorGate> per page and EVERY mount used to fire
 * its own /api/operator/me — 2 guaranteed-401 requests per page for an
 * anonymous visitor (12 of the audit's 13 actionable failures). Two fixes,
 * both frontend-only (the backend L8 gate stays untouched — it must keep
 * refusing unauthenticated callers):
 *
 *   1. SESSION GATE: /me is only called when hasAdminSession() says the
 *      operator signed in (companion TTL cookie). Anonymous visitors get
 *      the honest blocked state (OPERATOR_UNAUTHENTICATED) with ZERO
 *      network traffic — the L8 401 is the server's business, not a
 *      request we volunteer to fail.
 *   2. DEDUPE: a module-level shared promise makes N concurrent mounts
 *      share ONE request; refresh() invalidates it deliberately.
 *
 * NOTE: even with an admin session, /api/operator/me currently answers
 * 401 OPERATOR_MISSING_IDENTITY — the api-server mounts the operator
 * router without operatorIdentityMiddleware, and operator_parametrization
 * has no registered identity source yet. That is the honest live state
 * (rendered as blocked); fixing it requires an operator-registration +
 * request-signing design decision from the operator, not a FE hack.
 */

'use client';

import { useEffect, useState, useCallback } from 'react';
import { hasAdminSession } from '@/lib/admin-token';
import type { OperatorMeResponse, RegistryKey, OperatorRole } from './types';

export type OperatorState =
  | { status: 'loading' }
  | { status: 'ready'; data: OperatorMeResponse }
  | { status: 'blocked'; reason: string }
  | { status: 'unavailable'; error: string };

/** Shared outcome of the single in-flight /me request (dedupe). */
type SharedResult =
  | { kind: 'ready'; data: OperatorMeResponse }
  | { kind: 'blocked'; reason: string }
  | { kind: 'unavailable'; error: string };

let sharedPromise: Promise<SharedResult> | null = null;

async function fetchOperatorOnce(): Promise<SharedResult> {
  try {
    const res = await fetch('/api/operator/me', { credentials: 'include' });
    if (res.status === 401 || res.status === 403) {
      const body = await res.json().catch(() => ({ reason: 'OPERATOR_UNAUTHENTICATED' }));
      return { kind: 'blocked', reason: body.reason ?? 'UNKNOWN' };
    }
    if (!res.ok) {
      return { kind: 'unavailable', error: `HTTP_${res.status}` };
    }
    const data = (await res.json()) as OperatorMeResponse;
    return { kind: 'ready', data };
  } catch (err) {
    return {
      kind: 'unavailable',
      error: err instanceof Error ? err.message : 'NETWORK_ERROR',
    };
  }
}

/** Invalidate the shared request (refresh / post-signin / post-logout). */
export function invalidateOperatorShared(): void {
  sharedPromise = null;
}

/**
 * Resolve the operator state ONCE per page-load: session-gated + deduped.
 * Exported for the contract tests (anonymous = zero requests; N callers =
 * one request; 401 = honest blocked reason). The hook below is a thin
 * wrapper around this.
 */
export async function resolveOperator(): Promise<OperatorState> {
  // Anonymous visitor: honest blocked state, zero network traffic. The
  // companion TTL cookie is the ONLY session signal readable client-side
  // (the real session cookie is httpOnly by design, V-AT-1).
  if (!hasAdminSession()) {
    sharedPromise = null; // a later sign-in must trigger a fresh request
    return { status: 'blocked', reason: 'OPERATOR_UNAUTHENTICATED' };
  }
  if (!sharedPromise) sharedPromise = fetchOperatorOnce();
  const result = await sharedPromise;
  return (
    result.kind === 'ready'
      ? { status: 'ready', data: result.data }
      : result.kind === 'blocked'
        ? { status: 'blocked', reason: result.reason }
        : { status: 'unavailable', error: result.error }
  );
}

export function useOperator(): OperatorState & {
  refresh: () => void;
  canAccessRegistry: (registry: RegistryKey) => boolean;
  hasMinRole: (role: OperatorRole) => boolean;
} {
  const [state, setState] = useState<OperatorState>({ status: 'loading' });

  const load = useCallback(async () => {
    setState({ status: 'loading' });
    setState(await resolveOperator());
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const refresh = useCallback(() => {
    invalidateOperatorShared(); // deliberate invalidation — one new request
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
    refresh,
    canAccessRegistry,
    hasMinRole,
  });
}
