/**
 * Persistence layer for service_credentials (migration 057).
 *
 * Responsibilities:
 *   - CRUD with secret_value never returned raw in list/get; only last-4
 *     suffix is exposed via the public projection.
 *   - Status tracking: every upsert/test result updates status +
 *     last_validated_at + last_validation_error atomically.
 *   - Audit-friendly: created_by / updated_by populated from the actor
 *     header.
 *
 * The secret_value is stored as plain TEXT for the MVP. Migration 058 will
 * add at-rest encryption via pgcrypto once the Vault transit key is
 * provisioned — see docs/operations/credentials-encryption.md (todo).
 */

import type { Pool } from "pg";
import type {
  CredentialProvider,
  CredentialRowPublic,
  CredentialStatus,
} from "@arbx/shared";

function maskSuffix(secret: string | null): string {
  if (!secret) return "";
  const t = secret.trim();
  return t.length <= 4 ? "****" : `…${t.slice(-4)}`;
}

interface DbRow {
  id: string;
  provider: string;
  scope: string;
  display_name: string;
  secret_value: string | null;
  status: string;
  last_validated_at: Date | null;
  last_validation_error: string | null;
  metadata: Record<string, unknown>;
  updated_at: Date;
  updated_by: string | null;
}

function rowToPublic(r: DbRow): CredentialRowPublic {
  return {
    id: r.id,
    provider: r.provider as CredentialProvider,
    scope: r.scope,
    display_name: r.display_name,
    has_value: !!r.secret_value && r.secret_value.length > 0,
    value_suffix: maskSuffix(r.secret_value),
    status: r.status as CredentialStatus,
    last_validated_at: r.last_validated_at ? r.last_validated_at.toISOString() : null,
    last_validation_error: r.last_validation_error,
    metadata: r.metadata ?? {},
    updated_at: r.updated_at.toISOString(),
    updated_by: r.updated_by,
  };
}

/**
 * List ALL credentials with masked secrets. Operator-facing endpoint.
 */
export async function listCredentials(pool: Pool): Promise<CredentialRowPublic[]> {
  const q = await pool.query<DbRow>(
    `SELECT id, provider, scope, display_name, secret_value, status,
            last_validated_at, last_validation_error, metadata, updated_at, updated_by
       FROM service_credentials
      ORDER BY provider ASC, scope ASC`,
  );
  return q.rows.map(rowToPublic);
}

/**
 * Read a single credential's RAW secret_value + metadata. Used internally by
 * the test endpoint and by future runtime consumers (env-projector). Caller
 * MUST never return this from a public route.
 */
export async function readCredentialSecret(
  pool: Pool,
  provider: CredentialProvider,
  scope: string,
): Promise<{ secret: string; metadata: Record<string, unknown> } | null> {
  const q = await pool.query<{ secret_value: string | null; metadata: Record<string, unknown> }>(
    `SELECT secret_value, metadata
       FROM service_credentials
      WHERE provider = $1 AND scope = $2`,
    [provider, scope],
  );
  const r = q.rows[0];
  if (!r || !r.secret_value) return null;
  return { secret: r.secret_value, metadata: r.metadata ?? {} };
}

interface UpsertInput {
  provider: CredentialProvider;
  scope: string;
  display_name?: string | undefined;
  secret_value: string | null;
  metadata: Record<string, unknown>;
  status: CredentialStatus;
  validation_error: string | null;
  actor: string;
}

/**
 * Upsert a credential. Caller is expected to have already run the validator
 * and to pass `status` + `validation_error` accordingly so this write is
 * atomic with the test result.
 */
export async function upsertCredential(pool: Pool, input: UpsertInput): Promise<CredentialRowPublic> {
  const displayName = input.display_name ?? `${input.provider} (${input.scope})`;
  const q = await pool.query<DbRow>(
    `INSERT INTO service_credentials
        (provider, scope, display_name, secret_value, status,
         last_validated_at, last_validation_error, metadata, created_by, updated_by)
     VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7::jsonb, $8, $8)
     ON CONFLICT (provider, scope) DO UPDATE
        SET display_name           = EXCLUDED.display_name,
            secret_value           = COALESCE(EXCLUDED.secret_value, service_credentials.secret_value),
            status                 = EXCLUDED.status,
            last_validated_at      = NOW(),
            last_validation_error  = EXCLUDED.last_validation_error,
            metadata               = EXCLUDED.metadata,
            updated_by             = EXCLUDED.updated_by
     RETURNING id, provider, scope, display_name, secret_value, status,
               last_validated_at, last_validation_error, metadata, updated_at, updated_by`,
    [
      input.provider,
      input.scope,
      displayName,
      input.secret_value,
      input.status,
      input.validation_error,
      JSON.stringify(input.metadata),
      input.actor,
    ],
  );
  return rowToPublic(q.rows[0]!);
}

export async function deleteCredential(
  pool: Pool,
  provider: CredentialProvider,
  scope: string,
): Promise<boolean> {
  const r = await pool.query(
    `DELETE FROM service_credentials WHERE provider = $1 AND scope = $2`,
    [provider, scope],
  );
  return (r.rowCount ?? 0) > 0;
}
