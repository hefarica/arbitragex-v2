/**
 * On-demand ERC20 metadata resolver.
 *
 * When the api-server's LEFT JOIN against `tokens` misses (a brand-new
 * altcoin just appeared in the mempool, the enricher hasn't picked it
 * up yet), this module fetches `symbol()` + `decimals()` directly from
 * the chain via `eth_call`, inserts the result into `tokens`, and
 * returns it in the same request. Operator never sees an unresolved
 * token in the live feed.
 *
 * Design notes:
 *   - Per-process in-memory cache with 60s TTL absorbs bursty repeat
 *     queries for the same token while keeping the cache cold on
 *     restart (no persistence) — the DB row IS the long-term cache.
 *   - HTTP timeout 1500ms per RPC call so a slow upstream cannot stall
 *     the /opportunities/live response beyond a few hundred ms total.
 *   - All RPC errors swallowed to null: the route still renders, the
 *     token just keeps its "we don't know yet" placeholder. R8 fail-honest.
 *   - DB write is best-effort: ON CONFLICT DO NOTHING. If two requests
 *     race the same address, one wins, the other silently no-ops.
 *
 * No-hardcode discipline: chain → RPC URL is read from RPC_HTTP_<chain>
 * env (the same source the searcher uses). No URLs in source.
 */

import type { Pool } from "pg";

interface TokenMeta {
  symbol: string;
  /** F3 (audit §11 RC3): null when the on-chain decimals() call failed —
   *  decimals gate math, not display; a valid symbol still surfaces. */
  decimals: number | null;
}

interface CacheEntry {
  meta: TokenMeta | null;
  fetched_at: number;
}

const CACHE_TTL_MS = 60_000;
// F4 (audit §11 RC4): failed lookups retry sooner — a transient RPC hiccup
// (1.5s timeout, provider 5xx) must not freeze a token as "unresolved" for a
// full minute. Successes keep the 60s TTL.
const NEG_CACHE_TTL_MS = 15_000;
const RPC_TIMEOUT_MS = 1500;
// ERC-20 method selectors. These are protocol-level constants (RULE 00
// exception), not productive secrets.
const SELECTOR_SYMBOL = "0x95d89b41";
const SELECTOR_DECIMALS = "0x313ce567";

const cache = new Map<string, CacheEntry>();

function cacheKey(chainId: number, address: string): string {
  return `${chainId}:${address.toLowerCase()}`;
}

/**
 * Extract the first RPC URL from `RPC_HTTP_<chain_id>` (matches the CSV
 * format `name=url,name=url` that the rest of the system uses). Returns
 * null when no provider is configured for the chain.
 */
function firstRpcUrl(chainId: number): string | null {
  const raw = process.env[`RPC_HTTP_${chainId}`];
  if (!raw) return null;
  for (const tok of raw.split(",")) {
    const t = tok.trim();
    if (!t) continue;
    const url = t.includes("=") ? t.split("=", 2)[1]!.trim() : t;
    if (/^https?:\/\//.test(url)) return url;
  }
  return null;
}

async function rpcCall(rpcUrl: string, body: object): Promise<unknown> {
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), RPC_TIMEOUT_MS);
  try {
    const res = await fetch(rpcUrl, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: ctrl.signal,
    });
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  } finally {
    clearTimeout(t);
  }
}

/**
 * Decode a Solidity `string` return value from an `eth_call` response.
 *
 * Two encodings in the wild:
 *   1. Canonical ABI: offset (32) || length (32) || data (padded).
 *   2. Legacy bytes32: 32 raw bytes, the symbol is the leading
 *      non-null UTF-8 chunk (used by MakerDAO / older tokens like SAI).
 *
 * Returns null when the data is unparseable.
 */
function decodeStringResult(hex: string): string | null {
  if (!hex || hex === "0x") return null;
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  // ABI-encoded string: starts with offset (typically 0x20).
  if (clean.length >= 128) {
    const lenHex = clean.slice(64, 128);
    const len = parseInt(lenHex, 16);
    if (Number.isFinite(len) && len > 0 && len < 256 && clean.length >= 128 + len * 2) {
      const dataHex = clean.slice(128, 128 + len * 2);
      try {
        const bytes = Buffer.from(dataHex, "hex");
        const decoded = bytes.toString("utf8").replace(/\0+$/, "");
        if (decoded && /^[\x20-\x7e]+$/.test(decoded)) return decoded;
      } catch { /* fall through to bytes32 */ }
    }
  }
  // bytes32 legacy: read raw bytes, trim trailing nulls.
  try {
    const bytes = Buffer.from(clean.slice(0, 64), "hex");
    const decoded = bytes.toString("utf8").replace(/\0+$/, "").trim();
    if (decoded && /^[\x20-\x7e]+$/.test(decoded)) return decoded;
  } catch { /* unparseable */ }
  return null;
}

function decodeUint8Result(hex: string): number | null {
  if (!hex || hex === "0x") return null;
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const n = parseInt(clean, 16);
  if (Number.isFinite(n) && n >= 0 && n <= 36) return n;
  return null;
}

