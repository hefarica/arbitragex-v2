/**
 * Persistence layer for service_credentials (migration 057 + 120).
 *
 * Responsibilities:
 *   - CRUD with the secret never returned raw in list/get; only the masked
 *     last-4 suffix is exposed via the public projection.
 *   - Status tracking: every upsert/test result updates status +
 *     last_validated_at + last_validation_error atomically.
 *   - Audit-friendly: created_by / updated_by populated from the actor
 *     header.
 *
 * WO-03 (2026-09-06): at-rest ENVELOPE encryption via pgcrypto. The secret
 * lives in secret_ciphertext (pgp_sym_encrypt, AES-256); secret_value stays
 * as the legacy/plaintext source for rows not yet converted (and for the
 * keyless legacy mode). The master key is EXTERNAL to the DB — only the
 * per-row derived key crosses the wire (see ./crypto.ts and migration 120).
 */

import type { Pool } from "pg";
import type {
  CredentialProvider,
  CredentialRowPublic,
  CredentialStatus,
} from "@arbx/shared";
// WO-03 (2026-09-06)
import {
  CredentialsDecryptError,
  decryptSecret,
  encryptSecret,
  maskHint,
  resolveMasterKeys,
  type SecretEnvelope,
} from "./crypto.js";

interface DbRow {
  id: string;
  provider: string;
  scope: string;
  display_name: string;
  secret_value: string | null;
  // WO-03 (2026-09-06) — envelope columns (migration 120)
  secret_ciphertext: Buffer | null;
  secret_salt: Buffer | null;
  secret_key_version: number | null;
  secret_hint: string | null;
  status: string;
  last_validated_at: Date | null;
  last_validation_error: string | null;
  metadata: Record<string, unknown>;
  updated_at: Date;
  updated_by: string | null;
}

