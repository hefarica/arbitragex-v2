/**
 * Token Validation Engine — Phase 1 orchestrator + persistence.
 *
 * Three layers of caching, hottest to coldest:
 *
 *   1. In-process Map (this module's `cache`):
 *        - 5-minute TTL.
 *        - Absorbs the burst of repeated lookups inside a single
 *          /opportunities/live render (50 rows × 2 tokens = 100 lookups
 *          easily, mostly duplicates).
 *        - Survives across requests; lost on process restart.
 *
 *   2. PostgreSQL `token_validations` table:
 *        - 1-hour `stale_after` TTL.
 *        - Survives restarts. Source of truth for "did we ever validate
 *          this address?".
 *        - Hot path always reads from here.
 *
 *   3. Async validation work:
 *        - Triggered when a (chain_id, address) is requested with no PG
 *          row OR stale_after < NOW().
 *        - Fire-and-forget: the request returns the cached result (or
 *          PENDING) immediately; the validation runs in the background
 *          and writes back to PG. The next request picks it up.
 *
 * R8 fail-honest: when none of the layers have data, we return a
 * PENDING result with score=0 and the dashboard renders "validating…".
 * Never block the request waiting for external APIs.
 */

import type { Pool } from "pg";
import { validateOnChainTruth, type OnChainTruthResult } from "./onChainTruth.js";
import { validateLiquidityReality, type LiquidityRealityResult } from "./liquidityReality.js";
import { composeFinalScore, type FinalScoreResult, type TokenValidationStatus } from "./score.js";
import { verifyToken } from "../tokenRegistry.js";

const IN_PROC_TTL_MS  = 5 * 60 * 1000;
const PG_STALE_AFTER_MS = 60 * 60 * 1000;

export interface TokenValidationRow {
  chain_id: number;
  address: string;
  exists_onchain: boolean | null;
  is_contract: boolean | null;
  bytecode_size: number | null;
  standard: string | null;
  onchain_name: string | null;
  onchain_symbol: string | null;
  onchain_decimals: number | null;
  total_supply_str: string | null;

  pair_count: number | null;
  liquidity_usd: number | null;
  volume_24h_usd: number | null;
  volume_1h_usd: number | null;
  volume_5m_usd: number | null;
  price_usd: number | null;
  primary_dex: string | null;
  primary_pair_address: string | null;

  registry_verified: boolean | null;
  registry_source: string | null;
  registry_symbol: string | null;
  registry_name: string | null;

  final_status: TokenValidationStatus;
  score: number;
  score_reasons: Array<{ key: string; delta: number; note: string }> | null;

  validators_run: string[];
  errors: Record<string, string> | null;
  validated_at: string;       // ISO
  stale_after: string;        // ISO
}

interface CacheEntry {
  row: TokenValidationRow;
  fetched_at: number;
}

const cache = new Map<string, CacheEntry>();
const inflight = new Set<string>();

function cacheKey(chain_id: number, address: string): string {
  return `${chain_id}:${address.toLowerCase()}`;
}

// ── PG persistence ─────────────────────────────────────────────────────────

const SELECT_VALIDATION_SQL = `
SELECT
  chain_id, address,
  exists_onchain, is_contract, bytecode_size,
  standard, onchain_name, onchain_symbol, onchain_decimals, total_supply_str,
  pair_count,
  liquidity_usd::float        AS liquidity_usd,
  volume_24h_usd::float       AS volume_24h_usd,
  volume_1h_usd::float        AS volume_1h_usd,
  volume_5m_usd::float        AS volume_5m_usd,
  price_usd::float            AS price_usd,
  primary_dex, primary_pair_address,
  registry_verified, registry_source, registry_symbol, registry_name,
  final_status, score, score_reasons,
  validators_run, errors,
  validated_at, stale_after
FROM token_validations
WHERE chain_id = $1 AND address = LOWER($2)
LIMIT 1
`.trim();

