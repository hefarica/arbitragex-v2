/**
 * Ω FE-MASTER · Pair Intelligence contracts (FE-0002 tramo 2 — P5 §13)
 *
 * Canonical domain shape for the per-pair surface derived from
 * fe_normalization.rs (PairAlpha r15) + dirty_pairs.rs (XLS-QB-05) +
 * pool_sync reserves. Identity: a pair is SYMMETRIC as a set {A,B} but the
 * runtime evaluates it DIRECTED — alpha_forward / alpha_reverse NEVER
 * collapse (r15); the canonical leg order (address ascending) is fixed by
 * the BACKEND, never re-sorted client-side.
 *
 * RULE 00 / §28: until EMIT-06 lands there is no wire — the client returns
 * an honest 404 Result error, never a fabricated pair. `null` = not
 * computed (R8), which is different from 0.
 *
 * §79: alpha/F_e are runtime FE-normalization math — the frontend NEVER
 * recomputes rates from reserves; it renders what the payload carries.
 *
 * Envelope proposal (pending EMIT-06): `{ entries }` mirrors the catalog
 * convention (EMIT-07/08); each row self-scopes via its own chain_id.
 */
import { z } from 'zod';
import { ChainIdSchema, EvmAddressSchema, WeiAmountSchema } from './_primitives';
import { TokenKeyRefSchema } from './tokens';

// ─── PoolRef — one row per pool quoting the pair (§13) ───────────────────
export const PoolRefSchema = z.object({
  /** On-chain identity of the pool (hex lowercase). */
  pool_address: EvmAddressSchema,
  /** DEX id/label — canonical ALGORITHMS×DEXES value, backend-named. */
  venue: z.string().min(1),
  /** On-chain read fee (doctrina: fees are NEVER hardcoded). Fractional ok. */
  fee_bps: z.number().finite().min(0),
  /** §62 — u256 reserves never fit IEEE; ALWAYS decimal strings. */
  reserves_a: WeiAmountSchema,
  reserves_b: WeiAmountSchema,
}).strict();
export type PoolRef = z.infer<typeof PoolRefSchema>;

// ─── PairView — the pair as the runtime sees it (§13) ────────────────────
export const PairViewSchema = z.object({
  chain_id: ChainIdSchema,
  /** Canonical leg order (address ascending) is backend-fixed. */
  token_a: TokenKeyRefSchema,
  token_b: TokenKeyRefSchema,
  /** ALL pools quoting the pair, across venues. */
  pools: z.array(PoolRefSchema),
  /** Venues with ≥1 pool of the pair (derived backend-side). */
  venue_count: z.number().int().min(0),
  /** PairAlpha.forward — null = not computed this tick (R8). */
  alpha_forward: z.number().finite().nullable(),
  /** PairAlpha.reverse — NEVER −forward; independently computed (r15). */
  alpha_reverse: z.number().finite().nullable(),
  /** dirty_pairs bitset of the CURRENT tick. */
  dirty: z.boolean(),
  /** Epoch ms of the last reserve update — null honest if never. */
  last_reserve_update: z.number().int().min(0).nullable(),
}).strict();
export type PairView = z.infer<typeof PairViewSchema>;

// ─── EMIT-06 envelope (GET /api/pairs) ────────────────────────────────────
export const PairsResponseSchema = z.object({
  entries: z.array(PairViewSchema),
}).strict();
export type PairsResponse = z.infer<typeof PairsResponseSchema>;
