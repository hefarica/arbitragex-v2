/**
 * Ω-S5++ C9.∞ APEX · Registry Factory polimórfica
 * -------------------------------------------------------------------------
 * 12 registries × 7 capacidades (C9.3 Operator CRUD Sovereignty).
 *
 * Capacidades:  list, create, update, delete, validate, readiness, audit.
 *
 * REGLA: ningún componente debe hardcodear `chain_id`, `dex_family`, etc.
 * Todo se consulta vía registries. Esto preserva INV-5 Spatial Isolation:
 * mutar registry X no debe leakear a registry Y.
 */
import { z } from 'zod';
import type { TypedApiClient } from '../api/client';
import type { ApiResult } from '../api/client';
import {
  MutationResponseSchema,
  type IdempotencyKey,
  type MutationResponse,
} from '../schemas';

// ─── Capacidad: cada registry expone esto ───────────────────────────────
export interface RegistryCapability<TEntity, TCreate, TUpdate> {
  readonly name: RegistryName;
  readonly basePath: string;
  readonly entitySchema: z.ZodSchema<TEntity>;
  readonly listResponseSchema: z.ZodSchema<{ items: ReadonlyArray<TEntity>; sequence_id: number }>;
  readonly createSchema: z.ZodSchema<TCreate>;
  readonly updateSchema: z.ZodSchema<TUpdate>;
}

// ─── 12 nombres canónicos de registries ─────────────────────────────────
export const REGISTRY_NAMES = [
  'chain',
  'dex',
  'pool',
  'token',
  'wallet',
  'rpc',
  'strategy',
  'risk_gate',
  'capital_gate',
  'circuit_breaker',
  'relay',
  'agent_team',
] as const;
export type RegistryName = (typeof REGISTRY_NAMES)[number];

// ─── 7 capacidades canónicas ────────────────────────────────────────────
export const REGISTRY_CAPABILITIES = [
  'list',
  'create',
  'update',
  'delete',
  'validate',
  'readiness',
  'audit',
] as const;
export type RegistryCapabilityName = (typeof REGISTRY_CAPABILITIES)[number];

// ─── Factory: construye un cliente tipado para un registry específico ───
export function buildRegistryClient<TEntity, TCreate, TUpdate>(
  api: TypedApiClient,
  cap: RegistryCapability<TEntity, TCreate, TUpdate>,
) {
  return {
    list: (): Promise<ApiResult<{ items: ReadonlyArray<TEntity>; sequence_id: number }>> =>
      api.get(cap.basePath, cap.listResponseSchema),

    create: (
      body: TCreate,
      idempotencyKey: IdempotencyKey,
    ): Promise<ApiResult<MutationResponse>> =>
      api.post(cap.basePath, cap.createSchema, MutationResponseSchema, body, idempotencyKey),

    update: (
      id: string,
      body: TUpdate,
      idempotencyKey: IdempotencyKey,
    ): Promise<ApiResult<MutationResponse>> =>
      api.put(
        `${cap.basePath}/${encodeURIComponent(id)}`,
        cap.updateSchema,
        MutationResponseSchema,
        body,
        idempotencyKey,
      ),

    deleteById: (
      id: string,
      idempotencyKey: IdempotencyKey,
    ): Promise<ApiResult<MutationResponse>> =>
      api.delete(
        `${encodeURIComponent(cap.basePath)}/${encodeURIComponent(id)}`,
        MutationResponseSchema,
        idempotencyKey,
      ),
  } as const;
}

export type RegistryClient<TEntity, TCreate, TUpdate> = ReturnType<
  typeof buildRegistryClient<TEntity, TCreate, TUpdate>
>;