const UPSERT_VALIDATION_SQL = `
INSERT INTO token_validations (
  chain_id, address,
  exists_onchain, is_contract, bytecode_size,
  standard, onchain_name, onchain_symbol, onchain_decimals, total_supply_str,
  pair_count, liquidity_usd, volume_24h_usd, volume_1h_usd, volume_5m_usd,
  price_usd, primary_dex, primary_pair_address,
  registry_verified, registry_source, registry_symbol, registry_name,
  final_status, score, score_reasons,
  validators_run, errors,
  validated_at, stale_after
) VALUES (
  $1, LOWER($2),
  $3, $4, $5,
  $6, $7, $8, $9, $10,
  $11, $12, $13, $14, $15,
  $16, $17, $18,
  $19, $20, $21, $22,
  $23, $24, $25,
  $26, $27,
  $28, $29
)
ON CONFLICT (chain_id, address) DO UPDATE SET
  exists_onchain       = EXCLUDED.exists_onchain,
  is_contract          = EXCLUDED.is_contract,
  bytecode_size        = EXCLUDED.bytecode_size,
  standard             = EXCLUDED.standard,
  onchain_name         = EXCLUDED.onchain_name,
  onchain_symbol       = EXCLUDED.onchain_symbol,
  onchain_decimals     = EXCLUDED.onchain_decimals,
  total_supply_str     = EXCLUDED.total_supply_str,
  pair_count           = EXCLUDED.pair_count,
  liquidity_usd        = EXCLUDED.liquidity_usd,
  volume_24h_usd       = EXCLUDED.volume_24h_usd,
  volume_1h_usd        = EXCLUDED.volume_1h_usd,
  volume_5m_usd        = EXCLUDED.volume_5m_usd,
  price_usd            = EXCLUDED.price_usd,
  primary_dex          = EXCLUDED.primary_dex,
  primary_pair_address = EXCLUDED.primary_pair_address,
  registry_verified    = EXCLUDED.registry_verified,
  registry_source      = EXCLUDED.registry_source,
  registry_symbol      = EXCLUDED.registry_symbol,
  registry_name        = EXCLUDED.registry_name,
  final_status         = EXCLUDED.final_status,
  score                = EXCLUDED.score,
  score_reasons        = EXCLUDED.score_reasons,
  validators_run       = EXCLUDED.validators_run,
  errors               = EXCLUDED.errors,
  validated_at         = EXCLUDED.validated_at,
  stale_after          = EXCLUDED.stale_after
`.trim();

async function readFromPg(
  pool: Pool,
  chain_id: number,
  address: string,
): Promise<TokenValidationRow | null> {
  try {
    const r = await pool.query<{
      [K in keyof TokenValidationRow]: TokenValidationRow[K] | Date | unknown;
    }>(SELECT_VALIDATION_SQL, [chain_id, address]);
    if (r.rows.length === 0) return null;
    const row = r.rows[0]!;
    return {
      chain_id: Number(row["chain_id"]),
      address: String(row["address"]),
      exists_onchain:    (row["exists_onchain"] as boolean | null) ?? null,
      is_contract:       (row["is_contract"] as boolean | null) ?? null,
      bytecode_size:     row["bytecode_size"] as number | null,
      standard:          row["standard"] as string | null,
      onchain_name:      row["onchain_name"] as string | null,
      onchain_symbol:    row["onchain_symbol"] as string | null,
      onchain_decimals:  row["onchain_decimals"] as number | null,
      total_supply_str:  row["total_supply_str"] as string | null,
      pair_count:        row["pair_count"] as number | null,
      liquidity_usd:     row["liquidity_usd"] as number | null,
      volume_24h_usd:    row["volume_24h_usd"] as number | null,
      volume_1h_usd:     row["volume_1h_usd"] as number | null,
      volume_5m_usd:     row["volume_5m_usd"] as number | null,
      price_usd:         row["price_usd"] as number | null,
      primary_dex:       row["primary_dex"] as string | null,
      primary_pair_address: row["primary_pair_address"] as string | null,
      registry_verified: (row["registry_verified"] as boolean | null) ?? null,
      registry_source:   row["registry_source"] as string | null,
      registry_symbol:   row["registry_symbol"] as string | null,
      registry_name:     row["registry_name"] as string | null,
      final_status:      row["final_status"] as TokenValidationStatus,
      score:             Number(row["score"]),
      score_reasons:     (row["score_reasons"] as Array<{ key: string; delta: number; note: string }>) ?? null,
      validators_run:    Array.isArray(row["validators_run"]) ? (row["validators_run"] as string[]) : [],
      errors:            (row["errors"] as Record<string, string> | null) ?? null,
      validated_at:      row["validated_at"] instanceof Date
                            ? (row["validated_at"] as Date).toISOString()
                            : String(row["validated_at"]),
      stale_after:       row["stale_after"] instanceof Date
                            ? (row["stale_after"] as Date).toISOString()
                            : String(row["stale_after"]),
    };
  } catch {
    return null;
  }
}

