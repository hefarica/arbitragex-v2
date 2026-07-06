/**
 * <OperatorGate /> — Render condicional declarativo (C9.4).
 *
 * Mirror Law Extendida (C9.1):
 *   - Compone EXCLUSIVAMENTE primitivas shadcn/ui pre-existentes.
 *   - NO introduce nuevos colores, fuentes, escalas o variables CSS.
 *   - NO instala dependencias UI nuevas.
 *
 * Uso:
 *   <OperatorGate minRole="sovereign">
 *     <Button>Escalar capital</Button>
 *   </OperatorGate>
 *
 *   <OperatorGate registry="capital_gate">
 *     <CapitalGateEditor />
 *   </OperatorGate>
 */

'use client';

import { ReactNode } from 'react';
import { useOperator } from '@/lib/operator/useOperator';
import type { OperatorRole, RegistryKey } from '@/lib/operator/types';

export interface OperatorGateProps {
  children: ReactNode;
  /**
   * Rol mínimo requerido. Si se omite, sólo se verifica registry/chain.
   */
  minRole?: OperatorRole;
  /**
   * Registry específico. Si se especifica y el operador no es sovereign,
   * debe estar en allowed_registries.
   */
  registry?: RegistryKey;
  /**
   * Chain específica. Si se especifica y el operador no es sovereign,
   * debe estar en allowed_chains.
   */
  chainId?: number;
  /**
   * Fallback opcional cuando el gate bloquea (default: no render).
   */
  fallback?: ReactNode;
  /**
   * Si true, muestra `fallback` cuando aún se está cargando la identidad.
   * Si false (default), muestra `null` durante loading.
   */
  showFallbackWhileLoading?: boolean;
}

export function OperatorGate({
  children,
  minRole,
  registry,
  chainId,
  fallback = null,
  showFallbackWhileLoading = false,
}: OperatorGateProps): JSX.Element | null {
  const op = useOperator();

  if (op.status === 'loading') {
    return showFallbackWhileLoading ? <>{fallback}</> : null;
  }

  if (op.status !== 'ready') {
    return <>{fallback}</>;
  }

  const roleOk = !minRole || op.hasMinRole(minRole);
  if (!roleOk) return <>{fallback}</>;

  const registryOk = !registry || op.canAccessRegistry(registry);
  if (!registryOk) return <>{fallback}</>;

  const chainOk =
    chainId === undefined ||
    op.data.operator.role === 'sovereign' ||
    op.data.operator.allowed_chains.length === 0 ||
    op.data.operator.allowed_chains.includes(chainId);
  if (!chainOk) return <>{fallback}</>;

  return <>{children}</>;
}
