/**
 * /omega-s5/registry/[entity] — Client orchestrator.
 *
 * Receives `initialSnapshot` from the Server Component. The snapshot is a
 * discriminated union: OK (data ready), AUTH_REQUIRED (operator must sign
 * in), UNAVAILABLE (registry not wired or upstream failed). The Server
 * Component never throws on 401/403; it returns AUTH_REQUIRED and we render
 * the sign-in prompt here without ever exposing the real token.
 *
 * Mirror Law Extendida: only composes shadcn/ui primitives already present.
 * Does NOT touch layout.tsx, globals.css, tailwind.config.ts, or ui/*.
 *
 * V-AT-1: client refetch passes credentials:"include" automatically because
 * api-client's getValidated() now sets it; the operator's httpOnly cookie
 * travels alongside the request.
 */
'use client';

import React, { useCallback, useMemo, useState } from 'react';
import Link from 'next/link';

import { OperatorGate } from '@/components/operator/OperatorGate';
import { useRegistry } from '@/lib/registries/useRegistry';
import { useOmniDrift } from '@/lib/drift/useOmniDrift';
import { useActionState } from '@/lib/statemachine/useActionState';
import { RegistryCoherenceStrip } from '@/components/registries/RegistryCoherenceStrip';
import { getAdminChains } from '@/lib/api-client';
import type { RegistryKey } from '@/lib/operator/types';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { AdminSessionBadge } from '@/components/AdminSessionBadge';

export interface RegistryRow {
  readonly id: string;
  readonly enabled: boolean;
  readonly config_hash: string;
}

export type RegistrySnapshot =
  | { status: 'OK'; data: readonly RegistryRow[] }
  | { status: 'AUTH_REQUIRED' }
  | { status: 'UNAVAILABLE'; reason?: string };

interface Props {
  readonly registryKey: RegistryKey;
  readonly initialSnapshot: RegistrySnapshot;
}

const REGISTRY_LABELS: Record<RegistryKey, string> = {
  chain: 'Chains',
  rpc: 'RPC Endpoints',
  dex: 'DEXes',
  token: 'Tokens',
  pool: 'Pools',
  wallet: 'Wallets',
  strategy: 'Strategies',
  contract: 'Contract Registry',
  risk_gate: 'Risk Gates',
  capital_gate: 'Capital Gates',
  relay: 'Relays',
  agent: 'Agent Registry',
};

/**
 * Adapter: mapea cada RegistryKey a su fetcher canónico. Hoy solo `chain`
 * tiene endpoint admin productivo; los demás devuelven array vacío
 * (UNAVAILABLE en SSR, fail-honest en cliente). Esto NO es mock.
 */
async function fetchRegistryByKey(key: RegistryKey): Promise<readonly RegistryRow[]> {
  if (key === 'chain') {
    const res = await getAdminChains();
    if (!res.ok) throw new Error(res.error);
    return res.data.items.map((c) => ({
      id: String(c.chain_id),
      enabled: c.enabled,
      config_hash: c.config_hash ?? 'pending',
    }));
  }
  return [];
}