async function writeToPg(pool: Pool, row: TokenValidationRow): Promise<void> {
  try {
    await pool.query(UPSERT_VALIDATION_SQL, [
      row.chain_id, row.address,
      row.exists_onchain, row.is_contract, row.bytecode_size,
      row.standard, row.onchain_name, row.onchain_symbol, row.onchain_decimals, row.total_supply_str,
      row.pair_count, row.liquidity_usd, row.volume_24h_usd, row.volume_1h_usd, row.volume_5m_usd,
      row.price_usd, row.primary_dex, row.primary_pair_address,
      row.registry_verified, row.registry_source, row.registry_symbol, row.registry_name,
      row.final_status, row.score, JSON.stringify(row.score_reasons),
      row.validators_run, row.errors == null ? null : JSON.stringify(row.errors),
      row.validated_at, row.stale_after,
    ]);
  } catch {
    // Best-effort persistence. In-process cache still serves until next
    // request — the validation isn't lost, just not durable across restarts.
  }
}

// ── Validation pipeline ────────────────────────────────────────────────────

async function runValidators(
  chain_id: number,
  address: string,
): Promise<TokenValidationRow> {
  const [onChain, liquidity] = await Promise.all([
    validateOnChainTruth(chain_id, address),
    validateLiquidityReality(chain_id, address),
  ]);
  // Registry is local (no I/O); we still treat it as a validator for
  // uniform scoring. Pass the on-chain symbol so impersonation triggers
  // symbol-mismatch.
  const registry = verifyToken(chain_id, address, onChain.onchain_symbol);

  const score: FinalScoreResult = composeFinalScore(onChain, liquidity, registry);

  const validators_run: string[] = [];
  if (onChain.ran) validators_run.push("on-chain-truth");
  if (liquidity.ran) validators_run.push("liquidity-reality");
  validators_run.push("registry");

  const errors: Record<string, string> = {};
  if (onChain.error && onChain.error_message) errors["on-chain-truth"] = onChain.error_message;
  if (liquidity.error && liquidity.error_message) errors["liquidity-reality"] = liquidity.error_message;

  const now = new Date();
  return {
    chain_id,
    address: address.toLowerCase(),
    exists_onchain:    onChain.ran ? onChain.exists_onchain : null,
    is_contract:       onChain.ran ? onChain.is_contract : null,
    bytecode_size:     onChain.bytecode_size,
    standard:          onChain.standard,
    onchain_name:      onChain.onchain_name,
    onchain_symbol:    onChain.onchain_symbol,
    onchain_decimals:  onChain.onchain_decimals,
    total_supply_str:  onChain.total_supply_str,
    pair_count:        liquidity.ran ? liquidity.pair_count : null,
    liquidity_usd:     liquidity.liquidity_usd,
    volume_24h_usd:    liquidity.volume_24h_usd,
    volume_1h_usd:     liquidity.volume_1h_usd,
    volume_5m_usd:     liquidity.volume_5m_usd,
    price_usd:         liquidity.price_usd,
    primary_dex:       liquidity.primary_dex,
    primary_pair_address: liquidity.primary_pair_address,
    registry_verified: registry.verified,
    registry_source:   registry.source,
    registry_symbol:   registry.registry_symbol,
    registry_name:     registry.registry_name,
    final_status:      score.status,
    score:             score.score,
    score_reasons:     score.reasons,
    validators_run,
    errors:            Object.keys(errors).length ? errors : null,
    validated_at:      now.toISOString(),
    stale_after:       new Date(now.getTime() + PG_STALE_AFTER_MS).toISOString(),
  };
}

/**
 * Background validate: runs the full pipeline, persists to PG, primes the
 * in-process cache. Idempotent and deduplicated — if the same (chain_id,
 * address) is requested concurrently, only one validation runs.
 */