function rowToPublic(r: DbRow): CredentialRowPublic {
  const hasPlaintext = !!r.secret_value && r.secret_value.length > 0;
  const hasEnvelope = !!r.secret_ciphertext && r.secret_ciphertext.length > 0;
  // WO-03 (2026-09-06): envelope rows carry a precomputed public hint; the
  // list path NEVER decrypts (design D8).
  const valueSuffix = r.secret_hint ?? (hasPlaintext ? maskHint(r.secret_value!) : "");
  return {
    id: r.id,
    provider: r.provider as CredentialProvider,
    scope: r.scope,
    display_name: r.display_name,
    has_value: hasPlaintext || hasEnvelope,
    value_suffix: valueSuffix,
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
 * WO-03 (2026-09-06): never decrypts — suffix comes from secret_hint.
 */
export async function listCredentials(pool: Pool): Promise<CredentialRowPublic[]> {
  const q = await pool.query<DbRow>(
    `SELECT id, provider, scope, display_name, secret_value,
            secret_ciphertext, secret_salt, secret_key_version, secret_hint,
            status, last_validated_at, last_validation_error, metadata, updated_at, updated_by
       FROM service_credentials
      ORDER BY provider ASC, scope ASC`,
  );
  return q.rows.map(rowToPublic);
}

/**
 * WO-03 (2026-09-06) — resolve the effective secret for a fetched row:
 * envelope (decrypt, fail-fast without master key) else legacy plaintext.
 */
async function secretFromRow(
  pool: Pool,
  row: { secret_value: string | null; secret_ciphertext: Buffer | null; secret_salt: Buffer | null; secret_key_version: number | null },
  context: string,
): Promise<string | null> {
  if (row.secret_ciphertext) {
    if (!row.secret_salt || row.secret_key_version === null) {
      throw new CredentialsDecryptError(
        `incomplete envelope (missing salt/version) for ${context}`,
      );
    }
    return decryptSecret(
      pool,
      { ciphertext: row.secret_ciphertext, salt: row.secret_salt, keyVersion: row.secret_key_version },
      resolveMasterKeys(),
      context,
    );
  }
  if (!row.secret_value) return null;
  return row.secret_value;
}

/**
 * Read a single credential's RAW secret + metadata. Used internally by
 * the test endpoint and by future runtime consumers (env-projector). Caller
 * MUST never return this from a public route.
 * WO-03 (2026-09-06): decrypts the envelope when present (fail-fast via
 * CredentialsKeyRequiredError when the master key is not configured).
 */
export async function readCredentialSecret(
  pool: Pool,
  provider: CredentialProvider,
  scope: string,
): Promise<{ secret: string; metadata: Record<string, unknown> } | null> {
  const q = await pool.query<{
    secret_value: string | null;
    secret_ciphertext: Buffer | null;
    secret_salt: Buffer | null;
    secret_key_version: number | null;
    metadata: Record<string, unknown>;
  }>(
    `SELECT secret_value, secret_ciphertext, secret_salt, secret_key_version, metadata
       FROM service_credentials
      WHERE provider = $1 AND scope = $2`,
    [provider, scope],
  );
  const r = q.rows[0];
  if (!r) return null;
  const secret = await secretFromRow(pool, r, `${provider}/${scope}`);
  if (!secret) return null;
  return { secret, metadata: r.metadata ?? {} };
}

export interface UpsertInput {
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
 * RunFullSyncCycle FASE 1 — read the stored row WITHOUT masking for bulk
 * noop-detection (secret unchanged + metadata unchanged ⇒ skip the write so
 * updated_at does not rotate on idempotent re-runs). Server-internal only;
 * callers MUST never return the raw secret.
 */
export interface StoredCredentialRow {
  secret: string | null;
  metadata: Record<string, unknown>;
  status: CredentialStatus;
  display_name: string;
  updated_at: string;
}

export async function readCredentialForBulk(
  pool: Pool,
  provider: CredentialProvider,
  scope: string,
): Promise<StoredCredentialRow | null> {
  const q = await pool.query<{
    secret_value: string | null;
    secret_ciphertext: Buffer | null;
    secret_salt: Buffer | null;
    secret_key_version: number | null;
    metadata: Record<string, unknown> | null;
    status: string;
    display_name: string;
    updated_at: Date;
  }>(
    `SELECT secret_value, secret_ciphertext, secret_salt, secret_key_version,
            metadata, status, display_name, updated_at
       FROM service_credentials
      WHERE provider = $1 AND scope = $2`,
    [provider, scope],
  );
  const r = q.rows[0];
  if (!r) return null;
  // WO-03 (2026-09-06): decrypt-aware — the StoredCredentialRow contract the
  // routes rely on (noop byte-compare + Redis projection) is unchanged.
  const secret = await secretFromRow(pool, r, `${provider}/${scope}`);
  return {
    secret,
    metadata: r.metadata ?? {},
    status: r.status as CredentialStatus,
    display_name: r.display_name,
    updated_at: r.updated_at.toISOString(),
  };
}

/**
 * Upsert a credential. Caller is expected to have already run the validator
 * and to pass `status` + `validation_error` accordingly so this write is
 * atomic with the test result.
 *
 * WO-03 (2026-09-06): with a configured master key, a new secret is written
 * as a pgcrypto envelope (secret_value NULL). Without a key the write is
 * legacy plaintext — identical to pre-WO-03 behavior (honest degradation;
 * the boot sweep converts legacy rows once a key is provisioned). A
 * metadata-only refresh (secret_value=null, no envelope) keeps whichever
 * source the row already carries.
 */
export async function upsertCredential(pool: Pool, input: UpsertInput): Promise<CredentialRowPublic> {
  const displayName = input.display_name ?? `${input.provider} (${input.scope})`;
  const keys = resolveMasterKeys();
  let envelope: SecretEnvelope | null = null;
  if (input.secret_value && input.secret_value.length > 0 && keys) {
    envelope = await encryptSecret(pool, input.secret_value, keys);
  }
  const q = await pool.query<DbRow>(
    `INSERT INTO service_credentials
        (provider, scope, display_name, secret_value, status,
         last_validated_at, last_validation_error, metadata, created_by, updated_by,
         secret_ciphertext, secret_salt, secret_key_version, secret_hint)
     VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7::jsonb, $8, $8, $9, $10, $11, $12)
     ON CONFLICT (provider, scope) DO UPDATE
        SET display_name           = EXCLUDED.display_name,
            secret_value           = CASE
                                      WHEN EXCLUDED.secret_value IS NOT NULL THEN EXCLUDED.secret_value
                                      WHEN EXCLUDED.secret_ciphertext IS NOT NULL THEN NULL
                                      ELSE service_credentials.secret_value
                                    END,
            secret_ciphertext      = CASE
                                      WHEN EXCLUDED.secret_value IS NOT NULL THEN NULL
                                      ELSE COALESCE(EXCLUDED.secret_ciphertext, service_credentials.secret_ciphertext)
                                    END,
            secret_salt            = CASE
                                      WHEN EXCLUDED.secret_value IS NOT NULL THEN NULL
                                      ELSE COALESCE(EXCLUDED.secret_salt, service_credentials.secret_salt)
                                    END,
            secret_key_version     = CASE
                                      WHEN EXCLUDED.secret_value IS NOT NULL THEN NULL
                                      ELSE COALESCE(EXCLUDED.secret_key_version, service_credentials.secret_key_version)
                                    END,
            secret_hint            = CASE
                                      WHEN EXCLUDED.secret_value IS NOT NULL THEN NULL
                                      ELSE COALESCE(EXCLUDED.secret_hint, service_credentials.secret_hint)
                                    END,
            status                 = EXCLUDED.status,
            last_validated_at      = NOW(),
            last_validation_error  = EXCLUDED.last_validation_error,
            metadata               = EXCLUDED.metadata,
            updated_by             = EXCLUDED.updated_by
     RETURNING id, provider, scope, display_name, secret_value,
               secret_ciphertext, secret_salt, secret_key_version, secret_hint,
               status, last_validated_at, last_validation_error, metadata, updated_at, updated_by`,
    [
      input.provider,
      input.scope,
      displayName,
      envelope ? null : input.secret_value,
      input.status,
      input.validation_error,
      JSON.stringify(input.metadata),
      input.actor,
      envelope?.ciphertext ?? null,
      envelope?.salt ?? null,
      envelope?.keyVersion ?? null,
      envelope?.hint ?? null,
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