export function RegistryPageClient({ registryKey, initialSnapshot }: Props): JSX.Element {
  const fetchFn = useCallback(
    () => fetchRegistryByKey(registryKey),
    [registryKey],
  );

  // Only auto-refresh when SSR delivered usable data; AUTH_REQUIRED freezes
  // the polling loop so we don't hammer the edge with guaranteed-401s.
  const autoStart = initialSnapshot.status === 'OK';

  const {
    items,
    isLoading,
    error,
    hasData,
    refresh,
  } = useRegistry<RegistryRow>({
    fetchFn,
    pollIntervalMs: 30_000,
    keyFn: (r) => r.id,
    autoStart,
  });

  const drift = useOmniDrift(5_000);
  const observations = useMemo(
    () => drift.byResource[registryKey] ?? [],
    [drift.byResource, registryKey],
  );

  // SSR snapshot precedence: when SSR delivered OK we use those rows until
  // the client has refreshed at least once. When SSR delivered
  // AUTH_REQUIRED or UNAVAILABLE we render the explicit state.
  const ssrRows = initialSnapshot.status === 'OK' ? initialSnapshot.data : [];
  const rows = hasData ? items : ssrRows;
  const loading = isLoading;

  const status: string = error
    ? 'ERROR'
    : loading
      ? 'LOADING'
      : hasData
        ? 'READY'
        : initialSnapshot.status === 'OK'
          ? 'READY'
          : initialSnapshot.status === 'AUTH_REQUIRED'
            ? 'AUTH_REQUIRED'
            : 'UNAVAILABLE';

  // FE-0040 (§56): the REAL action machine replaces the null stub that
  // rendered "PENDING_RUNTIME_ACK" forever — a false runtime claim. Steady
  // state is IDLE (honest: no mutation in flight); the machine arms
  // useRuntimeAckSocket automatically when a future mutation parks it in
  // WAITING_RUNTIME_ACK (the standard trio, wired).
  const action = useActionState();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  void selectedId;

  // AUTH_REQUIRED panel — shown when SSR fetch returned 401/403 and the
  // client has not yet acquired a session. The real signin happens at
  // /admin/signin where the operator submits the token to the httpOnly cookie.
  if (initialSnapshot.status === 'AUTH_REQUIRED' && !hasData) {
    return (
      <div className="space-y-6">
        <div className="flex justify-end">
          <AdminSessionBadge />
        </div>
        <Card>
          <CardHeader>
            <CardTitle>
              {REGISTRY_LABELS[registryKey]}
              <Badge variant="outline" className="ml-2" data-testid={`${registryKey}-status`}>
                AUTH_REQUIRED
              </Badge>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground mb-4">
              Admin session required — sign in to view registry.
            </p>
            <Link href="/admin/signin">
              <Button variant="default" data-testid={`${registryKey}-signin-link`}>
                Sign in
              </Button>
            </Link>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex justify-end">
        <AdminSessionBadge />
      </div>

      {initialSnapshot.status === 'UNAVAILABLE' && (
        <Card>
          <CardHeader>
            <CardTitle>
              Registry unavailable
              <Badge variant="outline" className="ml-2">UNAVAILABLE</Badge>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              {initialSnapshot.reason ?? 'Upstream registry not wired or unreachable.'}
            </p>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>
              {REGISTRY_LABELS[registryKey]}
              <Badge variant="outline" className="ml-2" data-testid={`${registryKey}-status`}>
                {status}
              </Badge>
            </CardTitle>
            <div className="flex gap-2">
              <OperatorGate minRole="steward" registry={registryKey}>
                <Button
                  variant="default"
                  data-testid={`${registryKey}-create-btn`}
                  onClick={() => setSelectedId('__new__')}
                >
                  Agregar
                </Button>
              </OperatorGate>
              <OperatorGate minRole="steward" registry={registryKey}>
                <Button
                  variant="outline"
                  data-testid={`${registryKey}-reload-btn`}
                  onClick={() => void refresh()}
                >
                  Hot-reload
                </Button>
              </OperatorGate>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="text-sm text-muted-foreground mb-4">
            Runtime ack:{' '}
            <span
              data-testid={`${registryKey}-runtime-ack`}
              title={
                action.state.kind === 'IDLE'
                  ? 'useActionState: sin mutación en vuelo (§56)'
                  : 'useActionState: máquina de acción crítica en curso'
              }
            >
              {action.state.kind}
              {action.state.kind === 'WAITING_RUNTIME_ACK' &&
                ` — socket runtime_ack en vivo (${action.state.event_id})`}
            </span>
          </div>

          {error && (
            <div className="text-sm text-destructive mb-4" data-testid={`${registryKey}-error`}>
              {error}
            </div>
          )}

          <Table data-testid={`${registryKey}-list`}>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Estado</TableHead>
                <TableHead>Config Hash</TableHead>
                <TableHead className="text-right">Acciones</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {loading && !hasData && (
                <TableRow>
                  <TableCell colSpan={4} className="text-center text-muted-foreground">
                    Cargando…
                  </TableCell>
                </TableRow>
              )}
              {!loading && rows.length === 0 && (
                <TableRow>
                  <TableCell colSpan={4} className="text-center text-muted-foreground">
                    UNAVAILABLE — sin registros
                  </TableCell>
                </TableRow>
              )}
              {rows.map((row) => (
                <TableRow key={row.id} data-testid={`${registryKey}-row-${row.id}`}>
                  <TableCell className="font-mono text-xs">{row.id}</TableCell>
                  <TableCell>
                    <Badge variant={row.enabled ? 'default' : 'secondary'}>
                      {row.enabled ? 'ENABLED' : 'DISABLED'}
                    </Badge>
                  </TableCell>
                  <TableCell className="font-mono text-xs">{row.config_hash}</TableCell>
                  <TableCell className="text-right space-x-2">
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setSelectedId(row.id)}
                    >
                      Ver
                    </Button>
                    <OperatorGate minRole="steward" registry={registryKey}>
                      <Button
                        size="sm"
                        variant="outline"
                        data-testid={`${registryKey}-edit-btn`}
                        onClick={() => setSelectedId(row.id)}
                      >
                        Editar
                      </Button>
                    </OperatorGate>
                    <OperatorGate minRole="steward" registry={registryKey}>
                      <Button
                        size="sm"
                        variant="destructive"
                        data-testid={`${registryKey}-disable-btn`}
                      >
                        Deshabilitar
                      </Button>
                    </OperatorGate>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Audit trail</CardTitle>
        </CardHeader>
        <CardContent data-testid={`${registryKey}-audit-trail`}>
          <p className="text-sm text-muted-foreground">
            Última acción registrada con operator_id, operator_pubkey, operator_role,
            config_hash_before y config_hash_after.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Coherencia total — Drift</CardTitle>
        </CardHeader>
        <CardContent data-testid={`${registryKey}-drift-panel`}>
          <RegistryCoherenceStrip
            resource={registryKey}
            observations={observations}
            pollError={drift.error}
            reason={drift.reason}
            loading={drift.loading}
            frontendRows={
              hasData || initialSnapshot.status === 'OK' ? rows.length : null
            }
            frontendRefreshedAt={
              drift.refreshedAt !== new Date(0).toISOString() ? drift.refreshedAt : null
            }
          />
        </CardContent>
      </Card>

      <div
        data-feature={`registry:${registryKey}`}
        className="sr-only"
        aria-hidden="true"
      />
    </div>
  );
}