async function backgroundValidate(
  pool: Pool | null,
  chain_id: number,
  address: string,
): Promise<TokenValidationRow | null> {
  const key = cacheKey(chain_id, address);
  if (inflight.has(key)) return null;
  inflight.add(key);
  try {
    const row = await runValidators(chain_id, address);
    cache.set(key, { row, fetched_at: Date.now() });
    if (pool) {
      await writeToPg(pool, row);
    }
    return row;
  } finally {
    inflight.delete(key);
  }
}

/**
 * Hot-path read for the api-server route. Returns a result immediately:
 *   - Layer 1 (in-process cache): returned synchronously if fresh.
 *   - Layer 2 (PG): returned if present (even if "stale_after" elapsed;
 *     we'd rather show an hour-old result than block on validation).
 *   - Otherwise: returns a PENDING placeholder and fires off the async
 *     validation in the background. The next request picks up the result.
 *
 * Honest about laziness: the operator's first sighting of a new token
 * shows PENDING. By their next refresh (typically <5s with auto-poll),
 * the real validation is in PG and the dashboard renders the colour.
 */
export async function readTokenValidation(
  pool: Pool | null,
  chain_id: number,
  address: string,
): Promise<TokenValidationRow> {
  const key = cacheKey(chain_id, address);
  const now = Date.now();

  // Layer 1: in-process cache.
  const cached = cache.get(key);
  if (cached && now - cached.fetched_at < IN_PROC_TTL_MS) {
    return cached.row;
  }

  // Layer 2: PG.
  if (pool) {
    const pgRow = await readFromPg(pool, chain_id, address);
    if (pgRow) {
      cache.set(key, { row: pgRow, fetched_at: now });
      const staleAt = new Date(pgRow.stale_after).getTime();
      if (staleAt < now) {
        // Stale — refresh in background but return the cached row now.
        void backgroundValidate(pool, chain_id, address);
      }
      return pgRow;
    }
  }

  // Layer 3: kick off async validation, return PENDING now.
  void backgroundValidate(pool, chain_id, address);
  const pending: TokenValidationRow = {
    chain_id,
    address: address.toLowerCase(),
    exists_onchain: null,
    is_contract: null,
    bytecode_size: null,
    standard: null,
    onchain_name: null,
    onchain_symbol: null,
    onchain_decimals: null,
    total_supply_str: null,
    pair_count: null,
    liquidity_usd: null,
    volume_24h_usd: null,
    volume_1h_usd: null,
    volume_5m_usd: null,
    price_usd: null,
    primary_dex: null,
    primary_pair_address: null,
    registry_verified: null,
    registry_source: null,
    registry_symbol: null,
    registry_name: null,
    final_status: "PENDING",
    score: 0,
    score_reasons: [{ key: "first-sighting", delta: 0, note: "queued for background validation" }],
    validators_run: [],
    errors: null,
    validated_at: new Date(now).toISOString(),
    stale_after: new Date(now).toISOString(),
  };
  // Don't cache PENDING — let the next request hit the live layer again
  // (it'll likely find the freshly-written PG row).
  return pending;
}

/**
 * Read MANY validations in parallel — used by the /opportunities/live
 * route to look up validations for every (token_in, token_out) seen in
 * the result set, deduplicated.
 */
export async function readTokenValidationsBatch(
  pool: Pool | null,
  pairs: Array<{ chain_id: number; address: string }>,
): Promise<Map<string, TokenValidationRow>> {
  // Dedupe — many opportunity rows reference the same token_in (e.g.
  // WETH on Ethereum will repeat across 50 dex_arb rows).
  const unique = new Map<string, { chain_id: number; address: string }>();
  for (const p of pairs) {
    if (!p.address) continue;
    unique.set(cacheKey(p.chain_id, p.address), p);
  }
  const out = new Map<string, TokenValidationRow>();
  await Promise.all(
    Array.from(unique.entries()).map(async ([k, p]) => {
      const row = await readTokenValidation(pool, p.chain_id, p.address);
      out.set(k, row);
    }),
  );
  return out;
}

/** Test-only helper. */
export function _resetValidationCacheForTests(): void {
  cache.clear();
  inflight.clear();
}
