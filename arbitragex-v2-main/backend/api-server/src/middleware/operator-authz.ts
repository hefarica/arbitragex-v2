/**
 * Middleware Operator Authz (L8) + Audit (L9)
 * --------------------------------------------
 * Capa 8: Valida pubkey + role + allowed_chains/registries antes del Handler.
 * Capa 9: Registra operator_id, operator_pubkey, operator_role en audit_event.
 *
 * Doctrina C9.4 / C9.5:
 *   - observer  → solo lectura del manifest + observabilidad.
 *   - steward   → CRUD dentro de allowed_registries + allowed_chains.
 *   - sovereign → escalación de capital, promoción mainnet, feature_manifest.
 *
 * Lexicón Absoluto: NO usa mock/stub/dummy/placeholder/TODO/FIXME.
 */

import type { Request, Response, NextFunction } from 'express';
import type { Pool } from 'pg';

export type OperatorRole = 'observer' | 'steward' | 'sovereign';

export interface OperatorIdentity {
  operatorId: string;
  displayName: string;
  signingPubkey: string;
  role: OperatorRole;
  capUsdCeiling: string;          // NUMERIC serializado como string
  allowedChains: number[];
  allowedRegistries: string[];
  uiPreferences: Record<string, unknown>;
  featureOverrides: Record<string, unknown>;
  configHash: string;
  enabled: boolean;
}

declare global {
  namespace Express {
    interface Request {
      operator?: OperatorIdentity;
    }
  }
}

export interface OperatorResolverConfig {
  pool: Pool;
  /**
   * Extrae el pubkey desde el header firmado (HMAC, JWT, etc).
   * En producción debe verificar la firma; aquí se delega al integrador.
   */
  resolvePubkeyFromRequest: (req: Request) => string | null;
}

/**
 * Carga el operador a partir del pubkey firmado en cada request.
 * Si no hay pubkey o no existe el operador, devuelve 401 Unauthorized.
 * Si el operador está deshabilitado, devuelve 403 Forbidden.
 */
export function operatorIdentityMiddleware(cfg: OperatorResolverConfig) {
  return async function (req: Request, res: Response, next: NextFunction): Promise<void> {
    const pubkey = cfg.resolvePubkeyFromRequest(req);
    if (!pubkey) {
      res.status(401).json({
        status: 'BLOCKED',
        reason: 'OPERATOR_UNAUTHENTICATED',
        layer: 'L8_AUTHZ',
      });
      return;
    }

    const result = await cfg.pool.query(
      `SELECT operator_id, display_name, signing_pubkey, role,
              cap_usd_ceiling::text AS cap_usd_ceiling,
              allowed_chains, allowed_registries, ui_preferences,
              feature_overrides, config_hash, enabled
       FROM operator_parametrization
       WHERE signing_pubkey = $1`,
      [pubkey]
    );

    if (result.rowCount === 0) {
      res.status(401).json({
        status: 'BLOCKED',
        reason: 'OPERATOR_NOT_REGISTERED',
        layer: 'L8_AUTHZ',
      });
      return;
    }

    const row = result.rows[0];
    if (row.enabled === false) {
      res.status(403).json({
        status: 'BLOCKED',
        reason: 'OPERATOR_DISABLED',
        layer: 'L8_AUTHZ',
      });
      return;
    }

    req.operator = {
      operatorId: row.operator_id,
      displayName: row.display_name,
      signingPubkey: row.signing_pubkey,
      role: row.role as OperatorRole,
      capUsdCeiling: row.cap_usd_ceiling,
      allowedChains: row.allowed_chains as number[],
      allowedRegistries: row.allowed_registries as string[],
      uiPreferences: row.ui_preferences as Record<string, unknown>,
      featureOverrides: row.feature_overrides as Record<string, unknown>,
      configHash: row.config_hash,
      enabled: row.enabled,
    };

    next();
  };
}

/**
 * Exige un rol mínimo. Jerarquía: observer < steward < sovereign.
 * Opcionalmente exige que registry/chain esté en allowed_* del operador.
 */
export function requireOperatorRole(
  minRole: OperatorRole,
  opts?: { registry?: string; chainId?: number }
) {
  const hierarchy: Record<OperatorRole, number> = {
    observer: 1,
    steward: 2,
    sovereign: 3,
  };

  return function (req: Request, res: Response, next: NextFunction): void {
    const op = req.operator;
    if (!op) {
      res.status(401).json({
        status: 'BLOCKED',
        reason: 'OPERATOR_MISSING_IDENTITY',
        layer: 'L8_AUTHZ',
      });
      return;
    }

    if (hierarchy[op.role] < hierarchy[minRole]) {
      res.status(403).json({
        status: 'BLOCKED',
        reason: 'OPERATOR_ROLE_INSUFFICIENT',
        required: minRole,
        actual: op.role,
        layer: 'L8_AUTHZ',
      });
      return;
    }

    if (opts?.registry && op.role !== 'sovereign') {
      if (!op.allowedRegistries.includes(opts.registry)) {
        res.status(403).json({
          status: 'BLOCKED',
          reason: 'OPERATOR_REGISTRY_NOT_ALLOWED',
          registry: opts.registry,
          layer: 'L8_AUTHZ',
        });
        return;
      }
    }

    if (opts?.chainId !== undefined && op.role !== 'sovereign') {
      if (op.allowedChains.length > 0 && !op.allowedChains.includes(opts.chainId)) {
        res.status(403).json({
          status: 'BLOCKED',
          reason: 'OPERATOR_CHAIN_NOT_ALLOWED',
          chain_id: opts.chainId,
          layer: 'L8_AUTHZ',
        });
        return;
      }
    }

    next();
  };
}

/**
 * Helper de auditoría L9: agrega operator_id / pubkey / role a cada audit_event.
 */
export function buildOperatorAuditPayload(req: Request): {
  operator_id: string | null;
  operator_pubkey: string | null;
  operator_role: string | null;
} {
  const op = req.operator;
  return {
    operator_id: op?.operatorId ?? null,
    operator_pubkey: op?.signingPubkey ?? null,
    operator_role: op?.role ?? null,
  };
}