interface JsonRpcReply {
  result?: string;
  error?: { message?: string };
}

async function fetchOneTokenMeta(
  rpcUrl: string,
  address: string,
): Promise<TokenMeta | null> {
  const [symRes, decRes] = await Promise.all([
    rpcCall(rpcUrl, {
      jsonrpc: "2.0",
      id: 1,
      method: "eth_call",
      params: [{ to: address, data: SELECTOR_SYMBOL }, "latest"],
    }),
    rpcCall(rpcUrl, {
      jsonrpc: "2.0",
      id: 2,
      method: "eth_call",
      params: [{ to: address, data: SELECTOR_DECIMALS }, "latest"],
    }),
  ]);

  const symHex = (symRes as JsonRpcReply | null)?.result;
  const decHex = (decRes as JsonRpcReply | null)?.result;
  if (!symHex) return null;

  const symbol = decodeStringResult(symHex);
  // Sanity: ERC-20 symbols are short. Reject anything wild.
  if (!symbol || symbol.length > 24) return null;
  // F3 (audit §11 RC3): display-first. Previously ALL-OR-NOTHING — a symbol
  // that decoded fine was discarded when decimals() reverted or returned a
  // value outside [0,36]. Decimals only gate math; the symbol gates display.
  // Keep the symbol, carry decimals as null (persistToken COALESCEs).
  const decimals = decHex ? decodeUint8Result(decHex) : null;
  return { symbol, decimals };
}

async function persistToken(
  pool: Pool,
  chainId: number,
  address: string,
  meta: TokenMeta,
): Promise<void> {
  // tokens.decimals is NOT NULL (migration 021). F3 symbol-only results
  // (decimals() failed) cannot be persisted without breaking the constraint —
  // they live in the 60s in-process cache only, and the enricher (which owns
  // the full on-chain resolution) fills the row later. Dropping the NOT NULL
  // would be its own migration PR if this case proves common.
  if (meta.decimals == null) return;
  try {
    await pool.query(
      `INSERT INTO tokens (chain_id, address, symbol, decimals, resolved_via, resolved_at)
       VALUES ($1, LOWER($2), $3, $4, 'onchain_partial', NOW())
       ON CONFLICT (chain_id, address) DO UPDATE
         SET symbol       = COALESCE(tokens.symbol, EXCLUDED.symbol),
             decimals     = COALESCE(tokens.decimals, EXCLUDED.decimals),
             resolved_via = COALESCE(tokens.resolved_via, EXCLUDED.resolved_via),
             last_seen_at = NOW()`,
      [chainId, address, meta.symbol, meta.decimals],
    );
  } catch {
    // Best-effort write; cache still serves us until DB recovers.
  }
}

/**
 * Resolve a batch of (chain_id, address) pairs in parallel. Returns a
 * Map keyed by `chain_id:lowercase_address`. Missing entries indicate
 * the resolver could not fetch metadata (no RPC, revert, malformed
 * response). Caller should treat missing keys as "unknown" and render
 * the address-only fallback.
 */
export async function resolveTokensOnDemand(
  pool: Pool | null,
  pairs: Array<{ chain_id: number; address: string }>,
): Promise<Map<string, TokenMeta>> {
  const result = new Map<string, TokenMeta>();
  if (pairs.length === 0) return result;

  // Dedupe + cache lookup before any RPC.
  const now = Date.now();
  const toFetch: Array<{ chain_id: number; address: string; key: string }> = [];
  for (const { chain_id, address } of pairs) {
    const key = cacheKey(chain_id, address);
    if (result.has(key)) continue;
    const cached = cache.get(key);
    // F4: TTL depends on outcome — successes stick 60s, misses retry at 15s.
    if (cached && now - cached.fetched_at < (cached.meta ? CACHE_TTL_MS : NEG_CACHE_TTL_MS)) {
      if (cached.meta) result.set(key, cached.meta);
      continue;
    }
    toFetch.push({ chain_id, address, key });
  }

  if (toFetch.length === 0) return result;

  // Fan out — bounded by Promise.allSettled so one slow chain doesn't
  // block the rest.
  const settled = await Promise.allSettled(
    toFetch.map(async (p) => {
      const rpcUrl = firstRpcUrl(p.chain_id);
      if (!rpcUrl) return { key: p.key, meta: null };
      const meta = await fetchOneTokenMeta(rpcUrl, p.address);
      return { key: p.key, meta, chain_id: p.chain_id, address: p.address };
    }),
  );

  for (const r of settled) {
    if (r.status !== "fulfilled") continue;
    const { key, meta } = r.value;
    cache.set(key, { meta, fetched_at: now });
    if (meta) {
      result.set(key, meta);
      // Persist (best-effort) so future requests hit the DB JOIN.
      if (pool && r.value.chain_id != null && r.value.address) {
        void persistToken(pool, r.value.chain_id, r.value.address, meta);
      }
    }
  }

  return result;
}

/** Cache key helper exposed so callers can lookup their own results. */
export function tokenCacheKey(chain_id: number, address: string): string {
  return cacheKey(chain_id, address);
}
