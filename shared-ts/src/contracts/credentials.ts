import { z } from "zod";

/**
 * Operator-managed external credentials (`/settings/credentials` surface).
 *
 * Each provider type lists what the validator actually executes against. The
 * status is NEVER taken on faith — it transitions to "valid" only after the
 * api-server runs the provider-specific live test and it passes (R8 fail-honest).
 */

export const CredentialProvider = z.enum([
  // Hot-path RPC (per chain)
  "rpc_http",          // CSV of name=url providers consumed by HttpRpcPool
  "rpc_ws",            // CSV of name=wss-url providers consumed by scanner
  // Price oracles
  "coingecko_demo",    // Free tier; just /ping
  "coingecko_pro",     // X-Cg-Pro-Api-Key
  "alchemy_prices",    // tokens/by-address — KEY only, not URL
  // MEV submission
  "flashbots_signer",  // 0x-prefixed private key (32 bytes); test derives address
  "bloxroute",         // Authorization header value (raw, no "Bearer ")
  "titan",             // Builder URL + auth (metadata.url + secret_value=auth)
  // CEX (for CEX-DEX strategy)
  "binance",           // api_key in metadata.api_key, api_secret in secret_value
  "okx",               // api_key + api_secret + passphrase (metadata.passphrase)
  "bybit",             // api_key in metadata.api_key, api_secret in secret_value
  // External misc
  "github_token",      // for token-enricher TrustWallet rate limits
  // Internal auth (rotation surface, not new entry)
  "admin_token",       // ARBX_ADMIN_TOKEN — at-least-32 chars, no placeholders
  "edge_token",        // ARBX_EDGE_TOKEN — same constraints
]);
export type CredentialProvider = z.infer<typeof CredentialProvider>;

export const CredentialStatus = z.enum(["untested", "valid", "invalid", "expired"]);
export type CredentialStatus = z.infer<typeof CredentialStatus>;

/** Scope: "global" or "chain:<id>" (e.g. "chain:1"). */
export const CredentialScope = z.string().regex(/^(global|chain:\d+)$/);

/** Public row — secret_value redacted to last-4 suffix only. */
export const CredentialRowPublicSchema = z.object({
  id: z.string().uuid(),
  provider: CredentialProvider,
  scope: CredentialScope,
  display_name: z.string().min(1).max(200),
  has_value: z.boolean(),
  // Last 4 visible chars for operator recognition. Empty string when absent.
  value_suffix: z.string().max(8),
  status: CredentialStatus,
  last_validated_at: z.string().datetime().nullable(),
  last_validation_error: z.string().nullable(),
  metadata: z.record(z.unknown()),
  updated_at: z.string().datetime(),
  updated_by: z.string().nullable(),
});
export type CredentialRowPublic = z.infer<typeof CredentialRowPublicSchema>;

/** Upsert payload — operator sends new secret + optional metadata. */
export const CredentialUpsertSchema = z.object({
  provider: CredentialProvider,
  scope: CredentialScope,
  display_name: z.string().min(1).max(200).optional(),
  // null = clear; non-null = replace.
  secret_value: z.string().min(1).max(2048).nullable(),
  metadata: z.record(z.unknown()).default({}),
});
export type CredentialUpsert = z.infer<typeof CredentialUpsertSchema>;

/** Test-only payload — runs the validator without persisting. */
export const CredentialTestSchema = z.object({
  provider: CredentialProvider,
  scope: CredentialScope,
  secret_value: z.string().min(1).max(2048),
  metadata: z.record(z.unknown()).default({}),
});
export type CredentialTest = z.infer<typeof CredentialTestSchema>;

/** Result of a live validator run. */
export const CredentialTestResultSchema = z.object({
  status: CredentialStatus,
  message: z.string(),
  details: z.record(z.unknown()).optional(),
  tested_at: z.string().datetime(),
});
export type CredentialTestResult = z.infer<typeof CredentialTestResultSchema>;

/** GET /api/v1/credentials response wrapper. */
export const CredentialsListResponseSchema = z.object({
  items: z.array(CredentialRowPublicSchema),
  ts: z.string().datetime(),
});
export type CredentialsListResponse = z.infer<typeof CredentialsListResponseSchema>;

/**
 * RunFullSyncCycle FASE 1 — bulk upsert (operator macro).
 * One row per item; same validator + same upsert as the manual PUT. One bad
 * row never blocks the rest (fail-honest granular results).
 */
export const CredentialBulkItemSchema = z.object({
  provider: CredentialProvider,
  scope: CredentialScope,
  display_name: z.string().min(1).max(200).optional(),
  // Absent/null = keep the stored secret (metadata-only refresh).
  secret_value: z.string().min(1).max(2048).nullable().optional(),
  metadata: z.record(z.unknown()).default({}),
});
export type CredentialBulkItem = z.infer<typeof CredentialBulkItemSchema>;

export const CredentialBulkRequestSchema = z.object({
  items: z.array(CredentialBulkItemSchema).min(1).max(200),
  // dry_run=true validates everything and persists NOTHING (homologation pass).
  dry_run: z.boolean().default(false),
});
export type CredentialBulkRequest = z.infer<typeof CredentialBulkRequestSchema>;

export const CredentialBulkAction = z.enum(["updated", "noop", "validated", "invalid", "error"]);
export type CredentialBulkAction = z.infer<typeof CredentialBulkAction>;

export const CredentialBulkRowResultSchema = z.object({
  provider: z.string(),
  scope: z.string(),
  action: CredentialBulkAction,
  status: CredentialStatus.optional(),
  message: z.string().optional(),
  // Sanitized per-provider breakdown for rpc_* CSV arrays (name/ok/detail —
  // URLs with keys are stripped server-side).
  providers: z
    .array(
      z.object({
        name: z.string(),
        ok: z.boolean(),
        detail: z.string(),
      }),
    )
    .optional(),
  error: z.string().optional(),
});
export type CredentialBulkRowResult = z.infer<typeof CredentialBulkRowResultSchema>;

export const CredentialBulkResponseSchema = z.object({
  dry_run: z.boolean(),
  summary: z.object({
    total: z.number().int(),
    updated: z.number().int(),
    noop: z.number().int(),
    invalid: z.number().int(),
    error: z.number().int(),
  }),
  items: z.array(CredentialBulkRowResultSchema),
});
export type CredentialBulkResponse = z.infer<typeof CredentialBulkResponseSchema>;
