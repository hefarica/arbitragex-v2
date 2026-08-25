/**
 * Ω FE-MASTER · Token Universe contracts (FE-0001)
 *
 * Canonical domain shapes for the Dynamic Token Universe surface
 * (operator directive §4–§6): TokenKey `(chain_id, address)` is the ONLY
 * effective identity — symbols are display labels, never identity.
 *
 * Frontier discipline (RULE 00 / §28): every field the backend does not
 * serve yet is `nullable` — `null` means "not computed / not served",
 * NEVER a fabricated default. Counts are plain integers (safe in IEEE);
 * money is UsdDecimalSchema (decimal string, §62).
 *
 * Anchors:
 *   - §4 AllowedTokenRef (operator directive 2026-08-23)
 *   - §5 quick-resolve preview table
 *   - §6 universe consequence KPIs (C(N,2), N(N−1) — backend-derived,
 *     cross-chain sums via searcher pair_index::within_chain_pairs /
 *     within_chain_directed_edges, ARBX-0028).
 */
import { z } from 'zod';
import { ChainIdSchema, EvmAddressSchema, UsdDecimalSchema } from './_primitives';

// ─── TokenKey — the effective identity (§4) ─────────────────────────────
export const TokenKeySchema = z.object({
  chain_id: ChainIdSchema,
  address: EvmAddressSchema,
}).strict();
export type TokenKey = z.infer<typeof TokenKeySchema>;

export const TokenKeyStringSchema = z
  .string()
  .regex(/^\d+:(0x[0-9a-fA-F]{40})$/, 'TokenKey wire form "<chain_id>:<address>"');
export type TokenKeyString = z.infer<typeof TokenKeyStringSchema>;

// ─── TokenKeyRef — identity + resolved display metadata (P5 shapes §1) ───
/**
 * TokenKey plus the resolved metadata a PairView carries per leg (P5 §13).
 * Field constraints deliberately IDENTICAL to AllowedTokenRef's identity
 * block (a verified EMIT-01 contract — not tightened here): symbol is a
 * display label, never identity (§4); decimals is the on-chain ERC-20
 * value used to render decimal-string reserves.
 */
export const TokenKeyRefSchema = z.object({
  chain_id: ChainIdSchema,
  address: EvmAddressSchema,
  symbol: z.string(),
  decimals: z.number().int().min(0),
}).strict();
export type TokenKeyRef = z.infer<typeof TokenKeyRefSchema>;

// ─── Resolution lifecycle (§4/§5) ────────────────────────────────────────
export const TokenResolutionStatusSchema = z.enum([
  'RESOLVED',
  'AMBIGUOUS',
  'NOT_FOUND',
  'UNSUPPORTED',
]);
export type TokenResolutionStatus = z.infer<typeof TokenResolutionStatusSchema>;

/**
 * Canonical allowlist entry. Identity = TokenKey; `symbol`/`decimals` are
 * resolved display metadata. Entries that are not RESOLVED MUST NOT be
 * saved as active tokens (§5) — the UI blocks, not this schema.
 */
export const AllowedTokenRefSchema = z.object({
  chain_id: ChainIdSchema,
  address: EvmAddressSchema,
  symbol: z.string(),
  decimals: z.number().int().min(0),
  token_id: z.number().int().min(0).nullable().optional(),
  allowed: z.boolean(),
  resolution_status: TokenResolutionStatusSchema,
}).strict();
export type AllowedTokenRef = z.infer<typeof AllowedTokenRefSchema>;

// ─── Quick-resolve preview row (§5 table) ────────────────────────────────
export const TokenResolvePreviewRowSchema = z.object({
  input_symbol: z.string().min(1),
  chain_id: ChainIdSchema,
  /** Null until RESOLVED — the row shows the honest state, not a guess. */
  address: EvmAddressSchema.nullable(),
  decimals: z.number().int().min(0).nullable(),
  pool_count: z.number().int().min(0).nullable(),
  venue_count: z.number().int().min(0).nullable(),
  liquidity_usd: UsdDecimalSchema.nullable(),
  resolution_status: TokenResolutionStatusSchema,
}).strict();
export type TokenResolvePreviewRow = z.infer<typeof TokenResolvePreviewRowSchema>;

// ─── Universe consequence KPIs (§6) ──────────────────────────────────────
/**
 * All KPIs nullable: the frontend renders "—" when the backend has not
 * served the derived value yet (R8). Pair/edge math is computed backend-side
 * (never in React, §79) — including cross-chain sums over chain_sizes.
 */
export const TokenUniverseKpiSchema = z.object({
  allowed_tokens: z.number().int().min(0).nullable(),
  possible_pairs: z.number().int().min(0).nullable(), // Σ_c C(N_c,2)
  directed_token_pairs: z.number().int().min(0).nullable(), // Σ_c N_c(N_c−1)
  active_pools: z.number().int().min(0).nullable(),
  active_venues: z.number().int().min(0).nullable(),
  graph_version: z.number().int().min(0).nullable(),
  universe_version: z.number().int().min(0).nullable(),
}).strict();
export type TokenUniverseKpi = z.infer<typeof TokenUniverseKpiSchema>;

// ─── Resolve endpoint (§5 flow; wire contract for ARBX-FE-EMIT-01) ───────
/**
 * Divergence vs FE-P3-P4-DOMAIN-SHAPES.md §2 (flagged to d9, 7b review):
 * result rows carry `input_symbol` + a NULLABLE address — AMBIGUOUS /
 * NOT_FOUND symbols have no address to report, and the response must show
 * them honestly instead of dropping them. Dedupe by TokenKey happens
 * backend-side; one row per REQUESTED symbol.
 */
export const TokenResolveRequestSchema = z.object({
  chain_id: ChainIdSchema,
  symbols: z.array(z.string().min(1)).min(1).max(200),
}).strict();
export type TokenResolveRequest = z.infer<typeof TokenResolveRequestSchema>;

export const TokenResolveResponseSchema = z.object({
  results: z.array(TokenResolvePreviewRowSchema),
  universe: TokenUniverseKpiSchema,
}).strict();
export type TokenResolveResponse = z.infer<typeof TokenResolveResponseSchema>;
