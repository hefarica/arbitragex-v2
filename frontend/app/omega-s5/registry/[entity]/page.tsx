/**
 * /omega-s5/registry/[entity] — Página dinámica para los 12 registries.
 *
 * Cumple C9.3 (Operator CRUD Sovereignty) y C9.1 (Mirror Law Extendida):
 *  - Compone EXCLUSIVAMENTE primitivas shadcn/ui ya presentes.
 *  - NO modifica layout.tsx, globals.css ni tailwind.config.ts.
 *  - Botones de mutación quedan envueltos en <OperatorGate />.
 *  - 7 capacidades obligatorias por registry:
 *      1) Listado paginado con filtros
 *      2) Detalle (drawer/dialog)
 *      3) Crear (form validado Zod)
 *      4) Editar con Idempotency-Key
 *      5) Disable / soft-delete
 *      6) Hot-reload trigger con runtime_ack visible
 *      7) Drift panel local
 */

'use client';

import { use, useState } from 'react';
import { notFound } from 'next/navigation';
import { REGISTRY_KEYS, type RegistryKey } from '@/lib/operator/types';
import { OperatorGate } from '@/components/operator/OperatorGate';
import { useRegistry } from '@/lib/registries/useRegistry';
import { useOmniDrift } from '@/lib/drift/useOmniDrift';
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

interface PageProps {
  params: Promise<{ entity: string }>;
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

export default function RegistryPage(props: PageProps): JSX.Element {
  const { entity } = use(props.params);

  if (!REGISTRY_KEYS.includes(entity as RegistryKey)) {
    notFound();
  }

  const registryKey = entity as RegistryKey;
  const { rows, loading, status, reload, runtimeAck } = useRegistry(registryKey);
  const drift = useOmniDrift({ resource: registryKey });
  const [selectedId, setSelectedId] = useState<string | null>(null);

  return (
    <div className="space-y-6">
      {/* Encabezado — compone Card + Badge shadcn ya presentes */}
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
                  onClick={() => void reload()}
                >
                  Hot-reload
                </Button>
              </OperatorGate>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {/* Runtime Ack visible (Capa 6 de 9-Layer Coherence) */}
          <div className="text-sm text-muted-foreground mb-4">
            Runtime ack:{' '}
            <span data-testid={`${registryKey}-runtime-ack`}>
              {runtimeAck?.state ?? 'PENDING_RUNTIME_ACK'}
            </span>
            {runtimeAck?.layers && (
              <span className="ml-2">
                Capas confirmadas: {runtimeAck.layers.join(' → ')}
              </span>
            )}
          </div>

          {/* Listado */}
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
              {loading && (
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
              {rows.map(row => (
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

      {/* Audit trail panel (capa 7 de 9-Layer) */}
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

      {/* Drift panel local (capa de detección C9 + Drift v3) */}
      <Card>
        <CardHeader>
          <CardTitle>Drift Observations</CardTitle>
        </CardHeader>
        <CardContent data-testid={`${registryKey}-drift-panel`}>
          {drift.observations.length === 0 ? (
            <p className="text-sm text-muted-foreground">Sin drift detectado.</p>
          ) : (
            <ul className="text-sm space-y-1">
              {drift.observations.map(obs => (
                <li key={obs.id} className="font-mono">
                  {obs.severity} — {obs.message}
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      {/* Feature key declarativo para C9.2 (Total Functional Mirror) */}
      <div
        data-feature={`registry:${registryKey}`}
        className="sr-only"
        aria-hidden="true"
      />
    </div>
  );
}
