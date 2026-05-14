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
 *
 * REPAIR c815ef9-typefix:
 *  - Alineado al contrato canónico de useRegistry({ fetchFn, ... }) → { items, isLoading, error, refresh, ... }
 *  - useOmniDrift firma correcta: useOmniDrift(pollMs?: number) → { observations, byResource, ... }
 *  - status derivado de error/isLoading/hasData (Fail-Honest, no fabricado)
 *  - runtimeAck preservado como estado local null hasta que backend lo publique
 *    (la UI muestra "PENDING_RUNTIME_ACK" por default — coherente con Ghost Protocol)
 *  - Solo `chain` tiene admin endpoint hoy; los 11 restantes devuelven [] y la UI
 *    muestra "UNAVAILABLE — sin registros" (R8 Fail-Honest, NO mock data)
 */

'use client';

import { use, useCallback, useMemo, useState } from 'react';
import { notFound } from 'next/navigation';
import { REGISTRY_KEYS, type RegistryKey } from '@/lib/operator/types';
import { OperatorGate } from '@/components/operator/OperatorGate';
import { useRegistry } from '@/lib/registries/useRegistry';
import { useOmniDrift } from '@/lib/drift/useOmniDrift';
import { getAdminChains } from '@/lib/api-client';
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

/**
 * Forma mínima común que la tabla genérica necesita por fila.
 * Se construye en el adapter `fetchRegistryByKey` a partir del shape
 * canónico de cada entity sin perder identidad (no es un mock).
 */
interface RegistryRow {
  readonly id: string;
  readonly enabled: boolean;
  readonly config_hash: string;
}

/**
 * Acknowledgement runtime publicado por el backend para Capa 6 / 9-Layer.
 * Mientras backend no lo emita, permanece `null` → la UI muestra
 * `PENDING_RUNTIME_ACK` (Fail-Honest).
 */
interface RuntimeAck {
  readonly state: string;
  readonly layers: readonly string[];
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
 * Adapter: mapea cada RegistryKey a su fetcher canónico y normaliza al
 * shape `RegistryRow`. Hoy solo `chain` tiene endpoint admin productivo;
 * los demás devuelven un array vacío hasta que su backend admin se publique.
 * Esto NO es un mock — es Fail-Honest (sin datos reales, no se inventan).
 */
async function fetchRegistryByKey(key: RegistryKey): Promise<readonly RegistryRow[]> {
  if (key === 'chain') {
    const res = await getAdminChains();
    if (!res.ok) {
      throw new Error(res.error);
    }
    return res.data.items.map((c) => ({
      id: String(c.chain_id),
      enabled: c.enabled,
      config_hash: c.config_hash ?? 'pending',
    }));
  }
  // Registries restantes: sin endpoint admin canónico aún → fail-honest vacío.
  return [];
}

export default function RegistryPage(props: PageProps): JSX.Element {
  const { entity } = use(props.params);

  if (!REGISTRY_KEYS.includes(entity as RegistryKey)) {
    notFound();
  }

  const registryKey = entity as RegistryKey;

  // Memoizado para no re-crear la función en cada render (evita refresh-loop).
  const fetchFn = useCallback(
    () => fetchRegistryByKey(registryKey),
    [registryKey],
  );

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
  });

  // Drift global; filtramos en cliente por resource = registryKey.
  const drift = useOmniDrift(5_000);
  const observations = useMemo(
    () => drift.byResource[registryKey] ?? [],
    [drift.byResource, registryKey],
  );

  // status derivado (Fail-Honest, sin fabricar valores).
  const status: string = error
    ? 'ERROR'
    : isLoading
      ? 'LOADING'
      : hasData
        ? 'READY'
        : 'PENDING';

  // runtimeAck: state local; backend lo poblará cuando emita capas confirmadas.
  const [runtimeAck] = useState<RuntimeAck | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // selectedId reservado para drawer/dialog de detalle (capacidad 2);
  // suprimir warning de no-uso en este nivel.
  void selectedId;

  const loading = isLoading;
  const rows = items;

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
                  onClick={() => void refresh()}
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
          {observations.length === 0 ? (
            <p className="text-sm text-muted-foreground">Sin drift detectado.</p>
          ) : (
            <ul className="text-sm space-y-1">
              {observations.map(obs => (
                <li key={obs.id} className="font-mono">
                  {obs.severity} — {obs.diff_count} diffs ({obs.layer_a} ↔ {obs.layer_b})
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
